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

vi.mock("../services/invoke.js", () => ({
  invoke: vi.fn(),
}));

vi.mock("../components/confirmDialog.js", () => ({
  confirmDialog: vi.fn(() => Promise.resolve(true)),
}));

// Con isolate:false los módulos se comparten entre archivos de test: se
// recarga la vista para que use los mocks de este archivo.
let createLossInvoice, getInvoice, listInvoices, invoke;
let __testHooks, initFacturasView;
let pageHtml = "";

beforeAll(async () => {
  vi.resetModules();
  ({ createLossInvoice, getInvoice, listInvoices } =
    await import("../services/invoicesService.js"));
  ({ invoke } = await import("../services/invoke.js"));
  ({ __testHooks, initFacturasView } =
    await import("../views/facturasView.js"));
  const html = await readFile(
    path.resolve(process.cwd(), "src", "dashboard-unificado.html"),
    "utf8",
  );
  pageHtml = new DOMParser()
    .parseFromString(html, "text/html")
    .getElementById("page-facturas-perdidas").outerHTML;
});

const LOSS_DETAIL = {
  id: 5,
  number: "P-00005",
  invoiceType: "P",
  status: null,
  date: "2026-02-10",
  totalCents: 2250,
  notes: "Se mojó",
  lines: [
    {
      productName: "Cable",
      quantity: "1,5",
      unitAbbreviation: "m",
      unitPriceCents: 1500,
      subtotalCents: 2250,
    },
  ],
};

