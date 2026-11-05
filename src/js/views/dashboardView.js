import {
  getDashboardKpis,
  getDashboardSummary,
} from "../services/dashboardService.js";
import {
  getUserPreferences,
  listExchangeRates,
} from "../services/settingsService.js";
import { equivalents } from "../utils/currency.js";
import { el, messageRow } from "../utils/dom.js";
import {
  clearFieldErrors,
  notifyError,
  setFieldError,
} from "../utils/feedback.js";
import { formatDate, formatMoney } from "../utils/formatters.js";

const SVG_NS = "http://www.w3.org/2000/svg";
const NBSP = " ";

// inverse = true cuando subir es MALO (gastos y productos agotados).
const KPIS = {
  ingresos: { field: "income", type: "money", change: "percent" },
  ventas: { field: "sales", type: "integer", change: "percent" },
  promedio: {
    field: "dailySalesMilli",
    type: "decimal",
    scale: 1000,
    change: "percent",
    unit: "al día",
  },
  ganancia: { field: "netProfit", type: "money", change: "percent" },
  margen: {
    field: "marginHundredths",
    type: "percent",
    scale: 100,
    change: "points",
  },
  gastos: {
    field: "expenses",
    type: "money",
    change: "percent",
    inverse: true,
  },
  inventario: { field: "inventoryValue", type: "money", change: "percent" },
  agotados: {
    field: "outOfStock",
    type: "integer",
    change: "absolute",
    inverse: true,
    unit: "productos",
  },
};

// Paleta de la gráfica: mismos tonos azul/naranja de las tarjetas KPI. "Total"
// es un dato derivado: magenta y línea discontinua, validado para deuteranopia
// y protanopia.
const COLOR_SALES = "#2563eb";
const COLOR_SERVICES = "#f48430";
const COLOR_TOTAL = "#e11d74";

let initPromise = null;
let chart = null;
let period = { preset: "month" };
let requestId = 0;

export function initDashboardView() {
  if (!initPromise) {
    initPromise = initializeDashboardView();
  }
  return initPromise;
}

async function initializeDashboardView() {
  bindOutOfStockActions();
  bindDashboardControls();
  window.addEventListener("page:change", (event) => {
    if (event.detail?.pageId === "dashboard") void loadDashboard();
  });
  await loadDashboard();
  window.dashboardView = {
    renderDashboardSummary,
    refreshDashboardSummary,
  };
}

function bindOutOfStockActions() {
  document
    .getElementById("dash-agotados-ver-agotados")
    ?.addEventListener("click", () => {
      void openOutOfStockTab("outOfStock");
    });

  document
    .getElementById("dash-agotados-ver-descontinuados")
    ?.addEventListener("click", () => {
      void openOutOfStockTab("discontinued");
    });
}

function bindDashboardControls() {
  const root = document.getElementById("page-dashboard");
  root?.querySelectorAll("[data-periodo]").forEach((button) => {
    button.addEventListener("click", () => {
      const preset = button.dataset.periodo;
      if (preset === "custom") {
        markActivePeriod("custom");
        document.getElementById("dash-rango-desde")?.focus();
        return;
      }
      void loadDashboard({ preset });
    });
  });
  document
    .getElementById("dash-rango-form")
    ?.addEventListener("submit", (event) => {
      event.preventDefault();
      applyCustomRange(event.currentTarget);
    });
  document
    .getElementById("dash-reintentar")
    ?.addEventListener("click", () => void loadDashboard());
  root?.querySelectorAll("[data-serie]").forEach((button) => {
    button.addEventListener("click", () => {
      toggleSeries(Number(button.dataset.serie), button);
    });
  });
}

function applyCustomRange(form) {
  const from = document.getElementById("dash-rango-desde");
  const to = document.getElementById("dash-rango-hasta");
  clearFieldErrors(form);
  if (!from.value || !to.value) {
    setFieldError(from.value ? to : from, "Elige la fecha inicial y la final");
    return;
  }
  if (from.value > to.value) {
    setFieldError(from, "La fecha inicial no puede ser posterior a la final");
    return;
  }
  void loadDashboard({
    preset: "custom",
    fromDate: from.value,
    toDate: to.value,
  });
}

function markActivePeriod(preset) {
  document
    .querySelectorAll("#page-dashboard [data-periodo]")
    .forEach((button) => {
      const active = button.dataset.periodo === preset;
      button.classList.toggle("active", active);
      button.setAttribute("aria-pressed", String(active));
    });
  const form = document.getElementById("dash-rango-form");
  if (form) form.hidden = preset !== "custom";
}

