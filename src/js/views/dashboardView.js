import { getDashboardSummary } from "../services/dashboardService.js";
import { el } from "../utils/dom.js";

let initPromise = null;

export function initDashboardView() {
  if (!initPromise) {
    initPromise = initializeDashboardView();
  }
  return initPromise;
}

async function initializeDashboardView() {
  bindOutOfStockActions();
  hideLegacyDashboardMetrics();
  await refreshDashboardSummary();
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

function hideLegacyDashboardMetrics() {
  document
    .querySelectorAll('[data-kpi="inventario"], [data-kpi="agotados"]')
    .forEach((node) => {
      node.hidden = true;
    });

  ["dash-vendidos-a", "dash-vendidos-b"].forEach((id) => {
    const node = document.getElementById(id);
    if (node) node.replaceChildren();
  });

  const slowMovementNode = document.getElementById("dash-lento");
  if (slowMovementNode) slowMovementNode.replaceChildren();
}

if (document.readyState === "loading") {
  document.addEventListener("DOMContentLoaded", () => {
    void initDashboardView();
  });
} else {
  void initDashboardView();
}
