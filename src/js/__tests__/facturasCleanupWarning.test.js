import { beforeAll, beforeEach, describe, expect, it, vi } from "vitest";
import { readFile } from "node:fs/promises";
import path from "node:path";
import { chooseFromCombobox } from "./helpers/invoiceUi.js";

vi.mock("../services/invoicesService.js", () => ({
  listInvoices: vi.fn(),
  getInvoice: vi.fn(),
  createEntryInvoice: vi.fn(),
  createSaleInvoice: vi.fn(),
  createServiceInvoice: vi.fn(),
  createGuaranteeInvoice: vi.fn(),
  createInstallationInvoice: vi.fn(),
  createLossInvoice: vi.fn(),
  updateEntryInvoice: vi.fn(),
  updateSaleInvoice: vi.fn(),
  updateServiceInvoice: vi.fn(),
  updateGuaranteeInvoice: vi.fn(),
  updateInstallationInvoice: vi.fn(),
  saveInvoiceAttachment: vi.fn(),
  downloadInvoiceAttachment: vi.fn(),
  deleteInvoice: vi.fn(),
  changeInvoiceStatus: vi.fn(),
}));

vi.mock("../services/clientsService.js", () => ({
  searchClients: vi.fn(() => Promise.resolve([{ id: 4, fullName: "Ana" }])),
  getClientDetail: vi.fn(),
}));

vi.mock("../services/suppliersService.js", () => ({
  searchSuppliers: vi.fn(() => Promise.resolve([{ id: 1, fullName: "Ana" }])),
}));

vi.mock("../services/historyService.js", () => ({
  searchProductsForInvoice: vi.fn(() =>
    Promise.resolve([{ id: 1, name: "Cable", stock: "5" }]),
  ),
}));

vi.mock("../components/confirmDialog.js", () => ({
  confirmDialog: vi.fn(() => Promise.resolve(true)),
}));

// Con isolate:false los módulos se comparten entre archivos de test: se
// recarga la vista para que use los mocks de este archivo.
let services, getClientDetail, __testHooks, initFacturasView;
let mainHtml = "";

const WARNING =
  "No se pudieron limpiar 2 productos vencidos; se reintentará en la próxima operación";

beforeAll(async () => {
  vi.resetModules();
  services = await import("../services/invoicesService.js");
  ({ getClientDetail } = await import("../services/clientsService.js"));
  ({ __testHooks, initFacturasView } =
    await import("../views/facturasView.js"));
  const html = await readFile(
    path.resolve(process.cwd(), "src", "dashboard-unificado.html"),
    "utf8",
  );
  mainHtml = Array.from(
    new DOMParser()
      .parseFromString(html, "text/html")
      .querySelectorAll('[id^="page-facturas-"]'),
  )
    .map((page) => page.outerHTML)
    .join("");
});

const setValue = (id, value) => {
  document.getElementById(id).value = value;
};
const submit = (formId) =>
  document
    .getElementById(formId)
    .dispatchEvent(new Event("submit", { bubbles: true, cancelable: true }));

// Por submódulo: tipo, cuerpo del listado, id del aviso, comando de alta y
// cómo dejar el formulario listo para guardar.
// Elige contacto y producto con los buscadores y teclea la cantidad.
async function fillLines(contactInput, productInput, linesBody, isEntry) {
  await chooseFromCombobox(contactInput, "ana", "Ana");
  await chooseFromCombobox(productInput, "cab", "Cable");
  for (const [field, value] of [
    ["quantity", "1"],
    ...(isEntry ? [["unitPrice", "1"]] : []),
  ]) {
    const input = document.querySelector(
      `#${linesBody} [data-line-field="${field}"]`,
    );
    input.value = value;
    input.dispatchEvent(new Event("input", { bubbles: true }));
  }
}

async function chooseAssociated(selectId, value) {
  await vi.waitFor(() =>
    expect(
      document.querySelector(`#${selectId} option[value="${value}"]`),
    ).not.toBeNull(),
  );
  setValue(selectId, value);
}

