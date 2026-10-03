use serde::Serialize;

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Session {
    pub id: i64,
    pub user_id: i64,
    pub remember_session: bool,
    pub created_at: String,
    pub last_activity_at: String,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct TouchResult {
    pub status: String,
}
