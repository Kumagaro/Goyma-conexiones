import { createCombobox } from "../components/combobox.js";
import { confirmDialog } from "../components/confirmDialog.js";
import { openQuickContactDialog } from "../components/quickContactDialog.js";
import { openQuickProductDialog } from "../components/quickProductDialog.js";
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
  updateEntryInvoice,
  updateGuaranteeInvoice,
  updateInstallationInvoice,
  updateSaleInvoice,
  updateServiceInvoice,
} from "../services/invoicesService.js";
import { getClientDetail, searchClients } from "../services/clientsService.js";
import { searchSuppliers } from "../services/suppliersService.js";
import { searchProductsForInvoice } from "../services/historyService.js";
import {
  formatDate,
  formatMoney,
  formatTimestamp,
} from "../utils/formatters.js";
import {
  ASSOCIATED_STATUS_LABELS,
  ENTRY_STATUS_LABELS,
  INVOICE_TYPE_LABELS,
  SALE_STATUS_LABELS,
  invoiceStatusLabel,
} from "../utils/invoiceLabels.js";
import {
  clearFieldErrors,
  notifyError,
  notifySuccess,
  notifyWarning,
  requireFields,
  setFieldError,
} from "../utils/feedback.js";
import { bindDateSort, sortByDate } from "../utils/dateSort.js";
import { debounce } from "../utils/requestGuard.js";

const PAYMENT_LABELS = {
  cash: "Efectivo",
  transfer: "Transferencia",
  card: "Tarjeta",
  zelle: "Zelle",
};

let downloadDestinationOverride = null;

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

// ===================== Utilidades comunes =====================

