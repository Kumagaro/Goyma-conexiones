import { afterEach, beforeEach, describe, expect, it, vi } from "vitest";

vi.mock("../services/clientsService.js", () => ({
  listClients: vi.fn(),
  searchClients: vi.fn(),
  createClient: vi.fn(),
  updateClient: vi.fn(),
  deleteClient: vi.fn(),
  getClientDetail: vi.fn(),
}));

vi.mock("../components/confirmDialog.js", () => ({
  confirmDialog: vi.fn(() => Promise.resolve(true)),
}));

import {
  createClient,
  deleteClient,
  getClientDetail,
  listClients,
  searchClients,
  updateClient,
} from "../services/clientsService.js";
import { initClientesView } from "../views/clientesView.js";
import { formatDate, formatMoney } from "../utils/formatters.js";

function seedClientPage() {
  document.body.innerHTML = `
    <main>
      <section id="page-clientes" class="page active">
        <div class="view-tabs">
          <button id="tab-cliente-list" class="tab active">Clientes</button>
          <button id="tab-cliente-create" class="tab">Crear Nuevo Cliente</button>
        </div>
        <div id="cliente-search-bar">
          <input id="cliente-search" />
        </div>
        <div id="cliente-list" class="config-subview active">
          <table><tbody id="clientes-body"></tbody></table>
        </div>
        <div id="cliente-create" class="config-subview">
          <form id="cliente-form">
            <input id="cliente-nombres" />
            <input id="cliente-apellidos" />
            <input id="cliente-ci" />
            <input id="cliente-telefono" />
            <input id="cliente-correo" />
            <input id="cliente-empresa" />
            <input id="cliente-cargo" />
            <input id="cliente-direccion" />
            <input id="cliente-info-factura" />
            <button type="submit">Guardar</button>
          </form>
        </div>
        <div id="cliente-ver" class="config-subview">
          <span id="cliente-ver-id"></span>
          <h2 id="cliente-ver-nombre-completo"></h2>
          <span id="cliente-ver-empresa"></span>
          <span id="cliente-ver-cargo"></span>
          <span id="cliente-ver-telefono"></span>
          <span id="cliente-ver-correo"></span>
          <span id="cliente-ver-ci"></span>
          <span id="cliente-ver-direccion"></span>
          <span id="cliente-ver-fecha-entrada"></span>
          <span id="cliente-ver-info-factura-nota"></span>
          <span id="cliente-ver-total-facturado"></span>
          <span id="cliente-ver-saldo-pendiente"></span>
          <span id="cliente-ver-num-facturas"></span>
          <span id="cliente-ver-ultima-compra"></span>
          <div id="cliente-ver-historial-facturas"></div>
          <button id="cliente-ver-editar" type="button">Editar</button>
          <button id="cliente-ver-eliminar" type="button">Eliminar</button>
        </div>
      </section>
    </main>
  `;
}

