use chrono::{DateTime, SecondsFormat, Utc};
use rusqlite::{Connection, ErrorCode, OptionalExtension};

use crate::commands::auth::{hash_password, require_admin, require_session, Actor, AdminActor};
use crate::db::schema::{RESERVED_NATIONAL_ID, RESERVED_PHONE};
use crate::errors::AppError;
use crate::models::user::{UserListItem, UserProfile};
use crate::validators::{
    validate_national_id, validate_not_reserved, validate_password, validate_phone, validate_role,
    validate_username,
};

const INTERNAL_ERROR_MESSAGE: &str = "Ocurrió un error inesperado";
const SELF_UPDATE_MESSAGE: &str = "Para editar tus propios datos usa tu Perfil";
pub(crate) const SELF_DELETE_MESSAGE: &str = "No puedes eliminar tu propio usuario";
const LAST_ADMINISTRATOR_MESSAGE: &str = "No es posible: es el último Administrador del sistema";
const USER_NOT_FOUND_MESSAGE: &str = "El usuario no existe";

struct NewUser<'a> {
    username: &'a str,
    role: &'a str,
    phone: &'a str,
    national_id: &'a str,
    password: &'a str,
}

pub fn create_user_inner(
    connection: &mut Connection,
    now: DateTime<Utc>,
    username: &str,
    role: &str,
    phone: &str,
    national_id: &str,
    password: &str,
) -> Result<UserListItem, AppError> {
    let admin = require_admin(connection, now)?;
    let username = validate_username(username)?;
    validate_role(role)?;
    validate_phone(phone)?;
    validate_national_id(national_id)?;
    validate_password(password)?;
    validate_not_reserved(phone, "")?;
    validate_not_reserved(national_id, "")?;

    insert_user(
        connection,
        &admin,
        now,
        NewUser {
            username: &username,
            role,
            phone,
            national_id,
            password,
        },
    )
}

fn insert_user(
    connection: &mut Connection,
    admin: &AdminActor,
    now: DateTime<Utc>,
    user: NewUser<'_>,
) -> Result<UserListItem, AppError> {
    let password_hash = hash_password(user.password)
        .map_err(|_| internal_error("create_user password hashing failed"))?;
    let created_at = now.to_rfc3339_opts(SecondsFormat::Secs, true);
    let transaction = connection
        .transaction()
        .map_err(|_| internal_error("create_user transaction start failed"))?;
    transaction
        .execute(
            "INSERT INTO users (username, passwordHash, role, phone, nationalId, createdAt)
             VALUES (?1, ?2, ?3, ?4, ?5, ?6)",
            (
                user.username,
                password_hash,
                user.role,
                user.phone,
                user.national_id,
                created_at,
            ),
        )
        .map_err(map_create_user_error)?;

    let id = transaction.last_insert_rowid();
    notify_user_change(&transaction, "creado", user.username, now)?;
    transaction
        .commit()
        .map_err(|_| internal_error("create_user transaction commit failed"))?;
    Ok(UserListItem {
        id,
        username: user.username.to_owned(),
        role: user.role.to_owned(),
        phone: user.phone.to_owned(),
        national_id: user.national_id.to_owned(),
        is_current_user: id == admin.id(),
    })
}

#[tauri::command]
pub fn create_user(
    state: tauri::State<'_, std::sync::Mutex<Connection>>,
    username: String,
    role: String,
    phone: String,
    national_id: String,
    password: String,
) -> Result<UserListItem, AppError> {
    let mut connection = crate::db::connection::lock_conn(&state);
    let now = Utc::now();
    let result = create_user_inner(
        &mut connection,
        now,
        &username,
        &role,
        &phone,
        &national_id,
        &password,
    );
    if let Err(error) = &result {
        if error.should_log() {
            if let Some(app_data_dir) = crate::db::connection::current_app_data_dir() {
                crate::errors::log_error(
                    &connection,
                    "users",
                    "createUser",
                    error,
                    now,
                    app_data_dir,
                );
            }
        }
    }
    result
}

pub fn list_users_inner(
    connection: &mut Connection,
    now: DateTime<Utc>,
) -> Result<Vec<UserListItem>, AppError> {
    let admin = require_admin(connection, now)?;
    let mut statement = connection
        .prepare(
            "SELECT id, username, role, phone, nationalId
             FROM users
             ORDER BY id ASC",
        )
        .map_err(|_| internal_error("list_users query preparation failed"))?;
    let rows = statement
        .query_map([], |row| {
            let id: i64 = row.get(0)?;
            Ok(UserListItem {
                id,
                username: row.get(1)?,
                role: row.get(2)?,
                phone: row.get(3)?,
                national_id: row.get(4)?,
                is_current_user: id == admin.id(),
            })
        })
        .map_err(|_| internal_error("list_users query failed"))?;
    rows.collect::<rusqlite::Result<Vec<_>>>()
        .map_err(|_| internal_error("list_users row mapping failed"))
}

#[tauri::command]
pub fn list_users(
    state: tauri::State<'_, std::sync::Mutex<Connection>>,
) -> Result<Vec<UserListItem>, AppError> {
    let mut connection = crate::db::connection::lock_conn(&state);
    let now = Utc::now();
    let result = list_users_inner(&mut connection, now);
    if let Err(error) = &result {
        if error.should_log() {
            if let Some(app_data_dir) = crate::db::connection::current_app_data_dir() {
                crate::errors::log_error(
                    &connection,
                    "users",
                    "listUsers",
                    error,
                    now,
                    app_data_dir,
                );
            }
        }
    }
    result
}

pub fn get_user_profile_inner(
    connection: &mut Connection,
    now: DateTime<Utc>,
) -> Result<UserProfile, AppError> {
    let actor = require_session(connection, now)?;
    let (username, role, phone, national_id): (String, String, String, String) = connection
        .query_row(
            "SELECT username, role, phone, nationalId FROM users WHERE id = ?1",
            [actor.id()],
            |row| Ok((row.get(0)?, row.get(1)?, row.get(2)?, row.get(3)?)),
        )
        .map_err(|_| internal_error("get_user_profile query failed"))?;

    Ok(UserProfile {
        username,
        role,
        has_reserved_contact_data: phone == RESERVED_PHONE || national_id == RESERVED_NATIONAL_ID,
        phone,
        national_id,
    })
}

#[tauri::command]
pub fn get_user_profile(
    state: tauri::State<'_, std::sync::Mutex<Connection>>,
) -> Result<UserProfile, AppError> {
    let mut connection = crate::db::connection::lock_conn(&state);
    let now = Utc::now();
    let result = get_user_profile_inner(&mut connection, now);
    if let Err(error) = &result {
        if error.should_log() {
            if let Some(app_data_dir) = crate::db::connection::current_app_data_dir() {
                crate::errors::log_error(
                    &connection,
                    "users",
                    "getUserProfile",
                    error,
                    now,
                    app_data_dir,
                );
            }
        }
    }
    result
}

