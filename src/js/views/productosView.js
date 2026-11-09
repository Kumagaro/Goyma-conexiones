import { emptyState } from "../utils/dom.js";
import {
  attachDraft,
  registerDraftOpener,
  restoreForm,
  serializeForm,
} from "../utils/drafts.js";
import { createCombobox } from "../components/combobox.js";
import { confirmDialog } from "../components/confirmDialog.js";
import {
  createCategory,
  listCategories,
  searchCategories,
} from "../services/categoriesService.js";
import {
  activateProduct,
  createProduct,
  deleteProduct,
  discontinueProduct,
  getProduct,
  listOutOfStockAndDiscontinued,
  listProducts,
  previewSellingPrice,
  updateProduct,
} from "../services/productsService.js";
import { getProductAssociatedInvoices } from "../services/historyService.js";
import {
  getUserPreferences,
  listExchangeRates,
} from "../services/settingsService.js";
import { equivalents } from "../utils/currency.js";
import { searchUnits } from "../services/unitsService.js";
import {
  formatCalendarDate,
  formatId,
  formatLocalTimestamp,
  formatMoney,
  formatRemainingDays,
} from "../utils/formatters.js";
import {
  INVOICE_TYPE_LABELS,
  invoiceStatusLabel,
} from "../utils/invoiceLabels.js";
import {
  clearFieldErrors,
  notifyError,
  notifySuccess,
  notifyWarning,
  reportError,
  requireFields,
  setFieldError,
} from "../utils/feedback.js";
import { openUnitModalForProduct } from "./unidadesView.js";

let productDraft = null;
let initPromise = null;
let editingProductId = null;
let categoryCombobox = null;
let unitCombobox = null;
let previewTimer = null;
let previewRequestId = 0;
let productList = [];
let conditionList = [];
let currentProductDetail = null;
let currentTab = "list";
let productSearchTimer = null;
let conditionSearchTimer = null;

export function initProductosView() {
  if (!initPromise) initPromise = initializeProductosView();
  return initPromise;
}

async function initializeProductosView() {
  const form = document.getElementById("producto-form");
  if (!form) return;

  const categoryIdInput = document.getElementById("producto-categoria");
  const unitIdInput = document.getElementById("producto-unidad");
  const categoryContainer = document.getElementById(
    "producto-categoria-combobox",
  );
  const unitContainer = document.getElementById("producto-unidad-combobox");

  categoryCombobox = createCombobox({
    container: categoryContainer,
    id: "producto-categoria",
    placeholder: "Buscar categoría...",
    search: searchCategoryMatches,
    formatOption: (category) => String(category?.nombre ?? ""),
    addOptionLabel: (text) => `Agregar nueva categoría «${text}»`,
    onAddNew: createCategoryFromProduct,
    onSelect: (category) => {
      if (categoryIdInput) categoryIdInput.value = category?.id ?? "";
    },
    clearLabel: "Sin categoría",
  });

  unitCombobox = createCombobox({
    container: unitContainer,
    id: "producto-unidad",
    placeholder: "Buscar unidad...",
    search: searchUnitMatches,
    formatOption: (unit) =>
      `${unit?.nombre ?? ""} (${unit?.abbreviation ?? ""})`,
    addOptionLabel: (text) => `Agregar nueva unidad «${text}»`,
    onAddNew: requestUnitCreation,
    onSelect: (unit) => {
      if (unitIdInput) unitIdInput.value = unit?.id ?? "";
    },
    clearLabel: "Sin unidad",
  });

  form.addEventListener("submit", handleProductSubmit);
  productDraft = attachDraft(form, {
    formKey: "product",
    recordId: () => editingProductId,
    serialize: () => ({
      ...serializeForm(form),
      category: categoryCombobox.getSelected(),
      unit: unitCombobox.getSelected(),
    }),
    restore: (values) => {
      restoreForm(form, values);
      categoryCombobox.setSelected(values.category ?? null);
      unitCombobox.setSelected(values.unit ?? null);
    },
  });
  document
    .getElementById("producto-cancelar")
    ?.addEventListener("click", cancelProductForm);
  document
    .getElementById("producto-caducidad-tipo")
    ?.addEventListener("change", updateExpiryVisibility);

  for (const inputId of ["producto-precio-entrada", "producto-ganancia"]) {
    const input = document.getElementById(inputId);
    input?.addEventListener("input", scheduleSellingPricePreview);
    input?.addEventListener("blur", () => void updateSellingPricePreview(true));
  }

  const expiresOnInput = document.getElementById("producto-caducidad-fecha");
  if (expiresOnInput) {
    expiresOnInput.min = localDateText(new Date());
    expiresOnInput.max = "2100-12-31";
  }

  document
    .getElementById("productos-body")
    ?.addEventListener("click", handleProductListClick);
  document
    .getElementById("productos-condiciones-body")
    ?.addEventListener("click", handleConditionListClick);
  document
    .getElementById("producto-ver")
    ?.addEventListener("click", handleProductDetailClick);
  document
    .getElementById("producto-ver-volver")
    ?.addEventListener("click", () => void showProductoTab("list"));

  document
    .querySelector("#page-productos .view-tabs")
    ?.addEventListener("click", (event) => {
      const tab = event.target.closest("button[data-product-tab]");
      if (tab) void showProductoTab(tab.dataset.productTab, tab);
    });
  document
    .getElementById("producto-search")
    ?.addEventListener("input", scheduleProductListReload);
  for (const filterId of [
    "producto-filtro-categoria",
    "producto-filtro-stock",
    "producto-filtro-caducidad",
  ]) {
    document
      .getElementById(filterId)
      ?.addEventListener("change", () => void loadActiveProducts());
  }
  document
    .getElementById("producto-condiciones-search")
    ?.addEventListener("input", scheduleConditionListReload);
  document
    .getElementById("producto-condiciones-filter")
    ?.addEventListener("change", () => void loadConditionProducts());

  window.showProductoTab = showProductoTab;
  window.productosView = {
    openProductForm,
    cancelProductForm,
    showProductoTab,
    openProductConditionTab,
    openProductDetail,
    loadActiveProducts,
  };
  openProductForm();
  if (document.getElementById("productos-body")) {
    await loadProductCategoryFilters();
    await loadActiveProducts();
  }
}

