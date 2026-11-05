// Registro de actividad (spec-04 RA-02): consulta de solo lectura, exclusiva
// del Administrador. El backend también lo exige.

import { listActivityLog } from "../services/activityService.js";
import { getSession, onSessionChange } from "../session.js";
import { el, messageRow, setOptions } from "../utils/dom.js";
import { formatLocalTimestamp } from "../utils/formatters.js";
import { debounce } from "../utils/requestGuard.js";

const PAGE_ID = "actividad";
const ALL_USERS = "";

const ACTION_LABELS = {
  delete: "Eliminación",
  cancel: "Cancelación",
  edit: "Edición",
  config: "Configuración",
  backup: "Respaldo",
  restore: "Restauración",
  import: "Importación",
  purge: "Purga automática",
  expire: "Reserva vencida",
};

const ENTITY_LABELS = {
  product: "Producto",
  category: "Categoría",
  unit: "Unidad",
  conversion: "Conversión",
  client: "Cliente",
  supplier: "Proveedor",
  user: "Usuario",
  invoice: "Factura",
  settings: "Configuración",
  exchangeRate: "Tasa de cambio",
  backup: "Copia de seguridad",
  csv: "CSV",
};

let oldestFirst = false;
let requestId = 0;
const byId = (id) => document.getElementById(id);

function readFilters() {
  return {
    fromDate: byId("actividad-desde").value,
    toDate: byId("actividad-hasta").value,
    userName: byId("actividad-usuario").value,
    action: byId("actividad-accion").value,
    search: byId("actividad-buscar").value.trim(),
    oldestFirst,
  };
}

function entryRow(item) {
  return el(
    "tr",
    {},
    el("td", {}, formatLocalTimestamp(item.createdAt)),
    el("td", {}, item.userName),
    el("td", {}, ACTION_LABELS[item.action] ?? item.action),
    el("td", {}, ENTITY_LABELS[item.entityType] ?? item.entityType),
    el("td", {}, item.entityLabel),
    el("td", {}, item.details ?? ""),
  );
}

// El selector ofrece quienes aparecen en el registro, conservando la elección.
function refreshUsers(items) {
  const select = byId("actividad-usuario");
  const current = select.value;
  const names = new Set(items.map((item) => item.userName));
  if (current) names.add(current);
  setOptions(select, [
    [ALL_USERS, "Todos los usuarios"],
    ...[...names].sort().map((name) => [name, name]),
  ]);
  select.value = current;
}

export async function loadActivity() {
  const body = byId("actividad-body");
  if (!body) return;
  const id = ++requestId;
  try {
    const items = await listActivityLog(readFilters());
    if (id !== requestId) return;
    refreshUsers(items);
    if (items.length) body.replaceChildren(...items.map(entryRow));
    else messageRow(body, "No hay actividad con estos filtros.", 6);
  } catch {
    if (id !== requestId) return;
    messageRow(body, "No se pudo cargar el registro de actividad.", 6);
  }
}

function applyRoleVisibility() {
  const item = document
    .querySelector(`a[data-page="${PAGE_ID}"]`)
    ?.closest("li");
  if (item) item.hidden = getSession()?.role !== "administrador";
}

function toggleOrder() {
  oldestFirst = !oldestFirst;
  byId("actividad-orden").setAttribute(
    "aria-sort",
    oldestFirst ? "ascending" : "descending",
  );
  void loadActivity();
}

export function initActividadView() {
  applyRoleVisibility();
  onSessionChange(applyRoleVisibility);
  if (!byId(`page-${PAGE_ID}`)) return;
  const search = debounce(() => void loadActivity(), 300);
  byId("actividad-buscar").addEventListener("input", search);
  for (const id of [
    "actividad-desde",
    "actividad-hasta",
    "actividad-usuario",
    "actividad-accion",
  ]) {
    byId(id).addEventListener("change", () => void loadActivity());
  }
  byId("actividad-orden-boton").addEventListener("click", toggleOrder);
  setOptions(byId("actividad-accion"), [
    ["", "Todas las acciones"],
    ...Object.entries(ACTION_LABELS),
  ]);
  if (!window.__actividadPageChangeBound) {
    window.addEventListener("page:change", (event) => {
      if (event.detail?.pageId === PAGE_ID) void loadActivity();
    });
    window.__actividadPageChangeBound = true;
  }
}

if (document.readyState === "loading") {
  document.addEventListener("DOMContentLoaded", initActividadView);
} else {
  initActividadView();
}
