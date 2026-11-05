use serde::Serialize;

#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct Draft {
    pub form_key: String,
    pub record_id: Option<i64>,
    pub payload: String,
    pub updated_at: String,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct DraftItem {
    pub form_key: String,
    pub record_id: Option<i64>,
    pub updated_at: String,
}
