use std::collections::HashMap;
use std::fs::{create_dir_all, OpenOptions};
use std::io::{self, Write};
use std::path::Path;
use std::sync::{Mutex, OnceLock};

use chrono::{DateTime, Duration, SecondsFormat, Utc};
use rusqlite::{Connection, ErrorCode, OptionalExtension};
use serde::Serialize;

const INTERNAL_ERROR_MESSAGE: &str = "Ocurrió un error inesperado";
const INVALID_STATE_MESSAGE: &str = "La operación no es válida en el estado actual";
const THROTTLE_WINDOW: Duration = Duration::seconds(60);
const MAX_THROTTLE_ENTRIES: usize = 1000;

type ThrottledErrorKey = (String, i64);
type ThrottledErrorMap = HashMap<ThrottledErrorKey, DateTime<Utc>>;

static THROTTLED_ERRORS: OnceLock<Mutex<ThrottledErrorMap>> = OnceLock::new();

#[derive(Debug, Serialize)]
pub struct AppError {
    pub code: String,
    pub message: String,
    #[serde(skip)]
    pub cause: String,
}

impl AppError {
    pub fn new(code: &str, message: &str, cause: &str) -> Self {
        Self {
            code: code.to_owned(),
            message: message.to_owned(),
            cause: cause.to_owned(),
        }
    }

    pub fn should_log(&self) -> bool {
        matches!(
            self.code.as_str(),
            "duplicateField"
                | "backupFailed"
                | "invalidBackup"
                | "restoreFailed"
                | "notFound"
                | "invalidState"
                | "stockInsuficiente"
                | "loteEnUso"
                | "attachmentInvalid"
                | "stockLimitExceeded"
                | "exportFailed"
                | "internal"
        )
    }
}

pub fn invalid_state(message: &str, cause: &str) -> AppError {
    let message = if message.is_empty() {
        INVALID_STATE_MESSAGE
    } else {
        message
    };
    AppError::new("invalidState", message, cause)
}

pub fn map_db_error(
    error: rusqlite::Error,
    operation: &str,
    duplicates: &[(&str, &str)],
) -> AppError {
    match &error {
        rusqlite::Error::SqliteFailure(sqlite_error, Some(message)) => {
            if sqlite_error.code == ErrorCode::ConstraintViolation {
                for (column, user_message) in duplicates {
                    if message.contains(column) {
                        return AppError::new(
                            "duplicateField",
                            user_message,
                            &format!("{operation} unique constraint violated on {column}"),
                        );
                    }
                }

                return AppError::new(
                    "internal",
                    INTERNAL_ERROR_MESSAGE,
                    &format!("{operation} sqlite constraint violation"),
                );
            }

            AppError::new(
                "internal",
                INTERNAL_ERROR_MESSAGE,
                &format!("{operation} sqlite failure: {:?}", sqlite_error.code),
            )
        }
        rusqlite::Error::SqliteFailure(sqlite_error, None) => AppError::new(
            "internal",
            INTERNAL_ERROR_MESSAGE,
            &format!("{operation} sqlite failure: {:?}", sqlite_error.code),
        ),
        _ => AppError::new(
            "internal",
            INTERNAL_ERROR_MESSAGE,
            &format!("{operation} database operation failed"),
        ),
    }
}

pub fn log_error(
    connection: &Connection,
    module: &str,
    operation: &str,
    error: &AppError,
    now: DateTime<Utc>,
    app_data_dir: &Path,
) {
    if !error.should_log() {
        return;
    }

    if is_throttled_error_code(&error.code) {
        let user_id = connection
            .query_row("SELECT userId FROM sessions WHERE id = 1", [], |row| {
                row.get::<_, i64>(0)
            })
            .optional()
            .ok()
            .flatten();
        let Some(user_id) = user_id else {
            return;
        };
        if !should_log_throttled(&error.code, user_id, now) {
            return;
        }
    }

    let created_at = now.to_rfc3339_opts(SecondsFormat::Secs, true);
    if connection
        .execute(
            "INSERT INTO errorLogs (module, operation, cause, createdAt) VALUES (?1, ?2, ?3, ?4)",
            (module, operation, &error.cause, created_at),
        )
        .is_err()
    {
        append_error_log(
            app_data_dir,
            &format!(
                "module={module} operation={operation} cause={}",
                error.cause
            ),
        );
    }
}

