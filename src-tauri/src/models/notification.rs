use serde::{Deserialize, Serialize};

/// Una notificación tal como la ve la pestaña (NT-01) y el feed de toasts
/// (TO-02). `entityType`/`entityId` permiten abrir lo afectado (NT-04).
#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct NotificationItem {
    pub id: i64,
    #[serde(rename = "type")]
    pub notification_type: String,
    pub priority: String,
    pub message: String,
    pub entity_type: Option<String>,
    pub entity_id: Option<i64>,
    pub is_read: bool,
    pub created_at: String,
}

/// Filtros de la pestaña: "No leídas", buscador y fecha local `AAAA-MM-DD`.
#[derive(Debug, Clone, Default, PartialEq, Eq, Deserialize)]
#[serde(rename_all = "camelCase", default)]
pub struct NotificationFilters {
    pub unread_only: bool,
    pub search: Option<String>,
    pub date: Option<String>,
}
