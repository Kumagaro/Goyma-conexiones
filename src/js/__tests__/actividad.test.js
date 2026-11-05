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
const sessionMock = vi.hoisted(() => ({ role: "administrador" }));

const ENTRIES = [
  {
    id: 2,
    createdAt: "2026-11-05T12:00:00Z",
    userName: "ana",
    action: "delete",
    entityType: "product",
    entityLabel: '<img src=x onerror="alert(1)">Cable',
    details: null,
  },
  {
    id: 1,
    createdAt: "2026-11-04T12:00:00Z",
    userName: "Sistema",
    action: "expire",
    entityType: "invoice",
    entityLabel: "V-00004",
    details: "Tiempo límite excedido",
  },
];

describe("Registro de actividad (HTML de producción)", () => {
  let html = "";

  beforeAll(async () => {
    html = await readFile(
      path.resolve(process.cwd(), "src", "dashboard-unificado.html"),
      "utf8",
    );
  });

  async function start(role = "administrador") {
    sessionMock.role = role;
    const parsed = new DOMParser().parseFromString(html, "text/html");
    const link = parsed.querySelector('a[data-page="actividad"]').closest("li");
    document.body.replaceChildren(
      document.importNode(link, true),
      document.importNode(parsed.getElementById("page-actividad"), true),
    );
    const view = await import("../views/actividadView.js");
    view.initActividadView();
    return view;
  }

  const lastFilters = () => invokeMock.mock.calls.at(-1)[1].filters;

  beforeEach(() => {
    vi.resetModules();
    vi.clearAllMocks();
    vi.doMock("../services/invoke.js", () => ({ invoke: invokeMock }));
    vi.doMock("../session.js", () => ({
      getSession: () => sessionMock,
      onSessionChange: vi.fn(),
    }));
    invokeMock.mockResolvedValue(ENTRIES);
  });

  afterEach(() => {
    document.body.replaceChildren();
    vi.doUnmock("../session.js");
    delete window.__actividadPageChangeBound;
  });

  it("solo el Administrador ve la sección en el menú", async () => {
    await start("contador");
    const link = document.querySelector('a[data-page="actividad"]');
    expect(link.closest("li").hidden).toBe(true);
    document.body.replaceChildren();
    vi.resetModules();
    await start("administrador");
    expect(
      document.querySelector('a[data-page="actividad"]').closest("li").hidden,
    ).toBe(false);
  });

  it("pinta las entradas como texto seguro", async () => {
    const view = await start();
    await view.loadActivity();
    const rows = document.querySelectorAll("#actividad-body tr");
    expect(rows).toHaveLength(2);
    expect(rows[0].textContent).toContain("Eliminación");
    expect(rows[0].textContent).toContain("Producto");
    expect(rows[1].textContent).toContain("Sistema");
    expect(rows[1].textContent).toContain("Reserva vencida");
    expect(rows[1].textContent).toContain("Tiempo límite excedido");
    expect(document.querySelector("#actividad-body img")).toBeNull();
  });

  it("los filtros, la búsqueda y el orden llegan al comando", async () => {
    const view = await start();
    await view.loadActivity();

    const action = document.getElementById("actividad-accion");
    expect(action.options.length).toBe(10);
    action.value = "cancel";
    action.dispatchEvent(new Event("change"));
    await vi.waitFor(() => expect(lastFilters().action).toBe("cancel"));

    const user = document.getElementById("actividad-usuario");
    user.value = "ana";
    user.dispatchEvent(new Event("change"));
    await vi.waitFor(() => expect(lastFilters().userName).toBe("ana"));

    const from = document.getElementById("actividad-desde");
    from.value = "2026-11-01";
    from.dispatchEvent(new Event("change"));
    await vi.waitFor(() => expect(lastFilters().fromDate).toBe("2026-11-01"));

    const search = document.getElementById("actividad-buscar");
    search.value = "cable";
    search.dispatchEvent(new Event("input"));
    await vi.waitFor(() => expect(lastFilters().search).toBe("cable"));

    expect(lastFilters().oldestFirst).toBe(false);
    document.getElementById("actividad-orden-boton").click();
    await vi.waitFor(() => expect(lastFilters().oldestFirst).toBe(true));
    expect(
      document.getElementById("actividad-orden").getAttribute("aria-sort"),
    ).toBe("ascending");
  });

  it("un fallo se dice en la tabla y una lista vacía también", async () => {
    const view = await start();
    invokeMock.mockResolvedValueOnce([]);
    await view.loadActivity();
    expect(document.getElementById("actividad-body").textContent).toContain(
      "No hay actividad",
    );
    invokeMock.mockRejectedValueOnce(new Error("falla"));
    await view.loadActivity();
    expect(document.getElementById("actividad-body").textContent).toContain(
      "No se pudo cargar",
    );
  });

  it("se recarga al entrar a la sección", async () => {
    await start();
    invokeMock.mockClear();
    window.dispatchEvent(
      new CustomEvent("page:change", { detail: { pageId: "actividad" } }),
    );
    await vi.waitFor(() => expect(invokeMock).toHaveBeenCalled());
  });
});
