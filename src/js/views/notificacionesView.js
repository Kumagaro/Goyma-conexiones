// Pestaña de Notificaciones con datos reales (spec-04 NT-01, NT-04).
// Sustituye a notificacionesData/renderNotificaciones del mockup.
import {
  PRIORITY_LABEL,
  onNotificationsChanged,
  refreshUnreadCount,
  startNotificationFeed,
} from "../services/notificationFeed.js";
import {
  deleteNotification,
  deleteNotifications,
  listNotifications,
  markAllNotificationsRead,
  markNotificationRead,
} from "../services/notificationsService.js";
import { getInvoice } from "../services/invoicesService.js";
import { getProduct } from "../services/productsService.js";
import { el } from "../utils/dom.js";
import { notifyError, notifyInfo, notifySuccess } from "../utils/feedback.js";
import { debounce } from "../utils/requestGuard.js";

const PAGE_ID = "notificaciones";
const PRIORITY_CLASS = {
  CRÍTICO: "critico",
  ADVERTENCIA: "advertencia",
  INFORMATIVO: "informativo",
};
const PRIORITY_ICON = {
  CRÍTICO: "icon-alert",
  ADVERTENCIA: "icon-alert",
  INFORMATIVO: "icon-bell",
};

let currentTab = "todas";
let items = [];
let loadToken = 0;

const byId = (id) => document.getElementById(id);

function readFilters() {
  return {
    unreadOnly: currentTab === "no-leidas",
    search: byId("notificacion-search")?.value.trim() || null,
    date: byId("notificacion-fecha")?.value || null,
  };
}

function isPageActive() {
  return byId("page-notificaciones")?.classList.contains("active") ?? false;
}

function svgIcon(iconId, className = "icon") {
  const svgNs = "http://www.w3.org/2000/svg";
  const svg = document.createElementNS(svgNs, "svg");
  svg.setAttribute("class", className);
  svg.setAttribute("aria-hidden", "true");
  const use = document.createElementNS(svgNs, "use");
  use.setAttribute("href", `#${iconId}`);
  svg.append(use);
  return svg;
}

function localDayKey(date) {
  return `${date.getFullYear()}-${date.getMonth()}-${date.getDate()}`;
}

/** "Hoy · 14:05", "Ayer · 09:30" o "27/10/2026 · 18:00" en hora local. */
export function formatNotificationTime(createdAt, now = new Date()) {
  const date = new Date(createdAt);
  if (Number.isNaN(date.getTime())) return "—";
  const two = (value) => String(value).padStart(2, "0");
  const time = `${two(date.getHours())}:${two(date.getMinutes())}`;
  const yesterday = new Date(now);
  yesterday.setDate(now.getDate() - 1);
  let day = `${two(date.getDate())}/${two(date.getMonth() + 1)}/${date.getFullYear()}`;
  if (localDayKey(date) === localDayKey(now)) day = "Hoy";
  else if (localDayKey(date) === localDayKey(yesterday)) day = "Ayer";
  return `${day} · ${time}`;
}

function renderRow(item) {
  const priority = PRIORITY_CLASS[item.priority] ?? "informativo";
  const title = el(
    "div",
    { class: "notif-title" },
    el(
      "span",
      { class: `notif-prioridad notif-prioridad-${priority}` },
      PRIORITY_LABEL[item.priority] ?? "Informativo",
    ),
    item.isRead
      ? null
      : el("span", { class: "notif-dot", "aria-label": "Sin leer" }),
  );
  const deleteButton = el(
    "button",
    {
      type: "button",
      class: "notif-delete",
      "aria-label": "Eliminar notificación",
      title: "Eliminar",
      dataset: { notificationDelete: item.id },
    },
    svgIcon("icon-trash"),
  );
  return el(
    "div",
    {
      class: `notif-row notif-row-${priority}${item.isRead ? "" : " no-leida"}`,
      role: "button",
      tabindex: "0",
      dataset: { notificationId: item.id },
    },
    el(
      "div",
      { class: "notif-icon" },
      svgIcon(
        PRIORITY_ICON[item.priority] ?? "icon-bell",
        "icon notif-icon-svg",
      ),
    ),
    el(
      "div",
      { class: "notif-body" },
      title,
      el("div", { class: "notif-desc" }, item.message),
    ),
    el("div", { class: "notif-time" }, formatNotificationTime(item.createdAt)),
    deleteButton,
  );
}

function renderList() {
  const list = byId("notificaciones-list");
  if (!list) return;
  if (!items.length) {
    list.replaceChildren(
      el(
        "div",
        { class: "empty-row" },
        "No hay notificaciones que coincidan con los filtros.",
      ),
    );
    return;
  }
  list.replaceChildren(...items.map(renderRow));
}

export async function loadNotifications() {
  const token = ++loadToken;
  try {
    const result = (await listNotifications(readFilters())) ?? [];
    // Solo la última petición pinta (filtros escritos rápido).
    if (token !== loadToken) return;
    items = result;
    renderList();
  } catch (error) {
    if (token !== loadToken) return;
    items = [];
    renderList();
    notifyError(error, "No se pudieron cargar las notificaciones.");
  }
}