async function loadCurrencies() {
  try {
    const [preferences, rates] = await Promise.all([
      getUserPreferences(),
      listExchangeRates(),
    ]);
    return {
      currencies: preferences?.visibleCurrencies ?? [],
      rates: rates ?? [],
    };
  } catch {
    // Sin monedas el dashboard sigue mostrando sus cifras en USD.
    return { currencies: [], rates: [] };
  }
}

async function loadDashboard(next = period) {
  const summary = refreshDashboardSummary();
  const root = document.getElementById("page-dashboard");
  const errorBox = document.getElementById("dash-error");
  if (!errorBox) return summary;

  period = next;
  const id = ++requestId;
  root.setAttribute("aria-busy", "true");
  let kpis;
  let money;
  try {
    [kpis, money] = await Promise.all([
      getDashboardKpis(next),
      loadCurrencies(),
    ]);
  } catch (error) {
    if (id !== requestId) return summary;
    root.removeAttribute("aria-busy");
    if (error?.code === "validationFailed") notifyError(error);
    else errorBox.hidden = false;
    return summary;
  }
  if (id !== requestId) return summary; // llegó una respuesta más nueva
  root.removeAttribute("aria-busy");
  errorBox.hidden = true;
  markActivePeriod(next.preset);
  renderDashboardKpis(kpis, money);
  return summary;
}

/** Pinta todas las cifras de DB-02 a DB-07. */
export function renderDashboardKpis(kpis, { currencies, rates }) {
  const inOtherCurrencies = (cents) => equivalents(cents, currencies, rates);
  const note = (...parts) => parts.flat().join(" · ");
  const today = period.preset === "today";

  renderKpiCards(kpis, today ? "ayer" : "el período anterior");
  renderKpiNotes({
    ingresos: note(inOtherCurrencies(kpis.income.value)),
    promedio: note(
      `${formatMoney(kpis.dailySalesAmountCents)} al día`,
      inOtherCurrencies(kpis.dailySalesAmountCents),
    ),
    ganancia: note(inOtherCurrencies(kpis.netProfit.value)),
    margen: note(
      `Total ${formatMoney(kpis.grossMarginCents)}`,
      `Promedio ${percentText(kpis.averageProfitHundredths / 100)}`,
    ),
    gastos: note(inOtherCurrencies(kpis.expenses.value)),
    inventario: note(inOtherCurrencies(kpis.inventoryValue.value)),
  });
  document.getElementById("dash-rango").textContent = today
    ? "Hoy, comparado con ayer"
    : `Del ${formatDate(kpis.fromDate)} al ${formatDate(kpis.toDate)}, comparado con el período anterior`;
  renderChart(kpis.buckets);
  renderStars("clientes", kpis.starClients);
  renderStars("productos", kpis.starProducts);
  renderFrequentClients(kpis.frequentClients, kpis.starClients);
  renderExpenses(kpis);
  renderBestSellers(kpis.bestSellers);
  renderSlowMovers(kpis.slowMovers);
  renderReservations(kpis.reservations);
  document.getElementById("dash-inventario-cantidad").textContent =
    `Productos en inventario: ${kpis.inventoryQuantity}`;
}

/* ---------- Formato (coma decimal, espacio en los miles) ---------- */

const thousands = (value) =>
  String(Math.round(value)).replace(/\B(?=(\d{3})+(?!\d))/g, NBSP);
const decimal = (value, digits = 1) => value.toFixed(digits).replace(".", ",");
const trimmed = (value, digits) =>
  String(Number(value.toFixed(digits))).replace(".", ",");
const percentText = (value) => `${decimal(value)} %`;

// 118403 → { amount: '118,4', unit: 'mil' } | 1425999 → { amount: '1,43', unit: 'M' }
function moneyParts(value) {
  const size = Math.abs(value);
  if (size >= 1e6) return { amount: trimmed(value / 1e6, 2), unit: "M" };
  if (size >= 1e4) return { amount: trimmed(value / 1e3, 1), unit: "mil" };
  return { amount: decimal(value, 2), unit: "" };
}

function moneyNodes(cents, unitClass) {
  const { amount, unit } = moneyParts(cents / 100);
  return [amount, unit ? el("span", { class: unitClass }, unit) : null];
}

/* ---------- Tarjetas ---------- */