pub fn update_own_profile_inner(
    connection: &mut Connection,
    now: DateTime<Utc>,
    username: &str,
    phone: &str,
    national_id: &str,
) -> Result<UserProfile, AppError> {
    let actor = require_session(connection, now)?;
    let (current_phone, current_national_id): (String, String) = connection
        .query_row(
            "SELECT phone, nationalId FROM users WHERE id = ?1",
            [actor.id()],
            |row| Ok((row.get(0)?, row.get(1)?)),
        )
        .map_err(|_| internal_error("update_own_profile current user lookup failed"))?;

    let username = validate_username(username)?;
    validate_phone(phone)?;
    validate_national_id(national_id)?;
    validate_not_reserved(phone, &current_phone)?;
    validate_not_reserved(national_id, &current_national_id)?;

    connection
        .execute(
            "UPDATE users SET username = ?1, phone = ?2, nationalId = ?3 WHERE id = ?4",
            (&username, phone, national_id, actor.id()),
        )
        .map_err(map_profile_update_error)?;

    Ok(UserProfile {
        username,
        role: actor.role().to_owned(),
        phone: phone.to_owned(),
        national_id: national_id.to_owned(),
        has_reserved_contact_data: phone == RESERVED_PHONE || national_id == RESERVED_NATIONAL_ID,
    })
}

#[tauri::command]
pub fn update_own_profile(
    state: tauri::State<'_, std::sync::Mutex<Connection>>,
    username: String,
    phone: String,
    national_id: String,
) -> Result<UserProfile, AppError> {
    let mut connection = crate::db::connection::lock_conn(&state);
    let now = Utc::now();
    let result = update_own_profile_inner(&mut connection, now, &username, &phone, &national_id);
    if let Err(error) = &result {
        if error.should_log() {
            if let Some(app_data_dir) = crate::db::connection::current_app_data_dir() {
                crate::errors::log_error(
                    &connection,
                    "users",
                    "updateOwnProfile",
                    error,
                    now,
                    app_data_dir,
                );
            }
        }
    }
    result
}

pub fn change_own_password_inner(
    connection: &mut Connection,
    now: DateTime<Utc>,
    current_password: &str,
    new_password: &str,
) -> Result<(), AppError> {
    let actor = require_session(connection, now)?;
    change_password_with_current(connection, &actor, current_password, new_password)
}

fn change_password_with_current(
    connection: &mut Connection,
    actor: &Actor,
    current_password: &str,
    new_password: &str,
) -> Result<(), AppError> {
    let current_hash: String = connection
        .query_row(
            "SELECT passwordHash FROM users WHERE id = ?1",
            [actor.id()],
            |row| row.get(0),
        )
        .map_err(|_| internal_error("change_own_password current user lookup failed"))?;

    if current_password.len() > 72
        || !bcrypt::verify(current_password, &current_hash)
            .map_err(|_| internal_error("change_own_password bcrypt verification failed"))?
    {
        return Err(AppError::new(
            "validationFailed",
            "La contraseña actual es incorrecta",
            "current password did not match",
        ));
    }

    validate_password(new_password)?;
    let new_hash = hash_password(new_password)
        .map_err(|_| internal_error("change_own_password password hashing failed"))?;
    let updated = connection
        .execute(
            "UPDATE users SET passwordHash = ?1 WHERE id = ?2",
            (&new_hash, actor.id()),
        )
        .map_err(|_| internal_error("change_own_password database update failed"))?;
    if updated != 1 {
        return Err(internal_error(
            "change_own_password user row was not updated",
        ));
    }
    Ok(())
}

#[tauri::command]
pub fn change_own_password(
    state: tauri::State<'_, std::sync::Mutex<Connection>>,
    current_password: String,
    new_password: String,
) -> Result<(), AppError> {
    let mut connection = crate::db::connection::lock_conn(&state);
    let now = Utc::now();
    let result = change_own_password_inner(&mut connection, now, &current_password, &new_password);
    if let Err(error) = &result {
        if error.should_log() {
            if let Some(app_data_dir) = crate::db::connection::current_app_data_dir() {
                crate::errors::log_error(
                    &connection,
                    "users",
                    "changeOwnPassword",
                    error,
                    now,
                    app_data_dir,
                );
            }
        }
    }
    result
}

pub fn update_user_inner(
    connection: &mut Connection,
    now: DateTime<Utc>,
    user_id: i64,
    username: &str,
    role: &str,
    phone: &str,
    national_id: &str,
) -> Result<UserListItem, AppError> {
    let admin = require_admin(connection, now)?;
    if user_id == admin.id() {
        return Err(AppError::new(
            "selfAction",
            SELF_UPDATE_MESSAGE,
            "administrator attempted to update own user row",
        ));
    }

    let transaction = connection
        .transaction()
        .map_err(|_| internal_error("update_user transaction start failed"))?;
    let current_user: Option<(String, String)> = transaction
        .query_row(
            "SELECT phone, nationalId FROM users WHERE id = ?1",
            [user_id],
            |row| Ok((row.get(0)?, row.get(1)?)),
        )
        .optional()
        .map_err(|_| internal_error("update_user target lookup failed"))?;
    let Some((current_phone, current_national_id)) = current_user else {
        return Err(user_not_found("update_user target does not exist"));
    };

    let username = validate_username(username)?;
    validate_role(role)?;
    validate_phone(phone)?;
    validate_national_id(national_id)?;
    validate_not_reserved(phone, &current_phone)?;
    validate_not_reserved(national_id, &current_national_id)?;
    ensure_admin_remains(&transaction, &admin, user_id, role)?;

    transaction
        .execute(
            "UPDATE users SET username = ?1, role = ?2, phone = ?3, nationalId = ?4
             WHERE id = ?5",
            (&username, role, phone, national_id, user_id),
        )
        .map_err(|error| map_user_unique_error(error, "update_user"))?;
    notify_user_change(&transaction, "editado", &username, now)?;
    transaction
        .commit()
        .map_err(|_| internal_error("update_user transaction commit failed"))?;

    Ok(UserListItem {
        id: user_id,
        username,
        role: role.to_owned(),
        phone: phone.to_owned(),
        national_id: national_id.to_owned(),
        is_current_user: false,
    })
}

fn ensure_admin_remains(
    connection: &Connection,
    _admin: &AdminActor,
    target_id: i64,
    new_role: &str,
) -> Result<(), AppError> {
    let current_role: Option<String> = connection
        .query_row("SELECT role FROM users WHERE id = ?1", [target_id], |row| {
            row.get(0)
        })
        .optional()
        .map_err(|_| internal_error("ensure_admin_remains target lookup failed"))?;
    let Some(current_role) = current_role else {
        return Err(user_not_found("ensure_admin_remains target does not exist"));
    };
    if current_role == "administrador" && new_role != "administrador" {
        let administrator_count: i64 = connection
            .query_row(
                "SELECT COUNT(*) FROM users WHERE role = 'administrador'",
                [],
                |row| row.get(0),
            )
            .map_err(|_| internal_error("ensure_admin_remains count failed"))?;
        if administrator_count <= 1 {
            return Err(AppError::new(
                "lastAdministrator",
                LAST_ADMINISTRATOR_MESSAGE,
                "attempted to demote final administrator",
            ));
        }
    }
    Ok(())
}