// Por submódulo: tipo, cuerpo del listado, id del aviso, comando de alta y
// cómo dejar el formulario listo para guardar.
const SUBMODULES = [
  {
    name: "entry",
    type: "E",
    bodyId: "facturas-entrada-body",
    warningId: "facturas-entrada-cleanup-warning",
    create: "createEntryInvoice",
    prepare: async () => {
      await fillLines(
        "factura-proveedor",
        "factura-buscar-producto",
        "factura-lineas-body",
        true,
      );
      setValue("factura-fecha-emision", "2026-02-10");
      submit("factura-entrada-form");
    },
  },
  {
    name: "sale",
    type: "V",
    bodyId: "facturas-venta-body",
    warningId: "facturas-venta-cleanup-warning",
    create: "createSaleInvoice",
    prepare: async () => {
      await fillLines(
        "factura-venta-cliente",
        "factura-venta-buscar-producto",
        "factura-venta-lineas-body",
      );
      setValue("factura-venta-fecha-emision", "2026-02-10");
      submit("factura-venta-form");
    },
  },
  {
    name: "service",
    type: "S",
    bodyId: "facturas-servicios-body",
    warningId: "facturas-servicios-cleanup-warning",
    create: "createServiceInvoice",
    prepare: async () => {
      await fillLines(
        "factura-servicio-cliente",
        "factura-servicio-buscar",
        "factura-servicio-lineas-body",
      );
      setValue("factura-servicio-fecha-emision", "2026-02-10");
      submit("factura-servicios-form");
    },
  },
  {
    name: "guarantee",
    type: "GG",
    bodyId: "facturas-garantia-body",
    warningId: "facturas-garantia-cleanup-warning",
    create: "createGuaranteeInvoice",
    prepare: async () => {
      await fillLines(
        "factura-garantia-cliente",
        "factura-garantia-buscar",
        "factura-garantia-lineas-body",
      );
      await chooseAssociated("factura-garantia-original", "9");
      setValue("factura-garantia-fecha-emision", "2026-02-10");
      submit("factura-garantia-form");
    },
  },
  {
    name: "installation",
    type: "GI",
    bodyId: "facturas-insumos-body",
    warningId: "facturas-insumos-cleanup-warning",
    create: "createInstallationInvoice",
    prepare: async () => {
      await fillLines(
        "factura-insumo-cliente",
        "factura-insumo-buscar",
        "factura-insumo-lineas-body",
      );
      await chooseAssociated("factura-insumo-servicio", "8");
      setValue("factura-insumo-fecha-emision", "2026-02-10");
      submit("factura-insumos-form");
    },
  },
  {
    name: "loss",
    type: "P",
    bodyId: "facturas-perdidas-body",
    warningId: "facturas-perdidas-cleanup-warning",
    create: "createLossInvoice",
    prepare: async () => {
      await chooseFromCombobox("factura-perdida-producto", "cab", "Cable");
      setValue("factura-perdida-cantidad", "1");
      setValue("factura-perdida-fecha", "2026-02-10");
      submit("factura-perdidas-form");
    },
  },
];

function listRow(type) {
  return {
    id: 5,
    number: `${type}-00005`,
    invoiceType: type,
    contactName: "Cliente",
    date: "2026-02-10",
    totalCents: 100,
    status: type === "P" ? null : "pending",
  };
}

describe("expiredCleanupWarning en las facturas (H-7)", () => {
  beforeEach(async () => {
    document.body.innerHTML = `<main>${mainHtml}</main>`;
    vi.clearAllMocks();
    window.showPage = vi.fn();
    services.getInvoice.mockResolvedValue({
      id: 5,
      number: "X-00005",
      date: "2026-02-10",
      totalCents: 100,
      lines: [],
    });
    getClientDetail.mockResolvedValue({
      invoiceHistory: [
        { id: 9, number: "V-00009", invoiceType: "V", status: "pending" },
        { id: 8, number: "S-00008", invoiceType: "S", status: "pending" },
      ],
    });
  });

  it.each(SUBMODULES)(
    "$name: el aviso se muestra tras crear y eliminar, y se limpia con null",
    async (submodule) => {
      services.listInvoices.mockImplementation((filters) =>
        Promise.resolve(
          filters?.invoiceType === submodule.type
            ? [listRow(submodule.type)]
            : [],
        ),
      );
      await initFacturasView();
      const status = () => document.getElementById(submodule.warningId);

      // Crear con aviso.
      services[submodule.create].mockResolvedValue({
        invoiceId: 5,
        number: `${submodule.type}-00005`,
        expiredCleanupWarning: WARNING,
      });
      await submodule.prepare();
      await vi.waitFor(() => expect(status()?.textContent).toBe(WARNING));
      expect(status().getAttribute("role")).toBe("status");
      expect(status().hidden).toBe(false);

      // Eliminar con aviso y luego sin él: el null lo limpia.
      const deleteButton = () =>
        Array.from(
          document.getElementById(submodule.bodyId).querySelectorAll("button"),
        ).find((button) => button.textContent === "Eliminar");
      services.deleteInvoice.mockResolvedValueOnce({
        invoiceId: 5,
        number: `${submodule.type}-00005`,
        expiredCleanupWarning: WARNING,
      });
      await vi.waitFor(() => expect(deleteButton()).toBeTruthy());
      status().textContent = "";
      deleteButton().click();
      await vi.waitFor(() => expect(status().textContent).toBe(WARNING));

      services.deleteInvoice.mockResolvedValueOnce({
        invoiceId: 5,
        number: `${submodule.type}-00005`,
        expiredCleanupWarning: null,
      });
      await vi.waitFor(() => expect(deleteButton()).toBeTruthy());
      deleteButton().click();
      await vi.waitFor(() => expect(status().textContent).toBe(""));
      expect(status().hidden).toBe(true);
    },
  );
});
