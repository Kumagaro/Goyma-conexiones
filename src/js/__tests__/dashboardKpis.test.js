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

const invokeMock = vi.hoisted(() => vi.fn());

const series = (value, previous, trend = [value]) => ({
  value,
  previous,
  trend,
});

const KPIS = {
  fromDate: "2026-11-01",
  toDate: "2026-11-18",
  income: series(1300000, 1000000, [500000, 800000]),
  sales: series(12, 16, [4, 8]),
  dailySalesMilli: series(1500, 1500),
  dailySalesAmountCents: 44400,
  netProfit: series(550000, 0),
  marginHundredths: series(4692, 4000),
  grossMarginCents: 610000,
  averageProfitHundredths: 5667,
  expenses: series(60000, 30000),
  inventoryValue: series(400000, null, []),
  outOfStock: series(3, null, []),
  inventoryQuantity: "3,5",
  buckets: [
    { label: "01/11", salesCents: 300000, servicesCents: 100000 },
    { label: "02/11", salesCents: 500000, servicesCents: 400000 },
  ],
  guaranteeCents: 30000,
  installationCents: 20000,
  lossCents: 10000,
  bestSellers: Array.from({ length: 7 }, (_, index) => ({
    name: `Producto ${index + 1}`,
    quantity: String(20 - index),
    marginCents: 1000 * (index + 1),
  })),
  frequentClients: [
    { name: "Ana Pérez", purchases: 2, totalCents: 1200000 },
    { name: "Luis Gómez", purchases: 1, totalCents: 100000 },
  ],
  starProducts: [{ name: "Bomba", percent: 82 }],
  starClients: [{ name: "Ana Pérez", percent: 92 }],
  slowMovers: [
    {
      name: '<img src=x onerror="alert(1)">Nunca',
      sold: "0",
      sellingPriceCents: 600,
      daysWithoutSale: 46,
    },
  ],
  reservations: {
    count: 2,
    amountCents: 10099,
    expiring: [
      {
        id: 7,
        number: "V-00004",
        invoiceType: "V",
        contactName: "Ana Pérez",
        reservedUntil: "2026-11-20",
      },
    ],
  },
};

const SUMMARY = {
  outOfStockTotal: 3,
  topCategories: [],
  withoutCategoryCount: 0,
};

function respond(overrides = {}) {
  invokeMock.mockImplementation((command) => {
    if (command in overrides) {
      const value = overrides[command];
      return value instanceof Error
        ? Promise.reject(value)
        : Promise.resolve(value);
    }
    switch (command) {
      case "get_dashboard_kpis":
        return Promise.resolve(KPIS);
      case "get_dashboard_summary":
        return Promise.resolve(SUMMARY);
      case "get_user_preferences":
        return Promise.resolve({ visibleCurrencies: ["CUP"], pageSize: 25 });
      case "list_exchange_rates":
        return Promise.resolve([{ currency: "CUP", rate: "100" }]);
      default:
        return Promise.resolve(null);
    }
  });
}

const kpiCalls = () =>
  invokeMock.mock.calls.filter(([command]) => command === "get_dashboard_kpis");
const text = (id) => document.getElementById(id).textContent.trim();
const card = (key) => document.querySelector(`[data-kpi="${key}"]`);
const part = (key, role) =>
  card(key).querySelector(`[data-role="${role}"]`).textContent.trim();
const periodButton = (value) =>
  document.querySelector(`#page-dashboard [data-periodo="${value}"]`);

