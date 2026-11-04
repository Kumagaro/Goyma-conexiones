//! Pestaña de notificaciones (NT-01, NT-04) y feed de toasts (TO-02).
//! Una sola lista compartida: leer o eliminar cambia para todos. El
//! Contador no ve las de `audience = 'admin'` (N-13).

use chrono::{DateTime, NaiveDate, Utc};
use rusqlite::{params, Connection};

use crate::commands::auth::{require_session, Actor};
use crate::db::connection::{current_app_data_dir, lock_conn};
use crate::errors::{log_error, map_db_error, AppError};
use crate::models::notification::{NotificationFilters, NotificationItem};
use crate::text::normalize_text;
use crate::timestamps::{parse_utc_timestamp, today_local};

const NOTIFICATION_NOT_FOUND: &str = "La notificación ya no existe";
const MAX_LISTED: i64 = 500;

/// Condición SQL de visibilidad para el rol de la sesión.
fn visible_to(actor: &Actor) -> &'static str {
    if actor.role() == "administrador" {
        "1 = 1"
    } else {
        "audience = 'all'"
    }
}

fn read_items(
    connection: &Connection,
    sql: &str,
    args: impl rusqlite::Params,
    operation: &str,
) -> Result<Vec<NotificationItem>, AppError> {
    connection
        .prepare(sql)
        .and_then(|mut statement| {
            statement
                .query_map(args, |row| {
                    Ok(NotificationItem {
                        id: row.get(0)?,
                        notification_type: row.get(1)?,
                        priority: row.get(2)?,
                        message: row.get(3)?,
                        entity_type: row.get(4)?,
                        entity_id: row.get(5)?,
                        is_read: row.get::<_, i64>(6)? == 1,
                        created_at: row.get(7)?,
                    })
                })?
                .collect()
        })
        .map_err(|error| map_db_error(error, operation, &[]))
}

const COLUMNS: &str = "id, type, priority, message, entityType, entityId, isRead, createdAt";

fn validation_failed(message: &str, cause: &str) -> AppError {
    AppError::new("validationFailed", message, cause)
}

/// Filtra en Rust: el buscador ignora mayúsculas y acentos, y la fecha es la
/// local del momento en que se generó.
fn matching(
    items: Vec<NotificationItem>,
    filters: &NotificationFilters,
) -> Result<Vec<NotificationItem>, AppError> {
    let search = filters
        .search
        .as_deref()
        .map(normalize_text)
        .filter(|text| !text.is_empty());
    let date = match filters.date.as_deref().filter(|date| !date.is_empty()) {
        Some(raw) => Some(NaiveDate::parse_from_str(raw, "%Y-%m-%d").map_err(|_| {
            validation_failed("La fecha no es válida", "invalid notification date filter")
        })?),
        None => None,
    };
    Ok(items
        .into_iter()
        .filter(|item| !filters.unread_only || !item.is_read)
        .filter(|item| {
            search
                .as_deref()
                .is_none_or(|text| normalize_text(&item.message).contains(text))
        })
        .filter(|item| {
            date.is_none_or(|date| {
                parse_utc_timestamp(&item.created_at).map(today_local) == Some(date)
            })
        })
        .collect())
}

fn visible_items(
    connection: &Connection,
    actor: &Actor,
    filters: &NotificationFilters,
    operation: &str,
) -> Result<Vec<NotificationItem>, AppError> {
    let items = read_items(
        connection,
        &format!(
            "SELECT {COLUMNS} FROM notifications WHERE {} ORDER BY createdAt DESC, id DESC",
            visible_to(actor)
        ),
        [],
        operation,
    )?;
    matching(items, filters)
}

pub fn list_notifications_inner(
    connection: &mut Connection,
    now: DateTime<Utc>,
    filters: NotificationFilters,
) -> Result<Vec<NotificationItem>, AppError> {
    let actor = require_session(connection, now)?;
    let mut items = visible_items(connection, &actor, &filters, "listNotifications")?;
    items.truncate(MAX_LISTED as usize);
    Ok(items)
}