function setText(id, value) {
  const el = document.getElementById(id);
  if (el) el.textContent = value;
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

function toItems(response) {
  return Array.isArray(response) ? response : (response?.items ?? []);
}

// Solo los rechazos de validación (p. ej. rango de fechas invertido) traen un
// mensaje para el usuario; el resto se resume en un texto genérico.
function listErrorMessage(error, fallback) {
  return error?.code === "validationFailed" && error?.message
    ? error.message
    : fallback;
}

// Filtros de los listados (§7.7): búsqueda, estado y rango de fechas.
function readListFilters(prefix, withStatus) {
  const value = (suffix) =>
    String(document.getElementById(`${prefix}-${suffix}`)?.value ?? "").trim();
  const filters = {};
  if (value("search")) filters.search = value("search");
  if (withStatus && value("filtro-estado")) {
    filters.status = value("filtro-estado");
  }
  if (value("desde")) filters.fromDate = value("desde");
  if (value("hasta")) filters.toDate = value("hasta");
  return filters;
}

function bindListFilters(prefix, reload, claim) {
  const search = document.getElementById(`${prefix}-search`);
  if (claim(search)) {
    const debounced = debounce(() => void reload(), 300);
    search.addEventListener("input", debounced);
  }
  for (const suffix of ["filtro-estado", "desde", "hasta"]) {
    const control = document.getElementById(`${prefix}-${suffix}`);
    if (claim(control)) {
      control.addEventListener("change", () => void reload());
    }
  }
}

// Sustituye un <input> del HTML por un combobox (components/combobox.js) que
// conserva su id, para que las etiquetas y los tests sigan encontrándolo.
function mountCombobox(original, { label, ...options }) {
  if (!original) return null;
  const container = document.createElement("div");
  container.style.flex = "1";
  original.replaceWith(container);
  const combobox = createCombobox({ container, id: original.id, ...options });
  const input = combobox.getInput();
  input.id = original.id;
  input.required = original.required;
  input.setAttribute("aria-label", label);
  return combobox;
}

function formatContactOption(item) {
  const name = item?.fullName ?? "";
  return item?.company ? `${name} · ${item.company}` : name;
}

function formatProductOption(item) {
  const unit = item?.unitAbbreviation ? ` ${item.unitAbbreviation}` : "";
  return [
    `${item?.name ?? ""} · stock ${item?.stock ?? "0"}${unit}`,
    item?.isOutOfStock ? "Agotado" : null,
    item?.isDiscontinued ? "Descontinuado" : null,
  ]
    .filter(Boolean)
    .join(" · ");
}

function productSearch(purpose) {
  return async (query) => {
    if (!query) return [];
    const result = await searchProductsForInvoice(query, purpose);
    return Array.isArray(result) ? result : [];
  };
}

// Total preliminar (spec §7.2): comodidad del formulario sobre lo tecleado;
// el backend recalcula el total autoritativo al guardar.
function parseDecimal(text, decimals) {
  const normalized = String(text ?? "")
    .trim()
    .replace(",", ".");
  if (!/^\d+(\.\d+)?$/.test(normalized)) return null;
  return Math.round(Number(normalized) * 10 ** decimals);
}

// Fecha de hoy en hora local (aaaa-mm-dd): valor por defecto de las facturas.
export function todayLocal() {
  const now = new Date();
  const two = (value) => String(value).padStart(2, "0");
  return `${now.getFullYear()}-${two(now.getMonth() + 1)}-${two(now.getDate())}`;
}

// Fecha de creación y responsable del detalle (FE-03, FS-01, FP-01).
function renderRegistration(viewId, detail) {
  const view = document.getElementById(viewId);
  const anchor = view?.querySelector(".pv-header > div") ?? view;
  if (!anchor) return;
  const id = `${viewId}-registro`;
  let line = document.getElementById(id);
  if (!line) {
    line = document.createElement("p");
    line.id = id;
    line.className = "pv-desc-text";
    anchor.append(line);
  }
  line.textContent = detail?.createdAt
    ? `Registrada el ${formatTimestamp(detail.createdAt)} por ${detail.createdByName ?? "—"}`
    : "";
}

function centsToText(cents) {
  if (!Number.isSafeInteger(cents)) return "";
  return (cents / 100).toFixed(2).replace(".", ",");
}

// Espejo de state.rs para ofrecer solo las transiciones válidas; el backend
// vuelve a validar cada cambio.
function allowedTransitions(invoiceType, status) {
  if (!status || status === "cancelled") return [];
  const withPayment = ["E", "V", "S"].includes(invoiceType);
  const transitions = [];
  if (status === "pending") transitions.push("completed");
  if (status === "completed" && withPayment) transitions.push("pending");
  transitions.push("cancelled");
  return transitions;
}

function trashIcon() {
  const svg = document.createElementNS("http://www.w3.org/2000/svg", "svg");
  svg.setAttribute("class", "icon");
  svg.setAttribute("aria-hidden", "true");
  const use = document.createElementNS("http://www.w3.org/2000/svg", "use");
  use.setAttribute("href", "#icon-trash");
  svg.append(use);
  return svg;
}

// FE-05: al eliminar una compra se detallan, por producto, las unidades que
// se perderán y las que ya salieron en otras facturas.
function entryDeleteMessage(detail) {
  const affected = (detail?.lines ?? []).filter(
    (line) => line?.remainingQuantity && line.remainingQuantity !== "0",
  );
  if (!affected.length) return "¿Deseas eliminar esta factura de entrada?";
  const items = affected.map((line) => {
    const unit = line.unitAbbreviation ? ` ${line.unitAbbreviation}` : "";
    const consumed =
      line.consumedQuantity && line.consumedQuantity !== "0"
        ? `; ya se consumieron ${line.consumedQuantity}${unit} en salidas`
        : "";
    return `• ${line.productName ?? ""}: se perderán ${line.remainingQuantity}${unit}${consumed}.`;
  });
  return [
    "Al eliminar esta compra se quitarán del inventario las unidades que aún quedan:",
    ...items,
    "¿Deseas continuar?",
  ].join("\n");
}

// ============ Submódulos: entrada, venta, servicio, garantía e insumos ============
// Comparten listado, formulario (crear y editar), detalle y cambio de estado;
// las diferencias de cada tipo viven en su configuración.

function createInvoiceSubmodule(cfg) {
  const isEntry = cfg.invoiceType === "E";
  const hasPayment = ["E", "V", "S"].includes(cfg.invoiceType);
  const showsSalePrice = cfg.invoiceType === "V" || cfg.invoiceType === "S";
  const associated = cfg.associated ?? null;
  const contactNoun = cfg.contact === "client" ? "cliente" : "proveedor";

  let currentId = null;
  let currentDetail = null;
  let editing = null;
  let lines = [];
  let selectedContact = null;
  let contactCombobox = null;
  let productCombobox = null;
  let saving = false;
  let associatedRequest = 0;
  let createTitle = null;
  let pendingAttachmentPath = null;
  let removeAttachment = false;
  let lastItems = [];

  const field = (suffix) => document.getElementById(`${cfg.field}-${suffix}`);
  const button = (suffix) =>
    document.getElementById(`${cfg.buttons}-${suffix}`);
  const form = () => document.getElementById(`${cfg.view}-form`);
  const listTab = () => document.getElementById(`tab-${cfg.view}-list`);
  const createTab = () => document.getElementById(`tab-${cfg.view}-create`);

  function claim(el) {
    if (!el || el.dataset.invoiceBound === "1") return false;
    el.dataset.invoiceBound = "1";
    return true;
  }

  function init() {
    if (!window[cfg.windowFlag]) {
      window.addEventListener("page:change", (event) => {
        if (event.detail?.pageId !== cfg.pageKey) return;
        void init();
      });
      window[cfg.windowFlag] = true;
    }
    bindTabs();
    bindListFilters(cfg.view, loadInvoices, claim);
    mountContactCombobox();
    mountProductCombobox();
    bindForm();
    bindTableActions();
    bindDetailActions();
    if (isEntry) bindAttachment();
    if (hasPayment) {
      const status = field("estado");
      if (claim(status)) {
        status.addEventListener("change", updatePaymentVisibility);
      }
      updatePaymentVisibility();
    }
    bindDateSort(
      `th-fecha-${cfg.pageKey}`,
      cfg.sortKey,
      () => renderList(lastItems),
      claim,
    );
    createTitle ??= field("form-title")?.textContent.trim() ?? "";
    const date = field("fecha-emision");
    if (date && !date.value) date.value = todayLocal();
    renderLines();
    return loadInvoices();
  }

  function bindTabs() {
    const toList = () => showTab("list", listTab());
    for (const [el, handler] of [
      [listTab(), toList],
      [
        createTab(),
        () => {
          // Desde la pestaña siempre se crea: se abandona una edición a medias.
          if (editing) resetForm();
          showTab("create", createTab());
        },
      ],
      [
        button("cancelar"),
        () => {
          resetForm();
          toList();
        },
      ],
      [button("volver"), toList],
      [button("editar"), () => void startEdit()],
    ]) {
      if (claim(el)) el.addEventListener("click", handler);
    }
  }

  function showTab(tab, tabButton = null) {
    document
      .querySelectorAll(`#${cfg.pageId} .view-tabs .tab`)
      .forEach((el) => {
        const isActive = el === tabButton || el.id === `tab-${cfg.view}-${tab}`;
        el.classList.toggle("active", isActive);
      });
    document
      .querySelectorAll(`#${cfg.pageId} .config-subview`)
      .forEach((view) => {
        view.classList.toggle("active", view.id === `${cfg.view}-${tab}`);
      });
    const searchBar = document.getElementById(`${cfg.view}-search-bar`);
    if (searchBar) searchBar.style.display = tab === "list" ? "" : "none";
  }

  function updatePaymentVisibility() {
    const status = field("estado");
    const method = field("metodo-pago");
    if (!status || !method) return;
    const display = !editing && status.value === "completed" ? "" : "none";
    method.style.display = display;
    const wrapper = method.closest(".field");
    if (wrapper) wrapper.style.display = display;
  }

  // ---------- Buscadores ----------

  function mountContactCombobox() {
    if (contactCombobox?.getInput().isConnected) return;
    const search = cfg.contact === "client" ? searchClients : searchSuppliers;
    contactCombobox = mountCombobox(field(cfg.contactField), {
      label: `Buscar ${contactNoun}`,
      placeholder: `Escribe para buscar un ${contactNoun}...`,
      search: (query) => (query ? search(query) : Promise.resolve([])),
      formatOption: formatContactOption,
      addOnlyWhenEmpty: true,
      addOptionLabel: (query) =>
        `${cfg.contact === "client" ? "Cliente" : "Proveedor"} no encontrado · Crear nuevo ${contactNoun} «${query}»`,
      onAddNew: (query) =>
        openQuickContactDialog({
          kind: cfg.contact,
          initialText: query,
        }),
      onSelect: (item) => {
        selectedContact = item;
        if (associated) void loadAssociatedOptions(item);
      },
    });
    const addButton = field(`${cfg.contactField}-add`);
    if (claim(addButton)) {
      addButton.addEventListener("click", async () => {
        const created = await openQuickContactDialog({
          kind: cfg.contact,
          initialText: contactCombobox?.getInputValue() ?? "",
        });
        if (created) contactCombobox?.setSelected(created);
      });
    }
  }

  function mountProductCombobox() {
    if (productCombobox?.getInput().isConnected) return;
    productCombobox = mountCombobox(field(cfg.productField), {
      label: "Buscar producto",
      placeholder: "Escribe para buscar un producto...",
      search: productSearch(isEntry ? "entry" : "outgoing"),
      formatOption: formatProductOption,
      // Una salida no puede llevar un producto agotado (FS-01); una compra sí.
      isOptionDisabled: (item) => !isEntry && item?.isOutOfStock === true,
      // Solo una compra puede traer un producto que aún no existe (FE-01).
      ...(isEntry
        ? {
            addOnlyWhenEmpty: true,
            addOptionLabel: (query) =>
              `Producto no encontrado · Crear producto «${query}»`,
            onAddNew: (query) => startNewProduct(query),
          }
        : {}),
      onSelect: (item) => {
        if (!item) return;
        addProductLine(item);
        productCombobox?.setSelected(null);
      },
    });
  }

  function resetAssociatedSelect(placeholder) {
    const select = document.getElementById(associated.selectId);
    if (!select) return null;
    const empty = document.createElement("option");
    empty.value = "";
    empty.textContent = placeholder;
    select.replaceChildren(empty);
    select.value = "";
    return select;
  }

  // Llena el selector con las facturas asociables vigentes del cliente.
  async function loadAssociatedOptions(client, keepId = null) {
    const request = ++associatedRequest;
    const clientId = client?.id ?? null;
    if (clientId == null) {
      resetAssociatedSelect("Selecciona primero un cliente");
      return;
    }
    const select = resetAssociatedSelect("Cargando facturas del cliente…");
    if (!select) return;
    try {
      const detail = await getClientDetail(Number(clientId));
      // Si el usuario cambió de cliente mientras cargaba, se descarta.
      if (request !== associatedRequest) return;
      const candidates = (detail?.invoiceHistory ?? []).filter(
        (item) =>
          associated.types.includes(item?.invoiceType) &&
          (item?.status !== "cancelled" || Number(item?.id) === keepId),
      );
      resetAssociatedSelect(
        candidates.length ? associated.placeholder : associated.empty,
      );
      for (const item of candidates) {
        const option = document.createElement("option");
        option.value = String(item.id);
        option.textContent = associatedLabel(item);
        select.append(option);
      }
      if (keepId != null) select.value = String(keepId);
    } catch {
      if (request !== associatedRequest) return;
      resetAssociatedSelect("No se pudieron cargar las facturas del cliente");
    }
  }

  // ---------- Líneas ----------

  function addProductLine(item) {
    const productId = Number(item?.id);
    if (!Number.isSafeInteger(productId)) return;
    const name = item?.name ?? "";
    if (
      lines.some(
        (line) =>
          line.productId === productId || line.productIdOriginal === productId,
      )
    ) {
      notifyError(null, `«${name}» ya está en la factura.`);
      return;
    }
    if (!isEntry && item?.isOutOfStock) {
      notifyError(null, `«${name}» está agotado y no se puede agregar.`);
      return;
    }
    lines.push({
      productId,
      productIdOriginal: null,
      productName: name,
      unitAbbreviation: item?.unitAbbreviation ?? null,
      quantity: "1",
      originalQuantity: null,
      unitPrice: "",
      priceCents: Number.isSafeInteger(item?.sellingPriceCents)
        ? item.sellingPriceCents
        : null,
      productStatus: item?.isDiscontinued ? "discontinued" : "active",
    });
    renderLines();
    field("lineas-body")
      ?.querySelector(`[data-line-index="${lines.length - 1}"] input`)
      ?.focus();
  }

  // El producto se crea al guardar la factura, con la cantidad y el costo de
  // su línea; si la factura falla, no se crea (FE-01).
  async function startNewProduct(query) {
    if (editing) {
      // updateEntryInvoice no admite productos nuevos.
      notifyError(
        null,
        "Al editar solo se usan productos existentes. Registra el producto nuevo en una compra nueva.",
      );
      return null;
    }
    const product = await openQuickProductDialog({ initialName: query });
    if (!product) return null;
    const key = product.name.trim().toLowerCase();
    if (lines.some((line) => line.productName.trim().toLowerCase() === key)) {
      notifyError(null, `«${product.name}» ya está en la factura.`);
      return null;
    }
    lines.push({
      productId: null,
      productIdOriginal: null,
      newProduct: {
        name: product.name,
        description: product.description,
        categoryId: product.categoryId,
        unitId: product.unitId,
        profitPercent: product.profitPercent,
      },
      productName: product.name,
      unitAbbreviation: product.unitAbbreviation,
      quantity: "1",
      originalQuantity: null,
      unitPrice: "",
      priceCents: null,
      productStatus: "active",
    });
    productCombobox?.setSelected(null);
    renderLines();
    field("lineas-body")
      ?.querySelector(`[data-line-index="${lines.length - 1}"] input`)
      ?.focus();
    return null;
  }

  // Línea existente cuyo producto se eliminó: solo se puede quitar.
  const isDeletedLine = (line) =>
    line.productIdOriginal != null && line.productStatus == null;
  // Línea existente de un descontinuado: solo se puede quitar o reducir.
  const isDiscontinuedLine = (line) =>
    line.productIdOriginal != null && line.productStatus === "discontinued";

  function lineSubtotalCents(line) {
    const quantityMilli = parseDecimal(line.quantity, 3);
    const unitCents = isEntry
      ? parseDecimal(line.unitPrice, 2)
      : showsSalePrice
        ? line.priceCents
        : null;
    if (quantityMilli == null || unitCents == null) return null;
    return Math.round((quantityMilli * unitCents) / 1000);
  }

  function lineInput(line, index, key, label, disabled) {
    const input = document.createElement("input");
    input.type = "text";
    input.inputMode = "decimal";
    input.value = line[key] ?? "";
    input.disabled = disabled;
    input.dataset.lineField = key;
    input.setAttribute("aria-label", `${label} de ${line.productName}`);
    input.addEventListener("input", () => {
      lines[index][key] = input.value;
      const subtotal = lineSubtotalCents(lines[index]);
      const cell = input
        .closest(".li-row")
        ?.querySelector("[data-line-subtotal]");
      if (cell)
        cell.textContent = subtotal == null ? "—" : formatMoney(subtotal);
      updateTotals();
    });
    return input;
  }

  function renderLines() {
    const container = field("lineas-body");
    if (!container) return;
    container.replaceChildren();
    if (!lines.length) {
      const empty = document.createElement("div");
      empty.className = "li-empty";
      empty.textContent = "Busca un producto para agregarlo a la factura.";
      container.append(empty);
    }
    lines.forEach((line, index) => {
      const deleted = isDeletedLine(line);
      const discontinued = isDiscontinuedLine(line);
      const row = document.createElement("div");
      row.className = "li-row li-body";
      row.dataset.lineIndex = String(index);

      const position = document.createElement("div");
      position.textContent = String(index + 1);
      const name = document.createElement("div");
      name.textContent = deleted
        ? `${line.productName} (eliminado)`
        : discontinued
          ? `${line.productName} (descontinuado)`
          : line.newProduct
            ? `${line.productName} (nuevo)`
            : line.productName;
      const quantity = document.createElement("div");
      quantity.append(lineInput(line, index, "quantity", "Cantidad", deleted));
      const unit = document.createElement("div");
      unit.textContent = line.unitAbbreviation ?? "—";
      const price = document.createElement("div");
      if (isEntry) {
        price.append(
          lineInput(
            line,
            index,
            "unitPrice",
            "Costo unitario",
            deleted || discontinued,
          ),
        );
      } else {
        price.className = "li-costo";
        price.textContent = showsSalePrice
          ? formatMoney(line.priceCents)
          : "Al guardar";
      }
      const subtotal = document.createElement("div");
      subtotal.className = "li-subtotal";
      subtotal.dataset.lineSubtotal = "";
      const subtotalCents = lineSubtotalCents(line);
      subtotal.textContent =
        subtotalCents == null ? "—" : formatMoney(subtotalCents);
      const actions = document.createElement("div");
      const remove = document.createElement("button");
      remove.type = "button";
      remove.className = "li-remove-btn";
      remove.setAttribute("aria-label", `Quitar ${line.productName}`);
      remove.title = "Quitar línea";
      remove.append(trashIcon());
      remove.addEventListener("click", () => {
        lines.splice(index, 1);
        renderLines();
      });
      actions.append(remove);

      row.append(position, name, quantity, unit, price, subtotal, actions);
      container.append(row);
    });
    updateTotals();
  }

  function updateTotals() {
    let text;
    if (isEntry || showsSalePrice) {
      const total = lines.reduce(
        (sum, line) => sum + (lineSubtotalCents(line) ?? 0),
        0,
      );
      text = formatMoney(total);
    } else {
      // Garantía e insumos van a costo real (FIFO): solo se conoce al guardar.
      text = "Se calcula al guardar";
    }
    setText(`${cfg.field}-subtotal`, text);
    setText(`${cfg.field}-total`, text);
  }

  // ---------- Adjunto (solo entrada) ----------

  function setAttachmentLabel(name) {
    setText("factura-archivo-nombre", name ?? "");
    document
      .getElementById("factura-dropzone")
      ?.classList.toggle("has-file", Boolean(name));
  }

  // El backend copia el PDF desde una ruta del disco (FE-07). En Tauri la
  // ruta la dan el diálogo nativo o el evento de arrastre: un File del
  // webview no la trae.
  function setAttachmentPath(path) {
    if (!path) return;
    if (!/\.pdf$/i.test(path)) {
      notifyError(null, "Solo se puede adjuntar un archivo PDF.");
      return;
    }
    pendingAttachmentPath = path;
    removeAttachment = false;
    setAttachmentLabel(path.split(/[\\/]/).pop());
  }

  async function pickAttachment() {
    const open = window.__TAURI__?.dialog?.open;
    if (typeof open !== "function") {
      // Fuera de Tauri queda el selector del navegador.
      document.getElementById("factura-archivo-input")?.click();
      return;
    }
    try {
      const path = await open({
        multiple: false,
        directory: false,
        filters: [{ name: "Documento PDF", extensions: ["pdf"] }],
      });
      if (typeof path === "string") setAttachmentPath(path);
    } catch {
      notifyError(null, "No se pudo abrir el selector de archivos.");
    }
  }

  function chooseAttachment(file) {
    if (!file) return;
    setAttachmentPath(file.path ?? file.name ?? null);
  }

  // Tauri intercepta el arrastre de archivos y entrega sus rutas en un evento.
  function listenTauriDrop() {
    const listen = window.__TAURI__?.event?.listen;
    if (typeof listen !== "function" || window.__facturasDropBound) return;
    window.__facturasDropBound = true;
    void listen("tauri://drag-drop", (event) => {
      const formView = document.getElementById(`${cfg.view}-create`);
      if (!formView?.classList.contains("active")) return;
      setAttachmentPath(event?.payload?.paths?.[0] ?? null);
    });
  }

  function bindAttachment() {
    const input = document.getElementById("factura-archivo-input");
    if (claim(input)) {
      input.addEventListener("change", () =>
        chooseAttachment(input.files?.[0]),
      );
    }
    const dropzone = document.getElementById("factura-dropzone");
    if (claim(dropzone)) {
      dropzone.addEventListener("click", () => void pickAttachment());
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
        chooseAttachment(event.dataTransfer?.files?.[0]);
      });
    }
    listenTauriDrop();
    const removeButton = document.getElementById("factura-archivo-quitar");
    if (claim(removeButton)) {
      removeButton.addEventListener("click", (event) => {
        event.stopPropagation();
        pendingAttachmentPath = null;
        // Al editar, quitar el adjunto existente se aplica al guardar.
        removeAttachment = Boolean(editing?.attachmentOriginalName);
        const fileInput = document.getElementById("factura-archivo-input");
        if (fileInput) fileInput.value = "";
        setAttachmentLabel("");
      });
    }
  }

  async function saveAttachmentChanges(invoiceId) {
    if (!isEntry || invoiceId == null) return;
    const path = pendingAttachmentPath;
    if (!path && !removeAttachment) return;
    try {
      await saveInvoiceAttachment(invoiceId, path ?? null);
    } catch {
      notifyWarning(
        path
          ? "La factura se guardó pero no se pudo adjuntar el PDF."
          : "La factura se guardó pero no se pudo quitar el PDF.",
      );
    }
  }

  // ---------- Formulario: crear y editar ----------

  function bindForm() {
    const formEl = form();
    if (!claim(formEl)) return;
    // Formulario recién cargado en el DOM: nada de un borrador anterior.
    editing = null;
    lines = [];
    selectedContact = null;
    pendingAttachmentPath = null;
    removeAttachment = false;
    currentId = null;
    currentDetail = null;
    formEl.addEventListener("submit", (event) => {
      event.preventDefault();
      void submit();
    });
  }

  function setFormMode(detail) {
    const title = field("form-title");
    if (title) {
      title.textContent = detail
        ? `Editar factura ${detail.number ?? ""}`
        : createTitle;
    }
    const number = field("numero");
    if (number) number.value = detail?.number ?? "";
    // El estado se cambia desde el detalle, no al editar (FE-04, FS-01).
    const statusField = field("estado")?.closest(".field");
    if (statusField) statusField.style.display = detail ? "none" : "";
    if (hasPayment) updatePaymentVisibility();
  }

  function resetForm() {
    editing = null;
    lines = [];
    pendingAttachmentPath = null;
    removeAttachment = false;
    form()?.reset();
    const date = field("fecha-emision");
    if (date) date.value = todayLocal();
    contactCombobox?.setSelected(null);
    productCombobox?.setSelected(null);
    selectedContact = null;
    if (isEntry) setAttachmentLabel("");
    setFormMode(null);
    renderLines();
  }

  async function startEdit() {
    const detail = currentDetail;
    if (!detail || detail.status !== "pending") return;
    resetForm();
    editing = detail;
    setFormMode(detail);
    const contact =
      detail.contactId != null
        ? { id: detail.contactId, fullName: detail.contactFullName ?? "" }
        : null;
    contactCombobox?.setSelected(contact);
    selectedContact = contact;
    const date = field("fecha-emision");
    if (date) date.value = detail.date ?? "";
    const notes = field(cfg.notesField);
    if (notes) notes.value = detail.notes ?? "";
    lines = (detail.lines ?? []).map((line) => ({
      productIdOriginal: line.productIdOriginal,
      productId: line.productStatus ? line.productIdOriginal : null,
      productName: line.productName ?? "",
      unitAbbreviation: line.unitAbbreviation ?? null,
      quantity: line.quantity ?? "",
      originalQuantity: line.quantity ?? "",
      unitPrice: centsToText(line.unitPriceCents),
      // Las líneas existentes conservan su precio emitido (FS-01).
      priceCents: line.unitPriceCents ?? null,
      productStatus: line.productStatus ?? null,
    }));
    if (isEntry) setAttachmentLabel(detail.attachmentOriginalName ?? "");
    renderLines();
    showTab("create", createTab());
    if (associated)
      await loadAssociatedOptions(contact, detail.associatedInvoiceId);
  }

  function setSubmitting(isSubmitting) {
    const submitButton = form()?.querySelector('button[type="submit"]');
    if (submitButton) submitButton.disabled = isSubmitting;
  }

  function validateLines() {
    const missing = lines.find(
      (line) =>
        !String(line.quantity ?? "").trim() ||
        (isEntry && !String(line.unitPrice ?? "").trim()),
    );
    if (missing) {
      return isEntry
        ? `Escribe la cantidad y el costo unitario de «${missing.productName}».`
        : `Escribe la cantidad de «${missing.productName}».`;
    }
    const raised = lines.find((line) => {
      if (!isDiscontinuedLine(line)) return false;
      const next = parseDecimal(line.quantity, 3);
      const before = parseDecimal(line.originalQuantity, 3);
      return next != null && before != null && next > before;
    });
    if (raised) {
      return `«${raised.productName}» está descontinuado: solo puedes reducir su cantidad.`;
    }
    return "";
  }

  function buildInput(contactId, associatedId) {
    const invoiceDate = field("fecha-emision")?.value ?? "";
    const notes = field(cfg.notesField)?.value.trim() || undefined;
    const input = editing
      ? { invoiceId: editing.id, [cfg.contactKey]: Number(contactId) }
      : { [cfg.contactKey]: Number(contactId) };
    if (associated) input.associatedInvoiceId = Number(associatedId);
    input.invoiceDate = invoiceDate;
    if (!editing) {
      const status = field("estado")?.value ?? "pending";
      input.status = status;
      const paymentMethod = field("metodo-pago")?.value;
      if (hasPayment && status === "completed" && paymentMethod) {
        input.paymentMethod = paymentMethod;
      }
    }
    if (notes) input.notes = notes;
    input.lines = lines.map((line) => {
      const item = {};
      if (editing && line.productIdOriginal != null) {
        item.productIdOriginal = line.productIdOriginal;
      }
      // Un producto eliminado no tiene id vigente: nunca se envía NaN.
      if (line.productId != null) item.productId = line.productId;
      // El costo inicial del producto nuevo es el de su línea (FE-01).
      if (line.newProduct) {
        item.newProduct = {
          ...line.newProduct,
          cost: String(line.unitPrice).trim(),
        };
      }
      item.quantity = String(line.quantity).trim();
      if (isEntry) item.unitPrice = String(line.unitPrice).trim();
      return item;
    });
    return input;
  }

  async function submit() {
    if (saving) return;
    clearFieldErrors(form());
    const contactId = selectedContact?.id ?? null;
    const associatedId = associated
      ? (document.getElementById(associated.selectId)?.value ?? "")
      : null;
    const invoiceDate = field("fecha-emision")?.value ?? "";
    if (
      contactId == null ||
      !invoiceDate ||
      !lines.length ||
      (associated && !associatedId)
    ) {
      if (contactId == null) {
        setFieldError(contactCombobox?.getInput(), "Elige un contacto");
      }
      if (!invoiceDate)
        setFieldError(field("fecha-emision"), "Completa este campo");
      notifyError(null, cfg.missingMessage);
      return;
    }
    const lineProblem = validateLines();
    if (lineProblem) {
      notifyError(null, lineProblem);
      return;
    }

    const wasEditing = editing;
    const input = buildInput(contactId, associatedId);
    saving = true;
    setSubmitting(true);
    try {
      const result = wasEditing
        ? await cfg.update(input)
        : await cfg.create(input);
      const invoiceId = wasEditing ? wasEditing.id : result?.invoiceId;
      await saveAttachmentChanges(invoiceId);
      notifySuccess(
        wasEditing
          ? "Factura actualizada correctamente."
          : "Factura registrada correctamente.",
      );
      notifyWarning(result?.expiredCleanupWarning);
      resetForm();
      await loadInvoices();
      if (wasEditing) await openDetail(invoiceId);
      else showTab("list", listTab());
    } catch (error) {
      // Lo tecleado se conserva para que el usuario corrija y reintente.
      notifyError(error, "No se pudo guardar la factura.");
    } finally {
      saving = false;
      setSubmitting(false);
    }
  }

  // ---------- Listado ----------

  function bindTableActions() {
    const body = document.getElementById(cfg.bodyId);
    if (!claim(body)) return;
    body.addEventListener("click", (event) => {
      const action = event.target.closest("[data-invoice-action]");
      if (!action) return;
      const invoiceId = Number(action.dataset.invoiceId);
      if (!Number.isFinite(invoiceId)) return;
      if (action.dataset.invoiceAction === "detail") void openDetail(invoiceId);
      else if (action.dataset.invoiceAction === "delete") {
        void handleDelete(invoiceId);
      }
    });
  }

  async function loadInvoices() {
    const body = document.getElementById(cfg.bodyId);
    if (!body) return;
    messageRow(body, `Cargando facturas de ${cfg.noun}…`);
    try {
      const response = await listInvoices({
        invoiceType: cfg.invoiceType,
        ...readListFilters(cfg.view, true),
      });
      renderList(
        toItems(response).filter(
          (item) => (item?.invoiceType ?? cfg.invoiceType) === cfg.invoiceType,
        ),
      );
    } catch (error) {
      const message = listErrorMessage(
        error,
        `No se pudieron cargar las facturas de ${cfg.noun}.`,
      );
      messageRow(body, message);
      notifyError(null, message);
    }
  }

  function renderList(items) {
    lastItems = items;
    const body = document.getElementById(cfg.bodyId);
    if (!body) return;
    if (!items.length) {
      messageRow(body, `Aún no hay facturas de ${cfg.noun}.`);
      return;
    }
    body.replaceChildren();
    for (const item of sortByDate(items, `th-fecha-${cfg.pageKey}`)) {
      const row = document.createElement("tr");
      const texts = [
        item?.number ?? "—",
        item?.contactName ?? "—",
        item?.date ? formatDate(item.date) : "—",
      ];
      if (associated) {
        texts.push(
          item?.associatedInvoice
            ? associatedLabel(item.associatedInvoice)
            : "—",
        );
      }
      texts.push(
        formatMoney(item?.totalCents ?? 0),
        cfg.statusLabels[item?.status] ?? item?.status ?? "—",
      );
      if (cfg.showsProductCount) texts.push(String(item?.productCount ?? "—"));
      const cells = texts.map((text) => {
        const cell = document.createElement("td");
        cell.textContent = text;
        return cell;
      });
      const actionsCell = document.createElement("td");
      for (const [action, label] of [
        ["detail", "Ver"],
        ["delete", "Eliminar"],
      ]) {
        const actionButton = document.createElement("button");
        actionButton.type = "button";
        actionButton.textContent = label;
        actionButton.dataset.invoiceAction = action;
        actionButton.dataset.invoiceId = String(item?.id ?? "");
        actionsCell.append(actionButton);
      }
      row.append(...cells, actionsCell);
      body.append(row);
    }
  }

  // ---------- Detalle y cambio de estado ----------

  function bindDetailActions() {
    const exportButton = button("exportar");
    if (claim(exportButton)) {
      exportButton.addEventListener("click", () => {
        const exporter = window[cfg.exportFn];
        if (currentDetail && typeof exporter === "function") {
          exporter(currentDetail);
        }
      });
    }
    const deleteButton = button("eliminar");
    if (claim(deleteButton)) {
      deleteButton.addEventListener("click", () => {
        if (currentId != null) void handleDelete(currentId);
      });
    }
    const downloadButton = button("guardar-equipo");
    if (claim(downloadButton)) {
      downloadButton.addEventListener("click", () => {
        void handleDownloadAttachment();
      });
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
      notifyError(error, "No se pudo abrir la factura.");
    }
  }

  function renderDetail(detail) {
    const set = (suffix, value) => setText(`${cfg.field}-ver-${suffix}`, value);
    const status = cfg.statusLabels[detail?.status] ?? "—";
    const total = formatMoney(detail?.totalCents ?? 0);
    set("numero-badge", detail?.number ?? "");
    set("titulo", `${cfg.title} ${detail?.number ?? ""}`);
    set("estado-txt", status);
    set("estado-hero", status);
    if (hasPayment) {
      set(
        "metodo",
        detail?.paymentMethod
          ? (PAYMENT_LABELS[detail.paymentMethod] ?? detail.paymentMethod)
          : "—",
      );
    }
    for (const suffix of [
      "total",
      "subtotal",
      "subtotal-resumen",
      "total-resumen",
    ]) {
      set(suffix, total);
    }
    set(cfg.contactField, detail?.contactFullName ?? "—");
    set("numero", detail?.number ?? "");
    set("emision", detail?.date ? formatDate(detail.date) : "—");
    set("descripcion", detail?.notes ?? "—");
    if (isEntry) {
      set("observaciones", detail?.notes ?? "—");
      set("archivo", detail?.attachmentOriginalName ?? "Sin archivo adjunto");
    }
    if (associated) {
      setText(
        associated.viewId,
        detail?.associatedInvoice
          ? associatedLabel(detail.associatedInvoice)
          : "—",
      );
    }

    const products = document.getElementById(`${cfg.field}-ver-productos`);
    if (products) {
      products.replaceChildren();
      for (const [index, line] of (detail?.lines ?? []).entries()) {
        const row = document.createElement("div");
        row.className = "li-row li-body li-body-readonly";
        // En garantía e insumos el precio de la línea es el costo real.
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
        products.append(row);
      }
    }

    // Solo las facturas pendientes se pueden editar (FE-04, FS-01).
    const editButton = button("editar");
    if (editButton) editButton.hidden = detail?.status !== "pending";
    renderStatusActions(detail);
    renderRegistration(`${cfg.view}-ver`, detail);
  }

  function renderStatusActions(detail) {
    const view = document.getElementById(`${cfg.view}-ver`);
    if (!view) return;
    const boxId = `${cfg.view}-estado-acciones`;
    let box = document.getElementById(boxId);
    if (!box) {
      box = document.createElement("div");
      box.id = boxId;
      box.className = "form-actions";
      const actions = view.querySelector(".form-actions");
      if (actions) actions.before(box);
      else view.append(box);
    }
    box.replaceChildren();
    const transitions = allowedTransitions(cfg.invoiceType, detail?.status);
    box.hidden = !transitions.length;
    for (const target of transitions) {
      let paymentSelect = null;
      if (target === "completed" && hasPayment) {
        const wrapper = document.createElement("div");
        wrapper.className = "field";
        const label = document.createElement("label");
        label.htmlFor = `${cfg.view}-estado-metodo`;
        label.textContent = "Método de pago";
        paymentSelect = document.createElement("select");
        paymentSelect.id = `${cfg.view}-estado-metodo`;
        for (const [value, text] of Object.entries(PAYMENT_LABELS)) {
          const option = document.createElement("option");
          option.value = value;
          option.textContent = text;
          paymentSelect.append(option);
        }
        paymentSelect.value = detail?.paymentMethod ?? "cash";
        wrapper.append(label, paymentSelect);
        box.append(wrapper);
      }
      const actionButton = document.createElement("button");
      actionButton.type = "button";
      actionButton.className =
        target === "cancelled" ? "btn-danger" : "btn-secondary";
      actionButton.dataset.statusTarget = target;
      actionButton.textContent =
        target === "cancelled"
          ? "Cancelar factura"
          : `Marcar como ${cfg.statusLabels[target]}`;
      actionButton.addEventListener("click", () => {
        void handleStatusChange(target, paymentSelect);
      });
      box.append(actionButton);
    }
  }

  async function handleStatusChange(target, paymentSelect) {
    if (currentId == null) return;
    if (target === "cancelled") {
      const confirmed = await confirmDialog({
        title: "Cancelar factura",
        message: cfg.cancelMessage,
        confirmText: "Cancelar factura",
        cancelText: "Volver",
      });
      if (!confirmed) return;
    }
    const invoiceId = currentId;
    try {
      const result = await changeInvoiceStatus(
        invoiceId,
        target,
        target === "completed" && hasPayment ? paymentSelect?.value : undefined,
      );
      notifySuccess("Estado de la factura actualizado.");
      notifyWarning(result?.expiredCleanupWarning);
      await loadInvoices();
      await openDetail(invoiceId);
    } catch (error) {
      notifyError(error, "No se pudo cambiar el estado de la factura.");
    }
  }

  async function handleDownloadAttachment() {
    if (currentId == null) return;
    const destination = await resolveDownloadDestination();
    if (!destination) return;
    try {
      await downloadInvoiceAttachment(currentId, destination);
      notifySuccess("PDF guardado.");
    } catch (error) {
      notifyError(error, "No se pudo guardar el archivo.");
    }
  }

  async function handleDelete(invoiceId) {
    let message = `¿Deseas eliminar esta factura de ${cfg.noun}?`;
    if (isEntry) {
      try {
        const detail =
          currentDetail && currentId === Number(invoiceId)
            ? currentDetail
            : await getInvoice(Number(invoiceId));
        message = entryDeleteMessage(detail);
      } catch {
        // Sin el detalle se confirma con el mensaje general.
      }
    }
    const confirmed = await confirmDialog({
      title: "Eliminar factura",
      message,
      confirmText: "Eliminar",
    });
    if (!confirmed) return;
    try {
      const deleted = await deleteInvoice(Number(invoiceId));
      notifySuccess("Factura eliminada.");
      notifyWarning(deleted?.expiredCleanupWarning);
      if (currentId === Number(invoiceId)) {
        currentId = null;
        currentDetail = null;
      }
      await loadInvoices();
      showTab("list", listTab());
    } catch (error) {
      notifyError(error, "No se pudo eliminar la factura.");
    }
  }

  return {
    init,
    reload: () => loadInvoices(),
    showTab,
    openDetail,
    submit,
    setPendingAttachment: (path) => {
      pendingAttachmentPath = path;
    },
  };
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

