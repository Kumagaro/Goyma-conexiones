import { createCombobox } from "../components/combobox.js";
import { confirmDialog } from "../components/confirmDialog.js";
import {
  changeInvoiceStatus,
  createEntryInvoice,
  createGuaranteeInvoice,
  createInstallationInvoice,
  createLossInvoice,
  createSaleInvoice,
  createServiceInvoice,
  deleteInvoice,
  downloadInvoiceAttachment,
  getInvoice,
  listInvoices,
  saveInvoiceAttachment,
} from "../services/invoicesService.js";
import { getClientDetail, searchClients } from "../services/clientsService.js";
import { searchSuppliers } from "../services/suppliersService.js";
import { formatDate, formatMoney } from "../utils/formatters.js";
import {
  ASSOCIATED_STATUS_LABELS,
  ENTRY_STATUS_LABELS,
  INVOICE_TYPE_LABELS,
  SALE_STATUS_LABELS,
  invoiceStatusLabel,
} from "../utils/invoiceLabels.js";
import { showCleanupWarning } from "../utils/cleanupWarning.js";
import { debounce } from "../utils/requestGuard.js";

// Estado local del submódulo de facturas de entrada.
let entryInvoices = [];
let currentEntryId = null;
let selectedSupplier = null;
let entryLines = [];
let pendingAttachmentPath = null;
let downloadDestinationOverride = null;

const PAYMENT_LABELS = {
  cash: "Efectivo",
  transfer: "Transferencia",
  card: "Tarjeta",
  zelle: "Zelle",
};

export function initFacturasView() {
  return Promise.all([
    initEntryInvoicesView(),
    initSaleInvoicesView(),
    initServiceInvoicesView(),
    initGuaranteeInvoicesView(),
    initInstallationInvoicesView(),
    initLossInvoicesView(),
    initGeneralInvoicesView(),
  ]);
}

export function initSaleInvoicesView() {
  removeSaleInlineHandlers();
  bindSalePageChangeListener();
  bindSaleTabs();
  bindSaleSearch();
  bindSaleEstado();
  bindClientCombobox();
  bindSaleProductCombobox();
  bindSaleForm();
  bindSaleTableActions();
  bindSaleDetailActions();
  updateSalePaymentVisibility();
  return loadSaleInvoices();
}

export function initEntryInvoicesView() {
  removeLegacyInlineHandlers();
  bindPageChangeListener();
  bindEntryTabs();
  bindEntrySearch();
  bindEntryEstado();
  bindSupplierCombobox();
  bindProductCombobox();
  bindEntryForm();
  bindEntryTableActions();
  bindEntryDetailActions();
  bindAttachmentInput();
  updatePaymentVisibility();
  return loadEntryInvoices();
}

function removeLegacyInlineHandlers() {
  const root = document.getElementById("page-facturas-entrada");
  if (!root) return;
  root
    .querySelectorAll("[onclick]")
    .forEach((el) => el.removeAttribute("onclick"));
  root
    .querySelectorAll("[oninput]")
    .forEach((el) => el.removeAttribute("oninput"));
  root
    .querySelectorAll("[onchange]")
    .forEach((el) => el.removeAttribute("onchange"));
  root
    .querySelectorAll("[onsubmit]")
    .forEach((el) => el.removeAttribute("onsubmit"));
  root
    .querySelectorAll("[ondragover]")
    .forEach((el) => el.removeAttribute("ondragover"));
  root
    .querySelectorAll("[ondragleave]")
    .forEach((el) => el.removeAttribute("ondragleave"));
  root
    .querySelectorAll("[ondrop]")
    .forEach((el) => el.removeAttribute("ondrop"));
}

function bindPageChangeListener() {
  if (window.__facturasEntryPageChangeBound) return;
  window.addEventListener("page:change", (event) => {
    if (event.detail?.pageId !== "facturas-entrada") return;
    void initEntryInvoicesView();
  });
  window.__facturasEntryPageChangeBound = true;
}

function bindEntryTabs() {
  document
    .getElementById("tab-factura-entrada-list")
    ?.addEventListener("click", () => {
      showEntryTab("list", document.getElementById("tab-factura-entrada-list"));
    });
  document
    .getElementById("tab-factura-entrada-create")
    ?.addEventListener("click", () => {
      showEntryTab(
        "create",
        document.getElementById("tab-factura-entrada-create"),
      );
    });
  document
    .getElementById("factura-entrada-cancelar")
    ?.addEventListener("click", () => {
      showEntryTab("list", document.getElementById("tab-factura-entrada-list"));
    });
  document
    .getElementById("factura-entrada-volver")
    ?.addEventListener("click", () => {
      showEntryTab("list", document.getElementById("tab-factura-entrada-list"));
    });
  document
    .getElementById("factura-entrada-editar")
    ?.addEventListener("click", () => {
      showEntryTab(
        "create",
        document.getElementById("tab-factura-entrada-create"),
      );
    });
}

export function showEntryTab(tab, button = null) {
  const tabs = document.querySelectorAll(
    "#page-facturas-entrada .view-tabs .tab",
  );
  tabs.forEach((tabButton) => {
    const isActive =
      tabButton === button || tabButton.id === `tab-factura-entrada-${tab}`;
    tabButton.classList.toggle("active", isActive);
  });
  const subviews = document.querySelectorAll(
    "#page-facturas-entrada .config-subview",
  );
  subviews.forEach((view) => {
    const isActive = view.id === `factura-entrada-${tab}`;
    view.classList.toggle("active", isActive);
  });
  const searchBar = document.getElementById("factura-entrada-search-bar");
  if (searchBar) searchBar.style.display = tab === "list" ? "" : "none";
}

function bindEntrySearch() {
  const searchInput = document.getElementById("factura-entrada-search");
  if (!searchInput || searchInput.dataset.entryBound === "1") return;
  searchInput.dataset.entryBound = "1";
  const debounced = debounce((query) => {
    void loadEntryInvoices(query);
  }, 300);
  searchInput.addEventListener("input", (event) => {
    debounced(String(event.target.value ?? "").trim());
  });
}

function bindEntryEstado() {
  const estado = document.getElementById("factura-estado");
  if (!estado || estado.dataset.entryBound === "1") return;
  estado.dataset.entryBound = "1";
  estado.addEventListener("change", updatePaymentVisibility);
}

function updatePaymentVisibility() {
  const estado = document.getElementById("factura-estado");
  const metodo = document.getElementById("factura-metodo-pago");
  if (!estado || !metodo) return;
  const visible = estado.value === "completed";
  const display = visible ? "" : "none";
  metodo.style.display = display;
  const wrapper = metodo.closest(".field");
  if (wrapper) wrapper.style.display = display;
}

// El buscador de proveedor usa el servicio con relevancia del backend.
// Se mejora el input existente en lugar de reemplazarlo para conservar su id.
function bindSupplierCombobox() {
  const input = document.getElementById("factura-proveedor");
  if (!input || input.dataset.entryBound === "1") return;
  input.dataset.entryBound = "1";
  input.setAttribute("role", "combobox");
  input.setAttribute("aria-autocomplete", "list");
  input.setAttribute("aria-expanded", "false");
  input.removeAttribute("list");
  input.setAttribute("autocomplete", "off");
  input.setAttribute("placeholder", "Escribe para buscar un proveedor...");
  const debouncedLookup = debounce((query) => {
    void searchSuppliers(query).catch(() => {});
  }, 300);
  input.addEventListener("input", (event) => {
    selectedSupplier = null;
    debouncedLookup(String(event.target.value ?? "").trim());
  });

  document
    .getElementById("factura-proveedor-add")
    ?.addEventListener("click", openSupplierForm);
  // Botón "+" del HTML real (sin id propio): el hermano del input.
  input
    .closest(".autocomplete-row")
    ?.querySelector(".icon-btn")
    ?.addEventListener("click", openSupplierForm);
}

function openSupplierForm() {
  document.dispatchEvent(new CustomEvent("entry:openSupplierForm"));
  if (typeof window.showPage === "function") {
    window.showPage("proveedores");
  }
}

// El buscador de productos usa el comando con relevancia del backend.
function bindProductCombobox() {
  const input = document.getElementById("factura-buscar-producto");
  if (!input || input.dataset.entryBound === "1") return;
  input.dataset.entryBound = "1";
  input.setAttribute("role", "combobox");
  input.setAttribute("aria-autocomplete", "list");
  input.setAttribute("aria-expanded", "false");
  input.removeAttribute("list");
  input.setAttribute("autocomplete", "off");
  const debouncedLookup = debounce((query) => {
    void searchProductsForInvoice(query).catch(() => {});
  }, 300);
  input.addEventListener("input", (event) => {
    debouncedLookup(String(event.target.value ?? "").trim());
  });
  document
    .getElementById("factura-producto-add")
    ?.addEventListener("click", () => {
      void addProductRowFromSearch();
    });
}

async function searchProductsForInvoice(query) {
  if (!query) return [];
  const { invoke } = await import("../services/invoke.js");
  const result = await invoke("search_products_for_invoice", {
    query,
  }).catch(() => []);
  return Array.isArray(result) ? result : [];
}

function addProductRowFromSearch() {
  const input = document.getElementById("factura-buscar-producto");
  const name = String(input?.value ?? "").trim();
  if (!name) {
    input?.focus();
    return;
  }
  entryLines.push({ productName: name, quantity: "1", unitPrice: "0" });
  input.value = "";
  renderEntryLines();
}

function bindEntryForm() {
  const form =
    document.getElementById("factura-entrada-form") ??
    document.querySelector("#factura-entrada-create form");
  if (!form || form.dataset.entryBound === "1") return;
  form.dataset.entryBound = "1";
  form.addEventListener("submit", (event) => {
    event.preventDefault();
    void handleEntrySubmit();
  });
  document
    .getElementById("factura-agregar-fila")
    ?.addEventListener("click", () => {
      entryLines.push({ productName: "", quantity: "1", unitPrice: "0" });
      renderEntryLines();
    });
}

function bindEntryTableActions() {
  const body = document.getElementById("facturas-entrada-body");
  if (!body || body.dataset.entryBound === "1") return;
  body.dataset.entryBound = "1";
  body.addEventListener("click", (event) => {
    const action = event.target.closest("[data-entry-action]");
    if (!action) return;
    const invoiceId = Number(action.dataset.entryId);
    if (!Number.isFinite(invoiceId)) return;
    if (action.dataset.entryAction === "detail") {
      void openEntryDetail(invoiceId);
    } else if (action.dataset.entryAction === "delete") {
      void handleDeleteEntry(invoiceId);
    }
  });
}

function bindEntryDetailActions() {
  document
    .getElementById("factura-entrada-guardar-equipo")
    ?.addEventListener("click", () => {
      void handleDownloadAttachment();
    });
  document
    .getElementById("factura-entrada-exportar")
    ?.addEventListener("click", () => {
      const detail = currentEntryDetail;
      if (detail && typeof window.exportarFacturaPDF === "function") {
        window.exportarFacturaPDF(detail);
      }
    });
  document
    .getElementById("factura-entrada-eliminar")
    ?.addEventListener("click", () => {
      if (currentEntryId != null) void handleDeleteEntry(currentEntryId);
    });
}

