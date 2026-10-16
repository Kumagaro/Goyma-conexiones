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

vi.mock("../services/invoicesService.js", () => ({
  listInvoices: vi.fn(),
  getInvoice: vi.fn(),
  createEntryInvoice: vi.fn(),
  createSaleInvoice: vi.fn(),
  createServiceInvoice: vi.fn(),
  createGuaranteeInvoice: vi.fn(),
  createInstallationInvoice: vi.fn(),
  createLossInvoice: vi.fn(),
  saveInvoiceAttachment: vi.fn(),
  downloadInvoiceAttachment: vi.fn(),
  deleteInvoice: vi.fn(),
  changeInvoiceStatus: vi.fn(),
}));

vi.mock("../services/clientsService.js", () => ({
  searchClients: vi.fn(() => Promise.resolve([])),
  getClientDetail: vi.fn(),
}));

vi.mock("../services/suppliersService.js", () => ({
  searchSuppliers: vi.fn(() => Promise.resolve([])),
}));

vi.mock("../components/confirmDialog.js", () => ({
  confirmDialog: vi.fn(() => Promise.resolve(true)),
}));

// Con isolate:false los módulos se comparten entre archivos de test: se
// recarga la vista para que use los mocks de este archivo.
let getInvoice, listInvoices, initFacturasView, renderFacturasGeneral;
let mainHtml = "";
let dashboardSource = "";

beforeAll(async () => {
  vi.resetModules();
  ({ getInvoice, listInvoices } =
    await import("../services/invoicesService.js"));
  ({ initFacturasView, renderFacturasGeneral } =
    await import("../views/facturasView.js"));
  const root = path.resolve(process.cwd(), "src");
  const html = await readFile(
    path.join(root, "dashboard-unificado.html"),
    "utf8",
  );
  // Todas las páginas de facturas: "Ver" abre el detalle de cada tipo.
  mainHtml = Array.from(
    new DOMParser()
      .parseFromString(html, "text/html")
      .querySelectorAll('[id^="page-facturas-"]'),
  )
    .map((page) => page.outerHTML)
    .join("");
  dashboardSource = await readFile(
    path.join(root, "js", "dashboard.js"),
    "utf8",
  );
});

const ROWS = [
  {
    id: 6,
    number: "P-00001",
    invoiceType: "P",
    contactName: null,
    date: "2026-02-12",
    totalCents: 300,
    status: null,
    productName: "Cable",
    quantity: "1",
  },
  {
    id: 4,
    number: "GG-00001",
    invoiceType: "GG",
    contactName: "Cliente Uno",
    date: "2026-02-11",
    totalCents: 900,
    status: "completed",
  },
  {
    id: 2,
    number: "V-00001",
    invoiceType: "V",
    contactName: "Cliente Uno",
    date: "2026-02-10",
    totalCents: 2500,
    status: "pending",
  },
  {
    id: 1,
    number: "E-00001",
    invoiceType: "E",
    contactName: "Proveedor Uno",
    date: "2026-02-09",
    totalCents: 1000,
    status: "completed",
  },
];

const body = () => document.getElementById("facturas-general-body");
const rowTexts = () =>
  Array.from(body().querySelectorAll("tr")).map((tr) =>
    Array.from(tr.querySelectorAll("td")).map((td) => td.textContent),
  );

