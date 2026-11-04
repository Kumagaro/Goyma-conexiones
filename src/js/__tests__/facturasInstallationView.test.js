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
import { chooseFromCombobox, toastText } from "./helpers/invoiceUi.js";

vi.mock("../services/invoicesService.js", () => ({
  listInvoices: vi.fn(),
  getInvoice: vi.fn(),
  createEntryInvoice: vi.fn(),
  createSaleInvoice: vi.fn(),
  createServiceInvoice: vi.fn(),
  createGuaranteeInvoice: vi.fn(),
  createInstallationInvoice: vi.fn(),
  saveInvoiceAttachment: vi.fn(),
  downloadInvoiceAttachment: vi.fn(),
  deleteInvoice: vi.fn(),
  changeInvoiceStatus: vi.fn(),
}));

vi.mock("../services/clientsService.js", () => ({
  searchClients: vi.fn(() => Promise.resolve([])),
  getClientDetail: vi.fn(),
}));

vi.mock("../services/historyService.js", () => ({
  searchProductsForInvoice: vi.fn(() =>
    Promise.resolve([{ id: 11, name: "Cable", stock: "5" }]),
  ),
}));

vi.mock("../components/confirmDialog.js", () => ({
  confirmDialog: vi.fn(() => Promise.resolve(true)),
}));

// Con isolate:false los módulos se comparten entre archivos de test: se
// recarga la vista para que use los mocks de este archivo.
let createInstallationInvoice,
  getInvoice,
  listInvoices,
  getClientDetail,
  searchClients,
  __testHooks,
  initFacturasView;

beforeAll(async () => {
  vi.resetModules();
  ({ createInstallationInvoice, getInvoice, listInvoices } =
    await import("../services/invoicesService.js"));
  ({ getClientDetail, searchClients } =
    await import("../services/clientsService.js"));
  ({ __testHooks, initFacturasView } =
    await import("../views/facturasView.js"));
});

let pageHtml = "";

beforeAll(async () => {
  const html = await readFile(
    path.resolve(process.cwd(), "src", "dashboard-unificado.html"),
    "utf8",
  );
  pageHtml = new DOMParser()
    .parseFromString(html, "text/html")
    .getElementById("page-facturas-insumos").outerHTML;
});

const CLIENT_HISTORY = [
  { id: 1, number: "V-00001", invoiceType: "V", status: "pending" },
  { id: 2, number: "S-00001", invoiceType: "S", status: "completed" },
  { id: 3, number: "S-00002", invoiceType: "S", status: "cancelled" },
  { id: 4, number: "S-00003", invoiceType: "S", status: "pending" },
  { id: 5, number: "GG-00001", invoiceType: "GG", status: "pending" },
];

// Elige el cliente en el buscador como el usuario (task-03 §7).
async function selectClient(id, fullName = "Cliente Siete") {
  // Persistente: una búsqueda con debounce pendiente no consume la respuesta.
  searchClients.mockResolvedValue([{ id, fullName }]);
  await chooseFromCombobox("factura-insumo-cliente", "cli", fullName);
  await vi.waitFor(() =>
    expect(
      document.getElementById("factura-insumo-servicio").options.length,
    ).toBeGreaterThan(1),
  );
}

function optionTexts(select) {
  return Array.from(select.options).map((o) => [o.value, o.textContent]);
}

