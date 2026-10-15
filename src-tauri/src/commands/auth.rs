use chrono::{DateTime, Duration, SecondsFormat, Utc};
use rusqlite::{Connection, OptionalExtension};
use serde::Serialize;

use crate::errors::AppError;
use crate::models::session::Session;
use crate::validators::{
    normalize_username, validate_national_id, validate_not_reserved, validate_password,
    validate_phone,
};

#[cfg(test)]
const BCRYPT_COST: u32 = 4;
#[cfg(not(test))]
const BCRYPT_COST: u32 = 10;

const SESSION_EXPIRED_MESSAGE: &str = "Tu sesión ya no es válida. Inicia sesión de nuevo";
const FORBIDDEN_MESSAGE: &str = "No tienes permiso para realizar esta acción";
const INVALID_CREDENTIALS_MESSAGE: &str = "El nombre de usuario o la contraseña son incorrectos";
const SESSION_ALREADY_ACTIVE_MESSAGE: &str =
    "Ya hay una sesión activa. Cierra la sesión actual antes de iniciar otra";
const DUMMY_PASSWORD_HASH: &str = "$2a$10$N9qo8uLOickgx2ZMRZoMyeIjZAgcfl7p92ldGxad68LJZdL17lhWy";
const RECOVERY_DATA_MISMATCH_MESSAGE: &str = "Los datos no coinciden";

