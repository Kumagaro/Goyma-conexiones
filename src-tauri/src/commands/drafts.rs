//! Borradores de formularios (BR-01 a BR-03): uno por usuario, formulario y
//! registro. Cada usuario solo ve y borra los suyos.

use chrono::{DateTime, Duration, Utc};
use rusqlite::{params, Connection, OptionalExtension};
use serde_json::Value;

use crate::commands::auth::require_session;
use crate::db::connection::{current_app_data_dir, lock_conn};
use crate::errors::{log_error, map_db_error, AppError};
use crate::models::draft::{Draft, DraftItem};
use crate::timestamps::format_utc_timestamp;

/// Formularios con borrador: todos salvo los de sesión (BR-01).
pub const DRAFT_FORMS: &[&str] = &[
    "product",
    "category",
    "unit",
    "conversion",
    "client",
    "supplier",
    "entryInvoice",
    "saleInvoice",
    "serviceInvoice",
    "guaranteeInvoice",
    "installationInvoice",
    "lossInvoice",
    "user",
    "profile",
    "company",
    "thresholds",
    "rates",
    "quickClient",
    "quickSupplier",
    "quickProduct",
];

const MAX_PAYLOAD_CHARS: usize = 200_000;
const DRAFT_LIFETIME_DAYS: i64 = 30;

fn validation_failed(message: &str, cause: &str) -> AppError {
    AppError::new("validationFailed", message, cause)
}

fn validate_form_key(form_key: &str) -> Result<(), AppError> {
    if DRAFT_FORMS.contains(&form_key) {
        return Ok(());
    }
    Err(validation_failed(
        "El formulario no admite borradores",
        "unknown draft form",
    ))
}

/// Defensa en profundidad: el frontend nunca envía estos campos (BR-01).
fn has_password_key(value: &Value) -> bool {
    match value {
        Value::Object(fields) => fields
            .iter()
            .any(|(key, field)| key.to_lowercase().contains("password") || has_password_key(field)),
        Value::Array(items) => items.iter().any(has_password_key),
        _ => false,
    }
}

fn validate_payload(payload: &str) -> Result<(), AppError> {
    if payload.chars().count() > MAX_PAYLOAD_CHARS {
        return Err(validation_failed(
            "El borrador es demasiado grande",
            "draft payload too large",
        ));
    }
    let value: Value = serde_json::from_str(payload)
        .map_err(|_| validation_failed("El borrador no es válido", "draft payload is not JSON"))?;
    if has_password_key(&value) {
        return Err(validation_failed(
            "Un borrador no puede guardar contraseñas",
            "draft payload has a password key",
        ));
    }
    Ok(())
}

pub fn save_draft_inner(
    connection: &mut Connection,
    now: DateTime<Utc>,
    form_key: &str,
    record_id: Option<i64>,
    payload: &str,
) -> Result<(), AppError> {
    let actor = require_session(connection, now)?;
    validate_form_key(form_key)?;
    validate_payload(payload)?;
    connection
        .execute(
            "INSERT INTO drafts (userId, formKey, recordId, payload, createdAt, updatedAt)
             VALUES (?1, ?2, ?3, ?4, ?5, ?5)
             ON CONFLICT (userId, formKey, COALESCE(recordId, 0))
             DO UPDATE SET payload = excluded.payload, updatedAt = excluded.updatedAt",
            params![
                actor.id(),
                form_key,
                record_id,
                payload,
                format_utc_timestamp(now),
            ],
        )
        .map_err(|error| map_db_error(error, "saveDraft", &[]))?;
    Ok(())
}

pub fn get_draft_inner(
    connection: &mut Connection,
    now: DateTime<Utc>,
    form_key: &str,
    record_id: Option<i64>,
) -> Result<Option<Draft>, AppError> {
    let actor = require_session(connection, now)?;
    validate_form_key(form_key)?;
    connection
        .query_row(
            "SELECT formKey, recordId, payload, updatedAt FROM drafts
             WHERE userId = ?1 AND formKey = ?2 AND COALESCE(recordId, 0) = COALESCE(?3, 0)",
            params![actor.id(), form_key, record_id],
            |row| {
                Ok(Draft {
                    form_key: row.get(0)?,
                    record_id: row.get(1)?,
                    payload: row.get(2)?,
                    updated_at: row.get(3)?,
                })
            },
        )
        .optional()
        .map_err(|error| map_db_error(error, "getDraft", &[]))
}