pub fn count_unread_notifications_inner(
    connection: &mut Connection,
    now: DateTime<Utc>,
) -> Result<i64, AppError> {
    let actor = require_session(connection, now)?;
    connection
        .query_row(
            &format!(
                "SELECT COUNT(*) FROM notifications WHERE isRead = 0 AND {}",
                visible_to(&actor)
            ),
            [],
            |row| row.get(0),
        )
        .map_err(|error| map_db_error(error, "countUnreadNotifications", &[]))
}

/// Las generadas después de `after_id`, en orden de llegada (TO-02).
pub fn list_notifications_since_inner(
    connection: &mut Connection,
    now: DateTime<Utc>,
    after_id: i64,
) -> Result<Vec<NotificationItem>, AppError> {
    let actor = require_session(connection, now)?;
    read_items(
        connection,
        &format!(
            "SELECT {COLUMNS} FROM notifications WHERE id > ?1 AND {} ORDER BY id LIMIT ?2",
            visible_to(&actor)
        ),
        params![after_id, MAX_LISTED],
        "listNotificationsSince",
    )
}

fn update_visible(
    connection: &Connection,
    actor: &Actor,
    sql: &str,
    id: i64,
    operation: &str,
) -> Result<(), AppError> {
    let changed = connection
        .execute(&format!("{sql} AND {}", visible_to(actor)), [id])
        .map_err(|error| map_db_error(error, operation, &[]))?;
    if changed == 0 {
        return Err(AppError::new(
            "notFound",
            NOTIFICATION_NOT_FOUND,
            &format!("{operation}: notification missing or not visible"),
        ));
    }
    Ok(())
}

pub fn mark_notification_read_inner(
    connection: &mut Connection,
    now: DateTime<Utc>,
    notification_id: i64,
) -> Result<(), AppError> {
    let actor = require_session(connection, now)?;
    update_visible(
        connection,
        &actor,
        "UPDATE notifications SET isRead = 1 WHERE id = ?1",
        notification_id,
        "markNotificationRead",
    )
}

pub fn mark_all_notifications_read_inner(
    connection: &mut Connection,
    now: DateTime<Utc>,
) -> Result<usize, AppError> {
    let actor = require_session(connection, now)?;
    connection
        .execute(
            &format!(
                "UPDATE notifications SET isRead = 1 WHERE isRead = 0 AND {}",
                visible_to(&actor)
            ),
            [],
        )
        .map_err(|error| map_db_error(error, "markAllNotificationsRead", &[]))
}

pub fn delete_notification_inner(
    connection: &mut Connection,
    now: DateTime<Utc>,
    notification_id: i64,
) -> Result<(), AppError> {
    let actor = require_session(connection, now)?;
    update_visible(
        connection,
        &actor,
        "DELETE FROM notifications WHERE id = ?1",
        notification_id,
        "deleteNotification",
    )
}

/// "Eliminar todas": las que el usuario ve con los filtros actuales (NT-01).
pub fn delete_notifications_inner(
    connection: &mut Connection,
    now: DateTime<Utc>,
    filters: NotificationFilters,
) -> Result<usize, AppError> {
    let actor = require_session(connection, now)?;
    let transaction = connection
        .transaction()
        .map_err(|error| map_db_error(error, "deleteNotifications", &[]))?;
    let items = visible_items(&transaction, &actor, &filters, "deleteNotifications")?;
    for item in &items {
        transaction
            .execute("DELETE FROM notifications WHERE id = ?1", [item.id])
            .map_err(|error| map_db_error(error, "deleteNotifications", &[]))?;
    }
    transaction
        .commit()
        .map_err(|error| map_db_error(error, "deleteNotifications", &[]))?;
    Ok(items.len())
}

