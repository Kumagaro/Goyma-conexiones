use serde::Serialize;

#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct Consumption {
    pub id: i64,
    pub invoice_line_id: i64,
    pub lot_id: Option<i64>,
    pub quantity_milli: i64,
    pub unit_cost_cents: i64,
    pub created_at: String,
}
