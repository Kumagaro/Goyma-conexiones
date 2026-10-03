use chrono::{DateTime, SecondsFormat, Utc};
use rusqlite::Connection;

pub const RESERVED_PHONE: &str = "+5300000000";
pub const RESERVED_NATIONAL_ID: &str = "00000000000";

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
