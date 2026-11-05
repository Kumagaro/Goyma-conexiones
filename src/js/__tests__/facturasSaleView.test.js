import { chooseFromCombobox } from "./helpers/invoiceUi.js";
import {
  afterEach,
  beforeAll,
  beforeEach,
  describe,
  expect,
  it,
  vi,
} from "vitest";

vi.mock("../services/invoicesService.js", () => ({
  listInvoices: vi.fn(),
  getInvoice: vi.fn(),
  createEntryInvoice: vi.fn(),
  updateEntryInvoice: vi.fn(),
  createSaleInvoice: vi.fn(),
  updateSaleInvoice: vi.fn(),
  saveInvoiceAttachment: vi.fn(),
  downloadInvoiceAttachment: vi.fn(),
  deleteInvoice: vi.fn(),
  changeInvoiceStatus: vi.fn(),
}));

vi.mock("../services/clientsService.js", () => ({
  searchClients: vi.fn(() =>
    Promise.resolve([{ id: 4, fullName: "Cliente Cuatro" }]),
  ),
  listClients: vi.fn(() => Promise.resolve([])),
}));

vi.mock("../services/historyService.js", () => ({
  searchProductsForInvoice: vi.fn(() =>
    Promise.resolve([
      { id: 7, name: "Cable", stock: "5", sellingPriceCents: 100 },
    ]),
  ),
}));

vi.mock("../components/confirmDialog.js", () => ({
  confirmDialog: vi.fn(() => Promise.resolve(true)),
}));

// Con isolate:false los módulos se comparten entre archivos de test: se
// recarga la vista para que use los mocks de este archivo.
let createSaleInvoice, getInvoice, listInvoices, initFacturasView;

beforeAll(async () => {
  vi.resetModules();
  ({ createSaleInvoice, getInvoice, listInvoices } =
    await import("../services/invoicesService.js"));
  ({ initFacturasView } = await import("../views/facturasView.js"));
});

function seedSalePage() {
  document.body.innerHTML = `
    <main>
      <section id="page-facturas-venta" class="page active">
        <div class="view-tabs">
          <button id="tab-factura-venta-list" class="tab active">Lista</button>
          <button id="tab-factura-venta-create" class="tab">Nueva</button>
        </div>
        <div class="search-bar" id="factura-venta-search-bar">
          <input id="factura-venta-search" />
        </div>
        <div id="factura-venta-list" class="config-subview active">
          <table><tbody id="facturas-venta-body"></tbody></table>
        </div>
        <div id="factura-venta-create" class="config-subview">
          <form id="factura-venta-form">
            <input id="factura-venta-cliente" />
            <button id="factura-venta-cliente-add" type="button">+</button>
            <input id="factura-venta-numero" readonly />
            <input id="factura-venta-fecha-emision" type="date" />
            <select id="factura-venta-estado">
              <option value="pending">Por Cobrar</option>
              <option value="completed">Cobrada</option>
              <option value="cancelled">Cancelada</option>
            </select>
            <select id="factura-venta-metodo-pago">
              <option value="cash">Efectivo</option>
              <option value="transfer">Transferencia</option>
              <option value="card">Tarjeta</option>
              <option value="zelle">Zelle</option>
            </select>
            <input id="factura-venta-observaciones" />
            <input id="factura-venta-buscar-producto" />
            <button id="factura-venta-producto-add" type="button">+</button>
            <div id="factura-venta-lineas-body"></div>
            <button id="factura-venta-agregar-fila" type="button">Agregar Fila</button>
            <span id="factura-venta-subtotal"></span>
            <span id="factura-venta-total"></span>
            <textarea id="factura-venta-descripcion"></textarea>
            <button type="submit">Guardar Factura</button>
          </form>
        </div>
        <div id="factura-venta-ver" class="config-subview">
          <span id="factura-venta-ver-numero-badge"></span>
          <h2 id="factura-venta-ver-titulo"></h2>
          <span id="factura-venta-ver-estado-txt"></span>
          <span id="factura-venta-ver-estado-hero"></span>
          <span id="factura-venta-ver-metodo"></span>
          <span id="factura-venta-ver-total"></span>
          <span id="factura-venta-ver-cliente"></span>
          <span id="factura-venta-ver-numero"></span>
          <span id="factura-venta-ver-emision"></span>
          <span id="factura-venta-ver-subtotal"></span>
          <div id="factura-venta-ver-productos"></div>
          <span id="factura-venta-ver-observaciones"></span>
          <span id="factura-venta-ver-descripcion"></span>
          <button id="factura-venta-editar" type="button">Editar</button>
          <button id="factura-venta-exportar" type="button">Exportar Factura</button>
          <button id="factura-venta-eliminar" type="button">Eliminar Factura</button>
        </div>
      </section>
    </main>
  `;
}

