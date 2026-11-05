import { attachDraft, registerDraftOpener } from "../utils/drafts.js";
import { confirmDialog } from "../components/confirmDialog.js";
import {
  createCategory,
  deleteCategory,
  getCategory,
  listCategories,
  searchCategories,
  updateCategory,
} from "../services/categoriesService.js";
import {
  clearFieldErrors,
  notifyError,
  notifySuccess,
  reportError,
  requireFields,
} from "../utils/feedback.js";
import { el, messageRow } from "../utils/dom.js";
import { formatMoney } from "../utils/formatters.js";

let categoryDraft = null;
let categories = [];
let editingCategoryId = null;
let initPromise = null;

export function initCategoriasView() {
  if (!initPromise) initPromise = initializeCategoriasView();
  return initPromise;
}

async function initializeCategoriasView() {
  const form = document.getElementById("categoria-form");
  const searchInput = document.getElementById("categoria-search");
  const nameInput = document.getElementById("categoria-nombre");
  const listBody = document.getElementById("categorias-body");
  const listTab = document.getElementById("tab-categoria-list");
  const createTab = document.getElementById("tab-categoria-create");
  const cancelButton = document.querySelector(
    "[data-categoria-action='cancel']",
  );
  const detailBack = document.querySelector(
    "#categoria-ver .cat-view-back[data-categoria-detail-action='back']",
  );
  const detailEdit = document.querySelector(
    "#categoria-ver [data-categoria-detail-action='edit']",
  );
  const detailDelete = document.querySelector(
    "#categoria-ver [data-categoria-detail-action='delete']",
  );

  if (form) {
    form.addEventListener("submit", handleCategorySubmit);
    categoryDraft = attachDraft(form, {
      formKey: "category",
      recordId: () => editingCategoryId,
    });
  }
  if (searchInput) searchInput.addEventListener("input", handleCategorySearch);
  if (nameInput) nameInput.addEventListener("input", handleCategoryNameInput);
  if (listBody) listBody.addEventListener("click", handleCategoryTableClick);
  if (listTab) listTab.addEventListener("click", () => showCategoryTab("list"));
  if (createTab)
    createTab.addEventListener("click", () => showCategoryTab("create"));
  if (detailBack)
    detailBack.addEventListener("click", () => showCategoryTab("list"));
  if (detailEdit) {
    detailEdit.addEventListener("click", () => {
      const currentId = Number(
        document.getElementById("categoria-ver-id")?.dataset.categoryId ?? "",
      );
      if (Number.isFinite(currentId)) openCategoryForm(currentId);
    });
  }
  if (detailDelete) {
    detailDelete.addEventListener("click", () => {
      const currentId = Number(
        document.getElementById("categoria-ver-id")?.dataset.categoryId ?? "",
      );
      if (Number.isFinite(currentId)) void handleDeleteCategory(currentId);
    });
  }
  if (cancelButton) {
    cancelButton.addEventListener("click", () => {
      categoryDraft?.clear();
      resetCategoryForm();
      showCategoryTab("list");
    });
  }

  await loadCategories();
  showCategoryTab("list");
}

async function loadCategories(searchTerm = "") {
  const feedback = document.getElementById("categoria-feedback");
  const body = document.getElementById("categorias-body");

  if (body) {
    messageRow(body, "Cargando categorías…", 5);
  }

  try {
    const response = await listCategories(searchTerm);
    const items = Array.isArray(response) ? response : (response?.items ?? []);
    categories = items;
    renderCategoryList(items);
    if (feedback) {
      feedback.textContent = "";
      feedback.hidden = true;
    }
  } catch (error) {
    if (body) {
      messageRow(body, "No se pudieron cargar las categorías.", 5);
    }
    notifyError(error, "No se pudieron cargar las categorías.");
  }
}

function normalizeCategoryName(value) {
  return String(value ?? "")
    .trim()
    .toLowerCase();
}

function handleCategorySearch(event) {
  const value = event.currentTarget.value.trim();
  loadCategories(value);
}