#[tauri::command]
pub fn update_user(
    state: tauri::State<'_, std::sync::Mutex<Connection>>,
    user_id: i64,
    username: String,
    role: String,
    phone: String,
    national_id: String,
) -> Result<UserListItem, AppError> {
    let mut connection = crate::db::connection::lock_conn(&state);
    let now = Utc::now();
    let result = update_user_inner(
        &mut connection,
        now,
        user_id,
        &username,
        &role,
        &phone,
        &national_id,
    );
    if let Err(error) = &result {
        if error.should_log() {
            if let Some(app_data_dir) = crate::db::connection::current_app_data_dir() {
                crate::errors::log_error(
                    &connection,
                    "users",
                    "updateUser",
                    error,
                    now,
                    app_data_dir,
                );
            }
        }
    }
    result
}

pub fn delete_user_inner(
    connection: &mut Connection,
    now: DateTime<Utc>,
    user_id: i64,
) -> Result<(), AppError> {
    let admin = require_admin(connection, now)?;
    if user_id == admin.id() {
        return Err(AppError::new(
            "selfAction",
            SELF_DELETE_MESSAGE,
            "administrator attempted to delete own user row",
        ));
    }

    let transaction = connection
        .transaction()
        .map_err(|_| internal_error("delete_user transaction start failed"))?;

    let target: Option<String> = transaction
        .query_row(
            "SELECT username FROM users WHERE id = ?1",
            [user_id],
            |row| row.get(0),
        )
        .optional()
        .map_err(|_| internal_error("delete_user target lookup failed"))?;
    let Some(deleted_username) = target else {
        return Err(user_not_found("delete_user target does not exist"));
    };

    ensure_admin_remains(&transaction, &admin, user_id, "contador")?;

    transaction
        .execute("DELETE FROM users WHERE id = ?1", [user_id])
        .map_err(|_| internal_error("delete_user database deletion failed"))?;
    notify_user_change(&transaction, "eliminado", &deleted_username, now)?;
    crate::activity::record_activity(
        &transaction,
        &crate::commands::auth::Author::Admin(&admin),
        now,
        crate::activity::Entry {
            action: "delete",
            entity_type: "user",
            label: &deleted_username,
            details: None,
        },
    )
    .map_err(|_| internal_error("delete_user activity entry failed"))?;

    transaction
        .commit()
        .map_err(|_| internal_error("delete_user transaction commit failed"))?;

    Ok(())
}

#[tauri::command]
pub fn delete_user(
    state: tauri::State<'_, std::sync::Mutex<Connection>>,
    user_id: i64,
) -> Result<(), AppError> {
    let mut connection = crate::db::connection::lock_conn(&state);
    let now = Utc::now();
    let result = delete_user_inner(&mut connection, now, user_id);
    if let Err(error) = &result {
        if error.should_log() {
            if let Some(app_data_dir) = crate::db::connection::current_app_data_dir() {
                crate::errors::log_error(
                    &connection,
                    "users",
                    "deleteUser",
                    error,
                    now,
                    app_data_dir,
                );
            }
        }
    }
    result
}

pub fn reset_user_password_inner(
    connection: &mut Connection,
    now: DateTime<Utc>,
    user_id: i64,
    new_password: &str,
    current_password: Option<&str>,
) -> Result<(), AppError> {
    let admin = require_admin(connection, now)?;
    let target_exists: bool = connection
        .query_row(
            "SELECT EXISTS(SELECT 1 FROM users WHERE id = ?1)",
            [user_id],
            |row| row.get(0),
        )
        .map_err(|_| internal_error("reset_user_password target lookup failed"))?;
    if !target_exists {
        return Err(user_not_found("reset_user_password target does not exist"));
    }

    if user_id == admin.id() {
        let actor = require_session(connection, now)?;
        return change_password_with_current(
            connection,
            &actor,
            current_password.unwrap_or_default(),
            new_password,
        );
    }

    set_user_password(connection, &admin, user_id, new_password)
}

fn set_user_password(
    connection: &mut Connection,
    _admin: &AdminActor,
    user_id: i64,
    new_password: &str,
) -> Result<(), AppError> {
    validate_password(new_password)?;
    let password_hash = hash_password(new_password)
        .map_err(|_| internal_error("reset_user_password hashing failed"))?;
    let updated = connection
        .execute(
            "UPDATE users SET passwordHash = ?1 WHERE id = ?2",
            (&password_hash, user_id),
        )
        .map_err(|_| internal_error("reset_user_password database update failed"))?;
    if updated != 1 {
        return Err(user_not_found("reset_user_password target disappeared"));
    }
    Ok(())
}

#[tauri::command]
pub fn reset_user_password(
    state: tauri::State<'_, std::sync::Mutex<Connection>>,
    user_id: i64,
    new_password: String,
    current_password: Option<String>,
) -> Result<(), AppError> {
    let mut connection = crate::db::connection::lock_conn(&state);
    let now = Utc::now();
    let result = reset_user_password_inner(
        &mut connection,
        now,
        user_id,
        &new_password,
        current_password.as_deref(),
    );
    if let Err(error) = &result {
        if error.should_log() {
            if let Some(app_data_dir) = crate::db::connection::current_app_data_dir() {
                crate::errors::log_error(
                    &connection,
                    "users",
                    "resetUserPassword",
                    error,
                    now,
                    app_data_dir,
                );
            }
        }
    }
    result
}

/// N-13 (NT-02): solo lo ve el Administrador, en la misma transacción.
fn notify_user_change(
    connection: &Connection,
    action: &str,
    username: &str,
    now: DateTime<Utc>,
) -> Result<(), AppError> {
    crate::db::notifications::notify(
        connection,
        "N-13",
        crate::db::notifications::INFO,
        &format!("Usuario {action}: {username}"),
        None,
        crate::db::notifications::Audience::Admin,
        now,
    )
    .map_err(|_| internal_error("user change notification failed"))
}

fn user_not_found(cause: &str) -> AppError {
    AppError::new("notFound", USER_NOT_FOUND_MESSAGE, cause)
}

fn map_create_user_error(error: rusqlite::Error) -> AppError {
    map_user_unique_error(error, "create_user")
}

fn map_user_unique_error(error: rusqlite::Error, operation: &str) -> AppError {
    if let rusqlite::Error::SqliteFailure(sqlite_error, message) = &error {
        if sqlite_error.code == ErrorCode::ConstraintViolation {
            let message = message.as_deref().unwrap_or_default();
            for (column, user_message) in [
                ("users.username", "El nombre de usuario ya está registrado"),
                ("users.phone", "El teléfono ya está registrado"),
                (
                    "users.nationalId",
                    "El carnet de identidad ya está registrado",
                ),
            ] {
                if message.contains(column) {
                    return AppError::new(
                        "duplicateField",
                        user_message,
                        &format!("{operation} unique constraint violated"),
                    );
                }
            }
        }
    }
    internal_error(&format!("{operation} database operation failed"))
}

