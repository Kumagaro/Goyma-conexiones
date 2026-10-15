import { afterEach, beforeEach, describe, expect, it, vi } from "vitest";

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

vi.mock("../components/confirmDialog.js", () => ({
  confirmDialog: vi.fn(() => Promise.resolve(true)),
}));

import {
  createServiceInvoice,
  getInvoice,
  listInvoices,
} from "../services/invoicesService.js";
import { initFacturasView } from "../views/facturasView.js";

function seedServicePage() {
  document.body.innerHTML = `
    <main>
      <section id="page-facturas-servicios" class="page active">
        <div class="view-tabs">
          <button id="tab-factura-servicios-list" class="tab active">Lista</button>
          <button id="tab-factura-servicios-create" class="tab">Nueva</button>
        </div>
        <div class="search-bar" id="factura-servicios-search-bar">
          <input id="factura-servicios-search" />
        </div>
        <div id="factura-servicios-list" class="config-subview active">
          <table>
            <thead><tr>
              <th>No. Factura</th><th>Cliente</th><th>Emisión</th>
              <th>Total</th><th>Estado</th><th>Acciones</th>
            </tr></thead>
            <tbody id="facturas-servicios-body"></tbody>
          </table>
        </div>
        <div id="factura-servicios-create" class="config-subview">
          <form id="factura-servicios-form">
            <label>Buscar Cliente <span class="required">*</span></label>
            <input id="factura-servicio-cliente" />
            <button id="factura-servicio-cliente-add" type="button">+</button>
            <input id="factura-servicio-numero" readonly />
            <input id="factura-servicio-fecha-emision" type="date" />
            <select id="factura-servicio-estado">
              <option value="pending">Por Cobrar</option>
              <option value="completed">Cobrada</option>
              <option value="cancelled">Cancelada</option>
            </select>
            <select id="factura-servicio-metodo-pago">
              <option value="cash">Efectivo</option>
              <option value="transfer">Transferencia</option>
              <option value="card">Tarjeta</option>
              <option value="zelle">Zelle</option>
            </select>
            <input id="factura-servicio-observaciones" />
            <input id="factura-servicio-buscar" />
            <button id="factura-servicio-producto-add" type="button">+</button>
            <div id="factura-servicio-lineas-body"></div>
            <button id="factura-servicio-agregar-fila" type="button">Agregar Fila</button>
            <span id="factura-servicio-subtotal"></span>
            <span id="factura-servicio-total"></span>
            <textarea id="factura-servicio-descripcion"></textarea>
            <button type="submit">Guardar Factura</button>
          </form>
        </div>
        <div id="factura-servicios-ver" class="config-subview">
          <span id="factura-servicio-ver-numero-badge"></span>
          <h2 id="factura-servicio-ver-titulo"></h2>
          <span id="factura-servicio-ver-estado-txt"></span>
          <span id="factura-servicio-ver-estado-hero"></span>
          <span id="factura-servicio-ver-metodo"></span>
          <span id="factura-servicio-ver-total"></span>
          <span id="factura-servicio-ver-cliente"></span>
          <span id="factura-servicio-ver-numero"></span>
          <span id="factura-servicio-ver-emision"></span>
          <span id="factura-servicio-ver-subtotal"></span>
          <div id="factura-servicio-ver-productos"></div>
          <span id="factura-servicio-ver-observaciones"></span>
          <span id="factura-servicio-ver-descripcion"></span>
          <button id="factura-servicio-editar" type="button">Editar</button>
          <button id="factura-servicio-exportar" type="button">Exportar Factura</button>
          <button id="factura-servicio-eliminar" type="button">Eliminar Factura</button>
        </div>
      </section>
    </main>
  `;
}

