import { afterEach, beforeEach, describe, expect, it, vi } from "vitest";
import { toastsOfType } from "./helpers/invoiceUi.js";

vi.mock("../services/productsService.js", () => ({
  activateProduct: vi.fn(),
  createProduct: vi.fn(),
  deleteProduct: vi.fn(),
  discontinueProduct: vi.fn(),
  getProduct: vi.fn(),
  listOutOfStockAndDiscontinued: vi.fn(() =>
    Promise.resolve({ items: [], expiredCleanupWarning: null }),
  ),
  listProducts: vi.fn(),
  previewSellingPrice: vi.fn(() =>
    Promise.resolve({ sellingPriceCents: 1500 }),
  ),
  updateProduct: vi.fn(),
}));

vi.mock("../services/categoriesService.js", () => ({
  createCategory: vi.fn(),
  deleteCategory: vi.fn(),
  getCategory: vi.fn(),
  listCategories: vi.fn(),
  searchCategories: vi.fn(() => Promise.resolve([])),
  updateCategory: vi.fn(),
}));

vi.mock("../services/unitsService.js", () => ({
  searchUnits: vi.fn(() => Promise.resolve([])),
}));

vi.mock("../views/unidadesView.js", () => ({
  openUnitModalForProduct: vi.fn(),
}));

vi.mock("../services/historyService.js", () => ({
  getProductAssociatedInvoices: vi.fn(() => Promise.resolve([])),
}));

vi.mock("../components/confirmDialog.js", () => ({
  confirmDialog: vi.fn(() => Promise.resolve(true)),
}));

import { getProduct, listProducts } from "../services/productsService.js";
import { getCategory, listCategories } from "../services/categoriesService.js";
import { getProductAssociatedInvoices } from "../services/historyService.js";

function seedProductsPage() {
  document.body.innerHTML = `
    <main id="page-productos">
      <div class="view-tabs">
        <button class="tab active" id="tab-producto-list">Productos</button>
        <button class="tab" id="tab-producto-agotados">Agotados y Descontinuados</button>
        <button class="tab" id="tab-producto-create">Crear Nuevo Producto</button>
        <button class="tab" id="tab-producto-historial">Historial</button>
      </div>
      <div id="producto-search-bar">
        <input id="producto-search" placeholder="Buscar producto…" />
        <select id="producto-filtro-categoria"><option value="">Todas las categorías</option></select>
        <select id="producto-filtro-stock">
          <option value="all">Todo el stock</option>
          <option value="available">Disponibles</option>
          <option value="outOfStock">Agotados</option>
          <option value="low">Stock bajo</option>
        </select>
        <select id="producto-filtro-caducidad">
          <option value="all">Todas las caducidades</option>
          <option value="withExpiry">Con caducidad</option>
          <option value="withoutExpiry">Sin caducidad</option>
          <option value="expired">Vencidos</option>
          <option value="expiringSoon">Próximos a vencer</option>
        </select>
      </div>
      <div id="producto-list" class="config-subview active">
        <div id="productos-feedback" role="status" hidden></div>
        <table>
          <thead><tr>
            <th>ID</th><th>Nombre</th><th>Categoría</th><th>Unidad</th>
            <th>Precio de Venta</th><th>Precio de Entrada</th><th>Disponible en Stock</th>
            <th id="productos-valor-total" hidden>Valor Total</th><th>Acciones</th>
          </tr></thead>
          <tbody id="productos-body"></tbody>
        </table>
      </div>
      <div id="producto-agotados" class="config-subview">
        <div id="producto-condiciones-feedback" role="status" hidden></div>
        <input id="producto-condiciones-search" placeholder="Buscar producto…" />
        <select id="producto-condiciones-filter">
          <option value="all">Todos</option>
          <option value="outOfStock">Agotados</option>
          <option value="discontinued">Descontinuados</option>
        </select>
        <table><tbody id="productos-condiciones-body"></tbody></table>
      </div>
      <div id="producto-create" class="config-subview">
        <form id="producto-form">
          <h3 id="producto-form-title"></h3>
          <input id="producto-nombre" />
          <input id="producto-id-sistema" readonly />
          <textarea id="producto-descripcion"></textarea>
          <div id="producto-categoria-combobox"></div><input id="producto-categoria" />
          <div id="producto-unidad-combobox"></div><input id="producto-unidad" />
          <input id="producto-precio-entrada" type="text" />
          <input id="producto-precio-venta" type="text" readonly />
          <div id="producto-precio-venta-feedback" role="status" hidden></div>
          <input id="producto-ganancia" type="text" />
          <input id="producto-stock" type="text" />
          <select id="producto-caducidad-tipo"><option value="no-aplica">No aplica</option><option value="fecha">Fecha</option></select>
          <div id="producto-caducidad-fecha-wrap"><input id="producto-caducidad-fecha" type="date" /></div>
          <div id="producto-facturas-asociadas" hidden><div id="producto-facturas-asociadas-lista"></div></div>
          <div id="producto-feedback" role="status" hidden></div>
          <button id="producto-cancelar" type="button">Cancelar</button>
          <button type="submit">Guardar</button>
        </form>
      </div>
      <div id="producto-ver" class="config-subview">
        <span id="producto-ver-id"></span><h2 id="producto-ver-nombre"></h2>
        <span id="producto-ver-categoria"></span><span id="producto-ver-stock-tag-txt"></span>
        <div id="producto-ver-precio-venta"></div><div id="producto-ver-ganancia" class="kpi-hero-value"></div>
        <div id="producto-ver-valor-total"></div>
        <div id="producto-ver-stock"></div><div id="producto-ver-caducidad"></div>
        <div id="producto-ver-precio-entrada"></div><div id="producto-ver-unidad"></div>
        <p id="producto-ver-descripcion"></p><details id="producto-ver-desc-section"></details>
        <div id="producto-ver-estado"></div><div id="producto-ver-fecha-descontinuacion"></div>
        <div id="producto-ver-dias-restantes"></div><div id="producto-detalle-feedback" role="status" hidden></div>
        <details id="producto-ver-facturas" hidden><div id="producto-ver-facturas-lista"></div></details>
        <button id="producto-ver-historial" hidden>Ver Historial</button>
        <button data-product-detail-action="edit">Editar</button>
        <button data-product-detail-action="discontinue">Descontinuar</button>
        <button data-product-detail-action="activate">Activar</button>
        <button data-product-detail-action="delete">Eliminar</button>
        <button id="producto-ver-volver">Volver</button>
      </div>
    </main>
  `;
}

