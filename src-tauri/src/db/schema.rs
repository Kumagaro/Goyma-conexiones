use chrono::{DateTime, SecondsFormat, Utc};
use rusqlite::Connection;

pub const RESERVED_PHONE: &str = "+5300000000";
pub const RESERVED_NATIONAL_ID: &str = "00000000000";

fn ts_check(column: &str) -> String {
    format!(
        "{column} TEXT NOT NULL CHECK ({column} GLOB '[0-9][0-9][0-9][0-9]-[0-9][0-9]-[0-9][0-9]T[0-9][0-9]:[0-9][0-9]:[0-9][0-9]Z' AND strftime('%Y-%m-%dT%H:%M:%SZ', {column}) IS {column})"
    )
}

fn ts_check_opt(column: &str) -> String {
    format!(
        "{column} TEXT CHECK ({column} IS NULL OR ({column} GLOB '[0-9][0-9][0-9][0-9]-[0-9][0-9]-[0-9][0-9]T[0-9][0-9]:[0-9][0-9]:[0-9][0-9]Z' AND strftime('%Y-%m-%dT%H:%M:%SZ', {column}) IS {column}))"
    )
}

pub fn create_schema(conn: &Connection) -> rusqlite::Result<()> {
    conn.execute_batch(
        "CREATE TABLE IF NOT EXISTS users (
            id INTEGER PRIMARY KEY AUTOINCREMENT,
            username TEXT NOT NULL UNIQUE COLLATE NOCASE
                CHECK (length(username) BETWEEN 3 AND 50 AND username NOT GLOB '*[^A-Za-z0-9]*'),
            passwordHash TEXT NOT NULL,
            role TEXT NOT NULL CHECK (role IN ('administrador', 'contador')),
            phone TEXT NOT NULL UNIQUE
                CHECK (phone GLOB '+53[0-9][0-9][0-9][0-9][0-9][0-9][0-9][0-9]'),
            nationalId TEXT NOT NULL UNIQUE
                CHECK (nationalId GLOB '[0-9][0-9][0-9][0-9][0-9][0-9][0-9][0-9][0-9][0-9][0-9]'),
            createdAt TEXT NOT NULL
        );

        CREATE TABLE IF NOT EXISTS sessions (
            id INTEGER PRIMARY KEY CHECK (id = 1),
            userId INTEGER NOT NULL REFERENCES users(id) ON DELETE CASCADE,
            rememberSession INTEGER NOT NULL CHECK (rememberSession IN (0, 1)),
            createdAt TEXT NOT NULL,
            lastActivityAt TEXT NOT NULL
        );

        CREATE TABLE IF NOT EXISTS errorLogs (
            id INTEGER PRIMARY KEY AUTOINCREMENT,
            module TEXT NOT NULL,
            operation TEXT NOT NULL,
            cause TEXT NOT NULL,
            createdAt TEXT NOT NULL
        );

        CREATE TABLE IF NOT EXISTS categories (
            id INTEGER PRIMARY KEY AUTOINCREMENT,
            name TEXT NOT NULL CHECK (length(name) BETWEEN 1 AND 50),
            nameNormalized TEXT NOT NULL UNIQUE CHECK (length(nameNormalized) >= 1),
            description TEXT CHECK (description IS NULL OR length(description) BETWEEN 1 AND 200),
            createdAt TEXT NOT NULL CHECK (
                createdAt GLOB '[0-9][0-9][0-9][0-9]-[0-9][0-9]-[0-9][0-9]T[0-9][0-9]:[0-9][0-9]:[0-9][0-9]Z'
                AND strftime('%Y-%m-%dT%H:%M:%SZ', createdAt) IS createdAt
            ),
            updatedAt TEXT NOT NULL CHECK (
                updatedAt GLOB '[0-9][0-9][0-9][0-9]-[0-9][0-9]-[0-9][0-9]T[0-9][0-9]:[0-9][0-9]:[0-9][0-9]Z'
                AND strftime('%Y-%m-%dT%H:%M:%SZ', updatedAt) IS updatedAt
            )
        );

        CREATE TABLE IF NOT EXISTS units (
            id INTEGER PRIMARY KEY AUTOINCREMENT,
            name TEXT NOT NULL CHECK (length(name) BETWEEN 1 AND 30),
            nameNormalized TEXT NOT NULL UNIQUE CHECK (length(nameNormalized) >= 1),
            abbreviation TEXT NOT NULL CHECK (length(abbreviation) BETWEEN 1 AND 10),
            abbreviationNormalized TEXT NOT NULL UNIQUE CHECK (length(abbreviationNormalized) >= 1),
            createdAt TEXT NOT NULL CHECK (
                createdAt GLOB '[0-9][0-9][0-9][0-9]-[0-9][0-9]-[0-9][0-9]T[0-9][0-9]:[0-9][0-9]:[0-9][0-9]Z'
                AND strftime('%Y-%m-%dT%H:%M:%SZ', createdAt) IS createdAt
            ),
            updatedAt TEXT NOT NULL CHECK (
                updatedAt GLOB '[0-9][0-9][0-9][0-9]-[0-9][0-9]-[0-9][0-9]T[0-9][0-9]:[0-9][0-9]:[0-9][0-9]Z'
                AND strftime('%Y-%m-%dT%H:%M:%SZ', updatedAt) IS updatedAt
            )
        );

        CREATE TABLE IF NOT EXISTS unitConversions (
            id INTEGER PRIMARY KEY AUTOINCREMENT,
            fromUnitId INTEGER NOT NULL REFERENCES units(id) ON DELETE CASCADE,
            toUnitId INTEGER NOT NULL REFERENCES units(id) ON DELETE CASCADE,
            factorMicro INTEGER NOT NULL CHECK (factorMicro BETWEEN 1 AND 999999999999999),
            createdAt TEXT NOT NULL CHECK (
                createdAt GLOB '[0-9][0-9][0-9][0-9]-[0-9][0-9]-[0-9][0-9]T[0-9][0-9]:[0-9][0-9]:[0-9][0-9]Z'
                AND strftime('%Y-%m-%dT%H:%M:%SZ', createdAt) IS createdAt
            ),
            updatedAt TEXT NOT NULL CHECK (
                updatedAt GLOB '[0-9][0-9][0-9][0-9]-[0-9][0-9]-[0-9][0-9]T[0-9][0-9]:[0-9][0-9]:[0-9][0-9]Z'
                AND strftime('%Y-%m-%dT%H:%M:%SZ', updatedAt) IS updatedAt
            ),
            CHECK (fromUnitId <> toUnitId)
        );

        CREATE UNIQUE INDEX IF NOT EXISTS unitConversionsPair
            ON unitConversions (min(fromUnitId, toUnitId), max(fromUnitId, toUnitId));

        CREATE TABLE IF NOT EXISTS products (
            id INTEGER PRIMARY KEY AUTOINCREMENT,
            name TEXT NOT NULL CHECK (length(name) BETWEEN 2 AND 100),
            nameNormalized TEXT NOT NULL CHECK (length(nameNormalized) >= 1),
            description TEXT CHECK (description IS NULL OR length(description) BETWEEN 1 AND 500),
            categoryId INTEGER REFERENCES categories(id) ON DELETE SET NULL,
            unitId INTEGER REFERENCES units(id) ON DELETE SET NULL,
            costCents INTEGER NOT NULL CHECK (costCents BETWEEN 1 AND 99999999),
            profitPercentHundredths INTEGER NOT NULL CHECK (profitPercentHundredths BETWEEN 0 AND 100000),
            expiresOn TEXT CHECK (
                expiresOn IS NULL OR (
                    expiresOn GLOB '[0-9][0-9][0-9][0-9]-[0-9][0-9]-[0-9][0-9]'
                    AND expiresOn <= '2100-12-31'
                    AND date(expiresOn) IS expiresOn
                )
            ),
            quantityMilli INTEGER NOT NULL CHECK (quantityMilli BETWEEN 0 AND 999999999),
            status TEXT NOT NULL DEFAULT 'active' CHECK (status IN ('active', 'discontinued')),
            discontinuedAt TEXT,
            createdAt TEXT NOT NULL CHECK (
                createdAt GLOB '[0-9][0-9][0-9][0-9]-[0-9][0-9]-[0-9][0-9]T[0-9][0-9]:[0-9][0-9]:[0-9][0-9]Z'
                AND strftime('%Y-%m-%dT%H:%M:%SZ', createdAt) IS createdAt
            ),
            updatedAt TEXT NOT NULL CHECK (
                updatedAt GLOB '[0-9][0-9][0-9][0-9]-[0-9][0-9]-[0-9][0-9]T[0-9][0-9]:[0-9][0-9]:[0-9][0-9]Z'
                AND strftime('%Y-%m-%dT%H:%M:%SZ', updatedAt) IS updatedAt
            ),
            CHECK (
                (status = 'active' AND discontinuedAt IS NULL)
                OR (status = 'discontinued' AND discontinuedAt IS NOT NULL AND discontinuedAt GLOB '[0-9][0-9][0-9][0-9]-[0-9][0-9]-[0-9][0-9]T[0-9][0-9]:[0-9][0-9]:[0-9][0-9]Z' AND strftime('%Y-%m-%dT%H:%M:%SZ', discontinuedAt) IS discontinuedAt)
            )
        );

        CREATE UNIQUE INDEX IF NOT EXISTS productsActiveName ON products (nameNormalized) WHERE status = 'active';

        CREATE TABLE IF NOT EXISTS notifications (
            id INTEGER PRIMARY KEY AUTOINCREMENT,
            type TEXT NOT NULL,
            priority TEXT NOT NULL,
            message TEXT NOT NULL,
            createdAt TEXT NOT NULL CHECK (
                createdAt GLOB '[0-9][0-9][0-9][0-9]-[0-9][0-9]-[0-9][0-9]T[0-9][0-9]:[0-9][0-9]:[0-9][0-9]Z'
                AND strftime('%Y-%m-%dT%H:%M:%SZ', createdAt) IS createdAt
            ),
            isRead INTEGER NOT NULL DEFAULT 0 CHECK (isRead IN (0, 1))
        );",
    )?;

    conn.execute_batch(&format!(
        r#"
        CREATE TABLE IF NOT EXISTS clients (
            id INTEGER PRIMARY KEY AUTOINCREMENT,
            firstName TEXT NOT NULL CHECK (length(firstName) BETWEEN 2 AND 50),
            lastName TEXT NOT NULL CHECK (length(lastName) BETWEEN 2 AND 50),
            nationalId TEXT NOT NULL UNIQUE CHECK (nationalId GLOB '[0-9][0-9][0-9][0-9][0-9][0-9][0-9][0-9][0-9][0-9][0-9]'),
            phone TEXT NOT NULL CHECK (phone GLOB '+53[0-9][0-9][0-9][0-9][0-9][0-9][0-9][0-9]'),
            company TEXT CHECK (company IS NULL OR length(company) BETWEEN 1 AND 100),
            position TEXT CHECK (position IS NULL OR length(position) BETWEEN 1 AND 100),
            email TEXT CHECK (email IS NULL OR length(email) BETWEEN 3 AND 100),
            address TEXT CHECK (address IS NULL OR length(address) BETWEEN 1 AND 200),
            invoiceInfo TEXT CHECK (invoiceInfo IS NULL OR length(invoiceInfo) BETWEEN 1 AND 200),
            {client_created},
            {client_updated}
        );
        CREATE TABLE IF NOT EXISTS suppliers (
            id INTEGER PRIMARY KEY AUTOINCREMENT,
            firstName TEXT NOT NULL CHECK (length(firstName) BETWEEN 2 AND 50),
            lastName TEXT NOT NULL CHECK (length(lastName) BETWEEN 2 AND 50),
            nationalId TEXT NOT NULL UNIQUE CHECK (nationalId GLOB '[0-9][0-9][0-9][0-9][0-9][0-9][0-9][0-9][0-9][0-9][0-9]'),
            phone TEXT NOT NULL CHECK (phone GLOB '+53[0-9][0-9][0-9][0-9][0-9][0-9][0-9][0-9]'),
            company TEXT CHECK (company IS NULL OR length(company) BETWEEN 1 AND 100),
            position TEXT CHECK (position IS NULL OR length(position) BETWEEN 1 AND 100),
            email TEXT CHECK (email IS NULL OR length(email) BETWEEN 3 AND 100),
            address TEXT CHECK (address IS NULL OR length(address) BETWEEN 1 AND 200),
            {supplier_created},
            {supplier_updated}
        );
        CREATE TABLE IF NOT EXISTS invoiceCounters (
            type TEXT PRIMARY KEY CHECK (type IN ('E','V','S','GG','GI','P')),
            nextNumber INTEGER NOT NULL CHECK (nextNumber >= 1)
        );
        INSERT OR IGNORE INTO invoiceCounters (type, nextNumber)
            VALUES ('E',1),('V',1),('S',1),('GG',1),('GI',1),('P',1);
        CREATE TABLE IF NOT EXISTS invoices (
            id INTEGER PRIMARY KEY AUTOINCREMENT,
            type TEXT NOT NULL CHECK (type IN ('E','V','S','GG','GI','P')),
            number TEXT NOT NULL UNIQUE CHECK (substr(number, 1, length(type) + 1) = type || '-'),
            invoiceDate TEXT NOT NULL CHECK (invoiceDate GLOB '[0-9][0-9][0-9][0-9]-[0-9][0-9]-[0-9][0-9]' AND date(invoiceDate) IS invoiceDate AND invoiceDate >= '2000-01-01'),
            status TEXT CHECK (status IN ('pending','completed','cancelled')),
            notes TEXT CHECK (notes IS NULL OR length(notes) BETWEEN 1 AND 500),
            paymentMethod TEXT CHECK (paymentMethod IN ('cash','transfer','card','zelle')),
            {paid_at},
            clientId INTEGER REFERENCES clients(id) ON DELETE SET NULL,
            supplierId INTEGER REFERENCES suppliers(id) ON DELETE SET NULL,
            contactFullName TEXT CHECK (contactFullName IS NULL OR length(contactFullName) BETWEEN 5 AND 101),
            contactFullNameNormalized TEXT,
            contactCompany TEXT,
            contactCompanyNormalized TEXT,
            contactNationalId TEXT,
            contactPhone TEXT,
            contactEmail TEXT,
            associatedInvoiceId INTEGER REFERENCES invoices(id) ON DELETE SET NULL,
            associatedNumber TEXT,
            associatedType TEXT CHECK (associatedType IN ('V','S')),
            attachmentFileName TEXT,
            attachmentOriginalName TEXT CHECK (attachmentOriginalName IS NULL OR length(attachmentOriginalName) BETWEEN 1 AND 100),
            attachmentSizeBytes INTEGER,
            {attachment_uploaded_at},
            createdByUserId INTEGER REFERENCES users(id) ON DELETE SET NULL,
            createdByName TEXT NOT NULL CHECK (length(createdByName) BETWEEN 1 AND 50),
            {invoice_created},
            {invoice_updated},
            CHECK ((type = 'P' AND status IS NULL) OR (type <> 'P' AND status IS NOT NULL AND status IN ('pending','completed','cancelled'))),
            CHECK (paidAt IS NULL OR (type IN ('E','V','S') AND status IN ('completed','cancelled'))),
            CHECK (NOT (type IN ('E','V','S') AND status = 'completed' AND paidAt IS NULL)),
            CHECK (paymentMethod IS NULL OR paidAt IS NOT NULL),
            CHECK ((type = 'P' AND contactFullName IS NULL) OR (type <> 'P' AND contactFullName IS NOT NULL AND contactFullNameNormalized IS NOT NULL AND contactNationalId IS NOT NULL AND contactPhone IS NOT NULL)),
            CHECK (clientId IS NULL OR type IN ('V','S','GG','GI')),
            CHECK (supplierId IS NULL OR type = 'E'),
            CHECK ((type IN ('GG','GI') AND associatedNumber IS NOT NULL AND associatedType IS NOT NULL AND (type = 'GG' OR associatedType = 'S')) OR (type NOT IN ('GG','GI') AND associatedInvoiceId IS NULL AND associatedNumber IS NULL AND associatedType IS NULL)),
            CHECK ((attachmentFileName IS NULL AND attachmentOriginalName IS NULL AND attachmentSizeBytes IS NULL AND attachmentUploadedAt IS NULL) OR (type = 'E' AND attachmentFileName IS NOT NULL AND attachmentOriginalName IS NOT NULL AND attachmentSizeBytes IS NOT NULL AND attachmentSizeBytes BETWEEN 1 AND 5242880 AND attachmentUploadedAt IS NOT NULL))
        );
        CREATE TABLE IF NOT EXISTS invoiceLines (
            id INTEGER PRIMARY KEY AUTOINCREMENT,
            invoiceId INTEGER NOT NULL REFERENCES invoices(id) ON DELETE CASCADE,
            position INTEGER NOT NULL CHECK (position BETWEEN 1 AND 100),
            productId INTEGER REFERENCES products(id) ON DELETE SET NULL,
            productIdOriginal INTEGER NOT NULL,
            productName TEXT NOT NULL CHECK (length(productName) BETWEEN 2 AND 100),
            productNameNormalized TEXT NOT NULL CHECK (length(productNameNormalized) >= 1),
            unitId INTEGER REFERENCES units(id) ON DELETE SET NULL,
            unitName TEXT,
            unitAbbreviation TEXT,
            quantityMilli INTEGER NOT NULL CHECK (quantityMilli BETWEEN 1 AND 999999999),
            unitPriceCents INTEGER NOT NULL CHECK (unitPriceCents BETWEEN 0 AND 9999999999),
            subtotalCents INTEGER NOT NULL CHECK (subtotalCents BETWEEN 0 AND 99999999999),
            categoryId INTEGER,
            categoryName TEXT,
            costAtSaleCents INTEGER CHECK (costAtSaleCents IS NULL OR costAtSaleCents BETWEEN 1 AND 99999999),
            {line_created},
            {line_updated},
            UNIQUE (invoiceId, position),
            UNIQUE (invoiceId, productIdOriginal),
            CHECK ((unitName IS NULL) = (unitAbbreviation IS NULL)),
            CHECK (categoryId IS NULL OR categoryName IS NOT NULL)
        );
        CREATE TABLE IF NOT EXISTS lots (
            id INTEGER PRIMARY KEY AUTOINCREMENT,
            productId INTEGER NOT NULL REFERENCES products(id) ON DELETE CASCADE,
            origin TEXT NOT NULL CHECK (origin IN ('initial','entry','restitution')),
            invoiceLineId INTEGER UNIQUE REFERENCES invoiceLines(id) ON DELETE CASCADE,
            arrivalOrder INTEGER NOT NULL,
            initialMilli INTEGER NOT NULL CHECK (initialMilli BETWEEN 1 AND 999999999),
            remainingMilli INTEGER NOT NULL CHECK (remainingMilli BETWEEN 0 AND initialMilli),
            unitCostCents INTEGER NOT NULL CHECK (unitCostCents BETWEEN 1 AND 99999999),
            {lot_created},
            {lot_updated},
            UNIQUE (productId, arrivalOrder),
            CHECK ((origin = 'entry' AND invoiceLineId IS NOT NULL) OR (origin <> 'entry' AND invoiceLineId IS NULL))
        );
        CREATE UNIQUE INDEX IF NOT EXISTS lotsOneInitial ON lots (productId) WHERE origin = 'initial';
        CREATE TABLE IF NOT EXISTS consumptions (
            id INTEGER PRIMARY KEY AUTOINCREMENT,
            invoiceLineId INTEGER NOT NULL REFERENCES invoiceLines(id) ON DELETE CASCADE,
            lotId INTEGER REFERENCES lots(id) ON DELETE SET NULL,
            quantityMilli INTEGER NOT NULL CHECK (quantityMilli BETWEEN 1 AND 999999999),
            unitCostCents INTEGER NOT NULL CHECK (unitCostCents BETWEEN 1 AND 99999999),
            {consumption_created}
        );
        CREATE TABLE IF NOT EXISTS productMovements (
            id INTEGER PRIMARY KEY AUTOINCREMENT,
            {movement_created},
            productId INTEGER REFERENCES products(id) ON DELETE SET NULL,
            productIdOriginal INTEGER NOT NULL,
            productName TEXT NOT NULL CHECK (length(productName) BETWEEN 2 AND 100),
            type TEXT NOT NULL CHECK (type IN ('initial','entry','saleOut','serviceOut','warrantyOut','supplyOut','loss','adjustment','reversal')),
            quantityMilli INTEGER NOT NULL CHECK (quantityMilli BETWEEN -999999999 AND 999999999),
            stockAfterMilli INTEGER NOT NULL CHECK (stockAfterMilli BETWEEN 0 AND 999999999),
            unitPriceCents INTEGER NOT NULL CHECK (unitPriceCents BETWEEN 0 AND 9999999999),
            totalPriceCents INTEGER NOT NULL CHECK (totalPriceCents BETWEEN 0 AND 99999999999),
            invoiceId INTEGER REFERENCES invoices(id) ON DELETE SET NULL,
            invoiceNumber TEXT,
            invoiceType TEXT CHECK (invoiceType IN ('E','V','S','GG','GI','P')),
            userId INTEGER REFERENCES users(id) ON DELETE SET NULL,
            userName TEXT NOT NULL CHECK (length(userName) BETWEEN 1 AND 50),
            CHECK ((type = 'initial' AND invoiceNumber IS NULL AND invoiceType IS NULL) OR (type <> 'initial' AND invoiceNumber IS NOT NULL AND invoiceType IS NOT NULL))
        );
        CREATE INDEX IF NOT EXISTS invoicesTypeDate ON invoices (type, invoiceDate);
        CREATE INDEX IF NOT EXISTS invoicesClient ON invoices (clientId);
        CREATE INDEX IF NOT EXISTS invoicesSupplier ON invoices (supplierId);
        CREATE INDEX IF NOT EXISTS invoiceLinesProduct ON invoiceLines (productId);
        CREATE INDEX IF NOT EXISTS consumptionsLot ON consumptions (lotId);
        CREATE INDEX IF NOT EXISTS consumptionsLine ON consumptions (invoiceLineId);
        CREATE INDEX IF NOT EXISTS productMovementsProduct ON productMovements (productIdOriginal, createdAt);
        CREATE TRIGGER IF NOT EXISTS invoiceCountersNoDecrease BEFORE UPDATE OF nextNumber ON invoiceCounters
            WHEN NEW.nextNumber < OLD.nextNumber
            BEGIN SELECT RAISE(ABORT, 'invoiceCounters: nextNumber no puede decrecer'); END;
        CREATE TRIGGER IF NOT EXISTS invoiceCountersNoDelete BEFORE DELETE ON invoiceCounters
            BEGIN SELECT RAISE(ABORT, 'invoiceCounters: no se puede borrar'); END;
        CREATE TRIGGER IF NOT EXISTS productMovementsImmutable
            BEFORE UPDATE OF createdAt, productIdOriginal, productName, type, quantityMilli, stockAfterMilli,
                unitPriceCents, totalPriceCents, invoiceNumber, invoiceType, userName ON productMovements
            BEGIN SELECT RAISE(ABORT, 'productMovements: el historial es inmutable'); END;
        CREATE TRIGGER IF NOT EXISTS productMovementsNoDelete BEFORE DELETE ON productMovements
            BEGIN SELECT RAISE(ABORT, 'productMovements: el historial es inmutable'); END;
        "#,
        client_created = ts_check("createdAt"),
        client_updated = ts_check("updatedAt"),
        supplier_created = ts_check("createdAt"),
        supplier_updated = ts_check("updatedAt"),
        invoice_created = ts_check("createdAt"),
        invoice_updated = ts_check("updatedAt"),
        paid_at = ts_check_opt("paidAt"),
        attachment_uploaded_at = ts_check_opt("attachmentUploadedAt"),
        line_created = ts_check("createdAt"),
        line_updated = ts_check("updatedAt"),
        lot_created = ts_check("createdAt"),
        lot_updated = ts_check("updatedAt"),
        consumption_created = ts_check("createdAt"),
        movement_created = ts_check("createdAt"),
    ))?;

    Ok(())
}

