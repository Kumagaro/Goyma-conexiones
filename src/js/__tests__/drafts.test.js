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

beforeEach(() => {
  vi.resetModules();
  vi.clearAllMocks();
  vi.doMock("../services/invoke.js", () => ({ invoke: invokeMock }));
  vi.doMock("../components/confirmDialog.js", () => ({
    confirmDialog: confirmMock,
  }));
  invokeMock.mockResolvedValue(null);
});

afterEach(() => {
  document.body.replaceChildren();
  vi.useRealTimers();
  vi.doUnmock("../components/confirmDialog.js");
});

function buildForm() {
  const form = document.createElement("form");
  form.append(
    Object.assign(document.createElement("input"), { id: "nombre" }),
    Object.assign(document.createElement("input"), {
      id: "activo",
      type: "checkbox",
    }),
    Object.assign(document.createElement("input"), {
      id: "clave",
      type: "password",
      value: "secreta",
    }),
    // El botón de mostrar la contraseña la pasa a type=text.
    Object.assign(document.createElement("input"), {
      id: "usuario-password",
      value: "visible",
    }),
    Object.assign(document.createElement("input"), {
      id: "adjunto",
      type: "file",
    }),
    Object.assign(document.createElement("input"), {
      id: "calculado",
      readOnly: true,
      value: "x",
    }),
  );
  document.body.append(form);
  return form;
}

describe("attachDraft", () => {
  it("guarda un segundo después de la última tecla, sin contraseñas ni archivos", async () => {
    const { attachDraft } = await import("../utils/drafts.js");
    vi.useFakeTimers();
    const form = buildForm();
    attachDraft(form, { formKey: "client", recordId: () => 4 });

    const name = form.querySelector("#nombre");
    name.value = "A";
    name.dispatchEvent(new Event("input", { bubbles: true }));
    await vi.advanceTimersByTimeAsync(900);
    name.value = "An";
    name.dispatchEvent(new Event("input", { bubbles: true }));
    await vi.advanceTimersByTimeAsync(900);
    expect(calls("save_draft")).toHaveLength(0);

    await vi.advanceTimersByTimeAsync(100);
    expect(calls("save_draft")).toHaveLength(1);
    const [, args] = calls("save_draft")[0];
    expect(args.formKey).toBe("client");
    expect(args.recordId).toBe(4);
    expect(JSON.parse(args.payload)).toEqual({ nombre: "An", activo: false });
  });

  it("un fallo al guardar no interrumpe el formulario", async () => {
    const { attachDraft } = await import("../utils/drafts.js");
    vi.useFakeTimers();
    invokeMock.mockRejectedValue(new Error("falla"));
    const form = buildForm();
    const draft = attachDraft(form, { formKey: "client" });
    draft.schedule();
    await vi.advanceTimersByTimeAsync(1000);
    expect(calls("save_draft")).toHaveLength(1);
  });

  it("clear cancela lo pendiente y borra el borrador", async () => {
    const { attachDraft } = await import("../utils/drafts.js");
    vi.useFakeTimers();
    const form = buildForm();
    const draft = attachDraft(form, { formKey: "client" });
    draft.schedule();
    draft.clear();
    await vi.advanceTimersByTimeAsync(2000);
    expect(calls("save_draft")).toHaveLength(0);
    expect(calls("delete_draft")[0][1]).toEqual({
      formKey: "client",
      recordId: null,
    });
  });

  it("Continuar carga el borrador, Descartar lo borra y cerrar lo conserva", async () => {
    const { attachDraft } = await import("../utils/drafts.js");
    const form = buildForm();
    const draft = attachDraft(form, { formKey: "client" });
    const name = form.querySelector("#nombre");
    invokeMock.mockImplementation((command) =>
      Promise.resolve(
        command === "get_draft"
          ? { payload: '{"nombre":"Ana","activo":true}' }
          : null,
      ),
    );

    confirmMock.mockResolvedValueOnce(true);
    await draft.offer();
    expect(confirmMock.mock.calls[0][0]).toMatchObject({
      message: "Tienes un formulario incompleto. ¿Deseas continuar?",
      confirmText: "Continuar",
      cancelText: "Descartar",
    });
    expect(name.value).toBe("Ana");
    expect(form.querySelector("#activo").checked).toBe(true);

    name.value = "";
    confirmMock.mockResolvedValueOnce(null);
    await draft.offer();
    expect(name.value).toBe("");
    expect(calls("delete_draft")).toHaveLength(0);

    confirmMock.mockResolvedValueOnce(false);
    await draft.offer();
    expect(name.value).toBe("");
    expect(calls("delete_draft")).toHaveLength(1);
  });

  it("tras recuperar guarda el estado completo, no el de medio camino", async () => {
    const { attachDraft, restoreForm, serializeForm } =
      await import("../utils/drafts.js");
    vi.useFakeTimers();
    const form = buildForm();
    let lines = [];
    const draft = attachDraft(form, {
      formKey: "saleInvoice",
      serialize: () => ({ ...serializeForm(form), lines }),
      restore: async (values) => {
        restoreForm(form, values);
        await Promise.resolve();
        lines = values.lines;
      },
    });
    invokeMock.mockImplementation((command) =>
      Promise.resolve(
        command === "get_draft"
          ? { payload: '{"nombre":"Ana","lines":[1,2]}' }
          : null,
      ),
    );
    confirmMock.mockResolvedValueOnce(true);

    await draft.offer();
    await vi.advanceTimersByTimeAsync(1000);

    const saved = calls("save_draft").at(-1)[1];
    expect(JSON.parse(saved.payload).lines).toEqual([1, 2]);
  });

  it("sin borrador no pregunta", async () => {
    const { attachDraft } = await import("../utils/drafts.js");
    const draft = attachDraft(buildForm(), { formKey: "client" });
    await draft.offer();
    expect(confirmMock).not.toHaveBeenCalled();
  });
});