export async function showProductoTab(tab, button = null) {
  currentTab = tab;
  document
    .querySelectorAll("#page-productos .config-subview")
    .forEach((view) =>
      view.classList.toggle("active", view.id === `producto-${tab}`),
    );
  document
    .querySelectorAll("#page-productos .view-tabs .tab")
    .forEach((tabButton) => {
      const active =
        tabButton === button || tabButton.dataset.productTab === tab;
      tabButton.classList.toggle("active", active);
      if (tabButton.hasAttribute("role")) {
        tabButton.setAttribute("aria-selected", String(active));
        tabButton.tabIndex = active ? 0 : -1;
      }
    });

  const searchBar = document.getElementById("producto-search-bar");
  if (searchBar) searchBar.hidden = tab !== "list";
  // El buscador de arriba solo filtra el listado; las demás pestañas tienen el suyo.
  const searchField = document
    .getElementById("producto-search")
    ?.closest(".search-bar");
  if (searchField) searchField.hidden = tab !== "list";
  const conditionFilters = document.getElementById(
    "producto-condiciones-toolbar",
  );
  if (conditionFilters) conditionFilters.hidden = tab !== "agotados";

  if (tab === "list") await loadActiveProducts();
  if (tab === "agotados") await loadConditionProducts();
  if (tab === "create" && editingProductId === null) openProductForm();
  if (tab === "create") void productDraft?.offer();
}

registerDraftOpener("product", async (productId) => {
  window.showPage?.(
    "productos",
    document.querySelector('#sidebar [data-page="productos"]'),
  );
  if (productId != null) openProductForm(await getProduct(productId));
  await showProductoTab(
    "create",
    document.getElementById("tab-producto-create"),
  );
});

export async function openProductConditionTab(condition = "all") {
  const productsLink = document.querySelector(
    '#sidebar a[data-page="productos"]',
  );
  window.showPage?.("productos", productsLink);
  const filter = document.getElementById("producto-condiciones-filter");
  if (filter) filter.value = condition;
  await showProductoTab(
    "agotados",
    document.getElementById("tab-producto-agotados"),
  );
}

async function loadProductCategoryFilters() {
  const select = document.getElementById("producto-filtro-categoria");
  if (!select) return;
  try {
    const response = await listCategories();
    const categories = Array.isArray(response)
      ? response
      : (response?.items ?? []);
    const options = [
      new Option("Todas las categorías", ""),
      new Option("Sin categoría", "withoutCategory"),
    ];
    for (const category of categories) {
      const option = new Option(
        String(category.name ?? ""),
        String(category.id),
      );
      options.push(option);
    }
    select.replaceChildren(...options);
  } catch {
    select.replaceChildren(new Option("Todas las categorías", ""));
  }
}

function readProductFilters() {
  const categoryValue =
    document.getElementById("producto-filtro-categoria")?.value ?? "";
  const categoryId =
    categoryValue && categoryValue !== "withoutCategory"
      ? Number(categoryValue)
      : null;
  return {
    search: document.getElementById("producto-search")?.value?.trim() ?? "",
    categoryId: Number.isSafeInteger(categoryId) ? categoryId : null,
    withoutCategory: categoryValue === "withoutCategory",
    stock: document.getElementById("producto-filtro-stock")?.value ?? "all",
    expiry:
      document.getElementById("producto-filtro-caducidad")?.value ?? "all",
  };
}

function scheduleProductListReload() {
  clearTimeout(productSearchTimer);
  productSearchTimer = setTimeout(() => void loadActiveProducts(), 300);
}

// Mensaje de error que puso la propia carga del listado. Al cargar bien solo se
// limpia ese: una carga que termina tarde no debe borrar el aviso de otra
// acción (p. ej. "El producto ya no existe").

async function loadActiveProducts() {
  const body = document.getElementById("productos-body");
  if (!body) return;
  renderLoadingRow(body, 9, "Cargando productos…");
  try {
    const result = await listProducts(readProductFilters());
    productList = Array.isArray(result) ? result : [];
    renderProductList(productList);
  } catch (error) {
    renderLoadingRow(body, 9, "No se pudieron cargar los productos.");
    notifyError(error, "No se pudieron cargar los productos.");
  }
}