pub fn init_schema(conn: &Connection, now: DateTime<Utc>) -> rusqlite::Result<()> {
    create_schema(conn)?;
    let user_count: i64 = conn.query_row("SELECT COUNT(*) FROM users", [], |row| row.get(0))?;
    if user_count == 0 {
        let bcrypt_cost = if cfg!(test) { 4 } else { 10 };
        let password_hash = bcrypt::hash("Admin123!", bcrypt_cost)
            .map_err(|error| rusqlite::Error::ToSqlConversionFailure(Box::new(error)))?;
        conn.execute(
            "INSERT INTO users (username, passwordHash, role, phone, nationalId, createdAt)
             VALUES ('admin', ?1, 'administrador', ?2, ?3, ?4)",
            (
                password_hash,
                RESERVED_PHONE,
                RESERVED_NATIONAL_ID,
                now.to_rfc3339_opts(SecondsFormat::Secs, true),
            ),
        )?;
    }

    Ok(())
}

pub fn clear_non_persistent_sessions(conn: &Connection) -> rusqlite::Result<usize> {
    conn.execute("DELETE FROM sessions WHERE rememberSession = 0", [])
}

pub fn purge_error_logs(conn: &Connection, now: DateTime<Utc>) -> rusqlite::Result<usize> {
    let cutoff = (now - chrono::Duration::days(90)).to_rfc3339_opts(SecondsFormat::Secs, true);
    conn.execute("DELETE FROM errorLogs WHERE createdAt < ?1", [cutoff])
}

