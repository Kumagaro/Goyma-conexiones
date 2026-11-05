//! Configuración (spec-04 CF-01, CF-02, CF-04): ajustes globales, preferencias
//! por usuario y tasas de cambio con su historial. Ambos roles.

use std::fs;
use std::path::{Path, PathBuf};

use chrono::{DateTime, Utc};
use rusqlite::{params, Connection, OptionalExtension};

use crate::commands::auth::{require_session, Author};
use crate::db::connection::{current_app_data_dir, lock_conn};
use crate::errors::{log_error, map_db_error, AppError};
use crate::models::product::format_factor_micro;
use crate::models::settings::{
    CompanyProfile, ExchangeRate, ExchangeRateChange, Settings, UpdateCompanyProfileInput,
    UpdateSettingsInput, UserPreferences,
};
use crate::timestamps::format_utc_timestamp;
use crate::validators::parse_scaled;

/// Monedas adicionales al USD, en el orden en que se muestran.
pub const CURRENCIES: [&str; 4] = ["EUR", "CUP", "MLC", "SALDO"];
const PAGE_SIZES: [i64; 4] = [10, 20, 50, 100];
const DEFAULT_PAGE_SIZE: i64 = 20;

fn validation_failed(message: &str, cause: &str) -> AppError {
    AppError::new("validationFailed", message, cause)
}

fn read_settings(connection: &Connection) -> Result<Settings, AppError> {
    let thresholds = crate::automation::thresholds(connection);
    let read = |key: &str| -> Result<Option<String>, AppError> {
        connection
            .query_row("SELECT value FROM settings WHERE key = ?1", [key], |row| {
                row.get(0)
            })
            .optional()
            .map_err(|error| map_db_error(error, "getSettings", &[]))
    };
    Ok(Settings {
        low_stock_threshold: thresholds.low_stock_units,
        stale_invoice_days: thresholds.stale_invoice_days,
        backup_reminder_days: read("backupReminderDays")?
            .and_then(|value| value.parse().ok())
            .unwrap_or(7),
        last_backup_at: read("lastBackupAt")?,
    })
}

pub fn get_settings_inner(
    connection: &mut Connection,
    now: DateTime<Utc>,
) -> Result<Settings, AppError> {
    require_session(connection, now)?;
    read_settings(connection)
}

/// CF-04: cambiar un umbral reevalúa las situaciones de NT-03.
pub fn update_settings_inner(
    connection: &mut Connection,
    now: DateTime<Utc>,
    input: UpdateSettingsInput,
) -> Result<Settings, AppError> {
    let actor = require_session(connection, now)?;
    for (value, min, max, message) in [
        (
            input.low_stock_threshold,
            1,
            9_999,
            "El umbral de stock bajo debe ser un número entero de 1 a 9999",
        ),
        (
            input.stale_invoice_days,
            1,
            365,
            "Los días de factura pendiente antigua deben estar entre 1 y 365",
        ),
        (
            input.backup_reminder_days,
            1,
            90,
            "Los días de respaldo pendiente deben estar entre 1 y 90",
        ),
    ] {
        if !(min..=max).contains(&value) {
            return Err(validation_failed(message, "setting out of range"));
        }
    }
    let transaction = connection
        .transaction()
        .map_err(|error| map_db_error(error, "updateSettings", &[]))?;
    let timestamp = format_utc_timestamp(now);
    for (key, value) in [
        ("lowStockThreshold", input.low_stock_threshold),
        ("staleInvoiceDays", input.stale_invoice_days),
        ("backupReminderDays", input.backup_reminder_days),
    ] {
        transaction
            .execute(
                "INSERT INTO settings (key, value, updatedAt) VALUES (?1, ?2, ?3)
                 ON CONFLICT (key) DO UPDATE SET value = excluded.value, updatedAt = excluded.updatedAt",
                params![key, value.to_string(), timestamp],
            )
            .map_err(|error| map_db_error(error, "updateSettings", &[]))?;
    }
    crate::activity::record_activity(
        &transaction,
        &Author::User(&actor),
        now,
        crate::activity::Entry {
            action: "config",
            entity_type: "settings",
            label: "Umbrales",
            details: None,
        },
    )
    .map_err(|error| map_db_error(error, "updateSettings", &[]))?;
    crate::automation::evaluate_watches_best_effort(&transaction, now);
    let settings = read_settings(&transaction)?;
    transaction
        .commit()
        .map_err(|error| map_db_error(error, "updateSettings", &[]))?;
    Ok(settings)
}

pub fn get_user_preferences_inner(
    connection: &mut Connection,
    now: DateTime<Utc>,
) -> Result<UserPreferences, AppError> {
    let actor = require_session(connection, now)?;
    let read = |key: &str| -> Result<Option<String>, AppError> {
        connection
            .query_row(
                "SELECT value FROM userPreferences WHERE userId = ?1 AND key = ?2",
                params![actor.id(), key],
                |row| row.get(0),
            )
            .optional()
            .map_err(|error| map_db_error(error, "getUserPreferences", &[]))
    };
    Ok(UserPreferences {
        visible_currencies: read("visibleCurrencies")?
            .map(|value| {
                value
                    .split(',')
                    .filter(|code| !code.is_empty())
                    .map(str::to_owned)
                    .collect()
            })
            .unwrap_or_default(),
        page_size: read("pageSize")?
            .and_then(|value| value.parse().ok())
            .unwrap_or(DEFAULT_PAGE_SIZE),
    })
}

