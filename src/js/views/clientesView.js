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
  createClient,
  deleteClient,
  getClientDetail,
  listClients,
  searchClients,
  updateClient,
} from "../services/clientsService.js";
import { formatDate, formatId, formatMoney } from "../utils/formatters.js";
import { el, messageRow, emptyState } from "../utils/dom.js";
import { debounce } from "../utils/requestGuard.js";

let clientDraft = null;
let clients = [];
let editingClientId = null;
let currentClientId = null;

export function initClientesView() {
  return initializeClientesView();
}

async function initializeClientesView() {
  removeLegacyDateField();
  bindPageChangeListener();
  // Cada page:change vuelve a inicializar la vista: los listeners se enlazan
  // una sola vez para que un envío no cree el cliente dos veces.
  const page = document.getElementById("page-clientes");
  if (page?.dataset.clientesBound !== "1") {
    if (page) page.dataset.clientesBound = "1";
    bindTabButtons();
    bindSearch();
    bindForm();
    bindTableActions();
    bindDetailActions();
  }
  await loadClients();
  showClienteTab("list", document.getElementById("tab-cliente-list"));
}

function removeLegacyDateField() {
  const field = document
    .getElementById("cliente-fecha-entrada")
    ?.closest(".field");
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

  document
    .getElementById("tab-cliente-create")
    ?.addEventListener("click", () => {
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
  clientDraft = attachDraft(form, {
    formKey: "client",
    recordId: () => editingClientId,
  });
  document.getElementById("cliente-cancelar")?.addEventListener("click", () => {
    clientDraft?.clear();
    resetClientForm();
    showClienteTab("list", document.getElementById("tab-cliente-list"));
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
    const isActive =
      tabButton === button || tabButton.id === `tab-cliente-${tab}`;
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
  if (tab === "create") void clientDraft?.offer();
}

async function loadClients(query = "") {
  const body = document.getElementById("clientes-body");
  if (!body) return;

  messageRow(body, "Cargando clientes…", 5);

  try {
    const response = query ? await searchClients(query) : await listClients();
    const items = Array.isArray(response) ? response : (response?.items ?? []);
    clients = items;
    renderClientList(items);
  } catch (error) {
    messageRow(body, "No se pudieron cargar los clientes.", 5);
    notifyError(error, "No se pudieron cargar los clientes.");
  }
}

function renderClientList(items) {
  const body = document.getElementById("clientes-body");
  if (!body) return;

  if (!items.length) {
    emptyState(body, {
      empty: "Aún no hay clientes.",
      filtered: Boolean(
        document.getElementById("cliente-search")?.value.trim(),
      ),
      action: {
        label: "Crear cliente",
        onClick: () =>
          showClienteTab(
            "create",
            document.getElementById("tab-cliente-create"),
          ),
      },
    });
    return;
  }

  body.replaceChildren(
    ...items.map((client) => {
      const id = Number(client.id);
      return el(
        "tr",
        { dataset: { clientId: id } },
        el("td", {}, formatId(client.id)),
        el("td", {}, client.fullName ?? ""),
        el("td", {}, client.company ?? "—"),
        el("td", {}, client.phone ?? "—"),
        el(
          "td",
          {},
          ROW_ACTIONS.map(([action, label, extra]) =>
            el(
              "button",
              {
                type: "button",
                class: `action-link${extra}`,
                dataset: { clientAction: action, clientId: id },
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
    invoiceInfo:
      document.getElementById("cliente-info-factura")?.value.trim() ?? "",
  };

  clearFieldErrors(form);
  if (!requireFields(["cliente-nombres"])) {
    notifyError(null, "Escribe el nombre del cliente.");
    return;
  }

  try {
    if (editingClientId !== null) {
      await updateClient(editingClientId, payload);
      notifySuccess(
        `Cliente «${payload.firstName} ${payload.lastName}» actualizado.`,
      );
    } else {
      await createClient(payload);
      notifySuccess(
        `Cliente «${payload.firstName} ${payload.lastName}» creado.`,
      );
    }

    clientDraft?.clear();
    resetClientForm();
    await loadClients();
    showClienteTab("list", document.getElementById("tab-cliente-list"));
  } catch (error) {
    reportError(error, "No se pudo guardar el cliente.", CLIENT_FIELDS);
  }
}

// Campo al que se refiere cada rechazo de validación del backend (TO-01).
const CLIENT_FIELDS = [
  [/apellidos/i, "cliente-apellidos"],
  [/nombre/i, "cliente-nombres"],
  [/carnet/i, "cliente-ci"],
  [/teléfono/i, "cliente-telefono"],
  [/correo/i, "cliente-correo"],
  [/empresa/i, "cliente-empresa"],
  [/cargo/i, "cliente-cargo"],
  [/dirección/i, "cliente-direccion"],
  [/factura/i, "cliente-info-factura"],
];

function resetClientForm() {
  editingClientId = null;
  const form = document.getElementById("cliente-form");
  if (form) form.reset();
  document
    .getElementById("cliente-form-title")
    ?.replaceChildren(document.createTextNode("Datos de Cliente"));
}

function openClientForm(clientId) {
  const client = clients.find((item) => Number(item.id) === Number(clientId));
  if (!client) return;

  editingClientId = Number(clientId);
  const formTitle = document.getElementById("cliente-form-title");
  if (formTitle)
    formTitle.textContent = `Editar cliente: ${client.fullName ?? ""}`;

  document.getElementById("cliente-nombres").value =
    client.firstName ?? client.fullName?.split(" ")[0] ?? "";
  document.getElementById("cliente-apellidos").value =
    client.lastName ??
    client.fullName?.replace(client.firstName ?? "", "").trim() ??
    "";
  document.getElementById("cliente-ci").value = client.nationalId ?? "";
  document.getElementById("cliente-telefono").value = client.phone ?? "";
  document.getElementById("cliente-correo").value = client.email ?? "";
  document.getElementById("cliente-empresa").value = client.company ?? "";
  document.getElementById("cliente-cargo").value = client.position ?? "";
  document.getElementById("cliente-direccion").value = client.address ?? "";
  document.getElementById("cliente-info-factura").value =
    client.invoiceInfo ?? "";

  showClienteTab("create", document.getElementById("tab-cliente-create"));
}

async function openClientDetail(clientId) {
  try {
    const detail = await getClientDetail(clientId);
    currentClientId = Number(clientId);
    renderClientDetail(detail);
    showClienteTab("ver", null);
  } catch (error) {
    notifyError(error, "No se pudo abrir el detalle del cliente.");
  }
}

function renderClientDetail(detail) {
  const fullName =
    [detail?.firstName, detail?.lastName].filter(Boolean).join(" ") ||
    detail?.fullName ||
    "Cliente";
  const indicators = detail?.indicators ?? {};

  document.getElementById("cliente-ver-id").textContent = formatId(
    detail?.id ?? "",
  );
  document.getElementById("cliente-ver-nombre-completo").textContent = fullName;
  document.getElementById("cliente-ver-empresa").textContent =
    detail?.company || "Sin empresa";
  document.getElementById("cliente-ver-cargo").textContent =
    detail?.position || "Sin cargo";
  document.getElementById("cliente-ver-telefono").textContent =
    detail?.phone || "—";
  document.getElementById("cliente-ver-correo").textContent =
    detail?.email || "—";
  document.getElementById("cliente-ver-ci").textContent =
    detail?.nationalId || "—";
  document.getElementById("cliente-ver-direccion").textContent =
    detail?.address || "—";
  document.getElementById("cliente-ver-fecha-entrada").textContent =
    detail?.createdAt ? formatDate(detail.createdAt) : "—";
  document.getElementById("cliente-ver-info-factura-nota").textContent =
    detail?.invoiceInfo || "Sin información adicional de factura.";
  document.getElementById("cliente-ver-total-facturado").textContent =
    formatMoney(indicators.totalInvoicedCents ?? 0).replace(" USD", "");
  document.getElementById("cliente-ver-saldo-pendiente").textContent =
    formatMoney(indicators.pendingBalanceCents ?? 0).replace(" USD", "");
  document.getElementById("cliente-ver-num-facturas").textContent = String(
    indicators.invoiceCount ?? 0,
  );
  document.getElementById("cliente-ver-ultima-compra").textContent =
    indicators.lastPurchaseDate ? formatDate(indicators.lastPurchaseDate) : "—";

  renderInvoiceHistory(
    document.getElementById("cliente-ver-historial-facturas"),
    detail?.invoiceHistory,
    "Este cliente aún no tiene facturas registradas.",
  );

  const companyTag = document
    .getElementById("cliente-ver-empresa")
    .closest(".kpi-tag");
  if (companyTag)
    companyTag.style.display = detail?.company ? "inline-flex" : "none";

  const cargoTag = document
    .getElementById("cliente-ver-cargo")
    .closest(".kpi-tag");
  if (cargoTag)
    cargoTag.style.display = detail?.position ? "inline-flex" : "none";

  notifyWarning(detail?.expiredCleanupWarning);
}

async function handleDeleteClient(clientId) {
  const client = clients.find((item) => Number(item.id) === Number(clientId));
  if (!client) return;

  // El listado no trae el recuento: se pide el detalle antes de confirmar (CL-05).
  let invoiceCount = 0;
  try {
    const detail = await getClientDetail(clientId);
    // Se cuentan todas, también las canceladas: todas conservan la copia.
    invoiceCount = detail?.invoiceHistory?.length ?? 0;
  } catch {
    // Sin el detalle se confirma igual, sin el recuento.
  }
  const name = client.fullName ?? "este cliente";
  const confirmed = await confirmDialog({
    title: "Eliminar cliente",
    message:
      invoiceCount > 0
        ? `${name} tiene ${invoiceCount} ${invoiceCount === 1 ? "factura" : "facturas"}; se conservarán con la copia de sus datos.\n¿Deseas eliminarlo?`
        : `¿Deseas eliminar a ${name}?`,
    confirmText: "Eliminar",
  });

  if (!confirmed) return;

  try {
    await deleteClient(clientId);
    notifySuccess(`Cliente «${name}» eliminado.`);
    const detailId =
      currentClientId === Number(clientId) ? null : currentClientId;
    currentClientId = detailId;
    await loadClients();
    showClienteTab("list", document.getElementById("tab-cliente-list"));
  } catch (error) {
    notifyError(error, "No se pudo eliminar el cliente.");
  }
}

window.showClienteTab = showClienteTab;

registerDraftOpener("client", async (clientId) => {
  window.showPage?.(
    "clientes",
    document.querySelector('#sidebar [data-page="clientes"]'),
  );
  await loadClients();
  if (clientId == null) {
    showClienteTab("create", document.getElementById("tab-cliente-create"));
  } else openClientForm(clientId);
});
window.cancelarClienteForm = () => {
  clientDraft?.clear();
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
} else {
  // Los módulos corren con el documento ya analizado: nunca en "loading".
  void initClientesView();
}
