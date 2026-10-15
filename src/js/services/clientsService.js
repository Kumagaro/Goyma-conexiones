import { invoke } from "./invoke.js";

export function listClients() {
  return invoke("list_clients");
}

export function searchClients(query = "") {
  return invoke("search_clients", { query: query ?? "" });
}

export function createClient(input) {
  return invoke("create_client", { input });
}

export function updateClient(clientId, input) {
  return invoke("update_client", { clientId, input });
}

export function deleteClient(clientId) {
  return invoke("delete_client", { clientId });
}

export function getClientDetail(clientId) {
  return invoke("get_client_detail", { clientId });
}