function bindAttachmentInput() {
  const input = document.getElementById("factura-archivo-input");
  if (input && input.dataset.entryBound !== "1") {
    input.dataset.entryBound = "1";
    input.addEventListener("change", () => {
      const file = input.files?.[0];
      if (!file) return;
      // En Tauri el backend recibe la ruta; en el navegador se guarda el nombre.
      pendingAttachmentPath = file.path ?? file.name ?? null;
      const label = document.getElementById("factura-archivo-nombre");
      if (label) label.textContent = file.name ?? "";
      document.getElementById("factura-dropzone")?.classList.add("has-file");
    });
  }
  const dropzone = document.getElementById("factura-dropzone");
  if (dropzone && dropzone.dataset.entryBound !== "1") {
    dropzone.dataset.entryBound = "1";
    dropzone.addEventListener("click", () => {
      document.getElementById("factura-archivo-input")?.click();
    });
    dropzone.addEventListener("dragover", (event) => {
      event.preventDefault();
      dropzone.classList.add("dragover");
    });
    dropzone.addEventListener("dragleave", () => {
      dropzone.classList.remove("dragover");
    });
    dropzone.addEventListener("drop", (event) => {
      event.preventDefault();
      dropzone.classList.remove("dragover");
      const file = event.dataTransfer?.files?.[0];
      if (!file) return;
      pendingAttachmentPath = file.path ?? file.name ?? null;
      const label = document.getElementById("factura-archivo-nombre");
      if (label) label.textContent = file.name ?? "";
      dropzone.classList.add("has-file");
    });
  }
  document
    .getElementById("factura-archivo-quitar")
    ?.addEventListener("click", (event) => {
      event.stopPropagation();
      pendingAttachmentPath = null;
      if (input) input.value = "";
      const label = document.getElementById("factura-archivo-nombre");
      if (label) label.textContent = "";
      dropzone?.classList.remove("has-file");
    });
}

let currentEntryDetail = null;

async function loadEntryInvoices(query = "") {
  const body = document.getElementById("facturas-entrada-body");
  if (!body) return;
  body.replaceChildren();
  const loadingRow = document.createElement("tr");
  const loadingCell = document.createElement("td");
  loadingCell.textContent = "Cargando facturas de entrada…";
  loadingRow.append(loadingCell);
  body.append(loadingRow);

  try {
    const filters = query
      ? { invoiceType: "E", search: query }
      : { invoiceType: "E" };
    const response = await listInvoices(filters);
    const items = Array.isArray(response) ? response : (response?.items ?? []);
    entryInvoices = items.filter((item) => (item?.invoiceType ?? "E") === "E");
    renderEntryList(entryInvoices);
  } catch (error) {
    body.replaceChildren();
    const row = document.createElement("tr");
    const cell = document.createElement("td");
    cell.textContent = "No se pudieron cargar las facturas de entrada.";
    row.append(cell);
    body.append(row);
    showEntryFeedback("No se pudieron cargar las facturas de entrada.");
  }
}

function renderEntryList(items) {
  const body = document.getElementById("facturas-entrada-body");
  if (!body) return;
  body.replaceChildren();
  if (!items.length) {
    const row = document.createElement("tr");
    const cell = document.createElement("td");
    cell.textContent = "Aún no hay facturas de entrada.";
    row.append(cell);
    body.append(row);
    return;
  }
  for (const item of items) {
    const row = document.createElement("tr");
    const numberCell = document.createElement("td");
    numberCell.textContent = item?.number ?? "—";
    const supplierCell = document.createElement("td");
    supplierCell.textContent = item?.contactName ?? "—";
    const dateCell = document.createElement("td");
    dateCell.textContent = item?.date ? formatDate(item.date) : "—";
    const totalCell = document.createElement("td");
    totalCell.textContent = formatMoney(item?.totalCents ?? 0);
    const statusCell = document.createElement("td");
    statusCell.textContent =
      ENTRY_STATUS_LABELS[item?.status] ?? item?.status ?? "—";
    const actionsCell = document.createElement("td");
    const detailButton = document.createElement("button");
    detailButton.type = "button";
    detailButton.textContent = "Ver";
    detailButton.dataset.entryAction = "detail";
    detailButton.dataset.entryId = String(item?.id ?? "");
    const deleteButton = document.createElement("button");
    deleteButton.type = "button";
    deleteButton.textContent = "Eliminar";
    deleteButton.dataset.entryAction = "delete";
    deleteButton.dataset.entryId = String(item?.id ?? "");
    actionsCell.append(detailButton, deleteButton);
    row.append(
      numberCell,
      supplierCell,
      dateCell,
      totalCell,
      statusCell,
      actionsCell,
    );
    body.append(row);
  }
}

function renderEntryLines() {
  const container = document.getElementById("factura-lineas-body");
  if (!container) return;
  container.replaceChildren();
  entryLines.forEach((line, index) => {
    const row = document.createElement("div");
    row.className = "li-row li-body";
    const position = document.createElement("div");
    position.textContent = String(index + 1);
    const name = document.createElement("div");
    name.textContent = line.productName ?? line.productId ?? "";
    const quantity = document.createElement("div");
    quantity.textContent = line.quantity ?? "";
    const cost = document.createElement("div");
    cost.textContent = line.unitPrice ?? "";
    row.append(position, name, quantity, cost);
    container.append(row);
  });
}

async function handleEntrySubmit() {
  const supplierId =
    selectedSupplier?.id ?? selectedSupplier?.supplierId ?? null;
  const invoiceDate =
    document.getElementById("factura-fecha-emision")?.value ?? "";
  const status = document.getElementById("factura-estado")?.value ?? "pending";
  const paymentMethod =
    document.getElementById("factura-metodo-pago")?.value ?? undefined;
  const notes =
    document.getElementById("factura-observaciones")?.value.trim() || undefined;

  if (!supplierId || !invoiceDate || !entryLines.length) {
    showEntryFeedback(
      "Completa el proveedor, la fecha y al menos un producto.",
    );
    return;
  }

  const input = {
    supplierId: Number(supplierId),
    invoiceDate,
    status,
    ...(notes ? { notes } : {}),
    ...(status === "completed" && paymentMethod ? { paymentMethod } : {}),
    lines: entryLines.map((line) => ({
      productId: Number(line.productId),
      quantity: String(line.quantity ?? ""),
      unitPrice: String(line.unitPrice ?? ""),
    })),
  };

  try {
    const result = await createEntryInvoice(input);
    if (pendingAttachmentPath && result?.invoiceId != null) {
      try {
        await saveInvoiceAttachment(result.invoiceId, pendingAttachmentPath);
      } catch {
        showEntryFeedback(
          "La factura se guardó pero no se pudo adjuntar el PDF.",
        );
      }
    }
    showCleanupWarning(
      document.getElementById("page-facturas-entrada"),
      "facturas-entrada-cleanup-warning",
      result?.expiredCleanupWarning,
    );
    pendingAttachmentPath = null;
    entryLines = [];
    renderEntryLines();
    await loadEntryInvoices();
    showEntryTab("list", document.getElementById("tab-factura-entrada-list"));
  } catch (error) {
    showEntryFeedback(error?.message || "No se pudo guardar la factura.");
  }
}

export async function openEntryDetail(invoiceId) {
  try {
    const detail = await getInvoice(Number(invoiceId));
    currentEntryId = Number(invoiceId);
    currentEntryDetail = detail;
    renderEntryDetail(detail);
    showEntryTab("ver", null);
  } catch (error) {
    showEntryFeedback(error?.message || "No se pudo abrir la factura.");
  }
}

function renderEntryDetail(detail) {
  setText("factura-ver-numero-badge", detail?.number ?? "");
  setText("factura-ver-titulo", `Factura de Entrada ${detail?.number ?? ""}`);
  setText("factura-ver-estado-txt", ENTRY_STATUS_LABELS[detail?.status] ?? "—");
  setText(
    "factura-ver-estado-hero",
    ENTRY_STATUS_LABELS[detail?.status] ?? "—",
  );
  setText(
    "factura-ver-metodo",
    detail?.paymentMethod
      ? (PAYMENT_LABELS[detail.paymentMethod] ?? detail.paymentMethod)
      : "—",
  );
  setText("factura-ver-total", formatMoney(detail?.totalCents ?? 0));
  setText("factura-ver-proveedor", detail?.contactFullName ?? "—");
  setText("factura-ver-numero", detail?.number ?? "");
  setText("factura-ver-emision", detail?.date ? formatDate(detail.date) : "—");
  setText("factura-ver-subtotal", formatMoney(detail?.totalCents ?? 0));
  setText(
    "factura-ver-archivo",
    detail?.attachmentOriginalName ?? "Sin archivo adjunto",
  );
  setText("factura-ver-observaciones", detail?.notes ?? "—");
  setText("factura-ver-descripcion", detail?.notes ?? "—");

  const productos = document.getElementById("factura-ver-productos");
  if (productos) {
    productos.replaceChildren();
    for (const [index, line] of (detail?.lines ?? []).entries()) {
      const row = document.createElement("div");
      row.className = "li-row li-body li-body-readonly";
      const position = document.createElement("div");
      position.textContent = String(index + 1);
      const name = document.createElement("div");
      name.textContent = line?.productName ?? "";
      const quantity = document.createElement("div");
      quantity.textContent = String(line?.quantity ?? "");
      const unit = document.createElement("div");
      unit.textContent = line?.unitAbbreviation ?? line?.unitName ?? "—";
      const cost = document.createElement("div");
      cost.textContent = formatMoney(line?.unitPriceCents ?? 0);
      const subtotal = document.createElement("div");
      subtotal.textContent = formatMoney(line?.subtotalCents ?? 0);
      row.append(position, name, quantity, unit, cost, subtotal);
      productos.append(row);
    }
  }

  // Solo las facturas Por Pagar se pueden editar (FE-04).
  const editButton = document.getElementById("factura-entrada-editar");
  if (editButton) editButton.hidden = detail?.status !== "pending";
}

function setText(id, value) {
  const el = document.getElementById(id);
  if (el) el.textContent = value;
}

async function handleDownloadAttachment() {
  if (currentEntryId == null) return;
  const destination = await resolveDownloadDestination();
  if (!destination) return;
  try {
    await downloadInvoiceAttachment(currentEntryId, destination);
  } catch (error) {
    showEntryFeedback(error?.message || "No se pudo guardar el archivo.");
  }
}

async function resolveDownloadDestination() {
  if (downloadDestinationOverride) return downloadDestinationOverride;
  try {
    const path = await window.__TAURI__?.dialog?.save?.();
    return path ?? null;
  } catch {
    return null;
  }
}

async function handleDeleteEntry(invoiceId) {
  const confirmed = await confirmDialog({
    title: "Eliminar factura",
    message: "¿Deseas eliminar esta factura de entrada?",
    confirmText: "Eliminar",
  });
  if (!confirmed) return;
  try {
    const deleted = await deleteInvoice(Number(invoiceId));
    showCleanupWarning(
      document.getElementById("page-facturas-entrada"),
      "facturas-entrada-cleanup-warning",
      deleted?.expiredCleanupWarning,
    );
    if (currentEntryId === Number(invoiceId)) {
      currentEntryId = null;
      currentEntryDetail = null;
    }
    await loadEntryInvoices();
    showEntryTab("list", document.getElementById("tab-factura-entrada-list"));
  } catch (error) {
    showEntryFeedback(error?.message || "No se pudo eliminar la factura.");
  }
}

// ===================== Submódulo: facturas de venta =====================

let saleInvoices = [];
let currentSaleId = null;
let selectedClient = null;
let saleLines = [];
let currentSaleDetail = null;

function removeSaleInlineHandlers() {
  const root = document.getElementById("page-facturas-venta");
  if (!root) return;
  root
    .querySelectorAll("[onclick]")
    .forEach((el) => el.removeAttribute("onclick"));
  root
    .querySelectorAll("[oninput]")
    .forEach((el) => el.removeAttribute("oninput"));
  root
    .querySelectorAll("[onchange]")
    .forEach((el) => el.removeAttribute("onchange"));
  root
    .querySelectorAll("[onsubmit]")
    .forEach((el) => el.removeAttribute("onsubmit"));
}

function bindSalePageChangeListener() {
  if (window.__facturasSalePageChangeBound) return;
  window.addEventListener("page:change", (event) => {
    if (event.detail?.pageId !== "facturas-venta") return;
    void initSaleInvoicesView();
  });
  window.__facturasSalePageChangeBound = true;
}

