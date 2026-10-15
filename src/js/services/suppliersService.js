import { invoke } from "./invoke.js";

export function listSuppliers() {
  return invoke("list_suppliers");
}

export function searchSuppliers(query = "") {
  return invoke("search_suppliers", { query: query ?? "" });
}

export function createSupplier(input) {
  return invoke("create_supplier", { input });
}

export function updateSupplier(supplierId, input) {
  return invoke("update_supplier", { supplierId, input });
}

export function deleteSupplier(supplierId) {
  return invoke("delete_supplier", { supplierId });
}

export function getSupplierDetail(supplierId) {
  return invoke("get_supplier_detail", { supplierId });
}