function kpiValueNodes(def, value) {
  const unit = (text) => el("span", { class: "kpi-unit" }, text);
  switch (def.type) {
    case "money":
      return [
        el("span", { class: "kpi-cur" }, "$"),
        ...moneyNodes(value, "kpi-unit"),
      ];
    case "percent":
      return [decimal(value), unit("%")];
    case "decimal":
      return [
        Number.isInteger(value) ? String(value) : decimal(value),
        def.unit ? unit(def.unit) : null,
      ];
    default:
      return [thousands(value), def.unit ? unit(def.unit) : null];
  }
}

function kpiPlainText(def, value, rawValue) {
  if (def.type === "money") return formatMoney(rawValue);
  if (def.type === "percent") return percentText(value);
  const text = def.type === "decimal" ? decimal(value) : thousands(value);
  return def.unit ? `${text} ${def.unit}` : text;
}

function svgNode(tag, attributes, ...children) {
  const node = document.createElementNS(SVG_NS, tag);
  for (const [name, value] of Object.entries(attributes)) {
    node.setAttribute(name, value);
  }
  node.append(...children);
  return node;
}

function arrowNode(direction) {
  const shape =
    direction === 0
      ? {
          d: "M2 5h6",
          stroke: "currentColor",
          "stroke-width": "1.8",
          "stroke-linecap": "round",
        }
      : {
          d: direction > 0 ? "M5 2 9 8H1Z" : "M5 8 1 2h8Z",
          fill: "currentColor",
        };
  return svgNode(
    "svg",
    { class: "kpi-arrow", viewBox: "0 0 10 10", "aria-hidden": "true" },
    svgNode("path", shape),
  );
}

function changeNode(def, value, previous, comparison) {
  let difference;
  let text;
  if (def.change === "points") {
    difference = value - previous;
    text = `${decimal(Math.abs(difference))} pp`;
  } else if (def.change === "absolute") {
    difference = value - previous;
    text = String(Math.abs(difference));
  } else if (previous === 0) {
    // Sin base no hay porcentaje que calcular.
    if (value !== 0) {
      const reading = `Sin movimientos en ${comparison}`;
      return el(
        "span",
        { class: "kpi-delta is-flat", title: reading, "aria-label": reading },
        arrowNode(0),
        el("span", {}, "Sin comparación"),
      );
    }
    difference = 0;
    text = "";
  } else {
    difference = ((value - previous) / Math.abs(previous)) * 100;
    text = percentText(Math.abs(difference));
  }
  const direction = Math.abs(difference) < 0.05 ? 0 : Math.sign(difference);
  const good = def.inverse ? direction < 0 : direction > 0;
  const state = direction === 0 ? "is-flat" : good ? "is-good" : "is-bad";
  const sign = direction > 0 ? "+" : direction < 0 ? "−" : "";
  const reading =
    direction === 0
      ? `Sin cambios frente a ${comparison}`
      : `${direction > 0 ? "Subió" : "Bajó"} ${text} frente a ${comparison}`;
  return el(
    "span",
    { class: `kpi-delta ${state}`, title: reading, "aria-label": reading },
    arrowNode(direction),
    el("span", {}, direction === 0 ? "Sin cambio" : `${sign}${text}`),
  );
}

// Mini gráfico de tendencia en SVG puro.
function sparkNodes(raw) {
  if (!raw.length) return [];
  const width = 96;
  const height = 36;
  const padX = 3;
  const padTop = 5;
  const padBottom = 4;
  // Con muchos puntos se promedia una ventana corta: se ve la tendencia, no
  // el ruido diario.
  const span = raw.length > 20 ? 5 : raw.length > 12 ? 3 : 1;
  const values =
    span === 1
      ? raw
      : raw.map((_, index) => {
          const slice = raw.slice(
            Math.max(0, index - (span >> 1)),
            index + (span >> 1) + 1,
          );
          return slice.reduce((sum, item) => sum + item, 0) / slice.length;
        });
  const min = Math.min(...values);
  const max = Math.max(...values);
  const range = max - min || 1;
  const step = (width - padX * 2) / (values.length - 1 || 1);
  const points = values.map((value, index) => [
    padX + index * step,
    max === min
      ? height / 2
      : padTop + (1 - (value - min) / range) * (height - padTop - padBottom),
  ]);
  const line = points
    .map(
      ([x, y], index) => `${index ? "L" : "M"}${x.toFixed(1)} ${y.toFixed(1)}`,
    )
    .join(" ");
  const [lastX, lastY] = points[points.length - 1];
  const area = `${line} L${lastX.toFixed(1)} ${height} L${points[0][0].toFixed(1)} ${height} Z`;
  return [
    svgNode("path", { class: "kpi-spark-area", d: area }),
    svgNode("path", { class: "kpi-spark-line", d: line }),
    svgNode("circle", {
      class: "kpi-spark-dot",
      cx: lastX.toFixed(1),
      cy: lastY.toFixed(1),
      r: "2.6",
    }),
  ];
}

