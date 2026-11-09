import { invoke } from "./invoke.js";

// spec-06 SV-01: catálogo de servicios.
export function listServices(search = "") {
  return invoke("list_services", { search: search ?? "" });
}

export function getService(serviceId) {
  return invoke("get_service", { serviceId });
}

export function createService(input) {
  return invoke("create_service", { input });
}

export function updateService(serviceId, input) {
  return invoke("update_service", { serviceId, input });
}

export function deleteService(serviceId) {
  return invoke("delete_service", { serviceId });
}