fn is_throttled_error_code(code: &str) -> bool {
    matches!(
        code,
        "stockInsuficiente" | "loteEnUso" | "attachmentInvalid" | "stockLimitExceeded"
    )
}

fn should_log_throttled(code: &str, user_id: i64, now: DateTime<Utc>) -> bool {
    let throttle = THROTTLED_ERRORS.get_or_init(|| Mutex::new(HashMap::new()));
    let mut entries = throttle
        .lock()
        .unwrap_or_else(std::sync::PoisonError::into_inner);
    let cutoff = now - THROTTLE_WINDOW;
    entries.retain(|_, last_logged_at| *last_logged_at > cutoff);

    let key = (code.to_owned(), user_id);
    if entries.contains_key(&key) {
        return false;
    }

    if entries.len() >= MAX_THROTTLE_ENTRIES {
        if let Some(oldest_key) = entries
            .iter()
            .min_by_key(|(_, last_logged_at)| *last_logged_at)
            .map(|(key, _)| key.clone())
        {
            entries.remove(&oldest_key);
        }
    }

    entries.insert(key, now);
    true
}

pub fn append_error_log(app_data_dir: &Path, message: &str) {
    let result = (|| -> io::Result<()> {
        create_dir_all(app_data_dir)?;
        let mut file = OpenOptions::new()
            .create(true)
            .append(true)
            .open(app_data_dir.join("errors.log"))?;
        let single_line_message = message.replace(['\r', '\n'], " ");
        writeln!(file, "{} {single_line_message}", Utc::now().to_rfc3339())
    })();

    let _ = result;
}

#[cfg(test)]
mod tests {
    use std::fs;
    use std::path::PathBuf;
    use std::sync::atomic::{AtomicU64, Ordering};

    use chrono::{TimeZone, Utc};
    use serde_json::json;

    use crate::test_utils::{create_test_session, create_test_user, setup_test_db};

    use super::{
        append_error_log, log_error, map_db_error, should_log_throttled, AppError, THROTTLED_ERRORS,
    };

    static NEXT_TEST_PATH: AtomicU64 = AtomicU64::new(0);

    fn unique_path(label: &str) -> PathBuf {
        let sequence = NEXT_TEST_PATH.fetch_add(1, Ordering::Relaxed);
        std::env::temp_dir().join(format!("goyma-{label}-{}-{sequence}", std::process::id()))
    }

    #[test]
    fn append_error_log_creates_directory_and_appends_lines() {
        let app_data_dir = unique_path("append-log");
        assert!(!app_data_dir.exists());

        append_error_log(&app_data_dir, "primera causa");
        append_error_log(&app_data_dir, "segunda causa");

        let contents = fs::read_to_string(app_data_dir.join("errors.log")).unwrap();
        let lines: Vec<&str> = contents.lines().collect();
        assert_eq!(lines.len(), 2);
        assert!(lines[0].contains("primera causa"));
        assert!(lines[1].contains("segunda causa"));
        if app_data_dir.exists() {
            fs::remove_dir_all(app_data_dir).unwrap();
        }
    }

    #[test]
    fn append_error_log_ignores_write_failures() {
        let parent_file = unique_path("not-a-directory");
        fs::write(&parent_file, "file").unwrap();

        append_error_log(&parent_file, "causa no escribible");

        fs::remove_file(parent_file).unwrap();
    }

    #[test]
    fn app_error_serializes_public_fields_without_cause() {
        let error = AppError::new("internal", "Ocurrió un error inesperado", "SQLite error");

        assert_eq!(
            serde_json::to_value(error).unwrap(),
            json!({ "code": "internal", "message": "Ocurrió un error inesperado" })
        );
    }

    #[test]
    fn log_error_writes_operation_and_cause_to_error_logs() {
        let connection = setup_test_db();
        let error = AppError::new(
            "internal",
            "Ocurrió un error inesperado",
            "database unavailable",
        );
        let app_data_dir = unique_path("error-row");

        log_error(
            &connection,
            "auth",
            "login",
            &error,
            Utc::now(),
            &app_data_dir,
        );

        let row: (String, String, String) = connection
            .query_row(
                "SELECT module, operation, cause FROM errorLogs",
                [],
                |row| Ok((row.get(0)?, row.get(1)?, row.get(2)?)),
            )
            .unwrap();
        assert_eq!(
            row,
            ("auth".into(), "login".into(), "database unavailable".into())
        );
        assert!(!row.2.contains("Admin123!"));
        assert!(!row.2.contains("admin"));
        assert!(!row.2.contains("+5300000000"));
        assert!(!row.2.contains("00000000000"));
        assert!(!row.2.contains(&error.message));
    }