fn map_profile_update_error(error: rusqlite::Error) -> AppError {
    map_user_unique_error(error, "update_own_profile")
}

fn internal_error(cause: &str) -> AppError {
    AppError::new("internal", INTERNAL_ERROR_MESSAGE, cause)
}

#[cfg(test)]
mod tests {
    use chrono::Utc;

    use crate::commands::auth::hash_password;
    use crate::db::schema::{init_schema, RESERVED_NATIONAL_ID, RESERVED_PHONE};
    use crate::errors::AppError;
    use crate::models::user::UserProfile;
    use crate::test_utils::{
        assert_requires_admin, assert_requires_session, create_test_session, create_test_user,
        setup_test_db,
    };

    use super::{
        change_own_password_inner, create_user_inner, ensure_admin_remains, get_user_profile_inner,
        list_users_inner, reset_user_password_inner, update_own_profile_inner, update_user_inner,
        SELF_DELETE_MESSAGE,
    };

    const ADMIN_USERNAME: &str = "adminuser";
    const ADMIN_PHONE: &str = "+5350000000";
    const ADMIN_NATIONAL_ID: &str = "12345678901";
    const VALID_PASSWORD: &str = "ValidPass1!";

    fn create_admin_session(connection: &rusqlite::Connection, now: chrono::DateTime<Utc>) -> i64 {
        let admin_id = create_test_user(
            connection,
            ADMIN_USERNAME,
            "administrador",
            ADMIN_PHONE,
            ADMIN_NATIONAL_ID,
            VALID_PASSWORD,
        );
        create_test_session(connection, admin_id, false, now, now);
        admin_id
    }

    fn create_user(
        connection: &mut rusqlite::Connection,
        now: chrono::DateTime<Utc>,
        username: &str,
        role: &str,
        phone: &str,
        national_id: &str,
        password: &str,
    ) -> Result<crate::models::user::UserListItem, AppError> {
        create_user_inner(
            connection,
            now,
            username,
            role,
            phone,
            national_id,
            password,
        )
    }

    fn assert_error(result: Result<impl std::fmt::Debug, AppError>, code: &str, message: &str) {
        let error = result.unwrap_err();
        assert_eq!(error.code, code);
        assert_eq!(error.message, message);
    }

    #[test]
    fn n13_creating_editing_and_deleting_a_user_notify_only_the_administrator() {
        let now = Utc::now();
        let mut connection = setup_test_db();
        create_admin_session(&connection, now);

        let created = create_user(
            &mut connection,
            now,
            "nuevocontador",
            "contador",
            "+5350000021",
            "12345678921",
            VALID_PASSWORD,
        )
        .unwrap();
        update_user_inner(
            &mut connection,
            now,
            created.id,
            "contadoredit",
            "contador",
            "+5350000021",
            "12345678921",
        )
        .unwrap();
        crate::commands::users::delete_user_inner(&mut connection, now, created.id).unwrap();
        // Un rechazo no avisa.
        assert!(create_user(
            &mut connection,
            now,
            "otrocontador",
            "contador",
            "+5350000021",
            "12345678921",
            "corta",
        )
        .is_err());

        let rows: Vec<(String, String, String, String, Option<String>)> = connection
            .prepare("SELECT type, priority, message, audience, entityType FROM notifications ORDER BY id")
            .unwrap()
            .query_map([], |row| {
                Ok((row.get(0)?, row.get(1)?, row.get(2)?, row.get(3)?, row.get(4)?))
            })
            .unwrap()
            .collect::<rusqlite::Result<_>>()
            .unwrap();
        let expected = |message: &str| {
            (
                "N-13".to_owned(),
                "INFORMATIVO".to_owned(),
                message.to_owned(),
                "admin".to_owned(),
                None,
            )
        };
        assert_eq!(
            rows,
            vec![
                expected("Usuario creado: nuevocontador"),
                expected("Usuario editado: contadoredit"),
                expected("Usuario eliminado: contadoredit"),
            ]
        );
    }

    #[test]
    fn create_user_persists_normalized_user_with_bcrypt_and_returns_dto() {
        let now = Utc::now();
        let mut connection = setup_test_db();
        let admin_id = create_admin_session(&connection, now);

        let created = create_user(
            &mut connection,
            now,
            "NewCounter1",
            "contador",
            "+5350000001",
            "12345678902",
            VALID_PASSWORD,
        )
        .unwrap();

        assert_eq!(created.username, "newcounter1");
        assert_eq!(created.role, "contador");
        assert_eq!(created.phone, "+5350000001");
        assert_eq!(created.national_id, "12345678902");
        assert!(!created.is_current_user);
        assert_ne!(created.id, admin_id);

        let (username, password_hash): (String, String) = connection
            .query_row(
                "SELECT username, passwordHash FROM users WHERE id = ?1",
                [created.id],
                |row| Ok((row.get(0)?, row.get(1)?)),
            )
            .unwrap();
        assert_eq!(username, "newcounter1");
        assert_ne!(password_hash, VALID_PASSWORD);
        assert!(bcrypt::verify(VALID_PASSWORD, &password_hash).unwrap());
        assert!(hash_password(VALID_PASSWORD).is_ok());
    }

    #[test]
    fn create_user_accepts_both_supported_roles() {
        for (index, role) in ["administrador", "contador"].into_iter().enumerate() {
            let now = Utc::now();
            let mut connection = setup_test_db();
            create_admin_session(&connection, now);
            let created = create_user(
                &mut connection,
                now,
                &format!("roleuser{index}"),
                role,
                if index == 0 {
                    "+5350000001"
                } else {
                    "+5350000002"
                },
                if index == 0 {
                    "12345678902"
                } else {
                    "12345678903"
                },
                VALID_PASSWORD,
            )
            .unwrap();
            assert_eq!(created.role, role);
        }
    }

    #[test]
    fn create_user_reports_duplicate_phone() {
        let now = Utc::now();
        let mut connection = setup_test_db();
        create_admin_session(&connection, now);
        create_test_user(
            &connection,
            "existinguser",
            "contador",
            "+5350000001",
            "12345678902",
            VALID_PASSWORD,
        );

        assert_error(
            create_user(
                &mut connection,
                now,
                "anotheruser",
                "contador",
                "+5350000001",
                "12345678903",
                VALID_PASSWORD,
            ),
            "duplicateField",
            "El teléfono ya está registrado",
        );
    }

    #[test]
    fn create_user_reports_duplicate_national_id() {
        let now = Utc::now();
        let mut connection = setup_test_db();
        create_admin_session(&connection, now);
        create_test_user(
            &connection,
            "existinguser",
            "contador",
            "+5350000001",
            "12345678902",
            VALID_PASSWORD,
        );

        assert_error(
            create_user(
                &mut connection,
                now,
                "anotheruser",
                "contador",
                "+5350000002",
                "12345678902",
                VALID_PASSWORD,
            ),
            "duplicateField",
            "El carnet de identidad ya está registrado",
        );
    }