/// Cada usuario guarda las suyas; las monedas quedan sin repetir y en orden.
pub fn update_user_preferences_inner(
    connection: &mut Connection,
    now: DateTime<Utc>,
    input: UserPreferences,
) -> Result<UserPreferences, AppError> {
    let actor = require_session(connection, now)?;
    if let Some(unknown) = input
        .visible_currencies
        .iter()
        .find(|code| !CURRENCIES.contains(&code.as_str()))
    {
        return Err(validation_failed(
            "La moneda elegida no es válida",
            &format!("unknown currency {unknown}"),
        ));
    }
    if !PAGE_SIZES.contains(&input.page_size) {
        return Err(validation_failed(
            "El tamaño de página debe ser 10, 20, 50 o 100",
            "page size out of range",
        ));
    }
    let currencies: Vec<String> = CURRENCIES
        .iter()
        .filter(|code| {
            input
                .visible_currencies
                .iter()
                .any(|chosen| chosen == *code)
        })
        .map(|code| (*code).to_owned())
        .collect();
    let timestamp = format_utc_timestamp(now);
    let transaction = connection
        .transaction()
        .map_err(|error| map_db_error(error, "updateUserPreferences", &[]))?;
    for (key, value) in [
        ("visibleCurrencies", currencies.join(",")),
        ("pageSize", input.page_size.to_string()),
    ] {
        transaction
            .execute(
                "INSERT INTO userPreferences (userId, key, value, updatedAt) VALUES (?1, ?2, ?3, ?4)
                 ON CONFLICT (userId, key) DO UPDATE SET value = excluded.value, updatedAt = excluded.updatedAt",
                params![actor.id(), key, value, timestamp],
            )
            .map_err(|error| map_db_error(error, "updateUserPreferences", &[]))?;
    }
    transaction
        .commit()
        .map_err(|error| map_db_error(error, "updateUserPreferences", &[]))?;
    Ok(UserPreferences {
        visible_currencies: currencies,
        page_size: input.page_size,
    })
}

fn currency_order(currency: &str) -> usize {
    CURRENCIES
        .iter()
        .position(|code| *code == currency)
        .unwrap_or(CURRENCIES.len())
}

pub fn list_exchange_rates_inner(
    connection: &mut Connection,
    now: DateTime<Utc>,
) -> Result<Vec<ExchangeRate>, AppError> {
    require_session(connection, now)?;
    let mut rates: Vec<ExchangeRate> = connection
        .prepare("SELECT currency, rateMicro, updatedAt, updatedByName FROM exchangeRates")
        .and_then(|mut statement| {
            statement
                .query_map([], |row| {
                    Ok(ExchangeRate {
                        currency: row.get(0)?,
                        rate: format_factor_micro(row.get(1)?),
                        updated_at: row.get(2)?,
                        updated_by_name: row.get(3)?,
                    })
                })?
                .collect()
        })
        .map_err(|error| map_db_error(error, "listExchangeRates", &[]))?;
    rates.sort_by_key(|rate| currency_order(&rate.currency));
    Ok(rates)
}

/// CF-02: 1 USD = `rate` de la moneda, hasta 6 decimales y mayor que 0. Cada
/// cambio queda en el historial con la tasa anterior y el usuario.
pub fn update_exchange_rate_inner(
    connection: &mut Connection,
    now: DateTime<Utc>,
    currency: &str,
    rate: &str,
) -> Result<ExchangeRate, AppError> {
    let actor = require_session(connection, now)?;
    if !CURRENCIES.contains(&currency) {
        return Err(validation_failed(
            "La moneda elegida no es válida",
            "unknown currency for exchange rate",
        ));
    }
    let rate_micro = parse_scaled(rate, 9, 6, 1, 999_999_999_999_999).map_err(|_| {
        validation_failed(
            "La tasa debe ser un número mayor que 0 con hasta 6 decimales",
            "invalid exchange rate",
        )
    })? as i64;
    let timestamp = format_utc_timestamp(now);
    let transaction = connection
        .transaction()
        .map_err(|error| map_db_error(error, "updateExchangeRate", &[]))?;
    let previous: Option<i64> = transaction
        .query_row(
            "SELECT rateMicro FROM exchangeRates WHERE currency = ?1",
            [currency],
            |row| row.get(0),
        )
        .optional()
        .map_err(|error| map_db_error(error, "updateExchangeRate", &[]))?;
    transaction
        .execute(
            "INSERT INTO exchangeRates (currency, rateMicro, updatedAt, updatedByName) VALUES (?1, ?2, ?3, ?4)
             ON CONFLICT (currency) DO UPDATE SET rateMicro = excluded.rateMicro,
                updatedAt = excluded.updatedAt, updatedByName = excluded.updatedByName",
            params![currency, rate_micro, timestamp, actor.username()],
        )
        .map_err(|error| map_db_error(error, "updateExchangeRate", &[]))?;
    transaction
        .execute(
            "INSERT INTO exchangeRateHistory (createdAt, currency, previousRateMicro, newRateMicro, userId, userName)
             VALUES (?1, ?2, ?3, ?4, ?5, ?6)",
            params![timestamp, currency, previous, rate_micro, actor.id(), actor.username()],
        )
        .map_err(|error| map_db_error(error, "updateExchangeRate", &[]))?;
    let change = match previous {
        Some(previous) => format!(
            "{} → {}",
            format_factor_micro(previous),
            format_factor_micro(rate_micro)
        ),
        None => format_factor_micro(rate_micro),
    };
    crate::activity::record_activity(
        &transaction,
        &Author::User(&actor),
        now,
        crate::activity::Entry {
            action: "config",
            entity_type: "exchangeRate",
            label: currency,
            details: Some(&change),
        },
    )
    .map_err(|error| map_db_error(error, "updateExchangeRate", &[]))?;
    transaction
        .commit()
        .map_err(|error| map_db_error(error, "updateExchangeRate", &[]))?;
    Ok(ExchangeRate {
        currency: currency.to_owned(),
        rate: format_factor_micro(rate_micro),
        updated_at: timestamp,
        updated_by_name: actor.username().to_owned(),
    })
}

