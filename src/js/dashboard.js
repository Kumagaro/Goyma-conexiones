// ===== sidemenu.js =====
const logoutLink = document.querySelector(".cerrar-sesion a");
if (logoutLink) {
  logoutLink.addEventListener("click", async (event) => {
    event.preventDefault();
    const { logout } = await import("./services/authService.js");
    await logout();
    window.location.replace("./index.html");
  });
}

import("./session.js").then(
  async ({ loadSession, getSession, onSessionChange }) => {
    await loadSession();
    const greeting = document.querySelector(".bienvenido .text-wrapper");
    const updateGreeting = () => {
      const username = getSession()?.username;
      if (greeting && username)
        greeting.textContent = `Bienvenido de nuevo, ${username}.`;
    };
    updateGreeting();
    onSessionChange(updateGreeting);
  },
);

const toggleButton = document.getElementById("toggle-button");
const sidebar = document.getElementById("sidebar");

function toggleSidebar() {
  sidebar.classList.toggle("close");
  toggleButton.classList.toggle("rotate");

  Array.from(sidebar.getElementsByClassName("show")).forEach((ul) => {
    ul.classList.remove("show");
    ul.previousElementSibling.classList.remove("rotate");
  });
}

// ===== Sidebar deslizable en móvil/tablet =====
function toggleMobileSidebar(forceOpen) {
  const backdrop = document.getElementById("sidebar-backdrop");
  const willOpen =
    typeof forceOpen === "boolean"
      ? forceOpen
      : !sidebar.classList.contains("mobile-open");
  sidebar.classList.toggle("mobile-open", willOpen);
  if (backdrop) backdrop.classList.toggle("show", willOpen);
}

// Cierra el sidebar móvil al elegir una opción del menú
sidebar.addEventListener("click", (e) => {
  if (e.target.closest("a") && window.innerWidth <= 900) {
    toggleMobileSidebar(false);
  }
});

// Evita que el sidebar quede abierto en modo overlay si se agranda la ventana
window.addEventListener("resize", () => {
  if (window.innerWidth > 900) {
    toggleMobileSidebar(false);
  }
});

function toggleSubMenu(button) {
  button.nextElementSibling.classList.toggle("show");
  button.classList.toggle("rotates");

  if (sidebar.classList.contains("close")) {
    sidebar.classList.toggle("close");
    toggleButton.classList.toggle("rotate");
  }
}

// =====================================================
// ===== Navegación SPA del menú lateral unificado ====
// =====================================================
const knownPages = [
  "dashboard",
  "productos",
  "servicios",
  "categorias",
  "clientes",
  "proveedores",
  "unidades",
  "usuarios",
  "configuracion",
  "facturas-general",
  "facturas-entrada",
  "facturas-venta",
  "facturas-insumos",
  "facturas-garantia",
  "facturas-perdidas",
  "facturas-servicios",
  "notificaciones",
  "borradores",
  "actividad",
  "perfil",
];

function showPage(pageId, el, label) {
  const targetId = knownPages.includes(pageId) ? pageId : "placeholder";

  document
    .querySelectorAll(".page")
    .forEach((p) => p.classList.remove("active"));
  const target = document.getElementById("page-" + targetId);
  if (target) target.classList.add("active");

  if (targetId === "placeholder") {
    document.getElementById("placeholder-title").textContent =
      label || "Próximamente";
  }

  // Resalta el ñtem activo del menú lateral (enlaces y botones de submenú)
  document
    .querySelectorAll("#sidebar a, #sidebar .dropsown-button")
    .forEach((a) => a.classList.remove("active"));
  if (el) el.classList.add("active");

  if (targetId === "productos") {
    void window.showProductoTab?.(
      "list",
      document.getElementById("tab-producto-list"),
    );
  }
  // bubbles: las vistas escuchan en window, que solo lo recibe si sube.
  document.dispatchEvent(
    new CustomEvent("page:change", {
      bubbles: true,
      detail: { pageId: targetId },
    }),
  );
  document
    .querySelector("main.panel")
    .scrollTo?.({ top: 0, behavior: "smooth" });
  window.scrollTo({ top: 0, behavior: "smooth" });
  return false;
}

