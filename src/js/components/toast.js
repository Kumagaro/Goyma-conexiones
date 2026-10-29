// Toasts de respuesta y de notificación (spec-04 TO-01, TO-02): recuadros
// emergentes arriba a la derecha que sustituyen a los mensajes de texto plano.
// El contenido se pinta siempre con textContent (A-01 de spec-02).

const MAX_VISIBLE = 4;
const LEAVE_MS = 250;

const TYPES = {
  success: { title: "Listo", role: "status", duration: 4000, icon: "check" },
  info: { title: "Información", role: "status", duration: 4000, icon: "info" },
  warning: {
    title: "Atención",
    role: "status",
    duration: 7000,
    icon: "alert",
  },
  // Error y crítico se quedan hasta que el usuario los cierra.
  error: { title: "Error", role: "alert", duration: null, icon: "alert" },
  critical: { title: "Crítico", role: "alert", duration: null, icon: "alert" },
};

// Trazos propios: index.html y recovery.html no cargan el sprite de iconos.
const ICON_PATHS = {
  check: ["M20 6 9 17l-5-5"],
  info: ["M12 16v-4", "M12 8h.01", "M12 22a10 10 0 1 0 0-20 10 10 0 0 0 0 20z"],
  alert: [
    "M12 9v4",
    "M12 17h.01",
    "M10.3 3.9 1.8 18a2 2 0 0 0 1.7 3h17a2 2 0 0 0 1.7-3L13.7 3.9a2 2 0 0 0-3.4 0z",
  ],
  close: ["M18 6 6 18", "M6 6l12 12"],
};

const SVG_NS = "http://www.w3.org/2000/svg";

function icon(name, className) {
  const svg = document.createElementNS(SVG_NS, "svg");
  svg.setAttribute("viewBox", "0 0 24 24");
  svg.setAttribute("class", className);
  svg.setAttribute("aria-hidden", "true");
  for (const d of ICON_PATHS[name]) {
    const path = document.createElementNS(SVG_NS, "path");
    path.setAttribute("d", d);
    svg.append(path);
  }
  return svg;
}

const active = [];

function getRegion() {
  let region = document.getElementById("toast-region");
  if (region) return region;
  region = document.createElement("section");
  region.id = "toast-region";
  region.className = "toast-region";
  region.setAttribute("aria-label", "Notificaciones");
  const reduce = window.matchMedia?.("(prefers-reduced-motion: reduce)");
  if (reduce?.matches) region.classList.add("reduce-motion");
  document.body.append(region);
  return region;
}

function dismiss(entry) {
  if (entry.leaving) return;
  entry.leaving = true;
  clearTimeout(entry.timer);
  entry.element.classList.add("is-leaving");
  const index = active.indexOf(entry);
  if (index >= 0) active.splice(index, 1);
  setTimeout(() => entry.element.remove(), LEAVE_MS);
}

function startTimer(entry) {
  if (entry.remaining == null || entry.leaving) return;
  entry.startedAt = Date.now();
  entry.timer = setTimeout(() => dismiss(entry), entry.remaining);
}

function pauseTimer(entry) {
  if (entry.remaining == null || entry.timer == null) return;
  clearTimeout(entry.timer);
  entry.timer = null;
  entry.remaining = Math.max(
    0,
    entry.remaining - (Date.now() - entry.startedAt),
  );
}

function makeRoom() {
  while (active.length >= MAX_VISIBLE) {
    // Sale primero el más antiguo de los que se cierran solos.
    const oldest = active.find((entry) => entry.remaining != null) ?? active[0];
    dismiss(oldest);
  }
}

/**
 * Muestra un toast. `type`: success | info | warning | error | critical.
 * `action` opcional: { label, onClick }. Devuelve el elemento creado.
 */
export function showToast({ type = "info", title, message = "", action } = {}) {
  const config = TYPES[type] ?? TYPES.info;
  const region = getRegion();
  makeRoom();

  const element = document.createElement("div");
  element.className = `toast toast-${TYPES[type] ? type : "info"}`;
  element.setAttribute("role", config.role);

  const body = document.createElement("div");
  body.className = "toast-body";
  const heading = document.createElement("p");
  heading.className = "toast-title";
  heading.textContent = title ?? config.title;
  const text = document.createElement("p");
  text.className = "toast-message";
  text.textContent = String(message ?? "");
  body.append(heading, text);

  const entry = { element, remaining: config.duration, timer: null };

  if (action?.label) {
    const actionButton = document.createElement("button");
    actionButton.type = "button";
    actionButton.className = "toast-action";
    actionButton.textContent = action.label;
    actionButton.addEventListener("click", () => {
      action.onClick?.();
      dismiss(entry);
    });
    body.append(actionButton);
  }

  const close = document.createElement("button");
  close.type = "button";
  close.className = "toast-close";
  close.setAttribute("aria-label", "Cerrar notificación");
  close.append(icon("close", "toast-close-icon"));
  close.addEventListener("click", () => dismiss(entry));

  element.append(icon(config.icon, "toast-icon"), body, close);
  element.addEventListener("mouseenter", () => pauseTimer(entry));
  element.addEventListener("mouseleave", () => startTimer(entry));
  element.addEventListener("focusin", () => pauseTimer(entry));
  element.addEventListener("focusout", (event) => {
    if (!element.contains(event.relatedTarget)) startTimer(entry);
  });

  region.append(element);
  active.push(entry);
  startTimer(entry);
  return element;
}

/** Cierra todos los toasts (al cerrar sesión y en los tests). */
export function dismissAllToasts() {
  for (const entry of [...active]) dismiss(entry);
}