const OUTGOING_CANCEL_MESSAGE =
  "Los productos de esta factura vuelven al inventario. Una factura cancelada no se puede reactivar ni editar.";

const entrySubmodule = createInvoiceSubmodule({
  invoiceType: "E",
  pageId: "page-facturas-entrada",
  pageKey: "facturas-entrada",
  sortKey: "facturasEntrada",
  showsProductCount: true,
  view: "factura-entrada",
  field: "factura",
  buttons: "factura-entrada",
  bodyId: "facturas-entrada-body",
  contact: "supplier",
  contactField: "proveedor",
  contactKey: "supplierId",
  productField: "buscar-producto",
  notesField: "observaciones",
  statusLabels: ENTRY_STATUS_LABELS,
  missingMessage: "Completa el proveedor, la fecha y al menos un producto.",
  cancelMessage:
    "Las unidades que quedan de esta compra salen del inventario. Una factura cancelada no se puede reactivar ni editar.",
  noun: "entrada",
  title: "Factura de Entrada",
  exportFn: "exportarFacturaPDF",
  windowFlag: "__facturasEntryPageChangeBound",
  create: (input) => createEntryInvoice(input),
  update: (input) => updateEntryInvoice(input),
});

const saleSubmodule = createInvoiceSubmodule({
  invoiceType: "V",
  pageId: "page-facturas-venta",
  pageKey: "facturas-venta",
  sortKey: "facturasVenta",
  view: "factura-venta",
  field: "factura-venta",
  buttons: "factura-venta",
  bodyId: "facturas-venta-body",
  contact: "client",
  contactField: "cliente",
  contactKey: "clientId",
  productField: "buscar-producto",
  notesField: "descripcion",
  statusLabels: SALE_STATUS_LABELS,
  missingMessage: "Completa el cliente, la fecha y al menos un producto.",
  cancelMessage: OUTGOING_CANCEL_MESSAGE,
  noun: "venta",
  title: "Factura de Venta",
  exportFn: "exportarFacturaVentaPDF",
  windowFlag: "__facturasSalePageChangeBound",
  create: (input) => createSaleInvoice(input),
  update: (input) => updateSaleInvoice(input),
});

