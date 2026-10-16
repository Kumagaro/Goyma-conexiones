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
  saveInvoiceAttachment: vi.fn(),
  downloadInvoiceAttachment: vi.fn(),
  deleteInvoice: vi.fn(),
  changeInvoiceStatus: vi.fn(),
}));

vi.mock("../services/clientsService.js", () => ({
  searchClients: vi.fn(() => Promise.resolve([])),
  getClientDetail: vi.fn(),
}));

vi.mock("../components/confirmDialog.js", () => ({
  confirmDialog: vi.fn(() => Promise.resolve(true)),
}));

// Con isolate:false los módulos se comparten entre archivos de test: se
// recarga la vista para que use los mocks de este archivo.
let createGuaranteeInvoice,
  getInvoice,
  listInvoices,
  getClientDetail,
  __testHooks,
  initFacturasView;

beforeAll(async () => {
  vi.resetModules();
  ({ createGuaranteeInvoice, getInvoice, listInvoices } =
    await import("../services/invoicesService.js"));
  ({ getClientDetail } = await import("../services/clientsService.js"));
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
    .getElementById("page-facturas-garantia").outerHTML;
});

const CLIENT_HISTORY = [
  { id: 1, number: "V-00001", invoiceType: "V", status: "pending" },
  { id: 2, number: "S-00001", invoiceType: "S", status: "completed" },
  { id: 3, number: "V-00002", invoiceType: "V", status: "cancelled" },
  { id: 4, number: "GG-00001", invoiceType: "GG", status: "pending" },
  { id: 5, number: "GI-00001", invoiceType: "GI", status: "pending" },
];

function optionTexts(select) {
  return Array.from(select.options).map((o) => [o.value, o.textContent]);
}