    #[test]
    fn create_user_normalizes_username_and_rejects_case_insensitive_duplicates() {
        let now = Utc::now();
        let mut connection = setup_test_db();
        create_admin_session(&connection, now);
        let first = create_user(
            &mut connection,
            now,
            "Contador1",
            "contador",
            "+5350000001",
            "12345678902",
            VALID_PASSWORD,
        )
        .unwrap();
        assert_eq!(first.username, "contador1");

        for (index, duplicate) in ["contador1", "CONTADOR1"].into_iter().enumerate() {
            assert_error(
                create_user(
                    &mut connection,
                    now,
                    duplicate,
                    "contador",
                    if index == 0 {
                        "+5350000002"
                    } else {
                        "+5350000003"
                    },
                    if index == 0 {
                        "12345678903"
                    } else {
                        "12345678904"
                    },
                    VALID_PASSWORD,
                ),
                "duplicateField",
                "El nombre de usuario ya está registrado",
            );
        }
    }

    #[test]
    fn create_user_rejects_invalid_password_and_role() {
        let now = Utc::now();
        let mut connection = setup_test_db();
        create_admin_session(&connection, now);

        assert_error(
            create_user(
                &mut connection,
                now,
                "validuser",
                "contador",
                "+5350000001",
                "12345678902",
                "Valid pass1!",
            ),
            "validationFailed",
            "La contraseña solo puede contener caracteres ASCII imprimibles, sin espacios, acentos ni ñ",
        );
        assert_error(
            create_user(
                &mut connection,
                now,
                "validuser",
                "admin",
                "+5350000001",
                "12345678902",
                VALID_PASSWORD,
            ),
            "validationFailed",
            "El rol no es válido",
        );
    }

    #[test]
    fn create_user_rejects_reserved_phone_and_national_id() {
        let now = Utc::now();
        let mut connection = setup_test_db();
        create_admin_session(&connection, now);

        assert_error(
            create_user(
                &mut connection,
                now,
                "reservedphone",
                "contador",
                "+5300000000",
                "12345678902",
                VALID_PASSWORD,
            ),
            "validationFailed",
            "Ese teléfono está reservado por el sistema. Ingresa un teléfono real",
        );
        assert_error(
            create_user(
                &mut connection,
                now,
                "reservedci",
                "contador",
                "+5350000001",
                "00000000000",
                VALID_PASSWORD,
            ),
            "validationFailed",
            "Ese carnet de identidad está reservado por el sistema. Ingresa uno real",
        );
    }

    #[test]
    fn create_user_requires_admin_session() {
        assert_requires_admin(|connection, now| {
            create_user(
                connection,
                now,
                "newcounter",
                "contador",
                "+5350000001",
                "12345678902",
                VALID_PASSWORD,
            )
        });
    }

    #[test]
    fn list_users_returns_full_dtos_and_marks_only_the_authenticated_user() {
        let now = Utc::now();
        let mut connection = setup_test_db();
        let admin_id = create_admin_session(&connection, now);
        create_test_user(
            &connection,
            "counterone",
            "contador",
            "+5350000001",
            "12345678902",
            VALID_PASSWORD,
        );
        create_test_user(
            &connection,
            "admintwo",
            "administrador",
            "+5350000002",
            "12345678903",
            VALID_PASSWORD,
        );

        let users = list_users_inner(&mut connection, now).unwrap();

        assert_eq!(users.len(), 3);
        assert_eq!(users[0].id, admin_id);
        assert_eq!(users[0].username, ADMIN_USERNAME);
        assert_eq!(users[0].role, "administrador");
        assert_eq!(users[0].phone, ADMIN_PHONE);
        assert_eq!(users[0].national_id, ADMIN_NATIONAL_ID);
        assert!(users[0].is_current_user);
        assert_eq!(users[1].username, "counterone");
        assert_eq!(users[1].role, "contador");
        assert!(!users[1].is_current_user);
        assert_eq!(users[2].username, "admintwo");
        assert!(!users[2].is_current_user);
    }

    #[test]
    fn list_users_orders_results_by_creation_id() {
        let now = Utc::now();
        let mut connection = setup_test_db();
        let admin_id = create_admin_session(&connection, now);
        let second_id = create_test_user(
            &connection,
            "seconduser",
            "contador",
            "+5350000001",
            "12345678902",
            VALID_PASSWORD,
        );
        let third_id = create_test_user(
            &connection,
            "thirduser",
            "contador",
            "+5350000002",
            "12345678903",
            VALID_PASSWORD,
        );

        let users = list_users_inner(&mut connection, now).unwrap();

        assert_eq!(
            users.iter().map(|user| user.id).collect::<Vec<_>>(),
            vec![admin_id, second_id, third_id]
        );
    }

    #[test]
    fn list_users_requires_admin_session() {
        assert_requires_admin(list_users_inner);
    }

    #[test]
    fn get_user_profile_returns_own_data_and_detects_bootstrap_contact_replacement() {
        let now = Utc::now();
        let mut connection = setup_test_db();
        init_schema(&connection, now).unwrap();
        let admin_id: i64 = connection
            .query_row("SELECT id FROM users WHERE username = 'admin'", [], |row| {
                row.get(0)
            })
            .unwrap();
        create_test_session(&connection, admin_id, false, now, now);

        let bootstrap_profile: UserProfile = get_user_profile_inner(&mut connection, now).unwrap();
        assert_eq!(bootstrap_profile.username, "admin");
        assert_eq!(bootstrap_profile.role, "administrador");
        assert_eq!(bootstrap_profile.phone, RESERVED_PHONE);
        assert_eq!(bootstrap_profile.national_id, RESERVED_NATIONAL_ID);
        assert!(bootstrap_profile.has_reserved_contact_data);

        connection
            .execute(
                "UPDATE users SET phone = '+5350000001', nationalId = '12345678901' WHERE id = ?1",
                [admin_id],
            )
            .unwrap();
        let updated_profile = get_user_profile_inner(&mut connection, now).unwrap();
        assert_eq!(updated_profile.phone, "+5350000001");
        assert_eq!(updated_profile.national_id, "12345678901");
        assert!(!updated_profile.has_reserved_contact_data);
    }

    #[test]
    fn get_user_profile_flags_either_reserved_contact_field() {
        for (username, phone, national_id) in [
            ("reservedphone", RESERVED_PHONE, "12345678901"),
            ("reservedci", "+5350000001", RESERVED_NATIONAL_ID),
        ] {
            let now = Utc::now();
            let mut connection = setup_test_db();
            let user_id = create_test_user(
                &connection,
                username,
                "contador",
                phone,
                national_id,
                VALID_PASSWORD,
            );
            create_test_session(&connection, user_id, false, now, now);

            let profile = get_user_profile_inner(&mut connection, now).unwrap();

            assert_eq!(profile.username, username);
            assert!(profile.has_reserved_contact_data);
        }
    }