describe("sección Borradores (HTML de producción)", () => {
  let pageHtml = "";

  beforeAll(async () => {
    const html = await readFile(
      path.resolve(process.cwd(), "src", "dashboard-unificado.html"),
      "utf8",
    );
    pageHtml = new DOMParser()
      .parseFromString(html, "text/html")
      .getElementById("page-borradores").outerHTML;
  });

  const DRAFTS = [
    {
      formKey: "saleInvoice",
      recordId: null,
      updatedAt: "2026-11-05T12:00:00Z",
    },
    { formKey: "client", recordId: 12, updatedAt: "2026-11-04T12:00:00Z" },
  ];

  async function start(drafts = DRAFTS) {
    document.body.innerHTML = `<main>${pageHtml}</main>`;
    invokeMock.mockImplementation((command) =>
      Promise.resolve(command === "list_drafts" ? drafts : null),
    );
    const view = await import("../views/borradoresView.js");
    await flush();
    return view;
  }

  it("lista formulario, registro y fecha; abre y elimina", async () => {
    const view = await start();
    const { registerDraftOpener } = await import("../utils/drafts.js");
    const open = vi.fn();
    registerDraftOpener("client", open);
    await view.loadDrafts();

    const rows = document.querySelectorAll("#borradores-body tr");
    expect(rows).toHaveLength(2);
    expect(rows[0].textContent).toContain("Factura de venta");
    expect(rows[0].textContent).toContain("Nuevo");
    expect(rows[1].textContent).toContain("Cliente");
    expect(rows[1].textContent).toContain("Edición #00012");

    const [openButton, deleteButton] = rows[1].querySelectorAll("button");
    openButton.click();
    expect(open).toHaveBeenCalledWith(12);

    confirmMock.mockResolvedValueOnce(true);
    deleteButton.click();
    await vi.waitFor(() =>
      expect(calls("delete_draft")[0]?.[1]).toEqual({
        formKey: "client",
        recordId: 12,
      }),
    );
  });

  it("si el formulario no se puede abrir lo dice", async () => {
    const view = await start();
    const { registerDraftOpener } = await import("../utils/drafts.js");
    registerDraftOpener("client", () =>
      Promise.reject(new Error("El cliente ya no existe")),
    );
    await view.loadDrafts();

    document
      .querySelectorAll("#borradores-body tr")[1]
      .querySelector("button")
      .click();

    await vi.waitFor(() =>
      expect(document.body.textContent).toContain("El cliente ya no existe"),
    );
  });

  it("avisa con un toast que lleva a Borradores", async () => {
    const view = await start();
    await view.initBorradoresView();
    await vi.waitFor(() =>
      expect(document.body.textContent).toContain(
        "Tienes 2 formularios incompletos",
      ),
    );
    window.showPage = vi.fn();
    document.querySelector(".toast-action").click();
    expect(window.showPage.mock.calls[0][0]).toBe("borradores");
    delete window.showPage;
  });

  it("sin borradores no avisa y lo dice en la lista", async () => {
    const view = await start([]);
    await view.initBorradoresView();
    await view.loadDrafts();
    expect(document.querySelector(".toast")).toBeNull();
    expect(document.getElementById("borradores-body").textContent).toContain(
      "No tienes formularios incompletos",
    );
  });
});

