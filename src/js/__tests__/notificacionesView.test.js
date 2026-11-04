import { readFile } from "node:fs/promises";
import path from "node:path";
import {
  afterEach,
  beforeAll,
  beforeEach,
  describe,
  expect,
  it,
  vi,
} from "vitest";
import { toastsOfType, toastText } from "./helpers/invoiceUi.js";

// Los mismos dobles para cada instancia del módulo (vi.resetModules).
const services = vi.hoisted(() => ({
  listNotifications: vi.fn(),
  countUnreadNotifications: vi.fn(),
  listNotificationsSince: vi.fn(),
  markNotificationRead: vi.fn(),
  markAllNotificationsRead: vi.fn(),
  deleteNotification: vi.fn(),
  deleteNotifications: vi.fn(),
  getInvoice: vi.fn(),
  getProduct: vi.fn(),
}));

vi.mock("../services/notificationsService.js", () => ({
  listNotifications: services.listNotifications,
  countUnreadNotifications: services.countUnreadNotifications,
  listNotificationsSince: services.listNotificationsSince,
  markNotificationRead: services.markNotificationRead,
  markAllNotificationsRead: services.markAllNotificationsRead,
  deleteNotification: services.deleteNotification,
  deleteNotifications: services.deleteNotifications,
}));
vi.mock("../services/invoicesService.js", () => ({
  getInvoice: services.getInvoice,
}));
vi.mock("../services/productsService.js", () => ({
  getProduct: services.getProduct,
}));

const NOW = new Date();
const minutesAgo = (minutes) =>
  new Date(NOW.getTime() - minutes * 60_000).toISOString();

const ITEMS = [
  {
    id: 3,
    type: "N-02",
    priority: "CRÍTICO",
    message: "Producto agotado: Leche",
    entityType: "product",
    entityId: 11,
    isRead: false,
    createdAt: minutesAgo(5),
  },
  {
    id: 2,
    type: "N-05",
    priority: "ADVERTENCIA",
    message:
      "Reserva por vencerse: Factura # V-00007 - Cliente: Rosa Díaz - Días restantes: 2 días",
    entityType: "invoice",
    entityId: 7,
    isRead: false,
    createdAt: minutesAgo(10),
  },
  {
    id: 1,
    type: "N-12",
    priority: "INFORMATIVO",
    message:
      "Se eliminaron automáticamente estos productos descontinuados: Queso",
    entityType: null,
    entityId: null,
    isRead: true,
    createdAt: minutesAgo(60 * 24 * 3),
  },
];

let pageHtml = "";
let feed;
let view;

beforeAll(async () => {
  const html = await readFile(
    path.resolve(process.cwd(), "src", "dashboard-unificado.html"),
    "utf8",
  );
  const doc = new DOMParser().parseFromString(html, "text/html");
  pageHtml = [
    doc.getElementById("sidebar").outerHTML,
    doc.querySelector(".notificaciones").outerHTML,
    doc.getElementById("page-notificaciones").outerHTML,
  ].join("");
});

async function openView() {
  vi.resetModules();
  document.body.innerHTML = pageHtml;
  document.getElementById("page-notificaciones").classList.add("active");
  feed = await import("../services/notificationFeed.js");
  feed.__resetNotificationFeed();
  view = await import("../views/notificacionesView.js");
  await view.initNotificacionesView();
  await view.__testHooks.loadNotifications();
}

const rows = () => [
  ...document.querySelectorAll("#notificaciones-list [data-notification-id]"),
];
const badge = () => document.getElementById("topbar-notif-badge");

