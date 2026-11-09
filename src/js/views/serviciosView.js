// spec-06 SV-01: catálogo de servicios (listado, formulario y ficha).
import { confirmDialog } from "../components/confirmDialog.js";
import { createCombobox } from "../components/combobox.js";
import {
  createService,
  deleteService,
  getService,
  listServices,
  updateService,
} from "../services/servicesService.js";
import { searchProductsForInvoice } from "../services/historyService.js";
import {
  clearFieldErrors,
  notifyError,
  notifySuccess,
  reportError,
  requireFields,
} from "../utils/feedback.js";
import { el, emptyState, messageRow } from "../utils/dom.js";
import { formatMoney } from "../utils/formatters.js";
import { debounce } from "../utils/requestGuard.js";

let services = [];
let editingId = null;
let viewingId = null;
let formProducts = [];
let productCombobox = null;
let initPromise = null;

const byId = (id) => document.getElementById(id);

function trashIcon() {
  const svg = document.createElementNS("http://www.w3.org/2000/svg", "svg");
  svg.setAttribute("class", "icon");
  svg.setAttribute("aria-hidden", "true");
  const use = document.createElementNS("http://www.w3.org/2000/svg", "use");
  use.setAttribute("href", "#icon-trash");
  svg.append(use);
  return svg;
}

function centsToText(cents) {
  if (cents == null) return "";
  return (cents / 100).toFixed(2).replace(".", ",");
}

export function initServiciosView() {
  if (!initPromise) initPromise = initialize();
  return initPromise;
}

async function initialize() {
  const form = byId("servicio-form");
  if (!form) return;
  form.addEventListener("submit", (event) => {
    event.preventDefault();
    void handleSubmit();
  });
  byId("tab-servicio-list")?.addEventListener("click", () => showTab("list"));
  byId("tab-servicio-create")?.addEventListener("click", () => {
    resetForm();
    showTab("create");
  });
  document
    .querySelector("[data-servicio-action='cancel']")
    ?.addEventListener("click", () => {
      resetForm();
      showTab("list");
    });
  document
    .querySelector("[data-servicio-detail-action='back']")
    ?.addEventListener("click", () => showTab("list"));
  document
    .querySelector("[data-servicio-detail-action='edit']")
    ?.addEventListener("click", () => {
      if (viewingId != null) void openForm(viewingId);
    });
  document
    .querySelector("[data-servicio-detail-action='delete']")
    ?.addEventListener("click", () => {
      if (viewingId != null) void handleDelete(viewingId);
    });
  byId("servicio-search")?.addEventListener(
    "input",
    debounce((event) => void loadServices(event.target.value.trim()), 250),
  );
  byId("servicios-body")?.addEventListener("click", handleTableClick);
  mountProductSearch();
  renderFormProducts();
  window.addEventListener("page:change", (event) => {
    if (event.detail?.pageId === "servicios") void loadServices();
  });
  await loadServices();
  showTab("list");
}

function mountProductSearch() {
  const original = byId("servicio-buscar-producto");
  if (!original) return;
  const container = document.createElement("div");
  container.style.flex = "1";
  original.replaceWith(container);
  productCombobox = createCombobox({
    container,
    id: original.id,
    placeholder: "Escribe para buscar un producto...",
    search: async (query) => {
      if (!query) return [];
      const result = await searchProductsForInvoice(query, "outgoing");
      return Array.isArray(result) ? result : [];
    },
    formatOption: (item) =>
      [item?.name ?? "", item?.isDiscontinued ? "Descontinuado" : null]
        .filter(Boolean)
        .join(" · "),
    // Solo se añaden productos activos (SV-01).
    isOptionDisabled: (item) => item?.isDiscontinued === true,
    onSelect: (item) => {
      if (!item) return;
      addFormProduct(item);
      productCombobox?.setSelected(null);
    },
  });
  const input = productCombobox.getInput();
  input.id = original.id;
  input.setAttribute("aria-label", "Buscar producto");
}

function addFormProduct(item) {
  const productId = Number(item?.id);
  if (!Number.isSafeInteger(productId)) return;
  if (formProducts.some((product) => product.productId === productId)) {
    notifyError(null, `«${item.name}» ya está en el servicio.`);
    return;
  }
  formProducts.push({
    productId,
    name: item?.name ?? "",
    unitAbbreviation: item?.unitAbbreviation ?? null,
    quantity: "1",
    status: "active",
  });
  renderFormProducts();
}

