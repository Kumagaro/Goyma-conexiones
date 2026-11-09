use serde::Serialize;

#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct BackupItem {
    pub file_name: String,
    pub created_at: String,
    pub size_bytes: u64,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct RestoreResult {
    pub session_ended: bool,
}

/// spec-06 RP-01: si hay contraseña de respaldo y cuándo se definió.
#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct BackupPasswordStatus {
    pub is_set: bool,
    pub updated_at: Option<String>,
    /// Solo el Administrador la define o la cambia.
    pub can_change: bool,
}