describe("facturasView submódulo sale", () => {
  beforeEach(() => {
    vi.useFakeTimers();
    seedSalePage();
    vi.clearAllMocks();
  });

  afterEach(() => {
    vi.runOnlyPendingTimers();
    vi.useRealTimers();
    document.body.innerHTML = "";
    delete window.__facturasSalePageChangeBound;
  });

  it("pinta solo lo que devuelve listInvoices con estados de carga, vacío y error", async () => {
    listInvoices.mockResolvedValue([]);
    const initPromise = initFacturasView();
    expect(
      document.getElementById("facturas-venta-body").textContent,
    ).toContain("Cargando");
    await initPromise;
    expect(listInvoices).toHaveBeenCalledWith(
      expect.objectContaining({ invoiceType: "V" }),
    );
    expect(
      document.getElementById("facturas-venta-body").textContent,
    ).toContain("Aún no hay facturas de venta");

    listInvoices.mockResolvedValueOnce([
      {
        id: 1,
        number: "V-00001",
        invoiceType: "V",
        contactName: "Cliente Uno",
        date: "2026-02-10",
        totalCents: 250000,
        status: "pending",
      },
    ]);
    await initFacturasView();
    expect(
      document.getElementById("facturas-venta-body").textContent,
    ).toContain("V-00001");
    expect(
      document.getElementById("facturas-venta-body").textContent,
    ).toContain("Cliente Uno");

    listInvoices.mockRejectedValueOnce(new Error("falla"));
    await initFacturasView();
    expect(
      document.getElementById("facturas-venta-body").textContent,
    ).toContain("No se pudieron cargar");
  });

  it("debouncea el buscador #factura-venta-search con 300ms", async () => {
    listInvoices.mockResolvedValue([]);
    await initFacturasView();
    vi.clearAllMocks();

    const input = document.getElementById("factura-venta-search");
    input.value = "V-000";
    input.dispatchEvent(new Event("input", { bubbles: true }));
    input.value = "V-00001";
    input.dispatchEvent(new Event("input", { bubbles: true }));

    await Promise.resolve();
    expect(listInvoices).not.toHaveBeenCalled();

    await vi.advanceTimersByTimeAsync(300);
    expect(listInvoices).toHaveBeenCalledTimes(1);
    expect(listInvoices).toHaveBeenCalledWith(
      expect.objectContaining({ invoiceType: "V", search: "V-00001" }),
    );
  });

  it("el select de estado usa códigos pending/completed/cancelled y el método de pago solo se muestra con completed", async () => {
    listInvoices.mockResolvedValue([]);
    await initFacturasView();

    const estado = document.getElementById("factura-venta-estado");
    const valores = Array.from(estado.options).map((o) => o.value);
    expect(valores).toEqual(["pending", "completed", "cancelled"]);

    const metodo = document.getElementById("factura-venta-metodo-pago");
    estado.value = "pending";
    estado.dispatchEvent(new Event("change", { bubbles: true }));
    expect(
      metodo.closest(".field")?.style.display ?? metodo.style.display,
    ).toBe("none");

    estado.value = "completed";
    estado.dispatchEvent(new Event("change", { bubbles: true }));
    expect(metodo.style.display).not.toBe("none");
  });

  it("#factura-venta-cliente es combobox y el botón + abre el alta rápida de cliente", async () => {
    listInvoices.mockResolvedValue([]);
    await initFacturasView();

    const clienteInput = document.getElementById("factura-venta-cliente");
    expect(clienteInput.getAttribute("role")).toBe("combobox");
    expect(clienteInput.getAttribute("aria-autocomplete")).toBe("list");

    document.getElementById("factura-venta-cliente-add").click();
    const dialog = await vi.waitFor(() => {
      const opened = document.querySelector('[role="dialog"]');
      expect(opened).not.toBeNull();
      return opened;
    });
    expect(dialog.textContent).toContain("Nuevo cliente");
    [...dialog.querySelectorAll("button")]
      .find((button) => button.textContent === "Cancelar")
      .click();
  });

  it("#factura-venta-buscar-producto es combobox", async () => {
    listInvoices.mockResolvedValue([]);
    await initFacturasView();

    const productoInput = document.getElementById(
      "factura-venta-buscar-producto",
    );
    expect(productoInput.getAttribute("role")).toBe("combobox");
  });

  it("#factura-venta-descuento y #factura-venta-impuesto no existen", async () => {
    listInvoices.mockResolvedValue([]);
    await initFacturasView();
    expect(document.getElementById("factura-venta-descuento")).toBeNull();
    expect(document.getElementById("factura-venta-impuesto")).toBeNull();
  });

  it("al guardar llama a createSaleInvoice con cliente, fecha y líneas", async () => {
    listInvoices.mockResolvedValue([]);
    createSaleInvoice.mockResolvedValue({
      invoiceId: 6,
      number: "V-00006",
      expiredCleanupWarning: null,
    });
    await initFacturasView();

    document.getElementById("factura-venta-fecha-emision").value = "2026-02-10";
    await chooseFromCombobox("factura-venta-cliente", "cli", "Cliente Cuatro");
    await chooseFromCombobox("factura-venta-buscar-producto", "cab", "Cable");
    const quantity = document.querySelector(
      '#factura-venta-lineas-body [data-line-field="quantity"]',
    );
    quantity.value = "2";
    quantity.dispatchEvent(new Event("input", { bubbles: true }));

    const form = document.getElementById("factura-venta-form");
    form.dispatchEvent(
      new Event("submit", { bubbles: true, cancelable: true }),
    );
    await vi.waitFor(() => {
      expect(createSaleInvoice).toHaveBeenCalledWith(
        expect.objectContaining({
          clientId: 4,
          invoiceDate: "2026-02-10",
          lines: [{ productId: 7, quantity: "2" }],
        }),
      );
    });
  });

  it("el botón Editar del detalle solo se muestra si status es pending", async () => {
    listInvoices.mockResolvedValue([
      {
        id: 1,
        number: "V-00001",
        invoiceType: "V",
        contactName: "Cliente Uno",
        date: "2026-02-10",
        totalCents: 1000,
        status: "pending",
      },
    ]);
    getInvoice.mockResolvedValue({
      id: 1,
      number: "V-00001",
      invoiceType: "V",
      status: "pending",
      contactFullName: "Cliente Uno",
      date: "2026-02-10",
      totalCents: 1000,
      lines: [],
    });
    await initFacturasView();

    const { __testHooks } = await import("../views/facturasView.js");
    await __testHooks.openSaleDetail(1);
    expect(document.getElementById("factura-venta-editar").hidden).toBe(false);

    getInvoice.mockResolvedValueOnce({
      id: 2,
      number: "V-00002",
      invoiceType: "V",
      status: "completed",
      contactFullName: "Cliente Dos",
      date: "2026-02-11",
      totalCents: 2000,
      lines: [],
    });
    await __testHooks.openSaleDetail(2);
    expect(document.getElementById("factura-venta-editar").hidden).toBe(true);
  });

  it("muestra un nombre de cliente con HTML como texto (salida segura)", async () => {
    listInvoices.mockResolvedValue([
      {
        id: 1,
        number: "V-00001",
        invoiceType: "V",
        contactName: '<img src=x onerror="alert(1)">Cliente',
        date: "2026-02-10",
        totalCents: 1000,
        status: "pending",
      },
    ]);
    await initFacturasView();
    const body = document.getElementById("facturas-venta-body");
    expect(body.textContent).toContain("Cliente");
    expect(body.querySelector("img")).toBeNull();

    getInvoice.mockResolvedValue({
      id: 1,
      number: "V-00001",
      invoiceType: "V",
      status: "pending",
      contactFullName: '<img src=x onerror="alert(1)">Cliente',
      date: "2026-02-10",
      totalCents: 1000,
      lines: [
        {
          position: 1,
          productName: '<img src=x onerror="alert(1)">Café',
          quantity: "1",
          unitPriceCents: 1000,
          subtotalCents: 1000,
        },
      ],
    });
    const { __testHooks } = await import("../views/facturasView.js");
    await __testHooks.openSaleDetail(1);
    const detail = document.getElementById("factura-venta-ver");
    expect(detail.textContent).toContain("Cliente");
    expect(detail.querySelector("img")).toBeNull();
  });
});
