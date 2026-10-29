use std::error::Error;
use std::fs::create_dir_all;
use std::path::{Path, PathBuf};
use std::sync::{Mutex, MutexGuard, OnceLock};

use chrono::{DateTime, Utc};
use rusqlite::Connection;
use tauri::{Manager, State};

use super::migrations::run_migrations;
use super::schema::{clear_non_persistent_sessions, init_schema, purge_error_logs};
use crate::attachments::sweep_orphans;
use crate::errors::append_error_log;

static APP_DATA_DIR: OnceLock<PathBuf> = OnceLock::new();

pub fn resolve_app_data_dir(app: &tauri::AppHandle) -> Result<PathBuf, Box<dyn Error>> {
    let app_data_dir = app.path().app_data_dir()?;
    create_dir_all(&app_data_dir)?;
    Ok(app_data_dir)
}

pub fn open_database(app_data_dir: &Path) -> rusqlite::Result<Connection> {
    create_dir_all(app_data_dir)
        .map_err(|error| rusqlite::Error::ToSqlConversionFailure(Box::new(error)))?;
    let conn = Connection::open(app_data_dir.join("goyma.sqlite3"))?;
    conn.pragma_update(None, "foreign_keys", "ON")?;
    Ok(conn)
}

pub fn initialize_database(app_data_dir: &Path, now: DateTime<Utc>) -> Result<Connection, String> {
    let opened = (|| -> rusqlite::Result<Connection> {
        let conn = open_database(app_data_dir)?;
        init_schema(&conn, now)?;
        Ok(conn)
    })();
    let mut conn = opened.map_err(|error| {
        append_error_log(app_data_dir, &error.to_string());
        "No se pudo abrir la base de datos".to_owned()
    })?;

    // MG-01: la base llega a la última versión antes de usarse. Si no puede,
    // la aplicación no arranca a medias.
    run_migrations(&mut conn).map_err(|error| {
        append_error_log(app_data_dir, &format!("migrations: {}", error.cause));
        error.message
    })?;

    let maintenance = (|| -> rusqlite::Result<()> {
        clear_non_persistent_sessions(&conn)?;
        purge_error_logs(&conn, now)?;
        let _ = sweep_orphans(app_data_dir, &conn);
        Ok(())
    })();
    maintenance.map_err(|error| {
        append_error_log(app_data_dir, &error.to_string());
        "No se pudo abrir la base de datos".to_owned()
    })?;
    Ok(conn)
}

pub fn lock_conn<'a>(state: &State<'a, Mutex<Connection>>) -> MutexGuard<'a, Connection> {
    lock_connection(state.inner())
}

fn lock_connection(connection: &Mutex<Connection>) -> MutexGuard<'_, Connection> {
    connection
        .lock()
        .unwrap_or_else(|poisoned| poisoned.into_inner())
}

pub fn install_panic_hook(app_data_dir: &Path) {
    let _ = APP_DATA_DIR.set(app_data_dir.to_path_buf());
    std::panic::set_hook(Box::new(|panic_info| {
        if let Some(app_data_dir) = APP_DATA_DIR.get() {
            append_error_log(app_data_dir, &panic_info.to_string());
        }
    }));
}

pub fn current_app_data_dir() -> Option<&'static Path> {
    APP_DATA_DIR.get().map(PathBuf::as_path)
}

#[cfg(test)]
mod tests {
    use std::fs;
    use std::panic::{catch_unwind, AssertUnwindSafe};
    use std::path::PathBuf;
    use std::sync::atomic::{AtomicU64, Ordering};
    use std::sync::Mutex;

    use chrono::Utc;
    use rusqlite::Connection;

    use super::{initialize_database, lock_connection, open_database};

    static NEXT_TEST_PATH: AtomicU64 = AtomicU64::new(0);

    fn unique_path() -> PathBuf {
        let sequence = NEXT_TEST_PATH.fetch_add(1, Ordering::Relaxed);
        std::env::temp_dir().join(format!("goyma-db-test-{}-{sequence}", std::process::id()))
    }