pub fn list_drafts_inner(
    connection: &mut Connection,
    now: DateTime<Utc>,
) -> Result<Vec<DraftItem>, AppError> {
    let actor = require_session(connection, now)?;
    let read = || -> rusqlite::Result<Vec<DraftItem>> {
        connection
            .prepare(
                "SELECT formKey, recordId, updatedAt FROM drafts WHERE userId = ?1
                 ORDER BY updatedAt DESC, id DESC",
            )?
            .query_map([actor.id()], |row| {
                Ok(DraftItem {
                    form_key: row.get(0)?,
                    record_id: row.get(1)?,
                    updated_at: row.get(2)?,
                })
            })?
            .collect()
    };
    read().map_err(|error| map_db_error(error, "listDrafts", &[]))
}

pub fn delete_draft_inner(
    connection: &mut Connection,
    now: DateTime<Utc>,
    form_key: &str,
    record_id: Option<i64>,
) -> Result<(), AppError> {
    let actor = require_session(connection, now)?;
    validate_form_key(form_key)?;
    connection
        .execute(
            "DELETE FROM drafts
             WHERE userId = ?1 AND formKey = ?2 AND COALESCE(recordId, 0) = COALESCE(?3, 0)",
            params![actor.id(), form_key, record_id],
        )
        .map_err(|error| map_db_error(error, "deleteDraft", &[]))?;
    Ok(())
}

/// Caducidad de BR-03: se borra lo que lleva 30 días sin cambios.
pub fn purge_old_drafts(connection: &Connection, now: DateTime<Utc>) -> rusqlite::Result<usize> {
    connection.execute(
        "DELETE FROM drafts WHERE updatedAt < ?1",
        [format_utc_timestamp(
            now - Duration::days(DRAFT_LIFETIME_DAYS),
        )],
    )
}

fn logged<T>(
    state: tauri::State<'_, std::sync::Mutex<Connection>>,
    operation: &str,
    run: impl FnOnce(&mut Connection, DateTime<Utc>) -> Result<T, AppError>,
) -> Result<T, AppError> {
    let mut connection = lock_conn(&state);
    let now = Utc::now();
    let result = run(&mut connection, now);
    if let Err(error) = &result {
        if error.should_log() {
            if let Some(app_data_dir) = current_app_data_dir() {
                log_error(&connection, "drafts", operation, error, now, app_data_dir);
            }
        }
    }
    result
}

#[tauri::command]
pub fn save_draft(
    state: tauri::State<'_, std::sync::Mutex<Connection>>,
    form_key: String,
    record_id: Option<i64>,
    payload: String,
) -> Result<(), AppError> {
    logged(state, "saveDraft", |connection, now| {
        save_draft_inner(connection, now, &form_key, record_id, &payload)
    })
}

#[tauri::command]
pub fn get_draft(
    state: tauri::State<'_, std::sync::Mutex<Connection>>,
    form_key: String,
    record_id: Option<i64>,
) -> Result<Option<Draft>, AppError> {
    logged(state, "getDraft", |connection, now| {
        get_draft_inner(connection, now, &form_key, record_id)
    })
}

#[tauri::command]
pub fn list_drafts(
    state: tauri::State<'_, std::sync::Mutex<Connection>>,
) -> Result<Vec<DraftItem>, AppError> {
    logged(state, "listDrafts", list_drafts_inner)
}

