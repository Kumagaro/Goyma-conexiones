import { attachDraft, registerDraftOpener } from "../utils/drafts.js";
import { confirmDialog } from "../components/confirmDialog.js";
import { renderInvoiceHistory } from "../components/invoiceHistory.js";
import {
  clearFieldErrors,
  notifyError,
  notifySuccess,
  notifyWarning,
  reportError,
  requireFields,
} from "../utils/feedback.js";
import {
  createSupplier,
  deleteSupplier,
  getSupplierDetail,
  listSuppliers,
  searchSuppliers,
  updateSupplier,
} from "../services/suppliersService.js";
import { formatDate, formatId, formatMoney } from "../utils/formatters.js";
import { ENTRY_STATUS_LABELS } from "../utils/invoiceLabels.js";
import { el, messageRow, setOptions } from "../utils/dom.js";
import { debounce } from "../utils/requestGuard.js";

let supplierDraft = null;
let suppliers = [];
let editingSupplierId = null;
let currentSupplierId = null;
let historyItems = [];
let historyRequest = 0;

export function initProveedoresView() {
  return initializeProveedoresView();
}

async function initializeProveedoresView() {
  bindPageChangeListener();
  // Cada page:change vuelve a inicializar la vista: los listeners se enlazan
  // una sola vez para que un envío no cree el proveedor dos veces.
  const page = document.getElementById("page-proveedores");
  if (page?.dataset.proveedoresBound !== "1") {
    if (page) page.dataset.proveedoresBound = "1";
    bindTabButtons();
    bindSearch();
    bindForm();
    bindTableActions();
    bindDetailActions();
    bindHistoryControls();
  }
  await loadSuppliers();
  showProveedorTab("list", document.getElementById("tab-proveedor-list"));
}

function bindPageChangeListener() {
  if (window.__proveedoresPageChangeBound) return;

  window.addEventListener("page:change", (event) => {
    if (event.detail?.pageId !== "proveedores") return;
    void initProveedoresView();
  });

  window.__proveedoresPageChangeBound = true;
}

function bindTabButtons() {
  document
    .getElementById("tab-proveedor-list")
    ?.addEventListener("click", () => {
      showProveedorTab("list", document.getElementById("tab-proveedor-list"));
    });

  document
    .getElementById("tab-proveedor-create")
    ?.addEventListener("click", () => {
      showProveedorTab(
        "create",
        document.getElementById("tab-proveedor-create"),
      );
    });

  document
    .getElementById("tab-proveedor-historial")
    ?.addEventListener("click", () => {
      showProveedorTab(
        "historial",
        document.getElementById("tab-proveedor-historial"),
      );
    });
}

function bindSearch() {
  const searchInput = document.getElementById("proveedor-search");
  if (!searchInput) return;

  const debouncedSearch = debounce((query) => {
    void loadSuppliers(query);
  }, 300);

  searchInput.addEventListener("input", (event) => {
    const value = String(event.target.value ?? "").trim();
    debouncedSearch(value);
  });
}

function bindForm() {
  const form = document.getElementById("proveedor-form");
  if (!form) return;

  form.addEventListener("submit", (event) => {
    event.preventDefault();
    void handleSupplierSubmit();
  });
  supplierDraft = attachDraft(form, {
    formKey: "supplier",
    recordId: () => editingSupplierId,
  });
  document
    .getElementById("proveedor-cancelar")
    ?.addEventListener("click", () => {
      supplierDraft?.clear();
      resetSupplierForm();
      showProveedorTab("list", document.getElementById("tab-proveedor-list"));
    });
}

function bindTableActions() {
  const body = document.getElementById("proveedores-body");
  if (!body) return;

  body.addEventListener("click", (event) => {
    const action = event.target.closest("[data-supplier-action]");
    if (!action) return;

    const supplierId = Number(action.dataset.supplierId);
    if (!Number.isFinite(supplierId)) return;

    const actionName = action.dataset.supplierAction;
    if (actionName === "detail") {
      void openSupplierDetail(supplierId);
      return;
    }

    if (actionName === "edit") {
      openSupplierForm(supplierId);
      return;
    }

    if (actionName === "delete") {
      void handleDeleteSupplier(supplierId);
    }
  });
}

function bindDetailActions() {
  const editButton = document.getElementById("proveedor-ver-editar");
  if (editButton) {
    editButton.addEventListener("click", () => {
      if (currentSupplierId != null) openSupplierForm(currentSupplierId);
    });
  }

  const deleteButton = document.getElementById("proveedor-ver-eliminar");
  if (deleteButton) {
    deleteButton.addEventListener("click", () => {
      if (currentSupplierId != null)
        void handleDeleteSupplier(currentSupplierId);
    });
  }
}