function bindSaleTabs() {
  document
    .getElementById("tab-factura-venta-list")
    ?.addEventListener("click", () => {
      showSaleTab("list", document.getElementById("tab-factura-venta-list"));
    });
  document
    .getElementById("tab-factura-venta-create")
    ?.addEventListener("click", () => {
      showSaleTab(
        "create",
        document.getElementById("tab-factura-venta-create"),
      );
    });
  document
    .getElementById("factura-venta-cancelar")
    ?.addEventListener("click", () => {
      showSaleTab("list", document.getElementById("tab-factura-venta-list"));
    });
  document
    .getElementById("factura-venta-volver")
    ?.addEventListener("click", () => {
      showSaleTab("list", document.getElementById("tab-factura-venta-list"));
    });
  document
    .getElementById("factura-venta-editar")
    ?.addEventListener("click", () => {
      showSaleTab(
        "create",
        document.getElementById("tab-factura-venta-create"),
      );
    });
}

export function showSaleTab(tab, button = null) {
  const tabs = document.querySelectorAll(
    "#page-facturas-venta .view-tabs .tab",
  );
  tabs.forEach((tabButton) => {
    const isActive =
      tabButton === button || tabButton.id === `tab-factura-venta-${tab}`;
    tabButton.classList.toggle("active", isActive);
  });
  const subviews = document.querySelectorAll(
    "#page-facturas-venta .config-subview",
  );
  subviews.forEach((view) => {
    const isActive = view.id === `factura-venta-${tab}`;
    view.classList.toggle("active", isActive);
  });
  const searchBar = document.getElementById("factura-venta-search-bar");
  if (searchBar) searchBar.style.display = tab === "list" ? "" : "none";
}

function bindSaleSearch() {
  const searchInput = document.getElementById("factura-venta-search");
  if (!searchInput || searchInput.dataset.saleBound === "1") return;
  searchInput.dataset.saleBound = "1";
  const debounced = debounce((query) => {
    void loadSaleInvoices(query);
  }, 300);
  searchInput.addEventListener("input", (event) => {
    debounced(String(event.target.value ?? "").trim());
  });
}

function bindSaleEstado() {
  const estado = document.getElementById("factura-venta-estado");
  if (!estado || estado.dataset.saleBound === "1") return;
  estado.dataset.saleBound = "1";
  estado.addEventListener("change", updateSalePaymentVisibility);
}

function updateSalePaymentVisibility() {
  const estado = document.getElementById("factura-venta-estado");
  const metodo = document.getElementById("factura-venta-metodo-pago");
  if (!estado || !metodo) return;
  const visible = estado.value === "completed";
  const display = visible ? "" : "none";
  metodo.style.display = display;
  const wrapper = metodo.closest(".field");
  if (wrapper) wrapper.style.display = display;
}

// El buscador de cliente usa el servicio con relevancia del backend.
function bindClientCombobox() {
  const input = document.getElementById("factura-venta-cliente");
  if (!input || input.dataset.saleBound === "1") return;
  input.dataset.saleBound = "1";
  input.setAttribute("role", "combobox");
  input.setAttribute("aria-autocomplete", "list");
  input.setAttribute("aria-expanded", "false");
  input.removeAttribute("list");
  input.setAttribute("autocomplete", "off");
  input.setAttribute("placeholder", "Escribe para buscar un cliente...");
  const debouncedLookup = debounce((query) => {
    void searchClients(query).catch(() => {});
  }, 300);
  input.addEventListener("input", (event) => {
    selectedClient = null;
    debouncedLookup(String(event.target.value ?? "").trim());
  });

  document
    .getElementById("factura-venta-cliente-add")
    ?.addEventListener("click", openClientForm);
  input
    .closest(".autocomplete-row")
    ?.querySelector(".icon-btn")
    ?.addEventListener("click", openClientForm);
}

function openClientForm() {
  document.dispatchEvent(new CustomEvent("sale:openClientForm"));
  if (typeof window.showPage === "function") {
    window.showPage("clientes");
  }
}

function bindSaleProductCombobox() {
  const input = document.getElementById("factura-venta-buscar-producto");
  if (!input || input.dataset.saleBound === "1") return;
  input.dataset.saleBound = "1";
  input.setAttribute("role", "combobox");
  input.setAttribute("aria-autocomplete", "list");
  input.setAttribute("aria-expanded", "false");
  input.removeAttribute("list");
  input.setAttribute("autocomplete", "off");
  const debouncedLookup = debounce((query) => {
    void searchProductsForInvoice(query).catch(() => {});
  }, 300);
  input.addEventListener("input", (event) => {
    debouncedLookup(String(event.target.value ?? "").trim());
  });
  document
    .getElementById("factura-venta-producto-add")
    ?.addEventListener("click", () => {
      void addSaleProductRowFromSearch();
    });
}

function addSaleProductRowFromSearch() {
  const input = document.getElementById("factura-venta-buscar-producto");
  const name = String(input?.value ?? "").trim();
  if (!name) {
    input?.focus();
    return;
  }
  saleLines.push({ productName: name, quantity: "1" });
  input.value = "";
  renderSaleLines();
}

function bindSaleForm() {
  const form =
    document.getElementById("factura-venta-form") ??
    document.querySelector("#factura-venta-create form");
  if (!form || form.dataset.saleBound === "1") return;
  form.dataset.saleBound = "1";
  form.addEventListener("submit", (event) => {
    event.preventDefault();
    void handleSaleSubmit();
  });
  document
    .getElementById("factura-venta-agregar-fila")
    ?.addEventListener("click", () => {
      saleLines.push({ productName: "", quantity: "1" });
      renderSaleLines();
    });
}

function bindSaleTableActions() {
  const body = document.getElementById("facturas-venta-body");
  if (!body || body.dataset.saleBound === "1") return;
  body.dataset.saleBound = "1";
  body.addEventListener("click", (event) => {
    const action = event.target.closest("[data-sale-action]");
    if (!action) return;
    const invoiceId = Number(action.dataset.saleId);
    if (!Number.isFinite(invoiceId)) return;
    if (action.dataset.saleAction === "detail") {
      void openSaleDetail(invoiceId);
    } else if (action.dataset.saleAction === "delete") {
      void handleDeleteSale(invoiceId);
    }
  });
}

function bindSaleDetailActions() {
  document
    .getElementById("factura-venta-exportar")
    ?.addEventListener("click", () => {
      const detail = currentSaleDetail;
      if (detail && typeof window.exportarFacturaVentaPDF === "function") {
        window.exportarFacturaVentaPDF(detail);
      }
    });
  document
    .getElementById("factura-venta-eliminar")
    ?.addEventListener("click", () => {
      if (currentSaleId != null) void handleDeleteSale(currentSaleId);
    });
}

async function loadSaleInvoices(query = "") {
  const body = document.getElementById("facturas-venta-body");
  if (!body) return;
  body.replaceChildren();
  const loadingRow = document.createElement("tr");
  const loadingCell = document.createElement("td");
  loadingCell.textContent = "Cargando facturas de venta…";
  loadingRow.append(loadingCell);
  body.append(loadingRow);

  try {
    const filters = query
      ? { invoiceType: "V", search: query }
      : { invoiceType: "V" };
    const response = await listInvoices(filters);
    const items = Array.isArray(response) ? response : (response?.items ?? []);
    saleInvoices = items.filter((item) => (item?.invoiceType ?? "V") === "V");
    renderSaleList(saleInvoices);
  } catch (error) {
    body.replaceChildren();
    const row = document.createElement("tr");
    const cell = document.createElement("td");
    cell.textContent = "No se pudieron cargar las facturas de venta.";
    row.append(cell);
    body.append(row);
    showSaleFeedback("No se pudieron cargar las facturas de venta.");
  }
}

function renderSaleList(items) {
  const body = document.getElementById("facturas-venta-body");
  if (!body) return;
  body.replaceChildren();
  if (!items.length) {
    const row = document.createElement("tr");
    const cell = document.createElement("td");
    cell.textContent = "Aún no hay facturas de venta.";
    row.append(cell);
    body.append(row);
    return;
  }
  for (const item of items) {
    const row = document.createElement("tr");
    const numberCell = document.createElement("td");
    numberCell.textContent = item?.number ?? "—";
    const clientCell = document.createElement("td");
    clientCell.textContent = item?.contactName ?? "—";
    const dateCell = document.createElement("td");
    dateCell.textContent = item?.date ? formatDate(item.date) : "—";
    const totalCell = document.createElement("td");
    totalCell.textContent = formatMoney(item?.totalCents ?? 0);
    const statusCell = document.createElement("td");
    statusCell.textContent =
      SALE_STATUS_LABELS[item?.status] ?? item?.status ?? "—";
    const actionsCell = document.createElement("td");
    const detailButton = document.createElement("button");
    detailButton.type = "button";
    detailButton.textContent = "Ver";
    detailButton.dataset.saleAction = "detail";
    detailButton.dataset.saleId = String(item?.id ?? "");
    const deleteButton = document.createElement("button");
    deleteButton.type = "button";
    deleteButton.textContent = "Eliminar";
    deleteButton.dataset.saleAction = "delete";
    deleteButton.dataset.saleId = String(item?.id ?? "");
    actionsCell.append(detailButton, deleteButton);
    row.append(
      numberCell,
      clientCell,
      dateCell,
      totalCell,
      statusCell,
      actionsCell,
    );
    body.append(row);
  }
}

function renderSaleLines() {
  const container = document.getElementById("factura-venta-lineas-body");
  if (!container) return;
  container.replaceChildren();
  saleLines.forEach((line, index) => {
    const row = document.createElement("div");
    row.className = "li-row li-body";
    const position = document.createElement("div");
    position.textContent = String(index + 1);
    const name = document.createElement("div");
    name.textContent = line.productName ?? line.productId ?? "";
    const quantity = document.createElement("div");
    quantity.textContent = line.quantity ?? "";
    const price = document.createElement("div");
    price.textContent = line.unitPrice ?? "";
    row.append(position, name, quantity, price);
    container.append(row);
  });
}

async function handleSaleSubmit() {
  const clientId = selectedClient?.id ?? selectedClient?.clientId ?? null;
  const invoiceDate =
    document.getElementById("factura-venta-fecha-emision")?.value ?? "";
  const status =
    document.getElementById("factura-venta-estado")?.value ?? "pending";
  const paymentMethod =
    document.getElementById("factura-venta-metodo-pago")?.value ?? undefined;
  const notes =
    document.getElementById("factura-venta-descripcion")?.value.trim() ||
    undefined;

  if (!clientId || !invoiceDate || !saleLines.length) {
    showSaleFeedback("Completa el cliente, la fecha y al menos un producto.");
    return;
  }

  const input = {
    clientId: Number(clientId),
    invoiceDate,
    status,
    ...(notes ? { notes } : {}),
    ...(status === "completed" && paymentMethod ? { paymentMethod } : {}),
    lines: saleLines.map((line) => ({
      productId: Number(line.productId),
      quantity: String(line.quantity ?? ""),
    })),
  };

  try {
    const result = await createSaleInvoice(input);
    showCleanupWarning(
      document.getElementById("page-facturas-venta"),
      "facturas-venta-cleanup-warning",
      result?.expiredCleanupWarning,
    );
    saleLines = [];
    renderSaleLines();
    await loadSaleInvoices();
    showSaleTab("list", document.getElementById("tab-factura-venta-list"));
  } catch (error) {
    showSaleFeedback(error?.message || "No se pudo guardar la factura.");
  }
}

export async function openSaleDetail(invoiceId) {
  try {
    const detail = await getInvoice(Number(invoiceId));
    currentSaleId = Number(invoiceId);
    currentSaleDetail = detail;
    renderSaleDetail(detail);
    showSaleTab("ver", null);
  } catch (error) {
    showSaleFeedback(error?.message || "No se pudo abrir la factura.");
  }
}

