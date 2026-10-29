import { confirmDialog } from "../components/confirmDialog.js";
import { renderInvoiceHistory } from "../components/invoiceHistory.js";
import { showCleanupWarning } from "../utils/cleanupWarning.js";
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
import { debounce } from "../utils/requestGuard.js";

let suppliers = [];
let editingSupplierId = null;
let currentSupplierId = null;
let historyItems = [];
let historyRequest = 0;

export function initProveedoresView() {
  return initializeProveedoresView();
}

async function initializeProveedoresView() {
  removeLegacyInlineHandlers();
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

function removeLegacyInlineHandlers() {
  document
    .querySelectorAll("#page-proveedores [onclick]")
    .forEach((element) => element.removeAttribute("onclick"));
  document
    .querySelectorAll("#page-proveedores [oninput]")
    .forEach((element) => element.removeAttribute("oninput"));

  const form = document.getElementById("proveedor-form");
  if (form) form.removeAttribute("onsubmit");
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
  document
    .getElementById("proveedor-cancelar")
    ?.addEventListener("click", () => {
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
}

function escapeHtml(value) {
  return String(value ?? "")
    .replace(/&/g, "&amp;")
    .replace(/</g, "&lt;")
    .replace(/>/g, "&gt;")
    .replace(/\"/g, "&quot;")
    .replace(/'/g, "&#039;");
}

async function loadSuppliers(query = "") {
  const body = document.getElementById("proveedores-body");
  if (!body) return;

  body.innerHTML =
    '<tr><td colspan="5" class="empty-row">Cargando proveedores…</td></tr>';

  try {
    const response = query
      ? await searchSuppliers(query)
      : await listSuppliers();
    const items = Array.isArray(response) ? response : (response?.items ?? []);
    suppliers = items;
    renderSupplierList(items);
    renderProveedorHistory();
  } catch (error) {
    body.innerHTML = `
      <tr>
        <td colspan="5" class="empty-row">No se pudieron cargar los proveedores.</td>
      </tr>
    `;
    showSupplierFeedback(
      error?.message || "No se pudieron cargar los proveedores.",
    );
  }
}

function renderSupplierList(items) {
  const body = document.getElementById("proveedores-body");
  if (!body) return;

  if (!items.length) {
    body.innerHTML =
      '<tr><td colspan="5" class="empty-row">Aún no hay proveedores.</td></tr>';
    return;
  }

  body.innerHTML = items
    .map(
      (supplier) => `
        <tr data-supplier-id="${Number(supplier.id)}">
          <td>${escapeHtml(formatId(supplier.id))}</td>
          <td>${escapeHtml(supplier.fullName ?? "")}</td>
          <td>${escapeHtml(supplier.company ?? "—")}</td>
          <td>${escapeHtml(supplier.phone ?? "—")}</td>
          <td>
            <button type="button" class="action-link" data-supplier-action="detail" data-supplier-id="${Number(supplier.id)}">Ver</button>
            <button type="button" class="action-link" data-supplier-action="edit" data-supplier-id="${Number(supplier.id)}">Editar</button>
            <button type="button" class="action-link danger" data-supplier-action="delete" data-supplier-id="${Number(supplier.id)}">Eliminar</button>
          </td>
        </tr>
      `,
    )
    .join("");
}

// Historial de compras (PV-07): las facturas de entrada del proveedor salen
// de getSupplierDetail; el rango de fechas y el orden se aplican aquí.
function renderProveedorHistory() {
  const select = document.getElementById("historial-proveedor-select");
  const body = document.getElementById("historial-proveedor-body");

  if (!select || !body) return;

  if (!suppliers.length) {
    select.innerHTML = '<option value="">No hay proveedores</option>';
    historyItems = [];
    historyMessage(body, "No hay proveedores para consultar.");
    return;
  }

  const currentValue = select.value;
  const options = suppliers
    .map(
      (supplier) => `
        <option value="${Number(supplier.id)}">${escapeHtml(supplier.fullName || "Proveedor")}</option>
      `,
    )
    .join("");

  select.innerHTML = options;

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
  const row = document.createElement("tr");
  const cell = document.createElement("td");
  cell.colSpan = 5;
  cell.className = "empty-row";
  cell.textContent = text;
  row.append(cell);
  body.replaceChildren(row);
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
    const cells = [
      formatDate(item.date),
      item.number ?? "—",
      ENTRY_STATUS_LABELS[item.status] ?? item.status ?? "—",
      formatMoney(item.totalCents ?? 0),
    ].map((text) => {
      const cell = document.createElement("td");
      cell.textContent = text;
      return cell;
    });
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

  if (
    !payload.firstName ||
    !payload.lastName ||
    !payload.nationalId ||
    !payload.phone
  ) {
    showSupplierFeedback("Completa los campos obligatorios del proveedor.");
    return;
  }

  try {
    if (editingSupplierId !== null) {
      await updateSupplier(editingSupplierId, payload);
      showSupplierFeedback("Proveedor actualizado correctamente.");
    } else {
      await createSupplier(payload);
      showSupplierFeedback("Proveedor creado correctamente.");
    }

    resetSupplierForm();
    await loadSuppliers();
    showProveedorTab("list", document.getElementById("tab-proveedor-list"));
  } catch (error) {
    showSupplierFeedback(error?.message || "No se pudo guardar el proveedor.");
  }
}

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
    showSupplierFeedback(
      error?.message || "No se pudo abrir el detalle del proveedor.",
    );
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

  showCleanupWarning(
    document.getElementById("proveedor-ver"),
    "proveedor-ver-warning",
    detail?.expiredCleanupWarning,
  );
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
    const detailId =
      currentSupplierId === Number(supplierId) ? null : currentSupplierId;
    currentSupplierId = detailId;
    await loadSuppliers();
    showProveedorTab("list", document.getElementById("tab-proveedor-list"));
  } catch (error) {
    showSupplierFeedback(error?.message || "No se pudo eliminar el proveedor.");
  }
}

function showSupplierFeedback(message) {
  const form = document.getElementById("proveedor-form");
  if (!form) return;

  let feedback = document.getElementById("proveedor-feedback");
  if (!feedback) {
    feedback = document.createElement("div");
    feedback.id = "proveedor-feedback";
    feedback.className = "form-feedback";
    form.appendChild(feedback);
  }

  feedback.textContent = message;
  feedback.hidden = !message;
}

window.renderProveedores = () => {
  const searchInput = document.getElementById("proveedor-search");
  const query = searchInput ? String(searchInput.value ?? "").trim() : "";
  void loadSuppliers(query);
};

window.showProveedorTab = showProveedorTab;
window.cancelarProveedorForm = () => {
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