/// Historial de tasas, del más reciente al más antiguo.
pub fn list_exchange_rate_history_inner(
    connection: &mut Connection,
    now: DateTime<Utc>,
) -> Result<Vec<ExchangeRateChange>, AppError> {
    require_session(connection, now)?;
    connection
        .prepare(
            "SELECT createdAt, currency, previousRateMicro, newRateMicro, userName
             FROM exchangeRateHistory ORDER BY createdAt DESC, id DESC",
        )
        .and_then(|mut statement| {
            statement
                .query_map([], |row| {
                    Ok(ExchangeRateChange {
                        created_at: row.get(0)?,
                        currency: row.get(1)?,
                        previous_rate: row.get::<_, Option<i64>>(2)?.map(format_factor_micro),
                        new_rate: format_factor_micro(row.get(3)?),
                        user_name: row.get(4)?,
                    })
                })?
                .collect()
        })
        .map_err(|error| map_db_error(error, "listExchangeRateHistory", &[]))
}

// ---------- Empresa y logo (CF-03, D-07, D-17) ----------

const LOGO_FILE: &str = "logo.jpg";
const MAX_LOGO_BYTES: usize = 2 * 1024 * 1024;
const INVALID_LOGO: &str = "El logo debe ser una imagen PNG o JPG válida";

pub(crate) fn company_dir(app_data_dir: &Path) -> PathBuf {
    app_data_dir.join("company")
}

pub(crate) fn read_company_profile(
    connection: &Connection,
    app_data_dir: &Path,
) -> Result<CompanyProfile, AppError> {
    let read = |key: &str| -> Result<String, AppError> {
        Ok(connection
            .query_row("SELECT value FROM settings WHERE key = ?1", [key], |row| {
                row.get(0)
            })
            .optional()
            .map_err(|error| map_db_error(error, "getCompanyProfile", &[]))?
            .unwrap_or_default())
    };
    let logo_file = read("companyLogoFile")?;
    // Un logo que falta en disco se trata como "sin logo".
    let logo_data_url = (!logo_file.is_empty())
        .then(|| fs::read(company_dir(app_data_dir).join(&logo_file)).ok())
        .flatten()
        .map(|bytes| format!("data:image/jpeg;base64,{}", base64_encode(&bytes)));
    Ok(CompanyProfile {
        name: read("companyName")?,
        address: read("companyAddress")?,
        phone: read("companyPhone")?,
        tax_id: read("companyTaxId")?,
        logo_data_url,
    })
}

pub fn get_company_profile_inner(
    connection: &mut Connection,
    now: DateTime<Utc>,
    app_data_dir: &Path,
) -> Result<CompanyProfile, AppError> {
    require_session(connection, now)?;
    read_company_profile(connection, app_data_dir)
}

/// CF-03: los cuatro datos son opcionales; si faltan, el encabezado se compone
/// con lo que haya.
pub fn update_company_profile_inner(
    connection: &mut Connection,
    now: DateTime<Utc>,
    app_data_dir: &Path,
    input: UpdateCompanyProfileInput,
) -> Result<CompanyProfile, AppError> {
    let actor = require_session(connection, now)?;
    let fields = [
        (
            "companyName",
            input.name.trim(),
            100,
            "El nombre de la empresa admite hasta 100 caracteres",
        ),
        (
            "companyAddress",
            input.address.trim(),
            200,
            "La dirección admite hasta 200 caracteres",
        ),
        (
            "companyPhone",
            input.phone.trim(),
            30,
            "El teléfono admite hasta 30 caracteres",
        ),
        (
            "companyTaxId",
            input.tax_id.trim(),
            30,
            "El NIT o código fiscal admite hasta 30 caracteres",
        ),
    ];
    if let Some((_, _, _, message)) = fields
        .iter()
        .find(|(_, value, max, _)| value.chars().count() > *max)
    {
        return Err(validation_failed(message, "company field too long"));
    }
    let timestamp = format_utc_timestamp(now);
    let transaction = connection
        .transaction()
        .map_err(|error| map_db_error(error, "updateCompanyProfile", &[]))?;
    for (key, value, _, _) in fields {
        transaction
            .execute(
                "INSERT INTO settings (key, value, updatedAt) VALUES (?1, ?2, ?3)
                 ON CONFLICT (key) DO UPDATE SET value = excluded.value, updatedAt = excluded.updatedAt",
                params![key, value, timestamp],
            )
            .map_err(|error| map_db_error(error, "updateCompanyProfile", &[]))?;
    }
    crate::activity::record_activity(
        &transaction,
        &Author::User(&actor),
        now,
        crate::activity::Entry {
            action: "config",
            entity_type: "settings",
            label: "Datos de la empresa",
            details: None,
        },
    )
    .map_err(|error| map_db_error(error, "updateCompanyProfile", &[]))?;
    let profile = read_company_profile(&transaction, app_data_dir)?;
    transaction
        .commit()
        .map_err(|error| map_db_error(error, "updateCompanyProfile", &[]))?;
    Ok(profile)
}