function renderSaleDetail(detail) {
  setText("factura-venta-ver-numero-badge", detail?.number ?? "");
  setText(
    "factura-venta-ver-titulo",
    `Factura de Venta ${detail?.number ?? ""}`,
  );
  setText(
    "factura-venta-ver-estado-txt",
    SALE_STATUS_LABELS[detail?.status] ?? "—",
  );
  setText(
    "factura-venta-ver-estado-hero",
    SALE_STATUS_LABELS[detail?.status] ?? "—",
  );
  setText(
    "factura-venta-ver-metodo",
    detail?.paymentMethod
      ? (PAYMENT_LABELS[detail.paymentMethod] ?? detail.paymentMethod)
      : "—",
  );
  setText("factura-venta-ver-total", formatMoney(detail?.totalCents ?? 0));
  setText("factura-venta-ver-cliente", detail?.contactFullName ?? "—");
  setText("factura-venta-ver-numero", detail?.number ?? "");
  setText(
    "factura-venta-ver-emision",
    detail?.date ? formatDate(detail.date) : "—",
  );
  setText("factura-venta-ver-subtotal", formatMoney(detail?.totalCents ?? 0));
  setText("factura-venta-ver-descripcion", detail?.notes ?? "—");

  const productos = document.getElementById("factura-venta-ver-productos");
  if (productos) {
    productos.replaceChildren();
    for (const [index, line] of (detail?.lines ?? []).entries()) {
      const row = document.createElement("div");
      row.className = "li-row li-body li-body-readonly";
      const position = document.createElement("div");
      position.textContent = String(index + 1);
      const name = document.createElement("div");
      name.textContent = line?.productName ?? "";
      const quantity = document.createElement("div");
      quantity.textContent = String(line?.quantity ?? "");
      const unit = document.createElement("div");
      unit.textContent = line?.unitAbbreviation ?? line?.unitName ?? "—";
      const price = document.createElement("div");
      price.textContent = formatMoney(line?.unitPriceCents ?? 0);
      const subtotal = document.createElement("div");
      subtotal.textContent = formatMoney(line?.subtotalCents ?? 0);
      row.append(position, name, quantity, unit, price, subtotal);
      productos.append(row);
    }
  }

  // Solo las facturas Por Cobrar se pueden editar (FS-01).
  const editButton = document.getElementById("factura-venta-editar");
  if (editButton) editButton.hidden = detail?.status !== "pending";
}

async function handleDeleteSale(invoiceId) {
  const confirmed = await confirmDialog({
    title: "Eliminar factura",
    message: "¿Deseas eliminar esta factura de venta?",
    confirmText: "Eliminar",
  });
  if (!confirmed) return;
  try {
    const deleted = await deleteInvoice(Number(invoiceId));
    showCleanupWarning(
      document.getElementById("page-facturas-venta"),
      "facturas-venta-cleanup-warning",
      deleted?.expiredCleanupWarning,
    );
    if (currentSaleId === Number(invoiceId)) {
      currentSaleId = null;
      currentSaleDetail = null;
    }
    await loadSaleInvoices();
    showSaleTab("list", document.getElementById("tab-factura-venta-list"));
  } catch (error) {
    showSaleFeedback(error?.message || "No se pudo eliminar la factura.");
  }
}

function showSaleFeedback(message) {
  const form =
    document.getElementById("factura-venta-form") ??
    document.querySelector("#factura-venta-create form");
  if (!form) return;
  let feedback = document.getElementById("factura-venta-feedback");
  if (!feedback) {
    feedback = document.createElement("div");
    feedback.id = "factura-venta-feedback";
    feedback.className = "form-feedback";
    feedback.setAttribute("role", "status");
    form.append(feedback);
  }
  feedback.textContent = message;
}

// ===================== Submódulo: facturas de servicio =====================

let serviceInvoices = [];
let currentServiceId = null;
let selectedServiceClient = null;
let serviceLines = [];
let currentServiceDetail = null;

export function initServiceInvoicesView() {
  removeServiceInlineHandlers();
  bindServicePageChangeListener();
  bindServiceTabs();
  bindServiceSearch();
  bindServiceEstado();
  bindServiceClientCombobox();
  bindServiceProductCombobox();
  bindServiceForm();
  bindServiceTableActions();
  bindServiceDetailActions();
  updateServicePaymentVisibility();
  return loadServiceInvoices();
}

function removeServiceInlineHandlers() {
  const root = document.getElementById("page-facturas-servicios");
  if (!root) return;
  root
    .querySelectorAll("[onclick]")
    .forEach((el) => el.removeAttribute("onclick"));
  root
    .querySelectorAll("[oninput]")
    .forEach((el) => el.removeAttribute("oninput"));
  root
    .querySelectorAll("[onchange]")
    .forEach((el) => el.removeAttribute("onchange"));
  root
    .querySelectorAll("[onsubmit]")
    .forEach((el) => el.removeAttribute("onsubmit"));
}

function bindServicePageChangeListener() {
  if (window.__facturasServicePageChangeBound) return;
  window.addEventListener("page:change", (event) => {
    if (event.detail?.pageId !== "facturas-servicios") return;
    void initServiceInvoicesView();
  });
  window.__facturasServicePageChangeBound = true;
}

function bindServiceTabs() {
  document
    .getElementById("tab-factura-servicios-list")
    ?.addEventListener("click", () => {
      showServiceTab(
        "list",
        document.getElementById("tab-factura-servicios-list"),
      );
    });
  document
    .getElementById("tab-factura-servicios-create")
    ?.addEventListener("click", () => {
      showServiceTab(
        "create",
        document.getElementById("tab-factura-servicios-create"),
      );
    });
  document
    .getElementById("factura-servicio-cancelar")
    ?.addEventListener("click", () => {
      showServiceTab(
        "list",
        document.getElementById("tab-factura-servicios-list"),
      );
    });
  document
    .getElementById("factura-servicio-volver")
    ?.addEventListener("click", () => {
      showServiceTab(
        "list",
        document.getElementById("tab-factura-servicios-list"),
      );
    });
  document
    .getElementById("factura-servicio-editar")
    ?.addEventListener("click", () => {
      showServiceTab(
        "create",
        document.getElementById("tab-factura-servicios-create"),
      );
    });
}

export function showServiceTab(tab, button = null) {
  const tabs = document.querySelectorAll(
    "#page-facturas-servicios .view-tabs .tab",
  );
  tabs.forEach((tabButton) => {
    const isActive =
      tabButton === button || tabButton.id === `tab-factura-servicios-${tab}`;
    tabButton.classList.toggle("active", isActive);
  });
  const subviews = document.querySelectorAll(
    "#page-facturas-servicios .config-subview",
  );
  subviews.forEach((view) => {
    const isActive = view.id === `factura-servicios-${tab}`;
    view.classList.toggle("active", isActive);
  });
  const searchBar = document.getElementById("factura-servicios-search-bar");
  if (searchBar) searchBar.style.display = tab === "list" ? "" : "none";
}

function bindServiceSearch() {
  const searchInput = document.getElementById("factura-servicios-search");
  if (!searchInput || searchInput.dataset.serviceBound === "1") return;
  searchInput.dataset.serviceBound = "1";
  const debounced = debounce((query) => {
    void loadServiceInvoices(query);
  }, 300);
  searchInput.addEventListener("input", (event) => {
    debounced(String(event.target.value ?? "").trim());
  });
}

function bindServiceEstado() {
  const estado = document.getElementById("factura-servicio-estado");
  if (!estado || estado.dataset.serviceBound === "1") return;
  estado.dataset.serviceBound = "1";
  estado.addEventListener("change", updateServicePaymentVisibility);
}

function updateServicePaymentVisibility() {
  const estado = document.getElementById("factura-servicio-estado");
  const metodo = document.getElementById("factura-servicio-metodo-pago");
  if (!estado || !metodo) return;
  const visible = estado.value === "completed";
  const display = visible ? "" : "none";
  metodo.style.display = display;
  const wrapper = metodo.closest(".field");
  if (wrapper) wrapper.style.display = display;
}

function bindServiceClientCombobox() {
  const input = document.getElementById("factura-servicio-cliente");
  if (!input || input.dataset.serviceBound === "1") return;
  input.dataset.serviceBound = "1";
  input.setAttribute("role", "combobox");
  input.setAttribute("aria-autocomplete", "list");
  input.setAttribute("aria-expanded", "false");
  input.removeAttribute("list");
  input.setAttribute("autocomplete", "off");
  input.setAttribute("placeholder", "Escribe para buscar un cliente...");
  const debouncedLookup = debounce((query) => {
    void searchClients(query).catch(() => {});
  }, 300);
  input.addEventListener("input", (event) => {
    selectedServiceClient = null;
    debouncedLookup(String(event.target.value ?? "").trim());
  });

  document
    .getElementById("factura-servicio-cliente-add")
    ?.addEventListener("click", openServiceClientForm);
  input
    .closest(".autocomplete-row")
    ?.querySelector(".icon-btn")
    ?.addEventListener("click", openServiceClientForm);
}

function openServiceClientForm() {
  document.dispatchEvent(new CustomEvent("service:openClientForm"));
  if (typeof window.showPage === "function") {
    window.showPage("clientes");
  }
}

function bindServiceProductCombobox() {
  const input = document.getElementById("factura-servicio-buscar");
  if (!input || input.dataset.serviceBound === "1") return;
  input.dataset.serviceBound = "1";
  input.setAttribute("role", "combobox");
  input.setAttribute("aria-autocomplete", "list");
  input.setAttribute("aria-expanded", "false");
  input.removeAttribute("list");
  input.setAttribute("autocomplete", "off");
  const debouncedLookup = debounce((query) => {
    void searchProductsForInvoice(query).catch(() => {});
  }, 300);
  input.addEventListener("input", (event) => {
    debouncedLookup(String(event.target.value ?? "").trim());
  });
  document
    .getElementById("factura-servicio-producto-add")
    ?.addEventListener("click", () => {
      void addServiceProductRowFromSearch();
    });
}

function addServiceProductRowFromSearch() {
  const input = document.getElementById("factura-servicio-buscar");
  const name = String(input?.value ?? "").trim();
  if (!name) {
    input?.focus();
    return;
  }
  serviceLines.push({ productName: name, quantity: "1" });
  input.value = "";
  renderServiceLines();
}

function bindServiceForm() {
  const form =
    document.getElementById("factura-servicios-form") ??
    document.querySelector("#factura-servicios-create form");
  if (!form || form.dataset.serviceBound === "1") return;
  form.dataset.serviceBound = "1";
  form.addEventListener("submit", (event) => {
    event.preventDefault();
    void handleServiceSubmit();
  });
  document
    .getElementById("factura-servicio-agregar-fila")
    ?.addEventListener("click", () => {
      serviceLines.push({ productName: "", quantity: "1" });
      renderServiceLines();
    });
}

function bindServiceTableActions() {
  const body = document.getElementById("facturas-servicios-body");
  if (!body || body.dataset.serviceBound === "1") return;
  body.dataset.serviceBound = "1";
  body.addEventListener("click", (event) => {
    const action = event.target.closest("[data-service-action]");
    if (!action) return;
    const invoiceId = Number(action.dataset.serviceId);
    if (!Number.isFinite(invoiceId)) return;
    if (action.dataset.serviceAction === "detail") {
      void openServiceDetail(invoiceId);
    } else if (action.dataset.serviceAction === "delete") {
      void handleDeleteService(invoiceId);
    }
  });
}

function bindServiceDetailActions() {
  document
    .getElementById("factura-servicio-exportar")
    ?.addEventListener("click", () => {
      const detail = currentServiceDetail;
      if (detail && typeof window.exportarFacturaServicioPDF === "function") {
        window.exportarFacturaServicioPDF(detail);
      }
    });
  document
    .getElementById("factura-servicio-eliminar")
    ?.addEventListener("click", () => {
      if (currentServiceId != null) void handleDeleteService(currentServiceId);
    });
}