function bindHistoryControls() {
  for (const id of [
    "historial-proveedor-select",
    "historial-proveedor-desde",
    "historial-proveedor-hasta",
  ]) {
    document.getElementById(id)?.addEventListener("change", () => {
      if (id === "historial-proveedor-select") void loadSupplierHistory();
      else renderSupplierHistoryRows();
    });
  }
  // Sustituye al onclick inline del encabezado "Fecha"; el orden es local y
  // toggleOrdenFecha (dashboard.js) solo actualiza el icono.
  const dateHeader = document.getElementById("th-fecha-historial-proveedor");
  if (dateHeader && dateHeader.dataset.sortBound !== "1") {
    dateHeader.dataset.sortBound = "1";
    dateHeader.addEventListener("click", () => {
      // El orden vive en el encabezado: por defecto, más reciente primero.
      dateHeader.dataset.sortOrder =
        dateHeader.dataset.sortOrder === "asc" ? "desc" : "asc";
      if (typeof window.toggleOrdenFecha === "function") {
        window.toggleOrdenFecha(
          "historialProveedor",
          renderSupplierHistoryRows,
        );
      } else {
        renderSupplierHistoryRows();
      }
    });
  }
  document
    .getElementById("historial-proveedor-body")
    ?.addEventListener("click", (event) => {
      const button = event.target.closest("[data-history-invoice]");
      if (!button) return;
      window.irAFacturaDesdeGeneral?.(
        "E",
        Number(button.dataset.historyInvoice),
      );
    });
}

function showProveedorTab(tab, button = null) {
  const tabs = document.querySelectorAll("#page-proveedores .view-tabs .tab");
  tabs.forEach((tabButton) => {
    const isActive =
      tabButton === button || tabButton.id === `tab-proveedor-${tab}`;
    tabButton.classList.toggle("active", isActive);
  });

  const subviews = document.querySelectorAll(
    "#page-proveedores .config-subview",
  );
  subviews.forEach((view) => {
    const isActive =
      view.id === `proveedor-${tab}` ||
      (tab === "ver" && view.id === "proveedor-ver");
    view.classList.toggle("active", isActive);
  });

  const searchBar = document.getElementById("proveedor-search-bar");
  if (searchBar) {
    searchBar.style.display = tab === "list" ? "" : "none";
  }

  if (tab === "historial") {
    renderProveedorHistory();
  }

  if (tab === "create" && editingSupplierId === null) {
    resetSupplierForm();
  }
  if (tab === "create") void supplierDraft?.offer();
}

async function loadSuppliers(query = "") {
  const body = document.getElementById("proveedores-body");
  if (!body) return;

  messageRow(body, "Cargando proveedores…", 5);

  try {
    const response = query
      ? await searchSuppliers(query)
      : await listSuppliers();
    const items = Array.isArray(response) ? response : (response?.items ?? []);
    suppliers = items;
    renderSupplierList(items);
    renderProveedorHistory();
  } catch (error) {
    messageRow(body, "No se pudieron cargar los proveedores.", 5);
    notifyError(error, "No se pudieron cargar los proveedores.");
  }
}

function renderSupplierList(items) {
  const body = document.getElementById("proveedores-body");
  if (!body) return;

  if (!items.length) {
    messageRow(body, "Aún no hay proveedores.", 5);
    return;
  }

  body.replaceChildren(
    ...items.map((supplier) => {
      const id = Number(supplier.id);
      return el(
        "tr",
        { dataset: { supplierId: id } },
        el("td", {}, formatId(supplier.id)),
        el("td", {}, supplier.fullName ?? ""),
        el("td", {}, supplier.company ?? "—"),
        el("td", {}, supplier.phone ?? "—"),
        el(
          "td",
          {},
          ROW_ACTIONS.map(([action, label, extra]) =>
            el(
              "button",
              {
                type: "button",
                class: `action-link${extra}`,
                dataset: { supplierAction: action, supplierId: id },
              },
              label,
            ),
          ),
        ),
      );
    }),
  );
}

const ROW_ACTIONS = [
  ["detail", "Ver", ""],
  ["edit", "Editar", ""],
  ["delete", "Eliminar", " danger"],
];