const serviceSubmodule = createInvoiceSubmodule({
  invoiceType: "S",
  pageId: "page-facturas-servicios",
  pageKey: "facturas-servicios",
  sortKey: "facturasServicios",
  view: "factura-servicios",
  field: "factura-servicio",
  buttons: "factura-servicio",
  bodyId: "facturas-servicios-body",
  contact: "client",
  contactField: "cliente",
  contactKey: "clientId",
  productField: "buscar",
  notesField: "descripcion",
  statusLabels: SALE_STATUS_LABELS,
  missingMessage: "Completa el cliente, la fecha y al menos un producto.",
  cancelMessage: OUTGOING_CANCEL_MESSAGE,
  noun: "servicio",
  title: "Factura de Servicio",
  exportFn: "exportarFacturaServicioPDF",
  windowFlag: "__facturasServicePageChangeBound",
  create: (input) => createServiceInvoice(input),
  update: (input) => updateServiceInvoice(input),
});

// Garantía (FS-04) e insumos (FS-05): factura asociada elegida en un
// <select>, sin método de pago y a costo real.
const guaranteeSubmodule = createInvoiceSubmodule({
  invoiceType: "GG",
  pageId: "page-facturas-garantia",
  pageKey: "facturas-garantia",
  sortKey: "facturasGarantia",
  view: "factura-garantia",
  field: "factura-garantia",
  buttons: "factura-garantia",
  bodyId: "facturas-garantia-body",
  contact: "client",
  contactField: "cliente",
  contactKey: "clientId",
  productField: "buscar",
  notesField: "descripcion",
  statusLabels: ASSOCIATED_STATUS_LABELS,
  associated: {
    selectId: "factura-garantia-original",
    viewId: "factura-garantia-ver-original",
    types: ["V", "S"],
    placeholder: "Selecciona la factura original",
    empty: "Este cliente no tiene ventas ni servicios vigentes",
  },
  missingMessage:
    "Completa el cliente, la factura original, la fecha y al menos un producto.",
  cancelMessage: OUTGOING_CANCEL_MESSAGE,
  noun: "garantía",
  title: "Factura de Garantía",
  exportFn: "exportarFacturaGarantiaPDF",
  windowFlag: "__facturasGuaranteePageChangeBound",
  create: (input) => createGuaranteeInvoice(input),
  update: (input) => updateGuaranteeInvoice(input),
});