describe("pestaña de notificaciones (spec-04 NT-01, NT-04, TO-02)", () => {
  beforeEach(() => {
    Object.values(services).forEach((mock) => mock.mockReset());
    services.listNotifications.mockResolvedValue(ITEMS);
    services.countUnreadNotifications.mockResolvedValue(2);
    services.listNotificationsSince.mockResolvedValue([]);
    services.markNotificationRead.mockResolvedValue(null);
    services.markAllNotificationsRead.mockResolvedValue(2);
    services.deleteNotification.mockResolvedValue(null);
    services.deleteNotifications.mockResolvedValue(3);
    window.showPage = vi.fn();
    window.productosView = { openProductDetail: vi.fn() };
    window.irAFacturaDesdeGeneral = vi.fn();
  });

  afterEach(() => {
    feed?.__resetNotificationFeed();
    delete window.showPage;
    delete window.productosView;
    delete window.irAFacturaDesdeGeneral;
  });

  it("lista datos reales con prioridad, mensaje y hora; contador y un solo toast resumen", async () => {
    await openView();

    expect(rows().map((row) => row.dataset.notificationId)).toEqual([
      "3",
      "2",
      "1",
    ]);
    const first = rows()[0];
    expect(first.querySelector(".notif-prioridad").textContent).toBe("Crítico");
    expect(first.querySelector(".notif-desc").textContent).toBe(
      "Producto agotado: Leche",
    );
    expect(first.querySelector(".notif-time").textContent).toMatch(
      /^(Hoy|Ayer) · \d{2}:\d{2}$/,
    );
    expect(first.classList.contains("no-leida")).toBe(true);
    expect(rows()[2].classList.contains("no-leida")).toBe(false);
    expect(rows()[1].querySelector(".notif-prioridad").textContent).toBe(
      "Advertencia",
    );

    expect(badge().textContent).toBe("2");
    expect(badge().style.display).toBe("");
    expect(document.getElementById("notif-unread-count").textContent).toBe("2");
    expect(toastsOfType("info")).toHaveLength(1);
    expect(toastText()).toContain("Tienes 2 notificaciones sin leer");
  });

  it("las pestañas, el buscador y la fecha filtran en el backend", async () => {
    await openView();
    services.listNotifications.mockClear();

    document.getElementById("tab-notif-no-leidas").click();
    await vi.waitFor(() =>
      expect(services.listNotifications).toHaveBeenLastCalledWith({
        unreadOnly: true,
        search: null,
        date: null,
      }),
    );
    expect(
      document
        .getElementById("tab-notif-no-leidas")
        .classList.contains("active"),
    ).toBe(true);

    const search = document.getElementById("notificacion-search");
    search.value = "leche";
    search.dispatchEvent(new Event("input", { bubbles: true }));
    const date = document.getElementById("notificacion-fecha");
    date.value = "2026-10-28";
    date.dispatchEvent(new Event("change", { bubbles: true }));
    await vi.waitFor(() =>
      expect(services.listNotifications).toHaveBeenLastCalledWith({
        unreadOnly: true,
        search: "leche",
        date: "2026-10-28",
      }),
    );

    document.getElementById("notif-limpiar-fecha").click();
    await vi.waitFor(() =>
      expect(services.listNotifications).toHaveBeenLastCalledWith({
        unreadOnly: true,
        search: "leche",
        date: null,
      }),
    );
  });

  it("clic: marca leída y abre el producto o la factura; si ya no existe, avisa", async () => {
    await openView();
    services.getProduct.mockResolvedValue({ id: 11 });
    rows()[0].click();
    await vi.waitFor(() =>
      expect(window.productosView.openProductDetail).toHaveBeenCalledWith(11),
    );
    expect(services.markNotificationRead).toHaveBeenCalledWith(3);
    expect(window.showPage).toHaveBeenCalledWith(
      "productos",
      document.querySelector('#sidebar [data-page="productos"]'),
    );

    services.getInvoice.mockResolvedValue({ id: 7, invoiceType: "V" });
    rows()[1].click();
    await vi.waitFor(() =>
      expect(window.irAFacturaDesdeGeneral).toHaveBeenCalledWith("V", 7),
    );

    services.getProduct.mockRejectedValue({
      code: "notFound",
      message: "El producto ya no existe",
    });
    window.productosView.openProductDetail.mockClear();
    rows()[0].click();
    await vi.waitFor(() =>
      expect(toastText()).toContain("El producto ya no existe"),
    );
    expect(window.productosView.openProductDetail).not.toHaveBeenCalled();

    // La de la purga no abre nada; ya leída, no se vuelve a marcar.
    services.markNotificationRead.mockClear();
    rows()[2].click();
    await Promise.resolve();
    expect(services.markNotificationRead).not.toHaveBeenCalled();
  });

  it("marcar todas, eliminar una y eliminar todas (las filtradas) sin confirmación", async () => {
    await openView();

    document.getElementById("notif-marcar-todas").click();
    await vi.waitFor(() =>
      expect(services.markAllNotificationsRead).toHaveBeenCalled(),
    );

    services.countUnreadNotifications.mockResolvedValue(1);
    rows()[1].querySelector("[data-notification-delete]").click();
    await vi.waitFor(() =>
      expect(services.deleteNotification).toHaveBeenCalledWith(2),
    );
    await vi.waitFor(() => expect(badge().textContent).toBe("1"));
    // Borrar no abre la factura.
    expect(services.getInvoice).not.toHaveBeenCalled();

    document.getElementById("tab-notif-no-leidas").click();
    services.countUnreadNotifications.mockResolvedValue(0);
    document.getElementById("notif-eliminar-todas").click();
    await vi.waitFor(() =>
      expect(services.deleteNotifications).toHaveBeenCalledWith({
        unreadOnly: true,
        search: null,
        date: null,
      }),
    );
    await vi.waitFor(() =>
      expect(toastText()).toContain("Se eliminaron 3 notificaciones."),
    );
    await vi.waitFor(() => expect(badge().style.display).toBe("none"));
  });

  it("tras un comando de escritura muestra un toast por cada notificación nueva con su prioridad", async () => {
    await openView();
    const before = toastsOfType("critical").length;
    services.listNotificationsSince.mockResolvedValueOnce([
      { ...ITEMS[0], id: 4, message: "Producto agotado: Café" },
      {
        ...ITEMS[1],
        id: 5,
        message: "Producto con stock bajo: Azúcar - Cantidad actual: 2 kg",
      },
    ]);
    services.countUnreadNotifications.mockResolvedValue(4);

    window.dispatchEvent(
      new CustomEvent("command:success", {
        detail: { command: "create_sale_invoice" },
      }),
    );
    await vi.waitFor(() => expect(badge().textContent).toBe("4"));
    // Pide desde la más reciente que ya había (id 3).
    expect(services.listNotificationsSince).toHaveBeenCalledWith(3);
    expect(toastsOfType("critical")).toHaveLength(before + 1);
    expect(toastsOfType("warning").length).toBeGreaterThan(0);
    expect(toastText()).toContain("Producto agotado: Café");
    expect(toastText()).toContain(
      "Producto con stock bajo: Azúcar - Cantidad actual: 2 kg",
    );

    // La siguiente revisión parte del último id visto.
    window.dispatchEvent(
      new CustomEvent("command:success", { detail: { command: "x" } }),
    );
    await vi.waitFor(() =>
      expect(services.listNotificationsSince).toHaveBeenLastCalledWith(5),
    );
  });
});
