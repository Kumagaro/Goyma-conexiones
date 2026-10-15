import { invoke } from "./invoke.js";

export function getProductHistory(filters) {
  return invoke("get_product_history", { filters });
}

export function listHistoryProducts() {
  return invoke("list_history_products");
}

export function exportHistoryExcel(input) {
  return invoke("export_history_excel", { input });
}

export function exportHistoryPdf(input) {
  return invoke("export_history_pdf", { input });
}
