import { afterEach, beforeEach, describe, expect, it, vi } from "vitest";
import { toastText } from "./helpers/invoiceUi.js";
import { readFile } from "node:fs/promises";
import path from "node:path";

const listCategoriesMock = vi.hoisted(() => vi.fn());
const searchCategoriesMock = vi.hoisted(() => vi.fn());
const getCategoryMock = vi.hoisted(() => vi.fn());
const createCategoryMock = vi.hoisted(() => vi.fn());
const updateCategoryMock = vi.hoisted(() => vi.fn());
const deleteCategoryMock = vi.hoisted(() => vi.fn());
const invokeMock = vi.hoisted(() => vi.fn());
const confirmDialogMock = vi.hoisted(() => vi.fn().mockResolvedValue(true));

vi.mock("../services/invoke.js", () => ({
  invoke: invokeMock,
}));

describe("servicio de categorías", () => {
  beforeEach(() => {
    invokeMock.mockReset();
    listCategoriesMock.mockReset();
    searchCategoriesMock.mockReset();
    getCategoryMock.mockReset();
    createCategoryMock.mockReset();
    updateCategoryMock.mockReset();
    deleteCategoryMock.mockReset();
    vi.resetModules();
  });

  it("llama a los comandos con el nombre snake_case correcto", async () => {
    const service = await import("../services/categoriesService.js");

    invokeMock.mockResolvedValue([]);

    await service.listCategories("lap");
    await service.searchCategories("lap");
    await service.createCategory("Laptops", "Equipo");
    await service.updateCategory(12, "Laptops", "Equipo");
    await service.getCategory(12);
    await service.deleteCategory(12);

    expect(invokeMock).toHaveBeenNthCalledWith(1, "list_categories", {
      search: "lap",
    });
    expect(invokeMock).toHaveBeenNthCalledWith(2, "search_categories", {
      text: "lap",
    });
    expect(invokeMock).toHaveBeenNthCalledWith(3, "create_category", {
      name: "Laptops",
      description: "Equipo",
    });
    expect(invokeMock).toHaveBeenNthCalledWith(4, "update_category", {
      categoryId: 12,
      name: "Laptops",
      description: "Equipo",
    });
    expect(invokeMock).toHaveBeenNthCalledWith(5, "get_category", {
      categoryId: 12,
    });
    expect(invokeMock).toHaveBeenNthCalledWith(6, "delete_category", {
      categoryId: 12,
    });
  });
});

