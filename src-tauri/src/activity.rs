//! Registro de actividad (RA-01, RA-02). Cada entrada se escribe en la misma
//! transacción que la acción: si la acción falla, no queda entrada. La
//! descripción nunca lleva datos de contacto, solo nombres y números de
//! documento.

use chrono::{DateTime, NaiveDate, Utc};
use rusqlite::{params, Connection};

use crate::commands::auth::{require_admin, Author};
use crate::db::connection::{current_app_data_dir, lock_conn};
use crate::errors::{log_error, map_db_error, AppError};
use crate::models::activity::{ActivityFilters, ActivityItem};
use crate::text::normalize_text;
use crate::timestamps::{format_utc_timestamp, parse_utc_timestamp, today_local};

pub struct Entry<'a> {
    pub action: &'a str,
    pub entity_type: &'a str,
    pub label: &'a str,
    pub details: Option<&'a str>,
}

pub fn record_activity(
    connection: &Connection,
    author: &Author,
    now: DateTime<Utc>,
    entry: Entry,
) -> rusqlite::Result<()> {
    record_as(connection, author.id(), author.name(), now, entry)
}

/// Para una base donde el usuario puede no existir (RP-03).
pub fn record_as(
    connection: &Connection,
    user_id: Option<i64>,
    user_name: &str,
    now: DateTime<Utc>,
    entry: Entry,
) -> rusqlite::Result<()> {
    connection.execute(
        "INSERT INTO activityLog (createdAt, userId, userName, action, entityType, entityLabel, details)
         VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7)",
        params![
            format_utc_timestamp(now),
            user_id,
            user_name,
            entry.action,
            entry.entity_type,
            entry.label,
            entry.details,
        ],
    )?;
    Ok(())
}

/// Entrada de una eliminación; se llama antes del `DELETE` para leer el
/// nombre con `name_sql` (`?1` = id).
pub fn record_deletion(
    connection: &Connection,
    author: &Author,
    now: DateTime<Utc>,
    entity_type: &str,
    name_sql: &str,
    id: i64,
) -> rusqlite::Result<()> {
    let label: String = connection.query_row(name_sql, [id], |row| row.get(0))?;
    record_activity(
        connection,
        author,
        now,
        Entry {
            action: "delete",
            entity_type,
            label: &label,
            details: None,
        },
    )
}

fn parse_filter_date(raw: Option<&str>) -> Result<Option<NaiveDate>, AppError> {
    let Some(raw) = raw.filter(|date| !date.is_empty()) else {
        return Ok(None);
    };
    NaiveDate::parse_from_str(raw, "%Y-%m-%d")
        .map(Some)
        .map_err(|_| {
            AppError::new(
                "validationFailed",
                "La fecha no es válida",
                "invalid activity date filter",
            )
        })
}

pub fn list_activity_log_inner(
    connection: &mut Connection,
    now: DateTime<Utc>,
    filters: ActivityFilters,
) -> Result<Vec<ActivityItem>, AppError> {
    require_admin(connection, now)?;
    let from = parse_filter_date(filters.from_date.as_deref())?;
    let to = parse_filter_date(filters.to_date.as_deref())?;
    let order = if filters.oldest_first { "ASC" } else { "DESC" };
    let read = || -> rusqlite::Result<Vec<ActivityItem>> {
        connection
            .prepare(&format!(
                "SELECT id, createdAt, userName, action, entityType, entityLabel, details
                 FROM activityLog ORDER BY createdAt {order}, id {order}"
            ))?
            .query_map([], |row| {
                Ok(ActivityItem {
                    id: row.get(0)?,
                    created_at: row.get(1)?,
                    user_name: row.get(2)?,
                    action: row.get(3)?,
                    entity_type: row.get(4)?,
                    entity_label: row.get(5)?,
                    details: row.get(6)?,
                })
            })?
            .collect()
    };
    let items = read().map_err(|error| map_db_error(error, "listActivityLog", &[]))?;

    let selected = |value: &Option<String>| value.clone().filter(|text| !text.is_empty());
    let user_name = selected(&filters.user_name);
    let action = selected(&filters.action);
    let search = filters
        .search
        .as_deref()
        .map(normalize_text)
        .filter(|text| !text.is_empty());
    Ok(items
        .into_iter()
        .filter(|item| {
            user_name
                .as_ref()
                .is_none_or(|name| *name == item.user_name)
        })
        .filter(|item| action.as_ref().is_none_or(|kind| *kind == item.action))
        .filter(|item| {
            // La fecha es la local del momento en que ocurrió.
            let date = parse_utc_timestamp(&item.created_at).map(today_local);
            from.is_none_or(|from| date.is_some_and(|date| date >= from))
                && to.is_none_or(|to| date.is_some_and(|date| date <= to))
        })
        .filter(|item| {
            search.as_deref().is_none_or(|text| {
                normalize_text(&item.entity_label).contains(text)
                    || item
                        .details
                        .as_deref()
                        .is_some_and(|details| normalize_text(details).contains(text))
            })
        })
        .collect())
}