// ===== Acciones declarativas del HTML =====
// spec-04 SG-01: la CSP prohíbe los on* inline; el HTML declara la acción con
// data-* y estos listeners delegados la ejecutan.
const ACCIONES_CLICK = {
  toggleSidebar: () => toggleSidebar(),
  toggleSubMenu: (el) => toggleSubMenu(el),
  toggleMobileSidebar: () => toggleMobileSidebar(),
  closeMobileSidebar: () => toggleMobileSidebar(false),
  togglePassword: (el) => togglePassword(el.dataset.target, el),
  showConfigTab: (el) => showConfigTab(el.dataset.tab, el),
  irAPagina: (el) => irAPagina(el.dataset.tabla, Number(el.dataset.pagina)),
};

document.addEventListener("click", (event) => {
  const enlace = event.target.closest?.(
    "#sidebar [data-page], [data-goto-page]",
  );
  if (enlace) {
    event.preventDefault();
    const pagina = enlace.dataset.page ?? enlace.dataset.gotoPage;
    const menu = enlace.dataset.page
      ? enlace
      : document.querySelector(`#sidebar [data-page="${pagina}"]`);
    showPage(pagina, menu, enlace.dataset.pageLabel);
    return;
  }
  const boton = event.target.closest?.("[data-action]");
  const accion = boton && ACCIONES_CLICK[boton.dataset.action];
  if (accion) accion(boton);
});

// ===== Sub-pestañas: Configuración =====
function showConfigTab(tab, btn) {
  document
    .querySelectorAll("#page-configuracion .config-subview")
    .forEach((v) => v.classList.remove("active"));
  document.getElementById("config-" + tab).classList.add("active");
  document
    .querySelectorAll("#page-configuracion .config-tabs button")
    .forEach((b) => b.classList.remove("active"));
  btn.classList.add("active");
}

function togglePassword(inputId, btn) {
  const input = document.getElementById(inputId);
  input.type = input.type === "password" ? "text" : "password";
}

// =====================================================

// ===== Ordenamiento ascendente/descendente por Fecha (todas las tablas con columna Fecha) =====
const ordenFechaState = {
  historialProducto: "desc",
  historialProveedor: "desc",
  historialTasas: "desc",
  facturasEntrada: "desc",
  facturasVenta: "desc",
  facturasServicios: "desc",
  facturasInsumos: "desc",
  facturasGarantia: "desc",
  facturasPerdidas: "desc",
  facturasGeneral: "desc",
};

const ordenFechaThIds = {
  historialProducto: "th-fecha-historial-producto",
  historialProveedor: "th-fecha-historial-proveedor",
  historialTasas: "th-fecha-historial-tasas",
  facturasEntrada: "th-fecha-facturas-entrada",
  facturasVenta: "th-fecha-facturas-venta",
  facturasServicios: "th-fecha-facturas-servicios",
  facturasInsumos: "th-fecha-facturas-insumos",
  facturasGarantia: "th-fecha-facturas-garantia",
  facturasPerdidas: "th-fecha-facturas-perdidas",
  facturasGeneral: "th-fecha-facturas-general",
};

// Convierte fechas en formato ISO (YYYY-MM-DD) o en formato 'DD/MM/YYYY, h:mm AM/PM'
// (usado en el Historial de Tasas) a un valor numérico comparable.
function fechaAValorOrdenable(valor) {
  if (!valor) return 0;
  if (/^\d{4}-\d{2}-\d{2}/.test(valor)) {
    const t = new Date(valor).getTime();
    return isNaN(t) ? 0 : t;
  }
  let m = valor.match(
    /^(\d{1,2})\/(\d{1,2})\/(\d{4}),?\s*(\d{1,2}):(\d{2})\s*(AM|PM)?/i,
  );
  if (m) {
    let [, d, mo, y, h, mi, ap] = m;
    h = parseInt(h, 10);
    if (ap) {
      ap = ap.toUpperCase();
      if (ap === "PM" && h !== 12) h += 12;
      if (ap === "AM" && h === 12) h = 0;
    }
    return new Date(
      parseInt(y, 10),
      parseInt(mo, 10) - 1,
      parseInt(d, 10),
      h,
      parseInt(mi, 10),
    ).getTime();
  }
  m = valor.match(/^(\d{1,2})\/(\d{1,2})\/(\d{4})$/);
  if (m) {
    const [, d, mo, y] = m;
    return new Date(
      parseInt(y, 10),
      parseInt(mo, 10) - 1,
      parseInt(d, 10),
    ).getTime();
  }
  return 0;
}

