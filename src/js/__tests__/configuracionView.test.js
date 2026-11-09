import { readFile } from "node:fs/promises";
import path from "node:path";
import {
  afterEach,
  beforeAll,
  beforeEach,
  describe,
  expect,
  it,
  vi,
} from "vitest";
import { toastText } from "./helpers/invoiceUi.js";
import { currenciesWithoutRate, equivalents } from "../utils/currency.js";

const settings = vi.hoisted(() => ({
  getSettings: vi.fn(),
  updateSettings: vi.fn(),
  getUserPreferences: vi.fn(),
  updateUserPreferences: vi.fn(),
  listExchangeRates: vi.fn(),
  updateExchangeRate: vi.fn(),
  listExchangeRateHistory: vi.fn(),
  getCompanyProfile: vi.fn(),
  updateCompanyProfile: vi.fn(),
  saveCompanyLogo: vi.fn(),
}));

vi.mock("../services/settingsService.js", () => settings);

const RATES = [
  {
    currency: "CUP",
    rate: "670",
    updatedAt: "2026-11-04T12:00:00Z",
    updatedByName: "ana",
  },
  {
    currency: "EUR",
    rate: "0,925501",
    updatedAt: "2026-11-04T12:00:00Z",
    updatedByName: "luis",
  },
];
const HISTORY = [
  {
    createdAt: "2026-11-04T12:00:00Z",
    currency: "CUP",
    previousRate: "650",
    newRate: "670",
    userName: "ana",
  },
  {
    createdAt: "2026-11-01T12:00:00Z",
    currency: "CUP",
    previousRate: null,
    newRate: "650",
    userName: "luis",
  },
];
const COMPANY = {
  name: "Goyma Conexiones",
  address: "Calle 23, Vedado",
  phone: "+53 7 8300000",
  taxId: "",
  logoDataUrl: "data:image/jpeg;base64,/9j/",
};

let pageHtml = "";
let view;

beforeAll(async () => {
  const html = await readFile(
    path.resolve(process.cwd(), "src", "dashboard-unificado.html"),
    "utf8",
  );
  pageHtml = new DOMParser()
    .parseFromString(html, "text/html")
    .getElementById("page-configuracion").outerHTML;
});

const byId = (id) => document.getElementById(id);
const historyRows = () =>
  [...byId("historial-tasas-body").querySelectorAll("tr")].map((row) =>
    [...row.cells].map((cell) => cell.textContent),
  );

async function openView() {
  vi.resetModules();
  document.body.innerHTML = pageHtml;
  view = await import("../views/configuracionView.js");
  await view.initConfiguracionView();
  await view.__testHooks.loadConfiguration();
}

describe("utilidades de monedas (CF-01)", () => {
  it("convierte con la tasa actual, redondea a 2 decimales y omite las monedas sin tasa", () => {
    expect(equivalents(1_250, ["CUP", "EUR", "MLC"], RATES)).toEqual([
      "8.375,00 CUP",
      "11,57 EUR",
    ]);
    expect(currenciesWithoutRate(["CUP", "MLC", "SALDO"], RATES)).toEqual([
      "MLC",
      "SALDO",
    ]);
    expect(equivalents(1_000, [], RATES)).toEqual([]);
  });
});

