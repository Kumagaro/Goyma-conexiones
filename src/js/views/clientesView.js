import { confirmDialog } from "../components/confirmDialog.js";
import {
  createClient,
  deleteClient,
  getClientDetail,
  listClients,
  searchClients,
  updateClient,
} from "../services/clientsService.js";
import { formatDate, formatId, formatMoney } from "../utils/formatters.js";
import { debounce } from "../utils/requestGuard.js";

let clients = [];
let editingClientId = null;
let currentClientId = null;

export function initClientesView() {
  return initializeClientesView();
}

async function initializeClientesView() {
  removeLegacyInlineHandlers();
  removeLegacyDateField();
  bindPageChangeListener();
  bindTabButtons();
  bindSearch();
  bindForm();
  bindTableActions();
  bindDetailActions();
  await loadClients();
  showClienteTab("list", document.getElementById("tab-cliente-list"));
}

function removeLegacyInlineHandlers() {
  document
    .querySelectorAll("#page-clientes [onclick]")
    .forEach((element) => element.removeAttribute("onclick"));
  document
    .querySelectorAll("#page-clientes [oninput]")
    .forEach((element) => element.removeAttribute("oninput"));
  const form = document.getElementById("cliente-form");
  if (form) form.removeAttribute("onsubmit");
}

function removeLegacyDateField() {
  const field = document.getElementById("cliente-fecha-entrada")?.closest(".field");
  if (field) field.remove();
}

function bindPageChangeListener() {
  if (window.__clientesPageChangeBound) return;

  window.addEventListener("page:change", (event) => {
    if (event.detail?.pageId !== "clientes") return;
    void initClientesView();
  });

  window.__clientesPageChangeBound = true;
}

function bindTabButtons() {
  document.getElementById("tab-cliente-list")?.addEventListener("click", () => {
    showClienteTab("list", document.getElementById("tab-cliente-list"));
  });

  document.getElementById("tab-cliente-create")?.addEventListener("click", () => {
    showClienteTab("create", document.getElementById("tab-cliente-create"));
  });
}

function bindSearch() {
  const searchInput = document.getElementById("cliente-search");
  if (!searchInput) return;

  const debouncedSearch = debounce((query) => {
    void loadClients(query);
  }, 300);

  searchInput.addEventListener("input", (event) => {
    const value = String(event.target.value ?? "").trim();
    debouncedSearch(value);
  });
}

function bindForm() {
  const form = document.getElementById("cliente-form");
  if (!form) return;

  form.addEventListener("submit", (event) => {
    event.preventDefault();
    void handleClientSubmit();
  });
}

function bindTableActions() {
  const body = document.getElementById("clientes-body");
  if (!body) return;

  body.addEventListener("click", (event) => {
    const action = event.target.closest("[data-client-action]");
    if (!action) return;

    const clientId = Number(action.dataset.clientId);
    if (!Number.isFinite(clientId)) return;

    const actionName = action.dataset.clientAction;
    if (actionName === "detail") {
      void openClientDetail(clientId);
      return;
    }

    if (actionName === "edit") {
      openClientForm(clientId);
      return;
    }

    if (actionName === "delete") {
      void handleDeleteClient(clientId);
    }
  });
}

function bindDetailActions() {
  const editButton = document.getElementById("cliente-ver-editar");
  if (editButton) {
    editButton.addEventListener("click", () => {
      if (currentClientId != null) openClientForm(currentClientId);
    });
  }

  const deleteButton = document.getElementById("cliente-ver-eliminar");
  if (deleteButton) {
    deleteButton.addEventListener("click", () => {
      if (currentClientId != null) void handleDeleteClient(currentClientId);
    });
  }
}

function showClienteTab(tab, button = null) {
  const tabs = document.querySelectorAll("#page-clientes .view-tabs .tab");
  tabs.forEach((tabButton) => {
    const isActive = tabButton === button || tabButton.id === `tab-cliente-${tab}`;
    tabButton.classList.toggle("active", isActive);
  });

  const subviews = document.querySelectorAll("#page-clientes .config-subview");
  subviews.forEach((view) => {
    const isActive = view.id === `cliente-${tab}`;
    view.classList.toggle("active", isActive);
  });

  const searchBar = document.getElementById("cliente-search-bar");
  if (searchBar) searchBar.style.display = tab === "list" ? "" : "none";

  if (tab === "create" && editingClientId === null) resetClientForm();
}

