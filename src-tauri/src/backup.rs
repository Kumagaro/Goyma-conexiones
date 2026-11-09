//! Copias de seguridad y restauración (RP-01 a RP-03). `VACUUM INTO` da una
//! copia consistente sin dependencias. La copia no incluye los adjuntos ni el
//! logo, que viven en la carpeta de datos (§11, S-11).

use std::fs;
use std::path::{Path, PathBuf};

use chrono::{DateTime, Utc};
use rusqlite::{Connection, OpenFlags};

use crate::activity::{record_activity, record_as, Entry};
use crate::commands::auth::{require_session, Author};
use crate::commands::settings::require_app_data_dir;
use crate::db::connection::{current_app_data_dir, lock_conn, open_database};
use crate::db::migrations::{
    latest_version, run_migrations, schema_version, APPLICATION_ID, BASE_VERSION,
};
use crate::errors::{log_error, map_db_error, AppError};
use crate::models::backup::{BackupItem, RestoreResult};
use crate::timestamps::{format_utc_timestamp, today_local};

const BACKUP_DIR: &str = "backups";
const DATABASE_FILE: &str = "goyma.sqlite3";
const AUTOMATIC_PREFIX: &str = "goyma-auto-";
const EXTENSION: &str = "sqlite3";
const KEPT_AUTOMATIC: usize = 7;

fn backup_failed(cause: &str) -> AppError {
    AppError::new(
        "backupFailed",
        "No se pudo crear la copia de seguridad",
        &format!("backup failed: {cause}"),
    )
}

fn invalid_backup(cause: &str) -> AppError {
    AppError::new(
        "invalidBackup",
        "El archivo elegido no es una copia de seguridad válida",
        &format!("invalid backup: {cause}"),
    )
}

fn restore_failed(cause: &str) -> AppError {
    AppError::new(
        "restoreFailed",
        "No se pudo restaurar la copia. Tus datos actuales no han cambiado",
        &format!("restore failed: {cause}"),
    )
}

fn file_name_of(path: &Path) -> String {
    path.file_name()
        .map(|name| name.to_string_lossy().into_owned())
        .unwrap_or_default()
}

fn vacuum_into(connection: &Connection, destination: &Path) -> rusqlite::Result<()> {
    connection.execute(
        "VACUUM INTO ?1",
        [destination.to_string_lossy().into_owned()],
    )?;
    Ok(())
}

/// Copia de la base antes de una migración que reconstruye tablas (`plan-06`
/// §2.1, D-04). Su prefijo no se confunde con las automáticas ni con la previa
/// a restaurar.
pub fn copy_before_migration(
    connection: &Connection,
    app_data_dir: &Path,
    version: i64,
) -> std::io::Result<PathBuf> {
    let directory = app_data_dir.join(BACKUP_DIR);
    fs::create_dir_all(&directory)?;
    let destination = directory.join(format!("goyma-antes-de-migrar-{version}.{EXTENSION}"));
    // VACUUM INTO no sobrescribe: una copia anterior de la misma versión se
    // reemplaza (la migración volvió a intentarse).
    let _ = fs::remove_file(&destination);
    vacuum_into(connection, &destination).map_err(std::io::Error::other)?;
    Ok(destination)
}

/// Anota la copia en la misma transacción que su entrada del registro.
fn note_backup(
    connection: &mut Connection,
    author: &Author,
    now: DateTime<Utc>,
    file_name: &str,
) -> rusqlite::Result<()> {
    let transaction = connection.transaction()?;
    transaction.execute(
        "INSERT INTO settings (key, value, updatedAt) VALUES ('lastBackupAt', ?1, ?1)
         ON CONFLICT (key) DO UPDATE SET value = excluded.value, updatedAt = excluded.updatedAt",
        [format_utc_timestamp(now)],
    )?;
    record_activity(
        &transaction,
        author,
        now,
        Entry {
            action: "backup",
            entity_type: "backup",
            label: file_name,
            details: None,
        },
    )?;
    crate::automation::evaluate_backup_watch_best_effort(&transaction, now);
    transaction.commit()
}

