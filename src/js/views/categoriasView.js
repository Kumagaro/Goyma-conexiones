import { confirmDialog } from "../components/confirmDialog.js";
import {
  createCategory,
  deleteCategory,
  getCategory,
  listCategories,
  searchCategories,
  updateCategory,
} from "../services/categoriesService.js";
import { formatMoney } from "../utils/formatters.js";

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

  if (form) form.addEventListener("submit", handleCategorySubmit);
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
    body.innerHTML =
      '<tr><td colspan="5" class="empty-row">Cargando categorías…</td></tr>';
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
      body.innerHTML = `
        <tr>
          <td colspan="5" class="empty-row">No se pudieron cargar las categorías.</td>
        </tr>
      `;
    }
    if (feedback) {
      feedback.textContent =
        error?.message || "No se pudieron cargar las categorías.";
      feedback.hidden = false;
    }
  }
}

function normalizeCategoryName(value) {
  return String(value ?? "")
    .trim()
    .toLowerCase();
}

function normalizeCategoryFeedbackMessage(message) {
  const text = String(message ?? "").trim();
  if (!text) return "";
  if (/ya existe una categoría con ese nombre/i.test(text)) {
    return "ya existe una categoría con ese nombre.";
  }
  return text;
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

  if (!name) {
    showCategoryFeedback("El nombre es obligatorio.");
    return;
  }

  try {
    if (editingCategoryId !== null) {
      await updateCategory(editingCategoryId, name, description ?? "");
      const successMessage = "Categoría actualizada correctamente.";
      resetCategoryForm();
      await loadCategories();
      showCategoryFeedback(successMessage);
      showCategoryTab("list");
      return;
    } else {
      await createCategory(name, description ?? "");
      const successMessage = "Categoría creada correctamente.";
      resetCategoryForm();
      await loadCategories();
      showCategoryFeedback(successMessage);
      showCategoryTab("list");
      return;
    }
  } catch (error) {
    showCategoryFeedback(
      normalizeCategoryFeedbackMessage(
        error?.message || "No se pudo guardar la categoría.",
      ),
    );
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
    body.innerHTML =
      '<tr><td colspan="5" class="empty-row">Aún no hay categorías.</td></tr>';
    return;
  }

  body.innerHTML = items
    .map(
      (category) => `
        <tr data-category-id="${category.id}">
          <td>${escapeHtml(String(category.name ?? ""))}</td>
          <td>${escapeHtml(String(category.description ?? "")) || "—"}</td>
          <td style="text-align:center">${Number(category.productCount ?? 0)}</td>
          <td style="text-align:right">${formatMoney(category.ingresosCents)}</td>
          <td>
            <div class="col-acciones">
              <button type="button" class="action-link" data-categoria-action="view" data-category-id="${category.id}">Ver</button>
              <button type="button" class="action-link" data-categoria-action="edit" data-category-id="${category.id}">Editar</button>
              <button type="button" class="action-link danger" data-categoria-action="delete" data-category-id="${category.id}">Eliminar</button>
            </div>
          </td>
        </tr>
      `,
    )
    .join("");
}

function showCategoryFeedback(message) {
  const feedback = document.getElementById("categoria-feedback");
  if (!feedback) return;
  feedback.textContent = normalizeCategoryFeedbackMessage(message);
  feedback.hidden = false;
}

function showCategoryTab(tab) {
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
        productList.innerHTML =
          "<p>Esta categoría no tiene productos asociados.</p>";
      } else {
        productList.innerHTML = items
          .map(
            (product) => `
              <div class="pv-invoice-card">
                <div>
                  <span class="pv-invoice-label">#${product.id}</span>
                  <div class="pv-invoice-value">${escapeHtml(product.name ?? "")}</div>
                </div>
              </div>
            `,
          )
          .join("");
      }
    }

    showCategoryTab("ver");
  } catch (error) {
    showCategoryFeedback(error?.message || "No se pudo abrir la categoría.");
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
    showCategoryFeedback(
      `La categoría se eliminó correctamente. ${Number(result?.affectedProducts ?? category.productCount ?? 0)} producto(s) quedaron sin categoría.`,
    );
    showCategoryTab("list");
  } catch (error) {
    showCategoryFeedback(error?.message || "No se pudo eliminar la categoría.");
  }
}

function escapeHtml(value) {
  return String(value)
    .replaceAll("&", "&amp;")
    .replaceAll("<", "&lt;")
    .replaceAll(">", "&gt;")
    .replaceAll('"', "&quot;")
    .replaceAll("'", "&#39;");
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
