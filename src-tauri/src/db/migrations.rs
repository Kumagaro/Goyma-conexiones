use rusqlite::{Connection, Transaction};

use crate::errors::AppError;

/// Versión del esquema que crea `schema::create_schema` (specs 1 a 3). Todo
/// cambio posterior llega como migración numerada (`Constitution.md` §4,
/// v1.3.0; `plan-04` §2.1).
pub const BASE_VERSION: i64 = 1;

pub type Migration = fn(&Transaction) -> rusqlite::Result<()>;

/// Migraciones en orden estricto: cada una sube la versión en uno.
pub const MIGRATIONS: &[(i64, Migration)] = &[(2, notifications_v2), (3, reservations_v3)];

const CREATED_AT_CHECK: &str = "createdAt GLOB '[0-9][0-9][0-9][0-9]-[0-9][0-9]-[0-9][0-9]T[0-9][0-9]:[0-9][0-9]:[0-9][0-9]Z' AND strftime('%Y-%m-%dT%H:%M:%SZ', createdAt) IS createdAt";

/// Migración 2 (`plan-04` §2.2): entidad afectada, audiencia y prioridad
/// cerrada en `notifications`, y `notificationWatches` para avisar una sola
/// vez por situación (NT-03). SQLite no añade `CHECK` a columnas existentes:
/// la tabla se reconstruye conservando ids y datos.
fn notifications_v2(tx: &Transaction) -> rusqlite::Result<()> {
    tx.execute_batch(&format!(
        "CREATE TABLE notificationsNew (
            id INTEGER PRIMARY KEY AUTOINCREMENT,
            type TEXT NOT NULL,
            priority TEXT NOT NULL CHECK (priority IN ('CRÍTICO','ADVERTENCIA','INFORMATIVO')),
            message TEXT NOT NULL,
            entityType TEXT CHECK (entityType IN ('product','invoice')),
            entityId INTEGER,
            audience TEXT NOT NULL DEFAULT 'all' CHECK (audience IN ('all','admin')),
            createdAt TEXT NOT NULL CHECK ({CREATED_AT_CHECK}),
            isRead INTEGER NOT NULL DEFAULT 0 CHECK (isRead IN (0, 1)),
            CHECK ((entityType IS NULL) = (entityId IS NULL))
        );
        INSERT INTO notificationsNew (id, type, priority, message, createdAt, isRead)
            SELECT id, type, priority, message, createdAt, isRead FROM notifications;
        DROP TABLE notifications;
        ALTER TABLE notificationsNew RENAME TO notifications;
        CREATE INDEX notificationsReadCreated ON notifications (isRead, createdAt);

        CREATE TABLE notificationWatches (
            kind TEXT NOT NULL CHECK (kind IN ('N-02','N-03','N-04','N-05','N-07','N-10','N-11')),
            entityKey TEXT NOT NULL CHECK (length(entityKey) BETWEEN 1 AND 40),
            createdAt TEXT NOT NULL CHECK ({CREATED_AT_CHECK}),
            PRIMARY KEY (kind, entityKey)
        );"
    ))
}

/// Migración 3 (`plan-04` §2.3): una reserva es una venta o un servicio con
/// fecha límite posterior a la emisión (RS-01, D-08).
fn reservations_v3(tx: &Transaction) -> rusqlite::Result<()> {
    tx.execute_batch(
        "ALTER TABLE invoices ADD COLUMN reservedUntil TEXT CHECK (
            reservedUntil IS NULL OR (
                type IN ('V','S')
                AND reservedUntil GLOB '[0-9][0-9][0-9][0-9]-[0-9][0-9]-[0-9][0-9]'
                AND date(reservedUntil) IS reservedUntil
                AND reservedUntil > invoiceDate
            )
        );
        CREATE INDEX invoicesActiveReservations ON invoices (reservedUntil)
            WHERE reservedUntil IS NOT NULL AND status = 'pending';",
    )
}

pub fn latest_version() -> i64 {
    latest_of(MIGRATIONS)
}

