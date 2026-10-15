import { afterEach, beforeEach, describe, expect, it, vi } from "vitest";
import { readFile } from "node:fs/promises";
import path from "node:path";

const invokeMock = vi.hoisted(() => vi.fn());
const productServiceMocks = vi.hoisted(() => ({
  listProducts: vi.fn(),
  listOutOfStockAndDiscontinued: vi.fn(),
  getProduct: vi.fn(),
  createProduct: vi.fn(),
  updateProduct: vi.fn(),
  previewSellingPrice: vi.fn(),
  discontinueProduct: vi.fn(),
  activateProduct: vi.fn(),
  deleteProduct: vi.fn(),
}));
const categoryMocks = vi.hoisted(() => ({
  listCategories: vi.fn(),
  searchCategories: vi.fn(),
  createCategory: vi.fn(),
}));
const unitMocks = vi.hoisted(() => ({
  searchUnits: vi.fn(),
  openUnitModalForProduct: vi.fn(),
}));
const confirmDialogMock = vi.hoisted(() => vi.fn().mockResolvedValue(true));

describe("servicio de productos", () => {
  beforeEach(() => {
    vi.resetModules();
    vi.clearAllMocks();
    vi.doMock("../services/invoke.js", () => ({ invoke: invokeMock }));
  });

  it("llama los nueve comandos con argumentos camelCase", async () => {
    const service = await import("../services/productsService.js");
    invokeMock.mockResolvedValue({});
    const filters = { search: "cafe", stock: "low" };
    const createInput = {
      name: "Café",
      cost: "12,50",
      quantity: "2,5",
      profitPercent: "20,00",
    };
    const updateInput = {
      productId: 7,
      name: "Café nuevo",
      profitPercent: "25,00",
    };
    const previewInput = { cost: "12,50", profitPercent: "20,00" };

    await service.listProducts(filters);
    await service.listOutOfStockAndDiscontinued("cafe", "outOfStock");
    await service.getProduct(7);
    await service.createProduct(createInput);
    await service.updateProduct(updateInput);
    await service.previewSellingPrice(previewInput);
    await service.discontinueProduct(7);
    await service.activateProduct(7);
    await service.deleteProduct(7);

    expect(invokeMock.mock.calls).toEqual([
      ["list_products", { filters }],
      [
        "list_out_of_stock_and_discontinued",
        { search: "cafe", condition: "outOfStock" },
      ],
      ["get_product", { productId: 7 }],
      ["create_product", { input: createInput }],
      ["update_product", { input: updateInput }],
      ["preview_selling_price", { input: previewInput }],
      ["discontinue_product", { productId: 7 }],
      ["activate_product", { productId: 7 }],
      ["delete_product", { productId: 7 }],
    ]);
  });
});