describe("Configuración con datos reales (spec-04 CF-01 a CF-04)", () => {
  beforeEach(() => {
    Object.values(settings).forEach((mock) => mock.mockReset());
    settings.getSettings.mockResolvedValue({
      staleInvoiceDays: 30,
      backupReminderDays: 7,
      lastBackupAt: null,
    });
    settings.getUserPreferences.mockResolvedValue({
      visibleCurrencies: ["CUP", "MLC"],
      pageSize: 50,
    });
    settings.listExchangeRates.mockResolvedValue(RATES);
    settings.listExchangeRateHistory.mockResolvedValue(HISTORY);
    settings.getCompanyProfile.mockResolvedValue(COMPANY);
    window.establecerTamPagina = vi.fn();
  });

  afterEach(() => {
    delete window.establecerTamPagina;
    document.body.innerHTML = "";
  });

  it("pinta empresa, monedas, tasas, historial, umbrales y tamaño de página; sin 'Datos de Factura' ni demos", async () => {
    await openView();

    expect(byId("config-empresa-nombre").value).toBe("Goyma Conexiones");
    expect(byId("config-empresa-direccion").value).toBe("Calle 23, Vedado");
    expect(byId("config-logo-preview").hidden).toBe(false);
    expect(byId("config-logo-preview").getAttribute("src")).toBe(
      COMPANY.logoDataUrl,
    );
    expect(byId("config-logo-quitar").hidden).toBe(false);
    expect(
      document.querySelector("#page-configuracion .invoice-panel"),
    ).toBeNull();
    expect(byId("page-configuracion").outerHTML).not.toMatch(
      /placehold\.co|data-action="avisoDemo"/,
    );

    const switches = [
      ...document.querySelectorAll("#currency-grid [data-currency]"),
    ];
    expect(
      switches.map((input) => [input.dataset.currency, input.checked]),
    ).toEqual([
      ["EUR", false],
      ["CUP", true],
      ["MLC", true],
      ["SALDO", false],
    ]);
    expect(byId("config-moneda-aviso").hidden).toBe(false);
    expect(byId("config-moneda-aviso").textContent).toContain(
      "Sin tasa de cambio: MLC",
    );

    expect(byId("config-tasa-CUP").value).toBe("670");
    expect(byId("config-tasa-MLC").value).toBe("");
    expect(byId("rate-cards").textContent).toContain("1 USD = 670 CUP");

    expect(historyRows().map((row) => row.slice(1))).toEqual([
      ["CUP", "1 USD = 650 CUP", "1 USD = 670 CUP", "USD", "ana"],
      ["CUP", "—", "1 USD = 650 CUP", "USD", "luis"],
    ]);
    byId("th-fecha-historial-tasas").click();
    expect(historyRows()[0][2]).toBe("—");

    // spec-06 UM-01: el umbral de pocas unidades es de cada producto.
    expect(byId("config-umbral-stock")).toBeNull();
    expect(byId("config-umbral-respaldo").value).toBe("7");
    expect(byId("config-elementos-pagina").value).toBe("50");
    expect(window.establecerTamPagina).toHaveBeenCalledWith(50);
  });

  it("guarda solo las tasas cambiadas, las monedas del usuario, los umbrales y el tamaño de página", async () => {
    settings.updateExchangeRate.mockResolvedValue({});
    settings.updateUserPreferences.mockImplementation((input) =>
      Promise.resolve(input),
    );
    settings.updateSettings.mockResolvedValue({});
    await openView();

    byId("config-tasa-MLC").value = "120";
    byId("config-tasas-guardar").click();
    await vi.waitFor(() =>
      expect(settings.updateExchangeRate).toHaveBeenCalledTimes(1),
    );
    expect(settings.updateExchangeRate).toHaveBeenCalledWith("MLC", "120");
    await vi.waitFor(() =>
      expect(toastText()).toContain("Tasa de cambio guardada."),
    );

    document.querySelector('#currency-grid [data-currency="EUR"]').checked =
      true;
    byId("config-moneda-guardar").click();
    await vi.waitFor(() =>
      expect(settings.updateUserPreferences).toHaveBeenCalledWith({
        visibleCurrencies: ["EUR", "CUP", "MLC"],
        pageSize: 50,
      }),
    );

    byId("config-umbral-facturas").value = "45";
    byId("config-umbrales-form").dispatchEvent(
      new Event("submit", { cancelable: true }),
    );
    await vi.waitFor(() =>
      expect(settings.updateSettings).toHaveBeenCalledWith({
        staleInvoiceDays: 45,
        backupReminderDays: 7,
      }),
    );

    const pageSize = byId("config-elementos-pagina");
    pageSize.value = "10";
    pageSize.dispatchEvent(new Event("change", { bubbles: true }));
    await vi.waitFor(() =>
      expect(window.establecerTamPagina).toHaveBeenLastCalledWith(10),
    );
    expect(settings.updateUserPreferences).toHaveBeenLastCalledWith({
      visibleCurrencies: ["EUR", "CUP", "MLC"],
      pageSize: 10,
    });
  });

  it("un rechazo del backend marca el campo y sale como toast", async () => {
    settings.updateSettings.mockRejectedValue({
      code: "validationFailed",
      message:
        "Los días de factura pendiente antigua deben estar entre 1 y 365",
    });
    await openView();
    byId("config-umbral-facturas").value = "0";
    byId("config-umbrales-form").dispatchEvent(
      new Event("submit", { cancelable: true }),
    );
    await vi.waitFor(() =>
      expect(byId("config-umbral-facturas").getAttribute("aria-invalid")).toBe(
        "true",
      ),
    );
    expect(toastText()).toContain("Los días de factura pendiente antigua");
  });

  it("guarda los datos de la empresa y quita el logo", async () => {
    settings.updateCompanyProfile.mockResolvedValue({
      ...COMPANY,
      taxId: "123",
    });
    settings.saveCompanyLogo.mockResolvedValue({
      ...COMPANY,
      logoDataUrl: null,
    });
    await openView();

    byId("config-empresa-nit").value = "123";
    byId("config-empresa-form").dispatchEvent(
      new Event("submit", { cancelable: true }),
    );
    await vi.waitFor(() =>
      expect(settings.updateCompanyProfile).toHaveBeenCalledWith({
        name: "Goyma Conexiones",
        address: "Calle 23, Vedado",
        phone: "+53 7 8300000",
        taxId: "123",
      }),
    );

    byId("config-logo-quitar").click();
    await vi.waitFor(() =>
      expect(settings.saveCompanyLogo).toHaveBeenCalledWith(null),
    );
    await vi.waitFor(() =>
      expect(byId("config-logo-preview").hidden).toBe(true),
    );
    expect(byId("config-logo-empty").hidden).toBe(false);
  });

  it("rechaza un archivo que no es PNG ni JPG sin llamar al backend", async () => {
    await openView();
    const input = byId("config-logo-input");
    Object.defineProperty(input, "files", {
      value: [new File(["hola"], "logo.gif", { type: "image/gif" })],
      configurable: true,
    });
    input.dispatchEvent(new Event("change", { bubbles: true }));
    await vi.waitFor(() =>
      expect(toastText()).toContain(
        "El logo debe ser una imagen PNG o JPG válida",
      ),
    );
    expect(settings.saveCompanyLogo).not.toHaveBeenCalled();
  });
});