function renderKpiCards(kpis, comparison) {
  for (const [key, def] of Object.entries(KPIS)) {
    const card = document.querySelector(`[data-kpi="${key}"]`);
    const data = kpis[def.field];
    if (!card || !data) continue;
    const scale = def.scale ?? 1;
    const value = data.value / scale;
    const valueNode = card.querySelector('[data-role="valor"]');
    valueNode.replaceChildren(
      ...kpiValueNodes(def, def.type === "money" ? data.value : value).filter(
        Boolean,
      ),
    );
    valueNode.title = kpiPlainText(def, value, data.value);
    // Lo que solo se conoce hoy no tiene comparación ni tendencia.
    const delta = card.querySelector('[data-role="delta"]');
    if (data.previous == null) delta.replaceChildren();
    else {
      delta.replaceChildren(
        changeNode(def, value, data.previous / scale, comparison),
      );
    }
    card
      .querySelector('[data-role="spark"]')
      .replaceChildren(...sparkNodes(data.trend));
  }
}

function renderKpiNotes(notes) {
  for (const [key, text] of Object.entries(notes)) {
    const node = document.querySelector(
      `[data-kpi="${key}"] [data-role="nota"]`,
    );
    if (node) node.textContent = text;
  }
}

/* ---------- Listas ---------- */

function renderStars(kind, items) {
  const body = document.getElementById(`dash-estrella-${kind}`);
  body.replaceChildren(
    ...(items.length
      ? items.map(({ name, percent }) =>
          el(
            "tr",
            {},
            el("td", {}, name),
            el(
              "td",
              {},
              el(
                "span",
                { class: "star-bar-track" },
                el("span", {
                  class: "star-bar-fill",
                  style: `width:${Math.min(100, percent)}%`,
                }),
              ),
            ),
            el("td", { class: "percentage" }, `${percent}%`),
          ),
        )
      : [
          el(
            "tr",
            {},
            el("td", { class: "star-empty", colspan: 3 }, "Sin datos"),
          ),
        ]),
  );
  document.getElementById(`dash-estrella-${kind}-total`).textContent =
    `${items.reduce((sum, item) => sum + item.percent, 0)}%`;
}

function initials(name) {
  const [first = "", second = ""] = name.trim().split(/\s+/);
  return `${first.charAt(0)}${second.charAt(0)}`.toUpperCase();
}

function renderFrequentClients(clients, starClients) {
  const stars = new Set(starClients.map((client) => client.name));
  const list = document.getElementById("dash-frecuentes");
  if (!clients.length) {
    list.replaceChildren(
      el("div", { class: "customer-item star-empty" }, "Sin datos"),
    );
    return;
  }
  list.replaceChildren(
    ...clients.map(({ name, purchases, totalCents }) =>
      el(
        "div",
        {
          class: `customer-item${stars.has(name) ? " customer-highlight" : ""}`,
        },
        el("span", { class: "avatar", "aria-hidden": "true" }, initials(name)),
        el("span", { class: "cust-name", title: name }, name),
        el(
          "span",
          { class: "compras-tag" },
          `${purchases} ${purchases === 1 ? "compra" : "compras"} · ${formatMoney(totalCents)}`,
        ),
      ),
    ),
  );
}

function renderExpenses(kpis) {
  const total = kpis.expenses.value;
  const share = (cents) => (total ? (cents / total) * 100 : 0);
  for (const [id, cents] of [
    ["garantia", kpis.guaranteeCents],
    ["insumos", kpis.installationCents],
    ["perdidas", kpis.lossCents],
  ]) {
    document
      .getElementById(`dash-gasto-${id}`)
      .replaceChildren(
        ...moneyNodes(cents, "unit").filter(Boolean),
        el("span", { class: "dash-note" }, percentText(share(cents))),
      );
  }
  document
    .getElementById("dash-gasto-total")
    .replaceChildren(...moneyNodes(total, "unit").filter(Boolean));
  const income = kpis.income.value;
  document.getElementById("dash-gasto-pct").textContent =
    `${trimmed(income ? (total / income) * 100 : 0, 0)}%`;
}