// Historial de compras (PV-07): las facturas de entrada del proveedor salen
// de getSupplierDetail; el rango de fechas y el orden se aplican aquí.
function renderProveedorHistory() {
  const select = document.getElementById("historial-proveedor-select");
  const body = document.getElementById("historial-proveedor-body");

  if (!select || !body) return;

  if (!suppliers.length) {
    setOptions(select, [["", "No hay proveedores"]]);
    historyItems = [];
    historyMessage(body, "No hay proveedores para consultar.");
    return;
  }

  const currentValue = select.value;
  setOptions(
    select,
    suppliers.map((supplier) => [
      Number(supplier.id),
      supplier.fullName || "Proveedor",
    ]),
  );

  if (
    currentValue &&
    [...select.options].some((option) => option.value === currentValue)
  ) {
    select.value = currentValue;
  } else {
    select.selectedIndex = 0;
  }

  // Solo se consulta el backend con la pestaña de historial abierta.
  if (
    document.getElementById("proveedor-historial")?.classList.contains("active")
  ) {
    void loadSupplierHistory();
  }
}

function historyMessage(body, text) {
  messageRow(body, text, 7);
}

async function loadSupplierHistory() {
  const body = document.getElementById("historial-proveedor-body");
  const supplierId = Number(
    document.getElementById("historial-proveedor-select")?.value,
  );
  if (!body || !Number.isSafeInteger(supplierId) || supplierId <= 0) return;
  const request = ++historyRequest;
  historyMessage(body, "Cargando compras…");
  try {
    const detail = await getSupplierDetail(supplierId);
    // Si el usuario eligió otro proveedor mientras cargaba, se descarta.
    if (request !== historyRequest) return;
    historyItems = (detail?.invoiceHistory ?? []).filter(
      (item) => (item?.invoiceType ?? "E") === "E",
    );
    renderSupplierHistoryRows();
  } catch {
    if (request !== historyRequest) return;
    historyItems = [];
    historyMessage(body, "No se pudo cargar el historial de compras.");
  }
}

function renderSupplierHistoryRows() {
  const body = document.getElementById("historial-proveedor-body");
  if (!body) return;
  const from = document.getElementById("historial-proveedor-desde")?.value;
  const to = document.getElementById("historial-proveedor-hasta")?.value;
  if (from && to && from > to) {
    historyMessage(body, "La fecha inicial no puede ser posterior a la final.");
    return;
  }
  const ascending =
    document.getElementById("th-fecha-historial-proveedor")?.dataset
      .sortOrder === "asc";
  const items = historyItems
    .filter((item) => (!from || item.date >= from) && (!to || item.date <= to))
    .sort((left, right) => {
      const order =
        String(left.date).localeCompare(String(right.date)) ||
        Number(left.id) - Number(right.id);
      return ascending ? order : -order;
    });
  if (!items.length) {
    historyMessage(
      body,
      historyItems.length
        ? "No hay compras en el período seleccionado."
        : "Este proveedor aún no tiene compras registradas.",
    );
    return;
  }
  body.replaceChildren();
  for (const item of items) {
    const row = document.createElement("tr");
    // Columnas de PV-07: fecha, estado, cantidad de productos, total,
    // factura y responsable.
    const cells = [
      formatDate(item.date),
      ENTRY_STATUS_LABELS[item.status] ?? item.status ?? "—",
      String(item.productCount ?? "—"),
      formatMoney(item.totalCents ?? 0),
      item.number ?? "—",
      item.createdByName ?? "—",
    ].map((text) => {
      const cell = document.createElement("td");
      cell.textContent = text;
      return cell;
    });
    cells[2].style.textAlign = "center";
    cells[3].style.textAlign = "right";
    const actions = document.createElement("td");
    const view = document.createElement("button");
    view.type = "button";
    view.className = "action-link";
    view.textContent = "Ver";
    view.dataset.historyInvoice = String(item.id);
    view.setAttribute("aria-label", `Ver factura ${item.number ?? ""}`);
    actions.append(view);
    row.append(...cells, actions);
    body.append(row);
  }
}