function renderFormProducts() {
  const body = byId("servicio-productos-body");
  if (!body) return;
  if (!formProducts.length) {
    body.replaceChildren(
      el(
        "div",
        { class: "li-empty" },
        "Sin productos. Un servicio puede no llevarlos, como un mantenimiento.",
      ),
    );
    return;
  }
  body.replaceChildren(
    ...formProducts.map((product, index) => {
      const quantity = el("input", {
        type: "text",
        inputmode: "decimal",
        value: product.quantity,
        "aria-label": `Cantidad sugerida de ${product.name}`,
      });
      quantity.addEventListener("input", () => {
        formProducts[index].quantity = quantity.value;
      });
      const remove = el(
        "button",
        {
          type: "button",
          class: "li-remove-btn",
          "aria-label": `Quitar ${product.name}`,
          title: "Quitar producto",
        },
        trashIcon(),
      );
      remove.addEventListener("click", () => {
        formProducts.splice(index, 1);
        renderFormProducts();
      });
      return el(
        "div",
        { class: "li-row li-body li-row-service" },
        el("div", {}, String(index + 1)),
        el(
          "div",
          {},
          product.status === "discontinued"
            ? `${product.name} (descontinuado)`
            : product.name,
        ),
        el("div", {}, quantity),
        el("div", {}, product.unitAbbreviation ?? "—"),
        el("div", {}, remove),
      );
    }),
  );
}

async function loadServices(search = byId("servicio-search")?.value ?? "") {
  const body = byId("servicios-body");
  if (body) messageRow(body, "Cargando servicios…", 5);
  try {
    services = (await listServices(search)) ?? [];
    renderList(search);
  } catch (error) {
    if (body) messageRow(body, "No se pudieron cargar los servicios.", 5);
    notifyError(error, "No se pudieron cargar los servicios.");
  }
}

function renderList(search) {
  const body = byId("servicios-body");
  if (!body) return;
  if (!services.length) {
    emptyState(body, {
      empty: "Aún no hay servicios.",
      filtered: Boolean(search?.trim()),
      action: {
        label: "Crear servicio",
        onClick: () => {
          resetForm();
          showTab("create");
        },
      },
    });
    return;
  }
  body.replaceChildren(
    ...services.map((service) =>
      el(
        "tr",
        {},
        el("td", {}, service.name ?? ""),
        el("td", {}, service.description || "—"),
        el(
          "td",
          { class: "text-right" },
          service.suggestedPriceCents == null
            ? "—"
            : formatMoney(service.suggestedPriceCents),
        ),
        el(
          "td",
          { class: "text-center" },
          String(Number(service.productCount ?? 0)),
        ),
        el(
          "td",
          {},
          el(
            "div",
            { class: "col-acciones" },
            ...[
              ["view", "Ver", ""],
              ["edit", "Editar", ""],
              ["delete", "Eliminar", " danger"],
            ].map(([action, label, extra]) =>
              el(
                "button",
                {
                  type: "button",
                  class: `action-link${extra}`,
                  dataset: { servicioAction: action, serviceId: service.id },
                },
                label,
              ),
            ),
          ),
        ),
      ),
    ),
  );
}

function handleTableClick(event) {
  const target = event.target.closest("button[data-servicio-action]");
  if (!target) return;
  const serviceId = Number(target.dataset.serviceId);
  if (!Number.isSafeInteger(serviceId)) return;
  const action = target.dataset.servicioAction;
  if (action === "view") void openDetail(serviceId);
  else if (action === "edit") void openForm(serviceId);
  else if (action === "delete") void handleDelete(serviceId);
}

function showTab(tab) {
  document
    .querySelectorAll("#page-servicios .config-subview")
    .forEach((view) =>
      view.classList.toggle("active", view.id === `servicio-${tab}`),
    );
  document
    .querySelectorAll("#page-servicios .view-tabs .tab")
    .forEach((button) =>
      button.classList.toggle("active", button.id === `tab-servicio-${tab}`),
    );
  const searchBar = byId("servicio-search-bar");
  if (searchBar) searchBar.style.display = tab === "list" ? "flex" : "none";
}

function resetForm() {
  editingId = null;
  formProducts = [];
  byId("servicio-form")?.reset();
  clearFieldErrors(byId("servicio-form"));
  const title = byId("servicio-form-title");
  if (title) title.textContent = "Datos del Servicio";
  renderFormProducts();
}

