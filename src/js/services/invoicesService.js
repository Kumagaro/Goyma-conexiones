import { invoke } from "./invoke.js";

export function listInvoices(filters = {}) {
  return invoke("list_invoices", { filters });
}

export function getInvoice(invoiceId) {
  return invoke("get_invoice", { invoiceId });
}

export function createEntryInvoice(input) {
  return invoke("create_entry_invoice", { input });
}

export function updateEntryInvoice(input) {
  return invoke("update_entry_invoice", { input });
}

export function createSaleInvoice(input) {
  return invoke("create_sale_invoice", { input });
}

export function updateSaleInvoice(input) {
  return invoke("update_sale_invoice", { input });
}

export function updateServiceInvoice(input) {
  return invoke("update_service_invoice", { input });
}

export function updateGuaranteeInvoice(input) {
  return invoke("update_guarantee_invoice", { input });
}

export function updateInstallationInvoice(input) {
  return invoke("update_installation_invoice", { input });
}

export function createServiceInvoice(input) {
  return invoke("create_service_invoice", { input });
}

export function createGuaranteeInvoice(input) {
  return invoke("create_guarantee_invoice", { input });
}

export function createInstallationInvoice(input) {
  return invoke("create_installation_invoice", { input });
}

export function createLossInvoice(input) {
  return invoke("create_loss_invoice", { input });
}

export function saveInvoiceAttachment(invoiceId, sourcePath) {
  return invoke("save_invoice_attachment", { invoiceId, sourcePath });
}

export function downloadInvoiceAttachment(invoiceId, destinationPath) {
  return invoke("download_invoice_attachment", { invoiceId, destinationPath });
}

// spec-06 EN-02: entrega total o parcial de líneas de Venta o Servicio.
export function recordDeliveries(input) {
  return invoke("record_deliveries", { input });
}

export function changeInvoiceStatus(invoiceId, status, paymentMethod) {
  const args = { invoiceId, status };
  if (paymentMethod !== undefined) args.paymentMethod = paymentMethod;
  return invoke("change_invoice_status", args);
}

export function deleteInvoice(invoiceId) {
  return invoke("delete_invoice", { invoiceId });
}

// spec-04 IM-01: documento de impresión (mismo modelo que el PDF).
export function getInvoicePrintModel(invoiceId) {
  return invoke("get_invoice_print_model", { invoiceId });
}

// spec-04 IM-02: PDF generado por el backend en la ruta elegida.
export function exportInvoicePdf(invoiceId, destinationPath) {
  return invoke("export_invoice_pdf", { invoiceId, destinationPath });
}