function seedCategoriesPage() {
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
}

const MALICIOUS_NAME = '<img src=x onerror="alert(1)">Café';

describe("valores de Fase 2 en productos y categorías", () => {
  beforeEach(() => {
    vi.resetModules();
    vi.clearAllMocks();
  });

  afterEach(() => {
    document.body.innerHTML = "";
  });

  it("productosView muestra valorTotalCents en la columna y en el detalle", async () => {
    seedProductsPage();
    listProducts.mockResolvedValue([
      {
        id: 2,
        name: "Café molido",
        categoryId: 9,
        categoryName: "Bebidas",
        unitId: 3,
        unitName: "Paquete",
        unitAbbreviation: "Pq",
        quantity: "4,999",
        costCents: 2000,
        sellingPriceCents: 2500,
        valorTotalCents: 9998,
        profitPercent: "25,00",
        expiresOn: null,
        isOutOfStock: false,
        isLowStock: false,
      },
    ]);
    getProduct.mockResolvedValue({
      id: 2,
      name: "Café molido",
      description: "Molido fino",
      categoryId: 9,
      categoryName: "Bebidas",
      unitId: 3,
      unitName: "Paquete",
      unitAbbreviation: "Pq",
      quantity: "4,999",
      costCents: 2000,
      sellingPriceCents: 2500,
      valorTotalCents: 9998,
      profitPercent: "25,00",
      expiresOn: null,
      isDiscontinued: false,
      discontinuedAt: null,
      remainingDays: null,
      expiredCleanupWarning: null,
    });

    const view = await import("../views/productosView.js");
    await view.initProductosView();
    await view.showProductoTab("list");

    expect(document.getElementById("productos-valor-total").hidden).toBe(false);
    expect(document.getElementById("productos-body").textContent).toContain(
      "99,98 USD",
    );

    document
      .querySelector('#productos-body [data-product-action="view"]')
      .click();
    await vi.waitFor(() => expect(getProduct).toHaveBeenCalledWith(2));
    expect(
      document.getElementById("producto-ver-valor-total").textContent,
    ).toBe("99,98 USD");
  });

  it("el detalle del producto muestra sus Facturas Asociadas con Ver (HP-04)", async () => {
    seedProductsPage();
    const evil = '<img src=x onerror="alert(1)">V-00001';
    listProducts.mockResolvedValue([
      { id: 2, name: "Cable", quantity: "1", isOutOfStock: false },
    ]);
    getProduct.mockResolvedValue({
      id: 2,
      name: "Cable",
      quantity: "1",
      isDiscontinued: false,
      expiredCleanupWarning: null,
    });
    getProductAssociatedInvoices.mockResolvedValueOnce([
      {
        invoiceId: 31,
        invoiceNumber: evil,
        invoiceType: "V",
        invoiceDate: "2026-02-10",
        status: "pending",
        quantity: "2,5",
      },
    ]);
    window.irAFacturaDesdeGeneral = vi.fn();

    const view = await import("../views/productosView.js");
    await view.initProductosView();
    await view.showProductoTab("list");
    document
      .querySelector('#productos-body [data-product-action="view"]')
      .click();
    const list = () => document.getElementById("producto-ver-facturas-lista");
    await vi.waitFor(() =>
      expect(list().querySelector("table")).not.toBeNull(),
    );
    expect(getProductAssociatedInvoices).toHaveBeenCalledWith(2);
    expect(document.getElementById("producto-ver-facturas").hidden).toBe(false);
    const cells = [...list().querySelectorAll("tbody td")].map(
      (cell) => cell.textContent,
    );
    expect(cells.slice(0, 5)).toEqual([
      evil,
      "Venta",
      "10/02/2026",
      "Por Cobrar",
      "2,5",
    ]);
    expect(list().querySelector("img")).toBeNull();
    list().querySelector("button").click();
    expect(window.irAFacturaDesdeGeneral).toHaveBeenCalledWith("V", 31);
    delete window.irAFacturaDesdeGeneral;
  });

  it("al editar el producto el formulario también muestra sus Facturas Asociadas (HP-04)", async () => {
    seedProductsPage();
    listProducts.mockResolvedValue([
      { id: 5, name: "Switch", quantity: "1", isOutOfStock: false },
    ]);
    getProduct.mockResolvedValue({
      id: 5,
      name: "Switch",
      quantity: "1",
      isDiscontinued: false,
      expiredCleanupWarning: null,
    });
    getProductAssociatedInvoices.mockResolvedValueOnce([
      {
        invoiceId: 8,
        invoiceNumber: "E-00008",
        invoiceType: "E",
        invoiceDate: "2026-02-10",
        status: "completed",
        quantity: "4",
      },
    ]);
    const view = await import("../views/productosView.js");
    await view.initProductosView();
    await view.showProductoTab("list");
    // Al crear, la sección no aparece.
    expect(document.getElementById("producto-facturas-asociadas").hidden).toBe(
      true,
    );
    document
      .querySelector('#productos-body [data-product-action="edit"]')
      .click();
    const list = () =>
      document.getElementById("producto-facturas-asociadas-lista");
    await vi.waitFor(() => expect(list().textContent).toContain("E-00008"));
    expect(getProductAssociatedInvoices).toHaveBeenCalledWith(5);
    expect(document.getElementById("producto-facturas-asociadas").hidden).toBe(
      false,
    );
    expect(list().textContent).toContain("Pagada");
  });

  it("sin facturas asociadas muestra un estado vacío claro", async () => {
    seedProductsPage();
    listProducts.mockResolvedValue([
      { id: 3, name: "Router", quantity: "1", isOutOfStock: false },
    ]);
    getProduct.mockResolvedValue({
      id: 3,
      name: "Router",
      quantity: "1",
      isDiscontinued: false,
      expiredCleanupWarning: null,
    });
    getProductAssociatedInvoices.mockResolvedValueOnce([]);
    const view = await import("../views/productosView.js");
    await view.initProductosView();
    await view.showProductoTab("list");
    document
      .querySelector('#productos-body [data-product-action="view"]')
      .click();
    await vi.waitFor(() =>
      expect(
        document.getElementById("producto-ver-facturas-lista").textContent,
      ).toBe("Este producto aún no aparece en ninguna factura."),
    );
  });

  it("categoriasView muestra ingresosCents en la columna y en el detalle (0 sin ventas)", async () => {
    seedCategoriesPage();
    listCategories.mockResolvedValue({
      items: [
        {
          id: 1,
          name: "Bebidas",
          description: "Para tomar",
          productCount: 2,
          ingresosCents: 0,
        },
      ],
      expiredCleanupWarning: null,
    });
    getCategory.mockResolvedValue({
      id: 1,
      name: "Bebidas",
      description: "Para tomar",
      productCount: 2,
      ingresosCents: 0,
      products: [],
    });

    const view = await import("../views/categoriasView.js");
    await view.initCategoriasView();

    expect(document.getElementById("categorias-ingresos").hidden).toBe(false);
    // En Fase 2 no hay ventas: los ingresos son 0.
    expect(document.getElementById("categorias-body").textContent).toContain(
      "0,00 USD",
    );

    document
      .querySelector("#categorias-body button[data-categoria-action='view']")
      .click();
    await vi.waitFor(() =>
      expect(document.getElementById("categoria-ver-nombre").textContent).toBe(
        "Bebidas",
      ),
    );
    expect(document.getElementById("categoria-ver-ingresos").textContent).toBe(
      "0,00 USD",
    );
  });

  it("Fase 3: Ingresos y Valor Total pintan por fila los importes reales del backend", async () => {
    const { formatMoney } = await import("../utils/formatters.js");
    seedCategoriesPage();
    listCategories.mockResolvedValue({
      items: [
        {
          id: 1,
          name: "Bebidas",
          description: null,
          productCount: 2,
          ingresosCents: 1_234_567,
        },
        {
          id: 2,
          name: "Lácteos",
          description: null,
          productCount: 1,
          ingresosCents: 39_900,
        },
      ],
      expiredCleanupWarning: null,
    });
    getCategory.mockResolvedValue({
      id: 1,
      name: "Bebidas",
      description: null,
      productCount: 2,
      ingresosCents: 1_234_567,
      products: [],
    });

    const categories = await import("../views/categoriasView.js");
    await categories.initCategoriasView();
    const rows = Array.from(document.querySelectorAll("#categorias-body tr"));
    expect(rows[0].textContent).toContain(formatMoney(1_234_567));
    expect(rows[1].textContent).toContain(formatMoney(39_900));
    expect(rows[1].textContent).not.toContain(formatMoney(1_234_567));
    document
      .querySelector("#categorias-body button[data-categoria-action='view']")
      .click();
    await vi.waitFor(() =>
      expect(
        document.getElementById("categoria-ver-ingresos").textContent,
      ).toBe(formatMoney(1_234_567)),
    );

    seedProductsPage();
    const product = {
      id: 4,
      name: "Café SP03",
      categoryId: 2,
      categoryName: "Lácteos",
      unitId: null,
      unitName: null,
      unitAbbreviation: null,
      quantity: "12",
      costCents: 133,
      sellingPriceCents: 160,
      valorTotalCents: 1_700,
      profitPercent: "20,00",
      expiresOn: null,
      isOutOfStock: false,
      isLowStock: false,
    };
    listProducts.mockResolvedValue([product]);
    getProduct.mockResolvedValue({
      ...product,
      description: null,
      isDiscontinued: false,
      discontinuedAt: null,
      remainingDays: null,
      expiredCleanupWarning: null,
    });
    const products = await import("../views/productosView.js");
    await products.initProductosView();
    await products.showProductoTab("list");
    // Valor Total = lotes restantes × costo (no cantidad × costo promedio).
    expect(document.getElementById("productos-body").textContent).toContain(
      formatMoney(1_700),
    );
    document
      .querySelector('#productos-body [data-product-action="view"]')
      .click();
    await vi.waitFor(() =>
      expect(
        document.getElementById("producto-ver-valor-total").textContent,
      ).toBe(formatMoney(1_700)),
    );
  });

  it.each([
    ["null", { expiredCleanupWarning: null }],
    ["ausente", {}],
  ])(
    "H-7: productos y categorías no se rompen con expiredCleanupWarning %s",
    async (_label, warningField) => {
      seedCategoriesPage();
      listCategories.mockResolvedValue({
        items: [
          {
            id: 1,
            name: "Bebidas",
            description: null,
            productCount: 0,
            ingresosCents: 0,
          },
        ],
        ...warningField,
      });
      getCategory.mockResolvedValue({
        id: 1,
        name: "Bebidas",
        description: null,
        productCount: 0,
        ingresosCents: 0,
        products: [],
        ...warningField,
      });
      const categories = await import("../views/categoriasView.js");
      await categories.initCategoriasView();
      expect(document.getElementById("categorias-body").textContent).toContain(
        "Bebidas",
      );
      document
        .querySelector("#categorias-body button[data-categoria-action='view']")
        .click();
      await vi.waitFor(() =>
        expect(
          document.getElementById("categoria-ver-nombre").textContent,
        ).toBe("Bebidas"),
      );

      seedProductsPage();
      const product = {
        id: 4,
        name: "Café",
        categoryId: null,
        categoryName: null,
        unitId: null,
        unitName: null,
        unitAbbreviation: null,
        quantity: "1",
        costCents: 100,
        sellingPriceCents: 120,
        valorTotalCents: 100,
        profitPercent: "20,00",
        expiresOn: null,
        isOutOfStock: false,
        isLowStock: false,
      };
      listProducts.mockResolvedValue([product]);
      getProduct.mockResolvedValue({
        ...product,
        description: null,
        isDiscontinued: false,
        discontinuedAt: null,
        remainingDays: null,
        ...warningField,
      });
      const products = await import("../views/productosView.js");
      await products.initProductosView();
      await products.showProductoTab("list");
      document
        .querySelector('#productos-body [data-product-action="view"]')
        .click();
      await vi.waitFor(() =>
        expect(document.getElementById("producto-ver-nombre").textContent).toBe(
          "Café",
        ),
      );
      // Sin aviso no aparece ningún toast (ni uno con "undefined").
      expect(toastsOfType("warning")).toHaveLength(0);
    },
  );

  it("salida segura en productos: un nombre con HTML se muestra como texto en listado y detalle", async () => {
    seedProductsPage();
    const malicious = {
      id: 5,
      name: MALICIOUS_NAME,
      categoryId: null,
      categoryName: null,
      unitId: null,
      unitName: null,
      unitAbbreviation: null,
      quantity: "1",
      costCents: 1000,
      sellingPriceCents: 1500,
      valorTotalCents: 1000,
      profitPercent: "50,00",
      expiresOn: null,
      isOutOfStock: false,
      isLowStock: false,
    };
    listProducts.mockResolvedValue([malicious]);
    getProduct.mockResolvedValue({
      ...malicious,
      description: MALICIOUS_NAME,
      isDiscontinued: false,
      discontinuedAt: null,
      remainingDays: null,
      expiredCleanupWarning: null,
    });

    const view = await import("../views/productosView.js");
    await view.initProductosView();
    await view.showProductoTab("list");

    const body = document.getElementById("productos-body");
    expect(body.querySelector("img")).toBeNull();
    expect(body.textContent).toContain(MALICIOUS_NAME);

    document
      .querySelector('#productos-body [data-product-action="view"]')
      .click();
    await vi.waitFor(() => expect(getProduct).toHaveBeenCalledWith(5));
    const detail = document.getElementById("producto-ver");
    expect(detail.querySelector("img")).toBeNull();
    expect(
      document.getElementById("producto-ver-nombre").textContent,
    ).toContain(MALICIOUS_NAME);
  });

  it("salida segura en categorías: un nombre con HTML se muestra como texto en listado y detalle", async () => {
    seedCategoriesPage();
    listCategories.mockResolvedValue({
      items: [
        {
          id: 3,
          name: MALICIOUS_NAME,
          description: MALICIOUS_NAME,
          productCount: 1,
          ingresosCents: 0,
        },
      ],
      expiredCleanupWarning: null,
    });
    getCategory.mockResolvedValue({
      id: 3,
      name: MALICIOUS_NAME,
      description: MALICIOUS_NAME,
      productCount: 1,
      ingresosCents: 0,
      products: [{ id: 9, name: MALICIOUS_NAME }],
    });

    await import("../views/categoriasView.js");
    const { initCategoriasView } = await import("../views/categoriasView.js");
    await initCategoriasView();

    const body = document.getElementById("categorias-body");
    expect(body.querySelector("img")).toBeNull();
    expect(body.textContent).toContain("Café");

    document
      .querySelector("#categorias-body button[data-categoria-action='view']")
      .click();
    await vi.waitFor(() =>
      expect(
        document.getElementById("categoria-ver-nombre").textContent,
      ).toContain(MALICIOUS_NAME),
    );
    const detail = document.getElementById("categoria-ver");
    expect(detail.querySelector("img")).toBeNull();
  });
});