function renderBestSellers(items) {
  const row = ({ name, quantity, marginCents }, index) =>
    el(
      "div",
      {
        class: `product-item${index < 3 ? ` product-item-${index + 1}` : ""}`,
        title: `${name}: ${quantity} vendidos, margen ${formatMoney(marginCents)}`,
      },
      el("span", { class: "best-rank" }, String(index + 1)),
      el("span", { class: "best-name" }, name),
      el("span", { class: "best-units" }, quantity),
      el("span", { class: "dash-note" }, formatMoney(marginCents)),
    );
  const rows = items.slice(0, 10).map(row);
  document
    .getElementById("dash-vendidos-a")
    .replaceChildren(...rows.slice(0, 5));
  document.getElementById("dash-vendidos-b").replaceChildren(...rows.slice(5));
}

function renderSlowMovers(items) {
  const body = document.getElementById("dash-lento");
  if (!items.length) {
    messageRow(body, "Sin productos activos", 4);
    return;
  }
  body.replaceChildren(
    ...items.map(({ name, sold, sellingPriceCents, daysWithoutSale }) =>
      el(
        "tr",
        {},
        el("td", {}, name),
        el("td", {}, sold),
        el("td", {}, formatMoney(sellingPriceCents)),
        el("td", {}, String(daysWithoutSale)),
      ),
    ),
  );
}

function renderReservations({ count, amountCents, expiring }) {
  document.getElementById("dash-reservas-total").textContent = String(count);
  document.getElementById("dash-reservas-monto").textContent =
    formatMoney(amountCents);
  const list = document.getElementById("dash-reservas-lista");
  if (!expiring.length) {
    list.replaceChildren(
      el("p", { class: "dash-note" }, "Ninguna vence en los próximos 3 días."),
    );
    return;
  }
  list.replaceChildren(
    ...expiring.map(
      ({ id, number, invoiceType, contactName, reservedUntil }) => {
        const link = el(
          "button",
          {
            type: "button",
            class: "action-link",
            "aria-label": `Ver factura ${number}`,
          },
          number,
        );
        link.addEventListener("click", () => {
          window.irAFacturaDesdeGeneral?.(invoiceType, id);
        });
        return el(
          "div",
          { class: "dash-reserva" },
          link,
          el("span", { class: "dash-reserva-cliente" }, contactName),
          el("span", { class: "dash-note" }, formatDate(reservedUntil)),
        );
      },
    ),
  );
}

/* ---------- Gráfica de ingresos (Chart.js) ---------- */

// Ejes en notación K / M (200K, 150K, ...).
function shortAxis(value) {
  if (value >= 1e6) return `${trimmed(value / 1e6, 1)}M`;
  if (value >= 1e3) return `${trimmed(value / 1e3, 1)}K`;
  return String(value);
}

function createChart(canvas) {
  const { Chart } = window;
  if (!Chart) return null;
  if (window.matchMedia?.("(prefers-reduced-motion: reduce)").matches) {
    Chart.defaults.animation = false;
  }
  Chart.defaults.font.family = '"Inter-Regular", Helvetica, Arial, sans-serif';
  Chart.defaults.font.size = 12;
  Chart.defaults.color = "#020f1e80";

  // Línea/área: cada serie con su color y un relleno suave del mismo color.
  const series = (label, color, fill, order, extra = {}) => ({
    label,
    data: [],
    borderColor: color,
    backgroundColor: fill,
    borderWidth: 2,
    fill: true,
    tension: 0.35,
    pointRadius: 0,
    pointHoverRadius: 4,
    pointHoverBackgroundColor: color,
    pointHitRadius: 10,
    order,
    ...extra,
  });
  return new Chart(canvas.getContext("2d"), {
    type: "line",
    data: {
      labels: [],
      datasets: [
        series("Ventas", COLOR_SALES, "#2563eb14", 2),
        series("Servicios", COLOR_SERVICES, "#f4843020", 1),
        // Total: discontinua, sin relleno y por encima de las áreas.
        series("Total", COLOR_TOTAL, "transparent", 0, {
          fill: false,
          borderDash: [7, 5],
          borderWidth: 2.5,
        }),
      ],
    },
    options: {
      responsive: true,
      maintainAspectRatio: false,
      interaction: { mode: "index", intersect: false },
      plugins: {
        legend: { display: false },
        tooltip: {
          backgroundColor: "#020f1e",
          padding: 12,
          cornerRadius: 10,
          boxPadding: 4,
          callbacks: {
            label: (item) =>
              ` ${item.dataset.label}: ${formatMoney(Math.round(item.parsed.y * 100))}`,
          },
        },
      },
      scales: {
        x: {
          grid: { display: false },
          border: { display: false },
          ticks: { maxRotation: 0, autoSkip: true, maxTicksLimit: 12 },
        },
        y: {
          beginAtZero: true,
          grid: { color: "#020f1e33" },
          border: { display: false },
          ticks: { maxTicksLimit: 5, callback: shortAxis },
        },
      },
    },
  });
}

