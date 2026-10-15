use serde::Serialize;

#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct Lot {
    pub id: i64,
    pub product_id: i64,
    pub origin: String,
    pub invoice_line_id: Option<i64>,
    pub arrival_order: i64,
    pub initial_milli: i64,
    pub remaining_milli: i64,
    pub unit_cost_cents: i64,
    pub created_at: String,
    pub updated_at: String,
}
