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
    (6, drafts_v6),
    (7, activity_log_v7),
    (8, application_id_v8),
    (9, cost_base_and_threshold_v9),
    (10, invoices_and_contacts_v10),
];

/// Migraciones que reconstruyen tablas (`plan-06` §2.1): SQLite no puede
/// quitar un `UNIQUE` ni un `CHECK`, y borrar la tabla vieja con las claves
/// foráneas activas dispararía los `ON DELETE CASCADE`. Para estas, el
/// ejecutor las desactiva fuera de la transacción y comprueba la integridad
/// antes de confirmar.
pub const REBUILDS_TABLES: &[i64] = &[10];

/// Identifica una base de Goyma Conexiones ("GOYM" en ASCII) al restaurar
/// una copia (RP-03).
pub const APPLICATION_ID: i64 = 0x474F_594D;

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
    run_migrations_with(connection, MIGRATIONS, REBUILDS_TABLES)
}

/// Versión de la primera migración pendiente que reconstruye tablas, si la
/// hay; la usa `initialize_database` para copiar la base antes (D-04). Una
/// base recién creada (versión 0) no tiene nada que copiar.
pub fn pending_rebuild(connection: &Connection) -> rusqlite::Result<Option<i64>> {
    let current = schema_version(connection)?;
    if current == 0 {
        return Ok(None);
    }
    Ok(REBUILDS_TABLES
        .iter()
        .copied()
        .find(|version| *version > current))
}

/// Para tests: deja la base en una versión anterior concreta.
#[cfg(test)]
pub fn migrate_up_to(connection: &mut Connection, version: i64) -> Result<i64, AppError> {
    let migrations: Vec<(i64, Migration)> = MIGRATIONS
        .iter()
        .copied()
        .filter(|(number, _)| *number <= version)
        .collect();
    run_migrations_with(connection, &migrations, REBUILDS_TABLES)
}

fn run_migrations_with(
    connection: &mut Connection,
    migrations: &[(i64, Migration)],
    rebuilds: &[i64],
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
        let rebuild = rebuilds.contains(version);
        if rebuild {
            // Fuera de la transacción: dentro, este PRAGMA no tiene efecto.
            connection
                .pragma_update(None, "foreign_keys", "OFF")
                .map_err(|error| {
                    migration_failed(&format!("failed to disable foreign keys: {error}"))
                })?;
        }
        let result = apply_migration(connection, *version, *migrate, rebuild);
        if rebuild {
            // Siempre se reactivan, también si la migración falló.
            let restored = connection.pragma_update(None, "foreign_keys", "ON");
            result?;
            restored.map_err(|error| {
                migration_failed(&format!("failed to restore foreign keys: {error}"))
            })?;
        } else {
            result?;
        }
        current = *version;
    }
    Ok(current)
}

/// Cada migración y su número de versión van en la misma transacción: si
/// falla, la base queda en la versión anterior.
fn apply_migration(
    connection: &mut Connection,
    version: i64,
    migrate: Migration,
    rebuild: bool,
) -> Result<(), AppError> {
    let transaction = connection.transaction().map_err(|error| {
        migration_failed(&format!("failed to open migration {version}: {error}"))
    })?;
    migrate(&transaction)
        .map_err(|error| migration_failed(&format!("migration {version} failed: {error}")))?;
    if rebuild {
        let violations: i64 = transaction
            .query_row("SELECT COUNT(*) FROM pragma_foreign_key_check", [], |row| {
                row.get(0)
            })
            .map_err(|error| {
                migration_failed(&format!(
                    "migration {version} integrity check failed: {error}"
                ))
            })?;
        if violations > 0 {
            return Err(migration_failed(&format!(
                "migration {version} left {violations} foreign key violations"
            )));
        }
    }
    transaction
        .pragma_update(None, "user_version", version)
        .and_then(|()| transaction.commit())
        .map_err(|error| migration_failed(&format!("migration {version} failed: {error}")))
}