async function loadServiceInvoices(query = "") {
  const body = document.getElementById("facturas-servicios-body");
  if (!body) return;
  body.replaceChildren();
  const loadingRow = document.createElement("tr");
  const loadingCell = document.createElement("td");
  loadingCell.textContent = "Cargando facturas de servicio…";
  loadingRow.append(loadingCell);
  body.append(loadingRow);

  try {
    const filters = query
      ? { invoiceType: "S", search: query }
      : { invoiceType: "S" };
    const response = await listInvoices(filters);
    const items = Array.isArray(response) ? response : (response?.items ?? []);
    serviceInvoices = items.filter(
      (item) => (item?.invoiceType ?? "S") === "S",
    );
    renderServiceList(serviceInvoices);
  } catch (error) {
    body.replaceChildren();
    const row = document.createElement("tr");
    const cell = document.createElement("td");
    cell.textContent = "No se pudieron cargar las facturas de servicio.";
    row.append(cell);
    body.append(row);
    showServiceFeedback("No se pudieron cargar las facturas de servicio.");
  }
}

function renderServiceList(items) {
  const body = document.getElementById("facturas-servicios-body");
  if (!body) return;
  body.replaceChildren();
  if (!items.length) {
    const row = document.createElement("tr");
    const cell = document.createElement("td");
    cell.textContent = "Aún no hay facturas de servicio.";
    row.append(cell);
    body.append(row);
    return;
  }
  for (const item of items) {
    const row = document.createElement("tr");
    const numberCell = document.createElement("td");
    numberCell.textContent = item?.number ?? "—";
    const clientCell = document.createElement("td");
    clientCell.textContent = item?.contactName ?? "—";
    const dateCell = document.createElement("td");
    dateCell.textContent = item?.date ? formatDate(item.date) : "—";
    const totalCell = document.createElement("td");
    totalCell.textContent = formatMoney(item?.totalCents ?? 0);
    const statusCell = document.createElement("td");
    statusCell.textContent =
      SALE_STATUS_LABELS[item?.status] ?? item?.status ?? "—";
    const actionsCell = document.createElement("td");
    const detailButton = document.createElement("button");
    detailButton.type = "button";
    detailButton.textContent = "Ver";
    detailButton.dataset.serviceAction = "detail";
    detailButton.dataset.serviceId = String(item?.id ?? "");
    const deleteButton = document.createElement("button");
    deleteButton.type = "button";
    deleteButton.textContent = "Eliminar";
    deleteButton.dataset.serviceAction = "delete";
    deleteButton.dataset.serviceId = String(item?.id ?? "");
    actionsCell.append(detailButton, deleteButton);
    row.append(
      numberCell,
      clientCell,
      dateCell,
      totalCell,
      statusCell,
      actionsCell,
    );
    body.append(row);
  }
}

function renderServiceLines() {
  const container = document.getElementById("factura-servicio-lineas-body");
  if (!container) return;
  container.replaceChildren();
  serviceLines.forEach((line, index) => {
    const row = document.createElement("div");
    row.className = "li-row li-body";
    const position = document.createElement("div");
    position.textContent = String(index + 1);
    const name = document.createElement("div");
    name.textContent = line.productName ?? line.productId ?? "";
    const quantity = document.createElement("div");
    quantity.textContent = line.quantity ?? "";
    const price = document.createElement("div");
    price.textContent = line.unitPrice ?? "";
    row.append(position, name, quantity, price);
    container.append(row);
  });
}

async function handleServiceSubmit() {
  const clientId =
    selectedServiceClient?.id ?? selectedServiceClient?.clientId ?? null;
  const invoiceDate =
    document.getElementById("factura-servicio-fecha-emision")?.value ?? "";
  const status =
    document.getElementById("factura-servicio-estado")?.value ?? "pending";
  const paymentMethod =
    document.getElementById("factura-servicio-metodo-pago")?.value ?? undefined;
  const notes =
    document.getElementById("factura-servicio-descripcion")?.value.trim() ||
    undefined;

  if (!clientId || !invoiceDate || !serviceLines.length) {
    showServiceFeedback(
      "Completa el cliente, la fecha y al menos un producto.",
    );
    return;
  }

  const input = {
    clientId: Number(clientId),
    invoiceDate,
    status,
    ...(notes ? { notes } : {}),
    ...(status === "completed" && paymentMethod ? { paymentMethod } : {}),
    lines: serviceLines.map((line) => ({
      productId: Number(line.productId),
      quantity: String(line.quantity ?? ""),
    })),
  };

  try {
    const result = await createServiceInvoice(input);
    showCleanupWarning(
      document.getElementById("page-facturas-servicios"),
      "facturas-servicios-cleanup-warning",
      result?.expiredCleanupWarning,
    );
    serviceLines = [];
    renderServiceLines();
    await loadServiceInvoices();
    showServiceTab(
      "list",
      document.getElementById("tab-factura-servicios-list"),
    );
  } catch (error) {
    showServiceFeedback(error?.message || "No se pudo guardar la factura.");
  }
}

export async function openServiceDetail(invoiceId) {
  try {
    const detail = await getInvoice(Number(invoiceId));
    currentServiceId = Number(invoiceId);
    currentServiceDetail = detail;
    renderServiceDetail(detail);
    showServiceTab("ver", null);
  } catch (error) {
    showServiceFeedback(error?.message || "No se pudo abrir la factura.");
  }
}

function renderServiceDetail(detail) {
  setText("factura-servicio-ver-numero-badge", detail?.number ?? "");
  setText(
    "factura-servicio-ver-titulo",
    `Factura de Servicio ${detail?.number ?? ""}`,
  );
  setText(
    "factura-servicio-ver-estado-txt",
    SALE_STATUS_LABELS[detail?.status] ?? "—",
  );
  setText(
    "factura-servicio-ver-estado-hero",
    SALE_STATUS_LABELS[detail?.status] ?? "—",
  );
  setText(
    "factura-servicio-ver-metodo",
    detail?.paymentMethod
      ? (PAYMENT_LABELS[detail.paymentMethod] ?? detail.paymentMethod)
      : "—",
  );
  setText("factura-servicio-ver-total", formatMoney(detail?.totalCents ?? 0));
  setText("factura-servicio-ver-cliente", detail?.contactFullName ?? "—");
  setText("factura-servicio-ver-numero", detail?.number ?? "");
  setText(
    "factura-servicio-ver-emision",
    detail?.date ? formatDate(detail.date) : "—",
  );
  setText(
    "factura-servicio-ver-subtotal",
    formatMoney(detail?.totalCents ?? 0),
  );
  setText("factura-servicio-ver-descripcion", detail?.notes ?? "—");

  const productos = document.getElementById("factura-servicio-ver-productos");
  if (productos) {
    productos.replaceChildren();
    for (const [index, line] of (detail?.lines ?? []).entries()) {
      const row = document.createElement("div");
      row.className = "li-row li-body li-body-readonly";
      const position = document.createElement("div");
      position.textContent = String(index + 1);
      const name = document.createElement("div");
      name.textContent = line?.productName ?? "";
      const quantity = document.createElement("div");
      quantity.textContent = String(line?.quantity ?? "");
      const unit = document.createElement("div");
      unit.textContent = line?.unitAbbreviation ?? line?.unitName ?? "—";
      const price = document.createElement("div");
      price.textContent = formatMoney(line?.unitPriceCents ?? 0);
      const subtotal = document.createElement("div");
      subtotal.textContent = formatMoney(line?.subtotalCents ?? 0);
      row.append(position, name, quantity, unit, price, subtotal);
      productos.append(row);
    }
  }

  // Solo las facturas Por Cobrar se pueden editar (FS-01).
  const editButton = document.getElementById("factura-servicio-editar");
  if (editButton) editButton.hidden = detail?.status !== "pending";
}

async function handleDeleteService(invoiceId) {
  const confirmed = await confirmDialog({
    title: "Eliminar factura",
    message: "¿Deseas eliminar esta factura de servicio?",
    confirmText: "Eliminar",
  });
  if (!confirmed) return;
  try {
    const deleted = await deleteInvoice(Number(invoiceId));
    showCleanupWarning(
      document.getElementById("page-facturas-servicios"),
      "facturas-servicios-cleanup-warning",
      deleted?.expiredCleanupWarning,
    );
    if (currentServiceId === Number(invoiceId)) {
      currentServiceId = null;
      currentServiceDetail = null;
    }
    await loadServiceInvoices();
    showServiceTab(
      "list",
      document.getElementById("tab-factura-servicios-list"),
    );
  } catch (error) {
    showServiceFeedback(error?.message || "No se pudo eliminar la factura.");
  }
}

function showServiceFeedback(message) {
  const form =
    document.getElementById("factura-servicios-form") ??
    document.querySelector("#factura-servicios-create form");
  if (!form) return;
  let feedback = document.getElementById("factura-servicios-feedback");
  if (!feedback) {
    feedback = document.createElement("div");
    feedback.id = "factura-servicios-feedback";
    feedback.className = "form-feedback";
    feedback.setAttribute("role", "status");
    form.append(feedback);
  }
  feedback.textContent = message;
}

// ============ Submódulos: facturas de garantía y de insumos ============
// Garantía (FS-04) e insumos (FS-05) comparten mecánica: cliente, factura
// asociada elegida en un <select>, sin método de pago y costo real.