pub fn create_backup_inner(
    connection: &mut Connection,
    now: DateTime<Utc>,
    destination: &Path,
) -> Result<(), AppError> {
    let actor = require_session(connection, now)?;
    // VACUUM INTO no sobrescribe: se escribe aparte y se renombra, y un fallo
    // no destruye el archivo que el usuario eligió reemplazar.
    let temporary = destination.with_extension("goyma-tmp");
    let _ = fs::remove_file(&temporary);
    vacuum_into(connection, &temporary).map_err(|error| backup_failed(&error.to_string()))?;
    fs::rename(&temporary, destination).map_err(|error| {
        let _ = fs::remove_file(&temporary);
        backup_failed(&error.kind().to_string())
    })?;
    note_backup(
        connection,
        &Author::User(&actor),
        now,
        &file_name_of(destination),
    )
    .map_err(|error| map_db_error(error, "createBackup", &[]))
}

fn automatic_files(directory: &Path) -> Vec<PathBuf> {
    let mut files: Vec<PathBuf> = fs::read_dir(directory)
        .map(|entries| {
            entries
                .flatten()
                .map(|entry| entry.path())
                .filter(|path| file_name_of(path).starts_with(AUTOMATIC_PREFIX))
                .collect()
        })
        .unwrap_or_default();
    // La fecha va en el nombre: el orden alfabético es el cronológico.
    files.sort();
    files.reverse();
    files
}

fn daily_backup(
    connection: &mut Connection,
    now: DateTime<Utc>,
    app_data_dir: &Path,
) -> Result<(), AppError> {
    let directory = app_data_dir.join(BACKUP_DIR);
    fs::create_dir_all(&directory).map_err(|error| backup_failed(&error.kind().to_string()))?;
    let name = format!(
        "{AUTOMATIC_PREFIX}{}.{EXTENSION}",
        today_local(now).format("%Y-%m-%d")
    );
    let path = directory.join(&name);
    // RP-02: solo el primer arranque del día.
    if path.exists() {
        return Ok(());
    }
    vacuum_into(connection, &path).map_err(|error| backup_failed(&error.to_string()))?;
    for old in automatic_files(&directory).iter().skip(KEPT_AUTOMATIC) {
        let _ = fs::remove_file(old);
    }
    note_backup(connection, &Author::System, now, &name)
        .map_err(|error| map_db_error(error, "dailyBackup", &[]))
}

/// RP-02: un fallo se registra y la aplicación sigue.
pub fn run_daily_backup(connection: &mut Connection, now: DateTime<Utc>, app_data_dir: &Path) {
    if let Err(error) = daily_backup(connection, now, app_data_dir) {
        log_error(
            connection,
            "backup",
            "dailyBackup",
            &error,
            now,
            app_data_dir,
        );
    }
}

fn read_backups(app_data_dir: &Path) -> Vec<BackupItem> {
    let mut items: Vec<BackupItem> = fs::read_dir(app_data_dir.join(BACKUP_DIR))
        .map(|entries| {
            entries
                .flatten()
                .filter(|entry| {
                    entry.path().extension().and_then(|value| value.to_str()) == Some(EXTENSION)
                })
                .filter_map(|entry| {
                    let metadata = entry.metadata().ok()?;
                    Some(BackupItem {
                        file_name: entry.file_name().to_string_lossy().into_owned(),
                        created_at: format_utc_timestamp(metadata.modified().ok()?.into()),
                        size_bytes: metadata.len(),
                    })
                })
                .collect()
        })
        .unwrap_or_default();
    items.sort_by(|left, right| {
        (&right.created_at, &right.file_name).cmp(&(&left.created_at, &left.file_name))
    });
    items
}

pub fn list_automatic_backups_inner(
    connection: &mut Connection,
    now: DateTime<Utc>,
    app_data_dir: &Path,
) -> Result<Vec<BackupItem>, AppError> {
    require_session(connection, now)?;
    Ok(read_backups(app_data_dir))
}

