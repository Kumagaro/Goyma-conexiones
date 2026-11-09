import { invoke } from "./invoke.js";

export function createBackup(destinationPath) {
  return invoke("create_backup", { destinationPath });
}

export function listAutomaticBackups() {
  return invoke("list_automatic_backups");
}

export function restoreBackup({ sourcePath, automaticFileName, password }) {
  return invoke("restore_backup", { sourcePath, automaticFileName, password });
}

// spec-06 RP-01: contraseña de respaldo.
export function getBackupPasswordStatus() {
  return invoke("get_backup_password_status");
}

export function setBackupPassword(userPassword, newPassword) {
  return invoke("set_backup_password", { userPassword, newPassword });
}
