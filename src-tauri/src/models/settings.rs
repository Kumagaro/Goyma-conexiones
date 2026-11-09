use serde::{Deserialize, Serialize};

/// Ajustes globales de CF-04 (ambos usuarios).
#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct Settings {
    pub stale_invoice_days: i64,
    pub backup_reminder_days: i64,
    pub last_backup_at: Option<String>,
}

#[derive(Debug, Clone, PartialEq, Eq, Deserialize)]
#[serde(deny_unknown_fields, rename_all = "camelCase")]
pub struct UpdateSettingsInput {
    pub stale_invoice_days: i64,
    pub backup_reminder_days: i64,
}

/// Preferencias de cada usuario (CF-01, CF-04).
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields, rename_all = "camelCase")]
pub struct UserPreferences {
    pub visible_currencies: Vec<String>,
    pub page_size: i64,
}

/// Tasa actual de una moneda respecto al USD (CF-02).
#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct ExchangeRate {
    pub currency: String,
    pub rate: String,
    pub updated_at: String,
    pub updated_by_name: String,
}

/// Un cambio del historial de tasas (inmutable).
#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct ExchangeRateChange {
    pub created_at: String,
    pub currency: String,
    pub previous_rate: Option<String>,
    pub new_rate: String,
    pub user_name: String,
}

/// Datos de la empresa para el encabezado de las facturas (CF-03).
#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct CompanyProfile {
    pub name: String,
    pub address: String,
    pub phone: String,
    pub tax_id: String,
    /// `data:image/jpeg;base64,…` o `None` si no hay logo (D-07).
    pub logo_data_url: Option<String>,
}

#[derive(Debug, Clone, PartialEq, Eq, Deserialize)]
#[serde(deny_unknown_fields, rename_all = "camelCase")]
pub struct UpdateCompanyProfileInput {
    pub name: String,
    pub address: String,
    pub phone: String,
    pub tax_id: String,
}