    #[test]
    fn get_user_profile_requires_a_valid_session() {
        assert_requires_session(get_user_profile_inner);
    }

    #[test]
    fn update_own_profile_allows_unchanged_and_single_field_updates() {
        let now = Utc::now();
        let mut connection = setup_test_db();
        let user_id = create_test_user(
            &connection,
            "profileuser",
            "contador",
            "+5350000001",
            "12345678901",
            VALID_PASSWORD,
        );
        create_test_session(&connection, user_id, false, now, now);

        let unchanged = update_own_profile_inner(
            &mut connection,
            now,
            "profileuser",
            "+5350000001",
            "12345678901",
        )
        .unwrap();
        assert_eq!(unchanged.username, "profileuser");

        let normalized = update_own_profile_inner(
            &mut connection,
            now,
            "PROFILEUSER",
            "+5350000001",
            "12345678901",
        )
        .unwrap();
        assert_eq!(normalized.username, "profileuser");

        let phone_updated = update_own_profile_inner(
            &mut connection,
            now,
            "profileuser",
            "+5350000002",
            "12345678901",
        )
        .unwrap();
        assert_eq!(phone_updated.phone, "+5350000002");
        assert_eq!(phone_updated.national_id, "12345678901");

        let national_id_updated = update_own_profile_inner(
            &mut connection,
            now,
            "profileuser",
            "+5350000002",
            "12345678902",
        )
        .unwrap();
        assert_eq!(national_id_updated.phone, "+5350000002");
        assert_eq!(national_id_updated.national_id, "12345678902");
        assert_eq!(national_id_updated.role, "contador");
    }

    #[test]
    fn update_own_profile_reports_duplicate_username_phone_and_national_id() {
        let now = Utc::now();
        let mut connection = setup_test_db();
        let actor_id = create_test_user(
            &connection,
            "profileuser",
            "contador",
            "+5350000001",
            "12345678901",
            VALID_PASSWORD,
        );
        create_test_user(
            &connection,
            "counterone",
            "contador",
            "+5350000002",
            "12345678902",
            VALID_PASSWORD,
        );
        create_test_session(&connection, actor_id, false, now, now);

        assert_error(
            update_own_profile_inner(
                &mut connection,
                now,
                "COUNTERONE",
                "+5350000001",
                "12345678901",
            ),
            "duplicateField",
            "El nombre de usuario ya está registrado",
        );
        assert_error(
            update_own_profile_inner(
                &mut connection,
                now,
                "profileuser",
                "+5350000002",
                "12345678901",
            ),
            "duplicateField",
            "El teléfono ya está registrado",
        );
        assert_error(
            update_own_profile_inner(
                &mut connection,
                now,
                "profileuser",
                "+5350000001",
                "12345678902",
            ),
            "duplicateField",
            "El carnet de identidad ya está registrado",
        );
    }

    #[test]
    fn update_own_profile_allows_bootstrap_admin_to_keep_reserved_values() {
        let now = Utc::now();
        let mut connection = setup_test_db();
        init_schema(&connection, now).unwrap();
        let admin_id: i64 = connection
            .query_row("SELECT id FROM users WHERE username = 'admin'", [], |row| {
                row.get(0)
            })
            .unwrap();
        create_test_session(&connection, admin_id, false, now, now);

        let profile = update_own_profile_inner(
            &mut connection,
            now,
            "admin2",
            RESERVED_PHONE,
            RESERVED_NATIONAL_ID,
        )
        .unwrap();

        assert_eq!(profile.username, "admin2");
        assert_eq!(profile.phone, RESERVED_PHONE);
        assert_eq!(profile.national_id, RESERVED_NATIONAL_ID);
        assert!(profile.has_reserved_contact_data);
    }

    #[test]
    fn update_own_profile_allows_bootstrap_admin_to_replace_reserved_values() {
        let now = Utc::now();
        let mut connection = setup_test_db();
        init_schema(&connection, now).unwrap();
        let admin_id: i64 = connection
            .query_row("SELECT id FROM users WHERE username = 'admin'", [], |row| {
                row.get(0)
            })
            .unwrap();
        create_test_session(&connection, admin_id, false, now, now);

        let profile =
            update_own_profile_inner(&mut connection, now, "admin", "+5350000001", "12345678901")
                .unwrap();

        assert_eq!(profile.phone, "+5350000001");
        assert_eq!(profile.national_id, "12345678901");
        assert!(!profile.has_reserved_contact_data);
    }

    #[test]
    fn update_own_profile_rejects_assigning_reserved_phone_or_national_id() {
        let now = Utc::now();
        let mut connection = setup_test_db();
        let user_id = create_test_user(
            &connection,
            "profileuser",
            "contador",
            "+5350000001",
            "12345678901",
            VALID_PASSWORD,
        );
        create_test_session(&connection, user_id, false, now, now);

        assert_error(
            update_own_profile_inner(
                &mut connection,
                now,
                "profileuser",
                RESERVED_PHONE,
                "12345678901",
            ),
            "validationFailed",
            "Ese teléfono está reservado por el sistema. Ingresa un teléfono real",
        );
        assert_error(
            update_own_profile_inner(
                &mut connection,
                now,
                "profileuser",
                "+5350000001",
                RESERVED_NATIONAL_ID,
            ),
            "validationFailed",
            "Ese carnet de identidad está reservado por el sistema. Ingresa uno real",
        );
    }

    #[test]
    fn update_own_profile_requires_a_valid_session() {
        assert_requires_session(|connection, now| {
            update_own_profile_inner(connection, now, "matrixuser", "+5350000000", "12345678901")
        });
    }

    #[test]
    fn change_own_password_verifies_current_and_stores_new_bcrypt_hash() {
        let now = Utc::now();
        let mut connection = setup_test_db();
        let user_id = create_test_user(
            &connection,
            "passworduser",
            "contador",
            "+5350000001",
            "12345678901",
            "CurrentPass1!",
        );
        create_test_session(&connection, user_id, false, now, now);

        change_own_password_inner(&mut connection, now, "CurrentPass1!", "NewPassword2!").unwrap();

        let password_hash: String = connection
            .query_row(
                "SELECT passwordHash FROM users WHERE id = ?1",
                [user_id],
                |row| row.get(0),
            )
            .unwrap();
        assert!(bcrypt::verify("NewPassword2!", &password_hash).unwrap());
        assert!(!bcrypt::verify("CurrentPass1!", &password_hash).unwrap());
        assert_ne!(password_hash, "NewPassword2!");
    }