/// Reconstruye `table` desde el SQL que la propia base guarda, con cambios
/// exactos (`plan-06` §2.1). Si un cambio no encaja con el SQL guardado, falla
/// en vez de seguir a medias. Conserva ids, datos, índices y disparadores.
/// Las columnas nuevas (nombre y valor inicial) se declaran con un cambio que
/// las ponga antes de las restricciones de tabla, como hace SQLite al añadirlas.
fn rebuild_table(
    tx: &Transaction,
    table: &str,
    edits: &[(&str, &str)],
    extra_columns: &[(&str, &str)],
) -> rusqlite::Result<()> {
    let sql: String = tx.query_row(
        "SELECT sql FROM sqlite_master WHERE type = 'table' AND name = ?1",
        [table],
        |row| row.get(0),
    )?;
    let open = sql
        .find('(')
        .ok_or_else(|| rusqlite::Error::InvalidColumnName(format!("{table}: sin columnas")))?;
    let mut new_sql = format!("CREATE TABLE {table}New {}", &sql[open..]);
    for (from, to) in edits {
        if !new_sql.contains(from) {
            return Err(rusqlite::Error::InvalidColumnName(format!(
                "{table}: no se encontró «{from}»"
            )));
        }
        new_sql = new_sql.replacen(from, to, 1);
    }
    let dependents: Vec<String> = tx
        .prepare(
            "SELECT sql FROM sqlite_master
             WHERE tbl_name = ?1 AND type IN ('index', 'trigger') AND sql IS NOT NULL",
        )?
        .query_map([table], |row| row.get(0))?
        .collect::<rusqlite::Result<_>>()?;
    let columns: Vec<String> = tx
        .prepare(&format!("SELECT name FROM pragma_table_info('{table}')"))?
        .query_map([], |row| row.get(0))?
        .collect::<rusqlite::Result<_>>()?;

    tx.execute_batch(&new_sql)?;
    let mut targets = columns.clone();
    let mut sources = columns;
    for (name, expression) in extra_columns {
        targets.push((*name).to_owned());
        sources.push((*expression).to_owned());
    }
    tx.execute(
        &format!(
            "INSERT INTO {table}New ({}) SELECT {} FROM {table}",
            targets.join(", "),
            sources.join(", ")
        ),
        [],
    )?;
    tx.execute_batch(&format!(
        "DROP TABLE {table}; ALTER TABLE {table}New RENAME TO {table};"
    ))?;
    for statement in dependents {
        tx.execute_batch(&statement)?;
    }
    Ok(())
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

/// Migración 6 (`plan-04` §2.6): un borrador por usuario, formulario y
/// registro; "nuevo" (sin registro) cuenta como uno solo.
fn drafts_v6(tx: &Transaction) -> rusqlite::Result<()> {
    let updated_at_check = CREATED_AT_CHECK.replace("createdAt", "updatedAt");
    tx.execute_batch(&format!(
        "CREATE TABLE drafts (
            id INTEGER PRIMARY KEY AUTOINCREMENT,
            userId INTEGER NOT NULL REFERENCES users(id) ON DELETE CASCADE,
            formKey TEXT NOT NULL CHECK (length(formKey) BETWEEN 1 AND 60),
            recordId INTEGER,
            payload TEXT NOT NULL CHECK (length(payload) <= 200000),
            createdAt TEXT NOT NULL CHECK ({CREATED_AT_CHECK}),
            updatedAt TEXT NOT NULL CHECK ({updated_at_check})
        );
        CREATE UNIQUE INDEX draftsOnePerForm ON drafts (userId, formKey, COALESCE(recordId, 0));"
    ))
}

/// Migración 7 (`plan-04` §2.7): registro inmutable y sin borrado (RA-02).
/// La única escritura admitida es la del `ON DELETE SET NULL` al eliminar un
/// usuario: `userName` conserva la copia de su nombre.
fn activity_log_v7(tx: &Transaction) -> rusqlite::Result<()> {
    tx.execute_batch(&format!(
        "CREATE TABLE activityLog (
            id INTEGER PRIMARY KEY AUTOINCREMENT,
            createdAt TEXT NOT NULL CHECK ({CREATED_AT_CHECK}),
            userId INTEGER REFERENCES users(id) ON DELETE SET NULL,
            userName TEXT NOT NULL,
            action TEXT NOT NULL CHECK (action IN ('delete','cancel','edit','config','backup','restore','import','purge','expire')),
            entityType TEXT NOT NULL CHECK (entityType IN ('product','category','unit','conversion','client','supplier','user','invoice','settings','exchangeRate','backup','csv')),
            entityLabel TEXT NOT NULL,
            details TEXT
        );
        CREATE INDEX activityLogCreated ON activityLog (createdAt);
        CREATE TRIGGER activityLogImmutable BEFORE UPDATE ON activityLog
        WHEN NOT (
            OLD.userId IS NOT NULL AND NEW.userId IS NULL
            AND NEW.id = OLD.id AND NEW.createdAt = OLD.createdAt
            AND NEW.userName = OLD.userName AND NEW.action = OLD.action
            AND NEW.entityType = OLD.entityType AND NEW.entityLabel = OLD.entityLabel
            AND NEW.details IS OLD.details
        )
        BEGIN SELECT RAISE(ABORT, 'activityLog is immutable'); END;
        CREATE TRIGGER activityLogNoDelete BEFORE DELETE ON activityLog
        BEGIN SELECT RAISE(ABORT, 'activityLog is immutable'); END;"
    ))
}

/// Migración 8 (`plan-04` §2.8): la marca que valida una copia (RP-03).
fn application_id_v8(tx: &Transaction) -> rusqlite::Result<()> {
    tx.execute_batch(&format!("PRAGMA application_id = {APPLICATION_ID};"))
}