fn latest_of(migrations: &[(i64, Migration)]) -> i64 {
    migrations
        .last()
        .map_or(BASE_VERSION, |(version, _)| *version)
}

pub fn schema_version(connection: &Connection) -> rusqlite::Result<i64> {
    connection.pragma_query_value(None, "user_version", |row| row.get(0))
}

/// Lleva la base a la última versión. Se llama después de `init_schema`, que
/// deja creado (o intacto) el esquema base.
pub fn run_migrations(connection: &mut Connection) -> Result<i64, AppError> {
    run_migrations_with(connection, MIGRATIONS)
}

fn run_migrations_with(
    connection: &mut Connection,
    migrations: &[(i64, Migration)],
) -> Result<i64, AppError> {
    let mut current = schema_version(connection)
        .map_err(|error| migration_failed(&format!("failed to read schema version: {error}")))?;
    // Una base sin versión es la de las specs 1 a 3 (o una recién creada por
    // init_schema): se marca como versión base sin tocar sus datos.
    if current == 0 {
        connection
            .pragma_update(None, "user_version", BASE_VERSION)
            .map_err(|error| migration_failed(&format!("failed to set base version: {error}")))?;
        current = BASE_VERSION;
    }
    let latest = latest_of(migrations);
    if current > latest {
        return Err(AppError::new(
            "schemaTooNew",
            "Esta base de datos es de una versión más nueva de la aplicación",
            &format!("schema version {current} is newer than supported {latest}"),
        ));
    }
    let start = current;
    for (version, migrate) in migrations.iter().filter(|(version, _)| *version > start) {
        // Cada migración y su número de versión van en la misma transacción:
        // si falla, la base queda en la versión anterior.
        let transaction = connection.transaction().map_err(|error| {
            migration_failed(&format!("failed to open migration {version}: {error}"))
        })?;
        migrate(&transaction)
            .and_then(|()| transaction.pragma_update(None, "user_version", version))
            .and_then(|()| transaction.commit())
            .map_err(|error| migration_failed(&format!("migration {version} failed: {error}")))?;
        current = *version;
    }
    Ok(current)
}

fn migration_failed(cause: &str) -> AppError {
    AppError::new(
        "migrationFailed",
        "No se pudo actualizar la base de datos. La aplicación no puede continuar",
        cause,
    )
}

#[cfg(test)]
mod tests {
    use rusqlite::{Connection, Transaction};

    use super::{
        latest_version, run_migrations, run_migrations_with, schema_version, Migration,
        BASE_VERSION,
    };
    use crate::db::schema::create_schema;

    fn table_exists(connection: &Connection, name: &str) -> bool {
        connection
            .query_row(
                "SELECT EXISTS(SELECT 1 FROM sqlite_master WHERE type = 'table' AND name = ?1)",
                [name],
                |row| row.get(0),
            )
            .unwrap()
    }

    fn add_first(tx: &Transaction) -> rusqlite::Result<()> {
        tx.execute_batch("CREATE TABLE migrationFirst (id INTEGER PRIMARY KEY);")
    }

    fn add_second(tx: &Transaction) -> rusqlite::Result<()> {
        tx.execute_batch("CREATE TABLE migrationSecond (id INTEGER PRIMARY KEY);")
    }

    fn fails_halfway(tx: &Transaction) -> rusqlite::Result<()> {
        tx.execute_batch("CREATE TABLE migrationBroken (id INTEGER PRIMARY KEY);")?;
        tx.execute_batch("INSERT INTO tableThatDoesNotExist VALUES (1);")
    }