    #[test]
    fn log_error_falls_back_to_errors_log_when_table_write_fails() {
        let connection = setup_test_db();
        connection.execute("DROP TABLE errorLogs", []).unwrap();
        let error = AppError::new("internal", "Ocurrió un error inesperado", "write failed");
        let app_data_dir = unique_path("error-fallback");

        log_error(
            &connection,
            "users",
            "createUser",
            &error,
            Utc::now(),
            &app_data_dir,
        );

        let contents = fs::read_to_string(app_data_dir.join("errors.log")).unwrap();
        assert!(contents.contains("users"));
        assert!(contents.contains("createUser"));
        assert!(contents.contains("write failed"));
        fs::remove_dir_all(app_data_dir).unwrap();
    }

    #[test]
    fn log_error_ignores_expected_input_rejections() {
        let connection = setup_test_db();
        let app_data_dir = unique_path("expected-error");

        for code in ["invalidCredentials", "validationFailed"] {
            let error = AppError::new(code, "mensaje", "expected rejection");
            assert!(!error.should_log());
            log_error(
                &connection,
                "auth",
                "login",
                &error,
                Utc::now(),
                &app_data_dir,
            );
        }

        let count: i64 = connection
            .query_row("SELECT COUNT(*) FROM errorLogs", [], |row| row.get(0))
            .unwrap();
        assert_eq!(count, 0);
        assert!(!app_data_dir.exists());
    }

    #[test]
    fn invalid_state_serializes_without_cause_and_is_logged() {
        let error = AppError::new(
            "invalidState",
            "El producto ya no existe",
            "state transition failed",
        );

        assert!(error.should_log());
        assert_eq!(
            serde_json::to_value(&error).unwrap(),
            json!({ "code": "invalidState", "message": "El producto ya no existe" })
        );
    }

    #[test]
    fn should_log_covers_spec03_state_errors_and_preserves_input_rejections() {
        for code in [
            "stockInsuficiente",
            "loteEnUso",
            "attachmentInvalid",
            "stockLimitExceeded",
        ] {
            assert!(AppError::new(code, "mensaje", "cause").should_log());
        }
        for code in ["invalidCredentials", "validationFailed"] {
            assert!(!AppError::new(code, "mensaje", "cause").should_log());
        }
        for code in [
            "duplicateField",
            "notFound",
            "invalidState",
            "exportFailed",
            "internal",
        ] {
            assert!(AppError::new(code, "mensaje", "cause").should_log());
        }
    }