function renderProductList(items) {
  const body = document.getElementById("productos-body");
  if (!body) return;
  const totalHeader = document.getElementById("productos-valor-total");
  if (totalHeader) totalHeader.hidden = false;
  body.replaceChildren();
  if (!items.length) {
    const filters = readProductFilters();
    emptyState(body, {
      empty: "Aún no hay productos.",
      filtered: Boolean(
        filters.search ||
        filters.categoryId ||
        filters.withoutCategory ||
        filters.stock !== "all" ||
        filters.expiry !== "all",
      ),
      action: {
        label: "Crear producto",
        onClick: () =>
          showProductoTab(
            "create",
            document.getElementById("tab-producto-create"),
          ),
      },
    });
    return;
  }

  for (const product of items) {
    const row = document.createElement("tr");
    row.dataset.productId = String(product.id);
    appendCell(row, formatId(product.id));
    appendCell(row, product.name);
    appendCell(row, product.categoryName ?? "Sin categoría");
    appendCell(
      row,
      product.unitName
        ? `${product.unitName}${product.unitAbbreviation ? ` (${product.unitAbbreviation})` : ""}`
        : "—",
    );
    const priceCell = appendCell(
      row,
      formatMoney(product.sellingPriceCents),
      "right",
    );
    appendCalculatedPrice(priceCell, product);
    appendCell(row, formatMoney(product.costCents), "right");

    const stockCell = document.createElement("td");
    stockCell.dataset.label = "Disponible en Stock";
    stockCell.className = "product-stock-cell";
    // La cantidad y su badge naranja de reservado van siempre juntos.
    const main = document.createElement("span");
    main.className = "stock-main";
    const quantity = document.createElement("span");
    quantity.textContent = product.quantity;
    main.append(quantity);
    if (product.reservedQuantity && product.reservedQuantity !== "0") {
      const reserved = createBadge(
        `${product.reservedQuantity} reservado`,
        "tone-orange",
      );
      reserved.title = "Cantidad reservada en facturas";
      main.append(reserved);
    }
    stockCell.append(main);
    if (product.lowStockThreshold == null) {
      stockCell.append(
        Object.assign(document.createElement("small"), {
          className: "price-calculated",
          textContent: " Sin umbral",
        }),
      );
    }
    if (product.isOutOfStock)
      stockCell.append(createBadge("Agotado", "tone-red"));
    if (product.isLowStock)
      stockCell.append(createBadge("Stock Bajo", "tone-orange"));
    row.append(stockCell);
    appendCell(row, formatMoney(product.valorTotalCents), "right");

    const actionsCell = document.createElement("td");
    actionsCell.dataset.label = "Acciones";
    const actions = document.createElement("div");
    actions.className = "col-acciones";
    actions.append(
      createActionButton("view", product.id, "Ver"),
      createActionButton("edit", product.id, "Editar"),
      createIconButton("discontinue", product.id, "Descontinuar", "icon-close"),
      createIconButton(
        "delete",
        product.id,
        "Eliminar",
        "icon-trash",
        "danger",
      ),
    );
    actionsCell.append(actions);
    row.append(actionsCell);
    body.append(row);
  }
}

function scheduleConditionListReload() {
  clearTimeout(conditionSearchTimer);
  conditionSearchTimer = setTimeout(() => void loadConditionProducts(), 300);
}

async function loadConditionProducts() {
  const body = document.getElementById("productos-condiciones-body");
  if (!body) return;
  renderLoadingRow(body, 7, "Cargando productos…");
  try {
    const search =
      document.getElementById("producto-condiciones-search")?.value?.trim() ??
      "";
    const condition =
      document.getElementById("producto-condiciones-filter")?.value ?? "all";
    const response = await listOutOfStockAndDiscontinued(search, condition);
    conditionList = Array.isArray(response?.items) ? response.items : [];
    renderConditionList(conditionList);
    notifyWarning(response?.expiredCleanupWarning);
  } catch (error) {
    renderLoadingRow(body, 7, "No se pudieron cargar los productos.");
    notifyError(error, "No se pudieron cargar los productos.");
  }
}

function renderConditionList(items) {
  const body = document.getElementById("productos-condiciones-body");
  if (!body) return;
  body.replaceChildren();
  if (!items.length) {
    renderLoadingRow(body, 7, "No hay productos agotados ni descontinuados.");
    return;
  }

  for (const product of items) {
    const row = document.createElement("tr");
    row.dataset.productId = String(product.id);
    appendCell(row, formatId(product.id));
    appendCell(row, product.name);
    appendCell(row, product.categoryName ?? "Sin categoría");
    appendCell(
      row,
      product.unitName
        ? `${product.unitName}${product.unitAbbreviation ? ` (${product.unitAbbreviation})` : ""}`
        : "—",
    );
    appendCell(row, product.quantity);
    const conditionCell = document.createElement("td");
    const conditionNames = [];
    if (product.isOutOfStock) conditionNames.push("Agotado");
    if (product.isDiscontinued) conditionNames.push("Descontinuado");
    conditionCell.textContent = conditionNames.join(" · ");
    row.append(conditionCell);
    appendCell(
      row,
      product.remainingDays === null
        ? "—"
        : formatRemainingDays(product.remainingDays),
    );

    const actionsCell = document.createElement("td");
    const actions = document.createElement("div");
    actions.className = "col-acciones";
    actions.append(createActionButton("view", product.id, "Ver"));
    if (product.isDiscontinued) {
      actions.append(createActionButton("activate", product.id, "Activar"));
    } else {
      actions.append(
        createActionButton("edit", product.id, "Editar"),
        createActionButton("discontinue", product.id, "Descontinuar"),
      );
    }
    actions.append(
      createActionButton("delete", product.id, "Eliminar", "danger"),
    );
    actionsCell.append(actions);
    row.append(actionsCell);
    body.append(row);
  }
}