describe("facturasView submódulo loss (HTML de producción)", () => {
  beforeEach(() => {
    document.body.innerHTML = `<main>${pageHtml}</main>`;
    vi.clearAllMocks();
    listInvoices.mockResolvedValue([]);
  });

  afterEach(() => {
    document.body.innerHTML = "";
    delete window.exportarFacturaPerdidaPDF;
    delete window.__facturasLossPageChangeBound;
  });

  it("pinta producto, cantidad y costo total del backend con estados de carga, vacío y error", async () => {
    const body = () => document.getElementById("facturas-perdidas-body");
    const initPromise = initFacturasView();
    expect(body().textContent).toContain("Cargando");
    await initPromise;
    expect(listInvoices).toHaveBeenCalledWith({ invoiceType: "P" });
    expect(body().textContent).toContain("Aún no hay facturas de pérdidas");

    listInvoices.mockResolvedValueOnce([
      {
        id: 5,
        number: "P-00005",
        invoiceType: "P",
        contactName: null,
        date: "2026-02-10",
        totalCents: 2250,
        status: null,
        associatedInvoice: null,
        productName: "Cable",
        quantity: "1,5",
      },
    ]);
    await initFacturasView();
    const cells = Array.from(body().querySelectorAll("td")).map(
      (td) => td.textContent,
    );
    expect(cells.slice(0, 3)).toEqual(["P-00005", "Cable", "1,5"]);

    listInvoices.mockRejectedValueOnce(new Error("falla"));
    await initFacturasView();
    expect(body().textContent).toContain("No se pudieron cargar");
  });

  it("#factura-perdida-costo es un input de texto de solo lectura y #factura-perdida-cantidad es decimal", async () => {
    await initFacturasView();
    const costo = document.getElementById("factura-perdida-costo");
    expect(costo.tagName).toBe("INPUT");
    expect(costo.type).toBe("text");
    expect(costo.readOnly).toBe(true);
    expect(costo.required).toBe(false);
    const cantidad = document.getElementById("factura-perdida-cantidad");
    expect(cantidad.type).toBe("text");
    expect(cantidad.getAttribute("inputmode")).toBe("decimal");
  });

  it("#factura-perdida-producto es un combobox sin la opción Agregar nueva", async () => {
    invoke.mockResolvedValue([
      { id: 3, name: "Cable", stock: "4", unitAbbreviation: "m" },
    ]);
    await initFacturasView();
    const input = document.getElementById("factura-perdida-producto");
    expect(input.getAttribute("role")).toBe("combobox");

    input.value = "cab";
    input.dispatchEvent(new Event("input", { bubbles: true }));
    const listbox = document.getElementById("factura-perdida-producto-listbox");
    await vi.waitFor(() => expect(listbox.hidden).toBe(false));
    // La pérdida admite descontinuados con stock (FP-01, S-22).
    expect(invoke).toHaveBeenCalledWith("search_products_for_invoice", {
      query: "cab",
      purpose: "loss",
    });
    expect(listbox.textContent).toContain("Cable · stock 4 m");
    expect(listbox.querySelector("[data-combobox-add]")).toBeNull();
  });

  it("el formulario solo pide producto, cantidad, fecha y descripción", async () => {
    await initFacturasView();
    const form = document.getElementById("factura-perdidas-form");
    const editable = Array.from(
      form.querySelectorAll("input, select, textarea"),
    )
      .filter((el) => !el.readOnly)
      .map((el) => el.id);
    expect(editable).toEqual([
      "factura-perdida-producto",
      "factura-perdida-cantidad",
      "factura-perdida-fecha",
      "factura-perdida-descripcion",
    ]);
    const page = document.getElementById("page-facturas-perdidas");
    expect(
      page.querySelector("[onclick],[oninput],[onchange],[onsubmit],datalist"),
    ).toBeNull();
    expect(page.textContent).not.toContain("Editar");
  });

  it("al guardar envía producto, cantidad, fecha y descripción sin costo y abre el detalle", async () => {
    createLossInvoice.mockResolvedValue({
      invoiceId: 5,
      number: "P-00005",
      expiredCleanupWarning: null,
    });
    getInvoice.mockResolvedValue(LOSS_DETAIL);
    await initFacturasView();

    const form = document.getElementById("factura-perdidas-form");
    const submit = () =>
      form.dispatchEvent(
        new Event("submit", { bubbles: true, cancelable: true }),
      );
    document.getElementById("factura-perdida-fecha").value = "2026-02-10";
    document.getElementById("factura-perdida-cantidad").value = "1.5";
    document.getElementById("factura-perdida-descripcion").value = "Se mojó";

    // Sin producto elegido no se envía.
    submit();
    await Promise.resolve();
    expect(createLossInvoice).not.toHaveBeenCalled();
    expect(
      document.getElementById("factura-perdidas-feedback").textContent,
    ).toContain("producto");

    __testHooks.getLossCombobox().setSelected({ id: 3, name: "Cable" });
    submit();
    await vi.waitFor(() => expect(getInvoice).toHaveBeenCalledWith(5));
    expect(createLossInvoice).toHaveBeenCalledWith({
      productId: 3,
      quantity: "1.5",
      invoiceDate: "2026-02-10",
      notes: "Se mojó",
    });
    expect(
      document
        .getElementById("factura-perdidas-ver")
        .classList.contains("active"),
    ).toBe(true);
  });

  it("el detalle llena #factura-perdida-ver-costo-total y #factura-perdida-ver-costo con el costo real", async () => {
    getInvoice.mockResolvedValue(LOSS_DETAIL);
    await initFacturasView();
    await __testHooks.openLossDetail(5);
    const text = (id) => document.getElementById(id).textContent;
    expect(text("factura-perdida-ver-costo-total")).toContain("22");
    expect(text("factura-perdida-ver-costo-total")).not.toBe(
      text("factura-perdida-ver-costo"),
    );
    expect(text("factura-perdida-ver-costo")).toContain("15");
    expect(text("factura-perdida-ver-cantidad")).toBe("1,5 m");
    expect(text("factura-perdida-ver-producto")).toBe("Cable");
    expect(text("factura-perdida-ver-titulo")).toBe(
      "Factura de Pérdida P-00005",
    );
  });

  it("muestra un nombre de producto con HTML como texto (salida segura)", async () => {
    const evil = '<img src=x onerror="alert(1)">Cable';
    listInvoices.mockResolvedValue([
      {
        id: 5,
        number: "P-00005",
        invoiceType: "P",
        date: "2026-02-10",
        totalCents: 100,
        productName: evil,
        quantity: "1",
      },
    ]);
    getInvoice.mockResolvedValue({
      ...LOSS_DETAIL,
      notes: evil,
      lines: [{ ...LOSS_DETAIL.lines[0], productName: evil }],
    });
    await initFacturasView();
    await __testHooks.openLossDetail(5);
    const page = document.getElementById("page-facturas-perdidas");
    expect(page.textContent).toContain("Cable");
    expect(page.querySelector("img")).toBeNull();
  });
});