function escapeHtml(value) {
  return String(value ?? "")
    .replace(/&/g, "&amp;")
    .replace(/</g, "&lt;")
    .replace(/>/g, "&gt;")
    .replace(/\"/g, "&quot;")
    .replace(/'/g, "&#039;");
}

async function loadClients(query = "") {
  const body = document.getElementById("clientes-body");
  if (!body) return;

  body.innerHTML = '<tr><td colspan="5" class="empty-row">Cargando clientes…</td></tr>';

  try {
    const response = query
      ? await searchClients(query)
      : await listClients();
    const items = Array.isArray(response) ? response : response?.items ?? [];
    clients = items;
    renderClientList(items);
  } catch (error) {
    body.innerHTML = `
      <tr>
        <td colspan="5" class="empty-row">No se pudieron cargar los clientes.</td>
      </tr>
    `;
    showClientFeedback(error?.message || "No se pudieron cargar los clientes.");
  }
}

function renderClientList(items) {
  const body = document.getElementById("clientes-body");
  if (!body) return;

  if (!items.length) {
    body.innerHTML =
      '<tr><td colspan="5" class="empty-row">Aún no hay clientes.</td></tr>';
    return;
  }

  body.innerHTML = items
    .map(
      (client) => `
        <tr data-client-id="${Number(client.id)}">
          <td>${escapeHtml(formatId(client.id))}</td>
          <td>${escapeHtml(client.fullName ?? "")}</td>
          <td>${escapeHtml(client.company ?? "—")}</td>
          <td>${escapeHtml(client.phone ?? "—")}</td>
          <td>
            <button type="button" class="action-link" data-client-action="detail" data-client-id="${Number(client.id)}">Ver</button>
            <button type="button" class="action-link" data-client-action="edit" data-client-id="${Number(client.id)}">Editar</button>
            <button type="button" class="action-link danger" data-client-action="delete" data-client-id="${Number(client.id)}">Eliminar</button>
          </td>
        </tr>
      `,
    )
    .join("");
}

async function handleClientSubmit() {
  const form = document.getElementById("cliente-form");
  if (!form) return;

  const payload = {
    firstName: document.getElementById("cliente-nombres")?.value.trim() ?? "",
    lastName: document.getElementById("cliente-apellidos")?.value.trim() ?? "",
    nationalId: document.getElementById("cliente-ci")?.value.trim() ?? "",
    phone: document.getElementById("cliente-telefono")?.value.trim() ?? "",
    email: document.getElementById("cliente-correo")?.value.trim() ?? "",
    company: document.getElementById("cliente-empresa")?.value.trim() ?? "",
    position: document.getElementById("cliente-cargo")?.value.trim() ?? "",
    address: document.getElementById("cliente-direccion")?.value.trim() ?? "",
    invoiceInfo: document.getElementById("cliente-info-factura")?.value.trim() ?? "",
  };

  if (!payload.firstName || !payload.lastName || !payload.nationalId || !payload.phone) {
    showClientFeedback("Completa los campos obligatorios del cliente.");
    return;
  }

  try {
    if (editingClientId !== null) {
      await updateClient(editingClientId, payload);
      showClientFeedback("Cliente actualizado correctamente.");
    } else {
      await createClient(payload);
      showClientFeedback("Cliente creado correctamente.");
    }

    resetClientForm();
    await loadClients();
    showClienteTab("list", document.getElementById("tab-cliente-list"));
  } catch (error) {
    const message = error?.message || "No se pudo guardar el cliente.";
    showClientFeedback(message);
  }
}

function resetClientForm() {
  editingClientId = null;
  const form = document.getElementById("cliente-form");
  if (form) form.reset();
  document.getElementById("cliente-form-title")?.replaceChildren(
    document.createTextNode("Datos de Cliente"),
  );
}

function openClientForm(clientId) {
  const client = clients.find((item) => Number(item.id) === Number(clientId));
  if (!client) return;

  editingClientId = Number(clientId);
  const formTitle = document.getElementById("cliente-form-title");
  if (formTitle) formTitle.textContent = `Editar cliente: ${client.fullName ?? ""}`;

  document.getElementById("cliente-nombres").value = client.firstName ?? client.fullName?.split(" ")[0] ?? "";
  document.getElementById("cliente-apellidos").value = client.lastName ?? client.fullName?.replace(client.firstName ?? "", "").trim() ?? "";
  document.getElementById("cliente-ci").value = client.nationalId ?? "";
  document.getElementById("cliente-telefono").value = client.phone ?? "";
  document.getElementById("cliente-correo").value = client.email ?? "";
  document.getElementById("cliente-empresa").value = client.company ?? "";
  document.getElementById("cliente-cargo").value = client.position ?? "";
  document.getElementById("cliente-direccion").value = client.address ?? "";
  document.getElementById("cliente-info-factura").value = client.invoiceInfo ?? "";

  showClienteTab("create", document.getElementById("tab-cliente-create"));
}

async function openClientDetail(clientId) {
  try {
    const detail = await getClientDetail(clientId);
    currentClientId = Number(clientId);
    renderClientDetail(detail);
    showClienteTab("ver", null);
  } catch (error) {
    showClientFeedback(error?.message || "No se pudo abrir el detalle del cliente.");
  }
}

function renderClientDetail(detail) {
  const fullName = [detail?.firstName, detail?.lastName].filter(Boolean).join(" ") || detail?.fullName || "Cliente";
  const indicators = detail?.indicators ?? {};

  document.getElementById("cliente-ver-id").textContent = formatId(detail?.id ?? "");
  document.getElementById("cliente-ver-nombre-completo").textContent = fullName;
  document.getElementById("cliente-ver-empresa").textContent = detail?.company || "Sin empresa";
  document.getElementById("cliente-ver-cargo").textContent = detail?.position || "Sin cargo";
  document.getElementById("cliente-ver-telefono").textContent = detail?.phone || "—";
  document.getElementById("cliente-ver-correo").textContent = detail?.email || "—";
  document.getElementById("cliente-ver-ci").textContent = detail?.nationalId || "—";
  document.getElementById("cliente-ver-direccion").textContent = detail?.address || "—";
  document.getElementById("cliente-ver-fecha-entrada").textContent = detail?.createdAt ? formatDate(detail.createdAt) : "—";
  document.getElementById("cliente-ver-info-factura-nota").textContent = detail?.invoiceInfo || "Sin información adicional de factura.";
  document.getElementById("cliente-ver-total-facturado").textContent = formatMoney(indicators.totalInvoicedCents ?? 0).replace(" USD", "");
  document.getElementById("cliente-ver-saldo-pendiente").textContent = formatMoney(indicators.pendingBalanceCents ?? 0).replace(" USD", "");
  document.getElementById("cliente-ver-num-facturas").textContent = String(indicators.invoiceCount ?? 0);
  document.getElementById("cliente-ver-ultima-compra").textContent = indicators.lastPurchaseDate ? formatDate(indicators.lastPurchaseDate) : "—";

  const history = Array.isArray(detail?.invoiceHistory) ? detail.invoiceHistory : [];
  const historyContainer = document.getElementById("cliente-ver-historial-facturas");
  if (!history.length) {
    historyContainer.innerHTML = '<span class="pv-empty-note">Este cliente aún no tiene facturas registradas.</span>';
  } else {
    historyContainer.innerHTML = history
      .map(
        (invoice) => `
          <div class="pv-invoice-item">
            <span>${escapeHtml(invoice.number || "Factura")}</span>
            <span>${escapeHtml(invoice.type || "")}</span>
          </div>
        `,
      )
      .join("");
  }

  const companyTag = document.getElementById("cliente-ver-empresa").closest(".kpi-tag");
  if (companyTag) companyTag.style.display = detail?.company ? "inline-flex" : "none";

  const cargoTag = document.getElementById("cliente-ver-cargo").closest(".kpi-tag");
  if (cargoTag) cargoTag.style.display = detail?.position ? "inline-flex" : "none";

  if (detail?.expiredCleanupWarning) {
    const warning = document.getElementById("cliente-ver-warning");
    if (warning) {
      warning.textContent = detail.expiredCleanupWarning;
      warning.hidden = false;
    }
  }
}

async function handleDeleteClient(clientId) {
  const client = clients.find((item) => Number(item.id) === Number(clientId));
  if (!client) return;

  const invoiceCount = Number(client.invoiceCount ?? 0);
  const confirmed = await confirmDialog({
    title: "Eliminar cliente",
    message: invoiceCount > 0
      ? `Se conservarán ${invoiceCount} facturas asociadas a ${client.fullName ?? "este cliente"}.\n¿Deseas continuar?`
      : `¿Deseas eliminar a ${client.fullName ?? "este cliente"}?`,
    confirmText: "Eliminar",
  });

  if (!confirmed) return;

  try {
    await deleteClient(clientId);
    const detailId = currentClientId === Number(clientId) ? null : currentClientId;
    currentClientId = detailId;
    await loadClients();
    showClienteTab("list", document.getElementById("tab-cliente-list"));
  } catch (error) {
    showClientFeedback(error?.message || "No se pudo eliminar el cliente.");
  }
}

function showClientFeedback(message) {
  const form = document.getElementById("cliente-form");
  if (!form) return;

  let feedback = document.getElementById("cliente-feedback");
  if (!feedback) {
    feedback = document.createElement("div");
    feedback.id = "cliente-feedback";
    feedback.className = "form-feedback";
    form.appendChild(feedback);
  }

  feedback.textContent = message;
  feedback.hidden = !message;
}

window.showClienteTab = showClienteTab;
window.cancelarClienteForm = () => {
  resetClientForm();
  showClienteTab("list", document.getElementById("tab-cliente-list"));
};
window.guardarCliente = (event) => {
  if (event) event.preventDefault();
  void handleClientSubmit();
};
window.editarCliente = (id) => openClientForm(id);
window.eliminarCliente = (id) => void handleDeleteClient(id);
window.verCliente = (id) => void openClientDetail(id);
window.editarClienteDesdeVer = () => {
  if (currentClientId != null) openClientForm(currentClientId);
};
window.eliminarClienteDesdeVer = () => {
  if (currentClientId != null) void handleDeleteClient(currentClientId);
};

if (document.readyState === "loading") {
  document.addEventListener("DOMContentLoaded", () => {
    void initClientesView();
  });
}