function appendCell(row, text, alignment = "") {
  const cell = document.createElement("td");
  cell.textContent = text == null || text === "" ? "—" : String(text);
  if (alignment) cell.style.textAlign = alignment;
  row.append(cell);
  return cell;
}

function createBadge(label, tone) {
  const badge = document.createElement("span");
  badge.className = `kpi-tag ${tone}`;
  badge.textContent = label;
  return badge;
}

function createActionButton(action, productId, label, extraClass = "") {
  const button = document.createElement("button");
  button.type = "button";
  button.className = `action-link ${extraClass}`.trim();
  button.dataset.productAction = action;
  button.dataset.productId = String(productId);
  button.textContent = label;
  return button;
}

// El nombre sigue en el botón para lectores de pantalla y al pasar el ratón.
function createIconButton(action, productId, label, icon, extraClass = "") {
  const button = createActionButton(action, productId, "", extraClass);
  button.classList.add("icon-only");
  button.title = label;
  const svg = document.createElementNS("http://www.w3.org/2000/svg", "svg");
  svg.setAttribute("class", "icon icon-14");
  svg.setAttribute("aria-hidden", "true");
  const use = document.createElementNS("http://www.w3.org/2000/svg", "use");
  use.setAttribute("href", `#${icon}`);
  svg.append(use);
  const text = document.createElement("span");
  text.className = "sr-only";
  text.textContent = label;
  button.append(svg, text);
  return button;
}

function renderLoadingRow(body, columnCount, message) {
  body.replaceChildren();
  const row = document.createElement("tr");
  const cell = document.createElement("td");
  cell.colSpan = columnCount;
  cell.className = "empty-row";
  cell.textContent = message;
  row.append(cell);
  body.append(row);
}

function handleProductListClick(event) {
  const button = event.target.closest("button[data-product-action]");
  if (!button) return;
  const productId = Number(button.dataset.productId);
  if (!Number.isSafeInteger(productId)) return;
  void handleProductAction(button.dataset.productAction, productId);
}

function handleConditionListClick(event) {
  const button = event.target.closest("button[data-product-action]");
  if (!button) return;
  const productId = Number(button.dataset.productId);
  if (!Number.isSafeInteger(productId)) return;
  void handleProductAction(button.dataset.productAction, productId);
}

function handleProductDetailClick(event) {
  const historyButton = event.target.closest("#producto-ver-historial");
  if (historyButton && currentProductDetail) {
    document.dispatchEvent(
      new CustomEvent("product:viewHistory", {
        detail: { productId: currentProductDetail.id },
      }),
    );
    return;
  }

  const button = event.target.closest("button[data-product-detail-action]");
  if (!button || !currentProductDetail) return;
  const action = button.dataset.productDetailAction;
  if (action === "back") {
    void showProductoTab(
      currentProductDetail.isDiscontinued ? "agotados" : "list",
    );
    return;
  }
  void handleProductAction(action, currentProductDetail.id);
}

async function openProductDetail(productId) {
  try {
    const detail = await getProduct(productId);
    currentProductDetail = detail;
    renderProductDetail(detail);
    await showProductoTab("ver");
  } catch (error) {
    await refreshCurrentList();
    notifyError(error, "No se pudo abrir el producto.");
  }
}

// RS-03: cantidad en reservas activas y un enlace a cada factura.
function renderReservations(detail) {
  const invoices = detail.reservedInvoices ?? [];
  const card = document.getElementById("producto-ver-reservado-card");
  if (card) card.hidden = invoices.length === 0;
  setText("producto-ver-reservado", detail.reservedQuantity ?? "0");
  document.getElementById("producto-ver-reservas")?.replaceChildren(
    ...invoices.map((invoice) => {
      const link = document.createElement("button");
      link.type = "button";
      link.className = "action-link";
      link.textContent = `${invoice.number} · ${invoice.quantity} · hasta ${formatCalendarDate(invoice.reservedUntil)}`;
      link.addEventListener("click", () =>
        window.irAFacturaDesdeGeneral?.(invoice.invoiceType, invoice.id),
      );
      return link;
    }),
  );
}