/// D-17: el frontend convierte el PNG o JPG elegido a JPEG; aquí se valida
/// por su contenido (firma y marcador SOF con medidas y 1 o 3 componentes).
fn validate_jpeg(bytes: &[u8]) -> Result<(), AppError> {
    if bytes.len() > MAX_LOGO_BYTES {
        return Err(validation_failed(
            "El logo no puede superar los 2 MB",
            "logo too large",
        ));
    }
    jpeg_dimensions(bytes)
        .map(|_| ())
        .ok_or_else(|| validation_failed(INVALID_LOGO, "logo is not a valid jpeg"))
}

/// Ancho, alto y componentes (1 gris o 3 color) de un JPEG, leídos de su
/// marcador SOF; `None` si no es un JPEG que el PDF pueda incrustar (D-17).
pub(crate) fn jpeg_dimensions(bytes: &[u8]) -> Option<(u16, u16, u8)> {
    if !bytes.starts_with(&[0xFF, 0xD8]) {
        return None;
    }
    let mut at = 2;
    while at + 4 <= bytes.len() {
        if bytes[at] != 0xFF {
            return None;
        }
        let marker = bytes[at + 1];
        let length = usize::from(u16::from_be_bytes([bytes[at + 2], bytes[at + 3]]));
        // SOF0 a SOF15, salvo DHT (C4), JPG (C8) y DAC (CC).
        if (0xC0..=0xCF).contains(&marker) && ![0xC4, 0xC8, 0xCC].contains(&marker) {
            let header = bytes.get(at + 4..at + 10)?;
            let height = u16::from_be_bytes([header[1], header[2]]);
            let width = u16::from_be_bytes([header[3], header[4]]);
            return (height > 0 && width > 0 && matches!(header[5], 1 | 3))
                .then_some((width, height, header[5]));
        }
        if length < 2 {
            return None;
        }
        at += 2 + length;
    }
    None
}

/// Guarda el logo (sustituye y borra el anterior) o lo quita con `None`.
pub fn save_company_logo_inner(
    connection: &mut Connection,
    now: DateTime<Utc>,
    app_data_dir: &Path,
    jpeg: Option<Vec<u8>>,
) -> Result<CompanyProfile, AppError> {
    let actor = require_session(connection, now)?;
    if let Some(bytes) = &jpeg {
        validate_jpeg(bytes)?;
    }
    let directory = company_dir(app_data_dir);
    let path = directory.join(LOGO_FILE);
    let failed = |cause: String| AppError::new("internal", "No se pudo guardar el logo", &cause);
    match &jpeg {
        Some(bytes) => {
            fs::create_dir_all(&directory)
                .map_err(|error| failed(format!("create company dir: {error}")))?;
            // Se escribe aparte y se renombra: un fallo no deja un logo a medias.
            let temporary = directory.join("logo.tmp");
            fs::write(&temporary, bytes)
                .and_then(|()| fs::rename(&temporary, &path))
                .map_err(|error| failed(format!("write logo: {error}")))?;
        }
        None => {
            if path.exists() {
                fs::remove_file(&path).map_err(|error| failed(format!("remove logo: {error}")))?;
            }
        }
    }
    let value = if jpeg.is_some() { LOGO_FILE } else { "" };
    let transaction = connection
        .transaction()
        .map_err(|error| map_db_error(error, "saveCompanyLogo", &[]))?;
    transaction
        .execute(
            "INSERT INTO settings (key, value, updatedAt) VALUES ('companyLogoFile', ?1, ?2)
             ON CONFLICT (key) DO UPDATE SET value = excluded.value, updatedAt = excluded.updatedAt",
            params![value, format_utc_timestamp(now)],
        )
        .map_err(|error| map_db_error(error, "saveCompanyLogo", &[]))?;
    crate::activity::record_activity(
        &transaction,
        &Author::User(&actor),
        now,
        crate::activity::Entry {
            action: "config",
            entity_type: "settings",
            label: "Logo de la empresa",
            details: None,
        },
    )
    .map_err(|error| map_db_error(error, "saveCompanyLogo", &[]))?;
    transaction
        .commit()
        .map_err(|error| map_db_error(error, "saveCompanyLogo", &[]))?;
    read_company_profile(connection, app_data_dir)
}