function renderChart(buckets) {
  const sum = (field) =>
    buckets.reduce((total, bucket) => total + bucket[field], 0);
  const sales = sum("salesCents");
  const services = sum("servicesCents");
  const total = sales + services;
  const salesShare = total ? Math.round((sales / total) * 100) : 0;
  document.getElementById("dash-reparto").textContent = total
    ? `Ventas ${salesShare} % · Servicios ${100 - salesShare} %`
    : "";

  const canvas = document.getElementById("dash-chart");
  canvas.setAttribute(
    "aria-label",
    `Gráfica de ingresos. Ventas ${formatMoney(sales)}, servicios ${formatMoney(services)}, total ${formatMoney(total)}.`,
  );
  chart ??= createChart(canvas);
  if (!chart) {
    document.getElementById("dash-chart-wrap").classList.add("is-error");
    return;
  }
  const dollars = (cents) => cents / 100;
  const datasets = chart.data.datasets;
  chart.data.labels = buckets.map((bucket) => bucket.label);
  datasets[0].data = buckets.map((bucket) => dollars(bucket.salesCents));
  datasets[1].data = buckets.map((bucket) => dollars(bucket.servicesCents));
  datasets[2].data = buckets.map((bucket) =>
    dollars(bucket.salesCents + bucket.servicesCents),
  );
  // Con un solo tramo ("Hoy") no hay línea que dibujar: se ve el punto.
  datasets.forEach((dataset) => {
    dataset.pointRadius = buckets.length === 1 ? 4 : 0;
  });
  chart.update();
}

function toggleSeries(index, button) {
  if (!chart) return;
  const visible = chart.isDatasetVisible(index);
  chart.setDatasetVisibility(index, !visible);
  button.setAttribute("aria-pressed", String(!visible));
  chart.update();
}

/* ---------- Resumen de agotados (DASH-01) ---------- */

async function refreshDashboardSummary() {
  try {
    const summary = await getDashboardSummary();
    renderDashboardSummary(summary);
  } catch {
    renderDashboardSummary({
      outOfStockTotal: 0,
      topCategories: [],
      withoutCategoryCount: 0,
    });
  }
}

export function renderDashboardSummary(summary = {}) {
  const total = Number(summary?.outOfStockTotal ?? 0);
  const categories = Array.isArray(summary?.topCategories)
    ? summary.topCategories
    : [];
  const withoutCategoryCount = Number(summary?.withoutCategoryCount ?? 0);

  const totalNode = document.getElementById("dash-agotados-total");
  if (totalNode) totalNode.textContent = String(total);

  const categoriesNode = document.getElementById("dash-agotados-categorias");
  if (categoriesNode) {
    categoriesNode.replaceChildren(
      ...categories
        .slice(0, 3)
        .map(({ name, count }) =>
          el("span", {}, `${String(name ?? "")} (${Number(count ?? 0)})`),
        ),
    );
  }

  const withoutCategoryNode = document.getElementById(
    "dash-agotados-sin-categoria",
  );
  if (withoutCategoryNode) {
    withoutCategoryNode.textContent = String(withoutCategoryCount);
  }
}

async function openOutOfStockTab(condition) {
  const sidebarLink = document.querySelector(
    '#sidebar a[data-page="productos"]',
  );
  if (window.showPage) {
    window.showPage("productos", sidebarLink);
  }

  const filter = document.getElementById("producto-condiciones-filter");
  if (filter) {
    filter.value = condition;
  }

  if (window.showProductoTab) {
    const tabButton =
      document.getElementById("tab-producto-agotados") ??
      document.getElementById("producto-condiciones-filter") ??
      document.body;

    await window.showProductoTab("agotados", tabButton);
  }
}

if (document.readyState === "loading") {
  document.addEventListener("DOMContentLoaded", () => {
    void initDashboardView();
  });
} else {
  void initDashboardView();
}