// CF-01: equivalentes en las monedas que eligió el usuario, con la tasa actual.
async function renderEquivalents(detail) {
  const card = document.getElementById("producto-ver-equivalentes-card");
  const list = document.getElementById("producto-ver-equivalentes");
  if (!card || !list) return;
  card.hidden = true;
  let preferences;
  let rates;
  try {
    [preferences, rates] = await Promise.all([
      getUserPreferences(),
      listExchangeRates(),
    ]);
  } catch {
    // Sin preferencias o tasas no hay equivalentes que mostrar.
    return;
  }
  // Otro detalle pudo abrirse mientras llegaban las tasas.
  if (currentProductDetail?.id !== detail.id) return;
  const currencies = preferences?.visibleCurrencies ?? [];
  const rows = [
    ["Costo", detail.costCents],
    ["Precio de venta", detail.sellingPriceCents],
    ["Valor total", detail.valorTotalCents],
  ]
    .map(([label, cents]) => [
      label,
      equivalents(cents, currencies, rates ?? []),
    ])
    .filter(([, values]) => values.length);
  list.replaceChildren(
    ...rows.map(([label, values]) => {
      const row = document.createElement("div");
      row.className = "kpi-sec-value pv-text";
      row.textContent = `${label}: ${values.join(" · ")}`;
      return row;
    }),
  );
  card.hidden = rows.length === 0;
}

function renderProductDetail(detail) {
  const discontinued = Boolean(detail.isDiscontinued);
  setText("producto-ver-id", formatId(detail.id));
  setText("producto-ver-nombre", detail.name ?? "");
  setText(
    "producto-ver-descripcion",
    detail.description ?? "Sin descripción registrada.",
  );
  setText("producto-ver-categoria", detail.categoryName ?? "Sin categoría");
  setText(
    "producto-ver-unidad",
    detail.unitName
      ? `${detail.unitName}${detail.unitAbbreviation ? ` (${detail.unitAbbreviation})` : ""}`
      : "—",
  );
  setText("producto-ver-precio-entrada", formatMoney(detail.costCents));
  setText("producto-ver-precio-venta", formatMoney(detail.sellingPriceCents));
  showCalculatedPrice("producto-ver-precio-calculado", detail);
  setText("producto-ver-valor-total", formatMoney(detail.valorTotalCents));
  setText("producto-ver-ganancia", detail.profitPercent ?? "—");
  setText("producto-ver-stock", detail.quantity ?? "—");
  renderReservations(detail);
  void renderEquivalents(detail);
  setText(
    "producto-ver-caducidad",
    detail.expiresOn ? formatCalendarDate(detail.expiresOn) : "No Aplica",
  );
  setText("producto-ver-estado", discontinued ? "Descontinuado" : "Activo");
  setText(
    "producto-ver-fecha-descontinuacion",
    discontinued ? formatLocalTimestamp(detail.discontinuedAt) : "—",
  );
  setText(
    "producto-ver-dias-restantes",
    discontinued ? formatRemainingDays(detail.remainingDays) : "—",
  );
  const status = document.getElementById("producto-ver-estado");
  if (status) status.hidden = !discontinued;
  const dateWrap = document.getElementById("producto-ver-fecha-wrap");
  if (dateWrap) dateWrap.hidden = !discontinued;
  const daysWrap = document.getElementById("producto-ver-dias-wrap");
  if (daysWrap) daysWrap.hidden = !discontinued;
  const discontinuedTag = document.getElementById("producto-ver-condicion-tag");
  if (discontinuedTag) {
    discontinuedTag.hidden = !discontinued;
    discontinuedTag.textContent = discontinued ? "Descontinuado" : "";
  }

  const description = document.getElementById("producto-ver-desc-section");
  if (description) description.open = Boolean(detail.description);
  const gainCard = document
    .getElementById("producto-ver-ganancia")
    ?.closest(".kpi-hero");
  gainCard?.classList.remove("tone-orange");
  const stockTag = document.getElementById("producto-ver-stock-tag");
  const stockTagText = document.getElementById("producto-ver-stock-tag-txt");
  if (stockTagText) {
    stockTagText.textContent = detail.isOutOfStock
      ? "Agotado"
      : detail.isLowStock
        ? "Stock Bajo"
        : discontinued
          ? "Descontinuado"
          : "Disponible";
  }
  if (stockTag) {
    stockTag.classList.remove("tone-green", "tone-orange", "tone-red");
    stockTag.classList.add(
      detail.isOutOfStock
        ? "tone-red"
        : detail.isLowStock
          ? "tone-orange"
          : "tone-green",
    );
  }

  const actionVisibility = {
    edit: discontinued,
    discontinue: discontinued,
    activate: !discontinued,
  };
  for (const [action, hidden] of Object.entries(actionVisibility)) {
    const button = document.querySelector(
      `[data-product-detail-action="${action}"]`,
    );
    if (button) button.hidden = hidden;
  }
  notifyWarning(detail.expiredCleanupWarning);
  void renderAssociatedInvoices(
    detail.id,
    "producto-ver-facturas",
    "producto-ver-facturas-lista",
  );
}

// Facturas Asociadas (HP-04): número, tipo, fecha, estado y cantidad de cada
// factura con el producto; "Ver" abre su detalle con facturasView.js.
function isCurrentProduct(productId) {
  return (
    currentProductDetail?.id === productId || editingProductId === productId
  );
}

