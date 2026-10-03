use chrono::{DateTime, SecondsFormat, Utc};
use rusqlite::Connection;

use crate::commands::auth::hash_password;
use crate::db::schema::create_schema;
use crate::text::normalize_text;

const FIXTURE_TIMESTAMP: &str = "2026-10-03T00:00:00Z";

pub fn setup_test_db() -> Connection {
    let connection = Connection::open_in_memory().expect("abre SQLite en memoria");
    connection
        .pragma_update(None, "foreign_keys", "ON")
        .expect("activa claves foráneas");
    create_schema(&connection).expect("crea el esquema de test");
    connection
}

pub fn create_test_user(
    connection: &Connection,
    username: &str,
    role: &str,
    phone: &str,
    national_id: &str,
    password: &str,
) -> i64 {
    let password_hash = hash_password(password).expect("genera hash bcrypt de test");
    connection
        .execute(
            "INSERT INTO users (username, passwordHash, role, phone, nationalId, createdAt)
             VALUES (?1, ?2, ?3, ?4, ?5, ?6)",
            (
                username,
                password_hash,
                role,
                phone,
                national_id,
                Utc::now().to_rfc3339_opts(SecondsFormat::Secs, true),
            ),
        )
        .expect("inserta usuario de test");
    connection.last_insert_rowid()
}

pub fn create_test_session(
    connection: &Connection,
    user_id: i64,
    remember_session: bool,
    created_at: DateTime<Utc>,
    last_activity_at: DateTime<Utc>,
) {
    connection
        .execute(
            "INSERT INTO sessions (id, userId, rememberSession, createdAt, lastActivityAt)
             VALUES (1, ?1, ?2, ?3, ?4)",
            (
                user_id,
                remember_session,
                created_at.to_rfc3339_opts(SecondsFormat::Secs, true),
                last_activity_at.to_rfc3339_opts(SecondsFormat::Secs, true),
            ),
        )
        .expect("inserta sesión de test");
}

pub fn insert_category_fixture(
    connection: &Connection,
    name: &str,
    description: Option<&str>,
) -> i64 {
    let normalized_name = normalize_text(name);
    connection
        .execute(
            "INSERT INTO categories (name, nameNormalized, description, createdAt, updatedAt)
             VALUES (?1, ?2, ?3, ?4, ?4)",
            (name, normalized_name, description, FIXTURE_TIMESTAMP),
        )
        .expect("inserta categoría de test");
    connection.last_insert_rowid()
}

pub fn insert_unit_fixture(connection: &Connection, name: &str, abbreviation: &str) -> i64 {
    let normalized_name = normalize_text(name);
    let normalized_abbreviation = normalize_text(abbreviation);
    connection
        .execute(
            "INSERT INTO units (
                name, nameNormalized, abbreviation, abbreviationNormalized, createdAt, updatedAt
             ) VALUES (?1, ?2, ?3, ?4, ?5, ?5)",
            (
                name,
                normalized_name,
                abbreviation,
                normalized_abbreviation,
                FIXTURE_TIMESTAMP,
            ),
        )
        .expect("inserta unidad de test");
    connection.last_insert_rowid()
}

pub fn insert_conversion_fixture(
    connection: &Connection,
    from_unit_id: i64,
    to_unit_id: i64,
    factor_micro: i64,
) -> i64 {
    connection
        .execute(
            "INSERT INTO unitConversions (
                fromUnitId, toUnitId, factorMicro, createdAt, updatedAt
             ) VALUES (?1, ?2, ?3, ?4, ?4)",
            (from_unit_id, to_unit_id, factor_micro, FIXTURE_TIMESTAMP),
        )
        .expect("inserta conversión de test");
    connection.last_insert_rowid()
}

#[allow(clippy::too_many_arguments)]
pub fn insert_product_fixture(
    connection: &Connection,
    name: &str,
    category_id: Option<i64>,
    unit_id: Option<i64>,
    cost_cents: i64,
    profit_hundredths: i64,
    quantity_milli: i64,
    status: &str,
    discontinued_at: Option<&str>,
    expires_on: Option<&str>,
) -> i64 {
    let normalized_name = normalize_text(name);
    connection
        .execute(
            "INSERT INTO products (
                name, nameNormalized, description, categoryId, unitId, costCents,
                profitPercentHundredths, expiresOn, quantityMilli, status, discontinuedAt,
                createdAt, updatedAt
             ) VALUES (?1, ?2, NULL, ?3, ?4, ?5, ?6, ?7, ?8, ?9, ?10, ?11, ?11)",
            (
                name,
                normalized_name,
                category_id,
                unit_id,
                cost_cents,
                profit_hundredths,
                expires_on,
                quantity_milli,
                status,
                discontinued_at,
                FIXTURE_TIMESTAMP,
            ),
        )
        .expect("inserta producto de test");
    connection.last_insert_rowid()
}