#[tauri::command]
pub fn list_activity_log(
    state: tauri::State<'_, std::sync::Mutex<Connection>>,
    filters: ActivityFilters,
) -> Result<Vec<ActivityItem>, AppError> {
    let mut connection = lock_conn(&state);
    let now = Utc::now();
    let result = list_activity_log_inner(&mut connection, now, filters);
    if let Err(error) = &result {
        if error.should_log() {
            if let Some(app_data_dir) = current_app_data_dir() {
                log_error(
                    &connection,
                    "activity",
                    "listActivityLog",
                    error,
                    now,
                    app_data_dir,
                );
            }
        }
    }
    result
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::commands::categories::delete_category_inner;
    use crate::commands::clients::delete_client_inner;
    use crate::commands::conversions::delete_conversion_inner;
    use crate::commands::invoices::{
        change_invoice_status_inner, delete_invoice_inner, expire_reservations,
    };
    use crate::commands::products::{delete_product_inner, purge_expired_products};
    use crate::commands::settings::{
        save_company_logo_inner, update_company_profile_inner, update_exchange_rate_inner,
        update_settings_inner,
    };
    use crate::commands::suppliers::delete_supplier_inner;
    use crate::commands::units::delete_unit_inner;
    use crate::commands::users::delete_user_inner;
    use crate::models::settings::{UpdateCompanyProfileInput, UpdateSettingsInput};
    use crate::test_utils::{
        assert_requires_admin, create_fail_product_delete_trigger, create_test_session,
        create_test_user, insert_category_fixture, insert_conversion_fixture,
        insert_product_fixture, insert_unit_fixture, setup_test_db,
    };

    type Row = (Option<i64>, String, String, String, String, Option<String>);

    fn entries(connection: &Connection) -> Vec<Row> {
        connection
            .prepare(
                "SELECT userId, userName, action, entityType, entityLabel, details
                 FROM activityLog ORDER BY id",
            )
            .unwrap()
            .query_map([], |row| {
                Ok((
                    row.get(0)?,
                    row.get(1)?,
                    row.get(2)?,
                    row.get(3)?,
                    row.get(4)?,
                    row.get(5)?,
                ))
            })
            .unwrap()
            .collect::<rusqlite::Result<_>>()
            .unwrap()
    }

    fn labels(connection: &Connection) -> Vec<(String, String, String)> {
        entries(connection)
            .into_iter()
            .map(|row| (row.2, row.3, row.4))
            .collect()
    }

    fn triple(action: &str, entity: &str, label: &str) -> (String, String, String) {
        (action.to_owned(), entity.to_owned(), label.to_owned())
    }

    fn signed_in(role: &str) -> (Connection, i64, DateTime<Utc>) {
        let now = Utc::now();
        let connection = setup_test_db();
        let user_id = create_test_user(
            &connection,
            "actuser",
            role,
            "+5350000040",
            "12345678940",
            "ValidPass1!",
        );
        create_test_session(&connection, user_id, false, now, now);
        (connection, user_id, now)
    }

    fn insert_contact(connection: &Connection, table: &str, first: &str, id: &str) -> i64 {
        connection
            .execute(
                &format!(
                    "INSERT INTO {table} (firstName, lastName, nationalId, phone, createdAt, updatedAt)
                     VALUES (?1, 'Pérez', ?2, '+5355555555', '2026-10-03T00:00:00Z', '2026-10-03T00:00:00Z')"
                ),
                (first, id),
            )
            .unwrap();
        connection.last_insert_rowid()
    }

    fn insert_sale(connection: &Connection, number: &str, reserved_until: Option<&str>) -> i64 {
        connection
            .execute(
                "INSERT INTO invoices (type, number, invoiceDate, status, contactFullName,
                    contactFullNameNormalized, contactNationalId, contactPhone, createdByName,
                    createdAt, updatedAt, reservedUntil)
                 VALUES ('V', ?1, '2020-01-01', 'pending', 'Ana Pérez', 'ana perez', '90010112345',
                    '+5355555555', 'actuser', '2020-01-01T00:00:00Z', '2020-01-01T00:00:00Z', ?2)",
                (number, reserved_until),
            )
            .unwrap();
        connection.last_insert_rowid()
    }

    #[test]
    fn deletions_of_each_entity_are_recorded_with_their_author() {
        let (mut connection, user_id, now) = signed_in("administrador");
        let category = insert_category_fixture(&connection, "Cables", None);
        let unit = insert_unit_fixture(&connection, "Metro", "m");
        let box_unit = insert_unit_fixture(&connection, "Rollo", "r");
        let conversion = insert_conversion_fixture(&connection, box_unit, unit, 100_000_000);
        let client = insert_contact(&connection, "clients", "Ana", "90010112345");
        let supplier = insert_contact(&connection, "suppliers", "Luis", "85020254321");
        let product = insert_product_fixture(
            &connection,
            "Bomba",
            None,
            None,
            100,
            0,
            0,
            "active",
            None,
            None,
        );
        let other_user = create_test_user(
            &connection,
            "borrado",
            "contador",
            "+5350000041",
            "12345678941",
            "ValidPass1!",
        );
        let invoice = insert_sale(&connection, "V-00001", None);

        delete_conversion_inner(&mut connection, now, conversion).unwrap();
        delete_category_inner(&mut connection, now, category).unwrap();
        delete_unit_inner(&mut connection, now, unit).unwrap();
        delete_client_inner(&mut connection, now, client).unwrap();
        delete_supplier_inner(&mut connection, now, supplier).unwrap();
        delete_product_inner(&mut connection, now, product).unwrap();
        delete_user_inner(&mut connection, now, other_user).unwrap();
        delete_invoice_inner(&mut connection, now, invoice).unwrap();

        assert_eq!(
            labels(&connection),
            vec![
                triple("delete", "conversion", "Rollo → Metro"),
                triple("delete", "category", "Cables"),
                triple("delete", "unit", "Metro"),
                triple("delete", "client", "Ana Pérez"),
                triple("delete", "supplier", "Luis Pérez"),
                triple("delete", "product", "Bomba"),
                triple("delete", "user", "borrado"),
                triple("delete", "invoice", "V-00001"),
            ]
        );
        for row in entries(&connection) {
            assert_eq!(row.0, Some(user_id));
            assert_eq!(row.1, "actuser");
        }
    }

    #[test]
    fn a_failed_action_leaves_no_entry() {
        let (mut connection, _, now) = signed_in("contador");
        let product = insert_product_fixture(
            &connection,
            "Bomba",
            None,
            None,
            100,
            0,
            0,
            "active",
            None,
            None,
        );
        create_fail_product_delete_trigger(&connection).unwrap();

        assert!(delete_product_inner(&mut connection, now, product).is_err());
        assert!(delete_category_inner(&mut connection, now, 999).is_err());
        assert!(entries(&connection).is_empty());
    }

    #[test]
    fn cancellations_are_recorded_by_the_user_or_by_the_system() {
        let (mut connection, user_id, now) = signed_in("contador");
        let manual = insert_sale(&connection, "V-00001", None);

        change_invoice_status_inner(&mut connection, now, manual, "cancelled", None).unwrap();
        insert_sale(&connection, "V-00002", Some("2020-01-02"));
        expire_reservations(&connection, now).unwrap();

        let rows = entries(&connection);
        assert_eq!(rows.len(), 2);
        assert_eq!(
            rows[0],
            (
                Some(user_id),
                "actuser".to_owned(),
                "cancel".to_owned(),
                "invoice".to_owned(),
                "V-00001".to_owned(),
                None
            )
        );
        assert_eq!(rows[1].0, None);
        assert_eq!(rows[1].1, "Sistema");
        assert_eq!(rows[1].2, "expire");
        assert_eq!(rows[1].4, "V-00002");
    }

    #[test]
    fn collecting_an_invoice_is_not_a_cancellation() {
        let (mut connection, _, now) = signed_in("contador");
        let invoice = insert_sale(&connection, "V-00001", None);

        change_invoice_status_inner(&mut connection, now, invoice, "completed", Some("cash"))
            .unwrap();

        assert!(entries(&connection).is_empty());
    }

    #[test]
    fn the_automatic_purge_is_recorded_by_the_system() {
        let (connection, _, now) = signed_in("contador");
        insert_product_fixture(
            &connection,
            "Vencido",
            None,
            None,
            100,
            0,
            0,
            "discontinued",
            Some("2020-01-01T00:00:00Z"),
            None,
        );

        purge_expired_products(&connection, now).unwrap();

        let rows = entries(&connection);
        assert_eq!(rows.len(), 1);
        assert_eq!(rows[0].1, "Sistema");
        assert_eq!(rows[0].2, "purge");
        assert_eq!(rows[0].4, "Vencido");
    }

    #[test]
    fn configuration_changes_are_recorded_without_contact_data() {
        let (mut connection, _, now) = signed_in("contador");
        let dir = std::env::temp_dir().join(format!(
            "goyma-activity-{}",
            now.timestamp_nanos_opt().unwrap()
        ));
        std::fs::create_dir_all(&dir).unwrap();

        update_settings_inner(
            &mut connection,
            now,
            UpdateSettingsInput {
                stale_invoice_days: 10,
                backup_reminder_days: 5,
            },
        )
        .unwrap();
        update_exchange_rate_inner(&mut connection, now, "CUP", "100").unwrap();
        update_exchange_rate_inner(&mut connection, now, "CUP", "120,5").unwrap();
        update_company_profile_inner(
            &mut connection,
            now,
            &dir,
            UpdateCompanyProfileInput {
                name: "Goyma".to_owned(),
                address: "Calle 1".to_owned(),
                phone: "+5355555555".to_owned(),
                tax_id: "123".to_owned(),
            },
        )
        .unwrap();
        save_company_logo_inner(&mut connection, now, &dir, None).unwrap();

        let rows = entries(&connection);
        assert_eq!(
            labels(&connection),
            vec![
                triple("config", "settings", "Umbrales"),
                triple("config", "exchangeRate", "CUP"),
                triple("config", "exchangeRate", "CUP"),
                triple("config", "settings", "Datos de la empresa"),
                triple("config", "settings", "Logo de la empresa"),
            ]
        );
        assert_eq!(rows[2].5.as_deref(), Some("100 → 120,5"));
        assert!(rows
            .iter()
            .all(|row| !format!("{row:?}").contains("+5355555555")));
    }

    fn seed_log(connection: &Connection) {
        for (date, user, action, label, details) in [
            ("2026-01-10T12:00:00Z", "ana", "delete", "Cable Rojo", None),
            ("2026-02-10T12:00:00Z", "luis", "cancel", "V-00001", None),
            (
                "2026-03-10T12:00:00Z",
                "ana",
                "config",
                "CUP",
                Some("100 → 120"),
            ),
        ] {
            connection
                .execute(
                    "INSERT INTO activityLog (createdAt, userName, action, entityType, entityLabel, details)
                     VALUES (?1, ?2, ?3, 'product', ?4, ?5)",
                    (date, user, action, label, details),
                )
                .unwrap();
        }
    }

    fn listed(
        connection: &mut Connection,
        now: DateTime<Utc>,
        filters: ActivityFilters,
    ) -> Vec<String> {
        list_activity_log_inner(connection, now, filters)
            .unwrap()
            .into_iter()
            .map(|item| item.entity_label)
            .collect()
    }

    #[test]
    fn list_activity_log_filters_and_sorts() {
        let (mut connection, _, now) = signed_in("administrador");
        seed_log(&connection);

        assert_eq!(
            listed(&mut connection, now, ActivityFilters::default()),
            vec!["CUP", "V-00001", "Cable Rojo"]
        );
        assert_eq!(
            listed(
                &mut connection,
                now,
                ActivityFilters {
                    oldest_first: true,
                    ..ActivityFilters::default()
                }
            ),
            vec!["Cable Rojo", "V-00001", "CUP"]
        );
        assert_eq!(
            listed(
                &mut connection,
                now,
                ActivityFilters {
                    user_name: Some("ana".to_owned()),
                    action: Some("delete".to_owned()),
                    ..ActivityFilters::default()
                }
            ),
            vec!["Cable Rojo"]
        );
        assert_eq!(
            listed(
                &mut connection,
                now,
                ActivityFilters {
                    from_date: Some("2026-02-01".to_owned()),
                    to_date: Some("2026-02-28".to_owned()),
                    ..ActivityFilters::default()
                }
            ),
            vec!["V-00001"]
        );
        for search in ["cable rojo", "120"] {
            assert_eq!(
                listed(
                    &mut connection,
                    now,
                    ActivityFilters {
                        search: Some(search.to_owned()),
                        ..ActivityFilters::default()
                    }
                )
                .len(),
                1,
                "{search}"
            );
        }
    }

    #[test]
    fn list_activity_log_rejects_an_invalid_date() {
        let (mut connection, _, now) = signed_in("administrador");
        let error = list_activity_log_inner(
            &mut connection,
            now,
            ActivityFilters {
                from_date: Some("mañana".to_owned()),
                ..ActivityFilters::default()
            },
        )
        .unwrap_err();
        assert_eq!(error.code, "validationFailed");
    }

    #[test]
    fn list_activity_log_requires_an_administrator() {
        assert_requires_admin(|connection, now| {
            list_activity_log_inner(connection, now, ActivityFilters::default())
        });
    }
}