pub fn hash_password(password: &str) -> bcrypt::BcryptResult<String> {
    if password.len() > 72 {
        return Err(bcrypt::BcryptError::Truncation(password.len()));
    }
    bcrypt::hash(password, BCRYPT_COST)
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum SessionValidation {
    NoSession,
    Valid(Session),
    ExpiredByInactivity,
    ExpiredByPersistence,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Actor {
    id: i64,
    role: String,
    username: String,
}

impl Actor {
    pub fn id(&self) -> i64 {
        self.id
    }

    pub fn role(&self) -> &str {
        &self.role
    }

    pub fn username(&self) -> &str {
        &self.username
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct AdminActor {
    id: i64,
}

impl AdminActor {
    pub fn id(&self) -> i64 {
        self.id
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct SessionInfo {
    pub username: String,
    pub role: String,
    pub remember_session: bool,
}

pub fn validate_session(
    connection: &Connection,
    now: DateTime<Utc>,
) -> rusqlite::Result<SessionValidation> {
    let row: Option<(i64, i64, bool, String, String)> = connection
        .query_row(
            "SELECT sessions.id, sessions.userId, sessions.rememberSession,
                    sessions.createdAt, sessions.lastActivityAt
             FROM sessions",
            [],
            |row| {
                Ok((
                    row.get(0)?,
                    row.get(1)?,
                    row.get(2)?,
                    row.get(3)?,
                    row.get(4)?,
                ))
            },
        )
        .optional()?;

    let Some((id, user_id, remember_session, created_at, last_activity_at)) = row else {
        return Ok(SessionValidation::NoSession);
    };

    let created_at = DateTime::parse_from_rfc3339(&created_at)
        .map_err(|error| rusqlite::Error::ToSqlConversionFailure(Box::new(error)))?
        .with_timezone(&Utc);
    let last_activity_at = DateTime::parse_from_rfc3339(&last_activity_at)
        .map_err(|error| rusqlite::Error::ToSqlConversionFailure(Box::new(error)))?
        .with_timezone(&Utc);

    let expired = if !remember_session && now - last_activity_at > Duration::hours(3) {
        Some(SessionValidation::ExpiredByInactivity)
    } else if remember_session && now - created_at > Duration::days(30) {
        Some(SessionValidation::ExpiredByPersistence)
    } else {
        None
    };

    if let Some(expiration) = expired {
        connection.execute("DELETE FROM sessions WHERE id = 1", [])?;
        return Ok(expiration);
    }

    Ok(SessionValidation::Valid(Session {
        id,
        user_id,
        remember_session,
        created_at: created_at.to_rfc3339_opts(SecondsFormat::Secs, true),
        last_activity_at: last_activity_at.to_rfc3339_opts(SecondsFormat::Secs, true),
    }))
}

pub fn require_session(connection: &Connection, now: DateTime<Utc>) -> Result<Actor, AppError> {
    let validation = validate_session(connection, now)
        .map_err(|error| internal_error("require_session", error))?;
    let SessionValidation::Valid(session) = validation else {
        return Err(AppError::new(
            "sessionExpired",
            SESSION_EXPIRED_MESSAGE,
            "session missing or expired",
        ));
    };

    let (username, role): (String, String) = connection
        .query_row(
            "SELECT username, role FROM users WHERE id = ?1",
            [session.user_id],
            |row| Ok((row.get(0)?, row.get(1)?)),
        )
        .map_err(|error| internal_error("require_session", error))?;
    Ok(Actor {
        id: session.user_id,
        role,
        username,
    })
}

pub fn require_admin(connection: &Connection, now: DateTime<Utc>) -> Result<AdminActor, AppError> {
    let actor = require_session(connection, now)?;
    if actor.role != "administrador" {
        return Err(AppError::new(
            "forbidden",
            FORBIDDEN_MESSAGE,
            "actor role is not administrator",
        ));
    }
    Ok(AdminActor { id: actor.id })
}

pub fn login_inner(
    connection: &mut Connection,
    now: DateTime<Utc>,
    username: &str,
    password: &str,
    remember_session: bool,
) -> Result<SessionInfo, AppError> {
    match validate_session(connection, now).map_err(|error| internal_error("login", error))? {
        SessionValidation::Valid(_) => {
            return Err(AppError::new(
                "sessionAlreadyActive",
                SESSION_ALREADY_ACTIVE_MESSAGE,
                "a valid session already exists",
            ));
        }
        SessionValidation::NoSession
        | SessionValidation::ExpiredByInactivity
        | SessionValidation::ExpiredByPersistence => {}
    }

    let normalized_username = normalize_username(username);
    let user: Option<(i64, String, String)> = connection
        .query_row(
            "SELECT id, passwordHash, role FROM users WHERE username = ?1",
            [&normalized_username],
            |row| Ok((row.get(0)?, row.get(1)?, row.get(2)?)),
        )
        .optional()
        .map_err(|error| internal_error("login", error))?;

    if password.len() > 72 {
        return Err(invalid_credentials());
    }
    let Some((user_id, password_hash, role)) = user else {
        bcrypt::verify(password, DUMMY_PASSWORD_HASH)
            .map_err(|_| internal_error("login", "dummy bcrypt verification failed"))?;
        return Err(invalid_credentials());
    };

    let password_matches = bcrypt::verify(password, &password_hash)
        .map_err(|_| internal_error("login", "bcrypt verification failed"))?;
    if !password_matches {
        return Err(invalid_credentials());
    }

    let timestamp = now.to_rfc3339_opts(SecondsFormat::Secs, true);
    connection
        .execute(
            "INSERT INTO sessions (id, userId, rememberSession, createdAt, lastActivityAt)
             VALUES (1, ?1, ?2, ?3, ?3)",
            (user_id, remember_session, &timestamp),
        )
        .map_err(|error| internal_error("login", error))?;

    Ok(SessionInfo {
        username: normalized_username,
        role,
        remember_session,
    })
}

#[tauri::command]
pub fn login(
    state: tauri::State<'_, std::sync::Mutex<Connection>>,
    username: String,
    password: String,
    remember_session: bool,
) -> Result<SessionInfo, AppError> {
    let mut connection = crate::db::connection::lock_conn(&state);
    let now = Utc::now();
    let result = login_inner(&mut connection, now, &username, &password, remember_session);
    if let Err(error) = &result {
        if error.should_log() {
            if let Some(app_data_dir) = crate::db::connection::current_app_data_dir() {
                crate::errors::log_error(&connection, "auth", "login", error, now, app_data_dir);
            }
        }
    }
    result
}

fn invalid_credentials() -> AppError {
    AppError::new(
        "invalidCredentials",
        INVALID_CREDENTIALS_MESSAGE,
        "login credentials did not match",
    )
}

pub fn logout_inner(connection: &mut Connection, _now: DateTime<Utc>) -> Result<(), AppError> {
    connection
        .execute("DELETE FROM sessions", [])
        .map(|_| ())
        .map_err(|error| internal_error("logout", error))
}

#[tauri::command]
pub fn logout(state: tauri::State<'_, std::sync::Mutex<Connection>>) -> Result<(), AppError> {
    let mut connection = crate::db::connection::lock_conn(&state);
    let now = Utc::now();
    let result = logout_inner(&mut connection, now);
    if let Err(error) = &result {
        if error.should_log() {
            if let Some(app_data_dir) = crate::db::connection::current_app_data_dir() {
                crate::errors::log_error(&connection, "auth", "logout", error, now, app_data_dir);
            }
        }
    }
    result
}

pub fn get_session_info_inner(
    connection: &mut Connection,
    now: DateTime<Utc>,
) -> Result<SessionInfo, AppError> {
    let actor = require_session(connection, now)?;
    let row: (String, bool) = connection
        .query_row(
            "SELECT username, (SELECT rememberSession FROM sessions WHERE id = 1)
             FROM users WHERE id = ?1",
            [actor.id],
            |row| Ok((row.get(0)?, row.get(1)?)),
        )
        .map_err(|error| internal_error("get_session_info", error))?;
    Ok(SessionInfo {
        username: row.0,
        role: actor.role,
        remember_session: row.1,
    })
}

#[tauri::command]
pub fn get_session_info(
    state: tauri::State<'_, std::sync::Mutex<Connection>>,
) -> Result<SessionInfo, AppError> {
    let mut connection = crate::db::connection::lock_conn(&state);
    let now = Utc::now();
    let result = get_session_info_inner(&mut connection, now);
    if let Err(error) = &result {
        if let Some(app_data_dir) = crate::db::connection::current_app_data_dir() {
            crate::errors::log_error(
                &connection,
                "auth",
                "getSessionInfo",
                error,
                now,
                app_data_dir,
            );
        }
    }
    result
}

fn internal_error(operation: &str, error: impl std::fmt::Display) -> AppError {
    AppError::new(
        "internal",
        "Ocurrió un error inesperado",
        &format!("{operation}: {error}"),
    )
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct CurrentSession {
    pub username: String,
    pub role: String,
    pub remember_session: bool,
}

pub fn get_current_session_inner(
    connection: &Connection,
    now: DateTime<Utc>,
) -> Result<Option<CurrentSession>, AppError> {
    let validation = validate_session(connection, now)
        .map_err(|error| internal_error("get_current_session", error))?;
    let SessionValidation::Valid(session) = validation else {
        return Ok(None);
    };

    let row: Option<(String, String)> = connection
        .query_row(
            "SELECT username, role FROM users WHERE id = ?1",
            [session.user_id],
            |row| Ok((row.get(0)?, row.get(1)?)),
        )
        .optional()
        .map_err(|error| internal_error("get_current_session", error))?;

    let Some((username, role)) = row else {
        return Ok(None);
    };
    Ok(Some(CurrentSession {
        username,
        role,
        remember_session: session.remember_session,
    }))
}

#[tauri::command]
pub fn get_current_session(
    state: tauri::State<'_, std::sync::Mutex<Connection>>,
) -> Option<CurrentSession> {
    let connection = crate::db::connection::lock_conn(&state);
    let now = Utc::now();
    match get_current_session_inner(&connection, now) {
        Ok(session) => session,
        Err(error) => {
            if let Some(app_data_dir) = crate::db::connection::current_app_data_dir() {
                crate::errors::log_error(
                    &connection,
                    "auth",
                    "getCurrentSession",
                    &error,
                    now,
                    app_data_dir,
                );
            }
            None
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct TouchResult {
    pub status: String,
}

pub fn touch_session_inner(
    connection: &mut Connection,
    now: DateTime<Utc>,
) -> Result<TouchResult, AppError> {
    let validation = validate_session(connection, now)
        .map_err(|error| internal_error("touch_session", error))?;

    match validation {
        SessionValidation::Valid(_) => {
            connection
                .execute(
                    "UPDATE sessions SET lastActivityAt = ?1 WHERE id = 1",
                    [now.to_rfc3339_opts(SecondsFormat::Secs, true)],
                )
                .map_err(|error| internal_error("touch_session", error))?;
            Ok(TouchResult {
                status: "active".to_string(),
            })
        }
        SessionValidation::NoSession
        | SessionValidation::ExpiredByInactivity
        | SessionValidation::ExpiredByPersistence => Ok(TouchResult {
            status: "expired".to_string(),
        }),
    }
}

#[tauri::command]
pub fn touch_session(
    state: tauri::State<'_, std::sync::Mutex<Connection>>,
) -> Result<TouchResult, AppError> {
    let mut connection = crate::db::connection::lock_conn(&state);
    let now = Utc::now();
    let result = touch_session_inner(&mut connection, now);
    if let Err(error) = &result {
        if error.should_log() {
            if let Some(app_data_dir) = crate::db::connection::current_app_data_dir() {
                crate::errors::log_error(
                    &connection,
                    "auth",
                    "touchSession",
                    error,
                    now,
                    app_data_dir,
                );
            }
        }
    }
    result
}

fn verify_recovery_user(
    connection: &Connection,
    username: &str,
    phone: &str,
    national_id: &str,
) -> Result<i64, AppError> {
    let normalized_username = normalize_username(username);
    validate_phone(phone)?;
    validate_national_id(national_id)?;

    let user: Option<(i64, String, String)> = connection
        .query_row(
            "SELECT id, phone, nationalId FROM users WHERE username = ?1",
            [&normalized_username],
            |row| Ok((row.get(0)?, row.get(1)?, row.get(2)?)),
        )
        .optional()
        .map_err(|error| internal_error("verify_recovery_user", error))?;

    let Some((user_id, stored_phone, stored_national_id)) = user else {
        return Err(AppError::new(
            "validationFailed",
            RECOVERY_DATA_MISMATCH_MESSAGE,
            "recovery user not found",
        ));
    };

    validate_not_reserved(phone, &stored_phone)?;
    validate_not_reserved(national_id, &stored_national_id)?;

    if phone != stored_phone || national_id != stored_national_id {
        return Err(AppError::new(
            "validationFailed",
            RECOVERY_DATA_MISMATCH_MESSAGE,
            "recovery data mismatch",
        ));
    }

    Ok(user_id)
}

pub fn verify_recovery_identity_inner(
    connection: &Connection,
    username: &str,
    phone: &str,
    national_id: &str,
) -> Result<(), AppError> {
    verify_recovery_user(connection, username, phone, national_id)?;
    Ok(())
}

#[tauri::command]
pub fn verify_recovery_identity(
    state: tauri::State<'_, std::sync::Mutex<Connection>>,
    username: String,
    phone: String,
    national_id: String,
) -> Result<(), AppError> {
    let connection = crate::db::connection::lock_conn(&state);
    let now = Utc::now();
    let result = verify_recovery_identity_inner(&connection, &username, &phone, &national_id);
    if let Err(error) = &result {
        if error.should_log() {
            if let Some(app_data_dir) = crate::db::connection::current_app_data_dir() {
                crate::errors::log_error(
                    &connection,
                    "auth",
                    "verifyRecoveryIdentity",
                    error,
                    now,
                    app_data_dir,
                );
            }
        }
    }
    result
}

pub fn recover_password_inner(
    connection: &mut Connection,
    _now: DateTime<Utc>,
    username: &str,
    phone: &str,
    national_id: &str,
    new_password: &str,
) -> Result<(), AppError> {
    let user_id = verify_recovery_user(connection, username, phone, national_id)?;
    validate_password(new_password)?;

    let password_hash =
        hash_password(new_password).map_err(|error| internal_error("recover_password", error))?;

    let updated = connection
        .execute(
            "UPDATE users SET passwordHash = ?1 WHERE id = ?2",
            (&password_hash, user_id),
        )
        .map_err(|error| internal_error("recover_password", error))?;

    if updated != 1 {
        return Err(internal_error("recover_password", "user row not updated"));
    }

    Ok(())
}

#[tauri::command]
pub fn recover_password(
    state: tauri::State<'_, std::sync::Mutex<Connection>>,
    username: String,
    phone: String,
    national_id: String,
    new_password: String,
) -> Result<(), AppError> {
    let mut connection = crate::db::connection::lock_conn(&state);
    let now = Utc::now();
    let result = recover_password_inner(
        &mut connection,
        now,
        &username,
        &phone,
        &national_id,
        &new_password,
    );
    if let Err(error) = &result {
        if error.should_log() {
            if let Some(app_data_dir) = crate::db::connection::current_app_data_dir() {
                crate::errors::log_error(
                    &connection,
                    "auth",
                    "recoverPassword",
                    error,
                    now,
                    app_data_dir,
                );
            }
        }
    }
    result
}

#[cfg(test)]
mod tests {
    use chrono::{DateTime, Duration, SecondsFormat, TimeZone, Utc};
    use rusqlite::Connection;

    use crate::commands::auth::{
        get_current_session_inner, get_session_info_inner, hash_password, login_inner,
        logout_inner, recover_password_inner, require_admin, require_session, touch_session_inner,
        validate_session, verify_recovery_identity_inner, SessionInfo, SessionValidation,
        RECOVERY_DATA_MISMATCH_MESSAGE,
    };
    use crate::db::schema::init_schema;
    use crate::db::schema::{RESERVED_NATIONAL_ID, RESERVED_PHONE};
    use crate::errors::AppError;
    use crate::models::session::TouchResult;
    use crate::models::user::{UserListItem, UserProfile};
    use crate::test_utils::{create_test_session, create_test_user, setup_test_db};

    fn now() -> DateTime<Utc> {
        Utc.with_ymd_and_hms(2026, 9, 26, 12, 0, 0).unwrap()
    }

    fn add_user(connection: &Connection, username: &str, role: &str) -> i64 {
        let suffix = (username
            .bytes()
            .fold(0_u32, |sum, byte| sum + u32::from(byte))
            % 90
            + 10)
            .to_string();
        create_test_user(
            connection,
            username,
            role,
            &format!("+53000000{suffix}"),
            &format!("000000000{suffix}"),
            "ValidPass1!",
        )
    }

    fn assert_session_absent(connection: &Connection) {
        let count: i64 = connection
            .query_row("SELECT COUNT(*) FROM sessions", [], |row| row.get(0))
            .unwrap();
        assert_eq!(count, 0);
    }

    fn error_code(result: Result<impl std::fmt::Debug, AppError>) -> String {
        result.unwrap_err().code
    }

    #[test]
    fn hash_password_uses_fast_bcrypt_and_rejects_overlong_input() {
        let hash = hash_password("ValidPass1!").unwrap();

        assert!(hash.starts_with("$2b$04$"));
        assert_ne!(hash, "ValidPass1!");
        assert!(bcrypt::non_truncating_verify("ValidPass1!", &hash).unwrap());
        assert!(hash_password(&"a".repeat(73)).is_err());
    }

    #[test]
    fn dummy_login_hash_is_a_valid_bcrypt_hash() {
        assert!(super::DUMMY_PASSWORD_HASH.starts_with("$2a$10$"));
        assert!(!bcrypt::verify("not-the-dummy-password", super::DUMMY_PASSWORD_HASH).unwrap());
    }

    #[test]
    fn login_creates_session_for_valid_normalized_credentials() {
        let mut connection = setup_test_db();
        let user_id = add_user(&connection, "tester", "contador");
        let current_time = now();

        let info =
            login_inner(&mut connection, current_time, "TeStEr", "ValidPass1!", true).unwrap();

        assert_eq!(info.username, "tester");
        assert_eq!(info.role, "contador");
        assert!(info.remember_session);
        let session: (i64, bool, String, String) = connection
            .query_row(
                "SELECT userId, rememberSession, createdAt, lastActivityAt FROM sessions",
                [],
                |row| Ok((row.get(0)?, row.get(1)?, row.get(2)?, row.get(3)?)),
            )
            .unwrap();
        let timestamp = current_time.to_rfc3339_opts(SecondsFormat::Secs, true);
        assert_eq!(session, (user_id, true, timestamp.clone(), timestamp));
    }

    #[test]
    fn login_accepts_bootstrap_admin_credentials() {
        let mut connection = setup_test_db();
        let current_time = now();
        init_schema(&connection, current_time).unwrap();

        let info = login_inner(&mut connection, current_time, "ADMIN", "Admin123!", false).unwrap();

        assert_eq!(info.username, "admin");
        assert_eq!(info.role, "administrador");
        assert!(!info.remember_session);
    }

    #[test]
    fn login_returns_generic_error_for_unknown_user_or_wrong_password() {
        let mut connection = setup_test_db();
        add_user(&connection, "tester", "contador");
        let current_time = now();

        for (username, password) in [("tester", "IncorrectPass1!"), ("unknown", "ValidPass1!")] {
            let error =
                login_inner(&mut connection, current_time, username, password, false).unwrap_err();
            assert_eq!(error.code, "invalidCredentials");
            assert_eq!(
                error.message,
                "El nombre de usuario o la contraseña son incorrectos"
            );
        }
        assert_session_absent(&connection);
    }

    #[test]
    fn login_rejects_a_second_valid_session_without_changing_it() {
        let mut connection = setup_test_db();
        let user_id = add_user(&connection, "tester", "contador");
        let current_time = now();
        let previous_activity = current_time - Duration::hours(1);
        create_test_session(
            &connection,
            user_id,
            true,
            previous_activity,
            previous_activity,
        );

        let error = login_inner(
            &mut connection,
            current_time,
            "tester",
            "ValidPass1!",
            false,
        )
        .unwrap_err();

        assert_eq!(error.code, "sessionAlreadyActive");
        assert_eq!(
            error.message,
            "Ya hay una sesión activa. Cierra la sesión actual antes de iniciar otra"
        );
        let stored_activity: String = connection
            .query_row("SELECT lastActivityAt FROM sessions", [], |row| row.get(0))
            .unwrap();
        assert_eq!(
            stored_activity,
            previous_activity.to_rfc3339_opts(SecondsFormat::Secs, true)
        );
    }

    #[test]
    fn login_purges_an_expired_session_before_authenticating() {
        let mut connection = setup_test_db();
        let user_id = add_user(&connection, "tester", "contador");
        let current_time = now();
        create_test_session(
            &connection,
            user_id,
            false,
            current_time - Duration::hours(4),
            current_time - Duration::hours(4),
        );

        let info = login_inner(
            &mut connection,
            current_time,
            "tester",
            "ValidPass1!",
            false,
        )
        .unwrap();

        assert_eq!(info.username, "tester");
        let count: i64 = connection
            .query_row("SELECT COUNT(*) FROM sessions", [], |row| row.get(0))
            .unwrap();
        assert_eq!(count, 1);
    }

    #[test]
    fn login_does_not_accept_bcrypt_prefix_truncation_after_72_bytes() {
        let mut connection = setup_test_db();
        let password = format!("A{}!", "a".repeat(70));
        assert_eq!(password.len(), 72);
        create_test_user(
            &connection,
            "tester",
            "contador",
            "+5350000000",
            "12345678901",
            &password,
        );
        let current_time = now();

        assert!(login_inner(&mut connection, current_time, "tester", &password, false,).is_ok());
        connection.execute("DELETE FROM sessions", []).unwrap();

        let error = login_inner(
            &mut connection,
            current_time,
            "tester",
            &format!("{password}x"),
            false,
        )
        .unwrap_err();
        assert_eq!(error.code, "invalidCredentials");
    }

    #[test]
    fn logout_deletes_valid_and_expired_sessions_and_succeeds_without_one() {
        let mut connection = setup_test_db();
        let current_time = now();

        assert!(logout_inner(&mut connection, current_time).is_ok());
        assert_session_absent(&connection);

        let user_id = add_user(&connection, "tester", "contador");
        create_test_session(&connection, user_id, true, current_time, current_time);
        assert!(logout_inner(&mut connection, current_time).is_ok());
        assert_session_absent(&connection);

        create_test_session(
            &connection,
            user_id,
            false,
            current_time - Duration::hours(4),
            current_time - Duration::hours(4),
        );
        assert!(logout_inner(&mut connection, current_time).is_ok());
        assert_session_absent(&connection);
    }

    #[test]
    fn validate_session_covers_absent_valid_and_both_expiration_modes() {
        let connection = setup_test_db();
        let user_id = add_user(&connection, "tester", "contador");
        let current_time = now();

        assert!(matches!(
            validate_session(&connection, current_time).unwrap(),
            SessionValidation::NoSession
        ));

        create_test_session(
            &connection,
            user_id,
            false,
            current_time - Duration::hours(4),
            current_time - Duration::hours(3) - Duration::seconds(1),
        );
        assert!(matches!(
            validate_session(&connection, current_time).unwrap(),
            SessionValidation::ExpiredByInactivity
        ));
        assert_session_absent(&connection);

        create_test_session(
            &connection,
            user_id,
            true,
            current_time - Duration::days(31),
            current_time - Duration::hours(40),
        );
        assert!(matches!(
            validate_session(&connection, current_time).unwrap(),
            SessionValidation::ExpiredByPersistence
        ));
        assert_session_absent(&connection);

        create_test_session(
            &connection,
            user_id,
            true,
            current_time - Duration::hours(40),
            current_time - Duration::hours(40),
        );
        let validation = validate_session(&connection, current_time).unwrap();
        assert!(matches!(validation, SessionValidation::Valid(_)));
        assert_eq!(
            connection
                .query_row("SELECT COUNT(*) FROM sessions", [], |row| row
                    .get::<_, i64>(0))
                .unwrap(),
            1
        );
    }

    #[test]
    fn validate_session_respects_exact_inactivity_and_persistence_boundaries() {
        let connection = setup_test_db();
        let user_id = add_user(&connection, "tester", "contador");
        let current_time = now();

        create_test_session(
            &connection,
            user_id,
            false,
            current_time - Duration::hours(4),
            current_time - Duration::hours(3),
        );
        assert!(matches!(
            validate_session(&connection, current_time).unwrap(),
            SessionValidation::Valid(_)
        ));
        connection.execute("DELETE FROM sessions", []).unwrap();

        create_test_session(
            &connection,
            user_id,
            false,
            current_time - Duration::hours(4),
            current_time - Duration::hours(3) - Duration::seconds(1),
        );
        assert!(matches!(
            validate_session(&connection, current_time).unwrap(),
            SessionValidation::ExpiredByInactivity
        ));
        assert_session_absent(&connection);

        create_test_session(
            &connection,
            user_id,
            true,
            current_time - Duration::days(30),
            current_time - Duration::days(30),
        );
        assert!(matches!(
            validate_session(&connection, current_time).unwrap(),
            SessionValidation::Valid(_)
        ));
        connection.execute("DELETE FROM sessions", []).unwrap();

        create_test_session(
            &connection,
            user_id,
            true,
            current_time - Duration::days(30) - Duration::seconds(1),
            current_time - Duration::days(30) - Duration::seconds(1),
        );
        assert!(matches!(
            validate_session(&connection, current_time).unwrap(),
            SessionValidation::ExpiredByPersistence
        ));
        assert_session_absent(&connection);
    }

    #[test]
    fn remembered_session_does_not_expire_after_three_hours_inactive() {
        let connection = setup_test_db();
        let user_id = add_user(&connection, "tester", "contador");
        let current_time = now();
        create_test_session(
            &connection,
            user_id,
            true,
            current_time - Duration::days(1),
            current_time - Duration::hours(20),
        );

        assert!(matches!(
            validate_session(&connection, current_time).unwrap(),
            SessionValidation::Valid(_)
        ));
    }

    #[test]
    fn require_session_returns_actor_and_rejects_absent_or_expired_session() {
        let connection = setup_test_db();
        let user_id = add_user(&connection, "tester", "contador");
        let current_time = now();

        assert_eq!(
            error_code(require_session(&connection, current_time)),
            "sessionExpired"
        );
        create_test_session(
            &connection,
            user_id,
            false,
            current_time - Duration::hours(4),
            current_time - Duration::hours(4),
        );
        assert_eq!(
            error_code(require_session(&connection, current_time)),
            "sessionExpired"
        );
        assert_session_absent(&connection);

        create_test_session(
            &connection,
            user_id,
            true,
            current_time - Duration::days(31),
            current_time,
        );
        assert_eq!(
            error_code(require_session(&connection, current_time)),
            "sessionExpired"
        );
        assert_session_absent(&connection);

        create_test_session(&connection, user_id, false, current_time, current_time);
        let actor = require_session(&connection, current_time).unwrap();
        assert_eq!(actor.id(), user_id);
        assert_eq!(actor.role(), "contador");
        assert_eq!(actor.username(), "tester");
        let stored_activity: String = connection
            .query_row("SELECT lastActivityAt FROM sessions", [], |row| row.get(0))
            .unwrap();
        assert_eq!(
            stored_activity,
            current_time.to_rfc3339_opts(SecondsFormat::Secs, true)
        );
    }

    #[test]
    fn require_admin_checks_session_before_role_and_preserves_valid_counter_session() {
        let connection = setup_test_db();
        let admin_id = add_user(&connection, "adminx", "administrador");
        let counter_id = add_user(&connection, "counter", "contador");
        let current_time = now();

        assert_eq!(
            error_code(require_admin(&connection, current_time)),
            "sessionExpired"
        );
        create_test_session(&connection, counter_id, false, current_time, current_time);
        assert_eq!(
            error_code(require_admin(&connection, current_time)),
            "forbidden"
        );
        assert_eq!(
            connection
                .query_row("SELECT COUNT(*) FROM sessions", [], |row| row
                    .get::<_, i64>(0))
                .unwrap(),
            1
        );
        connection.execute("DELETE FROM sessions", []).unwrap();

        create_test_session(
            &connection,
            counter_id,
            false,
            current_time - Duration::hours(5),
            current_time - Duration::hours(5),
        );
        assert_eq!(
            error_code(require_admin(&connection, current_time)),
            "sessionExpired"
        );
        assert_session_absent(&connection);

        create_test_session(&connection, admin_id, false, current_time, current_time);
        let admin_actor = require_admin(&connection, current_time).unwrap();
        assert_eq!(admin_actor.id(), admin_id);
    }

    #[test]
    fn get_session_info_returns_camel_case_dto_without_touching_activity() {
        let mut connection = setup_test_db();
        let user_id = add_user(&connection, "tester", "contador");
        let current_time = now();
        let previous_activity = current_time - Duration::hours(1);
        create_test_session(&connection, user_id, true, current_time, previous_activity);

        let info = get_session_info_inner(&mut connection, current_time).unwrap();
        assert_eq!(
            info,
            SessionInfo {
                username: "tester".into(),
                role: "contador".into(),
                remember_session: true,
            }
        );
        let stored_activity: String = connection
            .query_row("SELECT lastActivityAt FROM sessions", [], |row| row.get(0))
            .unwrap();
        assert_eq!(
            stored_activity,
            previous_activity.to_rfc3339_opts(SecondsFormat::Secs, true)
        );
        let serialized = serde_json::to_value(info).unwrap();
        assert_eq!(serialized["rememberSession"], true);
        assert!(serialized.get("remember_session").is_none());

        let mut expired_connection = setup_test_db();
        let expired_user = add_user(&expired_connection, "expired", "contador");
        create_test_session(
            &expired_connection,
            expired_user,
            false,
            current_time - Duration::hours(4),
            current_time - Duration::hours(4),
        );
        assert_eq!(
            error_code(get_session_info_inner(
                &mut expired_connection,
                current_time
            )),
            "sessionExpired"
        );
        assert_session_absent(&expired_connection);
    }

    #[test]
    fn get_session_info_requires_a_session() {
        let mut connection = setup_test_db();
        assert_eq!(
            error_code(get_session_info_inner(&mut connection, now())),
            "sessionExpired"
        );
    }

    #[test]
    fn get_session_info_is_covered_by_the_shared_session_matrix() {
        crate::test_utils::assert_requires_session(|connection, current_time| {
            get_session_info_inner(connection, current_time)
        });
    }

    #[test]
    fn response_dtos_serialize_camel_case_without_password_hash() {
        let session_info = SessionInfo {
            username: "tester".into(),
            role: "contador".into(),
            remember_session: true,
        };
        let user_list_item = UserListItem {
            id: 7,
            username: "tester".into(),
            role: "contador".into(),
            phone: "+5350000000".into(),
            national_id: "12345678901".into(),
            is_current_user: true,
        };
        let user_profile = UserProfile {
            username: "tester".into(),
            role: "contador".into(),
            phone: "+5350000000".into(),
            national_id: "12345678901".into(),
            has_reserved_contact_data: false,
        };
        let touch_result = TouchResult {
            status: "active".into(),
        };

        let serialized_dtos = [
            serde_json::to_value(session_info).unwrap(),
            serde_json::to_value(user_list_item).unwrap(),
            serde_json::to_value(user_profile).unwrap(),
            serde_json::to_value(touch_result).unwrap(),
        ];

        assert_eq!(serialized_dtos[0]["rememberSession"], true);
        assert_eq!(serialized_dtos[1]["nationalId"], "12345678901");
        assert_eq!(serialized_dtos[1]["isCurrentUser"], true);
        assert_eq!(serialized_dtos[2]["hasReservedContactData"], false);
        assert_eq!(serialized_dtos[3]["status"], "active");
        for dto in serialized_dtos {
            assert!(dto.get("passwordHash").is_none());
            assert!(dto.get("national_id").is_none());
        }
    }

    #[test]
    fn verify_recovery_identity_accepts_valid_contact_data() {
        let connection = setup_test_db();
        let _current_time = now();
        create_test_user(
            &connection,
            "testuser",
            "contador",
            "+5350000001",
            "12345678901",
            "ValidPass1!",
        );

        assert!(verify_recovery_identity_inner(
            &connection,
            "testuser",
            "+5350000001",
            "12345678901",
        )
        .is_ok());
    }

    #[test]
    fn verify_recovery_identity_rejects_reserved_contact_data() {
        let connection = setup_test_db();
        let _current_time = now();
        init_schema(&connection, _current_time).unwrap();

        for (username, phone, national_id) in [
            ("admin", RESERVED_PHONE, "12345678901"),
            ("admin", "+5350000001", RESERVED_NATIONAL_ID),
        ] {
            let error = verify_recovery_identity_inner(&connection, username, phone, national_id)
                .unwrap_err();
            assert_eq!(error.code, "validationFailed");
            assert_eq!(error.message, RECOVERY_DATA_MISMATCH_MESSAGE);
        }
    }

    #[test]
    fn verify_recovery_identity_returns_same_generic_error_for_invalid_data() {
        let connection = setup_test_db();
        let _current_time = now();
        create_test_user(
            &connection,
            "testuser",
            "contador",
            "+5350000001",
            "12345678901",
            "ValidPass1!",
        );

        for (username, phone, national_id) in [
            ("unknown", "+5350000001", "12345678901"),
            ("testuser", "+5350000002", "12345678901"),
            ("testuser", "+5350000001", "12345678902"),
        ] {
            let error = verify_recovery_identity_inner(&connection, username, phone, national_id)
                .unwrap_err();
            assert_eq!(error.code, "validationFailed");
            assert_eq!(error.message, RECOVERY_DATA_MISMATCH_MESSAGE);
        }
    }

    #[test]
    fn recover_password_updates_hash_after_reverifying_contact_data() {
        let mut connection = setup_test_db();
        let current_time = now();
        let user_id = create_test_user(
            &connection,
            "testuser",
            "contador",
            "+5350000001",
            "12345678901",
            "ValidPass1!",
        );

        let old_hash: String = connection
            .query_row(
                "SELECT passwordHash FROM users WHERE id = ?1",
                [user_id],
                |row| row.get(0),
            )
            .unwrap();

        assert!(recover_password_inner(
            &mut connection,
            current_time,
            "testuser",
            "+5350000001",
            "12345678901",
            "NewPassword2!",
        )
        .is_ok());

        let new_hash: String = connection
            .query_row(
                "SELECT passwordHash FROM users WHERE id = ?1",
                [user_id],
                |row| row.get(0),
            )
            .unwrap();

        assert_ne!(new_hash, old_hash);
        assert!(bcrypt::verify("NewPassword2!", &new_hash).unwrap());
        assert!(!bcrypt::verify("ValidPass1!", &new_hash).unwrap());
    }

    #[test]
    fn recover_password_reverifies_data_and_rejects_invalid_inputs() {
        let mut connection = setup_test_db();
        let current_time = now();
        create_test_user(
            &connection,
            "testuser",
            "contador",
            "+5350000001",
            "12345678901",
            "ValidPass1!",
        );

        for (username, phone, national_id, password) in [
            ("unknown", "+5350000001", "12345678901", "NewPassword2!"),
            ("testuser", "+5350000002", "12345678901", "NewPassword2!"),
            ("testuser", "+5350000001", "12345678902", "NewPassword2!"),
            ("testuser", "+5350000001", "12345678901", "short"),
        ] {
            let error = recover_password_inner(
                &mut connection,
                current_time,
                username,
                phone,
                national_id,
                password,
            )
            .unwrap_err();
            assert!(matches!(error.code.as_str(), "validationFailed"));
        }
    }

    #[test]
    fn recover_password_rejects_reserved_contact_data() {
        let mut connection = setup_test_db();
        let current_time = now();
        init_schema(&connection, current_time).unwrap();

        for (phone, national_id) in [
            (RESERVED_PHONE, "12345678901"),
            ("+5350000001", RESERVED_NATIONAL_ID),
        ] {
            let error = recover_password_inner(
                &mut connection,
                current_time,
                "admin",
                phone,
                national_id,
                "NewPassword2!",
            )
            .unwrap_err();
            assert_eq!(error.code, "validationFailed");
            assert_eq!(error.message, RECOVERY_DATA_MISMATCH_MESSAGE);
        }
    }

    #[test]
    fn get_session_info_respects_exact_inactivity_and_persistence_boundaries() {
        let mut connection = setup_test_db();
        let user_id = add_user(&connection, "tester", "contador");
        let current_time = now();

        create_test_session(
            &connection,
            user_id,
            false,
            current_time - Duration::hours(4),
            current_time - Duration::hours(3),
        );
        assert!(get_session_info_inner(&mut connection, current_time).is_ok());
        connection.execute("DELETE FROM sessions", []).unwrap();

        create_test_session(
            &connection,
            user_id,
            false,
            current_time - Duration::hours(4),
            current_time - Duration::hours(3) - Duration::seconds(1),
        );
        let error = get_session_info_inner(&mut connection, current_time).unwrap_err();
        assert_eq!(error.code, "sessionExpired");
        connection.execute("DELETE FROM sessions", []).unwrap();

        create_test_session(
            &connection,
            user_id,
            true,
            current_time - Duration::days(30),
            current_time - Duration::days(30),
        );
        assert!(get_session_info_inner(&mut connection, current_time).is_ok());
        connection.execute("DELETE FROM sessions", []).unwrap();

        create_test_session(
            &connection,
            user_id,
            true,
            current_time - Duration::days(30) - Duration::seconds(1),
            current_time - Duration::days(30) - Duration::seconds(1),
        );
        let error = get_session_info_inner(&mut connection, current_time).unwrap_err();
        assert_eq!(error.code, "sessionExpired");
    }

    #[test]
    fn touch_session_updates_activity_within_timeout() {
        let mut connection = setup_test_db();
        let user_id = add_user(&connection, "tester", "contador");
        let current_time = now();
        let previous_activity = current_time - Duration::hours(1);
        create_test_session(
            &connection,
            user_id,
            false,
            previous_activity,
            previous_activity,
        );

        let result = touch_session_inner(&mut connection, current_time).unwrap();
        assert_eq!(result.status, "active");

        let last_activity: String = connection
            .query_row("SELECT lastActivityAt FROM sessions", [], |row| row.get(0))
            .unwrap();
        assert_eq!(
            last_activity,
            current_time.to_rfc3339_opts(SecondsFormat::Secs, true)
        );
    }

    #[test]
    fn touch_session_deletes_expired_session_without_updating() {
        let mut connection = setup_test_db();
        let user_id = add_user(&connection, "tester", "contador");
        let current_time = now();
        let previous_activity = current_time - Duration::hours(4);
        create_test_session(
            &connection,
            user_id,
            false,
            previous_activity,
            previous_activity,
        );

        let result = touch_session_inner(&mut connection, current_time).unwrap();
        assert_eq!(result.status, "expired");

        let count: i64 = connection
            .query_row("SELECT COUNT(*) FROM sessions", [], |row| row.get(0))
            .unwrap();
        assert_eq!(count, 0);
    }

    #[test]
    fn touch_session_never_expires_remembered_session_by_inactivity() {
        let mut connection = setup_test_db();
        let user_id = add_user(&connection, "tester", "contador");
        let current_time = now();
        let previous_activity = current_time - Duration::hours(4);
        create_test_session(
            &connection,
            user_id,
            true,
            previous_activity,
            previous_activity,
        );

        let result = touch_session_inner(&mut connection, current_time).unwrap();
        assert_eq!(result.status, "active");

        let count: i64 = connection
            .query_row("SELECT COUNT(*) FROM sessions", [], |row| row.get(0))
            .unwrap();
        assert_eq!(count, 1);
    }

    #[test]
    fn get_current_session_returns_session_when_valid() {
        let connection = setup_test_db();
        let user_id = add_user(&connection, "tester", "contador");
        let current_time = now();
        create_test_session(&connection, user_id, false, current_time, current_time);

        let session = get_current_session_inner(&connection, current_time)
            .unwrap()
            .unwrap();
        assert_eq!(session.username, "tester");
        assert_eq!(session.role, "contador");
        assert!(!session.remember_session);
    }

    #[test]
    fn get_current_session_returns_none_for_expired_session() {
        let connection = setup_test_db();
        let user_id = add_user(&connection, "tester", "contador");
        let current_time = now();
        create_test_session(
            &connection,
            user_id,
            false,
            current_time - Duration::hours(4),
            current_time - Duration::hours(4),
        );

        let session = get_current_session_inner(&connection, current_time).unwrap();
        assert!(session.is_none());
    }

    #[test]
    fn get_current_session_returns_none_for_expired_persistent_session() {
        let connection = setup_test_db();
        let user_id = add_user(&connection, "tester", "contador");
        let current_time = now();
        create_test_session(
            &connection,
            user_id,
            true,
            current_time - Duration::days(30) - Duration::seconds(1),
            current_time,
        );

        let session = get_current_session_inner(&connection, current_time).unwrap();

        assert!(session.is_none());
        assert_session_absent(&connection);
    }

    #[test]
    fn get_current_session_returns_none_when_no_session() {
        let connection = setup_test_db();
        let current_time = now();

        let session = get_current_session_inner(&connection, current_time).unwrap();
        assert!(session.is_none());
    }

    #[test]
    fn get_current_session_resumes_persistent_session_after_startup_cleanup() {
        let connection = setup_test_db();
        let user_id = add_user(&connection, "tester", "contador");
        let current_time = now();
        create_test_session(
            &connection,
            user_id,
            false,
            current_time - Duration::minutes(5),
            current_time - Duration::minutes(5),
        );

        crate::db::schema::clear_non_persistent_sessions(&connection).unwrap();
        assert!(get_current_session_inner(&connection, current_time)
            .unwrap()
            .is_none());

        create_test_session(
            &connection,
            user_id,
            true,
            current_time - Duration::days(2),
            current_time - Duration::hours(4),
        );
        let session = get_current_session_inner(&connection, current_time).unwrap();

        assert_eq!(session.unwrap().username, "tester");
        let session_count: i64 = connection
            .query_row("SELECT COUNT(*) FROM sessions", [], |row| row.get(0))
            .unwrap();
        assert_eq!(session_count, 1);
    }

    #[test]
    fn get_current_session_reports_internal_validation_errors() {
        let connection = setup_test_db();
        let user_id = add_user(&connection, "tester", "contador");
        let current_time = now();
        create_test_session(&connection, user_id, true, current_time, current_time);
        connection
            .execute("UPDATE sessions SET createdAt = 'invalid-date'", [])
            .unwrap();

        assert!(get_current_session_inner(&connection, current_time).is_err());
    }
}
