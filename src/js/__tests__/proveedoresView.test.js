import { afterEach, beforeEach, describe, expect, it, vi } from "vitest";

vi.mock("../services/suppliersService.js", () => ({
  listSuppliers: vi.fn(),
  searchSuppliers: vi.fn(),
  createSupplier: vi.fn(),
  updateSupplier: vi.fn(),
  deleteSupplier: vi.fn(),
  getSupplierDetail: vi.fn(),
}));

vi.mock("../components/confirmDialog.js", () => ({
  confirmDialog: vi.fn(() => Promise.resolve(true)),
}));

import {
  createSupplier,
  deleteSupplier,
  getSupplierDetail,
  listSuppliers,
  searchSuppliers,
  updateSupplier,
} from "../services/suppliersService.js";
import { initProveedoresView } from "../views/proveedoresView.js";
import { confirmDialog } from "../components/confirmDialog.js";
import { formatDate, formatMoney } from "../utils/formatters.js";

function seedSupplierPage() {
  document.body.innerHTML = `
    <main>
      <section id="page-proveedores" class="page active">
        <div class="view-tabs">
          <button id="tab-proveedor-list" class="tab active">Proveedores</button>
          <button id="tab-proveedor-create" class="tab">Crear Nuevo Proveedor</button>
          <button id="tab-proveedor-historial" class="tab">Historial de Compras</button>
        </div>
        <div id="proveedor-search-bar">
          <input id="proveedor-search" />
        </div>
        <div id="proveedor-list" class="config-subview active">
          <table><tbody id="proveedores-body"></tbody></table>
        </div>
        <div id="proveedor-create" class="config-subview">
          <form id="proveedor-form">
            <input id="proveedor-nombres" />
            <input id="proveedor-apellidos" />
            <input id="proveedor-ci" />
            <input id="proveedor-telefono" />
            <input id="proveedor-correo" />
            <input id="proveedor-empresa" />
            <input id="proveedor-cargo" />
            <input id="proveedor-direccion" />
            <button type="submit">Guardar</button>
          </form>
        </div>
        <div id="proveedor-historial" class="config-subview">
          <select id="historial-proveedor-select"></select>
          <input type="date" id="historial-proveedor-desde" />
          <input type="date" id="historial-proveedor-hasta" />
          <table>
            <thead><tr><th id="th-fecha-historial-proveedor">Fecha</th></tr></thead>
            <tbody id="historial-proveedor-body"></tbody>
          </table>
        </div>
        <div id="proveedor-ver" class="config-subview">
          <span id="proveedor-ver-id"></span>
          <h2 id="proveedor-ver-nombre-completo"></h2>
          <span id="proveedor-ver-empresa"></span>
          <span id="proveedor-ver-cargo"></span>
          <span id="proveedor-ver-telefono"></span>
          <span id="proveedor-ver-correo"></span>
          <span id="proveedor-ver-ci"></span>
          <span id="proveedor-ver-direccion"></span>
          <span id="proveedor-ver-total-comprado"></span>
          <span id="proveedor-ver-saldo-pendiente"></span>
          <span id="proveedor-ver-num-facturas"></span>
          <span id="proveedor-ver-ultima-compra"></span>
          <div id="proveedor-ver-historial-facturas"></div>
          <button id="proveedor-ver-editar" type="button">Editar</button>
          <button id="proveedor-ver-eliminar" type="button">Eliminar</button>
        </div>
      </section>
    </main>
  `;
}