function ordenarPorFecha(lista, clave, campoFecha) {
  const dir = ordenFechaState[clave] === "asc" ? 1 : -1;
  return [...lista].sort(
    (a, b) =>
      dir *
      (fechaAValorOrdenable(a[campoFecha]) -
        fechaAValorOrdenable(b[campoFecha])),
  );
}

function actualizarIconoOrdenFecha(clave) {
  const th = document.getElementById(ordenFechaThIds[clave]);
  if (!th) return;
  th.querySelectorAll(".sort-arrow").forEach((el) => el.remove());
  const arrow = document.createElement("span");
  arrow.className = "sort-arrow";
  arrow.textContent = ordenFechaState[clave] === "asc" ? "▲" : "▼";
  th.appendChild(arrow);
}

function inicializarIconosOrdenFecha() {
  Object.keys(ordenFechaThIds).forEach(actualizarIconoOrdenFecha);
}

function toggleOrdenFecha(clave, renderFn) {
  ordenFechaState[clave] = ordenFechaState[clave] === "asc" ? "desc" : "asc";
  actualizarIconoOrdenFecha(clave);
  if (typeof renderFn === "function") renderFn();
}

// Mock de facturas de entrada eliminado en la tarea 3.6.3: la sección
// Facturas de Entrada se sirve desde SQLite vía facturasView.js.

// =====================================================
// ======================= Usuarios ====================
// =====================================================

// =====================================================
// ===== Utilidad de fecha compartida (todas las vistas)
// =====================================================

function formatearFecha(iso) {
  if (!iso) return ":";
  const [y, m, d] = iso.split("-");
  return `${d}/${m}/${y}`;
}

// spec-04 IM-01/IM-02: imprimir y exportar facturas viven en
// views/printPreview.js y en el backend (invoice_print.rs); jsPDF se retiró.

// Todas las facturas (entrada, venta, servicio, garantía, insumos, pérdidas
// y lista general) las sirve facturasView.js desde SQLite; el mock se
// eliminó en las tareas 3.6.3 y 4.5.9.

// =====================================================
// ========================= Perfil =====================
// =====================================================
// Estado inicial
inicializarIconosOrdenFecha();

/* =========================================================================
           PAGINACIÓN GENÉRICA DE TABLAS Y LISTADOS
           -------------------------------------------------------------------------
           20 elementos por página (fijo). No hace falta tocar las funciones
           render*(): este módulo observa cada contenedor y, cada vez que se
           repinta, oculta las filas fuera de la página actual e inserta un
           paginador («Mostrando 1–20 de 87» + números) justo debajo de la tabla.
           ========================================================================= */
const PAGINACION_OPCIONES = [10, 20, 50, 100];
const PAGINACION_TAM_DEFECTO = 20;
let PAGINACION_TAM = PAGINACION_TAM_DEFECTO;
const paginacionEstado = {};

/* Contenedores paginables.
           - id: id del <tbody> o del <div> contenedor de filas.
           - filas: selector de fila cuando NO es un <tbody> (líneas de factura).
           - seguirUltima: al crecer el número de filas salta a la última página
             (útil en los editores de líneas, para ver la fila recién agregada). */
const PAGINABLES = [
  /* Dashboard: lento movimiento lista todos los productos activos (DB-06) */
  { id: "dash-lento" },
  /* Borradores y registro de actividad */
  { id: "borradores-body" },
  { id: "actividad-body" },
  /* Listados principales */
  { id: "productos-body" },
  { id: "productos-condiciones-body" },
  { id: "categorias-body" },
  { id: "servicios-body" },
  { id: "clientes-body" },
  { id: "proveedores-body" },
  { id: "usuarios-body" },
  { id: "unidades-body" },
  { id: "conversiones-body" },
  { id: "facturas-entrada-body" },
  { id: "facturas-venta-body" },
  { id: "facturas-servicios-body" },
  { id: "facturas-insumos-body" },
  { id: "facturas-garantia-body" },
  { id: "facturas-perdidas-body" },
  { id: "facturas-general-body" },
  /* Históricos */
  { id: "historial-producto-body" },
  { id: "historial-proveedor-body" },
  { id: "historial-tasas-body" },
  /* Líneas de factura: editores */
  { id: "factura-lineas-body", filas: ".li-row.li-body", seguirUltima: true },
  {
    id: "factura-venta-lineas-body",
    filas: ".li-row.li-body",
    seguirUltima: true,
  },
  {
    id: "factura-servicio-lineas-body",
    filas: ".li-row.li-body",
    seguirUltima: true,
  },
  {
    id: "factura-insumo-lineas-body",
    filas: ".li-row.li-body",
    seguirUltima: true,
  },
  {
    id: "factura-garantia-lineas-body",
    filas: ".li-row.li-body",
    seguirUltima: true,
  },
  /* Líneas de factura: vistas de solo lectura */
  { id: "factura-ver-productos", filas: ".li-row.li-body" },
  { id: "factura-venta-ver-productos", filas: ".li-row.li-body" },
  { id: "factura-servicio-ver-productos", filas: ".li-row.li-body" },
  { id: "factura-insumo-ver-productos", filas: ".li-row.li-body" },
  { id: "factura-garantia-ver-productos", filas: ".li-row.li-body" },
];