#[cfg(test)]
mod tests {
    use chrono::Utc;
    use rusqlite::Connection;

    use super::{init_schema, RESERVED_NATIONAL_ID, RESERVED_PHONE};

    fn setup_db() -> Connection {
        let conn = Connection::open_in_memory().unwrap();
        conn.pragma_update(None, "foreign_keys", "ON").unwrap();
        init_schema(&conn, Utc::now()).unwrap();
        conn
    }

    fn insert_user(
        conn: &Connection,
        username: &str,
        phone: &str,
        national_id: &str,
    ) -> rusqlite::Result<usize> {
        conn.execute(
            "INSERT INTO users (username, passwordHash, role, phone, nationalId, createdAt)
             VALUES (?1, 'hash', 'contador', ?2, ?3, '2026-09-26T00:00:00Z')",
            (username, phone, national_id),
        )
    }

    #[test]
    fn inventory_tables_and_constraints_are_created() {
        let conn = setup_db();

        let names: Vec<String> = conn
            .prepare("SELECT name FROM sqlite_master WHERE type = 'table' AND name IN ('categories', 'units', 'unitConversions', 'products', 'notifications') ORDER BY name")
            .unwrap()
            .query_map([], |row| row.get(0))
            .unwrap()
            .collect::<rusqlite::Result<Vec<_>>>()
            .unwrap();

        assert_eq!(
            names,
            vec![
                "categories",
                "notifications",
                "products",
                "unitConversions",
                "units"
            ]
        );

        let category_insert = conn.execute(
            "INSERT INTO categories (name, nameNormalized, description, createdAt, updatedAt) VALUES ('Café', 'cafe', NULL, '2026-10-03T00:00:00Z', '2026-10-03T00:00:00Z')",
            [],
        );
        assert!(category_insert.is_ok());

        let duplicate_name = conn.execute(
            "INSERT INTO categories (name, nameNormalized, description, createdAt, updatedAt) VALUES ('cafe', 'cafe', NULL, '2026-10-03T00:00:00Z', '2026-10-03T00:00:00Z')",
            [],
        );
        assert!(duplicate_name.is_err());

        let bad_ts = conn.execute(
            "INSERT INTO categories (name, nameNormalized, description, createdAt, updatedAt) VALUES ('Pan', 'pan', NULL, '2026-13-03T00:00:00Z', '2026-10-03T00:00:00Z')",
            [],
        );
        assert!(bad_ts.is_err());
    }

