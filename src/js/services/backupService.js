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