#[tauri::command]
pub fn delete_draft(
    state: tauri::State<'_, std::sync::Mutex<Connection>>,
    form_key: String,
    record_id: Option<i64>,
) -> Result<(), AppError> {
    logged(state, "deleteDraft", |connection, now| {
        delete_draft_inner(connection, now, &form_key, record_id)
    })
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::test_utils::{
        assert_allows_both_roles, assert_requires_session, create_test_session, create_test_user,
        setup_test_db,
    };

    fn signed_in(now: DateTime<Utc>) -> (Connection, i64) {
        let connection = setup_test_db();
        let user_id = create_test_user(
            &connection,
            "draftuser",
            "contador",
            "+5350000020",
            "12345678920",
            "ValidPass1!",
        );
        create_test_session(&connection, user_id, false, now, now);
        (connection, user_id)
    }

    #[test]
    fn save_draft_replaces_the_one_of_the_same_form_and_record() {
        let now = Utc::now();
        let (mut connection, _) = signed_in(now);

        save_draft_inner(&mut connection, now, "product", None, r#"{"a":1}"#).unwrap();
        save_draft_inner(&mut connection, now, "product", None, r#"{"a":2}"#).unwrap();
        save_draft_inner(&mut connection, now, "product", Some(7), r#"{"a":3}"#).unwrap();

        let drafts = list_drafts_inner(&mut connection, now).unwrap();
        assert_eq!(drafts.len(), 2);
        let draft = get_draft_inner(&mut connection, now, "product", None)
            .unwrap()
            .unwrap();
        assert_eq!(draft.payload, r#"{"a":2}"#);
        let edit = get_draft_inner(&mut connection, now, "product", Some(7))
            .unwrap()
            .unwrap();
        assert_eq!(edit.payload, r#"{"a":3}"#);
    }

    #[test]
    fn each_user_only_sees_and_deletes_their_own_drafts() {
        let now = Utc::now();
        let (mut connection, _) = signed_in(now);
        let other = create_test_user(
            &connection,
            "otheruser",
            "contador",
            "+5350000021",
            "12345678921",
            "ValidPass1!",
        );
        connection
            .execute(
                "INSERT INTO drafts (userId, formKey, payload, createdAt, updatedAt)
                 VALUES (?1, 'client', '{}', ?2, ?2)",
                params![other, format_utc_timestamp(now)],
            )
            .unwrap();

        assert!(list_drafts_inner(&mut connection, now).unwrap().is_empty());
        assert!(get_draft_inner(&mut connection, now, "client", None)
            .unwrap()
            .is_none());
        delete_draft_inner(&mut connection, now, "client", None).unwrap();
        let remaining: i64 = connection
            .query_row("SELECT COUNT(*) FROM drafts", [], |row| row.get(0))
            .unwrap();
        assert_eq!(remaining, 1);
    }

    #[test]
    fn delete_draft_only_removes_that_record() {
        let now = Utc::now();
        let (mut connection, _) = signed_in(now);
        save_draft_inner(&mut connection, now, "client", None, "{}").unwrap();
        save_draft_inner(&mut connection, now, "client", Some(3), "{}").unwrap();

        delete_draft_inner(&mut connection, now, "client", Some(3)).unwrap();

        let drafts = list_drafts_inner(&mut connection, now).unwrap();
        assert_eq!(drafts.len(), 1);
        assert_eq!(drafts[0].record_id, None);
    }

    #[test]
    fn invalid_drafts_are_rejected() {
        let now = Utc::now();
        let (mut connection, _) = signed_in(now);
        let oversized = format!(r#"{{"a":"{}"}}"#, "x".repeat(MAX_PAYLOAD_CHARS));

        for (form_key, payload) in [
            ("login", "{}"),
            ("product", "no es json"),
            ("user", r#"{"password":"x"}"#),
            ("user", r#"{"lines":[{"newPassword":"x"}]}"#),
            ("profile", r#"{"nested":{"currentPassword":"x"}}"#),
            ("user", r#"{"confirmPassword":""}"#),
            ("product", oversized.as_str()),
        ] {
            let error = save_draft_inner(&mut connection, now, form_key, None, payload);
            assert_eq!(error.unwrap_err().code, "validationFailed", "{form_key}");
        }
        assert!(list_drafts_inner(&mut connection, now).unwrap().is_empty());
    }

    #[test]
    fn drafts_expire_after_thirty_days_without_changes() {
        let now = Utc::now();
        let (mut connection, _) = signed_in(now);
        save_draft_inner(&mut connection, now, "client", None, "{}").unwrap();
        connection
            .execute(
                "UPDATE drafts SET updatedAt = ?1 WHERE formKey = 'client'",
                [format_utc_timestamp(now - Duration::days(31))],
            )
            .unwrap();
        save_draft_inner(&mut connection, now, "supplier", None, "{}").unwrap();

        assert_eq!(purge_old_drafts(&connection, now).unwrap(), 1);
        let drafts = list_drafts_inner(&mut connection, now).unwrap();
        assert_eq!(drafts.len(), 1);
        assert_eq!(drafts[0].form_key, "supplier");
    }

    #[test]
    fn draft_commands_require_session_and_allow_both_roles() {
        assert_requires_session(|connection, now| {
            save_draft_inner(connection, now, "client", None, "{}")
        });
        assert_requires_session(|connection, now| get_draft_inner(connection, now, "client", None));
        assert_requires_session(list_drafts_inner);
        assert_requires_session(|connection, now| {
            delete_draft_inner(connection, now, "client", None)
        });
        assert_allows_both_roles(|connection, now| {
            save_draft_inner(connection, now, "client", None, "{}")
        });
        assert_allows_both_roles(|connection, now| {
            get_draft_inner(connection, now, "client", None)
        });
        assert_allows_both_roles(list_drafts_inner);
        assert_allows_both_roles(|connection, now| {
            delete_draft_inner(connection, now, "client", None)
        });
    }
}
