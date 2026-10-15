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
}

#[derive(Debug, Clone, PartialEq, Eq, Deserialize)]
#[serde(deny_unknown_fields, rename_all = "camelCase")]
pub struct InvoiceLineInput {
    pub product_id: i64,
    pub quantity: String,
    #[serde(default)]
    pub unit_price: Option<String>,
}