/* ----- Tamaño de página (Configuración > General) -----
   spec-04 CF-04: el tamaño es una preferencia del usuario guardada en la base;
   configuracionView.js lo lee y lo aplica con esta función. */
function establecerTamPagina(valor) {
  const nuevo = parseInt(valor, 10);
  if (!PAGINACION_OPCIONES.includes(nuevo) || nuevo === PAGINACION_TAM) return;
  PAGINACION_TAM = nuevo;
  /* Todas las tablas vuelven a la página 1 con el nuevo tamaño */
  PAGINABLES.forEach((cfg) => {
    estadoPaginacion(cfg.id).pagina = 1;
    aplicarPaginacion(cfg);
  });
}

function configPaginacion(id) {
  return PAGINABLES.find((c) => c.id === id);
}

function estadoPaginacion(id) {
  if (!paginacionEstado[id])
    paginacionEstado[id] = { pagina: 1, ultimoTotal: 0 };
  return paginacionEstado[id];
}

/* Filas reales del contenedor (se excluye la fila de "sin resultados"). */
function filasPaginacion(cfg) {
  const cont = document.getElementById(cfg.id);
  if (!cont) return [];
  if (cfg.filas)
    return Array.from(cont.querySelectorAll(":scope > " + cfg.filas));
  return Array.from(cont.children).filter(
    (el) => el.tagName === "TR" && !el.querySelector(".empty-row"),
  );
}

/* Números a mostrar: 1 … actual-1, actual, actual+1 … última */
function numerosPaginacion(pagina, totalPaginas) {
  const visibles = new Set([1, totalPaginas, pagina - 1, pagina, pagina + 1]);
  if (pagina <= 3) [2, 3, 4].forEach((n) => visibles.add(n));
  if (pagina >= totalPaginas - 2)
    [totalPaginas - 1, totalPaginas - 2, totalPaginas - 3].forEach((n) =>
      visibles.add(n),
    );
  const nums = Array.from(visibles)
    .filter((n) => n >= 1 && n <= totalPaginas)
    .sort((a, b) => a - b);
  const salida = [];
  nums.forEach((n, i) => {
    if (i > 0 && n - nums[i - 1] > 1) salida.push("…");
    salida.push(n);
  });
  return salida;
}

function obtenerPaginador(cfg) {
  const cont = document.getElementById(cfg.id);
  if (!cont) return null;
  let pager = document.getElementById("pager-" + cfg.id);
  if (!pager) {
    pager = document.createElement("div");
    pager.id = "pager-" + cfg.id;
    pager.className = "pager";
    const ancla = cont.closest("table") || cont;
    ancla.parentNode.insertBefore(pager, ancla.nextSibling);
  }
  return pager;
}