    #[test]
    fn a_database_from_specs_1_to_3_is_marked_as_the_base_version_without_changes() {
        let mut connection = Connection::open_in_memory().unwrap();
        create_schema(&connection).unwrap();
        connection
            .execute("INSERT INTO categories (name, nameNormalized, createdAt, updatedAt) VALUES ('Cables', 'cables', '2026-10-28T12:00:00Z', '2026-10-28T12:00:00Z')", [])
            .unwrap();
        assert_eq!(schema_version(&connection).unwrap(), 0);

        let version = run_migrations(&mut connection).unwrap();
        assert_eq!(version, latest_version());
        assert!(version >= BASE_VERSION);
        assert_eq!(schema_version(&connection).unwrap(), version);
        let categories: i64 = connection
            .query_row("SELECT COUNT(*) FROM categories", [], |row| row.get(0))
            .unwrap();
        assert_eq!(categories, 1);

        // Ejecutarlo otra vez no cambia nada.
        assert_eq!(run_migrations(&mut connection).unwrap(), version);
    }

    #[test]
    fn pending_migrations_apply_in_order_and_only_once() {
        let mut connection = Connection::open_in_memory().unwrap();
        create_schema(&connection).unwrap();
        let migrations: &[(i64, Migration)] = &[(2, add_first), (3, add_second)];

        assert_eq!(run_migrations_with(&mut connection, migrations).unwrap(), 3);
        assert!(table_exists(&connection, "migrationFirst"));
        assert!(table_exists(&connection, "migrationSecond"));
        // Ya aplicadas: no se repiten (CREATE TABLE fallaría si lo hicieran).
        assert_eq!(run_migrations_with(&mut connection, migrations).unwrap(), 3);
    }

    #[test]
    fn a_failing_migration_leaves_the_previous_version_and_reports_migration_failed() {
        let mut connection = Connection::open_in_memory().unwrap();
        create_schema(&connection).unwrap();
        let migrations: &[(i64, Migration)] = &[(2, add_first), (3, fails_halfway)];

        let error = run_migrations_with(&mut connection, migrations).unwrap_err();
        assert_eq!(error.code, "migrationFailed");
        assert_eq!(
            error.message,
            "No se pudo actualizar la base de datos. La aplicación no puede continuar"
        );
        assert_eq!(schema_version(&connection).unwrap(), 2);
        assert!(table_exists(&connection, "migrationFirst"));
        // Lo que la migración fallida alcanzó a crear se revierte con ella.
        assert!(!table_exists(&connection, "migrationBroken"));
    }

    // Base versión 1 real (specs 1 a 3) con una notificación previa.
    fn migrated_v1_database() -> Connection {
        let mut connection = Connection::open_in_memory().unwrap();
        connection
            .pragma_update(None, "foreign_keys", "ON")
            .unwrap();
        create_schema(&connection).unwrap();
        connection
            .execute(
                "INSERT INTO notifications (type, priority, message, createdAt, isRead) VALUES ('N-09', 'ADVERTENCIA', 'Previa', '2026-10-01T10:00:00Z', 1)",
                [],
            )
            .unwrap();
        run_migrations(&mut connection).unwrap();
        connection
    }

    fn insert_notification(
        connection: &Connection,
        priority: &str,
        entity_type: Option<&str>,
        entity_id: Option<i64>,
        audience: &str,
    ) -> rusqlite::Result<usize> {
        connection.execute(
            "INSERT INTO notifications (type, priority, message, entityType, entityId, audience, createdAt) VALUES ('N-02', ?1, 'Producto agotado: Café', ?2, ?3, ?4, '2026-10-28T12:00:00Z')",
            rusqlite::params![priority, entity_type, entity_id, audience],
        )
    }

