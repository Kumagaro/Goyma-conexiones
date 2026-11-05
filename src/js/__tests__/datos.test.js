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
const confirmMock = vi.hoisted(() => vi.fn());

const flush = () => new Promise((resolve) => setTimeout(resolve, 0));
const calls = (command) =>
  invokeMock.mock.calls.filter(([name]) => name === command);

describe("CSV y respaldo (HTML de producción)", () => {
  let parsed;
  let view;
  let saveDialog;
  let openDialog;

  beforeAll(async () => {
    const html = await readFile(
      path.resolve(process.cwd(), "src", "dashboard-unificado.html"),
      "utf8",
    );
    parsed = new DOMParser().parseFromString(html, "text/html");
  });

  async function start(...pageIds) {
    document.body.replaceChildren(
      ...pageIds.map((id) =>
        document.importNode(parsed.getElementById(id), true),
      ),
    );
    view = await import("../views/datosView.js");
    view.initDatosView();
  }

  beforeEach(() => {
    vi.resetModules();
    vi.clearAllMocks();
    vi.doMock("../services/invoke.js", () => ({ invoke: invokeMock }));
    vi.doMock("../components/confirmDialog.js", () => ({
      confirmDialog: confirmMock,
    }));
    invokeMock.mockResolvedValue(null);
    confirmMock.mockResolvedValue(true);
    saveDialog = vi.fn(() => Promise.resolve("D:/salida/archivo"));
    openDialog = vi.fn(() => Promise.resolve("D:/entrada/datos.csv"));
    window.__TAURI__ = { dialog: { save: saveDialog, open: openDialog } };
  });

  afterEach(() => {
    document.body.replaceChildren();
    delete window.__TAURI__;
    delete window.__datosPageChangeBound;
    vi.doUnmock("../components/confirmDialog.js");
  });

  const exportButton = (kind) =>
    document.querySelector(`[data-csv-export="${kind}"]`);

  it("cada listado tiene su botón de exportar", () => {
    for (const [page, kind] of [
      ["page-productos", "products"],
      ["page-categorias", "categories"],
      ["page-unidades", "units"],
      ["page-clientes", "clients"],
      ["page-proveedores", "suppliers"],
      ["page-facturas-entrada", "invoices"],
      ["page-facturas-venta", "invoices"],
      ["page-facturas-servicios", "invoices"],
      ["page-facturas-garantia", "invoices"],
      ["page-facturas-insumos", "invoices"],
      ["page-facturas-perdidas", "invoices"],
      ["page-facturas-general", "invoices"],
    ]) {
      expect(
        parsed.querySelector(`#${page} [data-csv-export="${kind}"]`),
        page,
      ).not.toBeNull();
    }
  });

  it("exporta con Guardar como y los filtros activos del listado", async () => {
    await start("page-categorias");
    document.getElementById("categoria-search").value = "cab";
    invokeMock.mockResolvedValue(3);

    await view.exportList(exportButton("categories"));

    expect(saveDialog.mock.calls[0][0].defaultPath).toMatch(
      /^categorias-\d{4}-\d{2}-\d{2}\.csv$/,
    );
    expect(calls("export_csv")[0][1]).toEqual({
      kind: "categories",
      filters: { search: "cab" },
      destinationPath: "D:/salida/archivo",
    });
    expect(document.body.textContent).toContain("Se exportaron 3 filas.");
  });

  it("una factura exporta su tipo y sus filtros", async () => {
    await start("page-facturas-venta");
    document.getElementById("factura-venta-desde").value = "2026-11-01";

    await view.exportList(exportButton("invoices"));

    expect(calls("export_csv")[0][1].filters).toEqual({
      invoices: { invoiceType: "V", fromDate: "2026-11-01" },
    });
  });

  it("la lista general exporta el tipo elegido en su filtro", async () => {
    await start("page-facturas-general");
    document.getElementById("factura-general-tipo").value = "S";

    await view.exportList(exportButton("invoices"));

    expect(calls("export_csv")[0][1].filters).toEqual({
      invoices: { invoiceType: "S" },
    });
  });

  it("los productos exportan los mismos filtros que su listado", async () => {
    await start("page-productos");
    document.getElementById("producto-search").value = "cable";

    await view.exportList(exportButton("products"));

    expect(calls("export_csv")[0][1].filters).toEqual({
      products: {
        search: "cable",
        categoryId: null,
        withoutCategory: false,
        stock: "all",
        expiry: "all",
      },
    });
  });

  it("cancelar Guardar como no exporta", async () => {
    await start("page-categorias");
    saveDialog.mockResolvedValue(null);
    await view.exportList(exportButton("categories"));
    expect(calls("export_csv")).toHaveLength(0);
  });

  it("importa con Abrir y resume las filas omitidas", async () => {
    await start("page-configuracion");
    document.getElementById("config-csv-tipo").value = "units";
    invokeMock.mockResolvedValue({
      imported: 2,
      skipped: [{ row: 3, reason: "<b>Ya existe</b>" }],
    });

    await view.importFile();

    expect(calls("import_csv")[0][1]).toEqual({
      kind: "units",
      sourcePath: "D:/entrada/datos.csv",
    });
    expect(document.body.textContent).toContain(
      "Se importaron 2 filas y se omitieron 1.",
    );
    const list = document.getElementById("config-csv-omitidas");
    expect(list.hidden).toBe(false);
    expect(list.textContent).toBe("Fila 3: <b>Ya existe</b>");
    expect(list.querySelector("b")).toBeNull();
  });

  it("un archivo rechazado se dice con su motivo", async () => {
    await start("page-configuracion");
    invokeMock.mockRejectedValue({
      code: "validationFailed",
      message: "El archivo está vacío",
    });
    await view.importFile();
    expect(document.body.textContent).toContain("El archivo está vacío");
  });

  it("la pestaña Respaldo avisa de que la copia no está cifrada y la crea", async () => {
    await start("page-configuracion");
    expect(document.getElementById("config-respaldo").textContent).toContain(
      "no está cifrada",
    );

    await view.saveBackup();

    expect(saveDialog.mock.calls[0][0].defaultPath).toMatch(
      /^goyma-\d{4}-\d{2}-\d{2}\.sqlite3$/,
    );
    expect(calls("create_backup")[0][1]).toEqual({
      destinationPath: "D:/salida/archivo",
    });
    expect(document.body.textContent).toContain("Copia de seguridad creada.");
  });

  it("lista las copias automáticas al entrar a Configuración", async () => {
    await start("page-configuracion");
    invokeMock.mockResolvedValue([
      {
        fileName: "goyma-auto-2026-11-05.sqlite3",
        createdAt: "2026-11-05T12:00:00Z",
        sizeBytes: 2097152,
      },
    ]);

    window.dispatchEvent(
      new CustomEvent("page:change", { detail: { pageId: "configuracion" } }),
    );

    const body = document.getElementById("config-respaldos-body");
    await vi.waitFor(() =>
      expect(body.textContent).toContain("goyma-auto-2026-11-05.sqlite3"),
    );
    expect(body.textContent).toContain("2,00 MB");
  });

  async function typePassword(value) {
    const dialog = await vi.waitFor(() => {
      const opened = document.querySelector('[role="dialog"]');
      expect(opened).not.toBeNull();
      return opened;
    });
    const input = dialog.querySelector('input[type="password"]');
    input.value = value;
    dialog
      .querySelector("form")
      .dispatchEvent(new Event("submit", { bubbles: true, cancelable: true }));
    await flush();
  }

  it("restaurar pide confirmación y contraseña; sin confirmar no hace nada", async () => {
    await start("page-configuracion");
    confirmMock.mockResolvedValueOnce(false);
    await view.restoreFromFile();
    expect(document.querySelector('[role="dialog"]')).toBeNull();
    expect(calls("restore_backup")).toHaveLength(0);
  });

  it("restaurar una copia automática envía su nombre y la contraseña", async () => {
    await start("page-configuracion");
    invokeMock.mockImplementation((command) =>
      Promise.resolve(
        command === "list_automatic_backups"
          ? [
              {
                fileName: "goyma-auto-2026-11-05.sqlite3",
                createdAt: "2026-11-05T12:00:00Z",
                sizeBytes: 1024,
              },
            ]
          : { sessionEnded: false },
      ),
    );
    await view.loadBackups();

    document.querySelector("#config-respaldos-body button").click();
    await typePassword("ValidPass1!");

    await vi.waitFor(() =>
      expect(calls("restore_backup")[0]?.[1]).toEqual({
        sourcePath: undefined,
        automaticFileName: "goyma-auto-2026-11-05.sqlite3",
        password: "ValidPass1!",
      }),
    );
  });

  it("una contraseña incorrecta deja el diálogo abierto con el motivo", async () => {
    await start("page-configuracion");
    openDialog.mockResolvedValue("D:/copias/goyma.sqlite3");
    invokeMock.mockRejectedValue({
      code: "invalidCredentials",
      message: "La contraseña no es correcta",
    });

    void view.restoreFromFile();
    await typePassword("mala");

    await vi.waitFor(() =>
      expect(document.body.textContent).toContain(
        "La contraseña no es correcta",
      ),
    );
    expect(calls("restore_backup")[0][1].sourcePath).toBe(
      "D:/copias/goyma.sqlite3",
    );
    expect(document.querySelector('[role="dialog"]')).not.toBeNull();
    document.querySelector('[role="dialog"] .btn-secondary').click();
  });
});