    #[test]
    fn spec03_schema_creates_tables_counters_indexes_and_immutability_triggers() {
        let conn = setup_db();
        let tables: Vec<String> = conn
            .prepare("SELECT name FROM sqlite_master WHERE type = 'table' AND name IN ('clients', 'suppliers', 'invoiceCounters', 'invoices', 'invoiceLines', 'lots', 'consumptions', 'productMovements') ORDER BY name")
            .unwrap()
            .query_map([], |row| row.get(0))
            .unwrap()
            .collect::<rusqlite::Result<Vec<_>>>()
            .unwrap();
        assert_eq!(
            tables,
            vec![
                "clients",
                "consumptions",
                "invoiceCounters",
                "invoiceLines",
                "invoices",
                "lots",
                "productMovements",
                "suppliers"
            ]
        );

        let counters: i64 = conn
            .query_row(
                "SELECT COUNT(*) FROM invoiceCounters WHERE nextNumber = 1",
                [],
                |row| row.get(0),
            )
            .unwrap();
        assert_eq!(counters, 6);
        for table in [
            "clients",
            "suppliers",
            "invoices",
            "invoiceLines",
            "lots",
            "consumptions",
            "productMovements",
        ] {
            let count: i64 = conn
                .query_row(&format!("SELECT COUNT(*) FROM {table}"), [], |row| {
                    row.get(0)
                })
                .unwrap();
            assert_eq!(count, 0, "{table} no debe tener datos semilla");
        }

        for name in [
            "lotsOneInitial",
            "invoicesTypeDate",
            "invoicesClient",
            "invoicesSupplier",
            "invoiceLinesProduct",
            "consumptionsLot",
            "consumptionsLine",
            "productMovementsProduct",
        ] {
            let exists: i64 = conn
                .query_row(
                    "SELECT COUNT(*) FROM sqlite_master WHERE type = 'index' AND name = ?1",
                    [name],
                    |row| row.get(0),
                )
                .unwrap();
            assert_eq!(exists, 1, "falta índice {name}");
        }
        for name in [
            "invoiceCountersNoDecrease",
            "invoiceCountersNoDelete",
            "productMovementsImmutable",
            "productMovementsNoDelete",
        ] {
            let exists: i64 = conn
                .query_row(
                    "SELECT COUNT(*) FROM sqlite_master WHERE type = 'trigger' AND name = ?1",
                    [name],
                    |row| row.get(0),
                )
                .unwrap();
            assert_eq!(exists, 1, "falta trigger {name}");
        }
    }