fn log_command_error<T>(
    connection: &Connection,
    operation: &str,
    result: &Result<T, AppError>,
    now: DateTime<Utc>,
) {
    if let Err(error) = result {
        if error.should_log() {
            if let Some(app_data_dir) = current_app_data_dir() {
                log_error(
                    connection,
                    "notifications",
                    operation,
                    error,
                    now,
                    app_data_dir,
                );
            }
        }
    }
}

#[tauri::command]
pub fn list_notifications(
    state: tauri::State<'_, std::sync::Mutex<Connection>>,
    filters: Option<NotificationFilters>,
) -> Result<Vec<NotificationItem>, AppError> {
    let mut connection = lock_conn(&state);
    let now = Utc::now();
    let result = list_notifications_inner(&mut connection, now, filters.unwrap_or_default());
    log_command_error(&connection, "listNotifications", &result, now);
    result
}

#[tauri::command]
pub fn count_unread_notifications(
    state: tauri::State<'_, std::sync::Mutex<Connection>>,
) -> Result<i64, AppError> {
    let mut connection = lock_conn(&state);
    let now = Utc::now();
    let result = count_unread_notifications_inner(&mut connection, now);
    log_command_error(&connection, "countUnreadNotifications", &result, now);
    result
}

#[tauri::command]
pub fn list_notifications_since(
    state: tauri::State<'_, std::sync::Mutex<Connection>>,
    after_id: i64,
) -> Result<Vec<NotificationItem>, AppError> {
    let mut connection = lock_conn(&state);
    let now = Utc::now();
    let result = list_notifications_since_inner(&mut connection, now, after_id);
    log_command_error(&connection, "listNotificationsSince", &result, now);
    result
}

#[tauri::command]
pub fn mark_notification_read(
    state: tauri::State<'_, std::sync::Mutex<Connection>>,
    notification_id: i64,
) -> Result<(), AppError> {
    let mut connection = lock_conn(&state);
    let now = Utc::now();
    let result = mark_notification_read_inner(&mut connection, now, notification_id);
    log_command_error(&connection, "markNotificationRead", &result, now);
    result
}

#[tauri::command]
pub fn mark_all_notifications_read(
    state: tauri::State<'_, std::sync::Mutex<Connection>>,
) -> Result<usize, AppError> {
    let mut connection = lock_conn(&state);
    let now = Utc::now();
    let result = mark_all_notifications_read_inner(&mut connection, now);
    log_command_error(&connection, "markAllNotificationsRead", &result, now);
    result
}

#[tauri::command]
pub fn delete_notification(
    state: tauri::State<'_, std::sync::Mutex<Connection>>,
    notification_id: i64,
) -> Result<(), AppError> {
    let mut connection = lock_conn(&state);
    let now = Utc::now();
    let result = delete_notification_inner(&mut connection, now, notification_id);
    log_command_error(&connection, "deleteNotification", &result, now);
    result
}

#[tauri::command]
pub fn delete_notifications(
    state: tauri::State<'_, std::sync::Mutex<Connection>>,
    filters: Option<NotificationFilters>,
) -> Result<usize, AppError> {
    let mut connection = lock_conn(&state);
    let now = Utc::now();
    let result = delete_notifications_inner(&mut connection, now, filters.unwrap_or_default());
    log_command_error(&connection, "deleteNotifications", &result, now);
    result
}

#[cfg(test)]
mod tests {
    use chrono::{DateTime, Duration, TimeZone, Utc};
    use rusqlite::Connection;

    use super::{
        count_unread_notifications_inner, delete_notification_inner, delete_notifications_inner,
        list_notifications_inner, list_notifications_since_inner,
        mark_all_notifications_read_inner, mark_notification_read_inner,
    };
    use crate::models::notification::NotificationFilters;
    use crate::test_utils::{
        assert_allows_both_roles, assert_requires_session, create_test_session, create_test_user,
        setup_test_db,
    };
    use crate::timestamps::format_utc_timestamp;

    fn now() -> DateTime<Utc> {
        Utc.with_ymd_and_hms(2026, 10, 28, 16, 0, 0).unwrap()
    }

