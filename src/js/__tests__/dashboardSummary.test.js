import { beforeEach, describe, expect, it, vi } from "vitest";

const invokeMock = vi.hoisted(() => vi.fn());

function flushPromises() {
  return new Promise((resolve) => setTimeout(resolve, 0));
}

describe("dashboard summary service", () => {
  beforeEach(() => {
    vi.resetModules();
    vi.clearAllMocks();
    vi.doMock("../services/invoke.js", () => ({ invoke: invokeMock }));
  });

  it("llama al comando de resumen del dashboard con el nombre correcto", async () => {
    const service = await import("../services/dashboardService.js");
    invokeMock.mockResolvedValue({
      outOfStockTotal: 2,
      topCategories: [{ name: "Lácteos", count: 2 }],
      withoutCategoryCount: 1,
    });

    await service.getDashboardSummary();

    expect(invokeMock).toHaveBeenCalledWith("get_dashboard_summary");
  });
});

describe("dashboard resumen de productos agotados", () => {
  beforeEach(() => {
    vi.resetModules();
    vi.clearAllMocks();
    document.body.innerHTML = `
      <div id="page-dashboard">
        <div id="dash-agotados-total">0</div>
        <div id="dash-agotados-categorias"></div>
        <span id="dash-agotados-sin-categoria">0</span>
        <button id="dash-agotados-ver-agotados" type="button">Ver Agotados</button>
        <button id="dash-agotados-ver-descontinuados" type="button">Ver Descontinuados</button>
        <div id="producto-condiciones-filter">
          <option value="all"></option>
          <option value="outOfStock"></option>
          <option value="discontinued"></option>
        </div>
      </div>
    `;

    window.showPage = vi.fn();
    window.showProductoTab = vi.fn();
  });

  it("renderiza el total, categorías y dispara la pestaña filtrada", async () => {
    vi.doMock("../services/dashboardService.js", () => ({
      getDashboardSummary: vi.fn().mockResolvedValue({
        outOfStockTotal: 3,
        topCategories: [
          { name: "Abarrotes", count: 2 },
          { name: "Bebidas", count: 1 },
        ],
        withoutCategoryCount: 1,
      }),
    }));

    const view = await import("../views/dashboardView.js");
    await view.initDashboardView();
    await flushPromises();

    expect(document.getElementById("dash-agotados-total").textContent).toBe(
      "3",
    );
    expect(
      document.getElementById("dash-agotados-sin-categoria").textContent,
    ).toBe("1");
    expect(
      document.getElementById("dash-agotados-categorias").textContent,
    ).toContain("Abarrotes");
    expect(
      document.getElementById("dash-agotados-categorias").textContent,
    ).toContain("Bebidas");

    const agotadosButton = document.getElementById(
      "dash-agotados-ver-agotados",
    );
    agotadosButton.click();
    expect(window.showPage).toHaveBeenCalledWith(
      "productos",
      document.querySelector('#sidebar a[data-page="productos"]'),
    );
    expect(window.showProductoTab).toHaveBeenCalledWith(
      "agotados",
      expect.anything(),
    );

    const filter = document.getElementById("producto-condiciones-filter");
    expect(filter.value).toBe("outOfStock");
  });
});