describe("formulario de productos", () => {
  beforeEach(() => {
    vi.resetModules();
    vi.clearAllMocks();
    document.body.innerHTML = `
      <form id="producto-form">
        <h3 id="producto-form-title"></h3>
        <input id="producto-nombre" maxlength="100" required />
        <input id="producto-id-sistema" readonly />
        <textarea id="producto-descripcion" maxlength="500"></textarea>
        <div id="producto-categoria-combobox"></div>
        <input id="producto-categoria" type="hidden" />
        <div id="producto-unidad-combobox"></div>
        <input id="producto-unidad" type="hidden" />
        <input id="producto-precio-entrada" type="text" inputmode="decimal" required />
        <input id="producto-precio-venta" type="text" inputmode="decimal" readonly />
        <div id="producto-precio-venta-feedback" role="status" hidden></div>
        <input id="producto-ganancia" type="text" inputmode="decimal" required />
        <input id="producto-stock" type="text" inputmode="decimal" required />
        <select id="producto-caducidad-tipo">
          <option value="no-aplica">No aplica</option>
          <option value="fecha">Tiene fecha</option>
        </select>
        <div id="producto-caducidad-fecha-wrap" hidden>
          <input id="producto-caducidad-fecha" type="date" />
        </div>
        <div id="producto-feedback" role="status" hidden></div>
        <button id="producto-cancelar" type="button">Cancelar</button>
        <button type="submit">Guardar</button>
      </form>
      <div id="modal-unidad" class="modal-overlay">
        <form id="unidad-form">
          <input id="modal-unidad-nombre" />
          <input id="modal-unidad-abreviacion" />
          <button type="submit">Guardar Unidad</button>
          <button type="button" id="modal-unidad-cancelar">Cancelar</button>
        </form>
      </div>
    `;

    vi.doMock("../services/productsService.js", () => productServiceMocks);
    vi.doMock("../services/categoriesService.js", () => categoryMocks);
    vi.doMock("../services/unitsService.js", () => ({
      searchUnits: unitMocks.searchUnits,
    }));
    vi.doMock("../views/unidadesView.js", () => ({
      openUnitModalForProduct: unitMocks.openUnitModalForProduct,
    }));
    categoryMocks.searchCategories.mockResolvedValue([]);
    categoryMocks.createCategory.mockResolvedValue({ id: 41, name: "Bebidas" });
    unitMocks.searchUnits.mockResolvedValue([]);
    productServiceMocks.previewSellingPrice.mockResolvedValue({
      sellingPriceCents: 1_500,
    });
    productServiceMocks.createProduct.mockResolvedValue({ id: 5 });
    productServiceMocks.updateProduct.mockResolvedValue({ product: { id: 7 } });
  });

  afterEach(() => {
    document.body.innerHTML = "";
    vi.doUnmock("../services/productsService.js");
    vi.doUnmock("../services/categoriesService.js");
    vi.doUnmock("../services/unitsService.js");
    vi.doUnmock("../views/unidadesView.js");
  });

  async function initializeView() {
    const view = await import("../views/productosView.js");
    await view.initProductosView();
    return view;
  }

  it("mantiene el contrato del formulario y Facturas Asociadas oculta en producción", async () => {
    const html = await readFile(
      path.resolve(process.cwd(), "src", "dashboard-unificado.html"),
      "utf8",
    );
    const parsed = new DOMParser().parseFromString(html, "text/html");

    const form = parsed.getElementById("producto-form");
    const categoryInput = parsed.getElementById("producto-categoria");
    const unitInput = parsed.getElementById("producto-unidad");
    const costInput = parsed.getElementById("producto-precio-entrada");
    const sellingPriceInput = parsed.getElementById("producto-precio-venta");
    const profitInput = parsed.getElementById("producto-ganancia");
    const quantityInput = parsed.getElementById("producto-stock");

    expect(
      parsed.querySelector("#producto-facturas-asociadas[hidden]"),
    ).not.toBeNull();
    expect(form.hasAttribute("onsubmit")).toBe(false);
    expect(parsed.querySelector('[onclick*="irCrearCategoria"]')).toBeNull();
    expect(parsed.querySelector('[onclick*="agregarUnidadRapida"]')).toBeNull();
    expect(parsed.getElementById("producto-nombre").maxLength).toBe(100);
    expect(parsed.getElementById("producto-descripcion").maxLength).toBe(500);
    expect(categoryInput.required).toBe(false);
    expect(unitInput.required).toBe(false);
    expect(costInput.type).toBe("text");
    expect(costInput.inputMode).toBe("decimal");
    expect(costInput.required).toBe(true);
    expect(sellingPriceInput.readOnly).toBe(true);
    expect(sellingPriceInput.required).toBe(false);
    expect(profitInput.type).toBe("text");
    expect(profitInput.inputMode).toBe("decimal");
    expect(profitInput.required).toBe(true);
    expect(profitInput.readOnly).toBe(false);
    expect(quantityInput.type).toBe("text");
    expect(quantityInput.inputMode).toBe("decimal");
    expect(quantityInput.required).toBe(true);
  });

  it("crea una categoría al elegirla y la mantiene seleccionada aunque no se guarde el producto", async () => {
    const view = await initializeView();
    const input = document.querySelector(
      "#producto-categoria-combobox [role=combobox]",
    );
    input.value = "Bebidas";
    input.dispatchEvent(new Event("input", { bubbles: true }));
    await vi.waitFor(() =>
      expect(categoryMocks.searchCategories).toHaveBeenCalledWith("Bebidas"),
    );

    document
      .querySelector("#producto-categoria-combobox [data-combobox-add]")
      .click();
    await vi.waitFor(() =>
      expect(categoryMocks.createCategory).toHaveBeenCalledWith("Bebidas", ""),
    );
    await vi.waitFor(() => {
      expect(document.getElementById("producto-categoria").value).toBe("41");
    });
    expect(document.getElementById("producto-categoria").value).toBe("41");
    expect(input.value).toBe("Bebidas");
    expect(productServiceMocks.createProduct).not.toHaveBeenCalled();
    expect(view).toHaveProperty("openProductForm");
  });

  it("no guarda texto de categoría o unidad sin una opción seleccionada", async () => {
    await initializeView();
    document.getElementById("producto-nombre").value = "Producto válido";
    document.getElementById("producto-precio-entrada").value = "1,00";
    document.getElementById("producto-ganancia").value = "20,00";
    document.getElementById("producto-stock").value = "1";
    const categoryInput = document.querySelector(
      "#producto-categoria-combobox [role=combobox]",
    );
    categoryInput.value = "Sin elegir";
    categoryInput.dispatchEvent(new Event("input", { bubbles: true }));

    document
      .getElementById("producto-form")
      .dispatchEvent(new Event("submit", { bubbles: true, cancelable: true }));
    await vi.waitFor(() => {
      expect(document.getElementById("producto-feedback").textContent).toBe(
        "Elige una opción de la lista o vacía el campo",
      );
    });
    expect(productServiceMocks.createProduct).not.toHaveBeenCalled();
    expect(categoryMocks.createCategory).not.toHaveBeenCalled();
  });

  it("solicita la unidad nueva con el nombre escrito y solo la selecciona al confirmarse", async () => {
    await initializeView();
    const input = document.querySelector(
      "#producto-unidad-combobox [role=combobox]",
    );
    input.value = "Litro";
    input.dispatchEvent(new Event("input", { bubbles: true }));
    await vi.waitFor(() =>
      expect(unitMocks.searchUnits).toHaveBeenCalledWith("Litro"),
    );

    document
      .querySelector("#producto-unidad-combobox [data-combobox-add]")
      .click();
    await vi.waitFor(() =>
      expect(unitMocks.openUnitModalForProduct).toHaveBeenCalled(),
    );
    const [name, , onCancelled] =
      unitMocks.openUnitModalForProduct.mock.calls[0];
    expect(name).toBe("Litro");
    expect(document.getElementById("producto-unidad").value).toBe("");

    onCancelled();
    expect(document.getElementById("producto-unidad").value).toBe("");
    expect(input.value).toBe("Litro");

    input.dispatchEvent(new Event("input", { bubbles: true }));
    await vi.waitFor(() =>
      expect(unitMocks.searchUnits).toHaveBeenCalledTimes(2),
    );
    document
      .querySelector("#producto-unidad-combobox [data-combobox-add]")
      .click();
    const [, onConfirmed] = unitMocks.openUnitModalForProduct.mock.calls[1];
    onConfirmed({ id: 8, name: "Litro", abbreviation: "L" });
    await vi.waitFor(() => {
      expect(document.getElementById("producto-unidad").value).toBe("8");
    });
    expect(input.value).toBe("Litro (L)");
  });

  it("envía texto decimal tal cual, muestra vista previa y excluye costo, cantidad y precio al editar", async () => {
    const view = await initializeView();
    view.openProductForm({
      id: 7,
      name: "Café",
      description: "Molido",
      categoryId: 3,
      categoryName: "Bebidas",
      unitId: 8,
      unitName: "Litro",
      unitAbbreviation: "L",
      costCents: 1_250,
      quantity: "2,5",
      profitPercent: "20,00",
      expiresOn: "2026-10-10",
    });
    const costInput = document.getElementById("producto-precio-entrada");
    const quantityInput = document.getElementById("producto-stock");
    const sellingPriceInput = document.getElementById("producto-precio-venta");
    const profitInput = document.getElementById("producto-ganancia");

    expect(costInput.readOnly).toBe(true);
    expect(quantityInput.readOnly).toBe(true);
    expect(sellingPriceInput.readOnly).toBe(true);
    expect(sellingPriceInput.value).toBeTruthy();
    expect(profitInput.readOnly).toBe(false);
    expect(document.getElementById("producto-categoria").value).toBe("3");
    expect(document.getElementById("producto-unidad").value).toBe("8");
    expect(document.getElementById("producto-caducidad-fecha").min).toMatch(
      /^\d{4}-\d{2}-\d{2}$/,
    );
    expect(document.getElementById("producto-caducidad-fecha").max).toBe(
      "2100-12-31",
    );

    profitInput.value = "25,50";
    profitInput.dispatchEvent(new Event("input", { bubbles: true }));
    await vi.waitFor(() => {
      expect(productServiceMocks.previewSellingPrice).toHaveBeenCalledWith({
        productId: 7,
        profitPercent: "25,50",
      });
    });
    expect(sellingPriceInput.value).toBeTruthy();

    productServiceMocks.updateProduct.mockResolvedValueOnce({
      product: { id: 7 },
      expiredCleanupWarning:
        "No se pudieron limpiar 1 productos vencidos; se reintentará en la próxima operación",
    });
    document
      .getElementById("producto-form")
      .dispatchEvent(new Event("submit", { bubbles: true, cancelable: true }));
    await vi.waitFor(() =>
      expect(productServiceMocks.updateProduct).toHaveBeenCalled(),
    );
    const updateInput = productServiceMocks.updateProduct.mock.calls[0][0];
    expect(updateInput).toEqual({
      productId: 7,
      name: "Café",
      description: "Molido",
      categoryId: 3,
      unitId: 8,
      profitPercent: "25,50",
      expiresOn: "2026-10-10",
    });
    expect(updateInput).not.toHaveProperty("cost");
    expect(updateInput).not.toHaveProperty("quantity");
    expect(updateInput).not.toHaveProperty("sellingPrice");
    expect(document.getElementById("producto-feedback").textContent).toContain(
      "No se pudieron limpiar 1 productos vencidos",
    );
  });

  it("al crear exige cantidad inicial y costo, y envía los textos sin convertirlos", async () => {
    const view = await initializeView();
    view.openProductForm();
    const name = document.getElementById("producto-nombre");
    const cost = document.getElementById("producto-precio-entrada");
    const quantity = document.getElementById("producto-stock");
    const profit = document.getElementById("producto-ganancia");
    expect(name.maxLength).toBe(100);
    expect(document.getElementById("producto-descripcion").maxLength).toBe(500);
    expect(cost.type).toBe("text");
    expect(cost.inputMode).toBe("decimal");
    expect(cost.required).toBe(true);
    expect(quantity.type).toBe("text");
    expect(quantity.inputMode).toBe("decimal");
    expect(quantity.required).toBe(true);
    expect(profit.required).toBe(true);
    expect(
      document.querySelector("#producto-categoria-combobox [role=combobox]")
        .required,
    ).toBe(false);
    expect(
      document.querySelector("#producto-unidad-combobox [role=combobox]")
        .required,
    ).toBe(false);

    name.value = "Producto nuevo";
    cost.value = "12,50";
    quantity.value = "2,500";
    profit.value = "20,00";
    cost.dispatchEvent(new Event("input", { bubbles: true }));
    await vi.waitFor(() => {
      expect(productServiceMocks.previewSellingPrice).toHaveBeenCalledWith({
        cost: "12,50",
        profitPercent: "20,00",
      });
    });
    document
      .getElementById("producto-form")
      .dispatchEvent(new Event("submit", { bubbles: true, cancelable: true }));
    await vi.waitFor(() =>
      expect(productServiceMocks.createProduct).toHaveBeenCalled(),
    );
    expect(productServiceMocks.createProduct).toHaveBeenCalledWith({
      name: "Producto nuevo",
      description: null,
      categoryId: null,
      unitId: null,
      cost: "12,50",
      quantity: "2,500",
      profitPercent: "20,00",
      expiresOn: null,
    });
  });

  it("mantiene la vista previa vacía durante la escritura inválida y muestra el error al salir", async () => {
    productServiceMocks.previewSellingPrice.mockRejectedValue(
      new Error("El precio de entrada no usa separador de miles"),
    );
    await initializeView();
    document.getElementById("producto-precio-entrada").value = "1.234";
    document.getElementById("producto-ganancia").value = "20";
    const cost = document.getElementById("producto-precio-entrada");
    cost.dispatchEvent(new Event("input", { bubbles: true }));
    await vi.waitFor(() =>
      expect(productServiceMocks.previewSellingPrice).toHaveBeenCalled(),
    );

    expect(document.getElementById("producto-precio-venta").value).toBe("");
    expect(
      document.getElementById("producto-precio-venta-feedback").hidden,
    ).toBe(true);

    cost.dispatchEvent(new Event("blur"));
    await vi.waitFor(() => {
      expect(
        document.getElementById("producto-precio-venta-feedback").textContent,
      ).toContain("no usa separador de miles");
    });
  });

  it("muestra en pantalla los errores de duplicado e inexistencia del backend", async () => {
    const view = await initializeView();
    view.openProductForm();
    document.getElementById("producto-nombre").value = "Nombre duplicado";
    document.getElementById("producto-precio-entrada").value = "1,00";
    document.getElementById("producto-ganancia").value = "20,00";
    document.getElementById("producto-stock").value = "1";
    productServiceMocks.createProduct.mockRejectedValueOnce(
      new Error("Ya existe un producto con ese nombre"),
    );
    document
      .getElementById("producto-form")
      .dispatchEvent(new Event("submit", { bubbles: true, cancelable: true }));
    await vi.waitFor(() => {
      expect(document.getElementById("producto-feedback").textContent).toBe(
        "Ya existe un producto con ese nombre",
      );
    });

    view.openProductForm({
      id: 7,
      name: "Producto eliminado",
      costCents: 100,
      quantity: "1",
      profitPercent: "20,00",
    });
    productServiceMocks.updateProduct.mockRejectedValueOnce(
      new Error("El producto ya no existe"),
    );
    document
      .getElementById("producto-form")
      .dispatchEvent(new Event("submit", { bubbles: true, cancelable: true }));
    await vi.waitFor(() => {
      expect(document.getElementById("producto-feedback").textContent).toBe(
        "El producto ya no existe",
      );
    });
  });
});