    fn session(connection: &Connection, role: &str) {
        let id = create_test_user(
            connection,
            "notifuser",
            role,
            "+5350000040",
            "12345678940",
            "ValidPass1!",
        );
        create_test_session(connection, id, false, now(), now());
    }

    fn insert(
        connection: &Connection,
        message: &str,
        audience: &str,
        created_at: DateTime<Utc>,
    ) -> i64 {
        connection
            .execute(
                "INSERT INTO notifications (type, priority, message, audience, createdAt) VALUES ('N-03', 'ADVERTENCIA', ?1, ?2, ?3)",
                rusqlite::params![message, audience, format_utc_timestamp(created_at)],
            )
            .unwrap();
        connection.last_insert_rowid()
    }

    /// Tres para todos (una ayer) y una de usuarios solo para el Administrador.
    fn seed(connection: &Connection) -> [i64; 4] {
        [
            insert(
                connection,
                "Producto con stock bajo: Café",
                "all",
                now() - Duration::days(1),
            ),
            insert(
                connection,
                "Producto agotado: Azúcar",
                "all",
                now() - Duration::hours(2),
            ),
            insert(
                connection,
                "Producto agotado: Leche",
                "all",
                now() - Duration::hours(1),
            ),
            insert(connection, "Usuario creado: ana", "admin", now()),
        ]
    }

    fn filters(unread_only: bool, search: Option<&str>, date: Option<&str>) -> NotificationFilters {
        NotificationFilters {
            unread_only,
            search: search.map(str::to_owned),
            date: date.map(str::to_owned),
        }
    }

    fn messages(items: &[crate::models::notification::NotificationItem]) -> Vec<&str> {
        items.iter().map(|item| item.message.as_str()).collect()
    }

    #[test]
    fn list_filters_by_read_state_text_without_accents_and_local_date_newest_first() {
        let mut connection = setup_test_db();
        session(&connection, "administrador");
        let [_, sugar, ..] = seed(&connection);

        let all =
            list_notifications_inner(&mut connection, now(), filters(false, None, None)).unwrap();
        assert_eq!(
            messages(&all),
            vec![
                "Usuario creado: ana",
                "Producto agotado: Leche",
                "Producto agotado: Azúcar",
                "Producto con stock bajo: Café",
            ]
        );
        assert_eq!(all[1].priority, "ADVERTENCIA");
        assert!(!all[1].is_read);

        mark_notification_read_inner(&mut connection, now(), sugar).unwrap();
        let unread =
            list_notifications_inner(&mut connection, now(), filters(true, Some("AGOTADO"), None))
                .unwrap();
        assert_eq!(messages(&unread), vec!["Producto agotado: Leche"]);
        let accent =
            list_notifications_inner(&mut connection, now(), filters(false, Some("azucar"), None))
                .unwrap();
        assert_eq!(messages(&accent), vec!["Producto agotado: Azúcar"]);
        let yesterday = list_notifications_inner(
            &mut connection,
            now(),
            filters(false, None, Some("2026-10-27")),
        )
        .unwrap();
        assert_eq!(messages(&yesterday), vec!["Producto con stock bajo: Café"]);

        let error = list_notifications_inner(
            &mut connection,
            now(),
            filters(false, None, Some("27/10/2026")),
        )
        .unwrap_err();
        assert_eq!(error.code, "validationFailed");
    }

