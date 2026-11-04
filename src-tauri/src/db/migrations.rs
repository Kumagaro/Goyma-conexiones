use rusqlite::{Connection, Transaction};

use crate::errors::AppError;

/// Versión del esquema que crea `schema::create_schema` (specs 1 a 3). Todo
/// cambio posterior llega como migración numerada (`Constitution.md` §4,
/// v1.3.0; `plan-04` §2.1).
pub const BASE_VERSION: i64 = 1;

pub type Migration = fn(&Transaction) -> rusqlite::Result<()>;

/// Migraciones en orden estricto: cada una sube la versión en uno.
pub const MIGRATIONS: &[(i64, Migration)] = &[
    (2, notifications_v2),
    (3, reservations_v3),
    (4, settings_v4),
    (5, line_units_v5),
];

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

/// Entero canónico (sin ceros a la izquierda ni signos) entre dos límites.
fn integer_between(min: i64, max: i64) -> String {
    format!(
        "CAST(CAST(value AS INTEGER) AS TEXT) = value AND CAST(value AS INTEGER) BETWEEN {min} AND {max}"
    )
}

/// Migración 4 (`plan-04` §2.4, D-16): ajustes globales con claves cerradas,
/// preferencias por usuario, tasas de cambio y su historial inmutable.
fn settings_v4(tx: &Transaction) -> rusqlite::Result<()> {
    let updated_at = CREATED_AT_CHECK.replace("createdAt", "updatedAt");
    tx.execute_batch(&format!(
        "CREATE TABLE settings (
            key TEXT PRIMARY KEY CHECK (key IN ('lowStockThreshold','staleInvoiceDays','backupReminderDays',
                'companyName','companyAddress','companyPhone','companyTaxId','companyLogoFile','lastBackupAt')),
            value TEXT NOT NULL,
            updatedAt TEXT NOT NULL CHECK ({updated_at}),
            CHECK (CASE key
                WHEN 'lowStockThreshold' THEN {low}
                WHEN 'staleInvoiceDays' THEN {stale}
                WHEN 'backupReminderDays' THEN {backup}
                WHEN 'companyName' THEN length(value) <= 100
                WHEN 'companyAddress' THEN length(value) <= 200
                WHEN 'companyPhone' THEN length(value) <= 30
                WHEN 'companyTaxId' THEN length(value) <= 30
                ELSE 1 END)
        );
        INSERT INTO settings (key, value, updatedAt) VALUES
            ('lowStockThreshold', '5', strftime('%Y-%m-%dT%H:%M:%SZ', 'now')),
            ('staleInvoiceDays', '30', strftime('%Y-%m-%dT%H:%M:%SZ', 'now')),
            ('backupReminderDays', '7', strftime('%Y-%m-%dT%H:%M:%SZ', 'now'));

        CREATE TABLE userPreferences (
            userId INTEGER NOT NULL REFERENCES users(id) ON DELETE CASCADE,
            key TEXT NOT NULL CHECK (key IN ('visibleCurrencies','pageSize')),
            value TEXT NOT NULL,
            updatedAt TEXT NOT NULL CHECK ({updated_at}),
            PRIMARY KEY (userId, key),
            CHECK (CASE key
                WHEN 'pageSize' THEN value IN ('10','20','50','100')
                -- Lista separada por comas de EUR, CUP, MLC y SALDO (o vacía).
                ELSE replace(replace(replace(replace(replace(value, 'EUR', ''), 'CUP', ''), 'MLC', ''), 'SALDO', ''), ',', '') = ''
                END)
        );

        CREATE TABLE exchangeRates (
            currency TEXT PRIMARY KEY CHECK (currency IN ('EUR','CUP','MLC','SALDO')),
            rateMicro INTEGER NOT NULL CHECK (rateMicro BETWEEN 1 AND 999999999999999),
            updatedAt TEXT NOT NULL CHECK ({updated_at}),
            updatedByName TEXT NOT NULL CHECK (length(updatedByName) BETWEEN 1 AND 50)
        );

        CREATE TABLE exchangeRateHistory (
            id INTEGER PRIMARY KEY AUTOINCREMENT,
            createdAt TEXT NOT NULL CHECK ({CREATED_AT_CHECK}),
            currency TEXT NOT NULL CHECK (currency IN ('EUR','CUP','MLC','SALDO')),
            previousRateMicro INTEGER CHECK (previousRateMicro IS NULL OR previousRateMicro BETWEEN 1 AND 999999999999999),
            newRateMicro INTEGER NOT NULL CHECK (newRateMicro BETWEEN 1 AND 999999999999999),
            userId INTEGER REFERENCES users(id) ON DELETE SET NULL,
            userName TEXT NOT NULL CHECK (length(userName) BETWEEN 1 AND 50)
        );
        CREATE INDEX exchangeRateHistoryCreated ON exchangeRateHistory (createdAt);
        -- userId puede pasar a NULL al borrar el usuario; el resto no cambia.
        CREATE TRIGGER exchangeRateHistoryImmutable
            BEFORE UPDATE OF createdAt, currency, previousRateMicro, newRateMicro, userName ON exchangeRateHistory
            BEGIN SELECT RAISE(ABORT, 'exchangeRateHistory: el historial es inmutable'); END;
        CREATE TRIGGER exchangeRateHistoryNoDelete BEFORE DELETE ON exchangeRateHistory
            BEGIN SELECT RAISE(ABORT, 'exchangeRateHistory: el historial es inmutable'); END;",
        low = integer_between(1, 9999),
        stale = integer_between(1, 365),
        backup = integer_between(1, 90),
    ))
}

