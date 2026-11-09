import { emptyState } from "../utils/dom.js";
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
  exportInvoicePdf,
  getInvoicePrintModel,
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
import {
  getSupplierDetail,
  searchSuppliers,
} from "../services/suppliersService.js";
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
import { openPrintPreview } from "./printPreview.js";
import {
  getProduct,
  listUnitOptionsForProduct,
} from "../services/productsService.js";
import {
  attachDraft,
  registerDraftOpener,
  restoreForm,
  serializeForm,
} from "../utils/drafts.js";

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

/** Factor "12" o "0,083333" de listUnitOptionsForProduct → número. */
function parseFactor(value) {
  const factor = Number(String(value ?? "1").replace(",", "."));
  return Number.isFinite(factor) && factor > 0 ? factor : 1;
}

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
    Number.isSafeInteger(item?.sellingPriceCents)
      ? formatMoney(item.sellingPriceCents)
      : null,
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
  if (!affected.length) {
    return `¿Deseas eliminar la factura ${detail?.number ?? "de entrada"}?`;
  }
  const items = affected.map((line) => {
    const unit = line.unitAbbreviation ? ` ${line.unitAbbreviation}` : "";
    const consumed =
      line.consumedQuantity && line.consumedQuantity !== "0"
        ? `; ya se consumieron ${line.consumedQuantity}${unit} en salidas`
        : "";
    return `• ${line.productName ?? ""}: se perderán ${line.remainingQuantity}${unit}${consumed}.`;
  });
  return [
    `Al eliminar la compra ${detail?.number ?? ""} se quitarán del inventario las unidades que aún quedan:`,
    ...items,
    "¿Deseas continuar?",
  ].join("\n");
}

// ============ Submódulos: entrada, venta, servicio, garantía e insumos ============
// Comparten listado, formulario (crear y editar), detalle y cambio de estado;
// las diferencias de cada tipo viven en su configuración.

// §7.2: el mensaje nombra lo afectado.
function invoiceSavedMessage(number) {
  return number ? `Factura ${number} guardada.` : "Factura guardada.";
}

// BR-02: el contacto de un borrador se vuelve a comprobar al recuperarlo.
async function existingContact(kind, contact) {
  if (contact?.id == null) return null;
  const read = kind === "client" ? getClientDetail : getSupplierDetail;
  try {
    return (await read(Number(contact.id))) ? contact : null;
  } catch {
    return null;
  }
}

async function productExists(productId, mustBeActive) {
  try {
    const product = await getProduct(Number(productId));
    return Boolean(product) && (!mustBeActive || product.status === "active");
  } catch {
    return false;
  }
}