/// Base64 estándar con relleno, para la `data:` URL del logo.
fn base64_encode(bytes: &[u8]) -> String {
    const ALPHABET: &[u8; 64] = b"ABCDEFGHIJKLMNOPQRSTUVWXYZabcdefghijklmnopqrstuvwxyz0123456789+/";
    let mut out = String::with_capacity(bytes.len().div_ceil(3) * 4);
    for chunk in bytes.chunks(3) {
        let value = (u32::from(chunk[0]) << 16)
            | (u32::from(*chunk.get(1).unwrap_or(&0)) << 8)
            | u32::from(*chunk.get(2).unwrap_or(&0));
        for index in 0..4 {
            if index <= chunk.len() {
                out.push(char::from(
                    ALPHABET[(value >> (18 - 6 * index)) as usize & 63],
                ));
            } else {
                out.push('=');
            }
        }
    }
    out
}

pub(crate) fn require_app_data_dir() -> Result<&'static Path, AppError> {
    current_app_data_dir().ok_or_else(|| {
        AppError::new(
            "internal",
            "Ocurrió un error inesperado",
            "app data dir is not initialized",
        )
    })
}

#[tauri::command]
pub fn get_company_profile(
    state: tauri::State<'_, std::sync::Mutex<Connection>>,
) -> Result<CompanyProfile, AppError> {
    let mut connection = lock_conn(&state);
    let now = Utc::now();
    let result =
        require_app_data_dir().and_then(|dir| get_company_profile_inner(&mut connection, now, dir));
    log_command_error(&connection, "getCompanyProfile", &result, now);
    result
}

#[tauri::command]
pub fn update_company_profile(
    state: tauri::State<'_, std::sync::Mutex<Connection>>,
    input: UpdateCompanyProfileInput,
) -> Result<CompanyProfile, AppError> {
    let mut connection = lock_conn(&state);
    let now = Utc::now();
    let result = require_app_data_dir()
        .and_then(|dir| update_company_profile_inner(&mut connection, now, dir, input));
    log_command_error(&connection, "updateCompanyProfile", &result, now);
    result
}

#[tauri::command]
pub fn save_company_logo(
    state: tauri::State<'_, std::sync::Mutex<Connection>>,
    jpeg: Option<Vec<u8>>,
) -> Result<CompanyProfile, AppError> {
    let mut connection = lock_conn(&state);
    let now = Utc::now();
    let result = require_app_data_dir()
        .and_then(|dir| save_company_logo_inner(&mut connection, now, dir, jpeg));
    log_command_error(&connection, "saveCompanyLogo", &result, now);
    result
}

fn log_command_error<T>(
    connection: &Connection,
    operation: &str,
    result: &Result<T, AppError>,
    now: DateTime<Utc>,
) {
    if let Err(error) = result {
        if error.should_log() {
            if let Some(app_data_dir) = current_app_data_dir() {
                log_error(connection, "settings", operation, error, now, app_data_dir);
            }
        }
    }
}

#[tauri::command]
pub fn get_settings(
    state: tauri::State<'_, std::sync::Mutex<Connection>>,
) -> Result<Settings, AppError> {
    let mut connection = lock_conn(&state);
    let now = Utc::now();
    let result = get_settings_inner(&mut connection, now);
    log_command_error(&connection, "getSettings", &result, now);
    result
}

#[tauri::command]
pub fn update_settings(
    state: tauri::State<'_, std::sync::Mutex<Connection>>,
    input: UpdateSettingsInput,
) -> Result<Settings, AppError> {
    let mut connection = lock_conn(&state);
    let now = Utc::now();
    let result = update_settings_inner(&mut connection, now, input);
    log_command_error(&connection, "updateSettings", &result, now);
    result
}

#[tauri::command]
pub fn get_user_preferences(
    state: tauri::State<'_, std::sync::Mutex<Connection>>,
) -> Result<UserPreferences, AppError> {
    let mut connection = lock_conn(&state);
    let now = Utc::now();
    let result = get_user_preferences_inner(&mut connection, now);
    log_command_error(&connection, "getUserPreferences", &result, now);
    result
}

#[tauri::command]
pub fn update_user_preferences(
    state: tauri::State<'_, std::sync::Mutex<Connection>>,
    input: UserPreferences,
) -> Result<UserPreferences, AppError> {
    let mut connection = lock_conn(&state);
    let now = Utc::now();
    let result = update_user_preferences_inner(&mut connection, now, input);
    log_command_error(&connection, "updateUserPreferences", &result, now);
    result
}

#[tauri::command]
pub fn list_exchange_rates(
    state: tauri::State<'_, std::sync::Mutex<Connection>>,
) -> Result<Vec<ExchangeRate>, AppError> {
    let mut connection = lock_conn(&state);
    let now = Utc::now();
    let result = list_exchange_rates_inner(&mut connection, now);
    log_command_error(&connection, "listExchangeRates", &result, now);
    result
}

#[tauri::command]
pub fn update_exchange_rate(
    state: tauri::State<'_, std::sync::Mutex<Connection>>,
    currency: String,
    rate: String,
) -> Result<ExchangeRate, AppError> {
    let mut connection = lock_conn(&state);
    let now = Utc::now();
    let result = update_exchange_rate_inner(&mut connection, now, &currency, &rate);
    log_command_error(&connection, "updateExchangeRate", &result, now);
    result
}