/// Migración 5 (`plan-04` §2.5): unidad y cantidad escritas en cada línea.
/// Las líneas existentes quedan en NULL (= en la unidad del producto).
fn line_units_v5(tx: &Transaction) -> rusqlite::Result<()> {
    tx.execute_batch(
        "ALTER TABLE invoiceLines ADD COLUMN enteredUnitId INTEGER REFERENCES units(id) ON DELETE SET NULL;
        ALTER TABLE invoiceLines ADD COLUMN enteredUnitName TEXT;
        ALTER TABLE invoiceLines ADD COLUMN enteredUnitAbbreviation TEXT;
        ALTER TABLE invoiceLines ADD COLUMN enteredQuantityMilli INTEGER
            CHECK (enteredQuantityMilli IS NULL OR enteredQuantityMilli > 0);
        -- Cantidad escrita, factor y nombre de la unidad: juntos o ninguno.
        ALTER TABLE invoiceLines ADD COLUMN conversionFactorMicro INTEGER CHECK (
            (enteredQuantityMilli IS NULL) = (conversionFactorMicro IS NULL)
            AND (conversionFactorMicro IS NULL OR (conversionFactorMicro > 0 AND enteredUnitName IS NOT NULL))
        );",
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
    fn migration_4_closes_setting_keys_and_ranges_and_seeds_the_defaults() {
        let connection = migrated_v1_database();
        let seeded: Vec<(String, String)> = connection
            .prepare("SELECT key, value FROM settings ORDER BY key")
            .unwrap()
            .query_map([], |row| Ok((row.get(0)?, row.get(1)?)))
            .unwrap()
            .collect::<rusqlite::Result<_>>()
            .unwrap();
        assert_eq!(
            seeded,
            vec![
                ("backupReminderDays".into(), "7".into()),
                ("lowStockThreshold".into(), "5".into()),
                ("staleInvoiceDays".into(), "30".into()),
            ]
        );
        let set = |key: &str, value: &str| {
            connection.execute(
                "INSERT OR REPLACE INTO settings (key, value, updatedAt) VALUES (?1, ?2, '2026-11-04T12:00:00Z')",
                [key, value],
            )
        };
        assert!(set("lowStockThreshold", "9999").is_ok());
        assert!(set("lowStockThreshold", "10000").is_err());
        assert!(set("lowStockThreshold", "0").is_err());
        assert!(set("lowStockThreshold", "05").is_err());
        assert!(set("lowStockThreshold", "5,5").is_err());
        assert!(set("staleInvoiceDays", "366").is_err());
        assert!(set("backupReminderDays", "90").is_ok());
        assert!(set("backupReminderDays", "91").is_err());
        assert!(set("companyName", "Goyma Conexiones").is_ok());
        assert!(set("companyName", &"x".repeat(101)).is_err());
        assert!(set("theme", "dark").is_err());
    }

    #[test]
    fn migration_4_preferences_per_user_cascade_and_rates_with_immutable_history() {
        let connection = migrated_v1_database();
        connection
            .execute(
                "INSERT INTO users (username, passwordHash, role, phone, nationalId, createdAt) VALUES ('ana', 'x', 'contador', '+5350000001', '12345678901', '2026-11-04T12:00:00Z')",
                [],
            )
            .unwrap();
        let user = connection.last_insert_rowid();
        let prefer = |key: &str, value: &str| {
            connection.execute(
                "INSERT OR REPLACE INTO userPreferences (userId, key, value, updatedAt) VALUES (?1, ?2, ?3, '2026-11-04T12:00:00Z')",
                rusqlite::params![user, key, value],
            )
        };
        assert!(prefer("pageSize", "50").is_ok());
        assert!(prefer("pageSize", "25").is_err());
        assert!(prefer("visibleCurrencies", "EUR,CUP").is_ok());
        assert!(prefer("visibleCurrencies", "").is_ok());
        assert!(prefer("visibleCurrencies", "EUR,GBP").is_err());
        assert!(prefer("theme", "dark").is_err());

        let rate = |currency: &str, micro: i64| {
            connection.execute(
                "INSERT OR REPLACE INTO exchangeRates (currency, rateMicro, updatedAt, updatedByName) VALUES (?1, ?2, '2026-11-04T12:00:00Z', 'ana')",
                rusqlite::params![currency, micro],
            )
        };
        assert!(rate("CUP", 670_000_000).is_ok());
        assert!(rate("CUP", 0).is_err());
        assert!(rate("GBP", 1_000_000).is_err());

        connection
            .execute(
                "INSERT INTO exchangeRateHistory (createdAt, currency, previousRateMicro, newRateMicro, userId, userName) VALUES ('2026-11-04T12:00:00Z', 'CUP', NULL, 670000000, ?1, 'ana')",
                [user],
            )
            .unwrap();
        assert!(connection
            .execute("UPDATE exchangeRateHistory SET newRateMicro = 1", [])
            .is_err());
        assert!(connection
            .execute("DELETE FROM exchangeRateHistory", [])
            .is_err());

        // Borrar al usuario: sus preferencias se van; el historial queda sin id.
        connection
            .execute("DELETE FROM users WHERE id = ?1", [user])
            .unwrap();
        let preferences: i64 = connection
            .query_row("SELECT COUNT(*) FROM userPreferences", [], |row| row.get(0))
            .unwrap();
        assert_eq!(preferences, 0);
        let history: (Option<i64>, String) = connection
            .query_row(
                "SELECT userId, userName FROM exchangeRateHistory",
                [],
                |row| Ok((row.get(0)?, row.get(1)?)),
            )
            .unwrap();
        assert_eq!(history, (None, "ana".to_owned()));
    }

    #[test]
    fn migration_5_line_unit_columns_go_together_or_not_at_all() {
        let connection = migrated_v1_database();
        connection
            .execute_batch(
                "INSERT INTO invoices (type, number, invoiceDate, createdByName, createdAt, updatedAt)
                 VALUES ('P', 'P-00001', '2026-11-04', 'ana', '2026-11-04T12:00:00Z', '2026-11-04T12:00:00Z');",
            )
            .unwrap();
        let line = |position: i64,
                    entered: Option<i64>,
                    factor: Option<i64>,
                    unit: Option<&str>| {
            connection.execute(
                "INSERT INTO invoiceLines (invoiceId, position, productIdOriginal, productName, productNameNormalized, quantityMilli, unitPriceCents, subtotalCents, enteredQuantityMilli, conversionFactorMicro, enteredUnitName, createdAt, updatedAt)
                 VALUES (1, ?1, ?1, 'Cable', 'cable', 24000, 250, 6000, ?2, ?3, ?4, '2026-11-04T12:00:00Z', '2026-11-04T12:00:00Z')",
                rusqlite::params![position, entered, factor, unit],
            )
        };
        assert!(line(1, None, None, None).is_ok());
        assert!(line(2, Some(2_000), Some(12_000_000), Some("Caja")).is_ok());
        assert!(line(3, Some(2_000), None, Some("Caja")).is_err());
        assert!(line(4, None, Some(12_000_000), Some("Caja")).is_err());
        assert!(line(5, Some(2_000), Some(12_000_000), None).is_err());
        assert!(line(6, Some(2_000), Some(0), Some("Caja")).is_err());
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
