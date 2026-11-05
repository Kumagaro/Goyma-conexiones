import { invoke } from "./invoke.js";

export function listActivityLog(filters = {}) {
  return invoke("list_activity_log", { filters });
}
