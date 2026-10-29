import {
  afterEach,
  beforeAll,
  beforeEach,
  describe,
  expect,
  it,
  vi,
} from "vitest";
import { readFile } from "node:fs/promises";
import path from "node:path";

vi.mock("../services/historyService.js", () => ({
  getProductHistory: vi.fn(),
  listHistoryProducts: vi.fn(),
  exportHistoryExcel: vi.fn(),
  exportHistoryPdf: vi.fn(),
}));

import {
  exportHistoryExcel,
  exportHistoryPdf,
  getProductHistory,
  listHistoryProducts,
} from "../services/historyService.js";
import { formatTimestamp } from "../utils/formatters.js";
import { __testHooks, initHistorialView } from "../views/historialView.js";

function seedHistoryPage() {
  document.body.innerHTML = `
    <main>
      <section id="page-productos" class="page active">
        <div class="view-tabs">
          <button id="tab-producto-list" class="tab active">Productos</button>
          <button id="tab-producto-historial" class="tab" hidden>Historial</button>
        </div>
        <div id="producto-historial" class="config-subview" hidden>
          <div class="historial-toolbar">
            <select id="historial-producto-select"></select>
            <input type="date" id="historial-producto-desde" />
            <input type="date" id="historial-producto-hasta" />
            <select id="historial-producto-tipo"></select>
          </div>
          <table><tbody id="historial-producto-body"></tbody></table>
        </div>
        <div id="producto-ver" class="config-subview">
          <button id="producto-ver-historial" type="button" hidden>Ver Historial</button>
        </div>
      </section>
    </main>
  `;
}

const ACTIVE_PRODUCTS = [
  { id: 2, name: "Café molido", isDeleted: false },
  { id: 1, name: "Azúcar", isDeleted: false },
];

const DELETED_PRODUCTS = [{ id: 12, name: "Cable UTP", isDeleted: true }];

function mockEmptyHistory() {
  listHistoryProducts.mockResolvedValue([...ACTIVE_PRODUCTS]);
  getProductHistory.mockResolvedValue({ items: [] });
}