#[tauri::command]
pub fn list_exchange_rate_history(
    state: tauri::State<'_, std::sync::Mutex<Connection>>,
) -> Result<Vec<ExchangeRateChange>, AppError> {
    let mut connection = lock_conn(&state);
    let now = Utc::now();
    let result = list_exchange_rate_history_inner(&mut connection, now);
    log_command_error(&connection, "listExchangeRateHistory", &result, now);
    result
}

#[cfg(test)]
mod tests {
    use chrono::{DateTime, Duration, TimeZone, Utc};
    use rusqlite::Connection;

    use super::{
        get_settings_inner, get_user_preferences_inner, list_exchange_rate_history_inner,
        list_exchange_rates_inner, update_exchange_rate_inner, update_settings_inner,
        update_user_preferences_inner,
    };
    use crate::models::settings::{UpdateSettingsInput, UserPreferences};
    use crate::test_utils::{
        assert_allows_both_roles, assert_requires_session, create_test_session, create_test_user,
        insert_product_fixture, setup_test_db,
    };

    fn now() -> DateTime<Utc> {
        Utc.with_ymd_and_hms(2026, 11, 4, 16, 0, 0).unwrap()
    }

    fn login(connection: &Connection, username: &str, phone: &str, national_id: &str) -> i64 {
        let id = create_test_user(
            connection,
            username,
            "contador",
            phone,
            national_id,
            "ValidPass1!",
        );
        connection.execute("DELETE FROM sessions", []).unwrap();
        create_test_session(connection, id, false, now(), now());
        id
    }

    fn settings(low: i64, stale: i64, backup: i64) -> UpdateSettingsInput {
        UpdateSettingsInput {
            low_stock_threshold: low,
            stale_invoice_days: stale,
            backup_reminder_days: backup,
        }
    }

    #[test]
    fn settings_have_defaults_validate_ranges_and_a_new_threshold_reevaluates_low_stock() {
        let mut connection = setup_test_db();
        login(&connection, "ajustes", "+5350000051", "12345678951");
        let product = insert_product_fixture(
            &connection,
            "Café",
            None,
            None,
            100,
            0,
            7_000,
            "active",
            None,
            None,
        );

        let defaults = get_settings_inner(&mut connection, now()).unwrap();
        assert_eq!(
            (
                defaults.low_stock_threshold,
                defaults.stale_invoice_days,
                defaults.backup_reminder_days,
                defaults.last_backup_at
            ),
            (5, 30, 7, None)
        );

        for (input, message) in [
            (
                settings(0, 30, 7),
                "El umbral de stock bajo debe ser un número entero de 1 a 9999",
            ),
            (
                settings(10_000, 30, 7),
                "El umbral de stock bajo debe ser un número entero de 1 a 9999",
            ),
            (
                settings(5, 366, 7),
                "Los días de factura pendiente antigua deben estar entre 1 y 365",
            ),
            (
                settings(5, 30, 91),
                "Los días de respaldo pendiente deben estar entre 1 y 90",
            ),
        ] {
            let error = update_settings_inner(&mut connection, now(), input).unwrap_err();
            assert_eq!(
                (error.code.as_str(), error.message.as_str()),
                ("validationFailed", message)
            );
        }

        // 7 unidades no es stock bajo con 5; con 10 sí, y avisa (N-03).
        let saved = update_settings_inner(&mut connection, now(), settings(10, 45, 14)).unwrap();
        assert_eq!(
            (
                saved.low_stock_threshold,
                saved.stale_invoice_days,
                saved.backup_reminder_days
            ),
            (10, 45, 14)
        );
        let message: String = connection
            .query_row(
                "SELECT message FROM notifications WHERE type = 'N-03'",
                [],
                |row| row.get(0),
            )
            .unwrap();
        assert_eq!(
            message,
            "Producto con stock bajo: Café - Cantidad actual: 7"
        );
        let detail =
            crate::commands::products::get_product_inner(&mut connection, now(), product).unwrap();
        assert!(detail.product.is_low_stock);
        assert_eq!(
            crate::automation::thresholds(&connection).stale_invoice_days,
            45
        );
    }

    #[test]
    fn preferences_are_per_user_validated_and_stored_in_currency_order() {
        let mut connection = setup_test_db();
        login(&connection, "primero", "+5350000052", "12345678952");
        let defaults = get_user_preferences_inner(&mut connection, now()).unwrap();
        assert_eq!(
            defaults,
            UserPreferences {
                visible_currencies: vec![],
                page_size: 20
            }
        );

        let saved = update_user_preferences_inner(
            &mut connection,
            now(),
            UserPreferences {
                visible_currencies: vec!["SALDO".into(), "EUR".into(), "EUR".into()],
                page_size: 50,
            },
        )
        .unwrap();
        assert_eq!(saved.visible_currencies, vec!["EUR", "SALDO"]);
        assert_eq!(
            get_user_preferences_inner(&mut connection, now()).unwrap(),
            saved
        );

        for (currencies, size, message) in [
            (vec!["GBP".to_owned()], 20, "La moneda elegida no es válida"),
            (vec![], 25, "El tamaño de página debe ser 10, 20, 50 o 100"),
        ] {
            let error = update_user_preferences_inner(
                &mut connection,
                now(),
                UserPreferences {
                    visible_currencies: currencies,
                    page_size: size,
                },
            )
            .unwrap_err();
            assert_eq!(error.message, message);
        }

        // Otro usuario ve sus propios valores por defecto.
        login(&connection, "segundo", "+5350000053", "12345678953");
        assert_eq!(
            get_user_preferences_inner(&mut connection, now())
                .unwrap()
                .page_size,
            20
        );
    }