describe("clientesView", () => {
  beforeEach(() => {
    vi.useFakeTimers();
    seedClientPage();
    vi.clearAllMocks();
  });

  afterEach(() => {
    vi.runOnlyPendingTimers();
    vi.useRealTimers();
    document.body.innerHTML = "";
  });

  it("carga clientes y muestra el estado vacío si no llega nada", async () => {
    listClients.mockResolvedValue([]);

    await initClientesView();

    expect(listClients).toHaveBeenCalledTimes(1);
    expect(document.getElementById("clientes-body").textContent).toContain(
      "Aún no hay clientes",
    );
  });

  it("debouncea la búsqueda y reacciona al evento page:change", async () => {
    searchClients.mockResolvedValue([]);

    await initClientesView();
    const input = document.getElementById("cliente-search");
    input.value = "Ana";
    input.dispatchEvent(new Event("input", { bubbles: true }));

    await Promise.resolve();
    expect(searchClients).not.toHaveBeenCalled();

    await vi.advanceTimersByTimeAsync(300);
    expect(searchClients).toHaveBeenCalledWith("Ana");

    window.dispatchEvent(
      new CustomEvent("page:change", { detail: { pageId: "clientes" } }),
    );
    await Promise.resolve();
    expect(listClients).toHaveBeenCalledTimes(2);
  });

  it("crea, edita y elimina un cliente con mensajes seguros", async () => {
    listClients.mockResolvedValue([
      {
        id: 1,
        fullName: "Ana Pérez",
        company: "Goyma",
        phone: "+53 55512345",
      },
    ]);
    getClientDetail.mockResolvedValue({
      id: 1,
      firstName: "Ana",
      lastName: "Pérez",
      fullName: "Ana Pérez",
      nationalId: "90010112345",
      phone: "+53 55512345",
      company: "Goyma",
      position: "Gerente",
      email: "ana@goyma.test",
      address: "Calle 1",
      invoiceInfo: "Requiere factura",
      createdAt: "2026-01-15",
      indicators: {
        totalInvoicedCents: 0,
        pendingBalanceCents: 0,
        invoiceCount: 0,
        lastPurchaseDate: null,
      },
      invoiceHistory: [],
      expiredCleanupWarning: null,
    });
    createClient.mockResolvedValue({ id: 12 });
    updateClient.mockResolvedValue({ id: 1 });
    deleteClient.mockResolvedValue({ deleted: true });

    await initClientesView();
    const form = document.getElementById("cliente-form");
    document.getElementById("cliente-nombres").value = "Luis";
    document.getElementById("cliente-apellidos").value = "García";
    document.getElementById("cliente-ci").value = "90010112345";
    document.getElementById("cliente-telefono").value = "+53 55512345";
    document.getElementById("cliente-correo").value = "luis@goyma.test";
    document.getElementById("cliente-empresa").value = "Goyma";
    document.getElementById("cliente-cargo").value = "Analista";
    document.getElementById("cliente-direccion").value = "Calle 2";
    document.getElementById("cliente-info-factura").value = "Requiere factura";

    form.dispatchEvent(
      new Event("submit", { bubbles: true, cancelable: true }),
    );

    await vi.waitFor(() => {
      expect(createClient).toHaveBeenCalledWith({
        firstName: "Luis",
        lastName: "García",
        nationalId: "90010112345",
        phone: "+53 55512345",
        email: "luis@goyma.test",
        company: "Goyma",
        position: "Analista",
        address: "Calle 2",
        invoiceInfo: "Requiere factura",
      });
    });

    await vi.waitFor(() => {
      expect(
        document.querySelector("#clientes-body tr[data-client-id='1']"),
      ).not.toBeNull();
    });

    const firstRow = document.querySelector(
      "#clientes-body tr[data-client-id='1']",
    );
    firstRow.querySelector('[data-client-action="detail"]').click();
    await vi.waitFor(() => {
      expect(
        document.getElementById("cliente-ver-nombre-completo").textContent,
      ).toContain("Ana Pérez");
    });
    expect(
      document.getElementById("cliente-ver-fecha-entrada").textContent,
    ).toBe("15/01/2026");

    listClients.mockResolvedValueOnce([
      {
        id: 7,
        fullName: "<img src=x onerror=alert(1)>Ana",
        company: "",
        phone: "+53 55512345",
      },
    ]);
    await initClientesView();

    expect(document.getElementById("clientes-body").textContent).toContain(
      "Ana",
    );
    expect(
      document.getElementById("clientes-body").querySelector("img"),
    ).toBeNull();

    const row = document.querySelector('[data-client-id="7"]');
    row.querySelector('[data-client-action="delete"]').click();
    await vi.waitFor(() => {
      expect(deleteClient).toHaveBeenCalledWith(7);
    });
  });
  it("el detalle muestra los indicadores reales y el historial de getClientDetail", async () => {
    listClients.mockResolvedValue([
      { id: 5, fullName: "Ana Pérez", company: null, phone: "+5355550001" },
    ]);
    getClientDetail.mockResolvedValue({
      id: 5,
      firstName: "Ana",
      lastName: "Pérez",
      fullName: "Ana Pérez",
      nationalId: "90010112345",
      phone: "+5355550001",
      createdAt: "2026-01-02",
      indicators: {
        totalInvoicedCents: 123_450,
        totalPurchasedCents: 0,
        pendingBalanceCents: 50_000,
        invoiceCount: 3,
        lastPurchaseDate: "2026-02-10",
      },
      invoiceHistory: [
        {
          id: 11,
          number: "V-00011",
          invoiceType: "V",
          date: "2026-02-10",
          totalCents: 73_450,
          status: "pending",
        },
        {
          id: 12,
          number: '<img src=x onerror="alert(1)">GG-1',
          invoiceType: "GG",
          date: "2026-02-08",
          totalCents: 900,
          status: "completed",
        },
      ],
      expiredCleanupWarning: null,
    });
    window.irAFacturaDesdeGeneral = vi.fn();

    await initClientesView();
    await vi.waitFor(() =>
      expect(
        document.querySelector('[data-client-action="detail"]'),
      ).not.toBeNull(),
    );
    document.querySelector('[data-client-action="detail"]').click();
    await vi.waitFor(() => expect(getClientDetail).toHaveBeenCalledWith(5));
    await vi.waitFor(() =>
      expect(
        document.getElementById("cliente-ver-num-facturas").textContent,
      ).toBe("3"),
    );

    const text = (id) => document.getElementById(id).textContent;
    expect(text("cliente-ver-total-facturado")).toBe(
      formatMoney(123_450).replace(" USD", ""),
    );
    expect(text("cliente-ver-saldo-pendiente")).toBe(
      formatMoney(50_000).replace(" USD", ""),
    );
    expect(text("cliente-ver-ultima-compra")).toBe(formatDate("2026-02-10"));

    const history = document.getElementById("cliente-ver-historial-facturas");
    const rows = Array.from(history.querySelectorAll("tbody tr")).map((tr) =>
      Array.from(tr.querySelectorAll("td")).map((td) => td.textContent),
    );
    expect(rows[0].slice(0, 5)).toEqual([
      "V-00011",
      "Venta",
      formatDate("2026-02-10"),
      formatMoney(73_450),
      "Por Cobrar",
    ]);
    expect(rows[1][1]).toBe("Garantía");
    expect(rows[1][4]).toBe("Completada");
    expect(history.querySelector("img")).toBeNull();

    history.querySelector("tbody tr button").click();
    expect(window.irAFacturaDesdeGeneral).toHaveBeenCalledWith("V", 11);
    delete window.irAFacturaDesdeGeneral;
  });
  it("H-7: muestra el aviso de purga del detalle con role=status y lo limpia con null", async () => {
    listClients.mockResolvedValue([
      { id: 6, fullName: "Rosa Díaz", company: null, phone: "+5355550009" },
    ]);
    const detail = {
      id: 6,
      firstName: "Rosa",
      lastName: "Díaz",
      fullName: "Rosa Díaz",
      nationalId: "90010112347",
      phone: "+5355550009",
      createdAt: "2026-01-04",
      indicators: {
        totalInvoicedCents: 0,
        totalPurchasedCents: 0,
        pendingBalanceCents: 0,
        invoiceCount: 0,
        lastPurchaseDate: null,
      },
      invoiceHistory: [],
      expiredCleanupWarning:
        "No se pudieron limpiar 2 productos vencidos; se reintentará en la próxima operación",
    };
    getClientDetail.mockResolvedValue(detail);
    await initClientesView();
    const open = () =>
      document.querySelector('[data-client-action="detail"]').click();
    await vi.waitFor(() =>
      expect(
        document.querySelector('[data-client-action="detail"]'),
      ).not.toBeNull(),
    );
    open();
    const status = () => document.getElementById("cliente-ver-warning");
    await vi.waitFor(() =>
      expect(status()?.textContent).toBe(
        "No se pudieron limpiar 2 productos vencidos; se reintentará en la próxima operación",
      ),
    );
    expect(status().getAttribute("role")).toBe("status");
    expect(status().hidden).toBe(false);

    getClientDetail.mockResolvedValue({
      ...detail,
      expiredCleanupWarning: null,
    });
    open();
    await vi.waitFor(() => expect(status().textContent).toBe(""));
    expect(status().hidden).toBe(true);
  });
});