/// Migración 9 (`plan-06` §2.2): costo promedio móvil desde un costo base y
/// umbral de pocas unidades por producto. Los productos existentes conservan
/// su costo (que pasa a ser el base) y quedan sin umbral hasta editarlos.
fn cost_base_and_threshold_v9(tx: &Transaction) -> rusqlite::Result<()> {
    tx.execute_batch(
        "ALTER TABLE products ADD COLUMN costBaseCents INTEGER
            CHECK (costBaseCents IS NULL OR costBaseCents BETWEEN 1 AND 99999999);
        ALTER TABLE products ADD COLUMN costBaseArrivalOrder INTEGER NOT NULL DEFAULT 0;
        ALTER TABLE products ADD COLUMN lowStockThresholdMilli INTEGER
            CHECK (lowStockThresholdMilli IS NULL OR lowStockThresholdMilli BETWEEN 1 AND 999999999);
        ALTER TABLE lots ADD COLUMN stockBeforeMilli INTEGER
            CHECK (stockBeforeMilli IS NULL OR stockBeforeMilli BETWEEN 0 AND 999999999);
        UPDATE products SET costBaseCents = costCents,
            costBaseArrivalOrder = COALESCE((SELECT MAX(arrivalOrder) FROM lots WHERE lots.productId = products.id), 0);
        DELETE FROM settings WHERE key = 'lowStockThreshold';",
    )
}