async function handleSupplierSubmit() {
  const form = document.getElementById("proveedor-form");
  if (!form) return;

  const payload = {
    firstName: document.getElementById("proveedor-nombres")?.value.trim() ?? "",
    lastName:
      document.getElementById("proveedor-apellidos")?.value.trim() ?? "",
    nationalId: document.getElementById("proveedor-ci")?.value.trim() ?? "",
    phone: document.getElementById("proveedor-telefono")?.value.trim() ?? "",
    email: document.getElementById("proveedor-correo")?.value.trim() ?? "",
    company: document.getElementById("proveedor-empresa")?.value.trim() ?? "",
    position: document.getElementById("proveedor-cargo")?.value.trim() ?? "",
    address: document.getElementById("proveedor-direccion")?.value.trim() ?? "",
  };

  clearFieldErrors(form);
  if (
    !requireFields([
      "proveedor-nombres",
      "proveedor-apellidos",
      "proveedor-ci",
      "proveedor-telefono",
    ])
  ) {
    notifyError(null, "Completa los campos obligatorios del proveedor.");
    return;
  }

  try {
    if (editingSupplierId !== null) {
      await updateSupplier(editingSupplierId, payload);
      notifySuccess(
        `Proveedor «${payload.firstName} ${payload.lastName}» actualizado.`,
      );
    } else {
      await createSupplier(payload);
      notifySuccess(
        `Proveedor «${payload.firstName} ${payload.lastName}» creado.`,
      );
    }

    supplierDraft?.clear();
    resetSupplierForm();
    await loadSuppliers();
    showProveedorTab("list", document.getElementById("tab-proveedor-list"));
  } catch (error) {
    reportError(error, "No se pudo guardar el proveedor.", SUPPLIER_FIELDS);
  }
}

// Campo al que se refiere cada rechazo de validación del backend (TO-01).
const SUPPLIER_FIELDS = [
  [/apellidos/i, "proveedor-apellidos"],
  [/nombre/i, "proveedor-nombres"],
  [/carnet/i, "proveedor-ci"],
  [/teléfono/i, "proveedor-telefono"],
  [/correo/i, "proveedor-correo"],
  [/empresa/i, "proveedor-empresa"],
  [/cargo/i, "proveedor-cargo"],
  [/dirección/i, "proveedor-direccion"],
];

function resetSupplierForm() {
  editingSupplierId = null;
  const form = document.getElementById("proveedor-form");
  if (form) form.reset();
  document
    .getElementById("proveedor-form-title")
    ?.replaceChildren(document.createTextNode("Datos de Proveedor"));
}

function openSupplierForm(supplierId) {
  const supplier = suppliers.find(
    (item) => Number(item.id) === Number(supplierId),
  );
  if (!supplier) return;

  editingSupplierId = Number(supplierId);
  const formTitle = document.getElementById("proveedor-form-title");
  if (formTitle)
    formTitle.textContent = `Editar proveedor: ${supplier.fullName ?? ""}`;

  document.getElementById("proveedor-nombres").value =
    supplier.firstName ?? supplier.fullName?.split(" ")[0] ?? "";
  document.getElementById("proveedor-apellidos").value =
    supplier.lastName ??
    supplier.fullName?.replace(supplier.firstName ?? "", "").trim() ??
    "";
  document.getElementById("proveedor-ci").value = supplier.nationalId ?? "";
  document.getElementById("proveedor-telefono").value = supplier.phone ?? "";
  document.getElementById("proveedor-correo").value = supplier.email ?? "";
  document.getElementById("proveedor-empresa").value = supplier.company ?? "";
  document.getElementById("proveedor-cargo").value = supplier.position ?? "";
  document.getElementById("proveedor-direccion").value = supplier.address ?? "";

  showProveedorTab("create", document.getElementById("tab-proveedor-create"));
}

async function openSupplierDetail(supplierId) {
  try {
    const detail = await getSupplierDetail(supplierId);
    currentSupplierId = Number(supplierId);
    renderSupplierDetail(detail);
    showProveedorTab("ver", null);
  } catch (error) {
    notifyError(error, "No se pudo abrir el detalle del proveedor.");
  }
}