async function openForm(serviceId) {
  try {
    const service = await getService(serviceId);
    resetForm();
    editingId = service.id;
    byId("servicio-nombre").value = service.name ?? "";
    byId("servicio-precio").value = centsToText(service.suggestedPriceCents);
    byId("servicio-descripcion").value = service.description ?? "";
    formProducts = (service.products ?? []).map((product) => ({
      productId: product.productId,
      name: product.name,
      unitAbbreviation: product.unitAbbreviation ?? null,
      quantity: product.suggestedQuantity,
      status: product.status,
    }));
    const title = byId("servicio-form-title");
    if (title) title.textContent = `Editar servicio: ${service.name ?? ""}`;
    renderFormProducts();
    showTab("create");
  } catch (error) {
    notifyError(error, "No se pudo abrir el servicio.");
  }
}

async function handleSubmit() {
  const form = byId("servicio-form");
  const nameInput = byId("servicio-nombre");
  const priceInput = byId("servicio-precio");
  const descriptionInput = byId("servicio-descripcion");
  clearFieldErrors(form);
  if (!requireFields([nameInput])) {
    notifyError(null, "El nombre es obligatorio.");
    return;
  }
  const missing = formProducts.find((product) => !product.quantity.trim());
  if (missing) {
    notifyError(null, `Escribe la cantidad sugerida de «${missing.name}».`);
    return;
  }
  const input = {
    name: nameInput.value.trim(),
    description: descriptionInput.value.trim() || null,
    suggestedPrice: priceInput.value.trim() || null,
    products: formProducts.map((product) => ({
      productId: product.productId,
      quantity: product.quantity.trim(),
    })),
  };
  try {
    const saved =
      editingId == null
        ? await createService(input)
        : await updateService(editingId, input);
    notifySuccess(
      editingId == null
        ? `Servicio «${saved.name}» creado.`
        : `Servicio «${saved.name}» actualizado.`,
    );
    resetForm();
    await loadServices();
    showTab("list");
  } catch (error) {
    reportError(error, "No se pudo guardar el servicio.", [
      [/nombre/i, nameInput],
      [/precio/i, priceInput],
      [/descripción/i, descriptionInput],
    ]);
  }
}

async function openDetail(serviceId) {
  try {
    const service = await getService(serviceId);
    viewingId = service.id;
    byId("servicio-ver-id").textContent = `#${service.id}`;
    byId("servicio-ver-nombre").textContent = service.name ?? "";
    byId("servicio-ver-descripcion").textContent =
      service.description || "Sin descripción";
    byId("servicio-ver-precio").textContent =
      service.suggestedPriceCents == null
        ? "Sin precio sugerido"
        : formatMoney(service.suggestedPriceCents);
    byId("servicio-ver-cantidad").textContent = String(
      service.products?.length ?? 0,
    );
    const list = byId("servicio-ver-productos");
    const products = service.products ?? [];
    list.replaceChildren(
      ...(products.length
        ? products.map((product) =>
            el(
              "div",
              { class: "pv-invoice-card" },
              el(
                "div",
                {},
                el(
                  "span",
                  { class: "pv-invoice-label" },
                  `${product.suggestedQuantity} ${product.unitAbbreviation ?? ""}`.trim(),
                ),
                el("div", { class: "pv-invoice-value" }, product.name ?? ""),
                product.status === "discontinued"
                  ? el(
                      "span",
                      { class: "status-pill is-cancelled" },
                      "Descontinuado",
                    )
                  : null,
              ),
            ),
          )
        : [el("p", {}, "Este servicio no tiene productos asociados.")]),
    );
    showTab("ver");
  } catch (error) {
    notifyError(error, "No se pudo abrir el servicio.");
  }
}

async function handleDelete(serviceId) {
  const service = services.find((item) => item.id === serviceId);
  const name = service?.name ?? byId("servicio-ver-nombre")?.textContent ?? "";
  const confirmed = await confirmDialog({
    title: "Eliminar servicio",
    message: `¿Eliminar el servicio «${name}»?\nLas facturas que lo usaron no cambian.`,
    confirmText: "Eliminar",
  });
  if (!confirmed) return;
  try {
    await deleteService(serviceId);
    notifySuccess(`Servicio «${name}» eliminado.`);
    viewingId = null;
    await loadServices();
    showTab("list");
  } catch (error) {
    notifyError(error, "No se pudo eliminar el servicio.");
  }
}

if (document.readyState === "loading") {
  document.addEventListener(
    "DOMContentLoaded",
    () => void initServiciosView(),
    {
      once: true,
    },
  );
} else {
  void initServiciosView();
}