describe("historialView", () => {
  beforeEach(() => {
    seedHistoryPage();
    vi.clearAllMocks();
    delete window.showProductoTab;
    delete window.__historialPageChangeBound;
    delete window.__historialViewHistoryBound;
  });

  afterEach(() => {
    document.body.innerHTML = "";
  });

  it("deja visibles la pestaña, la vista y el botón Ver Historial", async () => {
    mockEmptyHistory();
    await initHistorialView();

    expect(
      document.getElementById("tab-producto-historial").hasAttribute("hidden"),
    ).toBe(false);
    expect(
      document.getElementById("producto-historial").hasAttribute("hidden"),
    ).toBe(false);
    expect(
      document.getElementById("producto-ver-historial").hasAttribute("hidden"),
    ).toBe(false);
  });

  it("puebla el selector con dos optgroup e identifica a los eliminados", async () => {
    listHistoryProducts.mockResolvedValue([
      ...ACTIVE_PRODUCTS,
      ...DELETED_PRODUCTS,
    ]);
    getProductHistory.mockResolvedValue({ items: [] });
    await initHistorialView();

    const select = document.getElementById("historial-producto-select");
    const groups = select.querySelectorAll("optgroup");
    expect(groups).toHaveLength(2);
    expect(groups[0].getAttribute("label")).toMatch(/activo/i);
    expect(groups[1].getAttribute("label")).toMatch(/eliminado/i);
    expect(groups[0].querySelectorAll("option")).toHaveLength(2);
    const deletedOption = groups[1].querySelector("option");
    expect(deletedOption.textContent).toBe("Cable UTP (eliminado, #00012)");
    expect(deletedOption.value).toBe("12");
  });

  it("el filtro de tipos es múltiple con tres optgroup y valores de código", async () => {
    mockEmptyHistory();
    await initHistorialView();

    const tipo = document.getElementById("historial-producto-tipo");
    expect(tipo.multiple).toBe(true);
    const groups = tipo.querySelectorAll("optgroup");
    expect(groups).toHaveLength(3);
    const values = Array.from(tipo.querySelectorAll("option")).map(
      (o) => o.value,
    );
    expect(values).toEqual([
      "initial",
      "entry",
      "saleOut",
      "serviceOut",
      "warrantyOut",
      "supplyOut",
      "loss",
      "adjustment",
      "reversal",
    ]);
    expect(Array.from(tipo.selectedOptions).map((o) => o.value)).toEqual([]);
  });

  it("pinta los movimientos del backend con textContent", async () => {
    listHistoryProducts.mockResolvedValue([...ACTIVE_PRODUCTS]);
    getProductHistory.mockResolvedValue({
      items: [
        {
          id: 1,
          createdAt: "2026-02-10T15:30:00Z",
          type: "entry",
          quantity: "10",
          stockAfter: "10",
          unitPriceCents: 100,
          totalPriceCents: 1000,
          invoiceNumber: "E-00001",
          invoiceType: "E",
          userName: "Ana",
          productIdOriginal: 2,
        },
      ],
    });
    await initHistorialView();
    document.getElementById("historial-producto-select").value = "2";
    document
      .getElementById("historial-producto-select")
      .dispatchEvent(new Event("change", { bubbles: true }));

    await vi.waitFor(() => {
      expect(getProductHistory).toHaveBeenCalledWith(
        expect.objectContaining({ productId: 2, types: [] }),
      );
    });
    const body = document.getElementById("historial-producto-body");
    expect(body.querySelectorAll("tr")).toHaveLength(1);
    expect(body.textContent).toContain("E-00001");
    expect(body.textContent).toContain("Ana");
  });

  it("HP-02: el número abre la factura si aún existe; si se eliminó queda como texto", async () => {
    const movement = {
      createdAt: "2026-02-10T15:30:00Z",
      type: "saleOut",
      quantity: "-1",
      stockAfter: "9",
      unitPriceCents: 100,
      totalPriceCents: 100,
      userName: "Ana",
      productIdOriginal: 2,
    };
    listHistoryProducts.mockResolvedValue([...ACTIVE_PRODUCTS]);
    getProductHistory.mockResolvedValue({
      items: [
        {
          ...movement,
          id: 1,
          invoiceNumber: "V-00004",
          invoiceType: "V",
          invoiceId: 44,
        },
        {
          ...movement,
          id: 2,
          invoiceNumber: "V-00003",
          invoiceType: "V",
          invoiceId: null,
        },
      ],
    });
    window.irAFacturaDesdeGeneral = vi.fn();
    await initHistorialView();
    const select = document.getElementById("historial-producto-select");
    select.value = "2";
    select.dispatchEvent(new Event("change", { bubbles: true }));

    const body = document.getElementById("historial-producto-body");
    await vi.waitFor(() => expect(body.querySelectorAll("tr")).toHaveLength(2));
    const [existing, deleted] = body.querySelectorAll("tr");
    const link = existing.querySelector("button");
    expect(link.textContent).toBe("V-00004");
    link.click();
    expect(window.irAFacturaDesdeGeneral).toHaveBeenCalledWith("V", 44);
    expect(deleted.querySelector("button")).toBeNull();
    expect(deleted.textContent).toContain("V-00003");
    delete window.irAFacturaDesdeGeneral;
  });

  it("escucha product:viewHistory y abre la pestaña con el producto preseleccionado", async () => {
    listHistoryProducts.mockResolvedValue([...ACTIVE_PRODUCTS]);
    getProductHistory.mockResolvedValue({ items: [] });
    window.showProductoTab = vi.fn();
    await initHistorialView();
    vi.clearAllMocks();

    document.dispatchEvent(
      new CustomEvent("product:viewHistory", { detail: { productId: 2 } }),
    );

    await vi.waitFor(() => {
      expect(document.getElementById("historial-producto-select").value).toBe(
        "2",
      );
    });
    expect(window.showProductoTab).toHaveBeenCalledWith(
      "historial",
      document.getElementById("tab-producto-historial"),
    );
    expect(getProductHistory).toHaveBeenCalledWith(
      expect.objectContaining({ productId: 2 }),
    );
  });

  it("recarga el historial al entrar a la pestaña", async () => {
    mockEmptyHistory();
    await initHistorialView();
    const calls = getProductHistory.mock.calls.length;

    document.getElementById("tab-producto-historial").click();

    await vi.waitFor(() => {
      expect(getProductHistory.mock.calls.length).toBeGreaterThan(calls);
    });
  });

  it("muestra las fechas en hora local con formatTimestamp", async () => {
    listHistoryProducts.mockResolvedValue([...ACTIVE_PRODUCTS]);
    const createdAt = "2026-02-10T15:30:00Z";
    getProductHistory.mockResolvedValue({
      items: [
        {
          id: 1,
          createdAt,
          type: "entry",
          quantity: "10",
          stockAfter: "10",
          unitPriceCents: 100,
          totalPriceCents: 1000,
          invoiceNumber: null,
          invoiceType: null,
          userName: "Ana",
          productIdOriginal: 2,
        },
      ],
    });
    await initHistorialView();

    const firstRow = document.querySelector("#historial-producto-body tr");
    expect(firstRow.cells[0].textContent).toBe(formatTimestamp(createdAt));
  });

  it("muestra un nombre con HTML como texto sin crear elementos", async () => {
    listHistoryProducts.mockResolvedValue([
      { id: 3, name: '<img src=x onerror="alert(1)">Café', isDeleted: false },
    ]);
    getProductHistory.mockResolvedValue({
      items: [
        {
          id: 1,
          createdAt: "2026-02-10T15:30:00Z",
          type: "entry",
          quantity: "10",
          stockAfter: "10",
          unitPriceCents: 100,
          totalPriceCents: 1000,
          invoiceNumber: null,
          invoiceType: null,
          userName: '<img src=x onerror="alert(1)">Ana',
          productIdOriginal: 3,
        },
      ],
    });
    await initHistorialView();

    const select = document.getElementById("historial-producto-select");
    expect(select.textContent).toContain("Café");
    expect(select.querySelector("img")).toBeNull();
    const body = document.getElementById("historial-producto-body");
    expect(body.textContent).toContain("Ana");
    expect(body.querySelector("img")).toBeNull();
  });
});

