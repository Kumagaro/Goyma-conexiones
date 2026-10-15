import { afterEach, beforeEach, describe, expect, it, vi } from "vitest";

vi.mock("../services/suppliersService.js", () => ({
  listSuppliers: vi.fn(),
  searchSuppliers: vi.fn(),
  createSupplier: vi.fn(),
  updateSupplier: vi.fn(),
  deleteSupplier: vi.fn(),
  getSupplierDetail: vi.fn(),
}));

vi.mock("../components/confirmDialog.js", () => ({
  confirmDialog: vi.fn(() => Promise.resolve(true)),
}));

import {
  createSupplier,
  deleteSupplier,
  getSupplierDetail,
  listSuppliers,
  searchSuppliers,
  updateSupplier,
} from "../services/suppliersService.js";
import { initProveedoresView } from "../views/proveedoresView.js";

function seedSupplierPage() {
  document.body.innerHTML = `
    <main>
      <section id="page-proveedores" class="page active">
        <div class="view-tabs">
          <button id="tab-proveedor-list" class="tab active">Proveedores</button>
          <button id="tab-proveedor-create" class="tab">Crear Nuevo Proveedor</button>
          <button id="tab-proveedor-historial" class="tab">Historial de Compras</button>
        </div>
        <div id="proveedor-search-bar">
          <input id="proveedor-search" />
        </div>
        <div id="proveedor-list" class="config-subview active">
          <table><tbody id="proveedores-body"></tbody></table>
        </div>
        <div id="proveedor-create" class="config-subview">
          <form id="proveedor-form">
            <input id="proveedor-nombres" />
            <input id="proveedor-apellidos" />
            <input id="proveedor-ci" />
            <input id="proveedor-telefono" />
            <input id="proveedor-correo" />
            <input id="proveedor-empresa" />
            <input id="proveedor-cargo" />
            <input id="proveedor-direccion" />
            <button type="submit">Guardar</button>
          </form>
        </div>
        <div id="proveedor-historial" class="config-subview">
          <select id="historial-proveedor-select"></select>
          <tbody id="historial-proveedor-body"></tbody>
        </div>
        <div id="proveedor-ver" class="config-subview">
          <span id="proveedor-ver-id"></span>
          <h2 id="proveedor-ver-nombre-completo"></h2>
          <span id="proveedor-ver-empresa"></span>
          <span id="proveedor-ver-cargo"></span>
          <span id="proveedor-ver-telefono"></span>
          <span id="proveedor-ver-correo"></span>
          <span id="proveedor-ver-ci"></span>
          <span id="proveedor-ver-direccion"></span>
          <span id="proveedor-ver-total-comprado"></span>
          <span id="proveedor-ver-saldo-pendiente"></span>
          <span id="proveedor-ver-num-facturas"></span>
          <span id="proveedor-ver-ultima-compra"></span>
          <div id="proveedor-ver-historial-facturas"></div>
          <button id="proveedor-ver-editar" type="button">Editar</button>
          <button id="proveedor-ver-eliminar" type="button">Eliminar</button>
        </div>
      </section>
    </main>
  `;
}