describe("facturasView submódulo general (HTML de producción)", () => {
  beforeEach(() => {
    document.body.innerHTML = `<aside id="sidebar"></aside><main>${mainHtml}</main>`;
    vi.clearAllMocks();
    listInvoices.mockResolvedValue([]);
    window.showPage = vi.fn();
  });

  afterEach(() => {
    vi.useRealTimers();
    document.body.innerHTML = "";
    delete window.showPage;
    delete window.__facturasGeneralPageChangeBound;
  });

  it("#factura-general-tipo y #factura-general-estado tienen los value del contrato", async () => {
    await initFacturasView();
    const values = (id) =>
      Array.from(document.getElementById(id).options).map((o) => o.value);
    expect(values("factura-general-tipo")).toEqual([
      "",
      "E",
      "V",
      "S",
      "GG",
      "GI",
      "P",
    ]);
    expect(values("factura-general-estado")).toEqual([
      "",
      "pendientes",
      "completadas",
      "canceladas",
    ]);
    expect(
      document
        .getElementById("page-facturas-general")
        .querySelector("[onclick],[oninput],[onchange]"),
    ).toBeNull();
  });

  it("lista todos los tipos con listInvoices sin tipo y traduce tipo y estado", async () => {
    listInvoices.mockImplementation((filters) =>
      Promise.resolve(filters?.invoiceType ? [] : ROWS),
    );
    await initFacturasView();
    expect(listInvoices).toHaveBeenCalledWith({});
    expect(rowTexts().map((cells) => cells.slice(0, 3))).toEqual([
      ["P-00001", "Pérdida", "—"],
      ["GG-00001", "Garantía", "Cliente Uno"],
      ["V-00001", "Venta", "Cliente Uno"],
      ["E-00001", "Entrada", "Proveedor Uno"],
    ]);
    expect(rowTexts().map((cells) => cells[5])).toEqual([
      "—",
      "Completada",
      "Por Cobrar",
      "Pagada",
    ]);
    // Solo lectura: cada fila tiene solo "Ver".
    expect(rowTexts().map((cells) => cells[6])).toEqual([
      "Ver",
      "Ver",
      "Ver",
      "Ver",
    ]);
  });

  it("los filtros y la búsqueda se envían al backend", async () => {
    await initFacturasView();
    vi.clearAllMocks();
    listInvoices.mockResolvedValue([]);

    const tipo = document.getElementById("factura-general-tipo");
    tipo.value = "GG";
    tipo.dispatchEvent(new Event("change"));
    const estado = document.getElementById("factura-general-estado");
    estado.value = "pendientes";
    estado.dispatchEvent(new Event("change"));
    await vi.waitFor(() =>
      expect(listInvoices).toHaveBeenLastCalledWith({
        invoiceType: "GG",
        status: "pendientes",
      }),
    );
    expect(body().textContent).toContain("No se encontraron facturas");

    const search = document.getElementById("factura-general-search");
    search.value = "uno";
    search.dispatchEvent(new Event("input"));
    await vi.waitFor(() =>
      expect(listInvoices).toHaveBeenLastCalledWith({
        search: "uno",
        invoiceType: "GG",
        status: "pendientes",
      }),
    );
  });

  it("muestra estados de carga, vacío y error", async () => {
    const pending = initFacturasView();
    expect(body().textContent).toContain("Cargando");
    await pending;
    expect(body().textContent).toContain("Aún no hay facturas");

    listInvoices.mockRejectedValueOnce(new Error("falla"));
    window.renderFacturasGeneral();
    await vi.waitFor(() =>
      expect(body().textContent).toContain("No se pudieron cargar"),
    );
  });

  it('el botón "Ver" abre la página y el detalle del tipo correspondiente', async () => {
    listInvoices.mockImplementation((filters) =>
      Promise.resolve(filters?.invoiceType ? [] : ROWS),
    );
    getInvoice.mockResolvedValue({
      id: 2,
      number: "V-00001",
      invoiceType: "V",
      status: "pending",
      contactFullName: "Cliente Uno",
      date: "2026-02-10",
      totalCents: 2500,
      lines: [],
    });
    await initFacturasView();

    body().querySelector('[data-general-type="V"]').click();
    await vi.waitFor(() => expect(getInvoice).toHaveBeenCalledWith(2));
    expect(window.showPage).toHaveBeenCalledWith("facturas-venta", null);
    expect(
      document.getElementById("factura-venta-ver").classList.contains("active"),
    ).toBe(true);
    expect(
      document.getElementById("factura-venta-ver-numero").textContent,
    ).toBe("V-00001");

    getInvoice.mockResolvedValue({
      id: 6,
      number: "P-00001",
      invoiceType: "P",
      date: "2026-02-12",
      totalCents: 300,
      lines: [{ productName: "Cable", quantity: "1", unitPriceCents: 300 }],
    });
    body().querySelector('[data-general-type="P"]').click();
    await vi.waitFor(() => expect(getInvoice).toHaveBeenCalledWith(6));
    expect(window.showPage).toHaveBeenLastCalledWith("facturas-perdidas", null);
    expect(
      document
        .getElementById("factura-perdidas-ver")
        .classList.contains("active"),
    ).toBe(true);
  });

  it("renderFacturasGeneral pinta con textContent: un nombre con HTML se muestra como texto", async () => {
    await initFacturasView();
    renderFacturasGeneral([
      {
        id: 9,
        number: "V-00009",
        invoiceType: "V",
        contactName: '<img src=x onerror="alert(1)">Cliente',
        date: "2026-02-10",
        totalCents: 100,
        status: "pending",
      },
    ]);
    expect(body().textContent).toContain("Cliente");
    expect(body().querySelector("img")).toBeNull();
  });

  it("facturaTipoInfo, obtenerFacturasCombinadas y la lista general simulada ya no existen en dashboard.js", () => {
    for (const name of [
      "facturaTipoInfo",
      "obtenerFacturasCombinadas",
      "function renderFacturasGeneral",
      "function irAFacturaDesdeGeneral",
    ]) {
      expect(dashboardSource, name).not.toContain(name);
    }
  });
});