describe("tasas de cambio", () => {
  it("guarda lo escrito sin guardar y lo recupera al volver", async () => {
    const html = await readFile(
      path.resolve(process.cwd(), "src", "dashboard-unificado.html"),
      "utf8",
    );
    const page = new DOMParser()
      .parseFromString(html, "text/html")
      .getElementById("page-configuracion");
    document.body.replaceChildren(document.importNode(page, true));
    const responses = {
      get_user_preferences: { visibleCurrencies: [], pageSize: 20 },
      get_company_profile: {},
      get_settings: {
        lowStockThreshold: 5,
        staleInvoiceDays: 30,
        backupReminderDays: 7,
      },
      list_exchange_rates: [],
      list_exchange_rate_history: [],
    };
    let rateDraft = null;
    invokeMock.mockImplementation((command, args) => {
      if (command === "get_draft") {
        return Promise.resolve(args.formKey === "rates" ? rateDraft : null);
      }
      return Promise.resolve(responses[command] ?? null);
    });
    const { __testHooks, initConfiguracionView } =
      await import("../views/configuracionView.js");
    await initConfiguracionView();
    await __testHooks.loadConfiguration();

    const input = document.getElementById("config-tasa-CUP");
    input.value = "670";
    input.dispatchEvent(new Event("input", { bubbles: true }));
    await vi.waitFor(
      () =>
        expect(
          calls("save_draft").some(([, args]) => args.formKey === "rates"),
        ).toBe(true),
      {
        timeout: 3000,
      },
    );
    const [, saved] = calls("save_draft").find(
      ([, args]) => args.formKey === "rates",
    );
    expect(saved.formKey).toBe("rates");
    expect(JSON.parse(saved.payload).CUP).toBe("670");

    rateDraft = { payload: saved.payload };
    confirmMock.mockResolvedValueOnce(true);
    await __testHooks.loadConfiguration();
    expect(document.getElementById("config-tasa-CUP").value).toBe("670");
  });
});

describe("factura recuperada de un borrador", () => {
  let pageHtml = "";

  beforeAll(async () => {
    const html = await readFile(
      path.resolve(process.cwd(), "src", "dashboard-unificado.html"),
      "utf8",
    );
    pageHtml = new DOMParser()
      .parseFromString(html, "text/html")
      .getElementById("page-facturas-venta").outerHTML;
  });

  const line = (productId, productName) => ({
    productId,
    productIdOriginal: null,
    productName,
    unitAbbreviation: null,
    quantity: "1",
    originalQuantity: null,
    unitPrice: "",
    priceCents: 100,
    productStatus: "active",
    unitId: null,
    factor: 1,
  });

  it("vuelve a validar contacto y productos, avisa y quita lo que ya no existe", async () => {
    document.body.innerHTML = `<main>${pageHtml}</main>`;
    const payload = JSON.stringify({
      contact: { id: 9, fullName: "Ana Pérez" },
      lines: [line(1, "Cable"), line(2, "Bomba"), line(3, "Retirado")],
    });
    invokeMock.mockImplementation((command, args) => {
      if (command === "get_draft") return Promise.resolve({ payload });
      if (command === "get_client_detail") {
        return Promise.reject(new Error("no existe"));
      }
      if (command === "get_product") {
        if (args.productId === 2) return Promise.reject(new Error("no existe"));
        return Promise.resolve({
          status: args.productId === 3 ? "discontinued" : "active",
        });
      }
      return Promise.resolve([]);
    });
    confirmMock.mockResolvedValue(true);

    const { initSaleInvoicesView } = await import("../views/facturasView.js");
    await initSaleInvoicesView();
    document.getElementById("tab-factura-venta-create").click();

    await vi.waitFor(() =>
      expect(document.body.textContent).toContain(
        "«Bomba» ya no existe y se quitó de la factura.",
      ),
    );
    expect(document.body.textContent).toContain(
      "«Retirado» ya no existe y se quitó de la factura.",
    );
    expect(document.body.textContent).toContain(
      "El cliente del borrador ya no existe y se quitó.",
    );
    await vi.waitFor(() =>
      expect(document.querySelectorAll("[data-line-index]")).toHaveLength(1),
    );
    expect(document.querySelector("[data-line-index]").textContent).toContain(
      "Cable",
    );
  });
});
