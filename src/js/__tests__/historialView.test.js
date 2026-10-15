import { afterEach, beforeEach, describe, expect, it, vi } from "vitest";

vi.mock("../services/historyService.js", () => ({
  getProductHistory: vi.fn(),
  listHistoryProducts: vi.fn(),
  exportHistoryExcel: vi.fn(),
  exportHistoryPdf: vi.fn(),
}));

import {
  getProductHistory,
  listHistoryProducts,
} from "../services/historyService.js";
import { formatTimestamp } from "../utils/formatters.js";
import { initHistorialView } from "../views/historialView.js";

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