    #[test]
    fn change_own_password_rejects_incorrect_current_password_without_mutation() {
        let now = Utc::now();
        let mut connection = setup_test_db();
        let user_id = create_test_user(
            &connection,
            "passworduser",
            "contador",
            "+5350000001",
            "12345678901",
            "CurrentPass1!",
        );
        create_test_session(&connection, user_id, false, now, now);
        let previous_hash: String = connection
            .query_row(
                "SELECT passwordHash FROM users WHERE id = ?1",
                [user_id],
                |row| row.get(0),
            )
            .unwrap();

        assert_error(
            change_own_password_inner(&mut connection, now, "WrongCurrent1!", "NewPassword2!"),
            "validationFailed",
            "La contraseña actual es incorrecta",
        );

        let stored_hash: String = connection
            .query_row(
                "SELECT passwordHash FROM users WHERE id = ?1",
                [user_id],
                |row| row.get(0),
            )
            .unwrap();
        assert_eq!(stored_hash, previous_hash);
    }

    #[test]
    fn change_own_password_rejects_invalid_new_password_with_requirement_message() {
        let now = Utc::now();
        let mut connection = setup_test_db();
        let user_id = create_test_user(
            &connection,
            "passworduser",
            "contador",
            "+5350000001",
            "12345678901",
            "CurrentPass1!",
        );
        create_test_session(&connection, user_id, false, now, now);

        assert_error(
            change_own_password_inner(&mut connection, now, "CurrentPass1!", "short"),
            "validationFailed",
            "La contraseña debe tener al menos 8 caracteres",
        );
    }

    #[test]
    fn change_own_password_requires_a_valid_session() {
        assert_requires_session(|connection, now| {
            change_own_password_inner(connection, now, "CurrentPass1!", "NewPassword2!")
        });
    }

    #[test]
    fn update_user_changes_another_account_and_preserves_its_password() {
        let now = Utc::now();
        let mut connection = setup_test_db();
        create_admin_session(&connection, now);
        let target_id = create_test_user(
            &connection,
            "counterone",
            "contador",
            "+5350000001",
            "12345678902",
            VALID_PASSWORD,
        );
        let second_admin_id = create_test_user(
            &connection,
            "secondadmin",
            "administrador",
            "+5350000003",
            "12345678904",
            VALID_PASSWORD,
        );
        let password_hash_before: String = connection
            .query_row(
                "SELECT passwordHash FROM users WHERE id = ?1",
                [target_id],
                |row| row.get(0),
            )
            .unwrap();

        let updated = update_user_inner(
            &mut connection,
            now,
            target_id,
            "NewCounter1",
            "administrador",
            "+5350000002",
            "12345678903",
        )
        .unwrap();

        assert_eq!(updated.id, target_id);
        assert_eq!(updated.username, "newcounter1");
        assert_eq!(updated.role, "administrador");
        assert_eq!(updated.phone, "+5350000002");
        assert_eq!(updated.national_id, "12345678903");
        assert!(!updated.is_current_user);
        let password_hash_after: String = connection
            .query_row(
                "SELECT passwordHash FROM users WHERE id = ?1",
                [target_id],
                |row| row.get(0),
            )
            .unwrap();
        assert_eq!(password_hash_after, password_hash_before);

        let demoted = update_user_inner(
            &mut connection,
            now,
            second_admin_id,
            "secondadmin",
            "contador",
            "+5350000003",
            "12345678904",
        )
        .unwrap();
        assert_eq!(demoted.role, "contador");
    }

    #[test]
    fn update_user_rejects_duplicates_with_field_specific_messages() {
        let now = Utc::now();
        let mut connection = setup_test_db();
        create_admin_session(&connection, now);
        let target_id = create_test_user(
            &connection,
            "targetuser",
            "contador",
            "+5350000001",
            "12345678902",
            VALID_PASSWORD,
        );
        create_test_user(
            &connection,
            "counterone",
            "contador",
            "+5350000002",
            "12345678903",
            VALID_PASSWORD,
        );
        create_test_user(
            &connection,
            "countertwo",
            "contador",
            "+5350000003",
            "12345678904",
            VALID_PASSWORD,
        );

        assert_error(
            update_user_inner(
                &mut connection,
                now,
                target_id,
                "COUNTERONE",
                "contador",
                "+5350000001",
                "12345678902",
            ),
            "duplicateField",
            "El nombre de usuario ya está registrado",
        );
        assert_error(
            update_user_inner(
                &mut connection,
                now,
                target_id,
                "targetuser",
                "contador",
                "+5350000002",
                "12345678902",
            ),
            "duplicateField",
            "El teléfono ya está registrado",
        );
        assert_error(
            update_user_inner(
                &mut connection,
                now,
                target_id,
                "targetuser",
                "contador",
                "+5350000001",
                "12345678903",
            ),
            "duplicateField",
            "El carnet de identidad ya está registrado",
        );
    }

    #[test]
    fn update_user_rejects_self_action_before_business_validation() {
        let now = Utc::now();
        let mut connection = setup_test_db();
        let admin_id = create_admin_session(&connection, now);

        assert_error(
            update_user_inner(
                &mut connection,
                now,
                admin_id,
                "invalid username",
                "invalid-role",
                "invalid phone",
                "invalid-id",
            ),
            "selfAction",
            "Para editar tus propios datos usa tu Perfil",
        );
    }

    #[test]
    fn update_user_rejects_missing_target_and_reserved_contacts() {
        let now = Utc::now();
        let mut connection = setup_test_db();
        create_admin_session(&connection, now);
        let target_id = create_test_user(
            &connection,
            "targetuser",
            "contador",
            "+5350000001",
            "12345678902",
            VALID_PASSWORD,
        );

        assert_error(
            update_user_inner(
                &mut connection,
                now,
                9999,
                "otheruser",
                "contador",
                "+5350000003",
                "12345678904",
            ),
            "notFound",
            "El usuario no existe",
        );
        assert_error(
            update_user_inner(
                &mut connection,
                now,
                target_id,
                "targetuser",
                "contador",
                RESERVED_PHONE,
                "12345678902",
            ),
            "validationFailed",
            "Ese teléfono está reservado por el sistema. Ingresa un teléfono real",
        );
        assert_error(
            update_user_inner(
                &mut connection,
                now,
                target_id,
                "targetuser",
                "contador",
                "+5350000001",
                RESERVED_NATIONAL_ID,
            ),
            "validationFailed",
            "Ese carnet de identidad está reservado por el sistema. Ingresa uno real",
        );
    }

    #[test]
    fn update_user_and_reset_password_require_admin() {
        assert_requires_admin(|connection, now| {
            update_user_inner(
                connection,
                now,
                900,
                "targetuser",
                "contador",
                "+5350000001",
                "12345678901",
            )
        });
        assert_requires_admin(|connection, now| {
            reset_user_password_inner(connection, now, 900, VALID_PASSWORD, None)
        });
    }