async function afterChange() {
  await loadNotifications();
  await refreshUnreadCount().catch(() => {});
}

/** NT-04: marca leída y abre el producto o la factura afectada. */
async function openNotification(item) {
  try {
    if (!item.isRead) await markNotificationRead(item.id);
  } catch (error) {
    notifyError(error, "No se pudo marcar la notificación como leída.");
    await afterChange();
    return;
  }
  await afterChange();
  if (item.entityType === "product") await openProduct(item.entityId);
  else if (item.entityType === "invoice") await openInvoice(item.entityId);
}

function sidebarLink(pageId) {
  return document.querySelector(`#sidebar [data-page="${pageId}"]`);
}

async function openProduct(productId) {
  try {
    await getProduct(productId);
  } catch (error) {
    if (error?.code === "notFound") notifyInfo("El producto ya no existe");
    else notifyError(error, "No se pudo abrir el producto.");
    return;
  }
  window.showPage?.("productos", sidebarLink("productos"));
  await window.productosView?.openProductDetail?.(productId);
}

async function openInvoice(invoiceId) {
  let detail;
  try {
    detail = await getInvoice(invoiceId);
  } catch (error) {
    if (error?.code === "notFound") notifyInfo("La factura ya no existe");
    else notifyError(error, "No se pudo abrir la factura.");
    return;
  }
  await window.irAFacturaDesdeGeneral?.(detail.invoiceType, Number(invoiceId));
}

function showTab(tab) {
  currentTab = tab;
  for (const [name, id] of [
    ["todas", "tab-notif-todas"],
    ["no-leidas", "tab-notif-no-leidas"],
  ]) {
    byId(id)?.classList.toggle("active", name === tab);
  }
  void loadNotifications();
}

function openTab() {
  window.showPage?.(PAGE_ID, sidebarLink(PAGE_ID));
}

function bindPage(page) {
  byId("tab-notif-todas")?.addEventListener("click", () => showTab("todas"));
  byId("tab-notif-no-leidas")?.addEventListener("click", () =>
    showTab("no-leidas"),
  );
  byId("notificacion-search")?.addEventListener(
    "input",
    debounce(() => void loadNotifications(), 250),
  );
  byId("notificacion-fecha")?.addEventListener(
    "change",
    () => void loadNotifications(),
  );
  byId("notif-limpiar-fecha")?.addEventListener("click", () => {
    const date = byId("notificacion-fecha");
    if (date) date.value = "";
    void loadNotifications();
  });
  byId("notif-marcar-todas")?.addEventListener("click", async () => {
    try {
      await markAllNotificationsRead();
    } catch (error) {
      notifyError(error, "No se pudieron marcar las notificaciones.");
    }
    await afterChange();
  });
  // RF-124: eliminar sin confirmación; "todas" son las que se ven ahora.
  byId("notif-eliminar-todas")?.addEventListener("click", async () => {
    try {
      const deleted = Number(await deleteNotifications(readFilters())) || 0;
      notifySuccess(
        deleted === 1
          ? "Se eliminó 1 notificación."
          : `Se eliminaron ${deleted} notificaciones.`,
      );
    } catch (error) {
      notifyError(error, "No se pudieron eliminar las notificaciones.");
    }
    await afterChange();
  });

  const list = byId("notificaciones-list");
  list?.addEventListener("click", async (event) => {
    const remove = event.target.closest("[data-notification-delete]");
    if (remove) {
      try {
        await deleteNotification(Number(remove.dataset.notificationDelete));
      } catch (error) {
        notifyError(error, "No se pudo eliminar la notificación.");
      }
      await afterChange();
      return;
    }
    const row = event.target.closest("[data-notification-id]");
    const item = items.find(
      (candidate) => candidate.id === Number(row?.dataset.notificationId),
    );
    if (item) await openNotification(item);
  });
  list?.addEventListener("keydown", (event) => {
    if (event.key !== "Enter" && event.key !== " ") return;
    const row = event.target.closest?.("[data-notification-id]");
    if (!row || event.target !== row) return;
    event.preventDefault();
    row.click();
  });

  window.addEventListener("page:change", (event) => {
    if (event.detail?.pageId === PAGE_ID) void loadNotifications();
  });
  // Llegan nuevas (feed) o cambia el contador: la lista visible se refresca.
  onNotificationsChanged(() => {
    if (isPageActive()) void loadNotifications();
  });
  page.dataset.notificationsBound = "1";
}

export function initNotificacionesView() {
  const page = byId("page-notificaciones");
  if (!page) return Promise.resolve();
  if (page.dataset.notificationsBound !== "1") bindPage(page);
  return startNotificationFeed({ openTab });
}

export const __testHooks = { loadNotifications, openNotification };

if (document.readyState === "loading") {
  document.addEventListener(
    "DOMContentLoaded",
    () => void initNotificacionesView(),
  );
} else {
  void initNotificacionesView();
}
