import { invoke } from "./invoke.js";

export function listUnits(search = "") {
  return invoke("list_units", { search: search ?? "" });
}

export function searchUnits(text = "") {
  return invoke("search_units", { text: text ?? "" });
}

export function createUnit(name, abbreviation) {
  return invoke("create_unit", { name, abbreviation });
}

export function updateUnit(unitId, name, abbreviation) {
  return invoke("update_unit", { unitId, name, abbreviation });
}

export function deleteUnit(unitId) {
  return invoke("delete_unit", { unitId });
}

export function listConversions(search = "") {
  return invoke("list_conversions", { search: search ?? "" });
}

export function createConversion(fromUnitId, toUnitId, factor) {
  return invoke("create_conversion", { fromUnitId, toUnitId, factor });
}

export function updateConversionFactor(conversionId, factor) {
  return invoke("update_conversion_factor", { conversionId, factor });
}

export function deleteConversion(conversionId) {
  return invoke("delete_conversion", { conversionId });
}

export function convertQuantity(fromUnitId, toUnitId, quantity) {
  return invoke("convert_quantity", { fromUnitId, toUnitId, quantity });
}
