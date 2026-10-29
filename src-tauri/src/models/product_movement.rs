use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct ProductMovement {
    pub id: i64,
    pub created_at: String,
    pub product_id: Option<i64>,
    pub product_id_original: i64,
    pub product_name: String,
    pub movement_type: String,
    pub quantity_milli: i64,
    pub stock_after_milli: i64,
    pub unit_price_cents: i64,
    pub total_price_cents: i64,
    pub invoice_id: Option<i64>,
    pub invoice_number: Option<String>,
    pub invoice_type: Option<String>,
    pub user_id: Option<i64>,
    pub user_name: String,
}

/// Exportación del historial (HP-03): los mismos filtros de la pantalla más la
/// ruta elegida en el diálogo "Guardar como".
#[derive(Debug, Clone, PartialEq, Eq, Deserialize)]
#[serde(deny_unknown_fields, rename_all = "camelCase")]
pub struct ExportHistoryInput {
    pub product_id: i64,
    #[serde(default)]
    pub from_date: Option<String>,
    #[serde(default)]
    pub to_date: Option<String>,
    #[serde(default)]
    pub types: Vec<String>,
    pub destination_path: String,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct ExportHistoryResult {
    pub exported_count: usize,
    pub expired_cleanup_warning: Option<String>,
}

#[derive(Debug, Clone, PartialEq, Eq, Deserialize, Default)]
#[serde(deny_unknown_fields, rename_all = "camelCase")]
pub struct ProductHistoryFilters {
    pub product_id: i64,
    #[serde(default)]
    pub from_date: Option<String>,
    #[serde(default)]
    pub to_date: Option<String>,
    #[serde(default)]
    pub types: Vec<String>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct ProductHistoryItem {
    pub id: i64,
    pub created_at: String,
    #[serde(rename = "type")]
    pub movement_type: String,
    pub quantity: String,
    pub stock_after: String,
    pub unit_price_cents: i64,
    pub total_price_cents: i64,
    pub invoice_number: Option<String>,
    pub invoice_type: Option<String>,
    /// Factura que sigue existiendo con ese número y tipo (los números nunca
    /// se reutilizan); `None` si se eliminó, para no abrir un detalle ajeno.
    pub invoice_id: Option<i64>,
    pub user_name: String,
    pub product_id_original: i64,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct ProductHistoryList {
    pub items: Vec<ProductHistoryItem>,
    pub expired_cleanup_warning: Option<String>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct ProductHistoryOption {
    pub id: i64,
    pub name: String,
    pub is_deleted: bool,
}
