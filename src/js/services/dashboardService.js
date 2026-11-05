import { invoke } from "./invoke.js";

export function getDashboardSummary() {
  return invoke("get_dashboard_summary");
}

export function getDashboardKpis(period) {
  return invoke("get_dashboard_kpis", { period });
}
