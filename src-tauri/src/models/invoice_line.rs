use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct InvoiceLineDetail {
    pub position: i64,
    pub product_id_original: i64,
    pub product_name: String,
    pub unit_name: Option<String>,
    pub unit_abbreviation: Option<String>,
    pub quantity: String,
    pub unit_price_cents: i64,
    pub subtotal_cents: i64,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub category_name: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub cost_at_sale_cents: Option<i64>,
    /// Estado actual del producto (`active`/`discontinued`); `None` si se
    /// eliminó. La edición lo usa para bloquear cambios (FE-04, FS-01).
    pub product_status: Option<String>,
    /// Solo en compras: unidades de su lote que siguen en inventario y las ya
    /// consumidas por salidas no canceladas (aviso al eliminar, FE-05).
    #[serde(skip_serializing_if = "Option::is_none")]
    pub remaining_quantity: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub consumed_quantity: Option<String>,
    /// CV-01: lo que se escribió ("2 Caja"); `quantity` y `unitPriceCents`
    /// siguen en la unidad del producto.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub entered: Option<EnteredUnit>,
    /// spec-06 PF-01: precio de lista (mínimo al editar) en Venta y Servicio.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub list_price_cents: Option<i64>,
    /// spec-06 PF-02: bruto y descuento cuando la línea tiene descuento.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub gross_cents: Option<i64>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub discount_kind: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub discount_value: Option<i64>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub discount_cents: Option<i64>,
    /// `spec-06` EN-02: lo ya entregado de la línea (Venta y Servicio).
    pub delivered_quantity: String,
    /// spec-06 SV-02: posición del servicio al que pertenece la línea.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub service_position: Option<i64>,
}

/// Unidad y cantidad escritas en la línea, y el precio por esa unidad.
#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct EnteredUnit {
    /// `None` si la unidad se eliminó después (la línea conserva su copia).
    pub unit_id: Option<i64>,
    pub name: String,
    pub abbreviation: String,
    pub quantity: String,
    pub unit_price_cents: i64,
}

#[derive(Debug, Clone, PartialEq, Eq, Deserialize)]
#[serde(deny_unknown_fields, rename_all = "camelCase")]
pub struct InvoiceLineInput {
    pub product_id: i64,
    pub quantity: String,
    #[serde(default)]
    pub unit_price: Option<String>,
}
