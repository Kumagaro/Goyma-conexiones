use std::error::Error;
use std::fs::create_dir_all;
use std::path::{Path, PathBuf};
use std::sync::{Mutex, MutexGuard, OnceLock};

use chrono::{DateTime, Utc};
use rusqlite::Connection;
use tauri::{Manager, State};

use super::schema::{clear_non_persistent_sessions, init_schema, purge_error_logs};
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
    let initialization = (|| -> rusqlite::Result<Connection> {
        let conn = open_database(app_data_dir)?;
        init_schema(&conn, now)?;
        clear_non_persistent_sessions(&conn)?;
        purge_error_logs(&conn, now)?;
        Ok(conn)
    })();

    initialization.map_err(|error| {
        append_error_log(app_data_dir, &error.to_string());
        "No se pudo abrir la base de datos".to_owned()
    })
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