describe("facturasView submódulo installation (HTML de producción)", () => {
  beforeEach(() => {
    document.body.innerHTML = `<main>${pageHtml}</main>`;
    vi.clearAllMocks();
    listInvoices.mockResolvedValue([]);
  });

  afterEach(() => {
    document.body.innerHTML = "";
    delete window.__facturasInstallationPageChangeBound;
  });

  it("pinta solo lo que devuelve listInvoices con estados de carga, vacío y error", async () => {
    const initPromise = initFacturasView();
    const body = () => document.getElementById("facturas-insumos-body");
    expect(body().textContent).toContain("Cargando");
    await initPromise;
    expect(listInvoices).toHaveBeenCalledWith({ invoiceType: "GI" });
    expect(body().textContent).toContain("Aún no hay facturas de insumos");

    listInvoices.mockResolvedValueOnce([
      {
        id: 9,
        number: "GI-00001",
        invoiceType: "GI",
        contactName: "Cliente Uno",
        date: "2026-02-10",
        totalCents: 1500,
        status: "completed",
        associatedInvoice: { number: "S-00005", invoiceType: "S" },
      },
    ]);
    await initFacturasView();
    expect(body().textContent).toContain("GI-00001");
    expect(body().textContent).toContain("S-00005 · Servicio");
    expect(body().textContent).toContain("Completada");

    listInvoices.mockRejectedValueOnce(new Error("falla"));
    await initFacturasView();
    expect(body().textContent).toContain("No se pudieron cargar");
  });

  it("#factura-insumo-estado tiene las opciones correctas y no hay select de método de pago", async () => {
    await initFacturasView();
    expect(
      optionTexts(document.getElementById("factura-insumo-estado")),
    ).toEqual([
      ["pending", "Pendiente"],
      ["completed", "Completada"],
      ["cancelled", "Cancelada"],
    ]);
    expect(document.getElementById("factura-insumo-metodo-pago")).toBeNull();
    expect(document.getElementById("factura-insumo-ver-metodo")).toBeNull();
  });

  it("no hay dropzone y #factura-insumo-servicio es un select required con los servicios vigentes del cliente", async () => {
    getClientDetail.mockResolvedValue({ invoiceHistory: CLIENT_HISTORY });
    await initFacturasView();

    expect(document.getElementById("factura-insumo-dropzone")).toBeNull();
    const select = document.getElementById("factura-insumo-servicio");
    expect(select.tagName).toBe("SELECT");
    expect(select.required).toBe(true);

    await selectClient(7);
    expect(getClientDetail).toHaveBeenCalledWith(7);
    // Solo servicios no cancelados; ni ventas ni garantías.
    expect(optionTexts(select)).toEqual([
      ["", "Selecciona la factura de servicio"],
      ["2", "S-00001 · Servicio"],
      ["4", "S-00003 · Servicio"],
    ]);
  });

  it("al cambiar de cliente el select se limpia", async () => {
    getClientDetail.mockResolvedValue({ invoiceHistory: CLIENT_HISTORY });
    await initFacturasView();
    await selectClient(7);
    const select = document.getElementById("factura-insumo-servicio");
    select.value = "2";

    const cliente = document.getElementById("factura-insumo-cliente");
    cliente.value = "Otro";
    cliente.dispatchEvent(new Event("input", { bubbles: true }));
    expect(select.value).toBe("");
    expect(select.options).toHaveLength(1);
  });

  it("#factura-insumo-ver-asociada muestra {numero} · {tipo visible} y el costo real", async () => {
    getInvoice.mockResolvedValue({
      id: 9,
      number: "GI-00001",
      invoiceType: "GI",
      status: "pending",
      contactFullName: "Cliente Uno",
      date: "2026-02-10",
      totalCents: 900,
      associatedInvoice: { number: "S-00003", invoiceType: "S" },
      lines: [
        {
          productName: "Tubo",
          quantity: "3",
          unitPriceCents: 300,
          subtotalCents: 900,
        },
      ],
    });
    await initFacturasView();
    await __testHooks.openInstallationDetail(9);
    expect(
      document.getElementById("factura-insumo-ver-asociada").textContent,
    ).toBe("S-00003 · Servicio");
    expect(
      document.getElementById("factura-insumo-ver-titulo").textContent,
    ).toBe("Factura de Insumos GI-00001");
    expect(
      document.getElementById("factura-insumo-ver-productos").textContent,
    ).toContain("Tubo");
    expect(document.getElementById("factura-insumo-editar").hidden).toBe(false);
  });

  it('el botón "Crear Nuevo Insumo", los datalist y los inputs de descuento e impuesto no existen', async () => {
    await initFacturasView();
    const page = document.getElementById("page-facturas-insumos");
    expect(page.textContent).not.toContain("Crear Nuevo Insumo");
    expect(page.querySelector("datalist")).toBeNull();
    for (const id of [
      "factura-insumo-descuento",
      "factura-insumo-impuesto",
      "factura-insumo-ver-descuento",
      "factura-insumo-ver-impuestos",
    ]) {
      expect(document.getElementById(id), id).toBeNull();
    }
    expect(
      page.querySelector("[onclick],[oninput],[onchange],[onsubmit],[ondrop]"),
    ).toBeNull();
  });

  it("al guardar llama a createInstallationInvoice con cliente, servicio asociado, fecha y líneas", async () => {
    getClientDetail.mockResolvedValue({ invoiceHistory: CLIENT_HISTORY });
    createInstallationInvoice.mockResolvedValue({
      invoiceId: 3,
      number: "GI-00003",
      expiredCleanupWarning: null,
    });
    await initFacturasView();

    const form = document.getElementById("factura-insumos-form");
    const submit = () =>
      form.dispatchEvent(
        new Event("submit", { bubbles: true, cancelable: true }),
      );

    await selectClient(7);
    document.getElementById("factura-insumo-fecha-emision").value =
      "2026-02-10";
    await chooseFromCombobox("factura-insumo-buscar", "cab", "Cable");
    const quantity = document.querySelector(
      '#factura-insumo-lineas-body [data-line-field="quantity"]',
    );
    quantity.value = "2";
    quantity.dispatchEvent(new Event("input", { bubbles: true }));
    submit();
    await Promise.resolve();
    expect(createInstallationInvoice).not.toHaveBeenCalled();
    expect(toastText()).toContain("factura de servicio");

    document.getElementById("factura-insumo-servicio").value = "4";
    document.getElementById("factura-insumo-estado").value = "completed";
    submit();
    await vi.waitFor(() => {
      expect(createInstallationInvoice).toHaveBeenCalledWith({
        clientId: 7,
        associatedInvoiceId: 4,
        invoiceDate: "2026-02-10",
        status: "completed",
        lines: [{ productId: 11, quantity: "2" }],
      });
    });
  });

  it("muestra un nombre de cliente con HTML como texto (salida segura)", async () => {
    const evil = '<img src=x onerror="alert(1)">Cliente';
    listInvoices.mockResolvedValue([
      {
        id: 1,
        number: "GI-00001",
        invoiceType: "GI",
        contactName: evil,
        date: "2026-02-10",
        totalCents: 100,
        status: "pending",
        associatedInvoice: null,
      },
    ]);
    getInvoice.mockResolvedValue({
      id: 1,
      number: "GI-00001",
      invoiceType: "GI",
      status: "pending",
      contactFullName: evil,
      date: "2026-02-10",
      totalCents: 100,
      lines: [{ productName: evil, quantity: "1" }],
    });
    await initFacturasView();
    await __testHooks.openInstallationDetail(1);
    const page = document.getElementById("page-facturas-insumos");
    expect(page.textContent).toContain("Cliente");
    expect(page.querySelector("img")).toBeNull();
  });
});
