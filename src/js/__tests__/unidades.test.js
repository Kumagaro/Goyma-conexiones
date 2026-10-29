import { afterEach, beforeEach, describe, expect, it, vi } from "vitest";
import { toastText } from "./helpers/invoiceUi.js";

const listUnitsMock = vi.hoisted(() => vi.fn());
const searchUnitsMock = vi.hoisted(() => vi.fn());
const createUnitMock = vi.hoisted(() => vi.fn());
const updateUnitMock = vi.hoisted(() => vi.fn());
const deleteUnitMock = vi.hoisted(() => vi.fn());
const confirmDialogMock = vi.hoisted(() => vi.fn().mockResolvedValue(true));

describe("servicio de unidades", () => {
  beforeEach(() => {
    vi.resetModules();
    vi.clearAllMocks();
  });

  it("llama a los comandos con el nombre snake_case correcto", async () => {
    vi.doMock("../services/invoke.js", () => ({
      invoke: vi.fn(),
    }));

    const service = await import("../services/unitsService.js");
    const { invoke } = await import("../services/invoke.js");

    invoke.mockResolvedValue([]);

    await service.listUnits("kg");
    await service.searchUnits("kg");
    await service.createUnit("Kilogramo", "kg");
    await service.updateUnit(12, "Kilogramo", "kg");
    await service.deleteUnit(12);

    expect(invoke).toHaveBeenNthCalledWith(1, "list_units", { search: "kg" });
    expect(invoke).toHaveBeenNthCalledWith(2, "search_units", { text: "kg" });
    expect(invoke).toHaveBeenNthCalledWith(3, "create_unit", {
      name: "Kilogramo",
      abbreviation: "kg",
    });
    expect(invoke).toHaveBeenNthCalledWith(4, "update_unit", {
      unitId: 12,
      name: "Kilogramo",
      abbreviation: "kg",
    });
    expect(invoke).toHaveBeenNthCalledWith(5, "delete_unit", { unitId: 12 });
  });
});

describe("vista de unidades", () => {
  beforeEach(() => {
    vi.resetModules();
    document.body.innerHTML = `
      <div id="page-unidades">
        <input id="unidad-search" />
        <div id="unidad-feedback" role="status" hidden></div>
        <table>
          <thead>
            <tr>
              <th>ID</th>
              <th>Nombre</th>
              <th>Abreviación</th>
              <th>Productos con Unidad</th>
              <th>Acciones</th>
            </tr>
          </thead>
          <tbody id="unidades-body"></tbody>
        </table>
        <div id="modal-unidad" class="modal-overlay">
          <form id="unidad-form">
            <input id="modal-unidad-nombre" name="nombre" value="" />
            <input id="modal-unidad-abreviacion" name="abreviacion" value="" />
            <button type="submit">Guardar</button>
            <button type="button" id="modal-unidad-cancelar">Cancelar</button>
          </form>
        </div>
      </div>
    `;
  });

  afterEach(() => {
    document.body.innerHTML = "";
    delete window.__TAURI__;
  });

  it("pinta las filas, muestra el aviso de duplicado y confirma la eliminación con los contadores reales", async () => {
    confirmDialogMock.mockResolvedValue(true);
    vi.doMock("../services/unitsService.js", () => ({
      listUnits: listUnitsMock,
      searchUnits: searchUnitsMock,
      createUnit: createUnitMock,
      updateUnit: updateUnitMock,
      deleteUnit: deleteUnitMock,
    }));
    vi.doMock("../components/confirmDialog.js", () => ({
      confirmDialog: confirmDialogMock,
    }));

    listUnitsMock.mockResolvedValue({
      items: [
        {
          id: 1,
          name: "Kilogramo",
          abbreviation: "kg",
          productCount: 2,
          conversionCount: 4,
        },
        {
          id: 2,
          name: "Gramo",
          abbreviation: "g",
          productCount: 0,
          conversionCount: 1,
        },
      ],
      expiredCleanupWarning: null,
    });
    searchUnitsMock.mockResolvedValue([
      { id: 1, name: "Kilogramo", abbreviation: "kg", isExactMatch: true },
    ]);
    deleteUnitMock.mockResolvedValue({
      affectedProducts: 2,
      deletedConversions: 4,
    });

    const view = await import("../views/unidadesView.js");
    await view.initUnidadesView();

    expect(document.querySelectorAll("#unidades-body tr")).toHaveLength(2);
    expect(document.getElementById("unidades-body").textContent).toContain(
      "Kilogramo",
    );
    expect(document.getElementById("unidades-body").textContent).not.toContain(
      "conversionCount",
    );

    document.getElementById("modal-unidad-nombre").value = "Kilogramo";
    document
      .getElementById("modal-unidad-nombre")
      .dispatchEvent(new Event("input", { bubbles: true }));

    await vi.waitFor(() => {
      expect(searchUnitsMock).toHaveBeenCalledWith("Kilogramo");
    });
    // Aviso en vivo del modal: sigue junto al formulario, no es un toast.
    expect(document.getElementById("unidad-feedback").textContent).toContain(
      "ya existe",
    );

    const row = document.querySelector("#unidades-body tr");
    row.querySelector("button[data-unit-action='delete']").click();
    await vi.waitFor(() => {
      expect(deleteUnitMock).toHaveBeenCalledWith(1);
    });
    expect(toastText()).toContain("2");
  });

  it("abre la unidad desde el formulario de producto y cancelar no crea ni selecciona", async () => {
    vi.doMock("../services/unitsService.js", () => ({
      listUnits: listUnitsMock,
      searchUnits: searchUnitsMock,
      createUnit: createUnitMock,
      updateUnit: updateUnitMock,
      deleteUnit: deleteUnitMock,
    }));
    listUnitsMock.mockResolvedValue({ items: [], expiredCleanupWarning: null });

    const view = await import("../views/unidadesView.js");
    await view.initUnidadesView();
    const onCreated = vi.fn();
    view.openUnitModalForProduct("Litro", onCreated);

    expect(
      document.getElementById("modal-unidad").classList.contains("active"),
    ).toBe(true);
    expect(document.getElementById("modal-unidad-nombre").value).toBe("Litro");
    expect(document.getElementById("modal-unidad-nombre").readOnly).toBe(false);
    expect(document.getElementById("modal-unidad-abreviacion").value).toBe("");
    expect(document.getElementById("modal-unidad-abreviacion").readOnly).toBe(
      false,
    );

    document.getElementById("modal-unidad-cancelar").click();

    expect(createUnitMock).not.toHaveBeenCalled();
    expect(onCreated).not.toHaveBeenCalled();
    expect(
      document.getElementById("modal-unidad").classList.contains("active"),
    ).toBe(false);
  });
});