function createAssociatedSubmodule(cfg) {
  let currentId = null;
  let selectedClient = null;
  let lines = [];
  let currentDetail = null;

  const byId = (suffix) => document.getElementById(`${cfg.field}-${suffix}`);
  const listTab = () => document.getElementById(`tab-${cfg.view}-list`);
  const createTab = () => document.getElementById(`tab-${cfg.view}-create`);

  function init() {
    removeInlineHandlers();
    bindPageChangeListener();
    bindTabs();
    bindSearch();
    bindClientCombobox();
    bindProductCombobox();
    bindForm();
    bindTableActions();
    bindDetailActions();
    return loadInvoices();
  }

  function removeInlineHandlers() {
    const root = document.getElementById(cfg.pageId);
    if (!root) return;
    for (const attr of ["onclick", "oninput", "onchange", "onsubmit"]) {
      root
        .querySelectorAll(`[${attr}]`)
        .forEach((el) => el.removeAttribute(attr));
    }
  }

  function bindPageChangeListener() {
    if (window[cfg.windowFlag]) return;
    window.addEventListener("page:change", (event) => {
      if (event.detail?.pageId !== cfg.pageKey) return;
      void init();
    });
    window[cfg.windowFlag] = true;
  }

  // Marca un elemento como enlazado; devuelve false si ya lo estaba.
  function claim(el) {
    if (!el || el.dataset.associatedBound === "1") return false;
    el.dataset.associatedBound = "1";
    return true;
  }

  function bindTabs() {
    const toList = () => showTab("list", listTab());
    const toCreate = () => showTab("create", createTab());
    for (const [el, handler] of [
      [listTab(), toList],
      [createTab(), toCreate],
      [byId("cancelar"), toList],
      [byId("volver"), toList],
      [byId("editar"), toCreate],
    ]) {
      if (claim(el)) el.addEventListener("click", handler);
    }
  }

  function showTab(tab, button = null) {
    document
      .querySelectorAll(`#${cfg.pageId} .view-tabs .tab`)
      .forEach((tabButton) => {
        const isActive =
          tabButton === button || tabButton.id === `tab-${cfg.view}-${tab}`;
        tabButton.classList.toggle("active", isActive);
      });
    document
      .querySelectorAll(`#${cfg.pageId} .config-subview`)
      .forEach((view) => {
        view.classList.toggle("active", view.id === `${cfg.view}-${tab}`);
      });
    const searchBar = document.getElementById(`${cfg.view}-search-bar`);
    if (searchBar) searchBar.style.display = tab === "list" ? "" : "none";
  }

  function bindSearch() {
    const searchInput = document.getElementById(`${cfg.view}-search`);
    if (!claim(searchInput)) return;
    const debounced = debounce((query) => {
      void loadInvoices(query);
    }, 300);
    searchInput.addEventListener("input", (event) => {
      debounced(String(event.target.value ?? "").trim());
    });
  }

  function bindClientCombobox() {
    const input = byId("cliente");
    if (!claim(input)) return;
    input.setAttribute("role", "combobox");
    input.setAttribute("aria-autocomplete", "list");
    input.setAttribute("aria-expanded", "false");
    input.removeAttribute("list");
    input.setAttribute("autocomplete", "off");
    const debouncedLookup = debounce((query) => {
      void searchClients(query).catch(() => {});
    }, 300);
    input.addEventListener("input", (event) => {
      // Cambiar el cliente invalida la factura asociada elegida (FS-04/05).
      void selectClient(null);
      debouncedLookup(String(event.target.value ?? "").trim());
    });
    byId("cliente-add")?.addEventListener("click", () => {
      document.dispatchEvent(new CustomEvent(cfg.openClientEvent));
      if (typeof window.showPage === "function") {
        window.showPage("clientes");
      }
    });
  }

  function resetAssociatedSelect(placeholder) {
    const select = document.getElementById(cfg.associatedSelectId);
    if (!select) return null;
    const empty = document.createElement("option");
    empty.value = "";
    empty.textContent = placeholder;
    select.replaceChildren(empty);
    select.value = "";
    return select;
  }

  // Fija el cliente y llena el selector con sus facturas asociables vigentes.
  async function selectClient(client) {
    selectedClient = client;
    const clientId = client?.id ?? client?.clientId ?? null;
    if (clientId == null) {
      resetAssociatedSelect("Selecciona primero un cliente");
      return;
    }
    const select = resetAssociatedSelect("Cargando facturas del cliente…");
    if (!select) return;
    try {
      const detail = await getClientDetail(Number(clientId));
      // Si el usuario cambió de cliente mientras cargaba, se descarta.
      if (selectedClient !== client) return;
      const candidates = (detail?.invoiceHistory ?? []).filter(
        (item) =>
          cfg.associatedTypes.includes(item?.invoiceType) &&
          item?.status !== "cancelled",
      );
      resetAssociatedSelect(
        candidates.length ? cfg.selectPlaceholder : cfg.selectEmpty,
      );
      for (const item of candidates) {
        const option = document.createElement("option");
        option.value = String(item.id);
        option.textContent = associatedLabel(item);
        select.append(option);
      }
    } catch {
      if (selectedClient !== client) return;
      resetAssociatedSelect("No se pudieron cargar las facturas del cliente");
    }
  }

  function bindProductCombobox() {
    const input = byId("buscar");
    if (!claim(input)) return;
    input.setAttribute("role", "combobox");
    input.setAttribute("aria-autocomplete", "list");
    input.setAttribute("aria-expanded", "false");
    input.removeAttribute("list");
    input.setAttribute("autocomplete", "off");
    const debouncedLookup = debounce((query) => {
      void searchProductsForInvoice(query).catch(() => {});
    }, 300);
    input.addEventListener("input", (event) => {
      debouncedLookup(String(event.target.value ?? "").trim());
    });
    byId("producto-add")?.addEventListener("click", () => {
      const name = String(input.value ?? "").trim();
      if (!name) {
        input.focus();
        return;
      }
      lines.push({ productName: name, quantity: "1" });
      input.value = "";
      renderLines();
    });
  }

  function bindForm() {
    const form = document.getElementById(`${cfg.view}-form`);
    if (!claim(form)) return;
    form.addEventListener("submit", (event) => {
      event.preventDefault();
      void submit();
    });
    byId("agregar-fila")?.addEventListener("click", () => {
      lines.push({ productName: "", quantity: "1" });
      renderLines();
    });
  }

  function bindTableActions() {
    const body = document.getElementById(cfg.bodyId);
    if (!claim(body)) return;
    body.addEventListener("click", (event) => {
      const action = event.target.closest("[data-associated-action]");
      if (!action) return;
      const invoiceId = Number(action.dataset.associatedId);
      if (!Number.isFinite(invoiceId)) return;
      if (action.dataset.associatedAction === "detail") {
        void openDetail(invoiceId);
      } else if (action.dataset.associatedAction === "delete") {
        void handleDelete(invoiceId);
      }
    });
  }

  function bindDetailActions() {
    const exportButton = byId("exportar");
    if (claim(exportButton)) {
      exportButton.addEventListener("click", () => {
        const exporter = window[cfg.exportFn];
        if (currentDetail && typeof exporter === "function") {
          exporter(currentDetail);
        }
      });
    }
    const deleteButton = byId("eliminar");
    if (claim(deleteButton)) {
      deleteButton.addEventListener("click", () => {
        if (currentId != null) void handleDelete(currentId);
      });
    }
  }

  async function loadInvoices(query = "") {
    const body = document.getElementById(cfg.bodyId);
    if (!body) return;
    messageRow(body, `Cargando facturas de ${cfg.noun}…`);
    try {
      const filters = query
        ? { invoiceType: cfg.invoiceType, search: query }
        : { invoiceType: cfg.invoiceType };
      const response = await listInvoices(filters);
      const items = Array.isArray(response)
        ? response
        : (response?.items ?? []);
      renderList(
        items.filter(
          (item) => (item?.invoiceType ?? cfg.invoiceType) === cfg.invoiceType,
        ),
      );
    } catch {
      const message = `No se pudieron cargar las facturas de ${cfg.noun}.`;
      messageRow(body, message);
      showFeedback(message);
    }
  }

  function renderList(items) {
    const body = document.getElementById(cfg.bodyId);
    if (!body) return;
    if (!items.length) {
      messageRow(body, `Aún no hay facturas de ${cfg.noun}.`);
      return;
    }
    body.replaceChildren();
    for (const item of items) {
      const row = document.createElement("tr");
      const cells = [
        item?.number ?? "—",
        item?.contactName ?? "—",
        item?.date ? formatDate(item.date) : "—",
        item?.associatedInvoice ? associatedLabel(item.associatedInvoice) : "—",
        formatMoney(item?.totalCents ?? 0),
        ASSOCIATED_STATUS_LABELS[item?.status] ?? item?.status ?? "—",
      ].map((text) => {
        const cell = document.createElement("td");
        cell.textContent = text;
        return cell;
      });
      const actionsCell = document.createElement("td");
      for (const [action, label] of [
        ["detail", "Ver"],
        ["delete", "Eliminar"],
      ]) {
        const button = document.createElement("button");
        button.type = "button";
        button.textContent = label;
        button.dataset.associatedAction = action;
        button.dataset.associatedId = String(item?.id ?? "");
        actionsCell.append(button);
      }
      row.append(...cells, actionsCell);
      body.append(row);
    }
  }

  function renderLines() {
    const container = byId("lineas-body");
    if (!container) return;
    container.replaceChildren();
    lines.forEach((line, index) => {
      const row = document.createElement("div");
      row.className = "li-row li-body";
      for (const text of [
        String(index + 1),
        line.productName ?? line.productId ?? "",
        line.quantity ?? "",
      ]) {
        const cell = document.createElement("div");
        cell.textContent = text;
        row.append(cell);
      }
      container.append(row);
    });
  }

  async function submit() {
    const clientId = selectedClient?.id ?? selectedClient?.clientId ?? null;
    const associatedInvoiceId =
      document.getElementById(cfg.associatedSelectId)?.value ?? "";
    const invoiceDate = byId("fecha-emision")?.value ?? "";
    const status = byId("estado")?.value ?? "pending";
    const notes = byId("descripcion")?.value.trim() || undefined;

    if (!clientId || !associatedInvoiceId || !invoiceDate || !lines.length) {
      showFeedback(cfg.missingMessage);
      return;
    }

    const input = {
      clientId: Number(clientId),
      associatedInvoiceId: Number(associatedInvoiceId),
      invoiceDate,
      status,
      ...(notes ? { notes } : {}),
      lines: lines.map((line) => ({
        productId: Number(line.productId),
        quantity: String(line.quantity ?? ""),
      })),
    };

    try {
      const result = await cfg.create(input);
      showCleanupWarning(
        document.getElementById(cfg.pageId),
        `${cfg.pageKey}-cleanup-warning`,
        result?.expiredCleanupWarning,
      );
      lines = [];
      renderLines();
      await loadInvoices();
      showTab("list", listTab());
    } catch (error) {
      showFeedback(error?.message || "No se pudo guardar la factura.");
    }
  }

  async function openDetail(invoiceId) {
    try {
      const detail = await getInvoice(Number(invoiceId));
      currentId = Number(invoiceId);
      currentDetail = detail;
      renderDetail(detail);
      showTab("ver", null);
    } catch (error) {
      showFeedback(error?.message || "No se pudo abrir la factura.");
    }
  }

  function renderDetail(detail) {
    const status = ASSOCIATED_STATUS_LABELS[detail?.status] ?? "—";
    const total = formatMoney(detail?.totalCents ?? 0);
    const set = (suffix, value) => setText(`${cfg.field}-${suffix}`, value);
    set("ver-numero-badge", detail?.number ?? "");
    set("ver-titulo", `${cfg.title} ${detail?.number ?? ""}`);
    set("ver-estado-txt", status);
    set("ver-estado-hero", status);
    set("ver-total", total);
    set("ver-subtotal", total);
    set("ver-total-resumen", total);
    set("ver-cliente", detail?.contactFullName ?? "—");
    set("ver-numero", detail?.number ?? "");
    set("ver-emision", detail?.date ? formatDate(detail.date) : "—");
    setText(
      cfg.associatedViewId,
      detail?.associatedInvoice
        ? associatedLabel(detail.associatedInvoice)
        : "—",
    );
    set("ver-descripcion", detail?.notes ?? "—");

    const productos = byId("ver-productos");
    if (productos) {
      productos.replaceChildren();
      for (const [index, line] of (detail?.lines ?? []).entries()) {
        const row = document.createElement("div");
        row.className = "li-row li-body li-body-readonly";
        // El precio de la línea es el costo real consumido (FS-04/05).
        for (const text of [
          String(index + 1),
          line?.productName ?? "",
          String(line?.quantity ?? ""),
          line?.unitAbbreviation ?? line?.unitName ?? "—",
          formatMoney(line?.unitPriceCents ?? 0),
          formatMoney(line?.subtotalCents ?? 0),
        ]) {
          const cell = document.createElement("div");
          cell.textContent = text;
          row.append(cell);
        }
        productos.append(row);
      }
    }

    // Solo las facturas Pendientes se pueden editar (FS-01).
    const editButton = byId("editar");
    if (editButton) editButton.hidden = detail?.status !== "pending";
  }

  async function handleDelete(invoiceId) {
    const confirmed = await confirmDialog({
      title: "Eliminar factura",
      message: `¿Deseas eliminar esta factura de ${cfg.noun}?`,
      confirmText: "Eliminar",
    });
    if (!confirmed) return;
    try {
      const deleted = await deleteInvoice(Number(invoiceId));
      showCleanupWarning(
        document.getElementById(cfg.pageId),
        `${cfg.pageKey}-cleanup-warning`,
        deleted?.expiredCleanupWarning,
      );
      if (currentId === Number(invoiceId)) {
        currentId = null;
        currentDetail = null;
      }
      await loadInvoices();
      showTab("list", listTab());
    } catch (error) {
      showFeedback(error?.message || "No se pudo eliminar la factura.");
    }
  }

  function showFeedback(message) {
    const form = document.getElementById(`${cfg.view}-form`);
    if (!form) return;
    const feedbackId = `${cfg.view}-feedback`;
    let feedback = document.getElementById(feedbackId);
    if (!feedback) {
      feedback = document.createElement("div");
      feedback.id = feedbackId;
      feedback.className = "form-feedback";
      feedback.setAttribute("role", "status");
      form.append(feedback);
    }
    feedback.textContent = message;
  }

  return {
    init,
    reload: () => loadInvoices(),
    showTab,
    selectClient,
    openDetail,
    submit,
    setLines: (next) => {
      lines = next;
    },
  };
}

