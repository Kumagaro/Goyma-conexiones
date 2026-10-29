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
  createServiceInvoice: vi.fn(),
  updateServiceInvoice: vi.fn(),
  saveInvoiceAttachment: vi.fn(),
  downloadInvoiceAttachment: vi.fn(),
  deleteInvoice: vi.fn(),
  changeInvoiceStatus: vi.fn(),
}));

vi.mock("../services/clientsService.js", () => ({
  searchClients: vi.fn(() => Promise.resolve([])),
  listClients: vi.fn(() => Promise.resolve([])),
}));

vi.mock("../services/suppliersService.js", () => ({
  searchSuppliers: vi.fn(() =>
    Promise.resolve([{ id: 3, fullName: "Proveedor Tres" }]),
  ),
  listSuppliers: vi.fn(() => Promise.resolve([])),
}));

vi.mock("../services/historyService.js", () => ({
  searchProductsForInvoice: vi.fn(() =>
    Promise.resolve([{ id: 7, name: "Cable", stock: "0", isOutOfStock: true }]),
  ),
}));

vi.mock("../components/confirmDialog.js", () => ({
  confirmDialog: vi.fn(() => Promise.resolve(true)),
}));

// Con isolate:false los módulos se comparten entre archivos de test: se
// recarga la vista para que use los mocks de este archivo.
let createEntryInvoice,
  downloadInvoiceAttachment,
  getInvoice,
  listInvoices,
  saveInvoiceAttachment,
  initFacturasView;

beforeAll(async () => {
  vi.resetModules();
  ({
    createEntryInvoice,
    downloadInvoiceAttachment,
    getInvoice,
    listInvoices,
    saveInvoiceAttachment,
  } = await import("../services/invoicesService.js"));
  ({ initFacturasView } = await import("../views/facturasView.js"));
});

function seedEntryPage() {
  document.body.innerHTML = `
    <main>
      <section id="page-facturas-entrada" class="page active">
        <div class="view-tabs">
          <button id="tab-factura-entrada-list" class="tab active">Lista</button>
          <button id="tab-factura-entrada-create" class="tab">Nueva</button>
        </div>
        <div class="search-bar" id="factura-entrada-search-bar">
          <input id="factura-entrada-search" />
        </div>
        <div id="factura-entrada-list" class="config-subview active">
          <table><tbody id="facturas-entrada-body"></tbody></table>
        </div>
        <div id="factura-entrada-create" class="config-subview">
          <form id="factura-entrada-form">
            <div id="factura-proveedor-combobox"></div>
            <input id="factura-proveedor" />
            <button id="factura-proveedor-add" type="button">+</button>
            <input id="factura-numero" readonly />
            <input id="factura-fecha-emision" type="date" />
            <select id="factura-estado">
              <option value="pending">Por Pagar</option>
              <option value="completed">Pagada</option>
              <option value="cancelled">Cancelada</option>
            </select>
            <select id="factura-metodo-pago">
              <option value="cash">Efectivo</option>
              <option value="transfer">Transferencia</option>
              <option value="card">Tarjeta</option>
              <option value="zelle">Zelle</option>
            </select>
            <input id="factura-observaciones" />
            <div id="factura-producto-combobox"></div>
            <input id="factura-buscar-producto" />
            <button id="factura-producto-add" type="button">+</button>
            <div id="factura-lineas-body"></div>
            <button id="factura-agregar-fila" type="button">Agregar Fila</button>
            <div class="dropzone" id="factura-dropzone"></div>
            <input id="factura-archivo-input" type="file" />
            <span id="factura-archivo-nombre"></span>
            <span id="factura-subtotal"></span>
            <span id="factura-total"></span>
            <textarea id="factura-descripcion"></textarea>
            <button type="submit">Guardar Factura</button>
          </form>
        </div>
        <div id="factura-entrada-ver" class="config-subview">
          <span id="factura-ver-numero-badge"></span>
          <h2 id="factura-ver-titulo"></h2>
          <span id="factura-ver-estado-txt"></span>
          <span id="factura-ver-estado-hero"></span>
          <span id="factura-ver-metodo"></span>
          <span id="factura-ver-total"></span>
          <span id="factura-ver-proveedor"></span>
          <span id="factura-ver-numero"></span>
          <span id="factura-ver-emision"></span>
          <span id="factura-ver-subtotal"></span>
          <div id="factura-ver-productos"></div>
          <span id="factura-ver-archivo"></span>
          <span id="factura-ver-observaciones"></span>
          <span id="factura-ver-descripcion"></span>
          <button id="factura-entrada-editar" type="button">Editar</button>
          <button id="factura-entrada-guardar-equipo" type="button">Guardar en el equipo</button>
          <button id="factura-entrada-exportar" type="button">Exportar Factura</button>
          <button id="factura-entrada-eliminar" type="button">Eliminar Factura</button>
        </div>
      </section>
    </main>
  `;
}