    #[test]
    fn spec03_contacts_enforce_ci_phone_lengths_and_timestamp_checks() {
        let conn = setup_db();
        let insert_client = |first_name: &str, national_id: &str, phone: &str, created_at: &str| {
            conn.execute(
                "INSERT INTO clients (firstName, lastName, nationalId, phone, createdAt, updatedAt)
                 VALUES (?1, 'Pérez', ?2, ?3, ?4, '2026-10-03T00:00:00Z')",
                (first_name, national_id, phone, created_at),
            )
        };
        insert_client("Ana", "12345678901", "+5355551234", "2026-10-03T00:00:00Z").unwrap();
        assert!(
            insert_client("Luis", "12345678902", "+5355551234", "2026-10-03T00:00:00Z").is_ok()
        );
        assert!(
            insert_client("Ana", "12345678901", "+5355551235", "2026-10-03T00:00:00Z").is_err()
        );
        for (name, id, phone, created_at) in [
            ("A", "12345678903", "+5355551236", "2026-10-03T00:00:00Z"),
            ("Ana", "1234567890x", "+5355551237", "2026-10-03T00:00:00Z"),
            ("Ana", "12345678904", "+535555123", "2026-10-03T00:00:00Z"),
            ("Ana", "12345678905", "+5355551238", "2026-13-03T00:00:00Z"),
            ("Ana", "12345678906", "+5355551239", "2026-02-30T00:00:00Z"),
        ] {
            assert!(insert_client(name, id, phone, created_at).is_err());
        }

        let supplier_columns: Vec<String> = conn
            .prepare("PRAGMA table_info(suppliers)")
            .unwrap()
            .query_map([], |row| row.get(1))
            .unwrap()
            .collect::<rusqlite::Result<Vec<_>>>()
            .unwrap();
        assert!(!supplier_columns
            .iter()
            .any(|column| column == "invoiceInfo"));
    }