// Se pinta en el detalle y en el formulario de edición (HP-04).
async function renderAssociatedInvoices(productId, sectionId, listId) {
  const section = document.getElementById(sectionId);
  const list = document.getElementById(listId);
  if (!section || !list) return;
  section.hidden = false;
  const message = (text) => {
    const paragraph = document.createElement("p");
    paragraph.className = "pv-desc-text";
    paragraph.textContent = text;
    list.replaceChildren(paragraph);
  };
  message("Cargando facturas…");
  let items;
  try {
    items = await getProductAssociatedInvoices(productId);
  } catch {
    if (isCurrentProduct(productId)) {
      message("No se pudieron cargar las facturas del producto.");
    }
    return;
  }
  // Si el usuario abrió otro producto mientras cargaba, se descarta.
  if (!isCurrentProduct(productId)) return;
  const invoices = Array.isArray(items) ? items : [];
  if (!invoices.length) {
    message("Este producto aún no aparece en ninguna factura.");
    return;
  }
  const table = document.createElement("table");
  table.className = "list-table";
  const headRow = document.createElement("tr");
  for (const label of [
    "No. Factura",
    "Tipo",
    "Fecha",
    "Estado",
    "Cantidad",
    "Acciones",
  ]) {
    const th = document.createElement("th");
    th.textContent = label;
    headRow.append(th);
  }
  const thead = document.createElement("thead");
  thead.append(headRow);
  const tbody = document.createElement("tbody");
  for (const invoice of invoices) {
    const type = invoice?.invoiceType ?? "";
    const row = document.createElement("tr");
    for (const text of [
      invoice?.invoiceNumber ?? "—",
      INVOICE_TYPE_LABELS[type] ?? type,
      formatCalendarDate(invoice?.invoiceDate),
      invoiceStatusLabel(type, invoice?.status),
      invoice?.quantity ?? "—",
    ]) {
      const cell = document.createElement("td");
      cell.textContent = text;
      row.append(cell);
    }
    const actions = document.createElement("td");
    const view = document.createElement("button");
    view.type = "button";
    view.className = "action-link";
    view.textContent = "Ver";
    view.setAttribute(
      "aria-label",
      `Ver factura ${invoice?.invoiceNumber ?? ""}`,
    );
    view.addEventListener("click", () => {
      window.irAFacturaDesdeGeneral?.(type, Number(invoice?.invoiceId));
    });
    actions.append(view);
    row.append(actions);
    tbody.append(row);
  }
  table.append(thead, tbody);
  list.replaceChildren(table);
}

async function handleProductAction(action, productId) {
  const product = findProduct(productId);
  if (action === "view") return openProductDetail(productId);
  if (action === "edit") {
    if (product?.isDiscontinued) return;
    try {
      const detail = await getProduct(productId);
      if (detail.isDiscontinued) {
        notifyError(
          null,
          "Un producto descontinuado no se puede editar. Actívalo primero",
        );
        await refreshCurrentList();
        return;
      }
      openProductForm(detail);
      await showProductoTab(
        "create",
        document.getElementById("tab-producto-create"),
      );
    } catch (error) {
      notifyError(error, "No se pudo abrir el producto.");
      await refreshCurrentList();
    }
    return;
  }

  if (!["discontinue", "activate", "delete"].includes(action)) return;
  const item = product ?? currentProductDetail;
  if (!item) return;
  if (action === "discontinue" || action === "delete") {
    const stockWarning = item.quantity && item.quantity !== "0";
    const message =
      action === "discontinue"
        ? stockWarning
          ? `Al descontinuar "${item.name}" se perderá el stock disponible (${item.quantity}). ¿Deseas continuar?`
          : `¿Descontinuar "${item.name}"?`
        : stockWarning
          ? `Al eliminar "${item.name}" se perderá definitivamente el stock disponible (${item.quantity}). ¿Deseas continuar?`
          : `¿Eliminar "${item.name}"? Esta acción no se puede deshacer.`;
    const confirmed = await confirmDialog({
      title:
        action === "delete" ? "Eliminar producto" : "Descontinuar producto",
      message,
      confirmText: action === "delete" ? "Eliminar" : "Descontinuar",
    });
    if (!confirmed) return;
  }

  try {
    let result;
    if (action === "discontinue") result = await discontinueProduct(productId);
    if (action === "activate") result = await activateProduct(productId);
    if (action === "delete") result = await deleteProduct(productId);
    if (action === "delete" && currentTab === "ver") {
      currentProductDetail = null;
      await showProductoTab(
        "list",
        document.getElementById("tab-producto-list"),
      );
    } else if (currentTab === "ver") {
      await openProductDetail(productId);
    } else {
      await refreshCurrentList();
    }
    notifySuccess(ACTION_SUCCESS[action](item.name));
    notifyWarning(result?.expiredCleanupWarning);
  } catch (error) {
    if (currentTab === "ver") await openProductDetail(productId);
    else await refreshCurrentList();
    notifyError(error, "No se pudo completar la acción.");
  }
}

const ACTION_SUCCESS = {
  discontinue: (name) => `Producto «${name}» descontinuado.`,
  activate: (name) => `Producto «${name}» activado.`,
  delete: (name) => `Producto «${name}» eliminado.`,
};

function findProduct(productId) {
  return (
    productList.find((product) => product.id === productId) ??
    conditionList.find((product) => product.id === productId) ??
    currentProductDetail
  );
}

async function refreshCurrentList() {
  if (currentTab === "agotados") return loadConditionProducts();
  return loadActiveProducts();
}

function setText(id, value) {
  const element = document.getElementById(id);
  if (element)
    element.textContent = value == null || value === "" ? "—" : String(value);
}

async function createCategoryFromProduct(name) {
  try {
    const category = await createCategory(name, "");
    return { id: category.id, nombre: category.name };
  } catch (error) {
    notifyError(error, "No se pudo crear la categoría.");
    return null;
  }
}