describe("facturasView submódulo entry", () => {
  beforeEach(() => {
    vi.useFakeTimers();
    seedEntryPage();
    vi.clearAllMocks();
  });

  afterEach(() => {
    vi.runOnlyPendingTimers();
    vi.useRealTimers();
    document.body.innerHTML = "";
    delete window.exportarFacturaPDF;
    delete window.__facturasEntryPageChangeBound;
  });

  it("pinta solo lo que devuelve listInvoices con estados de carga, vacío y error", async () => {
    listInvoices.mockResolvedValue([]);
    const initPromise = initFacturasView();
    // Estado de carga visible antes de resolver
    expect(
      document.getElementById("facturas-entrada-body").textContent,
    ).toContain("Cargando");
    await initPromise;
    expect(listInvoices).toHaveBeenCalledWith(
      expect.objectContaining({ invoiceType: "E" }),
    );
    expect(
      document.getElementById("facturas-entrada-body").textContent,
    ).toContain("Aún no hay facturas de entrada");

    listInvoices.mockResolvedValueOnce([
      {
        id: 1,
        number: "E-00001",
        invoiceType: "E",
        contactName: "Proveedor Uno",
        date: "2026-02-10",
        totalCents: 125000,
        status: "pending",
      },
    ]);
    await initFacturasView();
    expect(
      document.getElementById("facturas-entrada-body").textContent,
    ).toContain("E-00001");
    expect(
      document.getElementById("facturas-entrada-body").textContent,
    ).toContain("Proveedor Uno");

    listInvoices.mockRejectedValueOnce(new Error("falla"));
    await initFacturasView();
    expect(
      document.getElementById("facturas-entrada-body").textContent,
    ).toContain("No se pudieron cargar");
  });

  it("debouncea el buscador #factura-entrada-search con 300ms", async () => {
    listInvoices.mockResolvedValue([]);
    await initFacturasView();
    vi.clearAllMocks();

    const input = document.getElementById("factura-entrada-search");
    input.value = "E-000";
    input.dispatchEvent(new Event("input", { bubbles: true }));
    input.value = "E-00001";
    input.dispatchEvent(new Event("input", { bubbles: true }));

    await Promise.resolve();
    expect(listInvoices).not.toHaveBeenCalled();

    await vi.advanceTimersByTimeAsync(300);
    expect(listInvoices).toHaveBeenCalledTimes(1);
    expect(listInvoices).toHaveBeenCalledWith(
      expect.objectContaining({ invoiceType: "E", search: "E-00001" }),
    );
  });

  it("el select de estado usa códigos pending/completed/cancelled y el método de pago solo se muestra con completed", async () => {
    listInvoices.mockResolvedValue([]);
    await initFacturasView();

    const estado = document.getElementById("factura-estado");
    const valores = Array.from(estado.options).map((o) => o.value);
    expect(valores).toEqual(["pending", "completed", "cancelled"]);

    const metodo = document.getElementById("factura-metodo-pago");
    estado.value = "pending";
    estado.dispatchEvent(new Event("change", { bubbles: true }));
    expect(
      metodo.closest(".field")?.style.display ?? metodo.style.display,
    ).toBe("none");

    estado.value = "completed";
    estado.dispatchEvent(new Event("change", { bubbles: true }));
    expect(metodo.style.display).not.toBe("none");
  });

  it("#factura-proveedor es combobox y el botón + abre el alta rápida de proveedor", async () => {
    listInvoices.mockResolvedValue([]);
    await initFacturasView();

    const proveedorInput = document.getElementById("factura-proveedor");
    expect(proveedorInput.getAttribute("role")).toBe("combobox");
    expect(proveedorInput.getAttribute("aria-autocomplete")).toBe("list");

    document.getElementById("factura-proveedor-add").click();
    const dialog = document.querySelector('[role="dialog"]');
    expect(dialog.textContent).toContain("Nuevo proveedor");
    [...dialog.querySelectorAll("button")]
      .find((button) => button.textContent === "Cancelar")
      .click();
  });

  it("#factura-buscar-producto es combobox", async () => {
    listInvoices.mockResolvedValue([]);
    await initFacturasView();

    const productoInput = document.getElementById("factura-buscar-producto");
    expect(productoInput.getAttribute("role")).toBe("combobox");
  });

  it("#factura-descuento y #factura-impuesto no existen", async () => {
    listInvoices.mockResolvedValue([]);
    await initFacturasView();
    expect(document.getElementById("factura-descuento")).toBeNull();
    expect(document.getElementById("factura-impuesto")).toBeNull();
  });

  it("el dropzone sigue existiendo y al guardar llama a saveInvoiceAttachment si hay archivo", async () => {
    listInvoices.mockResolvedValue([]);
    createEntryInvoice.mockResolvedValue({
      invoiceId: 5,
      number: "E-00005",
      expiredCleanupWarning: null,
    });
    saveInvoiceAttachment.mockResolvedValue({ ok: true });
    await initFacturasView();

    expect(document.getElementById("factura-dropzone")).not.toBeNull();

    // Simula un adjunto pendiente registrado por la vista
    const { __testHooks } = await import("../views/facturasView.js");
    __testHooks.setPendingAttachment("/tmp/factura.pdf");

    document.getElementById("factura-fecha-emision").value = "2026-02-10";
    await chooseFromCombobox("factura-proveedor", "pro", "Proveedor Tres");
    // Una compra puede reponer un producto agotado (FE-01).
    await chooseFromCombobox("factura-buscar-producto", "cab", "Cable");
    for (const [field, value] of [
      ["quantity", "2"],
      ["unitPrice", "10,00"],
    ]) {
      const input = document.querySelector(
        `#factura-lineas-body [data-line-field="${field}"]`,
      );
      input.value = value;
      input.dispatchEvent(new Event("input", { bubbles: true }));
    }

    const form = document.getElementById("factura-entrada-form");
    form.dispatchEvent(
      new Event("submit", { bubbles: true, cancelable: true }),
    );
    await vi.waitFor(() => {
      expect(createEntryInvoice).toHaveBeenCalledWith(
        expect.objectContaining({
          supplierId: 3,
          lines: [{ productId: 7, quantity: "2", unitPrice: "10,00" }],
        }),
      );
    });
    await vi.waitFor(() => {
      expect(saveInvoiceAttachment).toHaveBeenCalledWith(5, "/tmp/factura.pdf");
    });
  });

  it("el botón Editar del detalle solo se muestra si status es pending", async () => {
    listInvoices.mockResolvedValue([
      {
        id: 1,
        number: "E-00001",
        invoiceType: "E",
        contactName: "Proveedor Uno",
        date: "2026-02-10",
        totalCents: 1000,
        status: "pending",
      },
    ]);
    getInvoice.mockResolvedValue({
      id: 1,
      number: "E-00001",
      invoiceType: "E",
      status: "pending",
      contactFullName: "Proveedor Uno",
      date: "2026-02-10",
      totalCents: 1000,
      lines: [],
    });
    await initFacturasView();
    document
      .querySelector('#facturas-entrada-body [data-entry-action="detail"]')
      ?.dispatchEvent(new Event("click", { bubbles: true }));
    // Apertura directa del detalle para aislar la regla de visibilidad
    const { __testHooks } = await import("../views/facturasView.js");
    await __testHooks.openEntryDetail(1);
    expect(document.getElementById("factura-entrada-editar").hidden).toBe(
      false,
    );

    getInvoice.mockResolvedValueOnce({
      id: 2,
      number: "E-00002",
      invoiceType: "E",
      status: "completed",
      contactFullName: "Proveedor Dos",
      date: "2026-02-11",
      totalCents: 2000,
      lines: [],
    });
    await __testHooks.openEntryDetail(2);
    expect(document.getElementById("factura-entrada-editar").hidden).toBe(true);
  });

  it('el botón "Guardar en el equipo" llama a downloadInvoiceAttachment', async () => {
    listInvoices.mockResolvedValue([]);
    getInvoice.mockResolvedValue({
      id: 9,
      number: "E-00009",
      invoiceType: "E",
      status: "pending",
      contactFullName: "Proveedor Nueve",
      date: "2026-02-10",
      totalCents: 500,
      lines: [],
      attachmentFileName: "abc.pdf",
    });
    downloadInvoiceAttachment.mockResolvedValue({ ok: true });
    await initFacturasView();

    const { __testHooks } = await import("../views/facturasView.js");
    await __testHooks.openEntryDetail(9);
    // Simula la ruta elegida en el diálogo nativo
    __testHooks.setDownloadDestination("/tmp/salida.pdf");

    document.getElementById("factura-entrada-guardar-equipo").click();
    await vi.waitFor(() => {
      expect(downloadInvoiceAttachment).toHaveBeenCalledWith(
        9,
        "/tmp/salida.pdf",
      );
    });
  });

  it('el botón "Exportar Factura" llama a exportarFacturaPDF con el DTO de getInvoice', async () => {
    listInvoices.mockResolvedValue([]);
    const detail = {
      id: 4,
      number: "E-00004",
      invoiceType: "E",
      status: "pending",
      contactFullName: "Proveedor Cuatro",
      date: "2026-02-10",
      totalCents: 800,
      lines: [],
    };
    getInvoice.mockResolvedValue(detail);
    window.exportarFacturaPDF = vi.fn();
    await initFacturasView();

    const { __testHooks } = await import("../views/facturasView.js");
    await __testHooks.openEntryDetail(4);
    document.getElementById("factura-entrada-exportar").click();
    expect(window.exportarFacturaPDF).toHaveBeenCalledWith(detail);
  });

  it("muestra un nombre de producto con HTML como texto (salida segura)", async () => {
    listInvoices.mockResolvedValue([
      {
        id: 1,
        number: "E-00001",
        invoiceType: "E",
        contactName: '<img src=x onerror="alert(1)">Proveedor',
        date: "2026-02-10",
        totalCents: 1000,
        status: "pending",
      },
    ]);
    await initFacturasView();
    const body = document.getElementById("facturas-entrada-body");
    expect(body.textContent).toContain("Proveedor");
    expect(body.querySelector("img")).toBeNull();

    getInvoice.mockResolvedValue({
      id: 1,
      number: "E-00001",
      invoiceType: "E",
      status: "pending",
      contactFullName: "Proveedor",
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
    await __testHooks.openEntryDetail(1);
    const productos = document.getElementById("factura-ver-productos");
    expect(productos.textContent).toContain("Café");
    expect(productos.querySelector("img")).toBeNull();
  });
});