function pintarPaginador(cfg, total, pagina, totalPaginas, inicio) {
  const pager = obtenerPaginador(cfg);
  if (!pager) return;

  /* Con 20 elementos o menos no se muestra el paginador */
  if (total <= PAGINACION_TAM) {
    pager.replaceChildren();
    pager.hidden = true;
    return;
  }
  pager.hidden = false;

  const desde = inicio + 1;
  const hasta = Math.min(inicio + PAGINACION_TAM, total);
  // spec-04 SG-01: el paginador se construye con DOM, sin innerHTML.
  const boton = (clase, destino, texto) => {
    const b = document.createElement("button");
    b.type = "button";
    b.className = clase;
    b.dataset.action = "irAPagina";
    b.dataset.tabla = cfg.id;
    b.dataset.pagina = String(destino);
    if (texto) b.append(texto);
    return b;
  };
  const chevron = (clase) => {
    const svgNs = "http://www.w3.org/2000/svg";
    const svg = document.createElementNS(svgNs, "svg");
    svg.setAttribute("class", clase);
    svg.setAttribute("style", "width:16px;height:16px");
    svg.setAttribute("aria-hidden", "true");
    const use = document.createElementNS(svgNs, "use");
    use.setAttribute("href", "#icon-chevron-left");
    svg.append(use);
    return svg;
  };

  const info = document.createElement("span");
  info.className = "pager-info";
  info.textContent = `Mostrando ${desde}–${hasta} de ${total}`;

  const anterior = boton("pager-btn", pagina - 1);
  anterior.disabled = pagina === 1;
  anterior.setAttribute("aria-label", "Página anterior");
  anterior.append(chevron("icon"), " Anterior");

  const siguiente = boton("pager-btn", pagina + 1);
  siguiente.disabled = pagina === totalPaginas;
  siguiente.setAttribute("aria-label", "Página siguiente");
  siguiente.append("Siguiente ", chevron("icon pager-chevron-next"));

  const numeros = numerosPaginacion(pagina, totalPaginas).map((n) => {
    if (n === "…") {
      const hueco = document.createElement("span");
      hueco.className = "pager-gap";
      hueco.textContent = "…";
      return hueco;
    }
    const b = boton(
      n === pagina ? "pager-num is-active" : "pager-num",
      n,
      String(n),
    );
    if (n === pagina) b.setAttribute("aria-current", "page");
    return b;
  });

  const controles = document.createElement("div");
  controles.className = "pager-controls";
  controles.append(anterior, ...numeros, siguiente);
  pager.replaceChildren(info, controles);
}

function aplicarPaginacion(cfg) {
  const cont = document.getElementById(cfg.id);
  if (!cont) return;
  const filas = filasPaginacion(cfg);
  const estado = estadoPaginacion(cfg.id);
  const totalPaginas = Math.max(1, Math.ceil(filas.length / PAGINACION_TAM));
  estado.pagina = Math.min(Math.max(1, estado.pagina), totalPaginas);
  const inicio = (estado.pagina - 1) * PAGINACION_TAM;
  const fin = inicio + PAGINACION_TAM;

  filas.forEach((fila, i) => {
    const visible = i >= inicio && i < fin;
    /* Se limpia el display en línea para no romper el modo tarjeta en móvil */
    fila.style.display = visible ? "" : "none";
  });

  pintarPaginador(cfg, filas.length, estado.pagina, totalPaginas, inicio);
}

function irAPagina(id, pagina) {
  const cfg = configPaginacion(id);
  if (!cfg) return;
  const estado = estadoPaginacion(id);
  estado.pagina = pagina;
  aplicarPaginacion(cfg);

  /* Si el inicio de la tabla quedó por encima de la pantalla, se sube a él */
  const cont = document.getElementById(id);
  const ancla = cont ? cont.closest("table") || cont : null;
  if (ancla && ancla.getBoundingClientRect().top < 0) {
    ancla.scrollIntoView({ behavior: "smooth", block: "start" });
  }
}

/* Cada vez que una función render*() repinta el contenedor:
           - los listados vuelven a la página 1 (búsquedas, filtros, orden);
           - los editores de líneas saltan a la última página si se agregó una fila. */
function alRepintarPaginable(cfg) {
  const estado = estadoPaginacion(cfg.id);
  const total = filasPaginacion(cfg).length;
  if (cfg.seguirUltima && total > estado.ultimoTotal) {
    estado.pagina = Math.max(1, Math.ceil(total / PAGINACION_TAM));
  } else if (!cfg.seguirUltima) {
    estado.pagina = 1;
  }
  estado.ultimoTotal = total;
  aplicarPaginacion(cfg);
}

function iniciarPaginacion() {
  PAGINABLES.forEach((cfg) => {
    const cont = document.getElementById(cfg.id);
    if (!cont) return;
    const observador = new MutationObserver(() => alRepintarPaginable(cfg));
    observador.observe(cont, { childList: true });
    estadoPaginacion(cfg.id).ultimoTotal = filasPaginacion(cfg).length;
    aplicarPaginacion(cfg);
  });
}

if (document.readyState === "loading") {
  document.addEventListener("DOMContentLoaded", iniciarPaginacion);
} else {
  iniciarPaginacion();
}

