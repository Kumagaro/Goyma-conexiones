import { invoke } from "./invoke.js";

export function getDashboardSummary() {
  return invoke("get_dashboard_summary");
}