async function searchCategoryMatches(text) {
  const matches = await searchCategories(text);
  return Array.isArray(matches)
    ? matches.map((category) => ({
        id: category.id,
        nombre: category.name,
        isExactMatch: category.isExactMatch,
      }))
    : [];
}

async function searchUnitMatches(text) {
  const matches = await searchUnits(text);
  return Array.isArray(matches)
    ? matches.map((unit) => ({
        id: unit.id,
        nombre: unit.name,
        abbreviation: unit.abbreviation,
        isExactMatch: unit.isExactMatch,
      }))
    : [];
}

function requestUnitCreation(name) {
  return new Promise((resolve) => {
    openUnitModalForProduct(
      name,
      (unit) =>
        resolve({
          id: unit.id,
          nombre: unit.name,
          abbreviation: unit.abbreviation,
        }),
      () => resolve(null),
    );
  });
}

export function openProductForm(product = null) {
  const form = document.getElementById("producto-form");
  if (!form || !categoryCombobox || !unitCombobox) return;

  editingProductId = product?.id ?? null;
  const editing = editingProductId !== null;
  form.dataset.productId = editing ? String(editingProductId) : "";

  setValue(
    "producto-form-title",
    editing ? `Editar producto: ${product.name ?? ""}` : "Crear Nuevo Producto",
    "text",
  );
  setValue("producto-nombre", product?.name ?? "");
  setValue("producto-id-sistema", editing ? formatId(product.id) : "");
  setValue("producto-descripcion", product?.description ?? "");
  setValue(
    "producto-precio-entrada",
    editing ? formatMoney(product.costCents) : "",
  );
  setValue(
    "producto-precio-venta",
    editing ? formatMoney(product.sellingPriceCents) : "",
  );
  setValue("producto-ganancia", product?.profitPercent ?? "");
  setValue("producto-stock", product?.quantity ?? "");
  setValue("producto-umbral", product?.lowStockThreshold ?? "");
  showCalculatedPrice("producto-precio-calculado", editing ? product : null);
  // HP-04: al editar, el formulario muestra también sus facturas asociadas.
  const invoicesSection = document.getElementById(
    "producto-facturas-asociadas",
  );
  if (invoicesSection) invoicesSection.hidden = !editing;
  if (editing) {
    void renderAssociatedInvoices(
      editingProductId,
      "producto-facturas-asociadas",
      "producto-facturas-asociadas-lista",
    );
  }

  const costInput = document.getElementById("producto-precio-entrada");
  const quantityInput = document.getElementById("producto-stock");
  if (costInput) {
    costInput.readOnly = editing;
    costInput.required = !editing;
  }
  if (quantityInput) {
    quantityInput.readOnly = editing;
    quantityInput.required = !editing;
  }
  const profitInput = document.getElementById("producto-ganancia");
  if (profitInput) {
    profitInput.readOnly = false;
    profitInput.required = true;
  }

  categoryCombobox.setEditing(editing);
  categoryCombobox.setSelected(
    product?.categoryId == null
      ? null
      : { id: product.categoryId, nombre: product.categoryName ?? "" },
  );
  unitCombobox.setEditing(editing);
  unitCombobox.setSelected(
    product?.unitId == null
      ? null
      : {
          id: product.unitId,
          nombre: product.unitName ?? "",
          abbreviation: product.unitAbbreviation ?? "",
        },
  );

  const expiryType = document.getElementById("producto-caducidad-tipo");
  const expiresOn = product?.expiresOn ?? "";
  if (expiryType) expiryType.value = expiresOn ? "fecha" : "no-aplica";
  setValue("producto-caducidad-fecha", expiresOn);
  updateExpiryVisibility();
  clearTimeout(previewTimer);
  previewRequestId += 1;
  setPricePreviewFeedback("");
}

export function cancelProductForm() {
  productDraft?.clear();
  const form = document.getElementById("producto-form");
  if (form) form.reset();
  editingProductId = null;
  if (form) form.dataset.productId = "";
  categoryCombobox?.setEditing(false);
  categoryCombobox?.setSelected(null);
  unitCombobox?.setEditing(false);
  unitCombobox?.setSelected(null);
  updateExpiryVisibility();
  clearSellingPricePreview();
  window.showProductoTab?.(
    "list",
    document.getElementById("tab-producto-list"),
  );
}