function messageRow(body, text) {
  const row = document.createElement("tr");
  const cell = document.createElement("td");
  cell.textContent = text;
  row.append(cell);
  body.replaceChildren(row);
}

function associatedLabel(item) {
  const number = item?.number ?? "";
  const type = item?.invoiceType ?? "";
  return `${number} · ${INVOICE_TYPE_LABELS[type] ?? type}`;
}

const guaranteeSubmodule = createAssociatedSubmodule({
  invoiceType: "GG",
  pageId: "page-facturas-garantia",
  pageKey: "facturas-garantia",
  view: "factura-garantia",
  field: "factura-garantia",
  bodyId: "facturas-garantia-body",
  associatedSelectId: "factura-garantia-original",
  associatedViewId: "factura-garantia-ver-original",
  associatedTypes: ["V", "S"],
  selectPlaceholder: "Selecciona la factura original",
  selectEmpty: "Este cliente no tiene ventas ni servicios vigentes",
  missingMessage:
    "Completa el cliente, la factura original, la fecha y al menos un producto.",
  noun: "garantía",
  title: "Factura de Garantía",
  exportFn: "exportarFacturaGarantiaPDF",
  openClientEvent: "guarantee:openClientForm",
  windowFlag: "__facturasGuaranteePageChangeBound",
  create: (input) => createGuaranteeInvoice(input),
});

const installationSubmodule = createAssociatedSubmodule({
  invoiceType: "GI",
  pageId: "page-facturas-insumos",
  pageKey: "facturas-insumos",
  view: "factura-insumos",
  field: "factura-insumo",
  bodyId: "facturas-insumos-body",
  associatedSelectId: "factura-insumo-servicio",
  associatedViewId: "factura-insumo-ver-asociada",
  associatedTypes: ["S"],
  selectPlaceholder: "Selecciona la factura de servicio",
  selectEmpty: "Este cliente no tiene servicios vigentes",
  missingMessage:
    "Completa el cliente, la factura de servicio, la fecha y al menos un insumo.",
  noun: "insumos",
  title: "Factura de Insumos",
  exportFn: "exportarFacturaInsumoPDF",
  openClientEvent: "installation:openClientForm",
  windowFlag: "__facturasInstallationPageChangeBound",
  create: (input) => createInstallationInvoice(input),
});

export const initGuaranteeInvoicesView = guaranteeSubmodule.init;
export const showGuaranteeTab = guaranteeSubmodule.showTab;
export const openGuaranteeDetail = guaranteeSubmodule.openDetail;
export const initInstallationInvoicesView = installationSubmodule.init;
export const showInstallationTab = installationSubmodule.showTab;
export const openInstallationDetail = installationSubmodule.openDetail;

// ===================== Submódulo: facturas de pérdida =====================
// FP-01: un solo producto, sin cliente ni estado; el costo lo calcula el
// backend con el consumo FIFO y solo se muestra después de guardar.

let lossProductCombobox = null;
let currentLossId = null;
let currentLossDetail = null;

export function initLossInvoicesView() {
  const root = document.getElementById("page-facturas-perdidas");
  if (!root) return Promise.resolve();
  for (const attr of ["onclick", "oninput", "onchange", "onsubmit"]) {
    root
      .querySelectorAll(`[${attr}]`)
      .forEach((el) => el.removeAttribute(attr));
  }
  if (!window.__facturasLossPageChangeBound) {
    window.addEventListener("page:change", (event) => {
      if (event.detail?.pageId !== "facturas-perdidas") return;
      void initLossInvoicesView();
    });
    window.__facturasLossPageChangeBound = true;
  }
  bindLossTabs();
  bindLossSearch();
  bindLossProductCombobox();
  bindLossForm();
  bindLossTableActions();
  bindLossDetailActions();
  return loadLossInvoices();
}

function claimLoss(el) {
  if (!el || el.dataset.lossBound === "1") return false;
  el.dataset.lossBound = "1";
  return true;
}

function bindLossTabs() {
  const listTab = () => document.getElementById("tab-factura-perdidas-list");
  const toList = () => showLossTab("list", listTab());
  for (const [id, handler] of [
    ["tab-factura-perdidas-list", toList],
    [
      "tab-factura-perdidas-create",
      () =>
        showLossTab(
          "create",
          document.getElementById("tab-factura-perdidas-create"),
        ),
    ],
    ["factura-perdida-cancelar", toList],
    ["factura-perdida-volver", toList],
  ]) {
    const el = document.getElementById(id);
    if (claimLoss(el)) el.addEventListener("click", handler);
  }
}

export function showLossTab(tab, button = null) {
  document
    .querySelectorAll("#page-facturas-perdidas .view-tabs .tab")
    .forEach((tabButton) => {
      const isActive =
        tabButton === button || tabButton.id === `tab-factura-perdidas-${tab}`;
      tabButton.classList.toggle("active", isActive);
    });
  document
    .querySelectorAll("#page-facturas-perdidas .config-subview")
    .forEach((view) => {
      view.classList.toggle("active", view.id === `factura-perdidas-${tab}`);
    });
  const searchBar = document.getElementById("factura-perdidas-search-bar");
  if (searchBar) searchBar.style.display = tab === "list" ? "" : "none";
}

function bindLossSearch() {
  const searchInput = document.getElementById("factura-perdidas-search");
  if (!claimLoss(searchInput)) return;
  const debounced = debounce((query) => {
    void loadLossInvoices(query);
  }, 300);
  searchInput.addEventListener("input", (event) => {
    debounced(String(event.target.value ?? "").trim());
  });
}

// Combobox sin "Agregar nueva": no se pasa onAddNew (FE-FP-03).
function bindLossProductCombobox() {
  const container = document.getElementById(
    "factura-perdida-producto-combobox",
  );
  if (!claimLoss(container)) return;
  lossProductCombobox = createCombobox({
    container,
    id: "factura-perdida-producto",
    placeholder: "Escribe para buscar un producto...",
    search: searchProductsForInvoice,
    formatOption: (item) => {
      const unit = item?.unitAbbreviation ? ` ${item.unitAbbreviation}` : "";
      return `${item?.name ?? ""} · stock ${item?.stock ?? "0"}${unit}`;
    },
  });
  // Se conserva el id del contrato de interfaz en el input real.
  const input = lossProductCombobox.getInput();
  input.id = "factura-perdida-producto";
  input.required = true;
}

function bindLossForm() {
  const form = document.getElementById("factura-perdidas-form");
  if (!claimLoss(form)) return;
  form.addEventListener("submit", (event) => {
    event.preventDefault();
    void handleLossSubmit();
  });
}

function bindLossTableActions() {
  const body = document.getElementById("facturas-perdidas-body");
  if (!claimLoss(body)) return;
  body.addEventListener("click", (event) => {
    const action = event.target.closest("[data-loss-action]");
    if (!action) return;
    const invoiceId = Number(action.dataset.lossId);
    if (!Number.isFinite(invoiceId)) return;
    if (action.dataset.lossAction === "detail") {
      void openLossDetail(invoiceId);
    } else if (action.dataset.lossAction === "delete") {
      void handleDeleteLoss(invoiceId);
    }
  });
}

function bindLossDetailActions() {
  const exportButton = document.getElementById("factura-perdida-exportar");
  if (claimLoss(exportButton)) {
    exportButton.addEventListener("click", () => {
      if (
        currentLossDetail &&
        typeof window.exportarFacturaPerdidaPDF === "function"
      ) {
        window.exportarFacturaPerdidaPDF(currentLossDetail);
      }
    });
  }
  const deleteButton = document.getElementById("factura-perdida-eliminar");
  if (claimLoss(deleteButton)) {
    deleteButton.addEventListener("click", () => {
      if (currentLossId != null) void handleDeleteLoss(currentLossId);
    });
  }
}

async function loadLossInvoices(query = "") {
  const body = document.getElementById("facturas-perdidas-body");
  if (!body) return;
  messageRow(body, "Cargando facturas de pérdidas…");
  try {
    const filters = query
      ? { invoiceType: "P", search: query }
      : { invoiceType: "P" };
    const response = await listInvoices(filters);
    const items = Array.isArray(response) ? response : (response?.items ?? []);
    renderLossList(items.filter((item) => (item?.invoiceType ?? "P") === "P"));
  } catch {
    messageRow(body, "No se pudieron cargar las facturas de pérdidas.");
    showLossFeedback("No se pudieron cargar las facturas de pérdidas.");
  }
}

function renderLossList(items) {
  const body = document.getElementById("facturas-perdidas-body");
  if (!body) return;
  if (!items.length) {
    messageRow(body, "Aún no hay facturas de pérdidas.");
    return;
  }
  body.replaceChildren();
  for (const item of items) {
    const row = document.createElement("tr");
    const cells = [
      item?.number ?? "—",
      item?.productName ?? "—",
      item?.quantity ?? "—",
      formatMoney(item?.totalCents ?? 0),
      item?.date ? formatDate(item.date) : "—",
    ].map((text) => {
      const cell = document.createElement("td");
      cell.textContent = text;
      return cell;
    });
    const actionsCell = document.createElement("td");
    for (const [action, label] of [
      ["detail", "Ver"],
      ["delete", "Eliminar"],
    ]) {
      const button = document.createElement("button");
      button.type = "button";
      button.textContent = label;
      button.dataset.lossAction = action;
      button.dataset.lossId = String(item?.id ?? "");
      actionsCell.append(button);
    }
    row.append(...cells, actionsCell);
    body.append(row);
  }
}

async function handleLossSubmit() {
  const product = lossProductCombobox?.getSelected() ?? null;
  const quantity = String(
    document.getElementById("factura-perdida-cantidad")?.value ?? "",
  ).trim();
  const invoiceDate =
    document.getElementById("factura-perdida-fecha")?.value ?? "";
  const notes =
    document.getElementById("factura-perdida-descripcion")?.value.trim() ||
    undefined;

  if (!product || !quantity || !invoiceDate) {
    showLossFeedback("Completa el producto, la cantidad y la fecha.");
    return;
  }

  // El costo no se envía: lo calcula el backend (FP-01).
  const input = {
    productId: Number(product.id),
    quantity,
    invoiceDate,
    ...(notes ? { notes } : {}),
  };

  try {
    const result = await createLossInvoice(input);
    showCleanupWarning(
      document.getElementById("page-facturas-perdidas"),
      "facturas-perdidas-cleanup-warning",
      result?.expiredCleanupWarning,
    );
    lossProductCombobox?.setSelected(null);
    const cantidad = document.getElementById("factura-perdida-cantidad");
    if (cantidad) cantidad.value = "1";
    const descripcion = document.getElementById("factura-perdida-descripcion");
    if (descripcion) descripcion.value = "";
    await loadLossInvoices();
    // Tras guardar se abre el detalle, que ya trae el costo real.
    if (result?.invoiceId != null) await openLossDetail(result.invoiceId);
  } catch (error) {
    showLossFeedback(error?.message || "No se pudo guardar la pérdida.");
  }
}

export async function openLossDetail(invoiceId) {
  try {
    const detail = await getInvoice(Number(invoiceId));
    currentLossId = Number(invoiceId);
    currentLossDetail = detail;
    renderLossDetail(detail);
    showLossTab("ver", null);
  } catch (error) {
    showLossFeedback(error?.message || "No se pudo abrir la pérdida.");
  }
}

