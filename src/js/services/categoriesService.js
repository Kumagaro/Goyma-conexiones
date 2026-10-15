import { invoke } from "./invoke.js";

export function listCategories(search = "") {
  return invoke("list_categories", { search: search ?? "" });
}

export function searchCategories(text = "") {
  return invoke("search_categories", { text: text ?? "" });
}

export function getCategory(categoryId) {
  return invoke("get_category", { categoryId });
}

export function createCategory(name, description = "") {
  return invoke("create_category", {
    name,
    description: description && description.trim() ? description : null,
  });
}

export function updateCategory(categoryId, name, description = "") {
  return invoke("update_category", {
    categoryId,
    name,
    description: description && description.trim() ? description : null,
  });
}

export function deleteCategory(categoryId) {
  return invoke("delete_category", { categoryId });
}