// =====================================================
// ===== Pestañas unificadas: píldora deslizante =======
// =====================================================
// Sirve a todos los contenedores .view-tabs y .config-tabs sin tocar las
// funciones showXxxTab existentes: observa el cambio de la clase .active y
// mueve la píldora blanca (.tabs-thumb) hasta la pestaña activa.
(function () {
  "use strict";

  const CONTENEDORES = ".view-tabs, .config-tabs";
  const reducirMovimiento = window.matchMedia(
    "(prefers-reduced-motion: reduce)",
  );

  const pestanasDe = (c) =>
    Array.from(c.children).filter((el) => el.classList.contains("tab"));

  function colocar(c, centrar) {
    const activa = pestanasDe(c).find((t) => t.classList.contains("active"));

    // Sin pestaña activa o pantalla oculta (display:none): se esconde la píldora
    // y se recoloca sin animar cuando la pantalla vuelve a mostrarse.
    if (!activa || activa.offsetWidth === 0) {
      c.classList.remove("tabs-placed", "tabs-animate");
      return;
    }

    c.style.setProperty("--thumb-x", activa.offsetLeft + "px");
    c.style.setProperty("--thumb-w", activa.offsetWidth + "px");

    if (!c.classList.contains("tabs-placed")) {
      c.classList.add("tabs-placed");
      requestAnimationFrame(() =>
        requestAnimationFrame(() => {
          if (c.classList.contains("tabs-placed"))
            c.classList.add("tabs-animate");
        }),
      );
    }

    // En móvil, si las pestañas no caben, la activa se centra sola
    if (centrar && c.scrollWidth > c.clientWidth) {
      const izquierda =
        activa.offsetLeft - (c.clientWidth - activa.offsetWidth) / 2;
      c.scrollTo({
        left: Math.max(0, izquierda),
        behavior: reducirMovimiento.matches ? "auto" : "smooth",
      });
    }
  }

  // Accesibilidad (los grupos que ya traen role, como el filtro de período, se respetan)
  function sincronizar(c) {
    if (!c._esTablist) return;
    const tabs = pestanasDe(c);
    const hayActiva = tabs.some((t) => t.classList.contains("active"));
    tabs.forEach((t) => {
      const activa = t.classList.contains("active");
      t.setAttribute("aria-selected", String(activa));
      t.tabIndex = activa || !hayActiva ? 0 : -1;
    });
  }

  function teclado(c, e) {
    const actual = e.target.closest(".tab");
    if (!actual || actual.parentElement !== c) return;
    const lista = pestanasDe(c).filter(
      (t) => !t.classList.contains("disabled"),
    );
    let i = lista.indexOf(actual);
    if (i < 0) return;
    if (e.key === "ArrowRight") i = (i + 1) % lista.length;
    else if (e.key === "ArrowLeft") i = (i - 1 + lista.length) % lista.length;
    else if (e.key === "Home") i = 0;
    else if (e.key === "End") i = lista.length - 1;
    else return;
    e.preventDefault();
    lista[i].focus();
    if (c._esTablist) lista[i].click(); // en pestañas de sección, la flecha también las abre
  }

  function iniciar(c) {
    if (c._tabsListo) return;
    c._tabsListo = true;

    const pildora = document.createElement("span");
    pildora.className = "tabs-thumb";
    pildora.setAttribute("aria-hidden", "true");
    c.insertBefore(pildora, c.firstChild);

    if (!c.hasAttribute("role")) {
      c.setAttribute("role", "tablist");
      c._esTablist = true;
      pestanasDe(c).forEach((t) => t.setAttribute("role", "tab"));
    }
    sincronizar(c);

    // Cambia la pestaña activa (lo hacen las funciones showXxxTab existentes)
    new MutationObserver((muts) => {
      if (!muts.some((m) => m.target !== c)) return;
      sincronizar(c);
      colocar(c, true);
    }).observe(c, {
      attributes: true,
      attributeFilter: ["class"],
      subtree: true,
    });

    // Cambia el tamaño: pantalla que se muestra, ventana, contador de "No leídas"...
    if ("ResizeObserver" in window) {
      const ro = new ResizeObserver(() => colocar(c, false));
      ro.observe(c);
      pestanasDe(c).forEach((t) => ro.observe(t));
    }

    c.addEventListener("keydown", (e) => teclado(c, e));
    colocar(c, false);
  }

  function iniciarTodas() {
    document.querySelectorAll(CONTENEDORES).forEach(iniciar);
  }

  if (document.readyState === "loading") {
    document.addEventListener("DOMContentLoaded", iniciarTodas);
  } else {
    iniciarTodas();
  }
})();
