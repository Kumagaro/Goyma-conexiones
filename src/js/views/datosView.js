// Exportar e importar CSV (spec-04 CSV-01, CSV-02) y la pestaña "Respaldo" de
// Configuración (RP-01 a RP-03).

import { confirmDialog } from "../components/confirmDialog.js";
import { openFormDialog } from "../components/formDialog.js";
import {
  createBackup,
  getBackupPasswordStatus,
  listAutomaticBackups,
  restoreBackup,
  setBackupPassword,
} from "../services/backupService.js";
import { exportCsv, importCsv } from "../services/csvService.js";
import { el, messageRow } from "../utils/dom.js";
import {
  notifyError,
  notifySuccess,
  notifyWarning,
} from "../utils/feedback.js";
import { formatLocalTimestamp } from "../utils/formatters.js";

const PAGE_ID = "configuracion";

const EXPORT_NAMES = {
  products: "productos",
  categories: "categorias",
  units: "unidades",
  clients: "clientes",
  suppliers: "proveedores",
  invoices: "facturas",
};

const SEARCH_INPUTS = {
  categories: "categoria-search",
  units: "unidad-search",
  clients: "cliente-search",
  suppliers: "proveedor-search",
};

const byId = (id) => document.getElementById(id);
const fieldValue = (id) => String(byId(id)?.value ?? "").trim();

function localDate() {
  const now = new Date();
  const month = String(now.getMonth() + 1).padStart(2, "0");
  const day = String(now.getDate()).padStart(2, "0");
  return `${now.getFullYear()}-${month}-${day}`;
}

async function askSavePath(defaultPath, name, extension) {
  try {
    const path = await window.__TAURI__?.dialog?.save?.({
      defaultPath,
      filters: [{ name, extensions: [extension] }],
    });
    return typeof path === "string" && path ? path : null;
  } catch {
    return null;
  }
}

async function askOpenPath(name, extensions) {
  try {
    const path = await window.__TAURI__?.dialog?.open?.({
      multiple: false,
      directory: false,
      filters: [{ name, extensions }],
    });
    return typeof path === "string" ? path : null;
  } catch {
    return null;
  }
}

/* ---------- Exportar ---------- */

function productFilters() {
  const category = fieldValue("producto-filtro-categoria");
  const categoryId = Number(category);
  return {
    search: fieldValue("producto-search"),
    categoryId:
      category && Number.isSafeInteger(categoryId) ? categoryId : null,
    withoutCategory: category === "withoutCategory",
    stock: fieldValue("producto-filtro-stock") || "all",
    expiry: fieldValue("producto-filtro-caducidad") || "all",
  };
}

function invoiceFilters({ csvPrefix: prefix, csvType }) {
  const filters = {};
  const put = (key, id) => {
    if (fieldValue(id)) filters[key] = fieldValue(id);
  };
  put("search", `${prefix}-search`);
  put("status", `${prefix}-filtro-estado`);
  // En `factura-venta-estado` y similares está el estado del formulario.
  if (prefix === "factura-general") put("status", `${prefix}-estado`);
  put("invoiceType", `${prefix}-tipo`);
  put("fromDate", `${prefix}-desde`);
  put("toDate", `${prefix}-hasta`);
  if (csvType) filters.invoiceType = csvType;
  return filters;
}

// CSV-01: los filtros activos del listado desde el que se exporta.
function activeFilters(kind, dataset) {
  if (kind === "products") return { products: productFilters() };
  if (kind === "invoices") return { invoices: invoiceFilters(dataset) };
  return { search: fieldValue(SEARCH_INPUTS[kind]) };
}

export async function exportList(button) {
  const kind = button.dataset.csvExport;
  const path = await askSavePath(
    `${EXPORT_NAMES[kind]}-${localDate()}.csv`,
    "Archivo CSV",
    "csv",
  );
  if (!path) return;
  try {
    const rows = await exportCsv(
      kind,
      activeFilters(kind, button.dataset),
      path,
    );
    notifySuccess(`Se exportaron ${rows} filas.`);
  } catch (error) {
    notifyError(error, "No se pudo exportar el listado.");
  }
}

/* ---------- Importar ---------- */

function renderSkipped(skipped) {
  const list = byId("config-csv-omitidas");
  list.hidden = !skipped.length;
  list.replaceChildren(
    ...skipped.map(({ row, reason }) => el("li", {}, `Fila ${row}: ${reason}`)),
  );
}

export async function importFile() {
  const path = await askOpenPath("Archivo CSV", ["csv"]);
  if (!path) return;
  renderSkipped([]);
  try {
    const { imported, skipped } = await importCsv(
      byId("config-csv-tipo").value,
      path,
    );
    renderSkipped(skipped);
    const message = `Se importaron ${imported} filas y se omitieron ${skipped.length}.`;
    if (skipped.length) notifyWarning(message);
    else notifySuccess(message);
  } catch (error) {
    notifyError(error, "No se pudo importar el archivo.");
  }
}

/* ---------- Respaldo ---------- */

export async function saveBackup() {
  const path = await askSavePath(
    `goyma-${localDate()}.sqlite3`,
    "Copia de seguridad",
    "sqlite3",
  );
  if (!path) return;
  try {
    await createBackup(path);
    notifySuccess("Copia de seguridad creada.");
  } catch (error) {
    notifyError(error, "No se pudo crear la copia de seguridad.");
  }
}