pub fn create_fail_product_delete_trigger(connection: &Connection) -> rusqlite::Result<()> {
    connection.execute_batch(
        "CREATE TRIGGER test_fail_product_delete
         BEFORE DELETE ON products
         BEGIN
             SELECT RAISE(ABORT, 'forced product deletion failure');
         END;",
    )
}

pub fn create_fail_notification_insert_trigger(connection: &Connection) -> rusqlite::Result<()> {
    connection.execute_batch(
        "CREATE TRIGGER test_fail_notification_insert
         BEFORE INSERT ON notifications
         BEGIN
             SELECT RAISE(ABORT, 'forced notification insertion failure');
         END;",
    )
}

pub fn assert_requires_session<F, T>(mut operation: F)
where
    F: FnMut(&mut Connection, DateTime<Utc>) -> Result<T, crate::errors::AppError>,
    T: std::fmt::Debug,
{
    let now = Utc::now();
    let mut connection = setup_test_db();
    assert_eq!(
        error_code(operation(&mut connection, now)),
        "sessionExpired"
    );

    let mut connection = setup_test_db();
    let user_id = create_test_user(
        &connection,
        "matrixuser",
        "contador",
        "+5350000000",
        "12345678901",
        "ValidPass1!",
    );
    create_test_session(
        &connection,
        user_id,
        false,
        now - chrono::Duration::hours(4),
        now - chrono::Duration::hours(4),
    );
    assert_eq!(
        error_code(operation(&mut connection, now)),
        "sessionExpired"
    );
}

pub fn assert_allows_both_roles<F, T>(mut operation: F)
where
    F: FnMut(&mut Connection, DateTime<Utc>) -> Result<T, crate::errors::AppError>,
    T: std::fmt::Debug,
{
    let now = Utc::now();
    for (username, role, phone, national_id) in [
        ("matrixadmin", "administrador", "+5350000002", "12345678903"),
        ("matrixcounter", "contador", "+5350000003", "12345678904"),
    ] {
        let mut connection = setup_test_db();
        let user_id = create_test_user(
            &connection,
            username,
            role,
            phone,
            national_id,
            "ValidPass1!",
        );
        create_test_session(&connection, user_id, false, now, now);

        if let Err(error) = operation(&mut connection, now) {
            assert!(
                error.code != "sessionExpired" && error.code != "forbidden",
                "el rol {role} fue rechazado por autorización: {}",
                error.code
            );
        }
    }
}

pub fn assert_requires_admin<F, T>(mut operation: F)
where
    F: FnMut(&mut Connection, DateTime<Utc>) -> Result<T, crate::errors::AppError>,
    T: std::fmt::Debug,
{
    let now = Utc::now();
    let mut connection = setup_test_db();
    assert_eq!(
        error_code(operation(&mut connection, now)),
        "sessionExpired"
    );

    let mut connection = setup_test_db();
    let user_id = create_test_user(
        &connection,
        "matrixuser",
        "contador",
        "+5350000000",
        "12345678901",
        "ValidPass1!",
    );
    create_test_session(
        &connection,
        user_id,
        false,
        now - chrono::Duration::hours(4),
        now - chrono::Duration::hours(4),
    );
    assert_eq!(
        error_code(operation(&mut connection, now)),
        "sessionExpired"
    );

    let mut connection = setup_test_db();
    let user_id = create_test_user(
        &connection,
        "matrixcounter",
        "contador",
        "+5350000001",
        "12345678902",
        "ValidPass1!",
    );
    create_test_session(&connection, user_id, false, now, now);
    assert_eq!(error_code(operation(&mut connection, now)), "forbidden");
    let session_count: i64 = connection
        .query_row("SELECT COUNT(*) FROM sessions", [], |row| row.get(0))
        .expect("consulta la sesión válida del Contador");
    assert_eq!(session_count, 1);
}

fn error_code<T: std::fmt::Debug>(result: Result<T, crate::errors::AppError>) -> String {
    result.unwrap_err().code
}

#[cfg(test)]
mod tests {
    use super::{
        assert_allows_both_roles, assert_requires_admin, assert_requires_session,
        create_fail_notification_insert_trigger, create_fail_product_delete_trigger,
        insert_category_fixture, insert_conversion_fixture, insert_product_fixture,
        insert_unit_fixture, setup_test_db,
    };
    use crate::commands::auth::{require_admin, require_session};