function createInvoiceSubmodule(cfg) {
  const isEntry = cfg.invoiceType === "E";
  const hasPayment = ["E", "V", "S"].includes(cfg.invoiceType);
  const showsSalePrice = cfg.invoiceType === "V" || cfg.invoiceType === "S";
  // RS-01: solo ventas y servicios se reservan.
  const reservable = showsSalePrice;
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
  let draft = null;

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
      [createTab(), startNew],
      [
        button("cancelar"),
        () => {
          draft?.clear();
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

  function startNew() {
    // Desde la pestaña siempre se crea: se abandona una edición a medias.
    if (editing) resetForm();
    showTab("create", createTab());
    return draft?.offer();
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
    const reserve = field("reserva-field");
    if (reserve) {
      reserve.hidden = !(reservable && (editing || status.value === "pending"));
    }
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
        draft?.schedule();
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
      calculatedPriceCents: item?.calculatedPriceCents ?? null,
      salePrice: "",
      discountKind: "",
      discountValue: "",
      productStatus: item?.isDiscontinued ? "discontinued" : "active",
      unitId: null,
      factor: 1,
    });
    renderLines();
    draft?.schedule();
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
        lowStockThreshold: product.lowStockThreshold,
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

  // spec-06 PF-01: el precio escrito; vacío, el de lista.
  function salePriceCents(line) {
    const typed = String(line.salePrice ?? "").trim();
    if (!typed) return line.priceCents ?? null;
    return parseDecimal(typed, 2);
  }

  // CV-01: precio × factor de la unidad elegida (total preliminar).
  function linePriceCents(line) {
    const price = showsSalePrice ? salePriceCents(line) : line.priceCents;
    if (price == null) return null;
    return Math.round(price * (line.factor ?? 1));
  }

  // spec-06 PF-02: descuento en % (centésimas) o en importe.
  function lineDiscountCents(line, grossCents) {
    const value = parseDecimal(line.discountValue, 2);
    if (!showsSalePrice || value == null || !line.discountKind) return 0;
    return line.discountKind === "percent"
      ? Math.round((grossCents * value) / 10000)
      : value;
  }

  function lineSubtotalCents(line) {
    const quantityMilli = parseDecimal(line.quantity, 3);
    const unitCents = isEntry
      ? parseDecimal(line.unitPrice, 2)
      : showsSalePrice
        ? linePriceCents(line)
        : null;
    if (quantityMilli == null || unitCents == null) return null;
    const gross = Math.round((quantityMilli * unitCents) / 1000);
    return Math.max(gross - lineDiscountCents(line, gross), 0);
  }

  function priceBelowList(line) {
    const price = salePriceCents(line);
    return (
      showsSalePrice &&
      price != null &&
      line.priceCents != null &&
      price < line.priceCents
    );
  }

  /** Precio editable y descuento de una línea de Venta o Servicio. */
  function salePriceControls(line, index, disabled) {
    const wrap = document.createElement("div");
    wrap.className = "li-price";
    // Con otra unidad, el precio es el de esa unidad: solo se muestra.
    if (line.unitId != null) {
      const fixed = document.createElement("span");
      fixed.className = "li-costo";
      fixed.textContent = formatMoney(linePriceCents(line));
      wrap.append(fixed);
    } else {
      const price = lineInput(line, index, "salePrice", "Precio", disabled);
      price.placeholder = centsToText(line.priceCents);
      price.addEventListener("blur", () => {
        if (priceBelowList(lines[index])) {
          setFieldError(
            price,
            `El precio no puede ser menor que ${formatMoney(line.priceCents)}`,
          );
        }
      });
      wrap.append(price);
      const calculated = line.calculatedPriceCents;
      if (calculated != null && calculated !== line.priceCents) {
        const hint = document.createElement("small");
        hint.className = "price-calculated";
        hint.title = "Precio calculado, antes de redondear";
        hint.textContent = formatMoney(calculated);
        wrap.append(hint);
      }
    }
    const discount = document.createElement("div");
    discount.className = "li-discount";
    const kind = document.createElement("select");
    kind.dataset.lineField = "discountKind";
    kind.disabled = disabled;
    kind.setAttribute("aria-label", `Tipo de descuento de ${line.productName}`);
    for (const [value, label] of [
      ["", "Sin desc."],
      ["percent", "% desc."],
      ["amount", "USD desc."],
    ]) {
      const option = document.createElement("option");
      option.value = value;
      option.textContent = label;
      kind.append(option);
    }
    kind.value = line.discountKind ?? "";
    const value = lineInput(
      line,
      index,
      "discountValue",
      "Descuento",
      disabled || !kind.value,
    );
    value.placeholder = "0";
    kind.addEventListener("change", () => {
      lines[index].discountKind = kind.value;
      if (!kind.value) lines[index].discountValue = "";
      renderLines();
      draft?.schedule();
    });
    discount.append(kind, value);
    wrap.append(discount);
    return wrap;
  }

  function lineInput(line, index, key, label, disabled) {
    const input = document.createElement("input");
    input.type = "text";
    input.inputMode = "decimal";
    input.value = line[key] ?? "";
    input.disabled = disabled;
    input.dataset.lineField = key;
    input.setAttribute("aria-label", `${label} de ${line.productName}`);
    // Enter vuelve al buscador para seguir añadiendo productos.
    input.addEventListener("keydown", (event) => {
      if (event.key !== "Enter") return;
      event.preventDefault();
      productCombobox?.getInput().focus();
    });
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
      unit.append(unitControl(line, index, deleted));
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
      } else if (showsSalePrice) {
        price.append(salePriceControls(line, index, deleted));
      } else {
        price.className = "li-costo";
        price.textContent = "Al guardar";
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
        draft?.schedule();
      });
      actions.append(remove);

      row.append(position, name, quantity, unit, price, subtotal, actions);
      container.append(row);
    });
    updateTotals();
    lines.forEach((line) => void loadUnitOptions(line));
  }

  // ---------- Unidad por línea (spec-04 CV-01) ----------

  /** Pide una vez las unidades del producto y repinta al llegar. */
  async function loadUnitOptions(line) {
    if (line.productId == null || line.unitOptions !== undefined) return;
    line.unitOptions = null;
    try {
      line.unitOptions =
        (await listUnitOptionsForProduct(line.productId)) ?? [];
    } catch {
      line.unitOptions = [];
    }
    const chosen = line.unitOptions.find(
      (option) => option.unitId === line.unitId,
    );
    if (chosen) line.factor = parseFactor(chosen.factor);
    if (lines.includes(line)) renderLines();
  }

  /** Selector si el producto tiene conversiones; si no, su unidad en texto. */
  function unitControl(line, index, disabled) {
    const options = line.unitOptions ?? [];
    if (disabled || options.length < 2) {
      const text = document.createElement("span");
      text.textContent = line.unitAbbreviation ?? "—";
      return text;
    }
    const select = document.createElement("select");
    select.dataset.lineField = "unit";
    select.setAttribute("aria-label", `Unidad de ${line.productName}`);
    for (const option of options) {
      const item = document.createElement("option");
      item.value = String(option.unitId);
      item.textContent = option.abbreviation;
      select.append(item);
    }
    select.value = String(line.unitId ?? options[0].unitId);
    select.addEventListener("change", () => {
      const option = options.find(
        (item) => String(item.unitId) === select.value,
      );
      // La primera es la del producto: elegirla es no convertir.
      lines[index].unitId =
        option && option !== options[0] ? option.unitId : null;
      lines[index].unitAbbreviation = option?.abbreviation ?? null;
      lines[index].factor = parseFactor(option?.factor);
      renderLines();
    });
    return select;
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
    showHeaderTotal(text);
  }

  // El total queda a la vista aunque la factura tenga muchas líneas.
  function showHeaderTotal(text) {
    const head = field("form-title")?.parentElement;
    if (!head) return;
    head.classList.add("form-head");
    let total = head.querySelector(".form-head-total");
    if (!total) {
      total = document.createElement("span");
      total.className = "form-head-total";
      head.append(total);
    }
    total.textContent = `Total: ${text}`;
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
    draft = attachDraft(formEl, {
      formKey: cfg.draftKey,
      recordId: () => editing?.id ?? null,
      serialize: () => ({
        ...serializeForm(formEl),
        contact: selectedContact,
        lines,
      }),
      restore: restoreDraft,
    });
  }

  // BR-02: lo que ya no existe se avisa y se quita.
  async function restoreDraft(values) {
    restoreForm(form(), values);
    const contact = await existingContact(cfg.contact, values.contact);
    contactCombobox?.setSelected(contact);
    selectedContact = contact;
    if (values.contact && !contact) {
      notifyWarning(`El ${contactNoun} del borrador ya no existe y se quitó.`);
    }
    const kept = [];
    for (const line of values.lines ?? []) {
      if (await lineStillValid(line)) kept.push(line);
      else {
        notifyWarning(
          `«${line.productName}» ya no existe y se quitó de la factura.`,
        );
      }
    }
    lines = kept;
    renderLines();
    if (associated) {
      const id = Number(values[associated.selectId]);
      await loadAssociatedOptions(contact, id || null);
    }
  }

  function lineStillValid(line) {
    // Sin id: un producto nuevo de la compra, o uno ya eliminado al editar.
    if (line.productId == null) return true;
    return productExists(line.productId, !isEntry);
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

  async function openForEdit(invoiceId) {
    await openDetail(invoiceId);
    await startEdit();
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
    const reservedUntil = field("reservar-hasta");
    if (reservedUntil) reservedUntil.value = detail.reservedUntil ?? "";
    lines = (detail.lines ?? []).map((line) => {
      const productId = line.productStatus ? line.productIdOriginal : null;
      // CV-01: se edita lo escrito ("2 Caja"); un producto eliminado o una
      // unidad que ya no existe se editan en la unidad del producto.
      const entered =
        productId != null && line.entered?.unitId != null ? line.entered : null;
      const quantity = entered?.quantity ?? line.quantity ?? "";
      const baseMilli = parseDecimal(line.quantity, 3);
      const enteredMilli = entered ? parseDecimal(entered.quantity, 3) : null;
      return {
        productIdOriginal: line.productIdOriginal,
        productId,
        productName: line.productName ?? "",
        unitAbbreviation:
          entered?.abbreviation ?? line.unitAbbreviation ?? null,
        quantity,
        originalQuantity: quantity,
        unitPrice: centsToText(entered?.unitPriceCents ?? line.unitPriceCents),
        // spec-06 PF-01 / S-03: el mínimo es el precio de lista guardado; el
        // precio emitido se conserva (FS-01) y se puede subir.
        priceCents: line.listPriceCents ?? line.unitPriceCents ?? null,
        salePrice: showsSalePrice ? centsToText(line.unitPriceCents) : "",
        originalSalePrice: showsSalePrice
          ? centsToText(line.unitPriceCents)
          : "",
        discountKind: line.discountKind ?? "",
        discountValue:
          line.discountValue == null ? "" : centsToText(line.discountValue),
        productStatus: line.productStatus ?? null,
        unitId: entered?.unitId ?? null,
        factor: enteredMilli ? baseMilli / enteredMilli : 1,
      };
    });
    if (isEntry) setAttachmentLabel(detail.attachmentOriginalName ?? "");
    renderLines();
    showTab("create", createTab());
    if (associated)
      await loadAssociatedOptions(contact, detail.associatedInvoiceId);
    await draft?.offer();
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
    const cheaper = lines.find(priceBelowList);
    if (cheaper) {
      return `El precio de «${cheaper.productName}» no puede ser menor que ${formatMoney(cheaper.priceCents)}.`;
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
    // Sin fecha (o al quitarla) es una venta normal (RS-01).
    const reservedUntil = field("reservar-hasta")?.value;
    if (
      reservable &&
      reservedUntil &&
      (editing || input.status === "pending")
    ) {
      input.reservedUntil = reservedUntil;
    }
    input.lines = lines.map((line) => {
      const item = {};
      if (editing && line.productIdOriginal != null) {
        item.productIdOriginal = line.productIdOriginal;
      }
      // Un producto eliminado no tiene id vigente: nunca se envía NaN.
      if (line.productId != null) item.productId = line.productId;
      if (line.productId != null && line.unitId != null)
        item.unitId = line.unitId;
      // El costo inicial del producto nuevo es el de su línea (FE-01).
      if (line.newProduct) {
        item.newProduct = {
          ...line.newProduct,
          cost: String(line.unitPrice).trim(),
        };
      }
      item.quantity = String(line.quantity).trim();
      if (isEntry) item.unitPrice = String(line.unitPrice).trim();
      if (showsSalePrice) {
        // Solo si cambió: sin precio, el backend conserva el emitido.
        const salePrice = String(line.salePrice ?? "").trim();
        if (salePrice && salePrice !== line.originalSalePrice)
          item.unitPrice = salePrice;
        const discountValue = String(line.discountValue ?? "").trim();
        if (line.discountKind && discountValue) {
          item.discountKind = line.discountKind;
          item.discountValue = discountValue;
        }
      }
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
      notifySuccess(invoiceSavedMessage(result?.number ?? wasEditing?.number));
      notifyWarning(result?.expiredCleanupWarning);
      draft?.clear();
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
      emptyState(body, {
        empty: `Aún no hay facturas de ${cfg.noun}.`,
        filtered: Object.keys(readListFilters(cfg.view, true)).length > 0,
        action: { label: "Crear factura", onClick: () => void startNew() },
      });
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
      const statusText = cfg.statusLabels[item?.status] ?? item?.status ?? "—";
      texts.push(formatMoney(item?.totalCents ?? 0), statusText);
      if (reservable) {
        texts.push(item?.reservedUntil ? formatDate(item.reservedUntil) : "—");
      }
      if (cfg.showsProductCount) texts.push(String(item?.productCount ?? "—"));
      const cells = texts.map((text) => {
        const cell = document.createElement("td");
        if (text === statusText && item?.status) {
          const pill = document.createElement("span");
          pill.className = `status-pill is-${item.status}`;
          pill.textContent = text;
          cell.append(pill);
        } else {
          cell.textContent = text;
        }
        return cell;
      });
      const actionsCell = document.createElement("td");
      for (const [action, label] of [
        ["detail", "Ver"],
        ["delete", "Eliminar"],
      ]) {
        const actionButton = document.createElement("button");
        actionButton.type = "button";
        actionButton.className =
          action === "delete" ? "action-link danger" : "action-link";
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
    const printButton = button("imprimir");
    if (claim(printButton)) {
      printButton.addEventListener("click", () => {
        if (currentId != null) void printInvoice(currentId);
      });
    }
    const exportButton = button("exportar");
    if (claim(exportButton)) {
      exportButton.addEventListener("click", () => {
        if (currentId != null) {
          void exportInvoiceToPdf(currentId, currentDetail?.number);
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
    if (reservable) {
      set(
        "reserva",
        detail?.reservedUntil ? formatDate(detail.reservedUntil) : "—",
      );
      const card = document.getElementById(`${cfg.field}-ver-reserva-card`);
      if (card) card.hidden = !detail?.reservedUntil;
    }
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
        // CV-01: se muestra lo escrito ("2 Caja"), no lo que se movió.
        const entered = line?.entered ?? null;
        for (const text of [
          String(index + 1),
          line?.productName ?? "",
          String(entered?.quantity ?? line?.quantity ?? ""),
          entered?.abbreviation ??
            line?.unitAbbreviation ??
            line?.unitName ??
            "—",
          formatMoney(entered?.unitPriceCents ?? line?.unitPriceCents ?? 0),
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
        title: `Cancelar factura ${currentDetail?.number ?? ""}`,
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
      notifySuccess(
        `Estado de la factura ${result?.number ?? ""} actualizado.`,
      );
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
    const number = lastItems.find(
      (item) => item.id === Number(invoiceId),
    )?.number;
    let message = `¿Deseas eliminar la factura ${number ?? `de ${cfg.noun}`}?`;
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
      notifySuccess(`Factura ${deleted?.number ?? ""} eliminada.`);
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
    startNew,
    startEdit: openForEdit,
    reload: () => loadInvoices(),
    showTab,
    openDetail,
    submit,
    setPendingAttachment: (path) => {
      pendingAttachmentPath = path;
    },
  };
}

// ---------- Imprimir y exportar (spec-04 IM-01, IM-02) ----------

async function printInvoice(invoiceId) {
  try {
    openPrintPreview(await getInvoicePrintModel(invoiceId));
  } catch (error) {
    notifyError(error, "No se pudo preparar la impresión.");
  }
}

async function chooseInvoicePdfDestination(number) {
  try {
    const path = await window.__TAURI__?.dialog?.save?.({
      defaultPath: `${number ?? "factura"}.pdf`,
      filters: [{ name: "Documento PDF", extensions: ["pdf"] }],
    });
    return typeof path === "string" && path ? path : null;
  } catch {
    return null;
  }
}

async function exportInvoiceToPdf(invoiceId, number) {
  const destination = await chooseInvoicePdfDestination(number);
  // Si el usuario cancela el diálogo, no pasa nada.
  if (!destination) return;
  try {
    await exportInvoicePdf(invoiceId, destination);
    notifySuccess("Factura exportada a PDF.");
  } catch (error) {
    notifyError(error, "No se pudo guardar el archivo.");
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

const OUTGOING_CANCEL_MESSAGE =
  "Los productos de esta factura vuelven al inventario. Una factura cancelada no se puede reactivar ni editar.";

const entrySubmodule = createInvoiceSubmodule({
  invoiceType: "E",
  draftKey: "entryInvoice",
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
  windowFlag: "__facturasEntryPageChangeBound",
  create: (input) => createEntryInvoice(input),
  update: (input) => updateEntryInvoice(input),
});

const saleSubmodule = createInvoiceSubmodule({
  invoiceType: "V",
  draftKey: "saleInvoice",
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
  windowFlag: "__facturasSalePageChangeBound",
  create: (input) => createSaleInvoice(input),
  update: (input) => updateSaleInvoice(input),
});

const serviceSubmodule = createInvoiceSubmodule({
  invoiceType: "S",
  draftKey: "serviceInvoice",
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
  windowFlag: "__facturasServicePageChangeBound",
  create: (input) => createServiceInvoice(input),
  update: (input) => updateServiceInvoice(input),
});

// Garantía (FS-04) e insumos (FS-05): factura asociada elegida en un
// <select>, sin método de pago y a costo real.
const guaranteeSubmodule = createInvoiceSubmodule({
  invoiceType: "GG",
  draftKey: "guaranteeInvoice",
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
  windowFlag: "__facturasGuaranteePageChangeBound",
  create: (input) => createGuaranteeInvoice(input),
  update: (input) => updateGuaranteeInvoice(input),
});

const installationSubmodule = createInvoiceSubmodule({
  invoiceType: "GI",
  draftKey: "installationInvoice",
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
  windowFlag: "__facturasInstallationPageChangeBound",
  create: (input) => createInstallationInvoice(input),
  update: (input) => updateInstallationInvoice(input),
});

for (const [formKey, page, submodule] of [
  ["entryInvoice", "facturas-entrada", entrySubmodule],
  ["saleInvoice", "facturas-venta", saleSubmodule],
  ["serviceInvoice", "facturas-servicios", serviceSubmodule],
  ["guaranteeInvoice", "facturas-garantia", guaranteeSubmodule],
  ["installationInvoice", "facturas-insumos", installationSubmodule],
]) {
  registerDraftOpener(formKey, async (invoiceId) => {
    window.showPage?.(
      page,
      document.querySelector(`#sidebar [data-page="${page}"]`),
    );
    await submodule.init();
    if (invoiceId == null) await submodule.startNew();
    else await submodule.startEdit(invoiceId);
  });
}
// La línea nueva se añade desde la compra: su diálogo pregunta por el borrador.
registerDraftOpener("quickProduct", () => {
  window.showPage?.(
    "facturas-entrada",
    document.querySelector('#sidebar [data-page="facturas-entrada"]'),
  );
  return entrySubmodule.startNew();
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
let lossDraft = null;
let lossItems = [];
let generalItems = [];

registerDraftOpener("lossInvoice", async () => {
  window.showPage?.(
    "facturas-perdidas",
    document.querySelector('#sidebar [data-page="facturas-perdidas"]'),
  );
  await initLossInvoicesView();
  showLossTab("create", document.getElementById("tab-factura-perdidas-create"));
});

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
    [
      "factura-perdida-cancelar",
      () => {
        lossDraft?.clear();
        toList();
      },
    ],
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
  if (tab === "create") void lossDraft?.offer();
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
    onSelect: (item) => void loadLossUnits(item),
  });
  // Se conserva el id del contrato de interfaz en el input real.
  const input = lossProductCombobox.getInput();
  input.id = "factura-perdida-producto";
  input.required = true;
}

// CV-01: la pérdida también puede escribirse en otra unidad del producto.
async function loadLossUnits(product) {
  const field = document.getElementById("factura-perdida-unidad-field");
  const select = document.getElementById("factura-perdida-unidad");
  if (!field || !select) return;
  let options = [];
  if (product?.id != null) {
    try {
      options = (await listUnitOptionsForProduct(Number(product.id))) ?? [];
    } catch {
      options = [];
    }
  }
  // Otro producto pudo elegirse mientras llegaban las unidades.
  if (lossProductCombobox?.getSelected()?.id !== product?.id) return;
  select.replaceChildren(
    ...options.map((option) => {
      const item = document.createElement("option");
      item.value = String(option.unitId);
      item.textContent = option.abbreviation;
      return item;
    }),
  );
  field.hidden = options.length < 2;
}

function lossUnitId() {
  const field = document.getElementById("factura-perdida-unidad-field");
  const select = document.getElementById("factura-perdida-unidad");
  // La primera opción es la del producto: elegirla es no convertir.
  if (!field || field.hidden || !select || select.selectedIndex <= 0)
    return null;
  return Number(select.value);
}

function bindLossForm() {
  const form = document.getElementById("factura-perdidas-form");
  if (!claimLoss(form)) return;
  form.addEventListener("submit", (event) => {
    event.preventDefault();
    void handleLossSubmit();
  });
  lossDraft = attachDraft(form, {
    formKey: "lossInvoice",
    serialize: () => ({
      ...serializeForm(form),
      product: lossProductCombobox?.getSelected() ?? null,
    }),
    restore: async (values) => {
      const product = values.product;
      const exists = product && (await productExists(product.id, false));
      if (product && !exists) {
        notifyWarning(
          `«${product.name}» ya no existe y se quitó del borrador.`,
        );
      }
      lossProductCombobox?.setSelected(exists ? product : null);
      await loadLossUnits(exists ? product : null);
      restoreForm(form, values);
    },
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
  const printButton = document.getElementById("factura-perdida-imprimir");
  if (claimLoss(printButton)) {
    printButton.addEventListener("click", () => {
      if (currentLossId != null) void printInvoice(currentLossId);
    });
  }
  const exportButton = document.getElementById("factura-perdida-exportar");
  if (claimLoss(exportButton)) {
    exportButton.addEventListener("click", () => {
      if (currentLossId != null) {
        void exportInvoiceToPdf(currentLossId, currentLossDetail?.number);
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
    emptyState(body, {
      empty: "Aún no hay facturas de pérdidas.",
      filtered:
        Object.keys(readListFilters("factura-perdidas", false)).length > 0,
      action: {
        label: "Registrar pérdida",
        onClick: () =>
          showLossTab(
            "create",
            document.getElementById("tab-factura-perdidas-create"),
          ),
      },
    });
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
      button.className =
        action === "delete" ? "action-link danger" : "action-link";
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
  const unitId = lossUnitId();
  const input = {
    productId: Number(product.id),
    quantity,
    ...(unitId != null ? { unitId } : {}),
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
    lossDraft?.clear();
    notifySuccess(invoiceSavedMessage(result?.number));
    notifyWarning(result?.expiredCleanupWarning);
    lossProductCombobox?.setSelected(null);
    void loadLossUnits(null);
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
  // CV-01: lo escrito ("0,25 cj") y el costo por esa unidad.
  const entered = line?.entered ?? null;
  const unit =
    entered?.abbreviation ?? line?.unitAbbreviation ?? line?.unitName ?? "";
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
    line
      ? formatMoney(entered?.unitPriceCents ?? line.unitPriceCents ?? 0)
      : "—",
  );
  setText(
    "factura-perdida-ver-cantidad",
    line
      ? `${entered?.quantity ?? line.quantity ?? ""}${unit ? ` ${unit}` : ""}`
      : "—",
  );
  setText(
    "factura-perdida-ver-fecha",
    detail?.date ? formatDate(detail.date) : "—",
  );
  setText("factura-perdida-ver-descripcion", detail?.notes ?? "—");
  renderRegistration("factura-perdidas-ver", detail);
}

async function handleDeleteLoss(invoiceId) {
  const number = lossItems.find(
    (item) => item.id === Number(invoiceId),
  )?.number;
  const confirmed = await confirmDialog({
    title: "Eliminar pérdida",
    message: `¿Deseas eliminar la factura ${number ?? "de pérdida"}?`,
    confirmText: "Eliminar",
  });
  if (!confirmed) return;
  try {
    const deleted = await deleteInvoice(Number(invoiceId));
    notifySuccess(`Factura ${deleted?.number ?? ""} eliminada.`);
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
    viewButton.className = "action-link";
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