describe("proveedoresView", () => {
  beforeEach(() => {
    vi.useFakeTimers();
    seedSupplierPage();
    vi.clearAllMocks();
  });

  afterEach(() => {
    vi.runOnlyPendingTimers();
    vi.useRealTimers();
    document.body.innerHTML = "";
  });

  it("el encabezado Fecha del historial de compras ordena sin onclick inline", async () => {
    listSuppliers.mockResolvedValue([]);
    window.toggleOrdenFecha = vi.fn();
    await initProveedoresView();
    document.getElementById("th-fecha-historial-proveedor").click();
    expect(window.toggleOrdenFecha).toHaveBeenCalledWith(
      "historialProveedor",
      expect.any(Function),
    );
    delete window.toggleOrdenFecha;
  });

  it("PV-07: el historial pinta las compras del proveedor elegido, filtra, ordena y abre la factura", async () => {
    listSuppliers.mockResolvedValue([
      { id: 1, fullName: "Ana Pérez" },
      { id: 2, fullName: "Luis Gómez" },
    ]);
    const evil = '<img src=x onerror="alert(1)">E-00009';
    getSupplierDetail.mockImplementation((id) =>
      Promise.resolve({
        invoiceHistory:
          id === 1
            ? [
                {
                  id: 11,
                  number: "E-00001",
                  invoiceType: "E",
                  date: "2026-01-05",
                  totalCents: 1500,
                  status: "completed",
                  productCount: 2,
                  createdByName: "contadora",
                },
                {
                  id: 12,
                  number: evil,
                  invoiceType: "E",
                  date: "2026-02-10",
                  totalCents: 2500,
                  status: "pending",
                },
              ]
            : [],
      }),
    );
    window.irAFacturaDesdeGeneral = vi.fn();
    await initProveedoresView();
    // Sin abrir la pestaña no se consulta el historial.
    expect(getSupplierDetail).not.toHaveBeenCalled();

    document.getElementById("tab-proveedor-historial").click();
    const body = document.getElementById("historial-proveedor-body");
    const numbers = () =>
      [...body.querySelectorAll("tr")].map((row) => row.cells[4]?.textContent);
    await vi.waitFor(() => expect(body.querySelectorAll("tr")).toHaveLength(2));
    expect(getSupplierDetail).toHaveBeenCalledWith(1);
    // Más reciente primero; texto seguro.
    expect(numbers()).toEqual([evil, "E-00001"]);
    expect(body.querySelector("img")).toBeNull();
    expect(body.textContent).toContain(formatDate("2026-01-05"));
    expect(body.textContent).toContain(formatMoney(1500));
    expect(body.textContent).toContain("Pagada");
    expect(body.textContent).toContain("Por Pagar");
    // Columnas de PV-07: fecha, estado, cantidad de productos, total, factura y responsable.
    expect(
      [...body.querySelectorAll("tr")[1].cells].map((cell) => cell.textContent),
    ).toEqual([
      formatDate("2026-01-05"),
      "Pagada",
      "2",
      formatMoney(1500),
      "E-00001",
      "contadora",
      "Ver",
    ]);

    document.getElementById("th-fecha-historial-proveedor").click();
    expect(numbers()).toEqual(["E-00001", evil]);

    const desde = document.getElementById("historial-proveedor-desde");
    desde.value = "2026-02-01";
    desde.dispatchEvent(new Event("change", { bubbles: true }));
    expect(numbers()).toEqual([evil]);

    body.querySelector("button").click();
    expect(window.irAFacturaDesdeGeneral).toHaveBeenCalledWith("E", 12);

    const hasta = document.getElementById("historial-proveedor-hasta");
    hasta.value = "2026-01-01";
    hasta.dispatchEvent(new Event("change", { bubbles: true }));
    expect(body.textContent).toContain(
      "La fecha inicial no puede ser posterior a la final",
    );

    desde.value = "";
    hasta.value = "";
    const select = document.getElementById("historial-proveedor-select");
    select.value = "2";
    select.dispatchEvent(new Event("change", { bubbles: true }));
    await vi.waitFor(() =>
      expect(body.textContent).toContain(
        "Este proveedor aún no tiene compras registradas.",
      ),
    );

    getSupplierDetail.mockRejectedValueOnce(new Error("falla"));
    select.value = "1";
    select.dispatchEvent(new Event("change", { bubbles: true }));
    await vi.waitFor(() =>
      expect(body.textContent).toContain(
        "No se pudo cargar el historial de compras.",
      ),
    );
    delete window.irAFacturaDesdeGeneral;
  });

  it("PV-05: al eliminar avisa cuántas facturas se conservan", async () => {
    listSuppliers.mockResolvedValue([{ id: 1, fullName: "Ana Pérez" }]);
    getSupplierDetail.mockResolvedValue({
      invoiceHistory: [{ id: 1 }, { id: 2, status: "cancelled" }],
    });
    confirmDialog.mockResolvedValueOnce(false);
    await initProveedoresView();
    document.querySelector('[data-supplier-action="delete"]').click();
    await vi.waitFor(() => expect(confirmDialog).toHaveBeenCalled());
    expect(confirmDialog.mock.calls[0][0].message).toContain(
      "Ana Pérez tiene 2 facturas; se conservarán con la copia de sus datos.",
    );
    expect(deleteSupplier).not.toHaveBeenCalled();
  });

  it("carga proveedores y muestra el estado vacío si no llega nada", async () => {
    listSuppliers.mockResolvedValue([]);

    await initProveedoresView();

    expect(listSuppliers).toHaveBeenCalledTimes(1);
    expect(document.getElementById("proveedores-body").textContent).toContain(
      "Aún no hay proveedores",
    );
  });

  it("debouncea la búsqueda y reacciona a page:change", async () => {
    searchSuppliers.mockResolvedValue([]);

    await initProveedoresView();
    const input = document.getElementById("proveedor-search");
    input.value = "Ana";
    input.dispatchEvent(new Event("input", { bubbles: true }));

    await Promise.resolve();
    expect(searchSuppliers).not.toHaveBeenCalled();

    await vi.advanceTimersByTimeAsync(300);
    expect(searchSuppliers).toHaveBeenCalledWith("Ana");

    window.dispatchEvent(
      new CustomEvent("page:change", { detail: { pageId: "proveedores" } }),
    );
    await Promise.resolve();
    expect(listSuppliers).toHaveBeenCalledTimes(2);
  });

  it("crea, detalla y elimina un proveedor con salida segura", async () => {
    listSuppliers.mockResolvedValue([
      {
        id: 3,
        fullName: "Marta López",
        company: "Goyma",
        phone: "+53 55512345",
      },
    ]);

    getSupplierDetail.mockResolvedValue({
      id: 3,
      firstName: "Marta",
      lastName: "López",
      fullName: "Marta López",
      nationalId: "90010112345",
      phone: "+53 55512345",
      company: "Goyma",
      position: "Gerente",
      email: "marta@goyma.test",
      address: "Calle 2",
      createdAt: "2026-02-05",
      indicators: {
        totalInvoicedCents: 0,
        totalPurchasedCents: 0,
        pendingBalanceCents: 0,
        invoiceCount: 0,
        lastPurchaseDate: null,
      },
      invoiceHistory: [],
      expiredCleanupWarning: null,
    });

    createSupplier.mockResolvedValue({ id: 99 });
    deleteSupplier.mockResolvedValue({ deleted: true });

    await initProveedoresView();
    const form = document.getElementById("proveedor-form");
    document.getElementById("proveedor-nombres").value = "Luis";
    document.getElementById("proveedor-apellidos").value = "Pérez";
    document.getElementById("proveedor-ci").value = "90010112345";
    document.getElementById("proveedor-telefono").value = "+53 55512345";
    document.getElementById("proveedor-correo").value = "luis@goyma.test";
    document.getElementById("proveedor-empresa").value = "Goyma";
    document.getElementById("proveedor-cargo").value = "Analista";
    document.getElementById("proveedor-direccion").value = "Calle 9";

    form.dispatchEvent(
      new Event("submit", { bubbles: true, cancelable: true }),
    );
    await vi.waitFor(() => {
      expect(createSupplier).toHaveBeenCalledWith({
        firstName: "Luis",
        lastName: "Pérez",
        nationalId: "90010112345",
        phone: "+53 55512345",
        email: "luis@goyma.test",
        company: "Goyma",
        position: "Analista",
        address: "Calle 9",
      });
    });

    await vi.waitFor(() => {
      expect(
        document.querySelector('#proveedores-body tr[data-supplier-id="3"]'),
      ).not.toBeNull();
    });
    const row = document.querySelector(
      '#proveedores-body tr[data-supplier-id="3"]',
    );
    row.querySelector('[data-supplier-action="detail"]').click();
    await vi.waitFor(() => {
      expect(
        document.getElementById("proveedor-ver-nombre-completo").textContent,
      ).toContain("Marta López");
    });
    expect(document.getElementById("proveedor-ver-fecha-entrada")).toBeNull();

    const safeName = "<img src=x onerror=alert(1)>Ana";
    listSuppliers.mockResolvedValueOnce([
      {
        id: 7,
        fullName: safeName,
        company: "",
        phone: "+53 55512345",
      },
    ]);
    await initProveedoresView();
    expect(document.getElementById("proveedores-body").textContent).toContain(
      "Ana",
    );
    expect(
      document.getElementById("proveedores-body").querySelector("img"),
    ).toBeNull();

    await vi.waitFor(() => {
      expect(
        document.querySelector('#proveedores-body tr[data-supplier-id="7"]'),
      ).not.toBeNull();
    });
    const deleteRow = document.querySelector(
      '#proveedores-body tr[data-supplier-id="7"]',
    );
    deleteRow.querySelector('[data-supplier-action="delete"]').click();
    await vi.waitFor(() => {
      expect(deleteSupplier).toHaveBeenCalledWith(7);
    });
  });
  it("el detalle muestra totalPurchasedCents, los indicadores y el historial de compras", async () => {
    listSuppliers.mockResolvedValue([
      { id: 8, fullName: "Luis Gómez", company: null, phone: "+5355550002" },
    ]);
    const detail = {
      id: 8,
      firstName: "Luis",
      lastName: "Gómez",
      fullName: "Luis Gómez",
      nationalId: "90010112346",
      phone: "+5355550002",
      createdAt: "2026-01-03",
      indicators: {
        totalInvoicedCents: 0,
        totalPurchasedCents: 98_700,
        pendingBalanceCents: 10_000,
        invoiceCount: 2,
        lastPurchaseDate: "2026-02-11",
      },
      invoiceHistory: [
        {
          id: 21,
          number: "E-00021",
          invoiceType: "E",
          date: "2026-02-11",
          totalCents: 88_700,
          status: "completed",
        },
      ],
      expiredCleanupWarning: null,
    };
    getSupplierDetail.mockResolvedValue(detail);
    window.irAFacturaDesdeGeneral = vi.fn();

    await initProveedoresView();
    await vi.waitFor(() =>
      expect(
        document.querySelector('[data-supplier-action="detail"]'),
      ).not.toBeNull(),
    );
    document.querySelector('[data-supplier-action="detail"]').click();
    await vi.waitFor(() =>
      expect(
        document.getElementById("proveedor-ver-num-facturas").textContent,
      ).toBe("2"),
    );

    const text = (id) => document.getElementById(id).textContent;
    expect(text("proveedor-ver-total-comprado")).toBe(
      formatMoney(98_700).replace(" USD", ""),
    );
    expect(text("proveedor-ver-saldo-pendiente")).toBe(
      formatMoney(10_000).replace(" USD", ""),
    );
    expect(text("proveedor-ver-ultima-compra")).toBe(formatDate("2026-02-11"));

    const history = document.getElementById("proveedor-ver-historial-facturas");
    const cells = Array.from(history.querySelectorAll("tbody td")).map(
      (td) => td.textContent,
    );
    expect(cells.slice(0, 5)).toEqual([
      "E-00021",
      "Entrada",
      formatDate("2026-02-11"),
      formatMoney(88_700),
      "Pagada",
    ]);
    history.querySelector("tbody button").click();
    expect(window.irAFacturaDesdeGeneral).toHaveBeenCalledWith("E", 21);
    delete window.irAFacturaDesdeGeneral;

    // Sin compras: estado vacío claro.
    getSupplierDetail.mockResolvedValue({ ...detail, invoiceHistory: [] });
    document.querySelector('[data-supplier-action="detail"]')?.click();
    await vi.waitFor(() =>
      expect(history.textContent).toContain(
        "Este proveedor aún no tiene compras registradas.",
      ),
    );
    expect(history.querySelector("table")).toBeNull();
  });
  it("H-7: muestra el aviso de purga del detalle con role=status y lo limpia con null", async () => {
    listSuppliers.mockResolvedValue([
      { id: 6, fullName: "Rosa Díaz", company: null, phone: "+5355550009" },
    ]);
    const detail = {
      id: 6,
      firstName: "Rosa",
      lastName: "Díaz",
      fullName: "Rosa Díaz",
      nationalId: "90010112347",
      phone: "+5355550009",
      createdAt: "2026-01-04",
      indicators: {
        totalInvoicedCents: 0,
        totalPurchasedCents: 0,
        pendingBalanceCents: 0,
        invoiceCount: 0,
        lastPurchaseDate: null,
      },
      invoiceHistory: [],
      expiredCleanupWarning:
        "No se pudieron limpiar 2 productos vencidos; se reintentará en la próxima operación",
    };
    getSupplierDetail.mockResolvedValue(detail);
    await initProveedoresView();
    const open = () =>
      document.querySelector('[data-supplier-action="detail"]').click();
    await vi.waitFor(() =>
      expect(
        document.querySelector('[data-supplier-action="detail"]'),
      ).not.toBeNull(),
    );
    open();
    const status = () => document.getElementById("proveedor-ver-warning");
    await vi.waitFor(() =>
      expect(status()?.textContent).toBe(
        "No se pudieron limpiar 2 productos vencidos; se reintentará en la próxima operación",
      ),
    );
    expect(status().getAttribute("role")).toBe("status");
    expect(status().hidden).toBe(false);

    getSupplierDetail.mockResolvedValue({
      ...detail,
      expiredCleanupWarning: null,
    });
    open();
    await vi.waitFor(() => expect(status().textContent).toBe(""));
    expect(status().hidden).toBe(true);
  });
});
