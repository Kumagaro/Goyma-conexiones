use serde::Serialize;

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct User {
    pub id: i64,
    pub username: String,
    pub password_hash: String,
    pub role: String,
    pub phone: String,
    pub national_id: String,
    pub created_at: String,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct UserListItem {
    pub id: i64,
    pub username: String,
    pub role: String,
    pub phone: String,
    pub national_id: String,
    pub is_current_user: bool,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct UserProfile {
    pub username: String,
    pub role: String,
    pub phone: String,
    pub national_id: String,
    pub has_reserved_contact_data: bool,
}