/// RP-03: debe ser una base de Goyma Conexiones sana y de una versión igual
/// o menor que la de la aplicación.
fn validate_backup(path: &Path) -> Result<(), AppError> {
    let invalid = |error: rusqlite::Error| invalid_backup(&error.to_string());
    let copy =
        Connection::open_with_flags(path, OpenFlags::SQLITE_OPEN_READ_ONLY).map_err(invalid)?;
    let application_id: i64 = copy
        .query_row("PRAGMA application_id", [], |row| row.get(0))
        .map_err(invalid)?;
    if application_id != APPLICATION_ID {
        return Err(invalid_backup("foreign application id"));
    }
    let version = schema_version(&copy).map_err(invalid)?;
    if !(BASE_VERSION..=latest_version()).contains(&version) {
        return Err(invalid_backup("unsupported schema version"));
    }
    let integrity: String = copy
        .query_row("PRAGMA integrity_check", [], |row| row.get(0))
        .map_err(invalid)?;
    if integrity != "ok" {
        return Err(invalid_backup("integrity check failed"));
    }
    Ok(())
}

fn verify_password(connection: &Connection, user_id: i64, password: &str) -> Result<(), AppError> {
    let hash: String = connection
        .query_row(
            "SELECT passwordHash FROM users WHERE id = ?1",
            [user_id],
            |row| row.get(0),
        )
        .map_err(|error| map_db_error(error, "restoreBackup", &[]))?;
    if password.len() <= 72 && bcrypt::verify(password, &hash).unwrap_or(false) {
        return Ok(());
    }
    Err(AppError::new(
        "invalidCredentials",
        "La contraseña no es correcta",
        "wrong password confirming a restore",
    ))
}

/// Deja la copia lista en un archivo aparte: si algo falla aquí, la base
/// actual ni se tocó.
fn stage_restore(
    source: &Path,
    staging: &Path,
    user_name: &str,
    now: DateTime<Utc>,
) -> Result<(), AppError> {
    fs::copy(source, staging).map_err(|error| restore_failed(&error.kind().to_string()))?;
    let failed = |error: rusqlite::Error| restore_failed(&error.to_string());
    let mut staged = Connection::open(staging).map_err(failed)?;
    staged
        .pragma_update(None, "foreign_keys", "ON")
        .map_err(failed)?;
    run_migrations(&mut staged).map_err(|error| restore_failed(&error.cause))?;
    // Las sesiones guardadas en la copia se invalidan.
    staged.execute("DELETE FROM sessions", []).map_err(failed)?;
    // El usuario puede no existir en la copia: queda la copia de su nombre.
    record_as(
        &staged,
        None,
        user_name,
        now,
        Entry {
            action: "restore",
            entity_type: "backup",
            label: &file_name_of(source),
            details: None,
        },
    )
    .map_err(failed)?;
    staged.close().map_err(|(_, error)| failed(error))
}

/// Cierra la base, pone la copia en su lugar y vuelve a abrir.
fn swap_database(
    connection: &mut Connection,
    staging: &Path,
    app_data_dir: &Path,
) -> Result<(), AppError> {
    let failed = |error: rusqlite::Error| restore_failed(&error.to_string());
    let placeholder = Connection::open_in_memory().map_err(failed)?;
    std::mem::replace(connection, placeholder)
        .close()
        .map_err(|(_, error)| failed(error))?;
    let moved = fs::rename(staging, app_data_dir.join(DATABASE_FILE));
    *connection = open_database(app_data_dir).map_err(failed)?;
    moved.map_err(|error| restore_failed(&error.kind().to_string()))
}

pub fn restore_backup_inner(
    connection: &mut Connection,
    now: DateTime<Utc>,
    app_data_dir: &Path,
    source_path: Option<&Path>,
    automatic_file_name: Option<&str>,
    password: &str,
) -> Result<RestoreResult, AppError> {
    let actor = require_session(connection, now)?;
    verify_password(connection, actor.id(), password)?;

    let directory = app_data_dir.join(BACKUP_DIR);
    let source = match (source_path, automatic_file_name) {
        (Some(path), None) => path.to_path_buf(),
        // Solo un nombre de la carpeta de copias, nunca una ruta.
        (None, Some(name)) if file_name_of(Path::new(name)) == name => directory.join(name),
        _ => return Err(invalid_backup("restore source is not valid")),
    };
    validate_backup(&source)?;

    fs::create_dir_all(&directory).map_err(|error| restore_failed(&error.kind().to_string()))?;
    let previous = directory.join(format!(
        "goyma-previa-{}.{EXTENSION}",
        now.format("%Y-%m-%d-%H%M%S")
    ));
    vacuum_into(connection, &previous).map_err(|error| restore_failed(&error.to_string()))?;

    let staging = app_data_dir.join("goyma.restore.tmp");
    let _ = fs::remove_file(&staging);
    let result = stage_restore(&source, &staging, actor.username(), now)
        .and_then(|()| swap_database(connection, &staging, app_data_dir));
    let _ = fs::remove_file(&staging);
    result.map(|()| RestoreResult {
        session_ended: true,
    })
}