    #[test]
    fn exchange_rates_keep_six_decimals_and_record_every_change_in_the_history() {
        let mut connection = setup_test_db();
        login(&connection, "tasas", "+5350000054", "12345678954");
        assert!(list_exchange_rates_inner(&mut connection, now())
            .unwrap()
            .is_empty());

        update_exchange_rate_inner(&mut connection, now(), "CUP", "670").unwrap();
        update_exchange_rate_inner(
            &mut connection,
            now() + Duration::minutes(1),
            "EUR",
            "0,925501",
        )
        .unwrap();
        let updated = update_exchange_rate_inner(
            &mut connection,
            now() + Duration::minutes(2),
            "CUP",
            "675,5",
        )
        .unwrap();
        assert_eq!(
            (updated.rate.as_str(), updated.updated_by_name.as_str()),
            ("675,5", "tasas")
        );

        let rates = list_exchange_rates_inner(&mut connection, now()).unwrap();
        assert_eq!(
            rates
                .iter()
                .map(|rate| (rate.currency.as_str(), rate.rate.as_str()))
                .collect::<Vec<_>>(),
            vec![("EUR", "0,925501"), ("CUP", "675,5")]
        );
        let history = list_exchange_rate_history_inner(&mut connection, now()).unwrap();
        assert_eq!(
            history
                .iter()
                .map(|change| (
                    change.currency.as_str(),
                    change.previous_rate.as_deref(),
                    change.new_rate.as_str()
                ))
                .collect::<Vec<_>>(),
            vec![
                ("CUP", Some("670"), "675,5"),
                ("EUR", None, "0,925501"),
                ("CUP", None, "670")
            ]
        );

        for (currency, rate, message) in [
            (
                "CUP",
                "0",
                "La tasa debe ser un número mayor que 0 con hasta 6 decimales",
            ),
            (
                "CUP",
                "1,1234567",
                "La tasa debe ser un número mayor que 0 con hasta 6 decimales",
            ),
            (
                "CUP",
                "abc",
                "La tasa debe ser un número mayor que 0 con hasta 6 decimales",
            ),
            ("GBP", "1", "La moneda elegida no es válida"),
        ] {
            let error =
                update_exchange_rate_inner(&mut connection, now(), currency, rate).unwrap_err();
            assert_eq!(error.message, message, "{currency} {rate}");
        }
        assert_eq!(
            list_exchange_rate_history_inner(&mut connection, now())
                .unwrap()
                .len(),
            3
        );
    }

    // Matriz de autorización (`Agents.md` §3): sesión obligatoria, ambos roles.
    #[test]
    fn settings_commands_require_a_session_and_allow_both_roles() {
        let prefs = || UserPreferences {
            visible_currencies: vec![],
            page_size: 20,
        };
        assert_requires_session(get_settings_inner);
        assert_allows_both_roles(get_settings_inner);
        assert_requires_session(|c, n| update_settings_inner(c, n, settings(5, 30, 7)));
        assert_allows_both_roles(|c, n| update_settings_inner(c, n, settings(5, 30, 7)));
        assert_requires_session(get_user_preferences_inner);
        assert_allows_both_roles(get_user_preferences_inner);
        assert_requires_session(|c, n| update_user_preferences_inner(c, n, prefs()));
        assert_allows_both_roles(|c, n| update_user_preferences_inner(c, n, prefs()));
        assert_requires_session(list_exchange_rates_inner);
        assert_allows_both_roles(list_exchange_rates_inner);
        assert_requires_session(|c, n| update_exchange_rate_inner(c, n, "CUP", "670"));
        assert_allows_both_roles(|c, n| update_exchange_rate_inner(c, n, "CUP", "670"));
        assert_requires_session(list_exchange_rate_history_inner);
        assert_allows_both_roles(list_exchange_rate_history_inner);
    }

    // ---------- Empresa y logo (CF-03) ----------

    static NEXT_DIR: std::sync::atomic::AtomicU64 = std::sync::atomic::AtomicU64::new(0);

    fn temp_dir() -> std::path::PathBuf {
        let id = NEXT_DIR.fetch_add(1, std::sync::atomic::Ordering::SeqCst);
        let dir = std::env::temp_dir().join(format!("goyma-company-{}-{id}", std::process::id()));
        let _ = std::fs::remove_dir_all(&dir);
        std::fs::create_dir_all(&dir).unwrap();
        dir
    }