async function handleCategoryNameInput(event) {
  const value = event.currentTarget.value.trim();
  const feedback = document.getElementById("categoria-feedback");

  if (!value) {
    if (feedback) {
      feedback.textContent = "";
      feedback.hidden = true;
    }
    return;
  }

  try {
    const matches = await searchCategories(value);
    const items = Array.isArray(matches) ? matches : [];
    const exact = items.find(
      (item) =>
        item?.isExactMatch ||
        normalizeCategoryName(item?.name) === normalizeCategoryName(value),
    );

    if (feedback) {
      if (exact) {
        feedback.textContent = "ya existe una categoría con ese nombre.";
        feedback.hidden = false;
      } else if (items.length) {
        feedback.textContent = "Coincidencia de categoría disponible.";
        feedback.hidden = false;
      } else {
        feedback.textContent = "";
        feedback.hidden = true;
      }
    }
  } catch {
    if (feedback) {
      feedback.textContent = "";
      feedback.hidden = true;
    }
  }
}

async function handleCategorySubmit(event) {
  event.preventDefault();
  const form = event.currentTarget;
  const nameInput = form.querySelector("#categoria-nombre");
  const descriptionInput = form.querySelector("#categoria-descripcion");
  const name = nameInput?.value?.trim();
  const description = descriptionInput?.value?.trim();

  clearFieldErrors(form);
  if (!requireFields([nameInput])) {
    notifyError(null, "El nombre es obligatorio.");
    return;
  }

  try {
    if (editingCategoryId !== null) {
      await updateCategory(editingCategoryId, name, description ?? "");
      const successMessage = "Categoría actualizada correctamente.";
      categoryDraft?.clear();
      resetCategoryForm();
      await loadCategories();
      notifySuccess(successMessage);
      showCategoryTab("list");
      return;
    } else {
      await createCategory(name, description ?? "");
      const successMessage = "Categoría creada correctamente.";
      categoryDraft?.clear();
      resetCategoryForm();
      await loadCategories();
      notifySuccess(successMessage);
      showCategoryTab("list");
      return;
    }
  } catch (error) {
    reportError(error, "No se pudo guardar la categoría.", [
      [/descripción/i, descriptionInput],
      [/nombre/i, nameInput],
    ]);
  }
}

function handleCategoryTableClick(event) {
  const viewButton = event.target.closest(
    "button[data-categoria-action='view']",
  );
  if (viewButton) {
    const { categoryId } = viewButton.dataset;
    if (categoryId) openCategoryDetail(Number(categoryId));
    return;
  }

  const editButton = event.target.closest(
    "button[data-categoria-action='edit']",
  );
  if (editButton) {
    const { categoryId } = editButton.dataset;
    if (categoryId) openCategoryForm(Number(categoryId));
    return;
  }

  const deleteButton = event.target.closest(
    "button[data-categoria-action='delete']",
  );
  if (deleteButton) {
    const { categoryId } = deleteButton.dataset;
    if (categoryId) void handleDeleteCategory(Number(categoryId));
  }
}