/// Migración 10 (`plan-06` §2.3, reconstruye tablas): contactos con solo el
/// nombre obligatorio, un producto puede repetirse en una factura, precio de
/// lista, descuento y entregas por línea, y las tablas de servicios (vacías).
fn invoices_and_contacts_v10(tx: &Transaction) -> rusqlite::Result<()> {
    let updated_at = CREATED_AT_CHECK.replace("createdAt", "updatedAt");
    tx.execute_batch(&format!(
        "CREATE TABLE services (
            id INTEGER PRIMARY KEY AUTOINCREMENT,
            name TEXT NOT NULL CHECK (length(name) BETWEEN 2 AND 100),
            nameNormalized TEXT NOT NULL UNIQUE CHECK (length(nameNormalized) >= 1),
            description TEXT CHECK (description IS NULL OR length(description) BETWEEN 1 AND 500),
            suggestedPriceCents INTEGER CHECK (suggestedPriceCents IS NULL OR suggestedPriceCents BETWEEN 0 AND 9999999999),
            createdAt TEXT NOT NULL CHECK ({CREATED_AT_CHECK}),
            updatedAt TEXT NOT NULL CHECK ({updated_at})
        );
        CREATE TABLE serviceProducts (
            serviceId INTEGER NOT NULL REFERENCES services(id) ON DELETE CASCADE,
            productId INTEGER NOT NULL REFERENCES products(id) ON DELETE CASCADE,
            suggestedQuantityMilli INTEGER NOT NULL CHECK (suggestedQuantityMilli BETWEEN 1 AND 999999999),
            position INTEGER NOT NULL CHECK (position BETWEEN 1 AND 100),
            PRIMARY KEY (serviceId, productId),
            UNIQUE (serviceId, position)
        );
        CREATE TABLE invoiceServiceLines (
            id INTEGER PRIMARY KEY AUTOINCREMENT,
            invoiceId INTEGER NOT NULL REFERENCES invoices(id) ON DELETE CASCADE,
            position INTEGER NOT NULL CHECK (position BETWEEN 1 AND 100),
            serviceId INTEGER REFERENCES services(id) ON DELETE SET NULL,
            serviceName TEXT NOT NULL CHECK (length(serviceName) BETWEEN 2 AND 100),
            priceCents INTEGER NOT NULL CHECK (priceCents BETWEEN 0 AND 9999999999),
            discountKind TEXT CHECK (discountKind IS NULL OR discountKind IN ('percent','amount')),
            discountValue INTEGER CHECK ((discountKind IS NULL) = (discountValue IS NULL) AND (discountValue IS NULL OR discountValue >= 0)),
            discountCents INTEGER NOT NULL DEFAULT 0 CHECK (discountCents BETWEEN 0 AND priceCents),
            totalCents INTEGER NOT NULL CHECK (totalCents = priceCents - discountCents),
            createdAt TEXT NOT NULL CHECK ({CREATED_AT_CHECK}),
            updatedAt TEXT NOT NULL CHECK ({updated_at}),
            UNIQUE (invoiceId, position)
        );
        CREATE INDEX invoiceServiceLinesInvoice ON invoiceServiceLines (invoiceId);"
    ))?;

    // CT-01: solo el nombre es obligatorio; si se escriben, mismo formato.
    let contact_edits: &[(&str, &str)] = &[
        (
            "lastName TEXT NOT NULL CHECK (length(lastName) BETWEEN 2 AND 50)",
            "lastName TEXT CHECK (lastName IS NULL OR length(lastName) BETWEEN 2 AND 50)",
        ),
        (
            "nationalId TEXT NOT NULL UNIQUE CHECK (nationalId GLOB",
            "nationalId TEXT UNIQUE CHECK (nationalId IS NULL OR nationalId GLOB",
        ),
        (
            "phone TEXT NOT NULL CHECK (phone GLOB",
            "phone TEXT CHECK (phone IS NULL OR phone GLOB",
        ),
    ];
    rebuild_table(tx, "clients", contact_edits, &[])?;
    rebuild_table(tx, "suppliers", contact_edits, &[])?;

    rebuild_table(
        tx,
        "invoices",
        &[
            (
                "length(contactFullName) BETWEEN 5 AND 101",
                "length(contactFullName) BETWEEN 2 AND 101",
            ),
            (
                " AND contactNationalId IS NOT NULL AND contactPhone IS NOT NULL",
                "",
            ),
        ],
        &[],
    )?;

    // Venta y Servicio no canceladas ya descontaron su stock: quedan
    // entregadas (EN-03). El bruto vacío equivale al importe sin descuento.
    let sale_or_service =
        "(SELECT type FROM invoices WHERE invoices.id = invoiceLines.invoiceId) IN ('V','S')";
    let list_price = format!("CASE WHEN {sale_or_service} THEN unitPriceCents END");
    let delivered = format!(
        "CASE WHEN {sale_or_service} AND (SELECT status FROM invoices WHERE invoices.id = invoiceLines.invoiceId) <> 'cancelled' THEN quantityMilli ELSE 0 END"
    );
    rebuild_table(
        tx,
        "invoiceLines",
        &[
            ("UNIQUE (invoiceId, productIdOriginal),", ""),
            (
                "UNIQUE (invoiceId, position),",
                "listPriceCents INTEGER CHECK (listPriceCents IS NULL OR listPriceCents BETWEEN 0 AND 9999999999),
            grossCents INTEGER CHECK (grossCents IS NULL OR grossCents BETWEEN 0 AND 99999999999),
            discountKind TEXT CHECK (discountKind IS NULL OR discountKind IN ('percent','amount')),
            discountValue INTEGER CHECK ((discountKind IS NULL) = (discountValue IS NULL) AND (discountValue IS NULL OR discountValue >= 0)),
            discountCents INTEGER NOT NULL DEFAULT 0 CHECK ((grossCents IS NULL AND discountCents = 0) OR (grossCents IS NOT NULL AND discountCents BETWEEN 0 AND grossCents AND subtotalCents = grossCents - discountCents)),
            serviceLineId INTEGER REFERENCES invoiceServiceLines(id) ON DELETE CASCADE,
            deliveredMilli INTEGER NOT NULL DEFAULT 0 CHECK (deliveredMilli BETWEEN 0 AND quantityMilli),
            UNIQUE (invoiceId, position),",
            ),
        ],
        &[
            ("listPriceCents", &list_price),
            ("grossCents", "NULL"),
            ("discountKind", "NULL"),
            ("discountValue", "NULL"),
            ("discountCents", "0"),
            ("serviceLineId", "NULL"),
            ("deliveredMilli", &delivered),
        ],
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

        assert_eq!(
            run_migrations_with(&mut connection, migrations, &[]).unwrap(),
            3
        );
        assert!(table_exists(&connection, "migrationFirst"));
        assert!(table_exists(&connection, "migrationSecond"));
        // Ya aplicadas: no se repiten (CREATE TABLE fallaría si lo hicieran).
        assert_eq!(
            run_migrations_with(&mut connection, migrations, &[]).unwrap(),
            3
        );
    }

    #[test]
    fn a_failing_migration_leaves_the_previous_version_and_reports_migration_failed() {
        let mut connection = Connection::open_in_memory().unwrap();
        create_schema(&connection).unwrap();
        let migrations: &[(i64, Migration)] = &[(2, add_first), (3, fails_halfway)];

        let error = run_migrations_with(&mut connection, migrations, &[]).unwrap_err();
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

    fn insert_user(connection: &Connection, name: &str, national_id: &str) -> i64 {
        connection
            .execute(
                "INSERT INTO users (username, passwordHash, role, phone, nationalId, createdAt)
                 VALUES (?1, 'hash', 'contador', '+5350000030', ?2, '2026-10-28T12:00:00Z')",
                (name, national_id),
            )
            .unwrap();
        connection.last_insert_rowid()
    }

    fn insert_draft(
        connection: &Connection,
        user_id: i64,
        record_id: Option<i64>,
    ) -> rusqlite::Result<usize> {
        connection.execute(
            "INSERT INTO drafts (userId, formKey, recordId, payload, createdAt, updatedAt)
             VALUES (?1, 'product', ?2, '{}', '2026-10-28T12:00:00Z', '2026-10-28T12:00:00Z')",
            (user_id, record_id),
        )
    }

    fn insert_activity(connection: &Connection, user_id: i64) {
        connection
            .execute(
                "INSERT INTO activityLog (createdAt, userId, userName, action, entityType, entityLabel)
                 VALUES ('2026-10-28T12:00:00Z', ?1, 'Ana', 'delete', 'product', 'Cable')",
                [user_id],
            )
            .unwrap();
    }

    #[test]
    fn migration_6_keeps_one_draft_per_user_form_and_record_and_cascades() {
        let connection = migrated_v1_database();
        let user_id = insert_user(&connection, "draftuser", "12345678930");

        insert_draft(&connection, user_id, None).unwrap();
        insert_draft(&connection, user_id, Some(4)).unwrap();
        assert!(insert_draft(&connection, user_id, None).is_err());
        assert!(insert_draft(&connection, user_id, Some(4)).is_err());
        connection
            .execute("DELETE FROM users WHERE id = ?1", [user_id])
            .unwrap();
        let drafts: i64 = connection
            .query_row("SELECT COUNT(*) FROM drafts", [], |row| row.get(0))
            .unwrap();
        assert_eq!(drafts, 0);
    }

    #[test]
    fn migration_7_makes_the_activity_log_immutable_but_lets_users_be_deleted() {
        let connection = migrated_v1_database();
        let user_id = insert_user(&connection, "activityuser", "12345678931");
        insert_activity(&connection, user_id);

        assert!(connection
            .execute("UPDATE activityLog SET entityLabel = 'Otro'", [])
            .is_err());
        assert!(connection.execute("DELETE FROM activityLog", []).is_err());

        connection
            .execute("DELETE FROM users WHERE id = ?1", [user_id])
            .unwrap();
        let (user, name): (Option<i64>, String) = connection
            .query_row("SELECT userId, userName FROM activityLog", [], |row| {
                Ok((row.get(0)?, row.get(1)?))
            })
            .unwrap();
        assert_eq!(user, None);
        assert_eq!(name, "Ana");
    }

    #[test]
    fn migration_9_keeps_costs_as_the_base_and_leaves_thresholds_empty() {
        let mut connection = Connection::open_in_memory().unwrap();
        connection
            .pragma_update(None, "foreign_keys", "ON")
            .unwrap();
        create_schema(&connection).unwrap();
        let first_eight: Vec<(i64, Migration)> = super::MIGRATIONS
            .iter()
            .copied()
            .filter(|(version, _)| *version <= 8)
            .collect();
        run_migrations_with(&mut connection, &first_eight, &[]).unwrap();
        connection
            .execute(
                "INSERT INTO products (name, nameNormalized, costCents, profitPercentHundredths, quantityMilli, createdAt, updatedAt)
                 VALUES ('Cable', 'cable', 1250, 2000, 5000, '2026-11-01T10:00:00Z', '2026-11-01T10:00:00Z')",
                [],
            )
            .unwrap();
        let product_id = connection.last_insert_rowid();
        connection
            .execute(
                "INSERT INTO lots (productId, origin, arrivalOrder, initialMilli, remainingMilli, unitCostCents, createdAt, updatedAt)
                 VALUES (?1, 'initial', 3, 5000, 5000, 1000, '2026-11-01T10:00:00Z', '2026-11-01T10:00:00Z')",
                [product_id],
            )
            .unwrap();

        run_migrations(&mut connection).unwrap();

        let migrated: (i64, Option<i64>, i64, Option<i64>) = connection
            .query_row(
                "SELECT costCents, costBaseCents, costBaseArrivalOrder, lowStockThresholdMilli FROM products WHERE id = ?1",
                [product_id],
                |row| Ok((row.get(0)?, row.get(1)?, row.get(2)?, row.get(3)?)),
            )
            .unwrap();
        assert_eq!(migrated, (1250, Some(1250), 3, None));
        let stock_before: Option<i64> = connection
            .query_row("SELECT stockBeforeMilli FROM lots", [], |row| row.get(0))
            .unwrap();
        assert_eq!(stock_before, None);
        let general_threshold: i64 = connection
            .query_row(
                "SELECT COUNT(*) FROM settings WHERE key = 'lowStockThreshold'",
                [],
                |row| row.get(0),
            )
            .unwrap();
        assert_eq!(general_threshold, 0);
        assert!(connection
            .execute(
                "UPDATE products SET lowStockThresholdMilli = 0 WHERE id = ?1",
                [product_id]
            )
            .is_err());
    }

    fn create_parent_and_child(tx: &Transaction) -> rusqlite::Result<()> {
        tx.execute_batch(
            "CREATE TABLE parent (id INTEGER PRIMARY KEY, name TEXT NOT NULL UNIQUE);
             CREATE TABLE child (id INTEGER PRIMARY KEY,
                 parentId INTEGER NOT NULL REFERENCES parent(id) ON DELETE CASCADE);
             INSERT INTO parent (id, name) VALUES (1, 'a'), (2, 'b');
             INSERT INTO child (id, parentId) VALUES (1, 1), (2, 2);",
        )
    }

    fn rebuild_parent(tx: &Transaction) -> rusqlite::Result<()> {
        tx.execute_batch(
            "CREATE TABLE parentNew (id INTEGER PRIMARY KEY, name TEXT NOT NULL);
             INSERT INTO parentNew (id, name) SELECT id, name FROM parent;
             DROP TABLE parent;
             ALTER TABLE parentNew RENAME TO parent;",
        )
    }

    fn leave_an_orphan(tx: &Transaction) -> rusqlite::Result<()> {
        tx.execute_batch("INSERT INTO child (id, parentId) VALUES (3, 99);")
    }

    fn children(connection: &Connection) -> i64 {
        connection
            .query_row("SELECT COUNT(*) FROM child", [], |row| row.get(0))
            .unwrap()
    }

    fn foreign_keys_on(connection: &Connection) -> bool {
        connection
            .query_row("PRAGMA foreign_keys", [], |row| row.get::<_, i64>(0))
            .unwrap()
            == 1
    }

    fn base_with_foreign_keys() -> Connection {
        let connection = Connection::open_in_memory().unwrap();
        connection
            .pragma_update(None, "foreign_keys", "ON")
            .unwrap();
        connection
            .pragma_update(None, "user_version", BASE_VERSION)
            .unwrap();
        connection
    }

    #[test]
    fn rebuilding_a_table_keeps_the_rows_that_reference_it() {
        let migrations: &[(i64, Migration)] = &[(2, create_parent_and_child), (3, rebuild_parent)];

        // Sin la protección, borrar la tabla vieja arrastra a sus hijas.
        let mut unprotected = base_with_foreign_keys();
        run_migrations_with(&mut unprotected, migrations, &[]).unwrap();
        assert_eq!(children(&unprotected), 0);

        let mut protected = base_with_foreign_keys();
        run_migrations_with(&mut protected, migrations, &[3]).unwrap();
        assert_eq!(children(&protected), 2);
        assert!(foreign_keys_on(&protected));
        // Las claves siguen funcionando después.
        protected
            .execute("DELETE FROM parent WHERE id = 1", [])
            .unwrap();
        assert_eq!(children(&protected), 1);
    }

    #[test]
    fn a_rebuild_that_leaves_orphans_is_rolled_back() {
        let mut connection = base_with_foreign_keys();
        let migrations: &[(i64, Migration)] = &[(2, create_parent_and_child), (3, leave_an_orphan)];
        let error = run_migrations_with(&mut connection, migrations, &[3]).unwrap_err();
        assert_eq!(error.code, "migrationFailed");
        assert_eq!(schema_version(&connection).unwrap(), 2);
        assert_eq!(children(&connection), 2);
        assert!(foreign_keys_on(&connection));
    }

    /// Base en la versión 9 con datos en todas las tablas que toca la 10.
    fn version_9_with_data() -> Connection {
        let mut connection = Connection::open_in_memory().unwrap();
        connection
            .pragma_update(None, "foreign_keys", "ON")
            .unwrap();
        create_schema(&connection).unwrap();
        let up_to_nine: Vec<(i64, Migration)> = super::MIGRATIONS
            .iter()
            .copied()
            .filter(|(version, _)| *version <= 9)
            .collect();
        run_migrations_with(&mut connection, &up_to_nine, &[]).unwrap();
        let ts = "'2026-11-01T10:00:00Z'";
        connection
            .execute_batch(&format!(
                "INSERT INTO users (id, username, passwordHash, role, phone, nationalId, createdAt)
                    VALUES (7, 'ana', 'x', 'administrador', '+5350000001', '12345678901', {ts});
                 INSERT INTO clients (id, firstName, lastName, nationalId, phone, createdAt, updatedAt)
                    VALUES (1, 'Luis', 'Gómez', '12345678902', '+5350000002', {ts}, {ts});
                 INSERT INTO suppliers (id, firstName, lastName, nationalId, phone, createdAt, updatedAt)
                    VALUES (1, 'Rosa', 'Díaz', '12345678903', '+5350000003', {ts}, {ts});
                 INSERT INTO products (id, name, nameNormalized, costCents, profitPercentHundredths, quantityMilli, createdAt, updatedAt)
                    VALUES (1, 'Cable', 'cable', 100, 2000, 8000, {ts}, {ts});
                 INSERT INTO invoices (id, type, number, invoiceDate, status, supplierId, contactFullName, contactFullNameNormalized,
                    contactNationalId, contactPhone, createdByUserId, createdByName, createdAt, updatedAt)
                    VALUES (1, 'E', 'E-00001', '2026-11-01', 'pending', 1, 'Rosa Díaz', 'rosa diaz', '12345678903', '+5350000003', 7, 'ana', {ts}, {ts});
                 INSERT INTO invoices (id, type, number, invoiceDate, status, clientId, contactFullName, contactFullNameNormalized,
                    contactNationalId, contactPhone, createdByUserId, createdByName, createdAt, updatedAt)
                    VALUES (2, 'V', 'V-00001', '2026-11-01', 'pending', 1, 'Luis Gómez', 'luis gomez', '12345678902', '+5350000002', 7, 'ana', {ts}, {ts}),
                           (3, 'V', 'V-00002', '2026-11-01', 'cancelled', 1, 'Luis Gómez', 'luis gomez', '12345678902', '+5350000002', 7, 'ana', {ts}, {ts});
                 INSERT INTO invoices (id, type, number, invoiceDate, status, clientId, contactFullName, contactFullNameNormalized,
                    contactNationalId, contactPhone, associatedInvoiceId, associatedNumber, associatedType, createdByUserId, createdByName, createdAt, updatedAt)
                    VALUES (4, 'GG', 'GG-00001', '2026-11-01', 'pending', 1, 'Luis Gómez', 'luis gomez', '12345678902', '+5350000002', 2, 'V-00001', 'V', 7, 'ana', {ts}, {ts});
                 INSERT INTO invoiceLines (id, invoiceId, position, productId, productIdOriginal, productName, productNameNormalized,
                    quantityMilli, unitPriceCents, subtotalCents, createdAt, updatedAt)
                    VALUES (1, 1, 1, 1, 1, 'Cable', 'cable', 10000, 100, 1000, {ts}, {ts}),
                           (2, 2, 1, 1, 1, 'Cable', 'cable', 2000, 120, 240, {ts}, {ts}),
                           (3, 3, 1, 1, 1, 'Cable', 'cable', 1000, 120, 120, {ts}, {ts}),
                           (4, 4, 1, 1, 1, 'Cable', 'cable', 1000, 0, 0, {ts}, {ts});
                 INSERT INTO lots (id, productId, origin, invoiceLineId, arrivalOrder, initialMilli, remainingMilli, unitCostCents, createdAt, updatedAt)
                    VALUES (1, 1, 'entry', 1, 1, 10000, 7000, 100, {ts}, {ts});
                 INSERT INTO consumptions (id, invoiceLineId, lotId, quantityMilli, unitCostCents, createdAt)
                    VALUES (1, 2, 1, 2000, 100, {ts}), (2, 4, 1, 1000, 100, {ts});
                 INSERT INTO productMovements (createdAt, productId, productIdOriginal, productName, type, quantityMilli, stockAfterMilli,
                    unitPriceCents, totalPriceCents, invoiceId, invoiceNumber, invoiceType, userId, userName)
                    VALUES ({ts}, 1, 1, 'Cable', 'entry', 10000, 10000, 100, 1000, 1, 'E-00001', 'E', 7, 'ana');"
            ))
            .unwrap();
        connection
    }

    fn table_fingerprints(connection: &Connection) -> Vec<(String, i64, i64)> {
        let tables: Vec<String> = connection
            .prepare(
                "SELECT name FROM sqlite_master WHERE type = 'table' AND name NOT LIKE 'sqlite_%' ORDER BY name",
            )
            .unwrap()
            .query_map([], |row| row.get(0))
            .unwrap()
            .collect::<rusqlite::Result<_>>()
            .unwrap();
        tables
            .into_iter()
            .map(|table| {
                let (count, ids): (i64, i64) = connection
                    .query_row(
                        &format!("SELECT COUNT(*), COALESCE(SUM(rowid), 0) FROM {table}"),
                        [],
                        |row| Ok((row.get(0)?, row.get(1)?)),
                    )
                    .unwrap();
                (table, count, ids)
            })
            .collect()
    }

    /// (id, precio de lista, entregado, bruto, descuento) de una línea migrada.
    type MigratedLine = (i64, Option<i64>, i64, Option<i64>, i64);

    #[test]
    fn migration_10_rebuilds_invoices_and_contacts_without_losing_anything() {
        let mut connection = version_9_with_data();
        let before = table_fingerprints(&connection);

        run_migrations(&mut connection).unwrap();

        let after: Vec<(String, i64, i64)> = table_fingerprints(&connection)
            .into_iter()
            .filter(|(table, _, _)| before.iter().any(|(name, _, _)| name == table))
            .collect();
        assert_eq!(after, before);
        for new_table in ["services", "serviceProducts", "invoiceServiceLines"] {
            assert!(table_exists(&connection, new_table));
        }
        let violations: i64 = connection
            .query_row("SELECT COUNT(*) FROM pragma_foreign_key_check", [], |row| {
                row.get(0)
            })
            .unwrap();
        assert_eq!(violations, 0);

        // Venta pendiente: entregada y con su precio de lista; la cancelada y
        // las demás, sin entregar ni precio de lista; ningún bruto (= subtotal).
        let lines: Vec<MigratedLine> = connection
            .prepare(
                "SELECT id, listPriceCents, deliveredMilli, grossCents, discountCents FROM invoiceLines ORDER BY id",
            )
            .unwrap()
            .query_map([], |row| {
                Ok((row.get(0)?, row.get(1)?, row.get(2)?, row.get(3)?, row.get(4)?))
            })
            .unwrap()
            .collect::<rusqlite::Result<_>>()
            .unwrap();
        assert_eq!(
            lines,
            vec![
                (1, None, 0, None, 0),
                (2, Some(120), 2000, None, 0),
                (3, Some(120), 0, None, 0),
                (4, None, 0, None, 0),
            ]
        );

        // Los índices siguen; las claves foráneas y sus cascadas también.
        let reservations_index: i64 = connection
            .query_row(
                "SELECT COUNT(*) FROM sqlite_master WHERE name = 'invoicesActiveReservations'",
                [],
                |row| row.get(0),
            )
            .unwrap();
        assert_eq!(reservations_index, 1);
        connection
            .execute("DELETE FROM invoices WHERE id = 3", [])
            .unwrap();
        let cancelled_lines: i64 = connection
            .query_row(
                "SELECT COUNT(*) FROM invoiceLines WHERE invoiceId = 3",
                [],
                |row| row.get(0),
            )
            .unwrap();
        assert_eq!(cancelled_lines, 0);
    }

    #[test]
    fn migration_10_accepts_contacts_with_only_a_name_and_repeated_products() {
        let mut connection = version_9_with_data();
        run_migrations(&mut connection).unwrap();
        let ts = "'2026-11-02T10:00:00Z'";
        connection
            .execute_batch(&format!(
                "INSERT INTO clients (firstName, createdAt, updatedAt) VALUES ('Mía', {ts}, {ts});
                 INSERT INTO clients (firstName, createdAt, updatedAt) VALUES ('Leo', {ts}, {ts});
                 INSERT INTO suppliers (firstName, createdAt, updatedAt) VALUES ('Taller', {ts}, {ts});
                 INSERT INTO invoices (type, number, invoiceDate, status, clientId, contactFullName, contactFullNameNormalized,
                    createdByUserId, createdByName, createdAt, updatedAt)
                    VALUES ('V', 'V-00009', '2026-11-02', 'pending', 2, 'Mía', 'mia', 7, 'ana', {ts}, {ts});
                 INSERT INTO invoiceLines (invoiceId, position, productId, productIdOriginal, productName, productNameNormalized,
                    quantityMilli, unitPriceCents, subtotalCents, createdAt, updatedAt)
                    VALUES (last_insert_rowid(), 1, 1, 1, 'Cable', 'cable', 1000, 200, 200, {ts}, {ts});"
            ))
            .unwrap();
        let invoice: i64 = connection
            .query_row(
                "SELECT id FROM invoices WHERE number = 'V-00009'",
                [],
                |row| row.get(0),
            )
            .unwrap();
        // El mismo producto otra vez en la misma factura (servicios, D-02).
        connection
            .execute(
                &format!(
                    "INSERT INTO invoiceLines (invoiceId, position, productId, productIdOriginal, productName, productNameNormalized,
                        quantityMilli, unitPriceCents, subtotalCents, createdAt, updatedAt)
                     VALUES (?1, 2, 1, 1, 'Cable', 'cable', 1000, 200, 200, {ts}, {ts})"
                ),
                [invoice],
            )
            .unwrap();
        // Formato y unicidad siguen cuando se escriben.
        assert!(connection
            .execute(
                &format!("INSERT INTO clients (firstName, phone, createdAt, updatedAt) VALUES ('Eva', '555', {ts}, {ts})"),
                [],
            )
            .is_err());
        assert!(connection
            .execute(
                &format!("INSERT INTO clients (firstName, nationalId, createdAt, updatedAt) VALUES ('Eva', '12345678902', {ts}, {ts})"),
                [],
            )
            .is_err());
        // Descuento: bruto − descuento = importe.
        assert!(connection
            .execute(
                "UPDATE invoiceLines SET grossCents = 200, discountKind = 'amount', discountValue = 50, discountCents = 50 WHERE invoiceId = ?1 AND position = 1",
                [invoice],
            )
            .is_err());
        connection
            .execute(
                "UPDATE invoiceLines SET grossCents = 200, discountKind = 'amount', discountValue = 50, discountCents = 50, subtotalCents = 150 WHERE invoiceId = ?1 AND position = 1",
                [invoice],
            )
            .unwrap();
    }

    #[test]
    fn pending_rebuild_reports_the_first_rebuild_ahead() {
        let mut connection = version_9_with_data();
        assert_eq!(super::pending_rebuild(&connection).unwrap(), Some(10));
        run_migrations(&mut connection).unwrap();
        assert_eq!(super::pending_rebuild(&connection).unwrap(), None);
        let fresh = Connection::open_in_memory().unwrap();
        assert_eq!(super::pending_rebuild(&fresh).unwrap(), None);
    }

    #[test]
    fn migration_8_marks_the_database_as_goyma() {
        let connection = migrated_v1_database();
        let id: i64 = connection
            .query_row("PRAGMA application_id", [], |row| row.get(0))
            .unwrap();
        assert_eq!(id, super::APPLICATION_ID);
    }
}