describe("dashboard con datos reales (HTML de producción)", () => {
  let pageHtml = "";
  let chartInstances;

  beforeAll(async () => {
    const html = await readFile(
      path.resolve(process.cwd(), "src", "dashboard-unificado.html"),
      "utf8",
    );
    pageHtml = new DOMParser()
      .parseFromString(html, "text/html")
      .getElementById("page-dashboard").outerHTML;
  });

  async function start() {
    vi.resetModules();
    vi.doMock("../services/invoke.js", () => ({ invoke: invokeMock }));
    const view = await import("../views/dashboardView.js");
    await view.initDashboardView();
    await vi.waitFor(() => expect(kpiCalls().length).toBeGreaterThan(0));
    await vi.waitFor(() =>
      expect(
        document.getElementById("page-dashboard").hasAttribute("aria-busy"),
      ).toBe(false),
    );
  }

  beforeEach(() => {
    document.body.innerHTML = `<main>${pageHtml}</main>`;
    localStorage.clear();
    vi.clearAllMocks();
    respond();
    chartInstances = [];
    window.Chart = class {
      static defaults = { font: {} };
      constructor(_context, config) {
        this.data = config.data;
        this.update = vi.fn();
        this.visible = new Map();
        chartInstances.push(this);
      }
      isDatasetVisible(index) {
        return this.visible.get(index) ?? true;
      }
      setDatasetVisibility(index, value) {
        this.visible.set(index, value);
      }
    };
    HTMLCanvasElement.prototype.getContext = () => ({});
  });

  afterEach(() => {
    document.body.innerHTML = "";
    delete window.Chart;
    delete window.irAFacturaDesdeGeneral;
  });

  it("pide el mes actual al abrir y pinta cada tarjeta del mockup", async () => {
    await start();

    expect(kpiCalls()[0][1]).toEqual({ period: { preset: "month" } });
    expect(part("ingresos", "valor")).toBe("$13mil");
    expect(card("ingresos").querySelector(".kpi-delta.is-good")).not.toBeNull();
    expect(part("ingresos", "delta")).toContain("+30,0 %");
    expect(part("ventas", "valor")).toBe("12");
    expect(card("ventas").querySelector(".kpi-delta.is-bad")).not.toBeNull();
    expect(part("promedio", "valor")).toContain("1,5");
    expect(part("promedio", "valor")).toContain("al día");
    expect(part("promedio", "delta")).toBe("Sin cambio");
    expect(part("ganancia", "delta")).toBe("Sin comparación");
    expect(part("margen", "valor")).toContain("46,9");
    expect(part("margen", "delta")).toContain("+6,9 pp");
    expect(part("margen", "nota")).toContain("Promedio 56,7 %");
    // Subir los gastos es malo.
    expect(card("gastos").querySelector(".kpi-delta.is-bad")).not.toBeNull();
    expect(part("agotados", "valor")).toContain("3");
    // Sin historial no hay variación ni minigráfico.
    expect(part("inventario", "delta")).toBe("");
    expect(
      card("inventario").querySelector('[data-role="spark"]').children,
    ).toHaveLength(0);
    expect(
      card("ingresos").querySelector('[data-role="spark"] .kpi-spark-line'),
    ).not.toBeNull();
    expect(text("dash-rango")).toBe(
      "Del 01/11/2026 al 18/11/2026, comparado con el período anterior",
    );
  });

  it("las tarjetas de inventario y agotados quedan visibles", async () => {
    await start();
    expect(card("inventario").hidden).toBe(false);
    expect(card("agotados").hidden).toBe(false);
  });

  it("muestra los equivalentes en las monedas del usuario", async () => {
    await start();
    expect(part("ingresos", "nota")).toContain("1.300.000,00 CUP");
    expect(part("ganancia", "nota")).toContain("550.000,00 CUP");
    expect(part("gastos", "nota")).toContain("60.000,00 CUP");
    expect(part("inventario", "nota")).toContain("400.000,00 CUP");
    expect(part("promedio", "nota")).toContain("44.400,00 CUP");
  });

  it("sin preferencias o tasas el dashboard sigue en USD", async () => {
    respond({ get_user_preferences: new Error("falla") });
    await start();
    expect(document.getElementById("dash-error").hidden).toBe(true);
    expect(part("ingresos", "nota")).toBe("");
    expect(part("ventas", "valor")).toBe("12");
  });

  it("pinta gastos, pérdidas aparte y su peso sobre los ingresos", async () => {
    await start();
    expect(text("dash-gasto-garantia")).toContain("300,00");
    expect(text("dash-gasto-garantia")).toContain("50,0 %");
    expect(text("dash-gasto-insumos")).toContain("33,3 %");
    expect(text("dash-gasto-perdidas")).toContain("100,00");
    expect(text("dash-gasto-total")).toContain("600,00");
    expect(text("dash-gasto-pct")).toBe("5%");
  });

  it("pinta rankings, Pareto y lento movimiento como texto", async () => {
    await start();
    const estrella = document.getElementById("dash-estrella-productos");
    expect(estrella.textContent).toContain("Bomba");
    expect(estrella.querySelector(".star-bar-fill").style.width).toBe("82%");
    expect(text("dash-estrella-productos-total")).toBe("82%");
    expect(text("dash-estrella-clientes-total")).toBe("92%");

    const frecuentes = document.getElementById("dash-frecuentes");
    expect(frecuentes.children).toHaveLength(2);
    expect(frecuentes.querySelectorAll(".customer-highlight")).toHaveLength(1);
    expect(frecuentes.textContent).toContain("2 compras");
    expect(frecuentes.textContent).toContain("1 compra ");

    expect(document.getElementById("dash-vendidos-a").children).toHaveLength(5);
    expect(document.getElementById("dash-vendidos-b").children).toHaveLength(2);
    expect(text("dash-vendidos-a")).toContain("10,00 USD");

    const lento = document.getElementById("dash-lento");
    expect(
      Array.from(lento.querySelectorAll("td")).map((td) => td.textContent),
    ).toEqual(['<img src=x onerror="alert(1)">Nunca', "0", "6,00 USD", "46"]);
    expect(lento.querySelector("img")).toBeNull();
    expect(text("dash-inventario-cantidad")).toContain("3,5");
  });

  it("la gráfica recibe los tramos en USD y deja ocultar una serie", async () => {
    await start();
    const [chart] = chartInstances;
    expect(chart.data.labels).toEqual(["01/11", "02/11"]);
    expect(chart.data.datasets[0].data).toEqual([3000, 5000]);
    expect(chart.data.datasets[1].data).toEqual([1000, 4000]);
    expect(chart.data.datasets[2].data).toEqual([4000, 9000]);
    expect(text("dash-reparto")).toBe("Ventas 62 % · Servicios 38 %");

    const legend = document.querySelector('[data-serie="1"]');
    legend.click();
    expect(chart.isDatasetVisible(1)).toBe(false);
    expect(legend.getAttribute("aria-pressed")).toBe("false");
  });

  it("sin Chart.js la gráfica muestra su estado de error", async () => {
    delete window.Chart;
    await start();
    expect(
      document.getElementById("dash-chart-wrap").classList.contains("is-error"),
    ).toBe(true);
    expect(part("ventas", "valor")).toBe("12");
  });

  it("el selector tiene las siete opciones y cada una recarga", async () => {
    await start();
    const buttons = Array.from(
      document.querySelectorAll("#page-dashboard [data-periodo]"),
    );
    expect(buttons.map((button) => button.textContent.trim())).toEqual([
      "Hoy",
      "Semana",
      "Mes",
      "3 Meses",
      "6 Meses",
      "1 Año",
      "Rango",
    ]);

    for (const preset of ["today", "week", "month", "3m", "6m", "1y"]) {
      invokeMock.mockClear();
      periodButton(preset).click();
      await vi.waitFor(() =>
        expect(kpiCalls()[0]?.[1]).toEqual({ period: { preset } }),
      );
      await vi.waitFor(() =>
        expect(periodButton(preset).getAttribute("aria-pressed")).toBe("true"),
      );
    }
    expect(text("dash-rango")).toContain("comparado con el período anterior");

    periodButton("today").click();
    await vi.waitFor(() =>
      expect(text("dash-rango")).toBe("Hoy, comparado con ayer"),
    );
  });

  it("el rango personalizado valida las fechas antes de pedir datos", async () => {
    await start();
    const form = document.getElementById("dash-rango-form");
    const from = document.getElementById("dash-rango-desde");
    const to = document.getElementById("dash-rango-hasta");
    const submit = () =>
      form.dispatchEvent(
        new Event("submit", { bubbles: true, cancelable: true }),
      );
    expect(form.hidden).toBe(true);

    invokeMock.mockClear();
    periodButton("custom").click();
    expect(form.hidden).toBe(false);
    expect(kpiCalls()).toHaveLength(0);

    submit();
    expect(text("dash-rango-desde-error")).toBe(
      "Elige la fecha inicial y la final",
    );
    from.value = "2026-11-10";
    to.value = "2026-11-01";
    submit();
    expect(text("dash-rango-desde-error")).toBe(
      "La fecha inicial no puede ser posterior a la final",
    );
    expect(kpiCalls()).toHaveLength(0);

    to.value = "2026-11-12";
    submit();
    await vi.waitFor(() =>
      expect(kpiCalls()[0]?.[1]).toEqual({
        period: {
          preset: "custom",
          fromDate: "2026-11-10",
          toDate: "2026-11-12",
        },
      }),
    );
    await vi.waitFor(() =>
      expect(periodButton("custom").classList.contains("active")).toBe(true),
    );
    expect(form.hidden).toBe(false);
  });

  it("un fallo muestra el estado de error y Reintentar vuelve a pedir", async () => {
    respond({ get_dashboard_kpis: new Error("falla") });
    await start();
    const error = document.getElementById("dash-error");
    await vi.waitFor(() => expect(error.hidden).toBe(false));

    respond();
    document.getElementById("dash-reintentar").click();
    await vi.waitFor(() => expect(error.hidden).toBe(true));
    expect(part("ventas", "valor")).toBe("12");
  });

  it("la tarjeta de reservas enlaza a cada factura que vence", async () => {
    window.irAFacturaDesdeGeneral = vi.fn();
    await start();
    expect(text("dash-reservas-total")).toBe("2");
    expect(text("dash-reservas-monto")).toBe("100,99 USD");
    const list = document.getElementById("dash-reservas-lista");
    expect(list.textContent).toContain("Ana Pérez");
    expect(list.textContent).toContain("20/11/2026");
    list.querySelector("button").click();
    expect(window.irAFacturaDesdeGeneral).toHaveBeenCalledWith("V", 7);
  });

  it("sin reservas por vencer lo dice", async () => {
    respond({
      get_dashboard_kpis: {
        ...KPIS,
        reservations: { count: 0, amountCents: 0, expiring: [] },
      },
    });
    await start();
    expect(text("dash-reservas-lista")).toContain(
      "Ninguna vence en los próximos 3 días",
    );
  });

  it("se recalcula al volver al dashboard con el período elegido", async () => {
    await start();
    periodButton("week").click();
    await vi.waitFor(() =>
      expect(periodButton("week").getAttribute("aria-pressed")).toBe("true"),
    );
    invokeMock.mockClear();

    window.dispatchEvent(
      new CustomEvent("page:change", { detail: { pageId: "productos" } }),
    );
    expect(kpiCalls()).toHaveLength(0);

    window.dispatchEvent(
      new CustomEvent("page:change", { detail: { pageId: "dashboard" } }),
    );
    await vi.waitFor(() =>
      // Con isolate:false quedan escuchando los módulos de tests anteriores.
      expect(kpiCalls().map(([, args]) => args)).toContainEqual({
        period: { preset: "week" },
      }),
    );
  });

  it("Ver Agotados del HTML real abre la pestaña de productos", async () => {
    window.showPage = vi.fn();
    await start();
    document.getElementById("dash-agotados-ver-agotados").click();
    expect(window.showPage).toHaveBeenCalledWith("productos", null);
    delete window.showPage;
  });
});