function renderCategoryList(items) {
  const body = document.getElementById("categorias-body");
  if (!body) return;
  const ingresosHeader = document.getElementById("categorias-ingresos");
  if (ingresosHeader) ingresosHeader.hidden = false;

  if (!items.length) {
    messageRow(body, "Aún no hay categorías.", 5);
    return;
  }

  body.replaceChildren(
    ...items.map((category) =>
      el(
        "tr",
        { dataset: { categoryId: category.id } },
        el("td", {}, String(category.name ?? "")),
        el("td", {}, String(category.description ?? "") || "—"),
        el(
          "td",
          { style: "text-align:center" },
          String(Number(category.productCount ?? 0)),
        ),
        el(
          "td",
          { style: "text-align:right" },
          formatMoney(category.ingresosCents),
        ),
        el(
          "td",
          {},
          el(
            "div",
            { class: "col-acciones" },
            CATEGORY_ACTIONS.map(([action, label, extra]) =>
              el(
                "button",
                {
                  type: "button",
                  class: `action-link${extra}`,
                  dataset: { categoriaAction: action, categoryId: category.id },
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

const CATEGORY_ACTIONS = [
  ["view", "Ver", ""],
  ["edit", "Editar", ""],
  ["delete", "Eliminar", " danger"],
];

function showCategoryTab(tab) {
  if (tab === "create") void categoryDraft?.offer();
  document
    .querySelectorAll("#page-categorias .config-subview")
    .forEach((view) =>
      view.classList.toggle("active", view.id === `categoria-${tab}`),
    );
  document
    .querySelectorAll("#page-categorias .view-tabs .tab")
    .forEach((tabButton) =>
      tabButton.classList.toggle(
        "active",
        tabButton.id === `tab-categoria-${tab}`,
      ),
    );

  const searchBar = document.getElementById("categoria-search-bar");
  if (searchBar) {
    searchBar.style.display = tab === "list" ? "flex" : "none";
  }
}

window.showCategoriaTab = showCategoryTab;

registerDraftOpener("category", async (categoryId) => {
  window.showPage?.(
    "categorias",
    document.querySelector('#sidebar [data-page="categorias"]'),
  );
  await loadCategories();
  if (categoryId == null) showCategoryTab("create");
  else openCategoryForm(categoryId);
});

function resetCategoryForm() {
  editingCategoryId = null;
  const form = document.getElementById("categoria-form");
  if (form) form.reset();
  const title = document.getElementById("categoria-form-title");
  if (title) title.textContent = "Datos de Categoría";
  const feedback = document.getElementById("categoria-feedback");
  if (feedback) {
    feedback.textContent = "";
    feedback.hidden = true;
  }
}

function openCategoryForm(categoryId) {
  const category = categories.find((item) => item.id === categoryId);
  if (!category) return;

  editingCategoryId = category.id;
  const form = document.getElementById("categoria-form");
  const nameInput = document.getElementById("categoria-nombre");
  const descriptionInput = document.getElementById("categoria-descripcion");
  const title = document.getElementById("categoria-form-title");

  if (nameInput) nameInput.value = category.name ?? "";
  if (descriptionInput) descriptionInput.value = category.description ?? "";
  if (title) title.textContent = `Editar categoría: ${category.name ?? ""}`;
  showCategoryTab("create");
}

async function openCategoryDetail(categoryId) {
  try {
    const category = await getCategory(categoryId);
    const name = document.getElementById("categoria-ver-nombre");
    const description = document.getElementById("categoria-ver-descripcion");
    const quantity = document.getElementById("categoria-ver-cantidad");
    const productList = document.getElementById("categoria-ver-productos");
    const idNode = document.getElementById("categoria-ver-id");
    const ingresos = document.getElementById("categoria-ver-ingresos");

    if (idNode) idNode.dataset.categoryId = String(category.id ?? "");
    if (idNode) idNode.textContent = `#${category.id}`;
    if (name) name.textContent = category.name ?? "";
    if (description)
      description.textContent = category.description ?? "Sin descripción";
    if (quantity)
      quantity.textContent = `${Number(category.productCount ?? 0)}`;
    if (ingresos) {
      ingresos.textContent = formatMoney(category.ingresosCents);
      ingresos.hidden = false;
    }

    if (productList) {
      const items = Array.isArray(category.products) ? category.products : [];
      if (!items.length) {
        productList.replaceChildren(
          el("p", {}, "Esta categoría no tiene productos asociados."),
        );
      } else {
        productList.replaceChildren(
          ...items.map((product) =>
            el(
              "div",
              { class: "pv-invoice-card" },
              el(
                "div",
                {},
                el("span", { class: "pv-invoice-label" }, `#${product.id}`),
                el("div", { class: "pv-invoice-value" }, product.name ?? ""),
              ),
            ),
          ),
        );
      }
    }

    showCategoryTab("ver");
  } catch (error) {
    notifyError(error, "No se pudo abrir la categoría.");
  }
}

async function handleDeleteCategory(categoryId) {
  const category = categories.find((item) => item.id === categoryId);
  if (!category) return;

  const confirmed = await confirmDialog({
    title: "Eliminar categoría",
    message: `¿Eliminar la categoría "${category.name}"?\nQuedarán ${Number(category.productCount ?? 0)} producto(s) sin categoría.`,
    confirmText: "Eliminar",
  });

  if (!confirmed) return;

  try {
    const result = await deleteCategory(categoryId);
    await loadCategories();
    notifySuccess(
      `La categoría se eliminó correctamente. ${Number(result?.affectedProducts ?? category.productCount ?? 0)} producto(s) quedaron sin categoría.`,
    );
    showCategoryTab("list");
  } catch (error) {
    notifyError(error, "No se pudo eliminar la categoría.");
  }
}

if (document.readyState === "loading") {
  document.addEventListener(
    "DOMContentLoaded",
    () => void initCategoriasView(),
    {
      once: true,
    },
  );
} else {
  void initCategoriasView();
}