function renderSupplierDetail(detail) {
  const fullName =
    [detail?.firstName, detail?.lastName].filter(Boolean).join(" ") ||
    detail?.fullName ||
    "Proveedor";
  const indicators = detail?.indicators ?? {};

  document.getElementById("proveedor-ver-id").textContent = formatId(
    detail?.id ?? "",
  );
  document.getElementById("proveedor-ver-nombre-completo").textContent =
    fullName;
  document.getElementById("proveedor-ver-empresa").textContent =
    detail?.company || "Sin empresa";
  document.getElementById("proveedor-ver-cargo").textContent =
    detail?.position || "Sin cargo";
  document.getElementById("proveedor-ver-telefono").textContent =
    detail?.phone || "—";
  document.getElementById("proveedor-ver-correo").textContent =
    detail?.email || "—";
  document.getElementById("proveedor-ver-ci").textContent =
    detail?.nationalId || "—";
  document.getElementById("proveedor-ver-direccion").textContent =
    detail?.address || "—";
  // En proveedores el backend llena totalPurchasedCents (PV-03).
  document.getElementById("proveedor-ver-total-comprado").textContent =
    formatMoney(indicators.totalPurchasedCents ?? 0).replace(" USD", "");
  document.getElementById("proveedor-ver-saldo-pendiente").textContent =
    formatMoney(indicators.pendingBalanceCents ?? 0).replace(" USD", "");
  document.getElementById("proveedor-ver-num-facturas").textContent = String(
    indicators.invoiceCount ?? 0,
  );
  document.getElementById("proveedor-ver-ultima-compra").textContent =
    indicators.lastPurchaseDate ? formatDate(indicators.lastPurchaseDate) : "—";

  renderInvoiceHistory(
    document.getElementById("proveedor-ver-historial-facturas"),
    detail?.invoiceHistory,
    "Este proveedor aún no tiene compras registradas.",
  );

  const companyTag = document
    .getElementById("proveedor-ver-empresa")
    .closest(".kpi-tag");
  if (companyTag)
    companyTag.style.display = detail?.company ? "inline-flex" : "none";

  const cargoTag = document
    .getElementById("proveedor-ver-cargo")
    .closest(".kpi-tag");
  if (cargoTag)
    cargoTag.style.display = detail?.position ? "inline-flex" : "none";

  notifyWarning(detail?.expiredCleanupWarning);
}

async function handleDeleteSupplier(supplierId) {
  const supplier = suppliers.find(
    (item) => Number(item.id) === Number(supplierId),
  );
  if (!supplier) return;

  // El listado no trae el recuento: se pide el detalle antes de confirmar (PV-05).
  let invoiceCount = 0;
  try {
    const detail = await getSupplierDetail(supplierId);
    // Se cuentan todas, también las canceladas: todas conservan la copia.
    invoiceCount = detail?.invoiceHistory?.length ?? 0;
  } catch {
    // Sin el detalle se confirma igual, sin el recuento.
  }
  const name = supplier.fullName ?? "este proveedor";
  const confirmed = await confirmDialog({
    title: "Eliminar proveedor",
    message:
      invoiceCount > 0
        ? `${name} tiene ${invoiceCount} ${invoiceCount === 1 ? "factura" : "facturas"}; se conservarán con la copia de sus datos.
¿Deseas eliminarlo?`
        : `¿Deseas eliminar a ${name}?`,
    confirmText: "Eliminar",
  });

  if (!confirmed) return;

  try {
    await deleteSupplier(supplierId);
    notifySuccess(`Proveedor «${name}» eliminado.`);
    const detailId =
      currentSupplierId === Number(supplierId) ? null : currentSupplierId;
    currentSupplierId = detailId;
    await loadSuppliers();
    showProveedorTab("list", document.getElementById("tab-proveedor-list"));
  } catch (error) {
    notifyError(error, "No se pudo eliminar el proveedor.");
  }
}

window.renderProveedores = () => {
  const searchInput = document.getElementById("proveedor-search");
  const query = searchInput ? String(searchInput.value ?? "").trim() : "";
  void loadSuppliers(query);
};

window.showProveedorTab = showProveedorTab;

registerDraftOpener("supplier", async (supplierId) => {
  window.showPage?.(
    "proveedores",
    document.querySelector('#sidebar [data-page="proveedores"]'),
  );
  await loadSuppliers();
  if (supplierId == null) {
    showProveedorTab("create", document.getElementById("tab-proveedor-create"));
  } else openSupplierForm(supplierId);
});
window.cancelarProveedorForm = () => {
  supplierDraft?.clear();
  resetSupplierForm();
  showProveedorTab("list", document.getElementById("tab-proveedor-list"));
};
window.guardarProveedor = (event) => {
  if (event) event.preventDefault();
  void handleSupplierSubmit();
};
window.editarProveedor = (id) => openSupplierForm(id);
window.eliminarProveedor = (id) => void handleDeleteSupplier(id);
window.verProveedor = (id) => void openSupplierDetail(id);
window.renderHistorialProveedor = () => renderProveedorHistory();
window.editarProveedorDesdeVer = () => {
  if (currentSupplierId != null) openSupplierForm(currentSupplierId);
};
window.eliminarProveedorDesdeVer = () => {
  if (currentSupplierId != null) void handleDeleteSupplier(currentSupplierId);
};

if (document.readyState === "loading") {
  document.addEventListener("DOMContentLoaded", () => {
    void initProveedoresView();
  });
}