const installationSubmodule = createInvoiceSubmodule({
  invoiceType: "GI",
  pageId: "page-facturas-insumos",
  pageKey: "facturas-insumos",
  sortKey: "facturasInsumos",
  view: "factura-insumos",
  field: "factura-insumo",
  buttons: "factura-insumo",
  bodyId: "facturas-insumos-body",
  contact: "client",
  contactField: "cliente",
  contactKey: "clientId",
  productField: "buscar",
  notesField: "descripcion",
  statusLabels: ASSOCIATED_STATUS_LABELS,
  associated: {
    selectId: "factura-insumo-servicio",
    viewId: "factura-insumo-ver-asociada",
    types: ["S"],
    placeholder: "Selecciona la factura de servicio",
    empty: "Este cliente no tiene servicios vigentes",
  },
  missingMessage:
    "Completa el cliente, la factura de servicio, la fecha y al menos un insumo.",
  cancelMessage: OUTGOING_CANCEL_MESSAGE,
  noun: "insumos",
  title: "Factura de Insumos",
  exportFn: "exportarFacturaInsumoPDF",
  windowFlag: "__facturasInstallationPageChangeBound",
  create: (input) => createInstallationInvoice(input),
  update: (input) => updateInstallationInvoice(input),
});

export const initEntryInvoicesView = entrySubmodule.init;
export const showEntryTab = entrySubmodule.showTab;
export const openEntryDetail = entrySubmodule.openDetail;
export const initSaleInvoicesView = saleSubmodule.init;
export const showSaleTab = saleSubmodule.showTab;
export const openSaleDetail = saleSubmodule.openDetail;
export const initServiceInvoicesView = serviceSubmodule.init;
export const showServiceTab = serviceSubmodule.showTab;
export const openServiceDetail = serviceSubmodule.openDetail;
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
let savingLoss = false;
let lossItems = [];
let generalItems = [];

