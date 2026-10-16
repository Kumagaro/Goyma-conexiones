import { beforeAll, beforeEach, describe, expect, it, vi } from "vitest";
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
let getInvoice, listInvoices, initFacturasView;
let mainHtml = "";
let dashboardSource = "";

const EVIL = '<img src=x onerror="alert(1)">Malicioso';

// Las 12 vistas de facturas de spec-03 §9 que antes usaban innerHTML.
const LIST_VIEWS = [
  ["renderFacturasEntrada", "facturas-entrada-body", "E"],
  ["renderFacturasVenta", "facturas-venta-body", "V"],
  ["renderFacturasServicios", "facturas-servicios-body", "S"],
  ["renderFacturasInsumos", "facturas-insumos-body", "GI"],
  ["renderFacturasGarantia", "facturas-garantia-body", "GG"],
  ["renderFacturasPerdidas", "facturas-perdidas-body", "P"],
  ["renderFacturasGeneral", "facturas-general-body", "V"],
];
const DETAIL_VIEWS = [
  ["verFactura", "factura-entrada-ver", "E"],
  ["verFacturaVenta", "factura-venta-ver", "V"],
  ["verFacturaServicio", "factura-servicios-ver", "S"],
  ["verFacturaInsumo", "factura-insumos-ver", "GI"],
  ["verFacturaGarantia", "factura-garantia-ver", "GG"],
];

function evilRow(type) {
  return {
    id: 1,
    number: "X-00001",
    invoiceType: type,
    contactName: EVIL,
    date: "2026-02-10",
    totalCents: 100,
    status: type === "P" ? null : "pending",
    associatedInvoice: { number: EVIL, invoiceType: "S" },
    productName: EVIL,
    quantity: "1",
  };
}

function evilDetail(type) {
  return {
    id: 1,
    number: "X-00001",
    invoiceType: type,
    status: "pending",
    contactFullName: EVIL,
    date: "2026-02-10",
    totalCents: 100,
    notes: EVIL,
    associatedInvoice: { number: EVIL, invoiceType: "S" },
    attachmentOriginalName: EVIL,
    lines: [
      {
        productName: EVIL,
        quantity: "1",
        unitAbbreviation: EVIL,
        unitPriceCents: 100,
        subtotalCents: 100,
      },
    ],
  };
}

beforeAll(async () => {
  vi.resetModules();
  ({ getInvoice, listInvoices } =
    await import("../services/invoicesService.js"));
  ({ initFacturasView } = await import("../views/facturasView.js"));
  const root = path.resolve(process.cwd(), "src");
  const html = await readFile(
    path.join(root, "dashboard-unificado.html"),
    "utf8",
  );
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

describe("vistas de facturas con textContent (XSS)", () => {
  beforeEach(async () => {
    document.body.innerHTML = `<main>${mainHtml}</main>`;
    vi.clearAllMocks();
    listInvoices.mockResolvedValue([]);
    window.showPage = vi.fn();
    await initFacturasView();
  });

  it.each(LIST_VIEWS)(
    "%s pinta un nombre con HTML como texto",
    async (name, bodyId, type) => {
      listInvoices.mockResolvedValue([evilRow(type)]);
      window[name]();
      const body = document.getElementById(bodyId);
      await vi.waitFor(() => expect(body.textContent).toContain("Malicioso"));
      expect(body.textContent).toContain("<img");
      expect(body.querySelector("img")).toBeNull();
    },
  );

  it.each(DETAIL_VIEWS)(
    "%s pinta un nombre con HTML como texto",
    async (name, viewId, type) => {
      getInvoice.mockResolvedValue(evilDetail(type));
      window[name](1);
      const view = document.getElementById(viewId);
      await vi.waitFor(() =>
        expect(view.classList.contains("active")).toBe(true),
      );
      expect(view.textContent).toContain("Malicioso");
      expect(view.querySelector("img")).toBeNull();
    },
  );

  it("dashboard.js ya no define las vistas de facturas con innerHTML", () => {
    for (const [name] of [...LIST_VIEWS, ...DETAIL_VIEWS]) {
      expect(dashboardSource, name).not.toMatch(
        new RegExp(`function ${name}\\(`),
      );
    }
  });
});
