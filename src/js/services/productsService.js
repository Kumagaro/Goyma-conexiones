import { invoke } from "./invoke.js";

export function listProducts(filters = {}) {
  return invoke("list_products", { filters });
}

export function listOutOfStockAndDiscontinued(search = "", condition = "all") {
  return invoke("list_out_of_stock_and_discontinued", {
    search: search ?? "",
    condition: condition ?? "all",
  });
}

export function getProduct(productId) {
  return invoke("get_product", { productId });
}

export function createProduct(input) {
  return invoke("create_product", { input });
}

export function updateProduct(input) {
  return invoke("update_product", { input });
}

export function previewSellingPrice(input) {
  return invoke("preview_selling_price", { input });
}

export function discontinueProduct(productId) {
  return invoke("discontinue_product", { productId });
}

export function activateProduct(productId) {
  return invoke("activate_product", { productId });
}

export function deleteProduct(productId) {
  return invoke("delete_product", { productId });
}
