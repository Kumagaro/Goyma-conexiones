import {
  exportHistoryExcel,
  exportHistoryPdf,
  getProductHistory,
  listHistoryProducts,
} from "../services/historyService.js";
import { showCleanupWarning } from "../utils/cleanupWarning.js";
import { formatId, formatMoney, formatTimestamp } from "../utils/formatters.js";

// Etiquetas visibles de cada tipo de movimiento del historial.
const MOVEMENT_TYPE_LABELS = {
  initial: "Inicial",
  entry: "Entrada",
  saleOut: "Salida Venta",
  serviceOut: "Salida Servicio",
  warrantyOut: "Salida Garantía",
  supplyOut: "Salida Insumos",
  loss: "Pérdida",
  adjustment: "Ajuste",
  reversal: "Reversión",
};

const TYPE_GROUPS = [
  { label: "Entradas", types: ["initial", "entry"] },
  {
    label: "Salidas",
    types: ["saleOut", "serviceOut", "warrantyOut", "supplyOut", "loss"],
  },
  { label: "Ajustes", types: ["adjustment", "reversal"] },
];

export function initHistorialView() {
  return initializeHistorialView();
}

async function initializeHistorialView() {
  makeHistoryVisible();
  bindViewHistoryListener();
  bindHistoryTab();
  bindHistoryFilters();
  bindExportButtons();
  buildTypeFilter();
  await loadHistoryOptions();
  await loadProductHistory();
}

function makeHistoryVisible() {
  // La pestaña, la vista y el botón quedan visibles (HP-02).
  for (const id of [
    "tab-producto-historial",
    "producto-historial",
    "producto-ver-historial",
  ]) {
    document.getElementById(id)?.removeAttribute("hidden");
  }
}

function bindViewHistoryListener() {
  if (window.__historialViewHistoryBound) return;
  // Lo dispara productosView.js con { productId } al pulsar "Ver Historial".
  document.addEventListener("product:viewHistory", (event) => {
    const productId = Number(event.detail?.productId);
    if (!Number.isFinite(productId)) return;
    void openProductHistory(productId);
  });
  window.__historialViewHistoryBound = true;
}

function bindHistoryTab() {
  const tab = document.getElementById("tab-producto-historial");
  if (!tab || tab.dataset.historialBound === "1") return;
  tab.dataset.historialBound = "1";
  // Al entrar a la pestaña se recarga el historial.
  tab.addEventListener("click", () => {
    void loadProductHistory();
  });
}

function bindHistoryFilters() {
  for (const id of [
    "historial-producto-select",
    "historial-producto-desde",
    "historial-producto-hasta",
    "historial-producto-tipo",
  ]) {
    const control = document.getElementById(id);
    if (!control || control.dataset.historialBound === "1") continue;
    control.dataset.historialBound = "1";
    control.addEventListener("change", () => {
      void loadProductHistory();
    });
  }
}

function buildTypeFilter() {
  const select = document.getElementById("historial-producto-tipo");
  if (!select) return;
  select.multiple = true;
  select.replaceChildren();
  for (const group of TYPE_GROUPS) {
    const optgroup = document.createElement("optgroup");
    optgroup.label = group.label;
    for (const type of group.types) {
      const option = document.createElement("option");
      option.value = type;
      option.textContent = MOVEMENT_TYPE_LABELS[type] ?? type;
      optgroup.append(option);
    }
    select.append(optgroup);
  }
}

async function loadHistoryOptions() {
  const select = document.getElementById("historial-producto-select");
  if (!select) return;
  const previous = select.value;
  try {
    const options = await listHistoryProducts();
    const items = Array.isArray(options) ? options : [];
    select.replaceChildren();
    const activeGroup = document.createElement("optgroup");
    activeGroup.label = "Productos activos";
    const deletedGroup = document.createElement("optgroup");
    deletedGroup.label = "Productos eliminados";
    for (const item of items) {
      const option = document.createElement("option");
      option.value = String(item?.id ?? "");
      // Los eliminados se identifican por nombre e ID original (HP-02).
      option.textContent = item?.isDeleted
        ? `${item?.name ?? ""} (eliminado, ${formatId(item?.id)})`
        : (item?.name ?? "");
      (item?.isDeleted ? deletedGroup : activeGroup).append(option);
    }
    select.append(activeGroup, deletedGroup);
    if (previous) select.value = previous;
  } catch {
    showHistoryFeedback("No se pudieron cargar los productos.");
  }
}