    /// JPEG mínimo: SOI, APP0, SOF0 (2×3 px, `components`) y EOI.
    fn jpeg(components: u8) -> Vec<u8> {
        let mut bytes = vec![0xFF, 0xD8, 0xFF, 0xE0, 0x00, 0x10];
        bytes.extend_from_slice(b"JFIF\0\x01\x01\0\0\x01\0\x01\0\0");
        bytes.extend_from_slice(&[
            0xFF, 0xC0, 0x00, 0x11, 0x08, 0x00, 0x02, 0x00, 0x03, components,
        ]);
        bytes.extend_from_slice(&[0; 9]);
        bytes.extend_from_slice(&[0xFF, 0xD9]);
        bytes
    }

    #[test]
    fn base64_matches_the_standard_vectors() {
        assert_eq!(super::base64_encode(b"Man"), "TWFu");
        assert_eq!(super::base64_encode(b"Ma"), "TWE=");
        assert_eq!(super::base64_encode(b"M"), "TQ==");
        assert_eq!(super::base64_encode(&[0xFF, 0xD8, 0xFF]), "/9j/");
    }

    #[test]
    fn company_profile_saves_trimmed_fields_and_rejects_long_ones() {
        let mut connection = setup_test_db();
        let dir = temp_dir();
        login(&connection, "empresa", "+5350000055", "12345678955");
        let empty = super::get_company_profile_inner(&mut connection, now(), &dir).unwrap();
        assert_eq!((empty.name.as_str(), empty.logo_data_url), ("", None));

        let saved = super::update_company_profile_inner(
            &mut connection,
            now(),
            &dir,
            crate::models::settings::UpdateCompanyProfileInput {
                name: "  Goyma Conexiones ".into(),
                address: "Calle 23, Vedado".into(),
                phone: "+53 7 8300000".into(),
                tax_id: "".into(),
            },
        )
        .unwrap();
        assert_eq!(
            (
                saved.name.as_str(),
                saved.address.as_str(),
                saved.phone.as_str(),
                saved.tax_id.as_str()
            ),
            ("Goyma Conexiones", "Calle 23, Vedado", "+53 7 8300000", "")
        );
        let error = super::update_company_profile_inner(
            &mut connection,
            now(),
            &dir,
            crate::models::settings::UpdateCompanyProfileInput {
                name: "x".repeat(101),
                address: String::new(),
                phone: String::new(),
                tax_id: String::new(),
            },
        )
        .unwrap_err();
        assert_eq!(
            error.message,
            "El nombre de la empresa admite hasta 100 caracteres"
        );
    }

    #[test]
    fn the_logo_is_validated_by_content_replaced_and_removed() {
        let mut connection = setup_test_db();
        let dir = temp_dir();
        login(&connection, "logo", "+5350000056", "12345678956");
        let file = dir.join("company").join("logo.jpg");

        let first = jpeg(3);
        let profile =
            super::save_company_logo_inner(&mut connection, now(), &dir, Some(first.clone()))
                .unwrap();
        assert_eq!(
            profile.logo_data_url.as_deref(),
            Some(format!("data:image/jpeg;base64,{}", super::base64_encode(&first)).as_str())
        );
        assert_eq!(std::fs::read(&file).unwrap(), first);

        // Sustituye al anterior (gris, 1 componente).
        let second = jpeg(1);
        super::save_company_logo_inner(&mut connection, now(), &dir, Some(second.clone())).unwrap();
        assert_eq!(std::fs::read(&file).unwrap(), second);

        let png = b"\x89PNG\r\n\x1a\n0000IHDR".to_vec();
        let mut too_big = jpeg(3);
        too_big.resize(2 * 1024 * 1024 + 1, 0);
        for (bytes, message) in [
            (png, "El logo debe ser una imagen PNG o JPG válida"),
            (jpeg(4), "El logo debe ser una imagen PNG o JPG válida"),
            (
                vec![0xFF, 0xD8, 0xFF, 0xD9],
                "El logo debe ser una imagen PNG o JPG válida",
            ),
            (too_big, "El logo no puede superar los 2 MB"),
        ] {
            let error = super::save_company_logo_inner(&mut connection, now(), &dir, Some(bytes))
                .unwrap_err();
            assert_eq!(error.message, message);
        }
        // Un rechazo no toca el logo guardado.
        assert_eq!(std::fs::read(&file).unwrap(), second);

        let removed = super::save_company_logo_inner(&mut connection, now(), &dir, None).unwrap();
        assert_eq!(removed.logo_data_url, None);
        assert!(!file.exists());
    }

    #[test]
    fn company_commands_require_a_session_and_allow_both_roles() {
        let dir = temp_dir();
        let input = || crate::models::settings::UpdateCompanyProfileInput {
            name: String::new(),
            address: String::new(),
            phone: String::new(),
            tax_id: String::new(),
        };
        assert_requires_session(|c, n| super::get_company_profile_inner(c, n, &dir));
        assert_allows_both_roles(|c, n| super::get_company_profile_inner(c, n, &dir));
        assert_requires_session(|c, n| super::update_company_profile_inner(c, n, &dir, input()));
        assert_allows_both_roles(|c, n| super::update_company_profile_inner(c, n, &dir, input()));
        assert_requires_session(|c, n| super::save_company_logo_inner(c, n, &dir, None));
        assert_allows_both_roles(|c, n| super::save_company_logo_inner(c, n, &dir, None));
    }
}