describe("vista de categorías", () => {
  beforeEach(() => {
    vi.resetModules();
    vi.clearAllMocks();
    document.body.innerHTML = `
      <div id="page-categorias">
        <div class="section-toolbar">
          <div class="view-tabs">
            <button class="tab active" id="tab-categoria-list" type="button">Categorías</button>
            <button class="tab" id="tab-categoria-create" type="button">Crear Nueva Categoría</button>
          </div>
          <div id="categoria-search-bar">
            <input id="categoria-search" />
          </div>
        </div>
        <div id="categoria-list" class="config-subview active">
          <table>
            <thead>
              <tr>
                <th>Nombre</th>
                <th>Descripción</th>
                <th>Cantidad de Productos</th>
                <th id="categorias-ingresos" hidden>Ingresos</th>
                <th>Acciones</th>
              </tr>
            </thead>
            <tbody id="categorias-body"></tbody>
          </table>
        </div>
        <div id="categoria-create" class="config-subview">
          <form id="categoria-form">
            <input id="categoria-nombre" name="name" />
            <input id="categoria-descripcion" name="description" />
            <div id="categoria-feedback" role="status"></div>
            <button type="submit">Guardar</button>
          </form>
        </div>
        <div id="categoria-ver" class="config-subview">
          <div id="categoria-ver-id"></div>
          <div id="categoria-ver-nombre"></div>
          <div id="categoria-ver-descripcion"></div>
          <div id="categoria-ver-cantidad"></div>
          <div id="categoria-ver-ingresos"></div>
          <div id="categoria-ver-productos"></div>
        </div>
      </div>
    `;
  });

  afterEach(() => {
    document.body.innerHTML = "";
    delete window.__TAURI__;
  });

  it("mantiene conectados el menú, las pestañas y el módulo en el HTML de producción", async () => {
    const html = await readFile(
      path.resolve(process.cwd(), "src", "dashboard-unificado.html"),
      "utf8",
    );
    const parsed = new DOMParser().parseFromString(html, "text/html");

    expect(
      parsed.querySelector('#sidebar a[data-page="categorias"]'),
    ).not.toBeNull();
    expect(parsed.getElementById("tab-categoria-list")).not.toBeNull();
    expect(parsed.getElementById("tab-categoria-create")).not.toBeNull();
    expect(parsed.getElementById("categorias-body")).not.toBeNull();
    expect(
      parsed.querySelector(
        'script[type="module"][src="./js/views/categoriasView.js"]',
      ),
    ).not.toBeNull();
  });

  it("se inicializa al cargar, muestra el estado vacío y abre el alta", async () => {
    vi.doMock("../services/categoriesService.js", () => ({
      listCategories: listCategoriesMock,
      searchCategories: searchCategoriesMock,
      getCategory: getCategoryMock,
      createCategory: createCategoryMock,
      updateCategory: updateCategoryMock,
      deleteCategory: deleteCategoryMock,
    }));
    listCategoriesMock.mockResolvedValue({
      items: [],
      expiredCleanupWarning: null,
    });

    await import("../views/categoriasView.js");
    document.dispatchEvent(new Event("DOMContentLoaded"));

    await vi.waitFor(() => expect(listCategoriesMock).toHaveBeenCalledOnce());
    expect(document.getElementById("categorias-body").textContent).toContain(
      "Aún no hay categorías.",
    );

    document.getElementById("tab-categoria-create").click();
    expect(
      document.getElementById("categoria-create").classList.contains("active"),
    ).toBe(true);
    expect(
      document.getElementById("categoria-list").classList.contains("active"),
    ).toBe(false);
  });

  it("pinta las filas y el aviso de duplicado del backend", async () => {
    confirmDialogMock.mockResolvedValue(true);
    vi.doMock("../services/categoriesService.js", () => ({
      listCategories: listCategoriesMock,
      searchCategories: searchCategoriesMock,
      getCategory: getCategoryMock,
      createCategory: createCategoryMock,
      updateCategory: updateCategoryMock,
      deleteCategory: deleteCategoryMock,
    }));
    vi.doMock("../components/confirmDialog.js", () => ({
      confirmDialog: confirmDialogMock,
    }));

    window.__TAURI__ = {
      core: {
        invoke: vi.fn().mockResolvedValue({}),
      },
    };
    listCategoriesMock.mockResolvedValue({
      items: [
        {
          id: 1,
          name: "Laptops",
          description: "Equipo",
          productCount: 2,
          ingresosCents: 12_500,
        },
        {
          id: 2,
          name: "Accesorios",
          description: "Periféricos",
          productCount: 0,
          ingresosCents: 0,
        },
      ],
      expiredCleanupWarning: null,
    });
    searchCategoriesMock.mockResolvedValue([
      { id: 1, name: "Laptops", isExactMatch: true },
    ]);

    const view = await import("../views/categoriasView.js");
    await view.initCategoriasView();

    expect(listCategoriesMock).toHaveBeenCalledOnce();
    expect(document.querySelectorAll("#categorias-body tr")).toHaveLength(2);
    expect(document.getElementById("categorias-ingresos").hidden).toBe(false);
    expect(document.getElementById("categorias-body").textContent).toContain(
      "Laptops",
    );
    expect(document.getElementById("categorias-body").textContent).toContain(
      "125,00 USD",
    );

    document.getElementById("categoria-nombre").value = "Laptops";
    document
      .getElementById("categoria-nombre")
      .dispatchEvent(new Event("input", { bubbles: true }));

    await vi.waitFor(() => {
      expect(searchCategoriesMock).toHaveBeenCalledWith("Laptops");
    });
    // Aviso en vivo del formulario: sigue junto al campo, no es un toast.
    expect(document.getElementById("categoria-feedback").textContent).toContain(
      "ya existe",
    );
  });

  it("muestra el detalle y confirma la eliminación con el conteo real", async () => {
    confirmDialogMock.mockResolvedValue(true);
    vi.doMock("../services/categoriesService.js", () => ({
      listCategories: listCategoriesMock,
      searchCategories: searchCategoriesMock,
      getCategory: getCategoryMock,
      createCategory: createCategoryMock,
      updateCategory: updateCategoryMock,
      deleteCategory: deleteCategoryMock,
    }));
    vi.doMock("../components/confirmDialog.js", () => ({
      confirmDialog: confirmDialogMock,
    }));

    window.__TAURI__ = {
      core: {
        invoke: vi.fn().mockResolvedValue({}),
      },
    };
    listCategoriesMock.mockResolvedValue({
      items: [{ id: 7, name: "Hogar", description: "Casa", productCount: 3 }],
      expiredCleanupWarning: null,
    });
    getCategoryMock.mockResolvedValue({
      id: 7,
      name: "Hogar",
      description: "Casa",
      productCount: 3,
      ingresosCents: 8_750,
      products: [
        { id: 11, name: "Silla", isOutOfStock: false, isDiscontinued: false },
      ],
    });
    deleteCategoryMock.mockResolvedValue({ affectedProducts: 3 });

    const view = await import("../views/categoriasView.js");
    await view.initCategoriasView();

    const row = document.querySelector("#categorias-body tr");
    row.querySelector("button[data-categoria-action='view']").click();
    await vi.waitFor(() => {
      expect(document.getElementById("categoria-ver-nombre").textContent).toBe(
        "Hogar",
      );
    });
    expect(
      document.getElementById("categoria-ver-cantidad").textContent,
    ).toContain("3");
    expect(document.getElementById("categoria-ver-ingresos").hidden).toBe(
      false,
    );
    expect(document.getElementById("categoria-ver-ingresos").textContent).toBe(
      "87,50 USD",
    );
    expect(
      document.getElementById("categoria-ver-productos").textContent,
    ).toContain("11");

    row.querySelector("button[data-categoria-action='delete']").click();
    await vi.waitFor(() => {
      expect(deleteCategoryMock).toHaveBeenCalledWith(7);
    });
    expect(toastText()).toContain("3");
  });
});