    #[test]
    fn the_counter_neither_sees_nor_counts_nor_touches_admin_only_notifications() {
        let mut connection = setup_test_db();
        session(&connection, "contador");
        let [.., admin_only] = seed(&connection);

        let items =
            list_notifications_inner(&mut connection, now(), NotificationFilters::default())
                .unwrap();
        assert_eq!(items.len(), 3);
        assert!(items.iter().all(|item| item.id != admin_only));
        assert_eq!(
            count_unread_notifications_inner(&mut connection, now()).unwrap(),
            3
        );
        assert_eq!(
            list_notifications_since_inner(&mut connection, now(), 0)
                .unwrap()
                .len(),
            3
        );
        assert_eq!(
            mark_notification_read_inner(&mut connection, now(), admin_only)
                .unwrap_err()
                .code,
            "notFound"
        );
        assert_eq!(
            delete_notification_inner(&mut connection, now(), admin_only)
                .unwrap_err()
                .code,
            "notFound"
        );

        assert_eq!(
            mark_all_notifications_read_inner(&mut connection, now()).unwrap(),
            3
        );
        assert_eq!(
            delete_notifications_inner(&mut connection, now(), NotificationFilters::default())
                .unwrap(),
            3
        );
        // La de usuarios sigue intacta y sin leer para el Administrador.
        let remaining: (i64, i64) = connection
            .query_row(
                "SELECT COUNT(*), SUM(isRead) FROM notifications",
                [],
                |row| Ok((row.get(0)?, row.get(1)?)),
            )
            .unwrap();
        assert_eq!(remaining, (1, 0));
    }

    #[test]
    fn since_returns_only_newer_ones_in_arrival_order_and_counts_unread() {
        let mut connection = setup_test_db();
        session(&connection, "administrador");
        let [first, second, ..] = seed(&connection);

        let newer = list_notifications_since_inner(&mut connection, now(), first).unwrap();
        assert_eq!(newer.first().map(|item| item.id), Some(second));
        assert_eq!(newer.len(), 3);
        assert_eq!(
            count_unread_notifications_inner(&mut connection, now()).unwrap(),
            4
        );
        mark_notification_read_inner(&mut connection, now(), second).unwrap();
        assert_eq!(
            count_unread_notifications_inner(&mut connection, now()).unwrap(),
            3
        );
    }

    #[test]
    fn delete_one_and_delete_the_filtered_ones_without_touching_the_rest() {
        let mut connection = setup_test_db();
        session(&connection, "administrador");
        let [coffee, ..] = seed(&connection);

        delete_notification_inner(&mut connection, now(), coffee).unwrap();
        assert_eq!(
            delete_notification_inner(&mut connection, now(), coffee)
                .unwrap_err()
                .code,
            "notFound"
        );
        let deleted = delete_notifications_inner(
            &mut connection,
            now(),
            filters(false, Some("agotado"), None),
        )
        .unwrap();
        assert_eq!(deleted, 2);
        let left = list_notifications_inner(&mut connection, now(), NotificationFilters::default())
            .unwrap();
        assert_eq!(messages(&left), vec!["Usuario creado: ana"]);
    }

    // Matriz de autorización (`Agents.md` §3): sesión obligatoria, ambos roles.
    #[test]
    fn notification_commands_require_a_session_and_allow_both_roles() {
        assert_requires_session(|c, n| {
            list_notifications_inner(c, n, NotificationFilters::default())
        });
        assert_allows_both_roles(|c, n| {
            list_notifications_inner(c, n, NotificationFilters::default())
        });
        assert_requires_session(count_unread_notifications_inner);
        assert_allows_both_roles(count_unread_notifications_inner);
        assert_requires_session(|c, n| list_notifications_since_inner(c, n, 0));
        assert_allows_both_roles(|c, n| list_notifications_since_inner(c, n, 0));
        assert_requires_session(|c, n| mark_notification_read_inner(c, n, 1));
        assert_allows_both_roles(|c, n| mark_notification_read_inner(c, n, 1));
        assert_requires_session(mark_all_notifications_read_inner);
        assert_allows_both_roles(mark_all_notifications_read_inner);
        assert_requires_session(|c, n| delete_notification_inner(c, n, 1));
        assert_allows_both_roles(|c, n| delete_notification_inner(c, n, 1));
        assert_requires_session(|c, n| {
            delete_notifications_inner(c, n, NotificationFilters::default())
        });
        assert_allows_both_roles(|c, n| {
            delete_notifications_inner(c, n, NotificationFilters::default())
        });
    }
}