function renderLossDetail(detail) {
  const line = detail?.lines?.[0] ?? null;
  const productName = line?.productName ?? "—";
  const unit = line?.unitAbbreviation ?? line?.unitName ?? "";
  setText("factura-perdida-ver-numero-badge", detail?.number ?? "");
  setText(
    "factura-perdida-ver-titulo",
    `Factura de Pérdida ${detail?.number ?? ""}`,
  );
  setText("factura-perdida-ver-producto-tag", productName);
  setText("factura-perdida-ver-producto", productName);
  setText(
    "factura-perdida-ver-costo-total",
    formatMoney(detail?.totalCents ?? 0),
  );
  setText(
    "factura-perdida-ver-costo",
    line ? formatMoney(line.unitPriceCents ?? 0) : "—",
  );
  setText(
    "factura-perdida-ver-cantidad",
    line ? `${line.quantity ?? ""}${unit ? ` ${unit}` : ""}` : "—",
  );
  setText(
    "factura-perdida-ver-fecha",
    detail?.date ? formatDate(detail.date) : "—",
  );
  setText("factura-perdida-ver-descripcion", detail?.notes ?? "—");
}

async function handleDeleteLoss(invoiceId) {
  const confirmed = await confirmDialog({
    title: "Eliminar pérdida",
    message: "¿Deseas eliminar esta factura de pérdida?",
    confirmText: "Eliminar",
  });
  if (!confirmed) return;
  try {
    const deleted = await deleteInvoice(Number(invoiceId));
    showCleanupWarning(
      document.getElementById("page-facturas-perdidas"),
      "facturas-perdidas-cleanup-warning",
      deleted?.expiredCleanupWarning,
    );
    if (currentLossId === Number(invoiceId)) {
      currentLossId = null;
      currentLossDetail = null;
    }
    await loadLossInvoices();
    showLossTab("list", document.getElementById("tab-factura-perdidas-list"));
  } catch (error) {
    showLossFeedback(error?.message || "No se pudo eliminar la pérdida.");
  }
}

function showLossFeedback(message) {
  const form = document.getElementById("factura-perdidas-form");
  if (!form) return;
  let feedback = document.getElementById("factura-perdidas-feedback");
  if (!feedback) {
    feedback = document.createElement("div");
    feedback.id = "factura-perdidas-feedback";
    feedback.className = "form-feedback";
    feedback.setAttribute("role", "status");
    form.append(feedback);
  }
  feedback.textContent = message;
}

// ===================== Submódulo: lista general =====================
// FG-01: solo lectura; todos los tipos vía listInvoices sin `invoiceType`.

// Página y apertura del detalle de cada tipo para el botón "Ver".
const GENERAL_TYPE_TARGETS = {
  E: { page: "facturas-entrada", open: (id) => openEntryDetail(id) },
  V: { page: "facturas-venta", open: (id) => openSaleDetail(id) },
  S: { page: "facturas-servicios", open: (id) => openServiceDetail(id) },
  GG: { page: "facturas-garantia", open: (id) => openGuaranteeDetail(id) },
  GI: { page: "facturas-insumos", open: (id) => openInstallationDetail(id) },
  P: { page: "facturas-perdidas", open: (id) => openLossDetail(id) },
};

export function initGeneralInvoicesView() {
  const search = document.getElementById("factura-general-search");
  if (!search) return Promise.resolve();
  if (search.dataset.generalBound !== "1") {
    search.dataset.generalBound = "1";
    const debounced = debounce(() => void loadGeneralInvoices(), 300);
    search.addEventListener("input", debounced);
    for (const id of ["factura-general-tipo", "factura-general-estado"]) {
      document
        .getElementById(id)
        ?.addEventListener("change", () => void loadGeneralInvoices());
    }
    document
      .getElementById("facturas-general-body")
      ?.addEventListener("click", (event) => {
        const button = event.target.closest("[data-general-type]");
        if (!button) return;
        void irAFacturaDesdeGeneral(
          button.dataset.generalType,
          Number(button.dataset.generalId),
        );
      });
  }
  if (!window.__facturasGeneralPageChangeBound) {
    window.addEventListener("page:change", (event) => {
      if (event.detail?.pageId !== "facturas-general") return;
      void loadGeneralInvoices();
    });
    window.__facturasGeneralPageChangeBound = true;
  }
  return loadGeneralInvoices();
}

async function loadGeneralInvoices() {
  const body = document.getElementById("facturas-general-body");
  if (!body) return;
  const value = (id) => String(document.getElementById(id)?.value ?? "").trim();
  const filters = {};
  if (value("factura-general-search"))
    filters.search = value("factura-general-search");
  if (value("factura-general-tipo"))
    filters.invoiceType = value("factura-general-tipo");
  if (value("factura-general-estado"))
    filters.status = value("factura-general-estado");
  messageRow(body, "Cargando facturas…");
  try {
    const response = await listInvoices(filters);
    renderFacturasGeneral(
      Array.isArray(response) ? response : (response?.items ?? []),
    );
  } catch {
    messageRow(body, "No se pudieron cargar las facturas.");
  }
}

export function renderFacturasGeneral(items) {
  const body = document.getElementById("facturas-general-body");
  if (!body) return;
  if (!items.length) {
    const filtered = [
      "factura-general-search",
      "factura-general-tipo",
      "factura-general-estado",
    ].some((id) => document.getElementById(id)?.value);
    messageRow(
      body,
      filtered
        ? "No se encontraron facturas que coincidan con los filtros."
        : "Aún no hay facturas.",
    );
    return;
  }
  body.replaceChildren();
  for (const item of items) {
    const type = item?.invoiceType ?? "";
    const row = document.createElement("tr");
    const cells = [
      item?.number ?? "—",
      INVOICE_TYPE_LABELS[type] ?? type,
      type === "P" ? "—" : (item?.contactName ?? "—"),
      item?.date ? formatDate(item.date) : "—",
      formatMoney(item?.totalCents ?? 0),
      invoiceStatusLabel(type, item?.status),
    ].map((text) => {
      const cell = document.createElement("td");
      cell.textContent = text;
      return cell;
    });
    const actionsCell = document.createElement("td");
    const viewButton = document.createElement("button");
    viewButton.type = "button";
    viewButton.textContent = "Ver";
    viewButton.dataset.generalType = type;
    viewButton.dataset.generalId = String(item?.id ?? "");
    actionsCell.append(viewButton);
    row.append(...cells, actionsCell);
    body.append(row);
  }
}

// Abre la página del tipo y su detalle; desde aquí no se crea, edita ni elimina.
export async function irAFacturaDesdeGeneral(type, invoiceId) {
  const target = GENERAL_TYPE_TARGETS[type];
  if (!target || !Number.isFinite(invoiceId)) return;
  if (typeof window.showPage === "function") {
    window.showPage(
      target.page,
      document.querySelector(`#sidebar a[onclick*="'${target.page}'"]`),
    );
  }
  await target.open(invoiceId);
}

function showEntryFeedback(message) {
  const form =
    document.getElementById("factura-entrada-form") ??
    document.querySelector("#factura-entrada-create form");
  if (!form) return;
  let feedback = document.getElementById("factura-entrada-feedback");
  if (!feedback) {
    feedback = document.createElement("div");
    feedback.id = "factura-entrada-feedback";
    feedback.className = "form-feedback";
    feedback.setAttribute("role", "status");
    form.append(feedback);
  }
  feedback.textContent = message;
}

// Evita exportar símbolos sin uso fuera de tests.
void changeInvoiceStatus;

// Compatibilidad con llamadas globales del HTML real hasta migrar el mock.
window.showFacturaEntradaTab = showEntryTab;
window.showFacturaVentaTab = showSaleTab;
window.showFacturaServiciosTab = showServiceTab;
window.guardarFactura = (event) => {
  if (event) event.preventDefault();
  void handleEntrySubmit();
};
window.guardarFacturaVenta = (event) => {
  if (event) event.preventDefault();
  void handleSaleSubmit();
};
window.guardarFacturaServicio = (event) => {
  if (event) event.preventDefault();
  void handleServiceSubmit();
};
window.verFactura = (id) => void openEntryDetail(id);
window.verFacturaVenta = (id) => void openSaleDetail(id);
window.verFacturaServicio = (id) => void openServiceDetail(id);
window.showFacturaGarantiaTab = showGuaranteeTab;
window.guardarFacturaGarantia = (event) => {
  if (event) event.preventDefault();
  void guaranteeSubmodule.submit();
};
window.verFacturaGarantia = (id) => void openGuaranteeDetail(id);
window.showFacturaInsumosTab = showInstallationTab;
window.guardarFacturaInsumo = (event) => {
  if (event) event.preventDefault();
  void installationSubmodule.submit();
};
window.verFacturaInsumo = (id) => void openInstallationDetail(id);
window.showFacturaPerdidasTab = showLossTab;
window.guardarFacturaPerdida = (event) => {
  if (event) event.preventDefault();
  void handleLossSubmit();
};
window.verFacturaPerdida = (id) => void openLossDetail(id);
window.renderFacturasGeneral = () => void loadGeneralInvoices();
// Los listados con los nombres globales antiguos (tarea 4.5.7): mismas vistas
// con textContent que sustituyen a las de dashboard.js.
window.renderFacturasEntrada = () => void loadEntryInvoices();
window.renderFacturasVenta = () => void loadSaleInvoices();
window.renderFacturasServicios = () => void loadServiceInvoices();
window.renderFacturasGarantia = () => void guaranteeSubmodule.reload();
window.renderFacturasInsumos = () => void installationSubmodule.reload();
window.renderFacturasPerdidas = () => void loadLossInvoices();
window.irAFacturaDesdeGeneral = (type, id) =>
  void irAFacturaDesdeGeneral(type, Number(id));

export const __testHooks = {
  setSelectedSupplier: (supplier) => {
    selectedSupplier = supplier;
  },
  setEntryLines: (lines) => {
    entryLines = lines;
  },
  setPendingAttachment: (path) => {
    pendingAttachmentPath = path;
  },
  setDownloadDestination: (path) => {
    downloadDestinationOverride = path;
  },
  openEntryDetail,
  setSelectedClient: (client) => {
    selectedClient = client;
  },
  setSaleLines: (lines) => {
    saleLines = lines;
  },
  openSaleDetail,
  setSelectedServiceClient: (client) => {
    selectedServiceClient = client;
  },
  setServiceLines: (lines) => {
    serviceLines = lines;
  },
  openServiceDetail,
  selectGuaranteeClient: guaranteeSubmodule.selectClient,
  setGuaranteeLines: guaranteeSubmodule.setLines,
  openGuaranteeDetail,
  selectInstallationClient: installationSubmodule.selectClient,
  setInstallationLines: installationSubmodule.setLines,
  openInstallationDetail,
  openLossDetail,
  getLossCombobox: () => lossProductCombobox,
};

if (document.readyState === "loading") {
  document.addEventListener("DOMContentLoaded", () => {
    if (document.getElementById("page-facturas-entrada")) {
      void initEntryInvoicesView();
    }
    if (document.getElementById("page-facturas-venta")) {
      void initSaleInvoicesView();
    }
    if (document.getElementById("page-facturas-servicios")) {
      void initServiceInvoicesView();
    }
    if (document.getElementById("page-facturas-garantia")) {
      void initGuaranteeInvoicesView();
    }
    if (document.getElementById("page-facturas-insumos")) {
      void initInstallationInvoicesView();
    }
    if (document.getElementById("page-facturas-perdidas")) {
      void initLossInvoicesView();
    }
    if (document.getElementById("page-facturas-general")) {
      void initGeneralInvoicesView();
    }
  });
} else {
  if (document.getElementById("page-facturas-entrada")) {
    void initEntryInvoicesView();
  }
  if (document.getElementById("page-facturas-venta")) {
    void initSaleInvoicesView();
  }
  if (document.getElementById("page-facturas-servicios")) {
    void initServiceInvoicesView();
  }
  if (document.getElementById("page-facturas-garantia")) {
    void initGuaranteeInvoicesView();
  }
  if (document.getElementById("page-facturas-insumos")) {
    void initInstallationInvoicesView();
  }
  if (document.getElementById("page-facturas-perdidas")) {
    void initLossInvoicesView();
  }
  if (document.getElementById("page-facturas-general")) {
    void initGeneralInvoicesView();
  }
}