    #[test]
    fn spec03_invoice_counters_are_structural_and_never_decrease_or_delete() {
        let conn = setup_db();
        let count: i64 = conn
            .query_row("SELECT COUNT(*) FROM invoiceCounters", [], |row| row.get(0))
            .unwrap();
        assert_eq!(count, 6);
        init_schema(&conn, Utc::now()).unwrap();
        let count_after_reinit: i64 = conn
            .query_row("SELECT COUNT(*) FROM invoiceCounters", [], |row| row.get(0))
            .unwrap();
        assert_eq!(count_after_reinit, 6);
        assert!(conn
            .execute(
                "UPDATE invoiceCounters SET nextNumber = 2 WHERE type = 'E'",
                []
            )
            .is_ok());
        let decrease = conn.execute(
            "UPDATE invoiceCounters SET nextNumber = 1 WHERE type = 'E'",
            [],
        );
        assert!(decrease
            .unwrap_err()
            .to_string()
            .contains("invoiceCounters: nextNumber no puede decrecer"));
        let delete = conn.execute("DELETE FROM invoiceCounters WHERE type = 'E'", []);
        assert!(delete
            .unwrap_err()
            .to_string()
            .contains("invoiceCounters: no se puede borrar"));
        assert!(conn
            .execute("INSERT INTO invoiceCounters VALUES ('X', 1)", [])
            .is_err());
        assert!(conn
            .execute(
                "UPDATE invoiceCounters SET nextNumber = 0 WHERE type = 'E'",
                []
            )
            .is_err());
    }

    #[test]
    fn spec03_invoice_line_lot_and_consumption_checks_enforce_ranges_and_relations() {
        let conn = setup_db();
        let supplier_id: i64 = {
            conn.execute(
                "INSERT INTO suppliers (firstName, lastName, nationalId, phone, createdAt, updatedAt)
                 VALUES ('Juan', 'Pérez', '12345678901', '+5355551234', '2026-10-03T00:00:00Z', '2026-10-03T00:00:00Z')",
                [],
            ).unwrap();
            conn.last_insert_rowid()
        };
        let user_id: i64 = conn
            .query_row("SELECT id FROM users WHERE username = 'admin'", [], |row| {
                row.get(0)
            })
            .unwrap();
        conn.execute(
            "INSERT INTO invoices (type, number, invoiceDate, status, supplierId, contactFullName,
                contactFullNameNormalized, contactNationalId, contactPhone, createdByUserId, createdByName, createdAt, updatedAt)
             VALUES ('E', 'E-00001', '2026-10-03', 'pending', ?1, 'Juan Pérez', 'juan perez', '12345678901', '+5355551234', ?2, 'admin', '2026-10-03T00:00:00Z', '2026-10-03T00:00:00Z')",
            (supplier_id, user_id),
        ).unwrap();
        let invoice_id = conn.last_insert_rowid();
        let product_id = crate::test_utils::insert_product_fixture(
            &conn, "Café", None, None, 100, 0, 1, "active", None, None,
        );

