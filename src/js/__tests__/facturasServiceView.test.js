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
import { chooseFromCombobox } from "./helpers/invoiceUi.js";

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
let createServiceInvoice, getInvoice, listInvoices, initFacturasView;

beforeAll(async () => {
  vi.resetModules();
  ({ createServiceInvoice, getInvoice, listInvoices } =
    await import("../services/invoicesService.js"));
  ({ initFacturasView } = await import("../views/facturasView.js"));
});

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

  it("el HTML de producción tiene la etiqueta de cliente, las columnas y los controles del submódulo", async () => {
    const html = await readFile(
      path.resolve(process.cwd(), "src", "dashboard-unificado.html"),
      "utf8",
    );
    const page = new DOMParser()
      .parseFromString(html, "text/html")
      .getElementById("page-facturas-servicios");
    const texts = (selector) =>
      Array.from(page.querySelectorAll(selector)).map((el) =>
        el.textContent.trim(),
      );

    expect(
      page.querySelector("#factura-servicios-create label").textContent,
    ).toContain("Buscar Cliente");
    expect(texts("#factura-servicios-list thead th")).toEqual([
      "No. Factura",
      "Cliente",
      "Emisión",
      "Total",
      "Estado",
      "Reservada hasta",
      "Acciones",
    ]);
    // Las líneas de servicio son insumos cobrados a precio de venta (FS-03).
    expect(texts("#factura-servicios-create .li-head div")).toEqual([
      "No",
      "Producto / Insumo",
      "Cantidad",
      "Unidad",
      "Precio Unitario",
      "Subtotal",
      "Acciones",
    ]);
    expect(texts("#factura-servicios-ver .li-head div")).toEqual([
      "No",
      "Producto / Insumo",
      "Cantidad",
      "Unidad",
      "Precio Unitario",
      "Subtotal",
    ]);
    expect(
      Array.from(page.querySelectorAll("#factura-servicio-estado option")).map(
        (o) => o.value,
      ),
    ).toEqual(["pending", "completed", "cancelled"]);
    expect(
      Array.from(
        page.querySelectorAll("#factura-servicio-metodo-pago option"),
      ).map((o) => o.value),
    ).toEqual(["cash", "transfer", "card", "zelle"]);

    for (const id of [
      "factura-servicios-form",
      "factura-servicio-cliente-add",
      "factura-servicio-cancelar",
      "factura-servicio-volver",
      "factura-servicio-editar",
      "factura-servicio-exportar",
      "factura-servicio-eliminar",
    ]) {
      expect(page.querySelector(`#${id}`), id).not.toBeNull();
    }
    for (const id of [
      "factura-servicio-descuento",
      "factura-servicio-impuesto",
      // §7.1: el buscador de producto basta; sin "+" ni "Agregar Fila".
      "factura-servicio-producto-add",
      "factura-servicio-agregar-fila",
      "factura-servicio-ver-descuento",
      "factura-servicio-ver-impuestos",
    ]) {
      expect(page.querySelector(`#${id}`), id).toBeNull();
    }
    expect(
      page.querySelector("[onclick],[oninput],[onchange],[onsubmit]"),
    ).toBeNull();
  });

  it("el select de estado usa códigos pending/completed/cancelled y el método de pago solo se muestra con completed", async () => {
    listInvoices.mockResolvedValue([]);
    await initFacturasView();

    const estado = document.getElementById("factura-servicio-estado");
    const metodo = document.getElementById("factura-servicio-metodo-pago");
    estado.value = "pending";
    estado.dispatchEvent(new Event("change", { bubbles: true }));
    expect(metodo.style.display).toBe("none");

    estado.value = "completed";
    estado.dispatchEvent(new Event("change", { bubbles: true }));
    expect(metodo.style.display).not.toBe("none");
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

  it("#factura-servicio-cliente es combobox y el botón + abre el alta rápida de cliente", async () => {
    listInvoices.mockResolvedValue([]);
    await initFacturasView();

    const clienteInput = document.getElementById("factura-servicio-cliente");
    expect(clienteInput.getAttribute("role")).toBe("combobox");

    document.getElementById("factura-servicio-cliente-add").click();
    const dialog = document.querySelector('[role="dialog"]');
    expect(dialog.textContent).toContain("Nuevo cliente");
    [...dialog.querySelectorAll("button")]
      .find((button) => button.textContent === "Cancelar")
      .click();
  });

  it("#factura-servicio-buscar es combobox", async () => {
    listInvoices.mockResolvedValue([]);
    await initFacturasView();

    expect(
      document.getElementById("factura-servicio-buscar").getAttribute("role"),
    ).toBe("combobox");
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

    document.getElementById("factura-servicio-fecha-emision").value =
      "2026-02-10";
    await chooseFromCombobox(
      "factura-servicio-cliente",
      "cli",
      "Cliente Cuatro",
    );
    await chooseFromCombobox("factura-servicio-buscar", "cab", "Cable");
    const quantity = document.querySelector(
      '#factura-servicio-lineas-body [data-line-field="quantity"]',
    );
    quantity.value = "2";
    quantity.dispatchEvent(new Event("input", { bubbles: true }));

    document
      .getElementById("factura-servicios-form")
      .dispatchEvent(new Event("submit", { bubbles: true, cancelable: true }));
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