    #[test]
    fn reset_user_password_supports_admin_reset_and_self_change() {
        let now = Utc::now();
        let mut connection = setup_test_db();
        let admin_id = create_admin_session(&connection, now);
        let target_id = create_test_user(
            &connection,
            "counterone",
            "contador",
            "+5350000001",
            "12345678902",
            VALID_PASSWORD,
        );
        let target_hash_before: String = connection
            .query_row(
                "SELECT passwordHash FROM users WHERE id = ?1",
                [target_id],
                |row| row.get(0),
            )
            .unwrap();

        reset_user_password_inner(&mut connection, now, target_id, "NewPassword2!", None).unwrap();
        let target_hash_after: String = connection
            .query_row(
                "SELECT passwordHash FROM users WHERE id = ?1",
                [target_id],
                |row| row.get(0),
            )
            .unwrap();
        assert_ne!(target_hash_after, target_hash_before);
        assert!(bcrypt::verify("NewPassword2!", &target_hash_after).unwrap());

        reset_user_password_inner(
            &mut connection,
            now,
            admin_id,
            "AdminNewPass2!",
            Some(VALID_PASSWORD),
        )
        .unwrap();
        let admin_hash: String = connection
            .query_row(
                "SELECT passwordHash FROM users WHERE id = ?1",
                [admin_id],
                |row| row.get(0),
            )
            .unwrap();
        assert!(bcrypt::verify("AdminNewPass2!", &admin_hash).unwrap());
    }

    #[test]
    fn reset_user_password_requires_correct_current_password_only_for_self() {
        let now = Utc::now();
        let mut connection = setup_test_db();
        let admin_id = create_admin_session(&connection, now);
        let old_hash: String = connection
            .query_row(
                "SELECT passwordHash FROM users WHERE id = ?1",
                [admin_id],
                |row| row.get(0),
            )
            .unwrap();

        for current_password in [None, Some("WrongCurrent1!")] {
            assert_error(
                reset_user_password_inner(
                    &mut connection,
                    now,
                    admin_id,
                    "AdminNewPass2!",
                    current_password,
                ),
                "validationFailed",
                "La contraseña actual es incorrecta",
            );
        }
        let hash_after_rejections: String = connection
            .query_row(
                "SELECT passwordHash FROM users WHERE id = ?1",
                [admin_id],
                |row| row.get(0),
            )
            .unwrap();
        assert_eq!(hash_after_rejections, old_hash);
    }

    #[test]
    fn reset_user_password_rejects_counter_even_for_own_account_and_missing_targets() {
        let now = Utc::now();
        let mut connection = setup_test_db();
        let counter_id = create_test_user(
            &connection,
            "counteruser",
            "contador",
            "+5350000001",
            "12345678901",
            VALID_PASSWORD,
        );
        let other_id = create_test_user(
            &connection,
            "othercounter",
            "contador",
            "+5350000002",
            "12345678902",
            VALID_PASSWORD,
        );
        create_test_session(&connection, counter_id, false, now, now);

        for target_id in [counter_id, other_id] {
            assert_error(
                reset_user_password_inner(
                    &mut connection,
                    now,
                    target_id,
                    "NewPassword2!",
                    Some(VALID_PASSWORD),
                ),
                "forbidden",
                "No tienes permiso para realizar esta acción",
            );
        }

        let mut connection = setup_test_db();
        create_admin_session(&connection, now);
        assert_error(
            reset_user_password_inner(&mut connection, now, 9999, "NewPassword2!", None),
            "notFound",
            "El usuario no existe",
        );
        assert_error(
            update_user_inner(
                &mut connection,
                now,
                9999,
                "targetuser",
                "contador",
                "+5350000001",
                "12345678901",
            ),
            "notFound",
            "El usuario no existe",
        );
    }

    #[test]
    fn ensure_admin_remains_blocks_demoting_last_admin() {
        let now = Utc::now();
        let connection = setup_test_db();
        let admin_id = create_admin_session(&connection, now);
        let admin = crate::commands::auth::require_admin(&connection, now).unwrap();

        assert_error(
            ensure_admin_remains(&connection, &admin, admin_id, "contador"),
            "lastAdministrator",
            "No es posible: es el último Administrador del sistema",
        );

        create_test_user(
            &connection,
            "secondadmin",
            "administrador",
            "+5350000001",
            "12345678902",
            VALID_PASSWORD,
        );
        assert!(ensure_admin_remains(&connection, &admin, admin_id, "contador").is_ok());
    }

    #[test]
    fn delete_user_removes_non_last_administrator_and_cascades_sessions() {
        let now = Utc::now();
        let mut connection = setup_test_db();
        let admin_id = create_test_user(
            &connection,
            ADMIN_USERNAME,
            "administrador",
            ADMIN_PHONE,
            ADMIN_NATIONAL_ID,
            VALID_PASSWORD,
        );
        let target_id = create_test_user(
            &connection,
            "secondadmin",
            "administrador",
            "+5350000003",
            "12345678904",
            VALID_PASSWORD,
        );
        create_test_user(
            &connection,
            "thirdadmin",
            "administrador",
            "+5350000005",
            "12345678906",
            VALID_PASSWORD,
        );

        // Crear sesión del target para probar cascada
        create_test_session(&connection, target_id, false, now, now);
        let session_exists_before: bool = connection
            .query_row(
                "SELECT EXISTS(SELECT 1 FROM sessions WHERE userId = ?1)",
                [target_id],
                |row| row.get(0),
            )
            .unwrap();
        assert!(session_exists_before);

        // Crear sesión del admin que ejecuta el comando
        connection.execute("DELETE FROM sessions", []).unwrap();
        create_test_session(&connection, admin_id, false, now, now);

        crate::commands::users::delete_user_inner(&mut connection, now, target_id).unwrap();

        let user_exists_after: bool = connection
            .query_row(
                "SELECT EXISTS(SELECT 1 FROM users WHERE id = ?1)",
                [target_id],
                |row| row.get(0),
            )
            .unwrap();
        assert!(!user_exists_after);

        let session_exists_after: bool = connection
            .query_row(
                "SELECT EXISTS(SELECT 1 FROM sessions WHERE userId = ?1)",
                [target_id],
                |row| row.get(0),
            )
            .unwrap();
        assert!(!session_exists_after);

        let remaining_admins: i64 = connection
            .query_row(
                "SELECT COUNT(*) FROM users WHERE role = 'administrador'",
                [],
                |row| row.get(0),
            )
            .unwrap();
        assert_eq!(remaining_admins, 2);
    }

    #[test]
    fn delete_user_rejects_self_action() {
        let now = Utc::now();
        let mut connection = setup_test_db();
        let admin_id = create_admin_session(&connection, now);

        assert_error(
            crate::commands::users::delete_user_inner(&mut connection, now, admin_id),
            "selfAction",
            SELF_DELETE_MESSAGE,
        );
    }

    #[test]
    fn delete_user_requires_admin() {
        assert_requires_admin(|connection, now| {
            crate::commands::users::delete_user_inner(connection, now, 900)
        });
    }

    #[test]
    fn delete_user_returns_not_found_for_nonexistent_user() {
        let now = Utc::now();
        let mut connection = setup_test_db();
        create_admin_session(&connection, now);

        assert_error(
            crate::commands::users::delete_user_inner(&mut connection, now, 9999),
            "notFound",
            "El usuario no existe",
        );
    }
}
