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

// `purpose`: "entry", "outgoing" o "loss"; decide qué productos se ofrecen.
export function searchProductsForInvoice(query, purpose) {
  return invoke("search_products_for_invoice", { query, purpose });
}

export function getProductAssociatedInvoices(productId) {
  return invoke("get_product_associated_invoices", { productId });
}