    #[test]
    fn shared_session_matrix_checks_absent_and_expired_sessions() {
        assert_requires_session(|connection, now| require_session(connection, now));
    }

    #[test]
    fn shared_admin_matrix_checks_absent_expired_and_counter_sessions() {
        assert_requires_admin(|connection, now| require_admin(connection, now));
    }

    #[test]
    fn inventory_fixtures_insert_normalized_values_with_fixed_audit_timestamps() {
        let connection = setup_test_db();
        let category_id = insert_category_fixture(&connection, "Café", None);
        let unit_id = insert_unit_fixture(&connection, "Kilogramo", "kg");
        let gram_id = insert_unit_fixture(&connection, "Gramo", "g");
        let conversion_id = insert_conversion_fixture(&connection, unit_id, gram_id, 1000);
        let product_id = insert_product_fixture(
            &connection,
            "Café molido",
            Some(category_id),
            Some(unit_id),
            1250,
            2000,
            0,
            "active",
            None,
            None,
        );

        let category: (String, Option<String>, String, String) = connection
            .query_row(
                "SELECT nameNormalized, description, createdAt, updatedAt FROM categories WHERE id = ?1",
                [category_id],
                |row| Ok((row.get(0)?, row.get(1)?, row.get(2)?, row.get(3)?)),
            )
            .unwrap();
        assert_eq!(
            category,
            (
                "cafe".to_owned(),
                None,
                "2026-10-03T00:00:00Z".to_owned(),
                "2026-10-03T00:00:00Z".to_owned()
            )
        );

        let unit: (String, String) = connection
            .query_row(
                "SELECT nameNormalized, abbreviationNormalized FROM units WHERE id = ?1",
                [unit_id],
                |row| Ok((row.get(0)?, row.get(1)?)),
            )
            .unwrap();
        assert_eq!(unit, ("kilogramo".to_owned(), "kg".to_owned()));

        let conversion_count: i64 = connection
            .query_row(
                "SELECT COUNT(*) FROM unitConversions WHERE id = ?1 AND factorMicro = 1000 AND createdAt = '2026-10-03T00:00:00Z' AND updatedAt = '2026-10-03T00:00:00Z'",
                [conversion_id],
                |row| row.get(0),
            )
            .unwrap();
        assert_eq!(conversion_count, 1);

        let product: (String, i64, i64, Option<String>, Option<String>, String, String) = connection
            .query_row(
                "SELECT nameNormalized, costCents, quantityMilli, discontinuedAt, expiresOn, createdAt, updatedAt FROM products WHERE id = ?1",
                [product_id],
                |row| Ok((row.get(0)?, row.get(1)?, row.get(2)?, row.get(3)?, row.get(4)?, row.get(5)?, row.get(6)?)),
            )
            .unwrap();
        assert_eq!(
            product,
            (
                "cafe molido".to_owned(),
                1250,
                0,
                None,
                None,
                "2026-10-03T00:00:00Z".to_owned(),
                "2026-10-03T00:00:00Z".to_owned()
            )
        );
    }

    #[test]
    fn both_inventory_roles_are_not_rejected_by_session_authorization() {
        assert_allows_both_roles(|connection, now| {
            require_session(connection, now).map(|actor| actor.role().to_owned())
        });
    }

    #[test]
    fn test_triggers_abort_only_the_requested_operations() {
        let connection = setup_test_db();
        let product_id = insert_product_fixture(
            &connection,
            "Producto de prueba",
            None,
            None,
            100,
            0,
            1,
            "active",
            None,
            None,
        );
        create_fail_product_delete_trigger(&connection).unwrap();
        assert!(connection
            .execute("DELETE FROM products WHERE id = ?1", [product_id])
            .is_err());
        let product_count: i64 = connection
            .query_row(
                "SELECT COUNT(*) FROM products WHERE id = ?1",
                [product_id],
                |row| row.get(0),
            )
            .unwrap();
        assert_eq!(product_count, 1);

        create_fail_notification_insert_trigger(&connection).unwrap();
        assert!(connection.execute(
            "INSERT INTO notifications (type, priority, message, createdAt) VALUES ('test', 'test', 'test', '2026-10-03T00:00:00Z')",
            [],
        ).is_err());
        let notification_count: i64 = connection
            .query_row("SELECT COUNT(*) FROM notifications", [], |row| row.get(0))
            .unwrap();
        assert_eq!(notification_count, 0);
    }
}
