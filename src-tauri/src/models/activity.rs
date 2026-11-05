use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Default, PartialEq, Eq, Deserialize)]
#[serde(default, deny_unknown_fields, rename_all = "camelCase")]
pub struct ActivityFilters {
    pub from_date: Option<String>,
    pub to_date: Option<String>,
    pub user_name: Option<String>,
    pub action: Option<String>,
    pub search: Option<String>,
    pub oldest_first: bool,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct ActivityItem {
    pub id: i64,
    pub created_at: String,
    pub user_name: String,
    pub action: String,
    pub entity_type: String,
    pub entity_label: String,
    pub details: Option<String>,
}