fn log_failure<T>(
    connection: &Connection,
    operation: &str,
    result: &Result<T, AppError>,
    now: DateTime<Utc>,
) {
    if let Err(error) = result {
        if error.should_log() {
            if let Some(app_data_dir) = current_app_data_dir() {
                log_error(connection, "backup", operation, error, now, app_data_dir);
            }
        }
    }
}

#[tauri::command]
pub fn create_backup(
    state: tauri::State<'_, std::sync::Mutex<Connection>>,
    destination_path: String,
) -> Result<(), AppError> {
    let mut connection = lock_conn(&state);
    let now = Utc::now();
    let result = create_backup_inner(&mut connection, now, Path::new(&destination_path));
    log_failure(&connection, "createBackup", &result, now);
    result
}

#[tauri::command]
pub fn list_automatic_backups(
    state: tauri::State<'_, std::sync::Mutex<Connection>>,
) -> Result<Vec<BackupItem>, AppError> {
    let mut connection = lock_conn(&state);
    let now = Utc::now();
    let result = require_app_data_dir()
        .and_then(|dir| list_automatic_backups_inner(&mut connection, now, dir));
    log_failure(&connection, "listAutomaticBackups", &result, now);
    result
}

#[tauri::command]
pub fn restore_backup(
    state: tauri::State<'_, std::sync::Mutex<Connection>>,
    source_path: Option<String>,
    automatic_file_name: Option<String>,
    password: String,
) -> Result<RestoreResult, AppError> {
    let mut connection = lock_conn(&state);
    let now = Utc::now();
    let result = require_app_data_dir().and_then(|dir| {
        restore_backup_inner(
            &mut connection,
            now,
            dir,
            source_path.as_deref().map(Path::new),
            automatic_file_name.as_deref(),
            &password,
        )
    });
    log_failure(&connection, "restoreBackup", &result, now);
    result
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::db::connection::initialize_database;
    use crate::test_utils::{
        assert_allows_both_roles, assert_requires_session, create_test_session, create_test_user,
        insert_category_fixture,
    };
    use chrono::Duration;

    const PASSWORD: &str = "ValidPass1!";

    fn temp_dir() -> PathBuf {
        let dir = std::env::temp_dir().join(format!("goyma-backup-{}", uuid::Uuid::new_v4()));
        fs::create_dir_all(&dir).unwrap();
        dir
    }

    /// Base real en disco con una sesión abierta.
    fn signed_in() -> (Connection, PathBuf, DateTime<Utc>) {
        let dir = temp_dir();
        let now = Utc::now();
        let connection = initialize_database(&dir, now).unwrap();
        let user_id = create_test_user(
            &connection,
            "backupuser",
            "contador",
            "+5350000060",
            "12345678960",
            PASSWORD,
        );
        create_test_session(&connection, user_id, false, now, now);
        (connection, dir, now)
    }

    fn count(connection: &Connection, table: &str) -> i64 {
        connection
            .query_row(&format!("SELECT COUNT(*) FROM {table}"), [], |row| {
                row.get(0)
            })
            .unwrap()
    }

    fn activity(connection: &Connection, action: &str) -> Vec<(String, String)> {
        // La copia diaria de `initialize_database` también deja su entrada.
        connection
            .prepare(
                "SELECT userName, entityLabel FROM activityLog
                 WHERE action = ?1 AND entityLabel NOT LIKE 'goyma-auto-%'",
            )
            .unwrap()
            .query_map([action], |row| Ok((row.get(0)?, row.get(1)?)))
            .unwrap()
            .collect::<rusqlite::Result<_>>()
            .unwrap()
    }

    fn make_copy(connection: &mut Connection, now: DateTime<Utc>) -> PathBuf {
        let path = temp_dir().join("copia.sqlite3");
        create_backup_inner(connection, now, &path).unwrap();
        path
    }

    fn restore(
        connection: &mut Connection,
        dir: &Path,
        now: DateTime<Utc>,
        source: &Path,
        password: &str,
    ) -> Result<RestoreResult, AppError> {
        restore_backup_inner(connection, now, dir, Some(source), None, password)
    }

    #[test]
    fn create_backup_writes_a_valid_copy_and_records_it() {
        let (mut connection, _, now) = signed_in();
        insert_category_fixture(&connection, "Cables", None);

        let path = make_copy(&mut connection, now);

        validate_backup(&path).unwrap();
        let copy = Connection::open(&path).unwrap();
        assert_eq!(count(&copy, "categories"), 1);
        let last: String = connection
            .query_row(
                "SELECT value FROM settings WHERE key = 'lastBackupAt'",
                [],
                |row| row.get(0),
            )
            .unwrap();
        assert_eq!(last, format_utc_timestamp(now));
        assert_eq!(
            activity(&connection, "backup"),
            vec![("backupuser".to_owned(), "copia.sqlite3".to_owned())]
        );
    }

    #[test]
    fn create_backup_replaces_the_chosen_file() {
        let (mut connection, _, now) = signed_in();
        let path = temp_dir().join("copia.sqlite3");
        fs::write(&path, b"antiguo").unwrap();

        create_backup_inner(&mut connection, now, &path).unwrap();

        validate_backup(&path).unwrap();
    }

    #[test]
    fn a_failed_backup_is_reported_and_keeps_last_backup_unset() {
        let (mut connection, _, now) = signed_in();
        let missing = temp_dir().join("no-existe").join("copia.sqlite3");

        let error = create_backup_inner(&mut connection, now, &missing).unwrap_err();

        assert_eq!(error.code, "backupFailed");
        assert!(activity(&connection, "backup").is_empty());
    }

    #[test]
    fn the_daily_copy_runs_once_a_day_and_keeps_the_last_seven() {
        let (mut connection, dir, now) = signed_in();
        let directory = dir.join(BACKUP_DIR);
        // La de hoy ya la hizo `initialize_database`.
        assert_eq!(automatic_files(&directory).len(), 1);
        run_daily_backup(&mut connection, now, &dir);
        assert_eq!(automatic_files(&directory).len(), 1);

        for day in 1..=8 {
            run_daily_backup(&mut connection, now + Duration::days(day), &dir);
        }

        let files = automatic_files(&directory);
        assert_eq!(files.len(), KEPT_AUTOMATIC);
        let newest = today_local(now + Duration::days(8)).format("%Y-%m-%d");
        assert!(file_name_of(&files[0]).contains(&newest.to_string()));
        let automatic: i64 = connection
            .query_row(
                "SELECT COUNT(*) FROM activityLog WHERE userName = 'Sistema' AND action = 'backup'",
                [],
                |row| row.get(0),
            )
            .unwrap();
        assert_eq!(automatic, 9);
    }

    #[test]
    fn the_backup_reminder_follows_its_threshold_and_a_new_copy() {
        let (mut connection, _, now) = signed_in();
        let reminders = |connection: &Connection| -> i64 {
            connection
                .query_row(
                    "SELECT COUNT(*) FROM notifications WHERE type = 'N-11'",
                    [],
                    |row| row.get(0),
                )
                .unwrap()
        };
        let last_copy = format_utc_timestamp(now - Duration::days(10));
        connection
            .execute(
                "UPDATE settings SET value = ?1 WHERE key = 'lastBackupAt'",
                [&last_copy],
            )
            .unwrap();

        // CF-04: cambiar el umbral reevalúa la situación con el valor nuevo.
        crate::commands::settings::update_settings_inner(
            &mut connection,
            now,
            crate::models::settings::UpdateSettingsInput {
                stale_invoice_days: 30,
                backup_reminder_days: 3,
            },
        )
        .unwrap();
        assert_eq!(reminders(&connection), 1);

        // Sale de la situación con una copia y vuelve a avisar al repetirse.
        make_copy(&mut connection, now);
        connection
            .execute(
                "UPDATE settings SET value = ?1 WHERE key = 'lastBackupAt'",
                [&last_copy],
            )
            .unwrap();
        crate::automation::evaluate_backup_watch_best_effort(&connection, now);
        assert_eq!(reminders(&connection), 2);
    }

    #[test]
    fn a_failed_daily_copy_does_not_stop_the_application() {
        let (mut connection, dir, now) = signed_in();
        fs::remove_dir_all(dir.join(BACKUP_DIR)).unwrap();
        fs::write(dir.join(BACKUP_DIR), b"no es una carpeta").unwrap();

        run_daily_backup(&mut connection, now + Duration::days(1), &dir);

        let logged: i64 = connection
            .query_row(
                "SELECT COUNT(*) FROM errorLogs WHERE operation = 'dailyBackup'",
                [],
                |row| row.get(0),
            )
            .unwrap();
        assert_eq!(logged, 1);
    }

    #[test]
    fn list_automatic_backups_gives_name_date_and_size() {
        let (mut connection, dir, now) = signed_in();
        fs::write(dir.join(BACKUP_DIR).join("otro.txt"), b"x").unwrap();

        let items = list_automatic_backups_inner(&mut connection, now, &dir).unwrap();

        assert_eq!(items.len(), 1);
        assert!(items[0].file_name.starts_with(AUTOMATIC_PREFIX));
        assert!(items[0].size_bytes > 0);
    }

    #[test]
    fn restoring_with_a_wrong_password_touches_nothing() {
        let (mut connection, dir, now) = signed_in();
        let copy = make_copy(&mut connection, now);
        insert_category_fixture(&connection, "Después", None);

        let error = restore(&mut connection, &dir, now, &copy, "Incorrecta1!").unwrap_err();

        assert_eq!(error.code, "invalidCredentials");
        assert_eq!(error.message, "La contraseña no es correcta");
        assert_eq!(count(&connection, "categories"), 1);
        assert_eq!(count(&connection, "sessions"), 1);
    }

    #[test]
    fn invalid_copies_are_rejected_without_touching_anything() {
        let (mut connection, dir, now) = signed_in();
        insert_category_fixture(&connection, "Actual", None);
        let folder = temp_dir();

        let foreign = folder.join("ajena.sqlite3");
        Connection::open(&foreign)
            .unwrap()
            .execute_batch("CREATE TABLE cosas (id INTEGER);")
            .unwrap();
        let corrupt = folder.join("corrupta.sqlite3");
        fs::write(&corrupt, b"esto no es una base de datos, solo texto").unwrap();
        let newer = make_copy(&mut connection, now);
        Connection::open(&newer)
            .unwrap()
            .pragma_update(None, "user_version", latest_version() + 1)
            .unwrap();

        for source in [foreign, corrupt, newer, folder.join("no-existe.sqlite3")] {
            let error = restore(&mut connection, &dir, now, &source, PASSWORD).unwrap_err();
            assert_eq!(error.code, "invalidBackup", "{source:?}");
        }
        assert_eq!(count(&connection, "categories"), 1);
        assert_eq!(count(&connection, "sessions"), 1);
        assert!(activity(&connection, "restore").is_empty());
    }

    #[test]
    fn restoring_replaces_the_database_and_ends_the_session() {
        let (mut connection, dir, now) = signed_in();
        insert_category_fixture(&connection, "De la copia", None);
        let copy = make_copy(&mut connection, now);
        connection.execute("DELETE FROM categories", []).unwrap();
        insert_category_fixture(&connection, "Actual", None);

        let result = restore(&mut connection, &dir, now, &copy, PASSWORD).unwrap();

        assert!(result.session_ended);
        let name: String = connection
            .query_row("SELECT name FROM categories", [], |row| row.get(0))
            .unwrap();
        assert_eq!(name, "De la copia");
        assert_eq!(count(&connection, "sessions"), 0);
        assert_eq!(
            activity(&connection, "restore"),
            vec![("backupuser".to_owned(), "copia.sqlite3".to_owned())]
        );
        let foreign_keys: i64 = connection
            .query_row("PRAGMA foreign_keys", [], |row| row.get(0))
            .unwrap();
        assert_eq!(foreign_keys, 1);
        assert!(!dir.join("goyma.restore.tmp").exists());

        // Lo anterior queda guardado en una copia previa.
        let previous = read_backups(&dir)
            .into_iter()
            .find(|item| item.file_name.starts_with("goyma-previa-"))
            .unwrap();
        let saved = Connection::open(dir.join(BACKUP_DIR).join(previous.file_name)).unwrap();
        let name: String = saved
            .query_row("SELECT name FROM categories", [], |row| row.get(0))
            .unwrap();
        assert_eq!(name, "Actual");
    }

    #[test]
    fn a_copy_of_an_older_version_is_migrated() {
        let (mut connection, dir, now) = signed_in();
        let current = make_copy(&mut connection, now);
        // Una copia real de la versión 5, con los usuarios de la base actual.
        let copy = temp_dir().join("copia-v5.sqlite3");
        let _ = fs::remove_file(&copy);
        {
            let mut old = Connection::open(&copy).unwrap();
            crate::db::schema::create_schema(&old).unwrap();
            crate::db::migrations::migrate_up_to(&mut old, 5).unwrap();
            old.pragma_update(None, "application_id", APPLICATION_ID)
                .unwrap();
            old.execute(
                "ATTACH DATABASE ?1 AS actual",
                [current.to_string_lossy().into_owned()],
            )
            .unwrap();
            old.execute_batch(
                "INSERT INTO users SELECT * FROM actual.users; DETACH DATABASE actual;",
            )
            .unwrap();
        }

        restore(&mut connection, &dir, now, &copy, PASSWORD).unwrap();

        assert_eq!(schema_version(&connection).unwrap(), latest_version());
        assert_eq!(count(&connection, "drafts"), 0);
        assert_eq!(activity(&connection, "restore").len(), 1);
    }

    #[test]
    fn an_automatic_copy_is_restored_by_name_and_never_by_path() {
        let (mut connection, dir, now) = signed_in();
        let name = read_backups(&dir)[0].file_name.clone();

        let error = restore_backup_inner(
            &mut connection,
            now,
            &dir,
            None,
            Some("../goyma.sqlite3"),
            PASSWORD,
        )
        .unwrap_err();
        assert_eq!(error.code, "invalidBackup");
        let error =
            restore_backup_inner(&mut connection, now, &dir, None, None, PASSWORD).unwrap_err();
        assert_eq!(error.code, "invalidBackup");

        restore_backup_inner(&mut connection, now, &dir, None, Some(&name), PASSWORD).unwrap();
        let label: String = connection
            .query_row(
                "SELECT entityLabel FROM activityLog WHERE action = 'restore'",
                [],
                |row| row.get(0),
            )
            .unwrap();
        assert_eq!(label, name);
    }

    #[test]
    fn backup_commands_require_session_and_allow_both_roles() {
        let dir = temp_dir();
        let destination = dir.join("copia.sqlite3");
        let missing = dir.join("no-existe.sqlite3");
        assert_requires_session(|connection, now| {
            create_backup_inner(connection, now, &destination)
        });
        assert_requires_session(|connection, now| {
            list_automatic_backups_inner(connection, now, &dir)
        });
        assert_requires_session(|connection, now| {
            restore_backup_inner(connection, now, &dir, Some(&missing), None, PASSWORD)
        });
        assert_allows_both_roles(|connection, now| {
            list_automatic_backups_inner(connection, now, &dir)
        });
        assert_allows_both_roles(|connection, now| {
            restore_backup_inner(connection, now, &dir, Some(&missing), None, PASSWORD)
        });
    }

    #[test]
    fn create_backup_allows_both_roles() {
        let (mut connection, _, now) = signed_in();
        connection
            .execute("UPDATE users SET role = 'administrador'", [])
            .unwrap();
        make_copy(&mut connection, now);
    }
}