    #[test]
    fn log_error_throttles_state_errors_per_code_and_user_and_cleans_old_entries() {
        let connection = setup_test_db();
        let first_user = create_test_user(
            &connection,
            "firstuser",
            "administrador",
            "+5350000001",
            "12345678901",
            "ValidPass1!",
        );
        let second_user = create_test_user(
            &connection,
            "seconduser",
            "contador",
            "+5350000002",
            "12345678902",
            "ValidPass1!",
        );
        let now = Utc.with_ymd_and_hms(2026, 10, 6, 12, 0, 0).unwrap();
        create_test_session(&connection, first_user, false, now, now);
        let app_data_dir = unique_path("throttled-errors");

        for code in [
            "stockInsuficiente",
            "loteEnUso",
            "attachmentInvalid",
            "stockLimitExceeded",
        ] {
            let error = AppError::new(code, "rechazo", "state error");
            log_error(&connection, "invoices", "save", &error, now, &app_data_dir);
            log_error(
                &connection,
                "invoices",
                "save",
                &error,
                now + chrono::Duration::seconds(30),
                &app_data_dir,
            );
        }
        let first_user_count: i64 = connection
            .query_row("SELECT COUNT(*) FROM errorLogs", [], |row| row.get(0))
            .unwrap();
        assert_eq!(first_user_count, 4);

        connection
            .execute(
                "UPDATE sessions SET userId = ?1 WHERE id = 1",
                [second_user],
            )
            .unwrap();
        let stock_error = AppError::new("stockInsuficiente", "rechazo", "state error");
        log_error(
            &connection,
            "invoices",
            "save",
            &stock_error,
            now + chrono::Duration::seconds(30),
            &app_data_dir,
        );
        connection
            .execute("UPDATE sessions SET userId = ?1 WHERE id = 1", [first_user])
            .unwrap();
        log_error(
            &connection,
            "invoices",
            "save",
            &stock_error,
            now + chrono::Duration::seconds(61),
            &app_data_dir,
        );

        let total: i64 = connection
            .query_row("SELECT COUNT(*) FROM errorLogs", [], |row| row.get(0))
            .unwrap();
        assert_eq!(total, 6);

        let always_logged = AppError::new("duplicateField", "duplicado", "duplicate");
        for _ in 0..2 {
            log_error(
                &connection,
                "clients",
                "create",
                &always_logged,
                now,
                &app_data_dir,
            );
        }
        let total: i64 = connection
            .query_row("SELECT COUNT(*) FROM errorLogs", [], |row| row.get(0))
            .unwrap();
        assert_eq!(total, 8);

        {
            let mut entries = THROTTLED_ERRORS.get().unwrap().lock().unwrap();
            entries.clear();
            for user_id in 0..1000 {
                entries.insert(("stockInsuficiente".to_owned(), user_id), now);
            }
        }
        assert!(should_log_throttled("stockInsuficiente", 1000, now));
        let entries = THROTTLED_ERRORS.get().unwrap().lock().unwrap();
        assert_eq!(entries.len(), 1000);
        assert!(entries.contains_key(&("stockInsuficiente".to_owned(), 1000)));
        let retained_entries = (0..1000)
            .filter(|user_id| entries.contains_key(&("stockInsuficiente".to_owned(), *user_id)))
            .count();
        assert_eq!(retained_entries, 999);
        drop(entries);

        let expired = now + chrono::Duration::seconds(121);
        assert!(should_log_throttled(
            "stockInsuficiente",
            first_user,
            expired
        ));
        if app_data_dir.exists() {
            fs::remove_dir_all(app_data_dir).unwrap();
        }
    }

    #[test]
    fn map_db_error_translates_unique_constraints_into_duplicate_field() {
        let connection = setup_test_db();
        connection
            .execute(
                "INSERT INTO users (username, passwordHash, role, phone, nationalId, createdAt) VALUES (?1, ?2, ?3, ?4, ?5, ?6)",
                (
                    "alice",
                    "hash",
                    "contador",
                    "+5350000000",
                    "12345678901",
                    Utc::now().to_rfc3339_opts(chrono::SecondsFormat::Secs, true),
                ),
            )
            .unwrap();

        let err = connection
            .execute(
                "INSERT INTO users (username, passwordHash, role, phone, nationalId, createdAt) VALUES (?1, ?2, ?3, ?4, ?5, ?6)",
                (
                    "alice",
                    "hash2",
                    "contador",
                    "+5350000001",
                    "12345678902",
                    Utc::now().to_rfc3339_opts(chrono::SecondsFormat::Secs, true),
                ),
            )
            .expect_err("duplicate username should be rejected");

        let mapped = map_db_error(
            err,
            "createUser",
            &[("users.username", "El nombre de usuario ya está registrado")],
        );

        assert_eq!(mapped.code, "duplicateField");
        assert_eq!(mapped.message, "El nombre de usuario ya está registrado");
        assert!(mapped.cause.contains("createUser"));
        assert!(mapped.cause.contains("users.username"));
        assert!(!mapped.cause.contains("alice"));
    }

    #[test]
    fn log_error_does_not_panic_when_database_and_file_fallback_fail() {
        let connection = setup_test_db();
        connection.execute("DROP TABLE errorLogs", []).unwrap();
        let error = AppError::new("internal", "Ocurrió un error inesperado", "write failed");
        let not_a_directory = unique_path("fallback-parent-file");
        fs::write(&not_a_directory, "file").unwrap();

        log_error(
            &connection,
            "users",
            "createUser",
            &error,
            Utc::now(),
            &not_a_directory,
        );

        fs::remove_file(not_a_directory).unwrap();
    }
}