    #[test]
    fn migration_2_keeps_existing_notifications_and_closes_entity_audience_and_priority() {
        let connection = migrated_v1_database();
        let previous: (i64, String, String, Option<String>, String, i64) = connection
            .query_row(
                "SELECT id, type, message, entityType, audience, isRead FROM notifications",
                [],
                |row| {
                    Ok((
                        row.get(0)?,
                        row.get(1)?,
                        row.get(2)?,
                        row.get(3)?,
                        row.get(4)?,
                        row.get(5)?,
                    ))
                },
            )
            .unwrap();
        assert_eq!(
            previous,
            (1, "N-09".into(), "Previa".into(), None, "all".into(), 1)
        );

        assert!(
            insert_notification(&connection, "CRÍTICO", Some("product"), Some(3), "all").is_ok()
        );
        assert!(insert_notification(&connection, "INFORMATIVO", None, None, "admin").is_ok());
        // Entidad y id van juntos o no van.
        assert!(insert_notification(&connection, "CRÍTICO", Some("product"), None, "all").is_err());
        assert!(insert_notification(&connection, "CRÍTICO", None, Some(3), "all").is_err());
        assert!(
            insert_notification(&connection, "CRÍTICO", Some("client"), Some(3), "all").is_err()
        );
        assert!(insert_notification(&connection, "CRÍTICO", None, None, "contador").is_err());
        assert!(insert_notification(&connection, "test", None, None, "all").is_err());
        // Los ids siguen creciendo tras la reconstrucción de la tabla.
        let max_id: i64 = connection
            .query_row("SELECT MAX(id) FROM notifications", [], |row| row.get(0))
            .unwrap();
        assert_eq!(max_id, 3);
    }

    #[test]
    fn migration_2_watches_reject_duplicates_and_unknown_kinds() {
        let connection = migrated_v1_database();
        let watch = |kind: &str, key: &str| {
            connection.execute(
                "INSERT INTO notificationWatches (kind, entityKey, createdAt) VALUES (?1, ?2, '2026-10-28T12:00:00Z')",
                [kind, key],
            )
        };
        assert!(watch("N-03", "p:1").is_ok());
        assert!(watch("N-02", "p:1").is_ok());
        assert!(watch("N-03", "p:1").is_err());
        assert!(watch("N-09", "p:2").is_err());
        assert!(watch("N-11", "").is_err());
    }

    #[test]
    fn migration_3_reserved_until_only_on_sales_and_services_after_the_issue_date() {
        let connection = migrated_v1_database();
        let insert = |kind: &str, number: &str, reserved: Option<&str>| {
            let contact = if kind == "P" {
                None
            } else {
                Some("Rosa Díaz")
            };
            let status = if kind == "P" { None } else { Some("pending") };
            connection.execute(
                "INSERT INTO invoices (type, number, invoiceDate, status, contactFullName, contactFullNameNormalized, contactNationalId, contactPhone, createdByName, createdAt, updatedAt, reservedUntil)
                 VALUES (?1, ?2, '2026-10-28', ?3, ?4, 'rosa diaz', '90010112345', '+5355550000', 'admin', '2026-10-28T12:00:00Z', '2026-10-28T12:00:00Z', ?5)",
                rusqlite::params![kind, number, status, contact, reserved],
            )
        };
        assert!(insert("V", "V-00001", Some("2026-11-05")).is_ok());
        assert!(insert("S", "S-00001", Some("2026-10-29")).is_ok());
        assert!(insert("V", "V-00002", None).is_ok());
        // Igual o anterior a la emisión, fecha inválida o tipo que no se cobra.
        assert!(insert("V", "V-00003", Some("2026-10-28")).is_err());
        assert!(insert("V", "V-00004", Some("2026-10-27")).is_err());
        assert!(insert("V", "V-00005", Some("2026-02-30")).is_err());
        assert!(insert("E", "E-00001", Some("2026-11-05")).is_err());
        assert!(insert("P", "P-00001", Some("2026-11-05")).is_err());
    }

    #[test]
    fn a_newer_database_is_rejected_with_schema_too_new() {
        let mut connection = Connection::open_in_memory().unwrap();
        create_schema(&connection).unwrap();
        connection
            .pragma_update(None, "user_version", latest_version() + 1)
            .unwrap();

        let error = run_migrations(&mut connection).unwrap_err();
        assert_eq!(error.code, "schemaTooNew");
        assert_eq!(
            error.message,
            "Esta base de datos es de una versión más nueva de la aplicación"
        );
        assert_eq!(schema_version(&connection).unwrap(), latest_version() + 1);
    }
}