describe("listados y detalle de productos", () => {
  beforeEach(() => {
    vi.resetModules();
    vi.clearAllMocks();
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
            <input id="producto-nombre" required />
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
          <details id="producto-ver-facturas" hidden></details>
          <button id="producto-ver-historial" hidden>Ver Historial</button>
          <button data-product-detail-action="edit">Editar</button>
          <button data-product-detail-action="discontinue">Descontinuar</button>
          <button data-product-detail-action="activate">Activar</button>
          <button data-product-detail-action="delete">Eliminar</button>
          <button id="producto-ver-volver">Volver</button>
        </div>
      </main>
      <div id="modal-unidad" class="modal-overlay"><form id="unidad-form">
        <input id="modal-unidad-nombre" /><input id="modal-unidad-abreviacion" />
        <button id="modal-unidad-cancelar" type="button">Cancelar</button>
      </form></div>
    `;
    vi.doMock("../services/productsService.js", () => productServiceMocks);
    vi.doMock("../services/categoriesService.js", () => categoryMocks);
    vi.doMock("../services/unitsService.js", () => ({
      searchUnits: unitMocks.searchUnits,
    }));
    vi.doMock("../views/unidadesView.js", () => ({
      openUnitModalForProduct: unitMocks.openUnitModalForProduct,
    }));
    vi.doMock("../components/confirmDialog.js", () => ({
      confirmDialog: confirmDialogMock,
    }));
    categoryMocks.listCategories.mockResolvedValue({
      items: [],
      expiredCleanupWarning: null,
    });
    categoryMocks.searchCategories.mockResolvedValue([]);
    unitMocks.searchUnits.mockResolvedValue([]);
    productServiceMocks.listProducts.mockResolvedValue([]);
    productServiceMocks.listOutOfStockAndDiscontinued.mockResolvedValue({
      items: [],
      expiredCleanupWarning: null,
    });
    productServiceMocks.previewSellingPrice.mockResolvedValue({
      sellingPriceCents: 1_500,
    });
  });

  afterEach(() => {
    document.body.innerHTML = "";
    vi.doUnmock("../services/productsService.js");
    vi.doUnmock("../services/categoriesService.js");
    vi.doUnmock("../services/unitsService.js");
    vi.doUnmock("../views/unidadesView.js");
    vi.doUnmock("../components/confirmDialog.js");
  });

  async function openProductsView() {
    const view = await import("../views/productosView.js");
    await view.initProductosView();
    return view;
  }

  const activeProducts = [
    {
      id: 1,
      name: "<img src=x onerror=alert(1)>",
      categoryId: null,
      categoryName: null,
      unitId: null,
      unitName: null,
      unitAbbreviation: null,
      quantity: "0",
      costCents: 1_000,
      sellingPriceCents: 1_500,
      valorTotalCents: 0,
      profitPercent: "50,00",
      expiresOn: null,
      isOutOfStock: true,
      isLowStock: false,
    },
    {
      id: 2,
      name: "Stock bajo",
      categoryId: 9,
      categoryName: "Bebidas",
      unitId: 3,
      unitName: "Litro",
      unitAbbreviation: "L",
      quantity: "4,999",
      costCents: 2_000,
      sellingPriceCents: 2_500,
      valorTotalCents: 9_998,
      profitPercent: "25,00",
      expiresOn: "2026-10-20",
      isOutOfStock: false,
      isLowStock: true,
    },
  ];

  it("pinta solo productos del backend, badges, acciones y filtra por categoría/stock/caducidad", async () => {
    productServiceMocks.listProducts.mockResolvedValue(activeProducts);
    categoryMocks.listCategories.mockResolvedValue({
      items: [{ id: 9, name: "Bebidas" }],
      expiredCleanupWarning: null,
    });
    const view = await openProductsView();
    await view.showProductoTab("list");

    const body = document.getElementById("productos-body");
    expect(body.querySelectorAll("tr")).toHaveLength(2);
    expect(body.textContent).toContain("#00001");
    expect(body.textContent).toContain("Sin categoría");
    expect(body.textContent).toContain("—");
    expect(body.textContent).toContain("Agotado");
    expect(body.textContent).toContain("Stock Bajo");
    expect(document.getElementById("productos-valor-total").hidden).toBe(false);
    expect(body.textContent).toContain("99,98 USD");
    expect(body.querySelector("img")).toBeNull();
    expect(body.textContent).toContain("<img src=x onerror=alert(1)>");
    expect(body.querySelectorAll("button[data-product-action]")).toHaveLength(
      8,
    );
    expect(productServiceMocks.listProducts).toHaveBeenLastCalledWith({
      search: "",
      categoryId: null,
      withoutCategory: false,
      stock: "all",
      expiry: "all",
    });

    document.getElementById("producto-search").value = "cafe";
    document.getElementById("producto-filtro-categoria").value = "9";
    document.getElementById("producto-filtro-stock").value = "low";
    document.getElementById("producto-filtro-caducidad").value = "expiringSoon";
    for (const id of [
      "producto-search",
      "producto-filtro-categoria",
      "producto-filtro-stock",
      "producto-filtro-caducidad",
    ]) {
      document.getElementById(id).dispatchEvent(
        new Event(id === "producto-search" ? "input" : "change", {
          bubbles: true,
        }),
      );
    }
    await vi.waitFor(() => {
      expect(productServiceMocks.listProducts).toHaveBeenLastCalledWith({
        search: "cafe",
        categoryId: 9,
        withoutCategory: false,
        stock: "low",
        expiry: "expiringSoon",
      });
    });
  });

  it("muestra P-13, distingue ambas condiciones y conserva el aviso de purga", async () => {
    productServiceMocks.listOutOfStockAndDiscontinued.mockResolvedValue({
      items: [
        {
          id: 4,
          name: "Ambos",
          categoryName: "Bebidas",
          unitName: "Litro",
          unitAbbreviation: "L",
          quantity: "0",
          isOutOfStock: true,
          isDiscontinued: true,
          remainingDays: 3,
        },
        {
          id: 5,
          name: "Solo agotado",
          categoryName: null,
          unitName: null,
          unitAbbreviation: null,
          quantity: "0",
          isOutOfStock: true,
          isDiscontinued: false,
          remainingDays: null,
        },
      ],
      expiredCleanupWarning:
        "No se pudieron limpiar 1 productos vencidos; se reintentará en la próxima operación",
    });
    const view = await openProductsView();
    await view.showProductoTab("agotados");

    const rows = document.querySelectorAll("#productos-condiciones-body tr");
    expect(rows).toHaveLength(2);
    expect(rows[0].textContent).toContain("Agotado");
    expect(rows[0].textContent).toContain("Descontinuado");
    expect(rows[0].textContent).toContain("3 días");
    expect(rows[0].textContent).toContain("Activar");
    expect(rows[0].textContent).toContain("Eliminar");
    expect(rows[1].textContent).toContain("Sin categoría");
    expect(rows[1].textContent).toContain("—");
    expect(rows[1].textContent).not.toContain("días");
    expect(
      document.getElementById("producto-condiciones-feedback").textContent,
    ).toContain("No se pudieron limpiar 1 productos vencidos");

    document.getElementById("producto-condiciones-filter").value =
      "discontinued";
    document
      .getElementById("producto-condiciones-filter")
      .dispatchEvent(new Event("change", { bubbles: true }));
    await vi.waitFor(() => {
      expect(
        productServiceMocks.listOutOfStockAndDiscontinued,
      ).toHaveBeenLastCalledWith("", "discontinued");
    });
  });

  it("el detalle activa solo acciones permitidas y confirma pérdida de stock antes de descontinuar/eliminar", async () => {
    productServiceMocks.listProducts.mockResolvedValue([activeProducts[1]]);
    productServiceMocks.getProduct.mockResolvedValue({
      ...activeProducts[1],
      description: "Descripción",
      isDiscontinued: false,
      discontinuedAt: null,
      remainingDays: null,
      expiredCleanupWarning: null,
    });
    productServiceMocks.discontinueProduct.mockResolvedValue({
      expiredCleanupWarning: null,
    });
    productServiceMocks.deleteProduct.mockResolvedValue({
      expiredCleanupWarning: null,
    });
    confirmDialogMock.mockResolvedValue(true);
    const view = await openProductsView();
    await view.showProductoTab("list");
    document
      .querySelector('#productos-body [data-product-action="view"]')
      .click();
    await vi.waitFor(() =>
      expect(productServiceMocks.getProduct).toHaveBeenCalledWith(2),
    );

    expect(document.getElementById("producto-ver-id").textContent).toBe(
      "#00002",
    );
    expect(document.getElementById("producto-ver-categoria").textContent).toBe(
      "Bebidas",
    );
    expect(
      document.getElementById("producto-ver-ganancia").textContent,
    ).toContain("25,00");
    expect(
      document.getElementById("producto-ver-valor-total").textContent,
    ).toBe("99,98 USD");
    expect(
      document
        .getElementById("producto-ver-ganancia")
        .classList.contains("tone-orange"),
    ).toBe(false);
    expect(
      document.querySelector('[data-product-detail-action="edit"]').hidden,
    ).toBe(false);
    expect(
      document.querySelector('[data-product-detail-action="discontinue"]')
        .hidden,
    ).toBe(false);
    expect(
      document.querySelector('[data-product-detail-action="activate"]').hidden,
    ).toBe(true);
    expect(document.getElementById("producto-ver-historial").hidden).toBe(true);
    expect(document.getElementById("producto-ver-facturas").hidden).toBe(true);

    const historyEventHandler = vi.fn();
    document.addEventListener("product:viewHistory", historyEventHandler, {
      once: true,
    });
    const historyButton = document.getElementById("producto-ver-historial");
    historyButton.hidden = false;
    historyButton.click();
    expect(historyEventHandler).toHaveBeenCalledOnce();
    expect(historyEventHandler.mock.calls[0][0]).toBeInstanceOf(CustomEvent);
    expect(historyEventHandler.mock.calls[0][0].detail).toEqual({
      productId: 2,
    });

    document
      .querySelector('[data-product-detail-action="discontinue"]')
      .click();
    await vi.waitFor(() =>
      expect(productServiceMocks.discontinueProduct).toHaveBeenCalledWith(2),
    );
    expect(confirmDialogMock).toHaveBeenCalledWith(
      expect.objectContaining({
        message: expect.stringContaining("se perderá el stock"),
      }),
    );
    document
      .querySelector('#productos-body [data-product-action="delete"]')
      .click();
    await vi.waitFor(() =>
      expect(productServiceMocks.deleteProduct).toHaveBeenCalledWith(2),
    );
    expect(confirmDialogMock).toHaveBeenLastCalledWith(
      expect.objectContaining({
        message: expect.stringContaining("se perderá definitivamente el stock"),
      }),
    );
  });

  it("muestra fecha y días del descontinuado y cambia las acciones al activarlo", async () => {
    productServiceMocks.getProduct
      .mockResolvedValueOnce({
        ...activeProducts[0],
        name: "Producto retirado",
        isDiscontinued: true,
        discontinuedAt: "2026-10-01T10:20:00Z",
        remainingDays: 3,
        description: null,
      })
      .mockResolvedValueOnce({
        ...activeProducts[0],
        name: "Producto retirado",
        isDiscontinued: false,
        discontinuedAt: null,
        remainingDays: null,
        description: null,
      });
    productServiceMocks.activateProduct.mockResolvedValue({
      expiredCleanupWarning: null,
    });
    const view = await openProductsView();
    await window.productosView.openProductDetail(1);

    expect(document.getElementById("producto-ver-estado").textContent).toBe(
      "Descontinuado",
    );
    expect(
      document.getElementById("producto-ver-fecha-descontinuacion").textContent,
    ).toMatch(/^\d{2}\/\d{2}\/2026/);
    expect(
      document.getElementById("producto-ver-dias-restantes").textContent,
    ).toBe("3 días");
    expect(
      document.querySelector('[data-product-detail-action="edit"]').hidden,
    ).toBe(true);
    expect(
      document.querySelector('[data-product-detail-action="discontinue"]')
        .hidden,
    ).toBe(true);
    expect(
      document.querySelector('[data-product-detail-action="activate"]').hidden,
    ).toBe(false);

    document.querySelector('[data-product-detail-action="activate"]').click();
    await vi.waitFor(() =>
      expect(productServiceMocks.activateProduct).toHaveBeenCalledWith(1),
    );
    await vi.waitFor(() =>
      expect(document.getElementById("producto-ver-estado").textContent).toBe(
        "Activo",
      ),
    );
    expect(
      document.querySelector('[data-product-detail-action="edit"]').hidden,
    ).toBe(false);
    expect(
      document.querySelector('[data-product-detail-action="activate"]').hidden,
    ).toBe(true);
    expect(confirmDialogMock).not.toHaveBeenCalled();
  });

  it("muestra errores stale del backend y recarga la lista correspondiente", async () => {
    productServiceMocks.listProducts.mockResolvedValue([activeProducts[1]]);
    productServiceMocks.getProduct.mockRejectedValueOnce(
      new Error("El producto ya no existe"),
    );
    productServiceMocks.discontinueProduct.mockRejectedValueOnce(
      new Error("El producto ya está descontinuado"),
    );
    productServiceMocks.listOutOfStockAndDiscontinued.mockResolvedValue({
      items: [
        {
          id: 2,
          name: "Stock bajo",
          categoryName: "Bebidas",
          unitName: "Litro",
          unitAbbreviation: "L",
          quantity: "4,999",
          isOutOfStock: false,
          isDiscontinued: true,
          remainingDays: 5,
        },
      ],
      expiredCleanupWarning: null,
    });
    productServiceMocks.activateProduct.mockRejectedValueOnce(
      new Error("El producto ya está activo"),
    );
    confirmDialogMock.mockResolvedValue(true);
    const view = await openProductsView();
    await view.showProductoTab("list");
    const listCallsBeforeDetail =
      productServiceMocks.listProducts.mock.calls.length;
    document
      .querySelector('#productos-body [data-product-action="view"]')
      .click();
    await vi.waitFor(() => {
      expect(document.getElementById("productos-feedback").textContent).toBe(
        "El producto ya no existe",
      );
    });
    expect(productServiceMocks.listProducts.mock.calls.length).toBeGreaterThan(
      listCallsBeforeDetail,
    );

    document
      .querySelector('#productos-body [data-product-action="discontinue"]')
      .click();
    await vi.waitFor(() => {
      expect(document.getElementById("productos-feedback").textContent).toBe(
        "El producto ya está descontinuado",
      );
    });

    await view.showProductoTab("agotados");
    document
      .querySelector(
        '#productos-condiciones-body [data-product-action="activate"]',
      )
      .click();
    await vi.waitFor(() => {
      expect(
        document.getElementById("producto-condiciones-feedback").textContent,
      ).toBe("El producto ya está activo");
    });
    expect(
      productServiceMocks.listOutOfStockAndDiscontinued,
    ).toHaveBeenCalledTimes(2);
  });

  it("oculta Historial y los mocks de negocio, y añade P-13 al paginador global", async () => {
    const [html, dashboard, stylesheet] = await Promise.all([
      readFile(
        path.resolve(process.cwd(), "src", "dashboard-unificado.html"),
        "utf8",
      ),
      readFile(
        path.resolve(process.cwd(), "src", "js", "dashboard.js"),
        "utf8",
      ),
      readFile(
        path.resolve(process.cwd(), "src", "css", "dashboard.css"),
        "utf8",
      ),
    ]);
    const parsed = new DOMParser().parseFromString(html, "text/html");
    const productPage = parsed.getElementById("page-productos");
    // Nota: esta aserción es válida solo dentro de spec-02. spec-03 §3.7.2
    // (HP-02) deja visibles la pestaña y la vista de Historial.
    expect(parsed.getElementById("tab-producto-historial").hidden).toBe(false);
    expect(parsed.getElementById("sidebar-facturas").hidden).toBe(true);
    expect(
      parsed.querySelectorAll("[id^='page-facturas-']:not([hidden])"),
    ).toHaveLength(0);
    expect(
      productPage.querySelector("[onclick], [oninput], [onchange], [onsubmit]"),
    ).toBeNull();
    expect(stylesheet).toMatch(
      /\[hidden\]\s*\{\s*display:\s*none\s*!important;/,
    );
    expect(dashboard).not.toMatch(
      /\b(?:productosData|productosDisponibles|nextProductoId|editingProductoId)\b/,
    );
    expect(dashboard).not.toMatch(
      /\b(?:categoriasData|unidadesData|conversionesData|nextCategoriaId|nextUnidadId|nextConversionId|editingCategoriaId|editingUnidadId|editingConversionId)\b/,
    );
    expect(dashboard).not.toMatch(
      /\b(?:poblarSelectHistorialProducto|obtenerMovimientosProducto|agregarLineaVenta|actualizarLineaVenta)\b/,
    );
    expect(dashboard).toMatch(/\{\s*id:\s*"productos-condiciones-body"\s*\}/);
  });

  it("oculta filas fuera de página en los dos tbody de Productos usando el paginador real", async () => {
    const dashboard = await readFile(
      path.resolve(process.cwd(), "src", "js", "dashboard.js"),
      "utf8",
    );
    const paginationStart = dashboard.indexOf("const PAGINACION_OPCIONES");
    const paginationEnd = dashboard.indexOf(
      "// ====== Dashboard: KPIs",
      paginationStart,
    );
    expect(paginationStart).toBeGreaterThanOrEqual(0);
    expect(paginationEnd).toBeGreaterThan(paginationStart);

    document.body.innerHTML = `
      <table><tbody id="productos-body"></tbody></table>
      <table><tbody id="productos-condiciones-body"></tbody></table>
    `;
    for (const bodyId of ["productos-body", "productos-condiciones-body"]) {
      const body = document.getElementById(bodyId);
      for (let index = 0; index < 21; index += 1) {
        const row = document.createElement("tr");
        row.textContent = String(index + 1);
        body.append(row);
      }
    }

    const paginationSource = dashboard.slice(paginationStart, paginationEnd);
    const pagination = new Function(
      "document",
      "window",
      "localStorage",
      "MutationObserver",
      `${paginationSource}\nreturn { PAGINABLES, estadoPaginacion };`,
    )(document, window, window.localStorage, MutationObserver);

    expect(
      pagination.PAGINABLES.some(
        (item) => item.id === "productos-condiciones-body",
      ),
    ).toBe(true);
    for (const bodyId of ["productos-body", "productos-condiciones-body"]) {
      const rows = document.getElementById(bodyId).querySelectorAll("tr");
      expect(rows).toHaveLength(21);
      expect(rows[0].style.display).toBe("");
      expect(rows[19].style.display).toBe("");
      expect(rows[20].style.display).toBe("none");
      expect(document.getElementById(`pager-${bodyId}`).textContent).toContain(
        "Mostrando 1",
      );
    }
  });
});