export function initLossInvoicesView() {
  const root = document.getElementById("page-facturas-perdidas");
  if (!root) return Promise.resolve();
  if (!window.__facturasLossPageChangeBound) {
    window.addEventListener("page:change", (event) => {
      if (event.detail?.pageId !== "facturas-perdidas") return;
      void initLossInvoicesView();
    });
    window.__facturasLossPageChangeBound = true;
  }
  bindLossTabs();
  bindListFilters("factura-perdidas", loadLossInvoices, claimLoss);
  bindLossProductCombobox();
  bindLossForm();
  bindLossTableActions();
  bindLossDetailActions();
  bindDateSort(
    "th-fecha-facturas-perdidas",
    "facturasPerdidas",
    () => renderLossList(lossItems),
    claimLoss,
  );
  const date = document.getElementById("factura-perdida-fecha");
  if (date && !date.value) date.value = todayLocal();
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

// Combobox sin "Agregar nueva": no se pasa onAddNew (FE-FP-03). Ofrece
// también los descontinuados con stock (FP-01, S-22).
function bindLossProductCombobox() {
  const container = document.getElementById(
    "factura-perdida-producto-combobox",
  );
  if (!claimLoss(container)) return;
  lossProductCombobox = createCombobox({
    container,
    id: "factura-perdida-producto",
    placeholder: "Escribe para buscar un producto...",
    search: productSearch("loss"),
    formatOption: formatProductOption,
    isOptionDisabled: (item) => item?.isOutOfStock === true,
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

async function loadLossInvoices() {
  const body = document.getElementById("facturas-perdidas-body");
  if (!body) return;
  messageRow(body, "Cargando facturas de pérdidas…");
  try {
    const response = await listInvoices({
      invoiceType: "P",
      ...readListFilters("factura-perdidas", false),
    });
    renderLossList(
      toItems(response).filter((item) => (item?.invoiceType ?? "P") === "P"),
    );
  } catch (error) {
    const message = listErrorMessage(
      error,
      "No se pudieron cargar las facturas de pérdidas.",
    );
    messageRow(body, message);
    notifyError(null, message);
  }
}

function renderLossList(items) {
  lossItems = items;
  const body = document.getElementById("facturas-perdidas-body");
  if (!body) return;
  if (!items.length) {
    messageRow(body, "Aún no hay facturas de pérdidas.");
    return;
  }
  body.replaceChildren();
  for (const item of sortByDate(items, "th-fecha-facturas-perdidas")) {
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
  if (savingLoss) return;
  const product = lossProductCombobox?.getSelected() ?? null;
  const quantity = String(
    document.getElementById("factura-perdida-cantidad")?.value ?? "",
  ).trim();
  const invoiceDate =
    document.getElementById("factura-perdida-fecha")?.value ?? "";
  const notes =
    document.getElementById("factura-perdida-descripcion")?.value.trim() ||
    undefined;

  clearFieldErrors(document.getElementById("factura-perdidas-form"));
  if (!product || !quantity || !invoiceDate) {
    if (!product) {
      setFieldError(lossProductCombobox?.getInput(), "Elige un producto");
    }
    requireFields(["factura-perdida-cantidad", "factura-perdida-fecha"]);
    notifyError(null, "Completa el producto, la cantidad y la fecha.");
    return;
  }

  // El costo no se envía: lo calcula el backend (FP-01).
  const input = {
    productId: Number(product.id),
    quantity,
    invoiceDate,
    ...(notes ? { notes } : {}),
  };

  const submitButton = document.querySelector(
    '#factura-perdidas-form button[type="submit"]',
  );
  savingLoss = true;
  if (submitButton) submitButton.disabled = true;
  try {
    const result = await createLossInvoice(input);
    notifySuccess("Pérdida registrada correctamente.");
    notifyWarning(result?.expiredCleanupWarning);
    lossProductCombobox?.setSelected(null);
    const cantidad = document.getElementById("factura-perdida-cantidad");
    if (cantidad) cantidad.value = "1";
    const descripcion = document.getElementById("factura-perdida-descripcion");
    if (descripcion) descripcion.value = "";
    await loadLossInvoices();
    // Tras guardar se abre el detalle, que ya trae el costo real.
    if (result?.invoiceId != null) await openLossDetail(result.invoiceId);
  } catch (error) {
    notifyError(error, "No se pudo guardar la pérdida.");
  } finally {
    savingLoss = false;
    if (submitButton) submitButton.disabled = false;
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
    notifyError(error, "No se pudo abrir la pérdida.");
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
  renderRegistration("factura-perdidas-ver", detail);
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
    notifySuccess("Pérdida eliminada.");
    notifyWarning(deleted?.expiredCleanupWarning);
    if (currentLossId === Number(invoiceId)) {
      currentLossId = null;
      currentLossDetail = null;
    }
    await loadLossInvoices();
    showLossTab("list", document.getElementById("tab-factura-perdidas-list"));
  } catch (error) {
    notifyError(error, "No se pudo eliminar la pérdida.");
  }
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

const GENERAL_FILTER_IDS = [
  "factura-general-search",
  "factura-general-tipo",
  "factura-general-estado",
  "factura-general-desde",
  "factura-general-hasta",
];

function claimGeneral(el) {
  if (!el || el.dataset.generalBound === "1") return false;
  el.dataset.generalBound = "1";
  return true;
}

export function initGeneralInvoicesView() {
  const search = document.getElementById("factura-general-search");
  if (!search) return Promise.resolve();
  bindListFilters("factura-general", loadGeneralInvoices, claimGeneral);
  for (const id of ["factura-general-tipo", "factura-general-estado"]) {
    const control = document.getElementById(id);
    if (claimGeneral(control)) {
      control.addEventListener("change", () => void loadGeneralInvoices());
    }
  }
  const body = document.getElementById("facturas-general-body");
  if (claimGeneral(body)) {
    body.addEventListener("click", (event) => {
      const button = event.target.closest("[data-general-type]");
      if (!button) return;
      void irAFacturaDesdeGeneral(
        button.dataset.generalType,
        Number(button.dataset.generalId),
      );
    });
  }
  bindDateSort(
    "th-fecha-facturas-general",
    "facturasGeneral",
    () => renderFacturasGeneral(generalItems),
    claimGeneral,
  );
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
  const filters = readListFilters("factura-general", false);
  if (value("factura-general-tipo")) {
    filters.invoiceType = value("factura-general-tipo");
  }
  if (value("factura-general-estado")) {
    filters.status = value("factura-general-estado");
  }
  messageRow(body, "Cargando facturas…");
  try {
    const response = await listInvoices(filters);
    renderFacturasGeneral(toItems(response));
  } catch (error) {
    messageRow(
      body,
      listErrorMessage(error, "No se pudieron cargar las facturas."),
    );
  }
}

export function renderFacturasGeneral(items) {
  generalItems = items;
  const body = document.getElementById("facturas-general-body");
  if (!body) return;
  if (!items.length) {
    const filtered = GENERAL_FILTER_IDS.some(
      (id) => document.getElementById(id)?.value,
    );
    messageRow(
      body,
      filtered
        ? "No se encontraron facturas que coincidan con los filtros."
        : "Aún no hay facturas.",
    );
    return;
  }
  body.replaceChildren();
  for (const item of sortByDate(items, "th-fecha-facturas-general")) {
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
      document.querySelector(`#sidebar [data-page="${target.page}"]`),
    );
  }
  await target.open(invoiceId);
}

// Compatibilidad con llamadas globales del HTML real hasta migrar el mock.
const submitWith = (submodule) => (event) => {
  if (event) event.preventDefault();
  void submodule.submit();
};
window.showFacturaEntradaTab = showEntryTab;
window.showFacturaVentaTab = showSaleTab;
window.showFacturaServiciosTab = showServiceTab;
window.showFacturaGarantiaTab = showGuaranteeTab;
window.showFacturaInsumosTab = showInstallationTab;
window.showFacturaPerdidasTab = showLossTab;
window.guardarFactura = submitWith(entrySubmodule);
window.guardarFacturaVenta = submitWith(saleSubmodule);
window.guardarFacturaServicio = submitWith(serviceSubmodule);
window.guardarFacturaGarantia = submitWith(guaranteeSubmodule);
window.guardarFacturaInsumo = submitWith(installationSubmodule);
window.guardarFacturaPerdida = (event) => {
  if (event) event.preventDefault();
  void handleLossSubmit();
};
window.verFactura = (id) => void openEntryDetail(id);
window.verFacturaVenta = (id) => void openSaleDetail(id);
window.verFacturaServicio = (id) => void openServiceDetail(id);
window.verFacturaGarantia = (id) => void openGuaranteeDetail(id);
window.verFacturaInsumo = (id) => void openInstallationDetail(id);
window.verFacturaPerdida = (id) => void openLossDetail(id);
window.renderFacturasGeneral = () => void loadGeneralInvoices();
// Los listados con los nombres globales antiguos (tarea 4.5.7): mismas vistas
// con textContent que sustituyen a las de dashboard.js.
window.renderFacturasEntrada = () => void entrySubmodule.reload();
window.renderFacturasVenta = () => void saleSubmodule.reload();
window.renderFacturasServicios = () => void serviceSubmodule.reload();
window.renderFacturasGarantia = () => void guaranteeSubmodule.reload();
window.renderFacturasInsumos = () => void installationSubmodule.reload();
window.renderFacturasPerdidas = () => void loadLossInvoices();
window.irAFacturaDesdeGeneral = (type, id) =>
  void irAFacturaDesdeGeneral(type, Number(id));

// Solo para tests: datos que el usuario no teclea (ruta del PDF y destino del
// diálogo nativo) y apertura directa de detalles. La selección de contactos y
// productos se prueba como la hace el usuario (task-03 §7).
export const __testHooks = {
  setPendingAttachment: entrySubmodule.setPendingAttachment,
  setDownloadDestination: (path) => {
    downloadDestinationOverride = path;
  },
  openEntryDetail,
  openSaleDetail,
  openServiceDetail,
  openGuaranteeDetail,
  openInstallationDetail,
  openLossDetail,
  getLossCombobox: () => lossProductCombobox,
};

function initPresentPages() {
  for (const [pageId, init] of [
    ["page-facturas-entrada", initEntryInvoicesView],
    ["page-facturas-venta", initSaleInvoicesView],
    ["page-facturas-servicios", initServiceInvoicesView],
    ["page-facturas-garantia", initGuaranteeInvoicesView],
    ["page-facturas-insumos", initInstallationInvoicesView],
    ["page-facturas-perdidas", initLossInvoicesView],
    ["page-facturas-general", initGeneralInvoicesView],
  ]) {
    if (document.getElementById(pageId)) void init();
  }
}

if (document.readyState === "loading") {
  document.addEventListener("DOMContentLoaded", initPresentPages);
} else {
  initPresentPages();
}
