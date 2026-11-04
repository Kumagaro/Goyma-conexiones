use chrono::{DateTime, Utc};
use rusqlite::Connection;

use crate::text::{sort_spanish, spanish_sort_key};
use crate::timestamps::format_utc_timestamp;

const NOTIFICATION_PRIORITY: &str = "ADVERTENCIA";
const MAX_LISTED_PRODUCTS: usize = 20;

#[derive(Clone, Copy)]
pub enum AffectedProductsKind {
    Category,
    Unit,
}

impl AffectedProductsKind {
    fn notification_type(self) -> &'static str {
        match self {
            Self::Category => "N-09",
            Self::Unit => "N-08",
        }
    }

    fn message_prefix(self) -> &'static str {
        match self {
            Self::Category => "Los siguientes productos quedaron sin categoría",
            Self::Unit => "Los siguientes productos quedaron sin unidad de medida",
        }
    }
}

pub const CRITICAL: &str = "CRÍTICO";
pub const WARNING: &str = "ADVERTENCIA";
pub const INFO: &str = "INFORMATIVO";

/// Lo que abre una notificación al hacer clic (NT-04).
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Entity {
    Product(i64),
    Invoice(i64),
}

/// Quién la ve: N-13 (gestión de usuarios) es solo del Administrador.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Audience {
    All,
    Admin,
}

/// Inserta una notificación con su entidad y audiencia (`plan-04` §3.3).
pub fn notify(
    connection: &Connection,
    notification_type: &str,
    priority: &str,
    message: &str,
    entity: Option<Entity>,
    audience: Audience,
    now: DateTime<Utc>,
) -> rusqlite::Result<()> {
    let (entity_type, entity_id) = match entity {
        Some(Entity::Product(id)) => (Some("product"), Some(id)),
        Some(Entity::Invoice(id)) => (Some("invoice"), Some(id)),
        None => (None, None),
    };
    let audience = match audience {
        Audience::All => "all",
        Audience::Admin => "admin",
    };
    connection.execute(
        "INSERT INTO notifications (type, priority, message, entityType, entityId, audience, createdAt)
         VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7)",
        rusqlite::params![
            notification_type,
            priority,
            message,
            entity_type,
            entity_id,
            audience,
            format_utc_timestamp(now)
        ],
    )?;
    Ok(())
}

/// Marca que la situación ya avisó; devuelve `true` solo la primera vez
/// (NT-03: una vez por situación).
pub fn watch_enter(
    connection: &Connection,
    kind: &str,
    key: &str,
    now: DateTime<Utc>,
) -> rusqlite::Result<bool> {
    let inserted = connection.execute(
        "INSERT OR IGNORE INTO notificationWatches (kind, entityKey, createdAt) VALUES (?1, ?2, ?3)",
        (kind, key, format_utc_timestamp(now)),
    )?;
    Ok(inserted == 1)
}

/// La situación terminó: si vuelve, avisará de nuevo.
pub fn watch_leave(connection: &Connection, kind: &str, key: &str) -> rusqlite::Result<()> {
    connection.execute(
        "DELETE FROM notificationWatches WHERE kind = ?1 AND entityKey = ?2",
        (kind, key),
    )?;
    Ok(())
}

pub fn insert_notification(
    connection: &Connection,
    notification_type: &str,
    priority: &str,
    message: &str,
    now: DateTime<Utc>,
) -> rusqlite::Result<()> {
    let created_at = format_utc_timestamp(now);
    connection.execute(
        "INSERT INTO notifications (type, priority, message, createdAt) VALUES (?1, ?2, ?3, ?4)",
        (notification_type, priority, message, created_at),
    )?;
    Ok(())
}

pub fn insert_affected_products_notification(
    connection: &Connection,
    kind: AffectedProductsKind,
    products: &[(i64, String)],
    now: DateTime<Utc>,
) -> rusqlite::Result<usize> {
    if products.is_empty() {
        return Ok(0);
    }

    let mut ordered_products = products.to_vec();
    sort_spanish(&mut ordered_products, |(id, name)| {
        (spanish_sort_key(name), *id)
    });

    let affected_count = ordered_products.len();
    let message = format!(
        "{}: {}",
        kind.message_prefix(),
        product_list(&ordered_products)
    );

    insert_notification(
        connection,
        kind.notification_type(),
        NOTIFICATION_PRIORITY,
        &message,
        now,
    )?;

    Ok(affected_count)
}

/// Nombres separados por comas; a partir del 21.º, "y N más".
fn product_list(ordered_products: &[(i64, String)]) -> String {
    let mut list = ordered_products
        .iter()
        .take(MAX_LISTED_PRODUCTS)
        .map(|(_, name)| name.as_str())
        .collect::<Vec<_>>()
        .join(", ");
    if ordered_products.len() > MAX_LISTED_PRODUCTS {
        list.push_str(&format!(
            " y {} más",
            ordered_products.len() - MAX_LISTED_PRODUCTS
        ));
    }
    list
}