describe("proveedoresView", () => {
  beforeEach(() => {
    vi.useFakeTimers();
    seedSupplierPage();
    vi.clearAllMocks();
  });

  afterEach(() => {
    vi.runOnlyPendingTimers();
    vi.useRealTimers();
    document.body.innerHTML = "";
  });

  it("carga proveedores y muestra el estado vacío si no llega nada", async () => {
    listSuppliers.mockResolvedValue([]);

    await initProveedoresView();

    expect(listSuppliers).toHaveBeenCalledTimes(1);
    expect(document.getElementById("proveedores-body").textContent).toContain(
      "Aún no hay proveedores",
    );
  });

  it("debouncea la búsqueda y reacciona a page:change", async () => {
    searchSuppliers.mockResolvedValue([]);

    await initProveedoresView();
    const input = document.getElementById("proveedor-search");
    input.value = "Ana";
    input.dispatchEvent(new Event("input", { bubbles: true }));

    await Promise.resolve();
    expect(searchSuppliers).not.toHaveBeenCalled();

    await vi.advanceTimersByTimeAsync(300);
    expect(searchSuppliers).toHaveBeenCalledWith("Ana");

    window.dispatchEvent(
      new CustomEvent("page:change", { detail: { pageId: "proveedores" } }),
    );
    await Promise.resolve();
    expect(listSuppliers).toHaveBeenCalledTimes(2);
  });

  it("crea, detalla y elimina un proveedor con salida segura", async () => {
    listSuppliers.mockResolvedValue([
      {
        id: 3,
        fullName: "Marta López",
        company: "Goyma",
        phone: "+53 55512345",
      },
    ]);

    getSupplierDetail.mockResolvedValue({
      id: 3,
      firstName: "Marta",
      lastName: "López",
      fullName: "Marta López",
      nationalId: "90010112345",
      phone: "+53 55512345",
      company: "Goyma",
      position: "Gerente",
      email: "marta@goyma.test",
      address: "Calle 2",
      createdAt: "2026-02-05",
      indicators: {
        totalInvoicedCents: 0,
        pendingBalanceCents: 0,
        invoiceCount: 0,
        lastPurchaseDate: null,
      },
      invoiceHistory: [],
      expiredCleanupWarning: null,
    });

    createSupplier.mockResolvedValue({ id: 99 });
    deleteSupplier.mockResolvedValue({ deleted: true });

    await initProveedoresView();
    const form = document.getElementById("proveedor-form");
    document.getElementById("proveedor-nombres").value = "Luis";
    document.getElementById("proveedor-apellidos").value = "Pérez";
    document.getElementById("proveedor-ci").value = "90010112345";
    document.getElementById("proveedor-telefono").value = "+53 55512345";
    document.getElementById("proveedor-correo").value = "luis@goyma.test";
    document.getElementById("proveedor-empresa").value = "Goyma";
    document.getElementById("proveedor-cargo").value = "Analista";
    document.getElementById("proveedor-direccion").value = "Calle 9";

    form.dispatchEvent(new Event("submit", { bubbles: true, cancelable: true }));
    await vi.waitFor(() => {
      expect(createSupplier).toHaveBeenCalledWith({
        firstName: "Luis",
        lastName: "Pérez",
        nationalId: "90010112345",
        phone: "+53 55512345",
        email: "luis@goyma.test",
        company: "Goyma",
        position: "Analista",
        address: "Calle 9",
      });
    });

    await vi.waitFor(() => {
      expect(document.querySelector('#proveedores-body tr[data-supplier-id="3"]')).not.toBeNull();
    });
    const row = document.querySelector('#proveedores-body tr[data-supplier-id="3"]');
    row.querySelector('[data-supplier-action="detail"]').click();
    await vi.waitFor(() => {
      expect(document.getElementById("proveedor-ver-nombre-completo").textContent).toContain(
        "Marta López",
      );
    });
    expect(document.getElementById("proveedor-ver-fecha-entrada")).toBeNull();

    const safeName = "<img src=x onerror=alert(1)>Ana";
    listSuppliers.mockResolvedValueOnce([
      {
        id: 7,
        fullName: safeName,
        company: "",
        phone: "+53 55512345",
      },
    ]);
    await initProveedoresView();
    expect(document.getElementById("proveedores-body").textContent).toContain("Ana");
    expect(document.getElementById("proveedores-body").querySelector("img")).toBeNull();

    await vi.waitFor(() => {
      expect(document.querySelector('#proveedores-body tr[data-supplier-id="7"]')).not.toBeNull();
    });
    const deleteRow = document.querySelector('#proveedores-body tr[data-supplier-id="7"]');
    deleteRow.querySelector('[data-supplier-action="delete"]').click();
    await vi.waitFor(() => {
      expect(deleteSupplier).toHaveBeenCalledWith(7);
    });
  });
});