function readHistoryFilters() {
  const select = document.getElementById("historial-producto-select");
  const productId = Number(select?.value);
  const fromDate =
    document.getElementById("historial-producto-desde")?.value || undefined;
  const toDate =
    document.getElementById("historial-producto-hasta")?.value || undefined;
  const typeSelect = document.getElementById("historial-producto-tipo");
  // Sin selección se devuelven todos los tipos (HP-02).
  const types = typeSelect
    ? Array.from(typeSelect.selectedOptions).map((o) => o.value)
    : [];
  return { productId, fromDate, toDate, types };
}

async function loadProductHistory() {
  const body = document.getElementById("historial-producto-body");
  if (!body) return;
  const { productId, fromDate, toDate, types } = readHistoryFilters();
  if (!Number.isFinite(productId)) {
    renderHistoryList([]);
    setExportAvailable(false);
    return;
  }
  setHistoryLoading(true);
  try {
    const response = await getProductHistory({
      productId,
      ...(fromDate ? { fromDate } : {}),
      ...(toDate ? { toDate } : {}),
      types,
    });
    const items = Array.isArray(response) ? response : (response?.items ?? []);
    renderHistoryList(items);
    setExportAvailable(items.length > 0);
    showCleanupWarning(
      document.getElementById("producto-historial"),
      "historial-cleanup-warning",
      response?.expiredCleanupWarning,
    );
  } catch (error) {
    renderHistoryList([]);
    setExportAvailable(false);
    showHistoryFeedback(
      error?.message || "No se pudo cargar el historial del producto.",
    );
  } finally {
    setHistoryLoading(false);
  }
}

function setHistoryLoading(isLoading) {
  const body = document.getElementById("historial-producto-body");
  if (!body || !isLoading) return;
  body.replaceChildren();
  const row = document.createElement("tr");
  const cell = document.createElement("td");
  cell.textContent = "Cargando historial…";
  row.append(cell);
  body.append(row);
}

function renderHistoryList(items) {
  const body = document.getElementById("historial-producto-body");
  if (!body) return;
  body.replaceChildren();
  if (!items.length) {
    const row = document.createElement("tr");
    const cell = document.createElement("td");
    cell.textContent = "Este producto aún no tiene movimientos registrados.";
    row.append(cell);
    body.append(row);
    return;
  }
  for (const item of items) {
    const row = document.createElement("tr");
    const dateCell = document.createElement("td");
    dateCell.textContent = item?.createdAt
      ? formatTimestamp(item.createdAt)
      : "—";
    const typeCell = document.createElement("td");
    typeCell.textContent = MOVEMENT_TYPE_LABELS[item?.type] ?? "—";
    const quantityCell = document.createElement("td");
    quantityCell.textContent = item?.quantity ?? "—";
    const stockCell = document.createElement("td");
    stockCell.textContent = item?.stockAfter ?? "—";
    const unitCell = document.createElement("td");
    unitCell.textContent = formatMoney(item?.unitPriceCents);
    const totalCell = document.createElement("td");
    totalCell.textContent = formatMoney(item?.totalPriceCents);
    const invoiceCell = document.createElement("td");
    invoiceCell.textContent = item?.invoiceNumber ?? "—";
    const userCell = document.createElement("td");
    userCell.textContent = item?.userName ?? "—";
    row.append(
      dateCell,
      typeCell,
      quantityCell,
      stockCell,
      unitCell,
      totalCell,
      invoiceCell,
      userCell,
    );
    body.append(row);
  }
}