    #[test]
    fn lock_connection_recovers_after_mutex_poisoning() {
        let connection = Mutex::new(Connection::open_in_memory().unwrap());
        let panic_result = catch_unwind(AssertUnwindSafe(|| {
            let _guard = connection.lock().unwrap();
            panic!("simulated panic while holding database lock");
        }));
        assert!(panic_result.is_err());

        let guard = lock_connection(&connection);
        assert!(guard.is_autocommit());
    }

    #[test]
    fn database_open_failure_is_logged_and_returns_generic_message() {
        let app_data_dir = unique_path();
        fs::create_dir_all(&app_data_dir).unwrap();
        fs::create_dir(app_data_dir.join("goyma.sqlite3")).unwrap();

        let error = initialize_database(&app_data_dir, Utc::now()).unwrap_err();

        assert_eq!(error, "No se pudo abrir la base de datos");
        let log = fs::read_to_string(app_data_dir.join("errors.log")).unwrap();
        assert!(log.contains("goyma.sqlite3"));
        fs::remove_dir_all(app_data_dir).unwrap();
    }

    #[test]
    fn initialize_database_sweeps_uuid_attachment_orphans_best_effort() {
        let app_data_dir = unique_path();
        let attachment_dir = app_data_dir.join("attachments");
        fs::create_dir_all(&attachment_dir).unwrap();
        let now = Utc::now();
        let timestamp = crate::timestamps::format_utc_timestamp(now);
        let connection = open_database(&app_data_dir).unwrap();
        crate::db::schema::init_schema(&connection, now).unwrap();
        for (number, file_name) in [("E-00001", "one.pdf"), ("E-00002", "two.pdf")] {
            connection
                .execute(
                    "INSERT INTO invoices (type, number, invoiceDate, status, contactFullName,
                        contactFullNameNormalized, contactNationalId, contactPhone, createdByName,
                        attachmentFileName, attachmentOriginalName, attachmentSizeBytes,
                        attachmentUploadedAt, createdAt, updatedAt)
                     VALUES ('E', ?1, '2026-10-04', 'pending', 'Ana Pérez', 'ana perez',
                        '12345678901', '+5350000000', 'admin', ?2, 'factura.pdf', 8, ?3, ?3, ?3)",
                    (number, file_name, &timestamp),
                )
                .unwrap();
            fs::write(attachment_dir.join(file_name), b"%PDF-ref").unwrap();
        }
        let orphan = attachment_dir.join("550e8400-e29b-41d4-a716-446655440000.pdf");
        fs::write(&orphan, b"%PDF-orphan").unwrap();
        drop(connection);

        let connection = initialize_database(&app_data_dir, now).unwrap();

        assert!(attachment_dir.join("one.pdf").exists());
        assert!(attachment_dir.join("two.pdf").exists());
        assert!(!orphan.exists());
        drop(connection);
        fs::remove_dir_all(app_data_dir).unwrap();
    }

    #[test]
    fn initialize_database_continues_and_logs_when_attachment_sweep_cannot_list() {
        let app_data_dir = unique_path();
        fs::create_dir_all(&app_data_dir).unwrap();
        fs::write(app_data_dir.join("attachments"), b"not a directory").unwrap();

        let connection = initialize_database(&app_data_dir, Utc::now()).unwrap();

        assert!(connection.is_autocommit());
        let log = fs::read_to_string(app_data_dir.join("errors.log")).unwrap();
        assert!(log.contains("attachment sweep listing failed"));
        drop(connection);
        fs::remove_dir_all(app_data_dir).unwrap();
    }

    #[test]
    fn open_database_enables_foreign_key_enforcement() {
        let app_data_dir = unique_path();
        let connection = open_database(&app_data_dir).unwrap();
        let foreign_keys_enabled: i64 = connection
            .pragma_query_value(None, "foreign_keys", |row| row.get(0))
            .unwrap();

        assert_eq!(foreign_keys_enabled, 1);

        drop(connection);
        fs::remove_dir_all(app_data_dir).unwrap();
    }
}