/// N-12: la purga automática (P-08) eliminó estos descontinuados.
pub fn insert_purged_products_notification(
    connection: &Connection,
    products: &[(i64, String)],
    now: DateTime<Utc>,
) -> rusqlite::Result<()> {
    if products.is_empty() {
        return Ok(());
    }
    let mut ordered_products = products.to_vec();
    sort_spanish(&mut ordered_products, |(id, name)| {
        (spanish_sort_key(name), *id)
    });
    notify(
        connection,
        "N-12",
        INFO,
        &format!(
            "Se eliminaron automáticamente estos productos descontinuados: {}",
            product_list(&ordered_products)
        ),
        None,
        Audience::All,
        now,
    )
}

#[cfg(test)]
mod tests {
    use chrono::{TimeZone, Utc};
    use rusqlite::{Connection, OptionalExtension};

    use crate::test_utils::setup_test_db;

    use super::{insert_affected_products_notification, insert_notification, AffectedProductsKind};

    fn notification_row(connection: &Connection) -> Option<(String, String, String, String, i64)> {
        connection
            .query_row(
                "SELECT type, priority, message, createdAt, isRead FROM notifications",
                [],
                |row| {
                    Ok((
                        row.get(0)?,
                        row.get(1)?,
                        row.get(2)?,
                        row.get(3)?,
                        row.get(4)?,
                    ))
                },
            )
            .optional()
            .unwrap()
    }

    #[test]
    fn insert_notification_persists_fixed_timestamp_and_unread_state() {
        let connection = setup_test_db();
        let now = Utc.with_ymd_and_hms(2026, 10, 3, 14, 15, 59).unwrap()
            + chrono::Duration::milliseconds(999);

        insert_notification(
            &connection,
            "N-09",
            "ADVERTENCIA",
            "Los siguientes productos quedaron sin categoría: Café",
            now,
        )
        .unwrap();

        assert_eq!(
            notification_row(&connection),
            Some((
                "N-09".to_owned(),
                "ADVERTENCIA".to_owned(),
                "Los siguientes productos quedaron sin categoría: Café".to_owned(),
                "2026-10-03T14:15:59Z".to_owned(),
                0,
            ))
        );
    }

    #[test]
    fn affected_products_notification_sorts_in_spanish_before_truncating() {
        let connection = setup_test_db();
        let products = vec![
            (4, "oso".to_owned()),
            (3, "ñame".to_owned()),
            (2, "nuez".to_owned()),
            (5, "angel".to_owned()),
            (1, "Ángel".to_owned()),
        ];

        let affected_count = insert_affected_products_notification(
            &connection,
            AffectedProductsKind::Category,
            &products,
            Utc::now(),
        )
        .unwrap();

        assert_eq!(affected_count, 5);
        let row = notification_row(&connection).unwrap();
        assert_eq!(row.0, "N-09");
        assert_eq!(row.1, "ADVERTENCIA");
        assert_eq!(
            row.2,
            "Los siguientes productos quedaron sin categoría: Ángel, angel, nuez, ñame, oso"
        );
    }

    #[test]
    fn affected_products_notification_obeys_one_twenty_twenty_one_and_twenty_five_limits() {
        for (count, expected_suffix) in [
            (1, None),
            (20, None),
            (21, Some("y 1 más")),
            (25, Some("y 5 más")),
        ] {
            let connection = setup_test_db();
            let products: Vec<(i64, String)> = (0..count)
                .rev()
                .map(|number| ((count - number) as i64, format!("P{number:02}")))
                .collect();

            let affected_count = insert_affected_products_notification(
                &connection,
                AffectedProductsKind::Unit,
                &products,
                Utc::now(),
            )
            .unwrap();

            assert_eq!(affected_count, count as usize);
            let row = notification_row(&connection).unwrap();
            assert_eq!(row.0, "N-08");
            assert_eq!(row.1, "ADVERTENCIA");
            let listed_names = (0..count.min(20))
                .map(|number| format!("P{number:02}"))
                .collect::<Vec<_>>()
                .join(", ");
            let expected_message = match expected_suffix {
                Some(suffix) => format!(
                    "Los siguientes productos quedaron sin unidad de medida: {listed_names} {suffix}"
                ),
                None => format!(
                    "Los siguientes productos quedaron sin unidad de medida: {listed_names}"
                ),
            };
            assert_eq!(row.2, expected_message);
        }
    }

    #[test]
    fn no_affected_products_means_no_notification() {
        let connection = setup_test_db();

        let affected_count = insert_affected_products_notification(
            &connection,
            AffectedProductsKind::Category,
            &[],
            Utc::now(),
        )
        .unwrap();

        let notification_count: i64 = connection
            .query_row("SELECT COUNT(*) FROM notifications", [], |row| row.get(0))
            .unwrap();
        assert_eq!(affected_count, 0);
        assert_eq!(notification_count, 0);
    }
}