// RP-03, spec-06 RP-02: confirma, pide la contraseña de respaldo y vuelve
// al login.
async function restore(source, label) {
  const confirmed = await confirmDialog({
    title: "Restaurar copia",
    message: `Se reemplazarán todos los datos actuales por los de «${label}». Antes se guardará una copia de lo actual y la sesión se cerrará.`,
    confirmText: "Restaurar",
  });
  if (!confirmed) return;
  const result = await openFormDialog({
    title: "Contraseña de respaldo",
    submitText: "Restaurar",
    missingMessage: "Escribe la contraseña de respaldo.",
    fields: [
      {
        name: "password",
        label: "Contraseña de respaldo",
        type: "password",
        required: true,
      },
    ],
    onSubmit: ({ password }) => restoreBackup({ ...source, password }),
  });
  if (result?.sessionEnded) window.location.replace("./index.html");
}

export async function restoreFromFile() {
  const path = await askOpenPath("Copia de seguridad", ["sqlite3"]);
  if (!path) return;
  await restore({ sourcePath: path }, path.split(/[\\/]/).pop());
}

function backupRow({ fileName, createdAt, sizeBytes }) {
  const button = el(
    "button",
    {
      type: "button",
      class: "action-link",
      "aria-label": `Restaurar ${fileName}`,
    },
    "Restaurar",
  );
  button.addEventListener("click", () => {
    void restore({ automaticFileName: fileName }, fileName);
  });
  return el(
    "tr",
    {},
    el("td", {}, fileName),
    el("td", {}, formatLocalTimestamp(createdAt)),
    el(
      "td",
      {},
      `${(sizeBytes / 1024 / 1024).toFixed(2).replace(".", ",")} MB`,
    ),
    el("td", {}, button),
  );
}

export async function loadBackups() {
  const body = byId("config-respaldos-body");
  if (!body) return;
  try {
    const items = await listAutomaticBackups();
    if (items?.length) body.replaceChildren(...items.map(backupRow));
    else messageRow(body, "Todavía no hay copias automáticas.", 4);
  } catch {
    messageRow(body, "No se pudieron cargar las copias.", 4);
  }
}

/* ---------- Contraseña de respaldo (spec-06 RP-01) ---------- */

export async function loadBackupPasswordStatus() {
  const text = byId("config-respaldo-clave-estado");
  const button = byId("config-respaldo-clave");
  if (!text) return;
  try {
    const status = await getBackupPasswordStatus();
    text.textContent = status?.isSet
      ? `Restaurar una copia pide la contraseña de respaldo. Se definió el ${formatLocalTimestamp(status.updatedAt)}.`
      : "Aún no hay contraseña de respaldo: las copias que se hagan mientras tanto no quedan protegidas y se podrán restaurar con la contraseña de usuario.";
    if (button) {
      button.textContent = status?.isSet
        ? "Cambiar contraseña de respaldo"
        : "Definir contraseña de respaldo";
      // Solo el Administrador la define o la cambia.
      button.hidden = !status?.canChange;
    }
  } catch {
    text.textContent = "No se pudo comprobar la contraseña de respaldo.";
  }
}

export async function changeBackupPassword() {
  const result = await openFormDialog({
    title: "Contraseña de respaldo",
    submitText: "Guardar contraseña",
    missingMessage: "Completa los tres campos.",
    fields: [
      {
        name: "userPassword",
        label: "Tu contraseña de usuario",
        type: "password",
        required: true,
      },
      {
        name: "newPassword",
        label: "Nueva contraseña de respaldo (mínimo 8 caracteres)",
        type: "password",
        required: true,
      },
      {
        name: "confirmation",
        label: "Repite la contraseña de respaldo",
        type: "password",
        required: true,
      },
    ],
    onSubmit: ({ userPassword, newPassword, confirmation }) => {
      if (newPassword !== confirmation) {
        throw { message: "Las contraseñas de respaldo no coinciden." };
      }
      return setBackupPassword(userPassword, newPassword);
    },
  });
  if (!result) return;
  notifySuccess("Contraseña de respaldo guardada.");
  await loadBackupPasswordStatus();
}

export function initDatosView() {
  document.addEventListener("click", (event) => {
    const button = event.target.closest?.("[data-csv-export]");
    if (button) void exportList(button);
  });
  byId("config-respaldo-crear")?.addEventListener(
    "click",
    () => void saveBackup(),
  );
  byId("config-respaldo-restaurar")?.addEventListener(
    "click",
    () => void restoreFromFile(),
  );
  byId("config-respaldo-clave")?.addEventListener(
    "click",
    () => void changeBackupPassword(),
  );
  byId("config-csv-importar")?.addEventListener(
    "click",
    () => void importFile(),
  );
  if (!window.__datosPageChangeBound) {
    window.addEventListener("page:change", (event) => {
      if (event.detail?.pageId !== PAGE_ID) return;
      void loadBackups();
      void loadBackupPasswordStatus();
    });
    window.__datosPageChangeBound = true;
  }
}

if (document.readyState === "loading") {
  document.addEventListener("DOMContentLoaded", initDatosView);
} else {
  initDatosView();
}