describe("facturasView submódulo guarantee (HTML de producción)", () => {
  beforeEach(() => {
    document.body.innerHTML = `<main>${pageHtml}</main>`;
    vi.clearAllMocks();
    listInvoices.mockResolvedValue([]);
  });

  afterEach(() => {
    document.body.innerHTML = "";
    delete window.exportarFacturaGarantiaPDF;
    delete window.__facturasGuaranteePageChangeBound;
  });

  it("pinta solo lo que devuelve listInvoices con estados de carga, vacío y error", async () => {
    const initPromise = initFacturasView();
    const body = () => document.getElementById("facturas-garantia-body");
    expect(body().textContent).toContain("Cargando");
    await initPromise;
    expect(listInvoices).toHaveBeenCalledWith({ invoiceType: "GG" });
    expect(body().textContent).toContain("Aún no hay facturas de garantía");

    listInvoices.mockResolvedValueOnce([
      {
        id: 9,
        number: "GG-00001",
        invoiceType: "GG",
        contactName: "Cliente Uno",
        date: "2026-02-10",
        totalCents: 1500,
        status: "pending",
        associatedInvoice: { number: "V-00005", invoiceType: "V" },
      },
    ]);
    await initFacturasView();
    expect(body().textContent).toContain("GG-00001");
    expect(body().textContent).toContain("V-00005 · Venta");
    expect(body().textContent).toContain("Pendiente");

    listInvoices.mockRejectedValueOnce(new Error("falla"));
    await initFacturasView();
    expect(body().textContent).toContain("No se pudieron cargar");
  });

  it("#factura-garantia-estado tiene las opciones correctas y no hay select de método de pago", async () => {
    await initFacturasView();
    const estado = document.getElementById("factura-garantia-estado");
    expect(optionTexts(estado)).toEqual([
      ["pending", "Pendiente"],
      ["completed", "Completada"],
      ["cancelled", "Cancelada"],
    ]);
    expect(document.getElementById("factura-garantia-metodo-pago")).toBeNull();
    expect(document.getElementById("factura-garantia-ver-metodo")).toBeNull();
  });

  it("no hay dropzone y #factura-garantia-original es un select required con las ventas y servicios del cliente", async () => {
    getClientDetail.mockResolvedValue({ invoiceHistory: CLIENT_HISTORY });
    await initFacturasView();

    expect(document.getElementById("factura-garantia-dropzone")).toBeNull();
    const select = document.getElementById("factura-garantia-original");
    expect(select.tagName).toBe("SELECT");
    expect(select.required).toBe(true);

    await __testHooks.selectGuaranteeClient({ id: 7, fullName: "Cliente" });
    expect(getClientDetail).toHaveBeenCalledWith(7);
    // Solo ventas y servicios no cancelados; ni garantías ni insumos.
    expect(optionTexts(select)).toEqual([
      ["", "Selecciona la factura original"],
      ["1", "V-00001 · Venta"],
      ["2", "S-00001 · Servicio"],
    ]);
  });

  it("al cambiar de cliente el select se limpia", async () => {
    getClientDetail.mockResolvedValue({ invoiceHistory: CLIENT_HISTORY });
    await initFacturasView();
    await __testHooks.selectGuaranteeClient({ id: 7 });
    const select = document.getElementById("factura-garantia-original");
    select.value = "1";

    const cliente = document.getElementById("factura-garantia-cliente");
    cliente.value = "Otro";
    cliente.dispatchEvent(new Event("input", { bubbles: true }));
    expect(select.value).toBe("");
    expect(select.options).toHaveLength(1);
  });

  it("una respuesta tardía del cliente anterior no llena el select", async () => {
    let resolveFirst;
    getClientDetail.mockImplementationOnce(
      () => new Promise((resolve) => (resolveFirst = resolve)),
    );
    await initFacturasView();
    const pending = __testHooks.selectGuaranteeClient({ id: 7 });
    await __testHooks.selectGuaranteeClient(null);
    resolveFirst({ invoiceHistory: CLIENT_HISTORY });
    await pending;
    expect(
      document.getElementById("factura-garantia-original").options,
    ).toHaveLength(1);
  });

  it("#factura-garantia-ver-original muestra {numero} · {tipo visible} y el costo real", async () => {
    getInvoice.mockResolvedValue({
      id: 9,
      number: "GG-00001",
      invoiceType: "GG",
      status: "pending",
      contactFullName: "Cliente Uno",
      date: "2026-02-10",
      totalCents: 1234,
      associatedInvoice: { number: "S-00003", invoiceType: "S" },
      lines: [
        {
          productName: "Cable",
          quantity: "2",
          unitPriceCents: 617,
          subtotalCents: 1234,
        },
      ],
    });
    await initFacturasView();
    await __testHooks.openGuaranteeDetail(9);
    expect(
      document.getElementById("factura-garantia-ver-original").textContent,
    ).toBe("S-00003 · Servicio");
    expect(
      document.getElementById("factura-garantia-ver-productos").textContent,
    ).toContain("Cable");
    expect(getInvoice).toHaveBeenCalledWith(9);
  });

  it('el botón "Crear Nuevo Producto" y los inputs de descuento e impuesto no existen', async () => {
    await initFacturasView();
    const page = document.getElementById("page-facturas-garantia");
    expect(page.textContent).not.toContain("Crear Nuevo Producto");
    for (const id of [
      "factura-garantia-descuento",
      "factura-garantia-impuesto",
      "factura-garantia-ver-descuento",
      "factura-garantia-ver-impuestos",
    ]) {
      expect(document.getElementById(id), id).toBeNull();
    }
    expect(
      page.querySelector("[onclick],[oninput],[onchange],[onsubmit],[ondrop]"),
    ).toBeNull();
  });

  it("al guardar llama a createGuaranteeInvoice con cliente, factura original, fecha y líneas", async () => {
    getClientDetail.mockResolvedValue({ invoiceHistory: CLIENT_HISTORY });
    createGuaranteeInvoice.mockResolvedValue({
      invoiceId: 3,
      number: "GG-00003",
      expiredCleanupWarning: null,
    });
    await initFacturasView();

    const form = document.getElementById("factura-garantia-form");
    const submit = () =>
      form.dispatchEvent(
        new Event("submit", { bubbles: true, cancelable: true }),
      );

    // Sin factura original no se envía.
    await __testHooks.selectGuaranteeClient({ id: 7 });
    document.getElementById("factura-garantia-fecha-emision").value =
      "2026-02-10";
    __testHooks.setGuaranteeLines([{ productId: 11, quantity: "1.5" }]);
    submit();
    await Promise.resolve();
    expect(createGuaranteeInvoice).not.toHaveBeenCalled();
    expect(
      document.getElementById("factura-garantia-feedback").textContent,
    ).toContain("factura original");

    document.getElementById("factura-garantia-original").value = "2";
    submit();
    await vi.waitFor(() => {
      expect(createGuaranteeInvoice).toHaveBeenCalledWith({
        clientId: 7,
        associatedInvoiceId: 2,
        invoiceDate: "2026-02-10",
        status: "pending",
        lines: [{ productId: 11, quantity: "1.5" }],
      });
    });
  });

  it("el botón Editar del detalle solo se muestra si status es pending", async () => {
    const base = {
      id: 1,
      number: "GG-00001",
      invoiceType: "GG",
      date: "2026-02-10",
      totalCents: 100,
      lines: [],
    };
    getInvoice.mockResolvedValueOnce({ ...base, status: "pending" });
    await initFacturasView();
    await __testHooks.openGuaranteeDetail(1);
    expect(document.getElementById("factura-garantia-editar").hidden).toBe(
      false,
    );

    getInvoice.mockResolvedValueOnce({ ...base, status: "completed" });
    await __testHooks.openGuaranteeDetail(1);
    expect(document.getElementById("factura-garantia-editar").hidden).toBe(
      true,
    );
  });

  it("muestra un nombre de cliente con HTML como texto (salida segura)", async () => {
    const evil = '<img src=x onerror="alert(1)">Cliente';
    listInvoices.mockResolvedValue([
      {
        id: 1,
        number: "GG-00001",
        invoiceType: "GG",
        contactName: evil,
        date: "2026-02-10",
        totalCents: 100,
        status: "pending",
        associatedInvoice: null,
      },
    ]);
    getClientDetail.mockResolvedValue({
      invoiceHistory: [
        { id: 1, number: evil, invoiceType: "V", status: "pending" },
      ],
    });
    getInvoice.mockResolvedValue({
      id: 1,
      number: "GG-00001",
      invoiceType: "GG",
      status: "pending",
      contactFullName: evil,
      date: "2026-02-10",
      totalCents: 100,
      lines: [{ productName: evil, quantity: "1" }],
    });
    await initFacturasView();
    await __testHooks.selectGuaranteeClient({ id: 1 });
    await __testHooks.openGuaranteeDetail(1);
    const page = document.getElementById("page-facturas-garantia");
    expect(page.textContent).toContain("Cliente");
    expect(page.querySelector("img")).toBeNull();
  });
});
