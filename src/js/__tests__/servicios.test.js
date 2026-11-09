import { beforeAll, beforeEach, describe, expect, it, vi } from "vitest";
import { readFile } from "node:fs/promises";
import path from "node:path";
import {
  chooseFromCombobox,
  setValue,
  toastText,
} from "./helpers/invoiceUi.js";

vi.mock("../services/servicesService.js", () => ({
  listServices: vi.fn(),
  getService: vi.fn(),
  createService: vi.fn(),
  updateService: vi.fn(),
  deleteService: vi.fn(),
}));

vi.mock("../services/historyService.js", () => ({
  searchProductsForInvoice: vi.fn(),
}));

vi.mock("../components/confirmDialog.js", () => ({
  confirmDialog: vi.fn(() => Promise.resolve(true)),
}));

let catalog, history, initServiciosView;
let pageHtml = "";

beforeAll(async () => {
  vi.resetModules();
  catalog = await import("../services/servicesService.js");
  history = await import("../services/historyService.js");
  const html = await readFile(
    path.resolve(process.cwd(), "src", "dashboard-unificado.html"),
    "utf8",
  );
  pageHtml = new DOMParser()
    .parseFromString(html, "text/html")
    .getElementById("page-servicios").outerHTML;
});

const INSTALACION = {
  id: 11,
  name: "Instalación",
  description: "Montaje",
  suggestedPriceCents: 5000,
  products: [
    {
      productId: 7,
      name: "Cable UTP",
      unitAbbreviation: "m",
      suggestedQuantity: "3",
      status: "discontinued",
      sellingPriceCents: 250,
    },
  ],
};

beforeEach(async () => {
  document.body.innerHTML = `<main>${pageHtml}</main>`;
  vi.clearAllMocks();
  catalog.listServices.mockResolvedValue([
    {
      id: 11,
      name: "Instalación",
      description: "Montaje",
      suggestedPriceCents: 5000,
      productCount: 1,
    },
  ]);
  catalog.getService.mockResolvedValue(INSTALACION);
  history.searchProductsForInvoice.mockResolvedValue([
    { id: 7, name: "Cable UTP", unitAbbreviation: "m", isDiscontinued: false },
    { id: 9, name: "Router", unitAbbreviation: "u", isDiscontinued: true },
  ]);
  vi.resetModules();
  ({ initServiciosView } = await import("../views/serviciosView.js"));
  await initServiciosView();
});

describe("catálogo de servicios (spec-06 SV-01)", () => {
  it("lista los servicios con su precio sugerido y sus productos", () => {
    const body = document.getElementById("servicios-body").textContent;
    expect(body).toContain("Instalación");
    expect(body).toContain("50,00");
  });

  it("crea un servicio con productos y cantidades sugeridas", async () => {
    catalog.createService.mockResolvedValue({ ...INSTALACION, id: 12 });
    document.getElementById("tab-servicio-create").click();
    setValue("servicio-nombre", "Instalación");
    setValue("servicio-precio", "50");
    await chooseFromCombobox("servicio-buscar-producto", "ca", "Cable UTP");
    const quantity = document.querySelector(
      '#servicio-productos-body input[aria-label="Cantidad sugerida de Cable UTP"]',
    );
    quantity.value = "3";
    quantity.dispatchEvent(new Event("input", { bubbles: true }));
    document
      .getElementById("servicio-form")
      .dispatchEvent(new Event("submit", { bubbles: true, cancelable: true }));
    await vi.waitFor(() =>
      expect(catalog.createService).toHaveBeenCalledWith({
        name: "Instalación",
        description: null,
        suggestedPrice: "50",
        products: [{ productId: 7, quantity: "3" }],
      }),
    );
    await vi.waitFor(() => expect(toastText()).toContain("creado"));
  });

  it("la ficha marca el producto descontinuado y eliminar avisa que las facturas no cambian", async () => {
    document
      .querySelector('[data-servicio-action="view"][data-service-id="11"]')
      .click();
    await vi.waitFor(() =>
      expect(
        document.getElementById("servicio-ver-productos").textContent,
      ).toContain("Descontinuado"),
    );
    const { confirmDialog } = await import("../components/confirmDialog.js");
    catalog.deleteService.mockResolvedValue(null);
    document.querySelector('[data-servicio-detail-action="delete"]').click();
    await vi.waitFor(() =>
      expect(catalog.deleteService).toHaveBeenCalledWith(11),
    );
    expect(confirmDialog.mock.calls[0][0].message).toContain(
      "Las facturas que lo usaron no cambian",
    );
  });
});