describe("historialView exportación (HTML de producción)", () => {
  let productsPage = "";

  const MOVEMENT = {
    id: 1,
    createdAt: "2026-02-10T15:30:00Z",
    type: "entry",
    quantity: "10",
    stockAfter: "10",
    unitPriceCents: 100,
    totalPriceCents: 1000,
    invoiceNumber: "E-00001",
    invoiceType: "E",
    userName: "Ana",
    productIdOriginal: 2,
  };

  beforeAll(async () => {
    const html = await readFile(
      path.resolve(process.cwd(), "src", "dashboard-unificado.html"),
      "utf8",
    );
    productsPage = new DOMParser()
      .parseFromString(html, "text/html")
      .getElementById("page-productos").outerHTML;
  });

  beforeEach(() => {
    document.body.innerHTML = `<main>${productsPage}</main>`;
    vi.clearAllMocks();
    delete window.__historialPageChangeBound;
    delete window.__historialViewHistoryBound;
    listHistoryProducts.mockResolvedValue([...ACTIVE_PRODUCTS]);
  });

  afterEach(() => {
    document.body.innerHTML = "";
    delete window.__TAURI__;
  });

  async function openWithProduct() {
    await initHistorialView();
    const select = document.getElementById("historial-producto-select");
    select.value = "2";
    select.dispatchEvent(new Event("change", { bubbles: true }));
    await vi.waitFor(() =>
      expect(getProductHistory).toHaveBeenCalledWith(
        expect.objectContaining({ productId: 2 }),
      ),
    );
  }

  it("los botones están en .historial-toolbar y se deshabilitan sin movimientos, con title y aria-live", async () => {
    getProductHistory.mockResolvedValue({ items: [] });
    await openWithProduct();

    const toolbar = document.querySelector(
      "#producto-historial .historial-toolbar",
    );
    const status = document.getElementById("historial-exportar-estado");
    expect(status.getAttribute("aria-live")).toBe("polite");
    for (const id of ["historial-exportar-excel", "historial-exportar-pdf"]) {
      const button = document.getElementById(id);
      expect(toolbar.contains(button), id).toBe(true);
      await vi.waitFor(() => expect(button.disabled).toBe(true));
      expect(button.title).toBe("No hay movimientos para exportar");
    }
    expect(status.textContent).toBe("No hay movimientos para exportar");
  });

  it("con movimientos los botones se habilitan", async () => {
    getProductHistory.mockResolvedValue({ items: [MOVEMENT] });
    await openWithProduct();
    const excel = document.getElementById("historial-exportar-excel");
    await vi.waitFor(() => expect(excel.disabled).toBe(false));
    expect(document.getElementById("historial-exportar-pdf").disabled).toBe(
      false,
    );
    expect(excel.hasAttribute("title")).toBe(false);
  });

  it.each([
    ["historial-exportar-excel", "xlsx", () => exportHistoryExcel],
    ["historial-exportar-pdf", "pdf", () => exportHistoryPdf],
  ])(
    "%s abre el diálogo nativo y exporta con la ruta y los filtros activos",
    async (buttonId, extension, exporterRef) => {
      getProductHistory.mockResolvedValue({ items: [MOVEMENT] });
      const save = vi.fn(() => Promise.resolve(`C:\\salida\\h.${extension}`));
      window.__TAURI__ = { dialog: { save } };
      exporterRef().mockResolvedValue({
        exportedCount: 1,
        expiredCleanupWarning: null,
      });
      await openWithProduct();
      document.getElementById("historial-producto-desde").value = "2026-02-01";
      const button = document.getElementById(buttonId);
      await vi.waitFor(() => expect(button.disabled).toBe(false));

      button.click();
      await vi.waitFor(() =>
        expect(exporterRef()).toHaveBeenCalledWith({
          productId: 2,
          fromDate: "2026-02-01",
          types: [],
          destinationPath: `C:\\salida\\h.${extension}`,
        }),
      );
      expect(save).toHaveBeenCalledWith(
        expect.objectContaining({
          filters: [expect.objectContaining({ extensions: [extension] })],
        }),
      );
      await vi.waitFor(() =>
        expect(
          document.getElementById("historial-exportar-estado").textContent,
        ).toBe("Historial exportado (1 movimiento)."),
      );
    },
  );

  it("si el usuario cancela el diálogo no se llama al comando", async () => {
    getProductHistory.mockResolvedValue({ items: [MOVEMENT] });
    const save = vi.fn(() => Promise.resolve(null));
    window.__TAURI__ = { dialog: { save } };
    await openWithProduct();
    const button = document.getElementById("historial-exportar-excel");
    await vi.waitFor(() => expect(button.disabled).toBe(false));

    button.click();
    await vi.waitFor(() => expect(save).toHaveBeenCalled());
    await Promise.resolve();
    expect(exportHistoryExcel).not.toHaveBeenCalled();
    expect(exportHistoryPdf).not.toHaveBeenCalled();
  });

  it("un fallo de escritura muestra el mensaje del backend", async () => {
    getProductHistory.mockResolvedValue({ items: [MOVEMENT] });
    window.__TAURI__ = {
      dialog: { save: vi.fn(() => Promise.resolve("C:\\x\\h.pdf")) },
    };
    exportHistoryPdf.mockRejectedValue(
      new Error("No se pudo guardar el archivo"),
    );
    await openWithProduct();
    const button = document.getElementById("historial-exportar-pdf");
    await vi.waitFor(() => expect(button.disabled).toBe(false));
    button.click();
    await vi.waitFor(() =>
      expect(document.getElementById("historial-feedback").textContent).toBe(
        "No se pudo guardar el archivo",
      ),
    );
  });
  it("H-7: muestra el aviso de purga del historial y lo limpia con null", async () => {
    getProductHistory.mockResolvedValue({
      items: [MOVEMENT],
      expiredCleanupWarning:
        "No se pudieron limpiar 2 productos vencidos; se reintentará en la próxima operación",
    });
    await openWithProduct();
    const status = () => document.getElementById("historial-cleanup-warning");
    await vi.waitFor(() =>
      expect(status()?.textContent).toBe(
        "No se pudieron limpiar 2 productos vencidos; se reintentará en la próxima operación",
      ),
    );
    expect(status().getAttribute("role")).toBe("status");
    // El listado se pinta igual.
    expect(
      document.getElementById("historial-producto-body").textContent,
    ).toContain("E-00001");

    getProductHistory.mockResolvedValue({
      items: [MOVEMENT],
      expiredCleanupWarning: null,
    });
    await __testHooks.loadProductHistory();
    expect(status().textContent).toBe("");
    expect(status().hidden).toBe(true);
  });
});