async function openProductHistory(productId) {
  makeHistoryVisible();
  const select = document.getElementById("historial-producto-select");
  if (select) {
    select.value = String(productId);
    if (select.value !== String(productId)) {
      // El producto aún no está en las opciones: se recargan primero.
      await loadHistoryOptions();
      select.value = String(productId);
    }
  }
  if (typeof window.showProductoTab === "function") {
    window.showProductoTab(
      "historial",
      document.getElementById("tab-producto-historial"),
    );
  }
  await loadProductHistory();
}

// ===================== Exportación (HP-03) =====================

const EXPORT_FORMATS = {
  excel: {
    buttonId: "historial-exportar-excel",
    extension: "xlsx",
    filterName: "Libro de Excel",
    run: (input) => exportHistoryExcel(input),
  },
  pdf: {
    buttonId: "historial-exportar-pdf",
    extension: "pdf",
    filterName: "Documento PDF",
    run: (input) => exportHistoryPdf(input),
  },
};

const NO_MOVEMENTS_MESSAGE = "No hay movimientos para exportar";

function bindExportButtons() {
  for (const [format, config] of Object.entries(EXPORT_FORMATS)) {
    const button = document.getElementById(config.buttonId);
    if (!button || button.dataset.exportBound === "1") continue;
    button.dataset.exportBound = "1";
    button.addEventListener("click", () => {
      void handleExport(format);
    });
  }
}

// Sin movimientos los botones se deshabilitan y el motivo se anuncia.
function setExportAvailable(available) {
  for (const config of Object.values(EXPORT_FORMATS)) {
    const button = document.getElementById(config.buttonId);
    if (!button) continue;
    button.disabled = !available;
    if (available) button.removeAttribute("title");
    else button.title = NO_MOVEMENTS_MESSAGE;
  }
  const status = document.getElementById("historial-exportar-estado");
  if (status) status.textContent = available ? "" : NO_MOVEMENTS_MESSAGE;
}

async function chooseDestination(config) {
  try {
    const path = await window.__TAURI__?.dialog?.save?.({
      defaultPath: `historial.${config.extension}`,
      filters: [{ name: config.filterName, extensions: [config.extension] }],
    });
    return typeof path === "string" && path ? path : null;
  } catch {
    return null;
  }
}

async function handleExport(format) {
  const config = EXPORT_FORMATS[format];
  const { productId, fromDate, toDate, types } = readHistoryFilters();
  if (!config || !Number.isFinite(productId)) return;
  const destinationPath = await chooseDestination(config);
  // Si el usuario cancela el diálogo, no ocurre nada.
  if (!destinationPath) return;
  const status = document.getElementById("historial-exportar-estado");
  try {
    const result = await config.run({
      productId,
      ...(fromDate ? { fromDate } : {}),
      ...(toDate ? { toDate } : {}),
      types,
      destinationPath,
    });
    const count = result?.exportedCount ?? 0;
    if (status) {
      status.textContent = `Historial exportado (${count} ${count === 1 ? "movimiento" : "movimientos"}).`;
    }
  } catch (error) {
    showHistoryFeedback(error?.message || "No se pudo guardar el archivo.");
  }
}

function showHistoryFeedback(message) {
  const toolbar = document.querySelector(
    "#producto-historial .historial-toolbar",
  );
  if (!toolbar) return;
  let feedback = document.getElementById("historial-feedback");
  if (!feedback) {
    feedback = document.createElement("div");
    feedback.id = "historial-feedback";
    feedback.className = "form-feedback";
    feedback.setAttribute("role", "status");
    toolbar.append(feedback);
  }
  feedback.textContent = message;
}

export const __testHooks = {
  openProductHistory,
  loadProductHistory,
  handleExport,
};

if (document.readyState === "loading") {
  document.addEventListener("DOMContentLoaded", () => {
    if (document.getElementById("producto-historial")) {
      void initHistorialView();
    }
  });
} else if (document.getElementById("producto-historial")) {
  void initHistorialView();
}