async function handleProductSubmit(event) {
  event.preventDefault();
  const form = event.currentTarget;

  clearFieldErrors(form);
  const unselected = [categoryCombobox, unitCombobox].filter((combobox) =>
    combobox.hasUnselectedValue(),
  );
  if (unselected.length) {
    for (const combobox of unselected) {
      setFieldError(
        combobox.getInput(),
        "Elige una opción de la lista o vacía el campo",
      );
    }
    notifyError(null, "Elige una opción de la lista o vacía el campo");
    return;
  }

  const name = getValue("producto-nombre").trim();
  const cost = getValue("producto-precio-entrada");
  const quantity = getValue("producto-stock");
  const profitPercent = getValue("producto-ganancia");
  const required = ["producto-nombre", "producto-ganancia", "producto-umbral"];
  if (editingProductId === null) {
    required.push("producto-precio-entrada", "producto-stock");
  }
  if (!requireFields(required)) {
    notifyError(null, "Completa los campos obligatorios.");
    return;
  }

  const quote = await updateSellingPricePreview(true);
  if (!quote) return;

  const input = {
    name,
    description: getValue("producto-descripcion").trim() || null,
    categoryId: categoryCombobox.getSelected()?.id ?? null,
    unitId: unitCombobox.getSelected()?.id ?? null,
    profitPercent,
    lowStockThreshold: getValue("producto-umbral"),
    expiresOn:
      getValue("producto-caducidad-tipo") === "fecha"
        ? getValue("producto-caducidad-fecha") || null
        : null,
  };

  try {
    if (editingProductId === null) {
      await createProduct({ ...input, cost, quantity });
      productDraft?.clear();
      openProductForm();
      notifySuccess(`Producto «${name}» creado.`);
    } else {
      const result = await updateProduct({
        productId: editingProductId,
        ...input,
      });
      productDraft?.clear();
      openProductForm();
      notifySuccess(`Producto «${name}» actualizado.`);
      notifyWarning(result?.expiredCleanupWarning);
    }
  } catch (error) {
    reportError(error, "No se pudo guardar el producto.", PRODUCT_FIELDS);
  }
}

// Campo al que se refiere cada rechazo de validación del backend (TO-01).
const PRODUCT_FIELDS = [
  [/umbral/i, "producto-umbral"],
  [/nombre/i, "producto-nombre"],
  [/descripción/i, "producto-descripcion"],
  [/ganancia|porcentaje/i, "producto-ganancia"],
  [/costo/i, "producto-precio-entrada"],
  [/cantidad|stock/i, "producto-stock"],
  [/caducidad/i, "producto-caducidad-fecha"],
];

function scheduleSellingPricePreview() {
  clearTimeout(previewTimer);
  clearSellingPricePreview();
  previewTimer = setTimeout(() => void updateSellingPricePreview(false), 300);
}

async function updateSellingPricePreview(showError) {
  clearTimeout(previewTimer);
  const cost = getValue("producto-precio-entrada");
  const profitPercent = getValue("producto-ganancia");
  if (!profitPercent.trim() || (editingProductId === null && !cost.trim())) {
    clearSellingPricePreview();
    return null;
  }

  const requestId = ++previewRequestId;
  const input =
    editingProductId === null
      ? { cost, profitPercent }
      : { productId: editingProductId, profitPercent };
  try {
    const quote = await previewSellingPrice(input);
    if (requestId !== previewRequestId) return null;
    const price = document.getElementById("producto-precio-venta");
    if (price) price.value = formatMoney(quote?.sellingPriceCents);
    showCalculatedPrice("producto-precio-calculado", quote);
    setPricePreviewFeedback("");
    return quote;
  } catch (error) {
    if (requestId !== previewRequestId) return null;
    clearSellingPricePreview();
    if (showError) {
      setPricePreviewFeedback(
        error?.message || "No se pudo calcular el precio de venta.",
      );
    }
    return null;
  }
}

function clearSellingPricePreview() {
  previewRequestId += 1;
  const price = document.getElementById("producto-precio-venta");
  if (price) price.value = "";
  showCalculatedPrice("producto-precio-calculado", null);
  setPricePreviewFeedback("");
}

// spec-06 PV-02: el precio sin redondear, más claro, solo si difiere.
function calculatedText(prices) {
  const calculated = prices?.calculatedPriceCents;
  if (calculated == null || calculated === prices.sellingPriceCents) return "";
  return formatMoney(calculated);
}

function showCalculatedPrice(id, prices) {
  const element = document.getElementById(id);
  if (!element) return;
  element.title = "Precio calculado, antes de redondear";
  element.textContent = calculatedText(prices);
  element.hidden = !element.textContent;
}

function appendCalculatedPrice(cell, prices) {
  const text = calculatedText(prices);
  if (!cell || !text) return;
  cell.append(
    document.createElement("br"),
    Object.assign(document.createElement("small"), {
      className: "price-calculated",
      title: "Precio calculado, antes de redondear",
      textContent: text,
    }),
  );
}

function updateExpiryVisibility() {
  const wrapper = document.getElementById("producto-caducidad-fecha-wrap");
  const hasExpiry = getValue("producto-caducidad-tipo") === "fecha";
  if (wrapper) {
    wrapper.hidden = !hasExpiry;
    wrapper.classList.toggle("field-oculto", !hasExpiry);
  }
}

function setPricePreviewFeedback(message) {
  const feedback = document.getElementById("producto-precio-venta-feedback");
  if (!feedback) return;
  feedback.textContent = message;
  feedback.hidden = !message;
}

function setValue(id, value, property = "value") {
  const element = document.getElementById(id);
  if (element) element[property] = value;
}

function getValue(id) {
  return document.getElementById(id)?.value ?? "";
}

function localDateText(date) {
  const year = date.getFullYear();
  const month = String(date.getMonth() + 1).padStart(2, "0");
  const day = String(date.getDate()).padStart(2, "0");
  return `${year}-${month}-${day}`;
}

if (document.readyState === "loading") {
  document.addEventListener(
    "DOMContentLoaded",
    () => void initProductosView(),
    {
      once: true,
    },
  );
} else {
  void initProductosView();
}