describe("facturasView submódulo service", () => {
  beforeEach(() => {
    vi.useFakeTimers();
    seedServicePage();
    vi.clearAllMocks();
  });

  afterEach(() => {
    vi.runOnlyPendingTimers();
    vi.useRealTimers();
    document.body.innerHTML = "";
    delete window.exportarFacturaServicioPDF;
    delete window.__facturasServicePageChangeBound;
  });

  it("pinta solo lo que devuelve listInvoices con la etiqueta de cliente y sus columnas", async () => {
    listInvoices.mockResolvedValue([]);
    const initPromise = initFacturasView();
    expect(
      document.getElementById("facturas-servicios-body").textContent,
    ).toContain("Cargando");
    await initPromise;
    expect(listInvoices).toHaveBeenCalledWith(
      expect.objectContaining({ invoiceType: "S" }),
    );
    expect(
      document.getElementById("facturas-servicios-body").textContent,
    ).toContain("Aún no hay facturas de servicio");

    // Etiqueta del cliente y columnas propias de la tabla de servicios.
    const headers = Array.from(
      document.querySelectorAll(
        "#factura-servicios-list thead th",
      ),
    ).map((th) => th.textContent);
    expect(headers).toEqual([
      "No. Factura",
      "Cliente",
      "Emisión",
      "Total",
      "Estado",
      "Acciones",
    ]);

    listInvoices.mockResolvedValueOnce([
      {
        id: 1,
        number: "S-00001",
        invoiceType: "S",
        contactName: "Cliente Uno",
        date: "2026-02-10",
        totalCents: 150000,
        status: "pending",
      },
    ]);
    await initFacturasView();
    const body = document.getElementById("facturas-servicios-body");
    expect(body.textContent).toContain("S-00001");
    expect(body.textContent).toContain("Cliente Uno");

    listInvoices.mockRejectedValueOnce(new Error("falla"));
    await initFacturasView();
    expect(
      document.getElementById("facturas-servicios-body").textContent,
    ).toContain("No se pudieron cargar");
  });

  it("debouncea el buscador #factura-servicios-search con 300ms", async () => {
    listInvoices.mockResolvedValue([]);
    await initFacturasView();
    vi.clearAllMocks();

    const input = document.getElementById("factura-servicios-search");
    input.value = "S-000";
    input.dispatchEvent(new Event("input", { bubbles: true }));
    input.value = "S-00001";
    input.dispatchEvent(new Event("input", { bubbles: true }));

    await Promise.resolve();
    expect(listInvoices).not.toHaveBeenCalled();

    await vi.advanceTimersByTimeAsync(300);
    expect(listInvoices).toHaveBeenCalledTimes(1);
    expect(listInvoices).toHaveBeenCalledWith(
      expect.objectContaining({ invoiceType: "S", search: "S-00001" }),
    );
  });

  it("#factura-servicio-cliente es combobox y el botón + abre el formulario de cliente", async () => {
    listInvoices.mockResolvedValue([]);
    await initFacturasView();

    const clienteInput = document.getElementById("factura-servicio-cliente");
    expect(clienteInput.getAttribute("role")).toBe("combobox");

    const dispatched = [];
    document.addEventListener(
      "service:openClientForm",
      (event) => dispatched.push(event),
      { once: true },
    );
    document.getElementById("factura-servicio-cliente-add").click();
    expect(dispatched).toHaveLength(1);
  });

  it("#factura-servicio-buscar es combobox con botón + y hay botón Agregar Fila", async () => {
    listInvoices.mockResolvedValue([]);
    await initFacturasView();

    expect(
      document.getElementById("factura-servicio-buscar").getAttribute("role"),
    ).toBe("combobox");
    expect(
      document.getElementById("factura-servicio-producto-add"),
    ).not.toBeNull();
    expect(
      document.getElementById("factura-servicio-agregar-fila"),
    ).not.toBeNull();
  });

  it("#factura-servicio-descuento y #factura-servicio-impuesto no existen", async () => {
    listInvoices.mockResolvedValue([]);
    await initFacturasView();
    expect(document.getElementById("factura-servicio-descuento")).toBeNull();
    expect(document.getElementById("factura-servicio-impuesto")).toBeNull();
  });

  it("al guardar llama a createServiceInvoice con cliente, fecha y líneas", async () => {
    listInvoices.mockResolvedValue([]);
    createServiceInvoice.mockResolvedValue({
      invoiceId: 7,
      number: "S-00007",
      expiredCleanupWarning: null,
    });
    await initFacturasView();

    const { __testHooks } = await import("../views/facturasView.js");
    document.getElementById("factura-servicio-fecha-emision").value =
      "2026-02-10";
    __testHooks.setSelectedServiceClient({ id: 4, fullName: "Cliente Cuatro" });
    __testHooks.setServiceLines([{ productId: 7, quantity: "2" }]);

    document
      .getElementById("factura-servicios-form")
      .dispatchEvent(
        new Event("submit", { bubbles: true, cancelable: true }),
      );
    await vi.waitFor(() => {
      expect(createServiceInvoice).toHaveBeenCalledWith(
        expect.objectContaining({
          clientId: 4,
          invoiceDate: "2026-02-10",
          lines: [{ productId: 7, quantity: "2" }],
        }),
      );
    });
  });

  it("el botón Editar del detalle solo se muestra si status es pending", async () => {
    listInvoices.mockResolvedValue([]);
    getInvoice.mockResolvedValue({
      id: 1,
      number: "S-00001",
      invoiceType: "S",
      status: "pending",
      contactFullName: "Cliente Uno",
      date: "2026-02-10",
      totalCents: 1000,
      lines: [],
    });
    await initFacturasView();

    const { __testHooks } = await import("../views/facturasView.js");
    await __testHooks.openServiceDetail(1);
    expect(document.getElementById("factura-servicio-editar").hidden).toBe(
      false,
    );

    getInvoice.mockResolvedValueOnce({
      id: 2,
      number: "S-00002",
      invoiceType: "S",
      status: "completed",
      contactFullName: "Cliente Dos",
      date: "2026-02-11",
      totalCents: 2000,
      lines: [],
    });
    await __testHooks.openServiceDetail(2);
    expect(document.getElementById("factura-servicio-editar").hidden).toBe(
      true,
    );
  });

  it('el botón "Exportar Factura" llama a exportarFacturaServicioPDF con el DTO de getInvoice', async () => {
    listInvoices.mockResolvedValue([]);
    const detail = {
      id: 4,
      number: "S-00004",
      invoiceType: "S",
      status: "pending",
      contactFullName: "Cliente Cuatro",
      date: "2026-02-10",
      totalCents: 800,
      lines: [],
    };
    getInvoice.mockResolvedValue(detail);
    window.exportarFacturaServicioPDF = vi.fn();
    await initFacturasView();

    const { __testHooks } = await import("../views/facturasView.js");
    await __testHooks.openServiceDetail(4);
    document.getElementById("factura-servicio-exportar").click();
    expect(window.exportarFacturaServicioPDF).toHaveBeenCalledWith(detail);
  });

  it("muestra un nombre de cliente con HTML como texto (salida segura)", async () => {
    listInvoices.mockResolvedValue([
      {
        id: 1,
        number: "S-00001",
        invoiceType: "S",
        contactName: '<img src=x onerror="alert(1)">Cliente',
        date: "2026-02-10",
        totalCents: 1000,
        status: "pending",
      },
    ]);
    await initFacturasView();
    const body = document.getElementById("facturas-servicios-body");
    expect(body.textContent).toContain("Cliente");
    expect(body.querySelector("img")).toBeNull();

    getInvoice.mockResolvedValue({
      id: 1,
      number: "S-00001",
      invoiceType: "S",
      status: "pending",
      contactFullName: '<img src=x onerror="alert(1)">Cliente',
      date: "2026-02-10",
      totalCents: 1000,
      lines: [],
    });
    const { __testHooks } = await import("../views/facturasView.js");
    await __testHooks.openServiceDetail(1);
    const detail = document.getElementById("factura-servicios-ver");
    expect(detail.textContent).toContain("Cliente");
    expect(detail.querySelector("img")).toBeNull();
  });
});
