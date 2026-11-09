use serde::{Deserialize, Serialize};

/// Fila del listado de servicios (`spec-06` SV-01).
#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct ServiceListItem {
    pub id: i64,
    pub name: String,
    pub description: Option<String>,
    pub suggested_price_cents: Option<i64>,
    pub product_count: i64,
}

/// Producto asociado a un servicio, con su estado actual para marcar los
/// descontinuados en la ficha y omitirlos al agregarlo a una factura.
#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct ServiceProductDetail {
    pub product_id: i64,
    pub name: String,
    pub unit_abbreviation: Option<String>,
    pub suggested_quantity: String,
    pub status: String,
    pub selling_price_cents: i64,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct ServiceDetail {
    pub id: i64,
    pub name: String,
    pub description: Option<String>,
    pub suggested_price_cents: Option<i64>,
    pub products: Vec<ServiceProductDetail>,
}

#[derive(Debug, Clone, PartialEq, Eq, Deserialize)]
#[serde(deny_unknown_fields, rename_all = "camelCase")]
pub struct ServiceProductInput {
    pub product_id: i64,
    pub quantity: String,
}

/// Alta y edición de un servicio: nombre, descripción, precio sugerido y
/// productos con su cantidad sugerida.
#[derive(Debug, Clone, PartialEq, Eq, Deserialize)]
#[serde(deny_unknown_fields, rename_all = "camelCase")]
pub struct ServiceInput {
    pub name: String,
    #[serde(default)]
    pub description: Option<String>,
    #[serde(default)]
    pub suggested_price: Option<String>,
    #[serde(default)]
    pub products: Vec<ServiceProductInput>,
}