        let insert_line = |position: i64, quantity: i64, product_id_original: i64| {
            conn.execute(
                "INSERT INTO invoiceLines (invoiceId, position, productId, productIdOriginal, productName, productNameNormalized, quantityMilli, unitPriceCents, subtotalCents, createdAt, updatedAt)
                 VALUES (?1, ?2, ?3, ?4, 'Café', 'cafe', ?5, 100, 100, '2026-10-03T00:00:00Z', '2026-10-03T00:00:00Z')",
                (invoice_id, position, product_id, product_id_original, quantity),
            )
        };
        assert!(insert_line(1, 1, product_id).is_ok());
        assert!(insert_line(1, 1, product_id + 1).is_err());
        assert!(insert_line(2, 0, product_id + 2).is_err());
        assert!(insert_line(2, 1_000_000_000, product_id + 2).is_err());

        let line_id: i64 = conn
            .query_row(
                "SELECT id FROM invoiceLines WHERE position = 1",
                [],
                |row| row.get(0),
            )
            .unwrap();
        assert!(conn.execute(
            "INSERT INTO lots (productId, origin, invoiceLineId, arrivalOrder, initialMilli, remainingMilli, unitCostCents, createdAt, updatedAt) VALUES (?1, 'entry', ?2, 1, 10, 11, 100, '2026-10-03T00:00:00Z', '2026-10-03T00:00:00Z')",
            (product_id, line_id),
        ).is_err());
        assert!(conn.execute(
            "INSERT INTO lots (productId, origin, invoiceLineId, arrivalOrder, initialMilli, remainingMilli, unitCostCents, createdAt, updatedAt) VALUES (?1, 'entry', ?2, 1, 10, 10, 100, '2026-10-03T00:00:00Z', '2026-10-03T00:00:00Z')",
            (product_id, line_id),
        ).is_ok());
        assert!(conn.execute(
            "INSERT INTO lots (productId, origin, invoiceLineId, arrivalOrder, initialMilli, remainingMilli, unitCostCents, createdAt, updatedAt) VALUES (?1, 'initial', ?2, 2, 10, 10, 100, '2026-10-03T00:00:00Z', '2026-10-03T00:00:00Z')",
            (product_id, line_id),
        ).is_err());
        let lot_id: i64 = conn
            .query_row("SELECT id FROM lots WHERE origin = 'entry'", [], |row| {
                row.get(0)
            })
            .unwrap();
        assert!(conn.execute("INSERT INTO consumptions (invoiceLineId, lotId, quantityMilli, unitCostCents, createdAt) VALUES (?1, ?2, 1, 100, '2026-10-03T00:00:00Z')", (line_id, lot_id)).is_ok());
        assert!(conn.execute("INSERT INTO consumptions (invoiceLineId, lotId, quantityMilli, unitCostCents, createdAt) VALUES (?1, ?2, 0, 100, '2026-10-03T00:00:00Z')", (line_id, lot_id)).is_err());
    }

    #[test]
    fn spec03_product_movements_are_immutable_but_allow_foreign_key_set_null() {
        let conn = setup_db();
        let user_id: i64 = conn
            .query_row("SELECT id FROM users WHERE username = 'admin'", [], |row| {
                row.get(0)
            })
            .unwrap();
        let product_id = crate::test_utils::insert_product_fixture(
            &conn, "Café", None, None, 100, 0, 1, "active", None, None,
        );
        conn.execute(
            "INSERT INTO productMovements (createdAt, productId, productIdOriginal, productName, type, quantityMilli, stockAfterMilli, unitPriceCents, totalPriceCents, userId, userName)
             VALUES ('2026-10-03T00:00:00Z', ?1, ?1, 'Café', 'initial', 1, 1, 100, 100, ?2, 'admin')",
            (product_id, user_id),
        ).unwrap();
        let update = conn.execute("UPDATE productMovements SET quantityMilli = 0", []);
        assert!(update
            .unwrap_err()
            .to_string()
            .contains("productMovements: el historial es inmutable"));
        let delete = conn.execute("DELETE FROM productMovements", []);
        assert!(delete
            .unwrap_err()
            .to_string()
            .contains("productMovements: el historial es inmutable"));
        conn.execute("DELETE FROM products WHERE id = ?1", [product_id])
            .unwrap();
        let (product_link, product_name): (Option<i64>, String) = conn
            .query_row(
                "SELECT productId, productName FROM productMovements",
                [],
                |row| Ok((row.get(0)?, row.get(1)?)),
            )
            .unwrap();
        assert_eq!(product_link, None);
        assert_eq!(product_name, "Café");
        conn.execute("DELETE FROM users WHERE id = ?1", [user_id])
            .unwrap();
        let user_link: Option<i64> = conn
            .query_row("SELECT userId FROM productMovements", [], |row| row.get(0))
            .unwrap();
        assert_eq!(user_link, None);
    }

    #[test]
    fn username_is_unique_without_case_sensitivity() {
        let conn = setup_db();
        insert_user(&conn, "admin2", "+5300000001", "00000000001").unwrap();

        assert!(insert_user(&conn, "Admin2", "+5300000002", "00000000002").is_err());
    }

    #[test]
    fn init_schema_is_idempotent_and_creates_bootstrap_admin() {
        let conn = Connection::open_in_memory().unwrap();
        let now = Utc::now();
        init_schema(&conn, now).unwrap();
        init_schema(&conn, now).unwrap();

        let (count, username, password_hash, role, phone, national_id): (
            i64,
            String,
            String,
            String,
            String,
            String,
        ) = conn
            .query_row(
                "SELECT COUNT(*), MIN(username), MIN(passwordHash), MIN(role), MIN(phone), MIN(nationalId) FROM users",
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

        assert_eq!(count, 1);
        assert_eq!(username, "admin");
        assert_eq!(role, "administrador");
        assert!(bcrypt::verify("Admin123!", &password_hash).unwrap());
        assert_eq!(phone, RESERVED_PHONE);
        assert_eq!(national_id, RESERVED_NATIONAL_ID);
    }

    #[test]
    fn session_table_allows_only_id_one() {
        let conn = setup_db();
        let user_id: i64 = conn
            .query_row("SELECT id FROM users WHERE username = 'admin'", [], |row| {
                row.get(0)
            })
            .unwrap();
        conn.execute(
            "INSERT INTO sessions (id, userId, rememberSession, createdAt, lastActivityAt)
             VALUES (1, ?1, 0, '2026-09-26T00:00:00Z', '2026-09-26T00:00:00Z')",
            [user_id],
        )
        .unwrap();

        assert!(conn
            .execute(
                "INSERT INTO sessions (id, userId, rememberSession, createdAt, lastActivityAt)
             VALUES (2, ?1, 0, '2026-09-26T00:00:00Z', '2026-09-26T00:00:00Z')",
                [user_id],
            )
            .is_err());
    }

    #[test]
    fn username_check_enforces_ascii_alphanumeric_length() {
        let conn = setup_db();
        for invalid in ["ab", &"a".repeat(51), "jose_1", "josé", "user name"] {
            assert!(insert_user(&conn, invalid, "+5300000001", "00000000001").is_err());
        }
        assert!(insert_user(&conn, "abc", "+5300000001", "00000000001").is_ok());
        assert!(insert_user(&conn, &"a".repeat(50), "+5300000002", "00000000002").is_ok());
    }

    #[test]
    fn phone_check_enforces_cuban_phone_format() {
        let conn = setup_db();
        assert!(insert_user(&conn, "phoneok", "+5312345678", "00000000001").is_ok());
        for (index, phone) in ["+531234567", "+53123456789", "5312345678", "+53abcdefgh"]
            .into_iter()
            .enumerate()
        {
            let national_id = format!("{:011}", index + 2);
            assert!(insert_user(&conn, &format!("phonebad{index}"), phone, &national_id).is_err());
        }
    }

    #[test]
    fn national_id_check_enforces_eleven_digits() {
        let conn = setup_db();
        assert!(insert_user(&conn, "idok", "+5300000001", "12345678901").is_ok());
        for (index, national_id) in ["1234567890", "123456789012", "1234567890a"]
            .into_iter()
            .enumerate()
        {
            assert!(insert_user(
                &conn,
                &format!("idbad{index}"),
                &format!("+530000000{}", index + 2),
                national_id
            )
            .is_err());
        }
    }

    #[test]
    fn remember_session_check_accepts_only_zero_or_one() {
        let conn = setup_db();
        let user_id: i64 = conn
            .query_row("SELECT id FROM users WHERE username = 'admin'", [], |row| {
                row.get(0)
            })
            .unwrap();
        for remember_session in [0, 1] {
            conn.execute(
                "INSERT INTO sessions (id, userId, rememberSession, createdAt, lastActivityAt)
                 VALUES (1, ?1, ?2, '2026-09-26T00:00:00Z', '2026-09-26T00:00:00Z')",
                (user_id, remember_session),
            )
            .unwrap();
            conn.execute("DELETE FROM sessions", []).unwrap();
        }

        assert!(conn
            .execute(
                "INSERT INTO sessions (id, userId, rememberSession, createdAt, lastActivityAt)
             VALUES (1, ?1, 2, '2026-09-26T00:00:00Z', '2026-09-26T00:00:00Z')",
                [user_id],
            )
            .is_err());
    }

    #[test]
    fn clear_non_persistent_sessions_removes_only_remember_session_zero() {
        let conn = setup_db();
        let user_id: i64 = conn
            .query_row("SELECT id FROM users WHERE username = 'admin'", [], |row| {
                row.get(0)
            })
            .unwrap();
        conn.execute(
            "INSERT INTO sessions (id, userId, rememberSession, createdAt, lastActivityAt)
             VALUES (1, ?1, 0, '2026-09-26T00:00:00Z', '2026-09-26T00:00:00Z')",
            [user_id],
        )
        .unwrap();

        super::clear_non_persistent_sessions(&conn).unwrap();

        let remaining: i64 = conn
            .query_row("SELECT COUNT(*) FROM sessions", [], |row| row.get(0))
            .unwrap();
        assert_eq!(remaining, 0);

        conn.execute(
            "INSERT INTO sessions (id, userId, rememberSession, createdAt, lastActivityAt)
             VALUES (1, ?1, 1, '2026-09-26T00:00:00Z', '2026-09-26T00:00:00Z')",
            [user_id],
        )
        .unwrap();
        assert_eq!(super::clear_non_persistent_sessions(&conn).unwrap(), 0);
        let remember_session: i64 = conn
            .query_row("SELECT rememberSession FROM sessions", [], |row| row.get(0))
            .unwrap();
        assert_eq!(remember_session, 1);

        conn.execute("DELETE FROM sessions", []).unwrap();
        assert_eq!(super::clear_non_persistent_sessions(&conn).unwrap(), 0);
    }

    #[test]
    fn startup_purge_removes_error_logs_older_than_ninety_days() {
        let conn = setup_db();
        let now = Utc::now();
        let old_timestamp = (now - chrono::Duration::days(91)).to_rfc3339();
        let recent_timestamp = (now - chrono::Duration::days(89)).to_rfc3339();
        conn.execute(
            "INSERT INTO errorLogs (module, operation, cause, createdAt)
             VALUES ('auth', 'test', 'old', ?1), ('auth', 'test', 'recent', ?2)",
            (&old_timestamp, &recent_timestamp),
        )
        .unwrap();

        super::purge_error_logs(&conn, now).unwrap();

        let causes: Vec<String> = conn
            .prepare("SELECT cause FROM errorLogs")
            .unwrap()
            .query_map([], |row| row.get(0))
            .unwrap()
            .collect::<rusqlite::Result<_>>()
            .unwrap();
        assert_eq!(causes, ["recent"]);
    }
}
