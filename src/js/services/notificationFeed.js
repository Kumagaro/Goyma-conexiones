// Notificaciones como toast (spec-04 TO-02, D-04): tras cada comando que
// escribe, se piden las generadas desde la última vista y se muestra un toast
// por cada una. Al entrar, un único toast resumen si hay no leídas.
import { showToast } from "../components/toast.js";
import {
  countUnreadNotifications,
  listNotifications,
  listNotificationsSince,
} from "./notificationsService.js";

const TOAST_TYPE = {
  CRÍTICO: "critical",
  ADVERTENCIA: "warning",
  INFORMATIVO: "info",
};

export const PRIORITY_LABEL = {
  CRÍTICO: "Crítico",
  ADVERTENCIA: "Advertencia",
  INFORMATIVO: "Informativo",
};

const BADGE_IDS = [
  "topbar-notif-badge",
  "sidebar-notif-badge",
  "notif-unread-count",
];

let lastSeenId = null;
let lastCount = null;
let queue = Promise.resolve();
let started = false;
const listeners = new Set();
const onCommandSuccess = () => void checkNotifications();

/** Avisa cuando cambian las notificaciones (nuevas, leídas o eliminadas). */
export function onNotificationsChanged(listener) {
  listeners.add(listener);
  return () => listeners.delete(listener);
}

export function updateBadges(count) {
  // La campana se agita solo si llegó algo nuevo, no al abrir la app.
  if (lastCount !== null && count > lastCount) {
    const bell = document.querySelector(".notificaciones");
    bell?.classList.add("is-ringing");
    bell?.addEventListener(
      "animationend",
      () => bell.classList.remove("is-ringing"),
      { once: true },
    );
  }
  lastCount = count;
  for (const id of BADGE_IDS) {
    const badge = document.getElementById(id);
    if (!badge) continue;
    badge.textContent = count > 99 ? "99+" : String(count);
    badge.style.display = count > 0 ? "" : "none";
  }
}

/** Relee el contador de no leídas y avisa a la pestaña. */
export async function refreshUnreadCount() {
  const count = Number(await countUnreadNotifications()) || 0;
  updateBadges(count);
  listeners.forEach((listener) => listener(count));
  return count;
}

async function showNewNotifications() {
  if (lastSeenId === null) return;
  const items = (await listNotificationsSince(lastSeenId)) ?? [];
  for (const item of items) {
    showToast({
      type: TOAST_TYPE[item.priority] ?? "info",
      title: PRIORITY_LABEL[item.priority] ?? "Notificación",
      message: item.message,
    });
    lastSeenId = Math.max(lastSeenId, Number(item.id));
  }
  await refreshUnreadCount();
}

/** Encola una revisión (una a la vez, en orden). */
export function checkNotifications() {
  queue = queue.then(showNewNotifications).catch(() => {});
  return queue;
}

export function startNotificationFeed({ openTab } = {}) {
  if (started) return queue;
  started = true;
  window.addEventListener("command:success", onCommandSuccess);
  queue = (async () => {
    const [latest] = (await listNotifications({})) ?? [];
    lastSeenId = Number(latest?.id ?? 0);
    const unread = await refreshUnreadCount();
    if (unread > 0) {
      showToast({
        type: "info",
        title: "Notificaciones",
        message:
          unread === 1
            ? "Tienes 1 notificación sin leer"
            : `Tienes ${unread} notificaciones sin leer`,
        action: openTab ? { label: "Ver", onClick: openTab } : undefined,
      });
    }
  })().catch(() => {});
  return queue;
}

// Solo para los tests (isolate: false comparte el módulo entre archivos).
export function __resetNotificationFeed() {
  window.removeEventListener("command:success", onCommandSuccess);
  lastSeenId = null;
  queue = Promise.resolve();
  started = false;
  listeners.clear();
}
