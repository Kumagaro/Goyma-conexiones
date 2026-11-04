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

  if (targetId === "configuracion") {
    renderMonedas();
    renderHistorialTasas();
    renderRateCards();
  }
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
  cambiarPeriodoDashboard: (el) =>
    window.cambiarPeriodoDashboard?.(el.dataset.periodo),
  alternarSerieIngresos: (el) =>
    window.alternarSerieIngresos?.(Number(el.dataset.serie), el),
  irAPagina: (el) => irAPagina(el.dataset.tabla, Number(el.dataset.pagina)),
  ordenarHistorialTasas: () =>
    toggleOrdenFecha("historialTasas", renderHistorialTasas),
  // shortcut: botones de demostración del mockup; la fase 3 (CF) los sustituye.
  avisoDemo: (el) =>
    import("./utils/feedback.js").then(({ notifyInfo }) =>
      notifyInfo(el.dataset.mensaje),
    ),
};
const ACCIONES_CAMPO = {
  cambiarTamPagina: (el) => cambiarTamPagina(el.value),
  activarMoneda: (el) => {
    const moneda = monedasData[Number(el.dataset.indice)];
    if (moneda) moneda.activo = el.checked;
  },
  actualizarEquivalencia: (el) => actualizarEquivalencia(el),
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
for (const [tipo, atributo] of [
  ["input", "inputAction"],
  ["change", "changeAction"],
]) {
  document.addEventListener(tipo, (event) => {
    const accion = ACCIONES_CAMPO[event.target?.dataset?.[atributo]];
    if (accion) accion(event.target);
  });
}

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

// DATOS DE PRUEBA - PLACEHOLDER
// Estos datos serán reemplazados por llamadas al backend cuando se implemente
// el módulo de tasas de cambio.
const monedasData = [
  { codigo: "USD", activo: true },
  { codigo: "EUR", activo: true },
  { codigo: "MLC", activo: true },
  { codigo: "CAD", activo: false },
  { codigo: "MXN", activo: false },
  { codigo: "ZELLE", activo: false },
  { codigo: "CLA", activo: false },
  { codigo: "CUP", activo: true },
];

// DATOS DE PRUEBA - PLACEHOLDER
// Estos datos serán reemplazados por llamadas al backend cuando se implemente
// el módulo de historial de tasas.
const historialTasasData = [
  {
    fecha: "01/01/2027, 9:30 AM",
    moneda: "EUR",
    anterior: "1 EUR = 1.080 USD",
    nueva: "1 EUR = 1.095 USD",
    referencia: "USD",
    usuario: "@pepemilio",
    rol: "Inversionista",
  },
  {
    fecha: "01/01/2027, 11:49 AM",
    moneda: "CUP",
    anterior: "1 CUP = 0.150 USD",
    nueva: "1 CUP = 0.095 USD",
    referencia: "USD",
    usuario: "@adamelisa",
    rol: "Contadora",
  },
  {
    fecha: "02/01/2027, 8:05 AM",
    moneda: "MLC",
    anterior: "1 MLC = 0.90 USD",
    nueva: "1 MLC = 1.050 USD",
    referencia: "USD",
    usuario: "@adamelisa",
    rol: "Contadora",
  },
];

const rateCardsData = [
  {
    paso: 1,
    codigo: "USD",
    desc: "Moneda de referencia principal",
    ref: "CUP",
  },
  { paso: 2, codigo: "CUP", desc: "Respecto a otras monedas", ref: "USD" },
  { paso: 2, codigo: "EUR", desc: "Respecto a otras monedas", ref: "USD" },
  { paso: 3, codigo: "MLC", desc: "Respecto al USD y CUP", ref: "USD" },
];

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

// ===== Exportación de facturas a PDF (tarea 4.5.8) =====
// Las seis funciones reciben el DTO de `getInvoice` que les pasa
// facturasView.js; ya no buscan en arreglos locales. jsPDF no interpreta
// HTML, así que los textos del usuario no abren un riesgo de XSS.
const PDF_PAYMENT_LABELS = {
  cash: "Efectivo",
  transfer: "Transferencia",
  card: "Tarjeta",
  zelle: "Zelle",
};
const PDF_TYPE_LABELS = {
  E: "Entrada",
  V: "Venta",
  S: "Servicio",
  GG: "Garantía",
  GI: "Insumo",
  P: "Pérdida",
};
const PDF_ENTRY_STATUS = {
  pending: "Por Pagar",
  completed: "Pagada",
  cancelled: "Cancelada",
};
const PDF_SALE_STATUS = {
  pending: "Por Cobrar",
  completed: "Cobrada",
  cancelled: "Cancelada",
};
const PDF_ASSOCIATED_STATUS = {
  pending: "Pendiente",
  completed: "Completada",
  cancelled: "Cancelada",
};

function pdfMoney(cents) {
  return (Number(cents || 0) / 100).toLocaleString("es-ES", {
    minimumFractionDigits: 2,
    maximumFractionDigits: 2,
  });
}

function exportarDetalleFacturaPDF(detail, config) {
  if (!detail || typeof detail !== "object" || !window.jspdf) return false;
  const { jsPDF } = window.jspdf;
  const doc = new jsPDF();
  let y = 20;
  doc.setFontSize(16);
  doc.text(`${config.title} ${detail.number ?? ""}`, 14, y);
  y += 10;

  const header = [];
  if (config.contactLabel) {
    header.push(`${config.contactLabel}: ${detail.contactFullName || "—"}`);
  }
  header.push(
    `${config.dateLabel ?? "Fecha de Emisión"}: ${formatearFecha(detail.date)}`,
  );
  if (config.statusLabels) {
    header.push(
      `Estado: ${config.statusLabels[detail.status] || detail.status || "—"}`,
    );
  }
  if (config.withPayment) {
    header.push(
      `Método de Pago: ${PDF_PAYMENT_LABELS[detail.paymentMethod] || detail.paymentMethod || "—"}`,
    );
  }
  if (config.associatedLabel) {
    const associated = detail.associatedInvoice;
    header.push(
      `${config.associatedLabel}: ${
        associated
          ? `${associated.number} · ${PDF_TYPE_LABELS[associated.invoiceType] ?? associated.invoiceType}`
          : "—"
      }`,
    );
  }
  doc.setFontSize(11);
  for (const row of header) {
    doc.text(row, 14, y);
    y += 7;
  }

  y += 3;
  doc.setFontSize(12);
  doc.text(config.linesTitle ?? "Productos", 14, y);
  y += 7;
  doc.setFontSize(10);
  doc.text("No.", 14, y);
  doc.text("Producto", 26, y);
  doc.text("Cant.", 110, y);
  doc.text("Unidad", 130, y);
  doc.text(config.priceLabel, 155, y);
  doc.text("Subtotal", 175, y);
  y += 5;
  doc.line(14, y, 196, y);
  y += 6;
  (detail.lines || []).forEach((line, index) => {
    doc.text(String(index + 1), 14, y);
    doc.text(String(line.productName || "").slice(0, 35), 26, y);
    doc.text(String(line.quantity ?? ""), 110, y);
    doc.text(String(line.unitAbbreviation || line.unitName || "—"), 130, y);
    doc.text(pdfMoney(line.unitPriceCents), 155, y);
    doc.text(pdfMoney(line.subtotalCents), 175, y);
    y += 7;
  });

  y += 5;
  doc.setFontSize(12);
  doc.text(`${config.totalLabel}: ${pdfMoney(detail.totalCents)} USD`, 140, y);
  y += 10;
  if (detail.notes) {
    doc.setFontSize(10);
    doc.text(
      doc.splitTextToSize(`${config.notesLabel}: ${detail.notes}`, 182),
      14,
      y,
    );
  }
  doc.save(`Factura_${detail.number}.pdf`);
  return true;
}

function exportarFacturaPDF(detail) {
  return exportarDetalleFacturaPDF(detail, {
    title: "Factura de Entrada",
    contactLabel: "Proveedor",
    statusLabels: PDF_ENTRY_STATUS,
    withPayment: true,
    priceLabel: "Costo",
    totalLabel: "Total",
    notesLabel: "Observaciones",
  });
}

function exportarFacturaVentaPDF(detail) {
  return exportarDetalleFacturaPDF(detail, {
    title: "Factura de Venta",
    contactLabel: "Cliente",
    statusLabels: PDF_SALE_STATUS,
    withPayment: true,
    priceLabel: "Precio",
    totalLabel: "Total",
    notesLabel: "Descripción",
  });
}

function exportarFacturaServicioPDF(detail) {
  return exportarDetalleFacturaPDF(detail, {
    title: "Factura de Servicio",
    contactLabel: "Cliente",
    statusLabels: PDF_SALE_STATUS,
    withPayment: true,
    linesTitle: "Productos / Insumos",
    priceLabel: "Precio",
    totalLabel: "Total",
    notesLabel: "Descripción",
  });
}

function exportarFacturaGarantiaPDF(detail) {
  return exportarDetalleFacturaPDF(detail, {
    title: "Factura de Gasto de Garantía",
    contactLabel: "Cliente",
    statusLabels: PDF_ASSOCIATED_STATUS,
    associatedLabel: "Factura Original",
    priceLabel: "Costo",
    totalLabel: "Costo Total",
    notesLabel: "Descripción",
  });
}

function exportarFacturaInsumoPDF(detail) {
  return exportarDetalleFacturaPDF(detail, {
    title: "Factura de Gasto de Insumos",
    contactLabel: "Cliente",
    statusLabels: PDF_ASSOCIATED_STATUS,
    associatedLabel: "Factura de Servicio Asociada",
    linesTitle: "Productos / Insumos",
    priceLabel: "Costo",
    totalLabel: "Costo Total",
    notesLabel: "Descripción",
  });
}

function exportarFacturaPerdidaPDF(detail) {
  return exportarDetalleFacturaPDF(detail, {
    title: "Factura de Pérdida",
    dateLabel: "Fecha",
    priceLabel: "Costo",
    totalLabel: "Costo Total",
    notesLabel: "Motivo de la Pérdida",
  });
}
// ===== Fin de la exportación de facturas a PDF =====

// Todas las facturas (entrada, venta, servicio, garantía, insumos, pérdidas
// y lista general) las sirve facturasView.js desde SQLite; el mock se
// eliminó en las tareas 3.6.3 y 4.5.9.

function renderMonedas() {
  const grid = document.getElementById("currency-grid");
  if (!grid || grid.dataset.rendered) return;
  grid.innerHTML = monedasData
    .map(
      (m, i) => `
                <label class="currency-item">
                    <span class="code"><span class="currency-flag">${m.codigo.slice(0, 3)}</span>${m.codigo}</span>
                    <span class="switch">
                        <input type="checkbox" ${m.activo ? "checked" : ""} data-change-action="activarMoneda" data-indice="${i}">
                        <span class="track"></span>
                    </span>
                </label>
            `,
    )
    .join("");
  grid.dataset.rendered = "1";
}

function renderHistorialTasas() {
  const body = document.getElementById("historial-tasas-body");
  if (!body) return;
  const filas = ordenarPorFecha(historialTasasData, "historialTasas", "fecha");
  body.innerHTML = filas
    .map(
      (h) => `
                <tr>
                    <td>${h.fecha}</td>
                    <td>${h.moneda}</td>
                    <td>${h.anterior}</td>
                    <td>${h.nueva}</td>
                    <td>${h.referencia}</td>
                    <td>${h.usuario}<span class="user-role">${h.rol}</span></td>
                </tr>
            `,
    )
    .join("");
  body.dataset.rendered = "1";
}

function renderRateCards() {
  const wrap = document.getElementById("rate-cards");
  if (!wrap || wrap.dataset.rendered) return;
  wrap.innerHTML = rateCardsData
    .map(
      (r) => `
                <div class="rate-card">
                    <div class="rate-card-head">
                        <span class="rate-step">${r.paso}</span>
                        <div>
                            <div class="code">${r.codigo}</div>
                            <div class="desc">${r.desc}</div>
                        </div>
                    </div>
                    <div class="rate-row">
                        <label>Tasa</label>
                        <input type="text" placeholder="0.000" data-input-action="actualizarEquivalencia">
                        <span class="rate-result">1 ${r.codigo} = xxxxxxx ${r.ref}</span>
                    </div>
                </div>
            `,
    )
    .join("");
  wrap.dataset.rendered = "1";
}

function actualizarEquivalencia(input) {
  const card = input.closest(".rate-card");
  const result = card.querySelector(".rate-result");
  const base = result.textContent.split("=")[0].trim();
  const ref = result.textContent.split(" ").pop();
  const valor = parseFloat(input.value);
  result.textContent = isNaN(valor)
    ? `${base} = xxxxxxx ${ref}`
    : `${base} = ${valor.toFixed(3)} ${ref}`;
}

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
const PAGINACION_CLAVE_STORAGE = "tablasPorPagina";
let PAGINACION_TAM = PAGINACION_TAM_DEFECTO;
const paginacionEstado = {};

/* Contenedores paginables.
           - id: id del <tbody> o del <div> contenedor de filas.
           - filas: selector de fila cuando NO es un <tbody> (líneas de factura).
           - seguirUltima: al crecer el número de filas salta a la última página
             (útil en los editores de líneas, para ver la fila recién agregada). */
const PAGINABLES = [
  /* Listados principales */
  { id: "productos-body" },
  { id: "productos-condiciones-body" },
  { id: "categorias-body" },
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

/* ----- Tamaño de página (Configuración > General) ----- */
function leerTamPaginaGuardado() {
  let guardado = null;
  try {
    guardado = parseInt(localStorage.getItem(PAGINACION_CLAVE_STORAGE), 10);
  } catch (e) {
    guardado = null;
  }
  return PAGINACION_OPCIONES.includes(guardado)
    ? guardado
    : PAGINACION_TAM_DEFECTO;
}

function guardarTamPagina(valor) {
  try {
    localStorage.setItem(PAGINACION_CLAVE_STORAGE, String(valor));
  } catch (e) {
    /* Si el navegador bloquea el almacenamiento, el cambio igual aplica en esta sesión */
  }
}

/* Se llama desde el select de Configuración > General */
function cambiarTamPagina(valor) {
  const nuevo = parseInt(valor, 10);
  if (!PAGINACION_OPCIONES.includes(nuevo)) return;

  PAGINACION_TAM = nuevo;
  guardarTamPagina(nuevo);

  /* Todas las tablas vuelven a la página 1 con el nuevo tamaño */
  PAGINABLES.forEach((cfg) => {
    estadoPaginacion(cfg.id).pagina = 1;
    aplicarPaginacion(cfg);
  });

  // spec-04 TO-01: el resultado sale como toast (script clásico: import dinámico).
  import("./utils/feedback.js").then(({ notifySuccess }) =>
    notifySuccess(
      `Se mostrarán ${nuevo} elementos por página en todas las tablas.`,
    ),
  );
}

function sincronizarSelectPaginacion() {
  const select = document.getElementById("config-elementos-pagina");
  if (select) select.value = String(PAGINACION_TAM);
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
  PAGINACION_TAM = leerTamPaginaGuardado();
  sincronizarSelectPaginacion();
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
// ====== Dashboard: KPIs, período y gráfica ===========
// =====================================================
// Todo va dentro de una función anónima para no chocar con el resto del archivo.
// Solo se exponen dos funciones globales (las usan los data-action del HTML):
//   cambiarPeriodoDashboard(periodo)  y  alternarSerieIngresos(indice, boton)
(function () {
  "use strict";

  const raiz = document.getElementById("page-dashboard");
  if (!raiz) return;

  /* ---------- 1. Datos de ejemplo ----------
             Un objeto por período ('hoy', '7d', 'mes', 'anio'), importes en USD (moneda de
             referencia de Configuración). Cada KPI trae: valor (período actual), anterior
             (período previo, para calcular la variación) y tendencia (serie del mini gráfico).
             Las etiquetas del eje X (horas, días, meses) se calculan más abajo. */
  const DATOS_EJEMPLO = {
    hoy: {
      kpis: {
        ingresos: {
          valor: 3980,
          anterior: 4154,
          tendencia: [
            201, 312, 419, 499, 415, 307, 294, 388, 428, 369, 223, 125,
          ],
        },
        ventas: {
          valor: 19,
          anterior: 20,
          tendencia: [1, 2, 2, 2, 2, 1, 1, 2, 2, 2, 1, 1],
        },
        promedio: {
          valor: 19,
          anterior: 20,
          tendencia: [1.5, 1.7, 2, 2, 1.7, 1.3, 1.3, 1.7, 2, 1.7, 1.3, 1],
        },
        ganancia: {
          valor: 1528,
          anterior: 1577,
          tendencia: [83, 125, 155, 185, 157, 114, 116, 157, 159, 151, 80, 45],
        },
        margen: {
          valor: 38.4,
          anterior: 37.5,
          tendencia: [
            41.1, 40, 37.1, 37, 37.9, 37.1, 39.4, 40.5, 37.2, 40.9, 35.9, 35.6,
          ],
        },
        gastos: {
          valor: 1075,
          anterior: 1050,
          tendencia: [52, 89, 119, 132, 124, 86, 82, 102, 104, 111, 56, 37],
        },
        inventario: {
          valor: 512300,
          anterior: 510259,
          tendencia: [
            514329, 507670, 510540, 506849, 516857, 512419, 515918, 514849,
            512964, 511459, 512138, 512300,
          ],
        },
        agotados: {
          valor: 5,
          anterior: 4,
          tendencia: [4, 4, 5, 5, 4, 3, 4, 4, 6, 6, 4, 5],
        },
      },
      ingresos: {
        ventas: [159, 253, 338, 396, 327, 229, 228, 318, 344, 294, 169, 93],
        servicios: [42, 59, 81, 103, 88, 78, 66, 70, 84, 75, 54, 32],
      },
      gastos: { garantia: 667, insumos: 408 },
      clientesEstrella: [
        { nombre: "Laura Martínez", porcentaje: 35 },
        { nombre: "Enrique López", porcentaje: 29 },
        { nombre: "Lorenzo Espinosa", porcentaje: 16 },
      ],
      productosEstrella: [
        { nombre: "Laptop HP 15", porcentaje: 42 },
        { nombre: 'Monitor 24"', porcentaje: 29 },
        { nombre: "Disco SSD 1TB", porcentaje: 11 },
      ],
      clientesFrecuentes: [
        { nombre: "José Fernández", compras: 1 },
        { nombre: "Tatiana Fonseca", compras: 1 },
        { nombre: "Laura Martínez", compras: 1 },
        { nombre: "Manuel Peña", compras: 1 },
        { nombre: "Ana Rizo", compras: 1 },
      ],
      mejorVendidos: [
        { nombre: "Mouse Inalámbrico", unidades: 2 },
        { nombre: "Cable HDMI 2 m", unidades: 2 },
        { nombre: "Teclado Mecánico", unidades: 2 },
        { nombre: "Cargador USB-C 65 W", unidades: 2 },
        { nombre: "Router WiFi", unidades: 1 },
        { nombre: "Disco SSD 1TB", unidades: 1 },
        { nombre: "Auriculares Bluetooth", unidades: 1 },
        { nombre: "Memoria RAM 8 GB", unidades: 1 },
        { nombre: "Laptop HP 15", unidades: 1 },
        { nombre: 'Monitor 24"', unidades: 1 },
      ],
    },
    "7d": {
      kpis: {
        ingresos: {
          valor: 27639,
          anterior: 25879,
          tendencia: [3956, 3141, 3569, 4606, 4760, 4126, 3481],
        },
        ventas: {
          valor: 126,
          anterior: 120,
          tendencia: [18, 14, 16, 21, 22, 19, 16],
        },
        promedio: {
          valor: 18,
          anterior: 17.1,
          tendencia: [16, 16, 17, 19.7, 20.7, 19, 17.5],
        },
        ganancia: {
          valor: 10945,
          anterior: 10005,
          tendencia: [1651, 1186, 1435, 1945, 1756, 1647, 1301],
        },
        margen: {
          valor: 39.6,
          anterior: 38.4,
          tendencia: [41.7, 37.8, 40.2, 42.2, 36.9, 39.9, 37.4],
        },
        gastos: {
          valor: 7324,
          anterior: 7458,
          tendencia: [1079, 852, 882, 1145, 1210, 1010, 1006],
        },
        inventario: {
          valor: 512300,
          anterior: 506726,
          tendencia: [505074, 502225, 507345, 507845, 515521, 515517, 512300],
        },
        agotados: {
          valor: 5,
          anterior: 6,
          tendencia: [6, 6, 5, 5, 5, 4, 5],
        },
      },
      ingresos: {
        ventas: [3213, 2451, 2643, 3467, 3818, 3277, 2656],
        servicios: [743, 690, 926, 1139, 942, 849, 825],
      },
      gastos: { garantia: 4541, insumos: 2783 },
      clientesEstrella: [
        { nombre: "Laura Martínez", porcentaje: 38 },
        { nombre: "Enrique López", porcentaje: 26 },
        { nombre: "Lorenzo Espinosa", porcentaje: 19 },
      ],
      productosEstrella: [
        { nombre: "Laptop HP 15", porcentaje: 42 },
        { nombre: 'Monitor 24"', porcentaje: 25 },
        { nombre: "Disco SSD 1TB", porcentaje: 12 },
      ],
      clientesFrecuentes: [
        { nombre: "José Fernández", compras: 3 },
        { nombre: "Tatiana Fonseca", compras: 2 },
        { nombre: "Laura Martínez", compras: 2 },
        { nombre: "Manuel Peña", compras: 2 },
        { nombre: "Ana Rizo", compras: 1 },
      ],
      mejorVendidos: [
        { nombre: "Cable HDMI 2 m", unidades: 14 },
        { nombre: "Mouse Inalámbrico", unidades: 13 },
        { nombre: "Teclado Mecánico", unidades: 11 },
        { nombre: "Cargador USB-C 65 W", unidades: 11 },
        { nombre: "Router WiFi", unidades: 8 },
        { nombre: "Disco SSD 1TB", unidades: 7 },
        { nombre: "Auriculares Bluetooth", unidades: 6 },
        { nombre: "Memoria RAM 8 GB", unidades: 5 },
        { nombre: "Laptop HP 15", unidades: 5 },
        { nombre: 'Monitor 24"', unidades: 4 },
      ],
    },
    mes: {
      kpis: {
        ingresos: {
          valor: 118403,
          anterior: 105341,
          tendencia: [
            3540, 4482, 4801, 3442, 3253, 3047, 3195, 3257, 3794, 4244, 3468,
            3560, 3246, 2994, 3540, 4100, 5013, 4676, 3508, 2912, 3197, 4760,
            5499, 5580, 4873, 3422, 3304, 3599, 5142, 4955,
          ],
        },
        ventas: {
          valor: 540,
          anterior: 499,
          tendencia: [
            16, 20, 22, 16, 15, 14, 15, 15, 17, 19, 16, 16, 15, 14, 16, 19, 23,
            21, 16, 13, 15, 22, 25, 25, 22, 16, 15, 16, 23, 23,
          ],
        },
        promedio: {
          valor: 18,
          anterior: 16.6,
          tendencia: [
            18, 19.3, 19.3, 17.7, 15, 14.7, 14.7, 15.7, 17, 17.3, 17, 15.7, 15,
            15, 16.3, 19.3, 21, 20, 16.7, 14.7, 16.7, 20.7, 24, 24, 21, 17.7,
            15.7, 18, 20.7, 23,
          ],
        },
        ganancia: {
          valor: 47598,
          anterior: 41354,
          tendencia: [
            1401, 1786, 1825, 1435, 1366, 1281, 1311, 1273, 1580, 1766, 1317,
            1476, 1343, 1204, 1343, 1768, 2017, 1882, 1415, 1186, 1326, 2042,
            2175, 2204, 2030, 1454, 1232, 1555, 2082, 1894,
          ],
        },
        margen: {
          valor: 40.2,
          anterior: 38.4,
          tendencia: [
            39.6, 39.8, 38, 41.7, 42, 42.1, 41, 39.1, 41.6, 41.6, 38, 41.5,
            41.4, 40.2, 37.9, 43.1, 40.2, 40.2, 40.3, 40.7, 41.5, 42.9, 39.6,
            39.5, 41.7, 42.5, 37.3, 43.2, 40.5, 38.2,
          ],
        },
        gastos: {
          valor: 30785,
          anterior: 29431,
          tendencia: [
            881, 1282, 1272, 964, 922, 874, 911, 800, 952, 1153, 825, 893, 877,
            765, 993, 1025, 1208, 1267, 989, 805, 780, 1242, 1276, 1451, 1159,
            954, 867, 964, 1339, 1423,
          ],
        },
        inventario: {
          valor: 512300,
          anterior: 501763,
          tendencia: [
            499964, 501113, 508211, 508207, 508056, 503643, 508413, 510176,
            510121, 499281, 501141, 506495, 502697, 501697, 506058, 502539,
            508213, 503147, 512405, 509518, 507598, 510512, 505843, 505884,
            511629, 510879, 515967, 515489, 510511, 512300,
          ],
        },
        agotados: {
          valor: 5,
          anterior: 3,
          tendencia: [
            3, 3, 3, 3, 3, 3, 4, 3, 3, 3, 4, 4, 3, 3, 5, 4, 5, 3, 5, 5, 4, 4, 4,
            6, 5, 5, 4, 6, 6, 5,
          ],
        },
      },
      ingresos: {
        ventas: [
          2815, 3330, 3748, 2704, 2532, 2307, 2426, 2620, 2816, 3429, 2816,
          2822, 2633, 2424, 2899, 3261, 4106, 3581, 2758, 2374, 2471, 3880,
          4342, 4539, 3837, 2714, 2579, 2794, 3989, 3725,
        ],
        servicios: [
          725, 1152, 1053, 738, 721, 740, 769, 637, 978, 815, 652, 738, 613,
          570, 641, 839, 907, 1095, 750, 538, 726, 880, 1157, 1041, 1036, 708,
          725, 805, 1153, 1230,
        ],
      },
      gastos: { garantia: 19087, insumos: 11698 },
      clientesEstrella: [
        { nombre: "Laura Martínez", porcentaje: 36 },
        { nombre: "Enrique López", porcentaje: 30 },
        { nombre: "Lorenzo Espinosa", porcentaje: 18 },
      ],
      productosEstrella: [
        { nombre: "Laptop HP 15", porcentaje: 37 },
        { nombre: 'Monitor 24"', porcentaje: 25 },
        { nombre: "Disco SSD 1TB", porcentaje: 12 },
      ],
      clientesFrecuentes: [
        { nombre: "José Fernández", compras: 13 },
        { nombre: "Tatiana Fonseca", compras: 11 },
        { nombre: "Laura Martínez", compras: 10 },
        { nombre: "Manuel Peña", compras: 8 },
        { nombre: "Ana Rizo", compras: 6 },
      ],
      mejorVendidos: [
        { nombre: "Mouse Inalámbrico", unidades: 64 },
        { nombre: "Cable HDMI 2 m", unidades: 57 },
        { nombre: "Cargador USB-C 65 W", unidades: 45 },
        { nombre: "Teclado Mecánico", unidades: 42 },
        { nombre: "Router WiFi", unidades: 35 },
        { nombre: "Disco SSD 1TB", unidades: 29 },
        { nombre: "Auriculares Bluetooth", unidades: 27 },
        { nombre: "Memoria RAM 8 GB", unidades: 21 },
        { nombre: "Laptop HP 15", unidades: 18 },
        { nombre: 'Monitor 24"', unidades: 15 },
      ],
    },
    anio: {
      kpis: {
        ingresos: {
          valor: 1425999,
          anterior: 1201347,
          tendencia: [
            93777, 101303, 97470, 106000, 117342, 123817, 117907, 117177,
            136974, 139560, 132147, 142525,
          ],
        },
        ventas: {
          valor: 6480,
          anterior: 5827,
          tendencia: [
            426, 460, 443, 482, 533, 563, 536, 532, 622, 634, 601, 648,
          ],
        },
        promedio: {
          valor: 17.8,
          anterior: 16,
          tendencia: [
            14.2, 15.3, 14.8, 16.1, 17.8, 18.8, 17.9, 17.7, 20.7, 21.1, 20,
            21.6,
          ],
        },
        ganancia: {
          valor: 586086,
          anterior: 483171,
          tendencia: [
            38982, 41491, 42106, 45168, 49668, 49468, 45298, 47458, 55609,
            55909, 54442, 60845,
          ],
        },
        margen: {
          valor: 41.1,
          anterior: 39.2,
          tendencia: [
            41.6, 41, 43.2, 42.6, 42.3, 40, 38.4, 40.5, 40.6, 40.1, 41.2, 42.7,
          ],
        },
        gastos: {
          valor: 359352,
          anterior: 327876,
          tendencia: [
            21353, 25500, 23121, 28956, 28044, 30195, 26481, 26577, 32207,
            31766, 35156, 39004,
          ],
        },
        inventario: {
          valor: 512300,
          anterior: 481485,
          tendencia: [
            476744, 489749, 492517, 484543, 491332, 490041, 494243, 503974,
            506549, 509689, 504424, 512300,
          ],
        },
        agotados: {
          valor: 5,
          anterior: 5,
          tendencia: [5, 4, 5, 6, 6, 5, 5, 6, 5, 6, 5, 5],
        },
      },
      ingresos: {
        ventas: [
          73243, 82925, 76618, 80841, 89130, 94331, 93603, 86741, 108236,
          110552, 104964, 115182,
        ],
        servicios: [
          20534, 18378, 20852, 25159, 28212, 29486, 24304, 30436, 28738, 29008,
          27183, 27343,
        ],
      },
      gastos: { garantia: 222798, insumos: 136554 },
      clientesEstrella: [
        { nombre: "Laura Martínez", porcentaje: 35 },
        { nombre: "Enrique López", porcentaje: 27 },
        { nombre: "Lorenzo Espinosa", porcentaje: 15 },
      ],
      productosEstrella: [
        { nombre: "Laptop HP 15", porcentaje: 40 },
        { nombre: 'Monitor 24"', porcentaje: 29 },
        { nombre: "Disco SSD 1TB", porcentaje: 13 },
      ],
      clientesFrecuentes: [
        { nombre: "José Fernández", compras: 169 },
        { nombre: "Tatiana Fonseca", compras: 142 },
        { nombre: "Laura Martínez", compras: 105 },
        { nombre: "Manuel Peña", compras: 88 },
        { nombre: "Ana Rizo", compras: 77 },
      ],
      mejorVendidos: [
        { nombre: "Mouse Inalámbrico", unidades: 746 },
        { nombre: "Cable HDMI 2 m", unidades: 634 },
        { nombre: "Teclado Mecánico", unidades: 498 },
        { nombre: "Cargador USB-C 65 W", unidades: 481 },
        { nombre: "Router WiFi", unidades: 381 },
        { nombre: "Disco SSD 1TB", unidades: 378 },
        { nombre: "Auriculares Bluetooth", unidades: 302 },
        { nombre: "Memoria RAM 8 GB", unidades: 288 },
        { nombre: "Laptop HP 15", unidades: 246 },
        { nombre: 'Monitor 24"', unidades: 198 },
      ],
    },
  };

  /* ---------- 2. Conexión con el backend ----------
             Cuando tengas API, reemplaza el cuerpo de estas funciones por un fetch que devuelva
             la misma forma de objeto, por ejemplo:
               return fetch('/api/dashboard?periodo=' + periodo).then(r => r.json()); */
  async function obtenerDatosDashboard(periodo) {
    return DATOS_EJEMPLO[periodo];
  }

  /* ---------- 3. Configuración ---------- */
  const PERIODOS = {
    hoy: { rango: "Hoy, comparado con ayer", por: "hora", vs: "ayer" },
    "7d": {
      rango: "Últimos 7 días, comparado con los 7 días anteriores",
      por: "día",
      vs: "los 7 días anteriores",
    },
    mes: {
      rango: "Últimos 30 días, comparado con los 30 días anteriores",
      por: "día",
      vs: "los 30 días anteriores",
    },
    anio: {
      rango: "Últimos 12 meses, comparado con los 12 meses anteriores",
      por: "mes",
      vs: "los 12 meses anteriores",
    },
  };

  // tipo = cómo se escribe el valor; variacion = cómo se mide el cambio;
  // inversa = true cuando subir es MALO (gastos y productos agotados).
  const KPIS = {
    ingresos: { tipo: "dinero", variacion: "porcentaje", inversa: false },
    ventas: { tipo: "entero", variacion: "porcentaje", inversa: false },
    promedio: {
      tipo: "decimal",
      variacion: "porcentaje",
      inversa: false,
      unidad: "al día",
    },
    ganancia: { tipo: "dinero", variacion: "porcentaje", inversa: false },
    margen: { tipo: "porcentaje", variacion: "puntos", inversa: false },
    gastos: { tipo: "dinero", variacion: "porcentaje", inversa: true },
    inventario: { tipo: "dinero", variacion: "porcentaje", inversa: false },
    agotados: {
      tipo: "entero",
      variacion: "absoluta",
      inversa: true,
      unidad: "productos",
    },
  };

  // Paleta de la gráfica: mismos tonos azul/naranja de las tarjetas KPI (.tone-blue / .tone-orange).
  // "Total" es un dato derivado: magenta vivo + línea discontinua. Combinación validada para
  // deuteranopia y protanopia; además Total se distingue por la forma (discontinua, sin relleno).
  const COLOR_VENTAS = "#2563eb";
  const COLOR_SERVICIOS = "#f48430";
  const COLOR_TOTAL = "#e11d74";

  const MESES = [
    "ene",
    "feb",
    "mar",
    "abr",
    "may",
    "jun",
    "jul",
    "ago",
    "sep",
    "oct",
    "nov",
    "dic",
  ];
  const DIAS = ["dom", "lun", "mar", "mié", "jue", "vie", "sáb"];
  const NBSP = "\u00A0";

  /* ---------- 4. Formato de números (coma decimal, espacio en los miles) ---------- */
  const miles = (n) =>
    String(Math.round(n)).replace(/\B(?=(\d{3})+(?!\d))/g, NBSP);
  const dec = (n, d = 1) => n.toFixed(d).replace(".", ",");
  const limpio = (n, d) => String(Number(n.toFixed(d))).replace(".", ",");
  const esc = (s) =>
    String(s).replace(
      /[&<>"']/g,
      (c) =>
        ({
          "&": "&amp;",
          "<": "&lt;",
          ">": "&gt;",
          '"': "&quot;",
          "'": "&#39;",
        })[c],
    );
  const $ = (sel, ctx = raiz) => ctx.querySelector(sel);

  // 118403 → { num: '118,4', unidad: 'mil' }   |   1425999 → { num: '1,43', unidad: 'M' }
  function partesDinero(n) {
    const a = Math.abs(n);
    if (a >= 1e6) return { num: limpio(n / 1e6, 2), unidad: "M" };
    if (a >= 1e4) return { num: limpio(n / 1e3, 1), unidad: "mil" };
    return { num: miles(n), unidad: "" };
  }
  const dineroCompleto = (n) => `$${NBSP}${miles(n)}`;

  // Igual que el resumen original: "200 <mil>"
  function dineroUnidad(n) {
    const p = partesDinero(n);
    return `${p.num}${p.unidad ? ` <span class="unit">${p.unidad}</span>` : ""}`;
  }

  function valorHtml(def, k) {
    const unidad = (t) => `<span class="kpi-unit">${t}</span>`;
    switch (def.tipo) {
      case "dinero": {
        const p = partesDinero(k.valor);
        return `<span class="kpi-cur">$</span>${p.num}${p.unidad ? unidad(p.unidad) : ""}`;
      }
      case "porcentaje":
        return `${dec(k.valor, 1)}${unidad("%")}`;
      case "decimal":
        return `${Number.isInteger(k.valor) ? k.valor : dec(k.valor, 1)}${def.unidad ? unidad(def.unidad) : ""}`;
      default:
        return `${miles(k.valor)}${def.unidad ? unidad(def.unidad) : ""}`;
    }
  }

  function valorPlano(def, k) {
    if (def.tipo === "dinero") return dineroCompleto(k.valor);
    if (def.tipo === "porcentaje") return `${dec(k.valor, 1)} %`;
    return `${miles(k.valor)}${def.unidad ? " " + def.unidad : ""}`;
  }

  /* ---------- 5. Variación y mini gráfico ---------- */
  const FLECHA = {
    1: '<svg class="kpi-arrow" viewBox="0 0 10 10" aria-hidden="true"><path d="M5 2 9 8H1Z" fill="currentColor"/></svg>',
    "-1": '<svg class="kpi-arrow" viewBox="0 0 10 10" aria-hidden="true"><path d="M5 8 1 2h8Z" fill="currentColor"/></svg>',
    0: '<svg class="kpi-arrow" viewBox="0 0 10 10" aria-hidden="true"><path d="M2 5h6" stroke="currentColor" stroke-width="1.8" stroke-linecap="round"/></svg>',
  };

  function variacion(def, k, comparacion) {
    let diff, texto;
    if (def.variacion === "puntos") {
      diff = k.valor - k.anterior;
      texto = `${dec(Math.abs(diff), 1)} pp`;
    } else if (def.variacion === "absoluta") {
      diff = k.valor - k.anterior;
      texto = `${Math.abs(diff)}`;
    } else {
      diff = k.anterior ? ((k.valor - k.anterior) / k.anterior) * 100 : 0;
      texto = `${dec(Math.abs(diff), 1)} %`;
    }
    const dir = Math.abs(diff) < 0.05 ? 0 : Math.sign(diff);
    const bueno = def.inversa ? dir < 0 : dir > 0;
    const estado = dir === 0 ? "is-flat" : bueno ? "is-good" : "is-bad";
    const signo = dir > 0 ? "+" : dir < 0 ? "−" : "";
    const lectura =
      dir === 0
        ? `Sin cambios frente a ${comparacion}`
        : `${dir > 0 ? "Subió" : "Bajó"} ${texto} frente a ${comparacion}`;
    const visible = dir === 0 ? "Sin cambio" : `${signo}${texto}`;
    return `<span class="kpi-delta ${estado}" title="${esc(lectura)}" aria-label="${esc(lectura)}">${FLECHA[dir]}<span>${visible}</span></span>`;
  }

  // Mini gráfico de tendencia en SVG puro (sin librerías)
  function sparkInterior(crudo) {
    const W = 96,
      H = 36,
      padX = 3,
      padTop = 5,
      padBot = 4;
    // Con muchos puntos (p. ej. 30 días) se promedia una ventana corta: se ve la tendencia, no el ruido diario
    const ventana = crudo.length > 20 ? 5 : crudo.length > 12 ? 3 : 1;
    const valores =
      ventana === 1
        ? crudo
        : crudo.map((_, i) => {
            const trozo = crudo.slice(
              Math.max(0, i - (ventana >> 1)),
              i + (ventana >> 1) + 1,
            );
            return trozo.reduce((a, c) => a + c, 0) / trozo.length;
          });
    const min = Math.min(...valores),
      max = Math.max(...valores);
    const rango = max - min || 1;
    const paso = (W - padX * 2) / (valores.length - 1 || 1);
    const pts = valores.map((v, i) => [
      padX + i * paso,
      max === min
        ? H / 2
        : padTop + (1 - (v - min) / rango) * (H - padTop - padBot),
    ]);
    const linea = pts
      .map((p, i) => `${i ? "L" : "M"}${p[0].toFixed(1)} ${p[1].toFixed(1)}`)
      .join(" ");
    const area = `${linea} L${pts[pts.length - 1][0].toFixed(1)} ${H} L${pts[0][0].toFixed(1)} ${H} Z`;
    const ultimo = pts[pts.length - 1];
    return `<path class="kpi-spark-area" d="${area}"/><path class="kpi-spark-line" d="${linea}"/><circle class="kpi-spark-dot" cx="${ultimo[0].toFixed(1)}" cy="${ultimo[1].toFixed(1)}" r="2.6"/>`;
  }

  /* ---------- 6. Render de cada bloque (respeta el markup original) ---------- */
  function renderKpis(datos, periodo) {
    Object.keys(KPIS).forEach((clave) => {
      const fila = $(`[data-kpi="${clave}"]`);
      const k = datos.kpis[clave];
      if (!fila || !k) return;
      const def = KPIS[clave];
      const valor = $('[data-role="valor"]', fila);
      valor.innerHTML = valorHtml(def, k);
      valor.title = valorPlano(def, k);
      $('[data-role="delta"]', fila).innerHTML = variacion(
        def,
        k,
        PERIODOS[periodo].vs,
      );
      $('[data-role="spark"]', fila).innerHTML = sparkInterior(k.tendencia);
    });
    $("#dash-rango").textContent = PERIODOS[periodo].rango;
  }

  function renderEstrellas(datos) {
    const pintar = (idCuerpo, idTotal, items) => {
      $(idCuerpo).innerHTML = items.length
        ? items
            .map(
              (x) =>
                `<tr><td>${esc(x.nombre)}</td><td><span class="star-bar-track"><span class="star-bar-fill" style="width:${Math.min(100, x.porcentaje)}%"></span></span></td><td class="percentage">${x.porcentaje}%</td></tr>`,
            )
            .join("")
        : '<tr><td class="star-empty" colspan="3">Sin datos</td></tr>';
      $(idTotal).textContent =
        `${items.reduce((a, x) => a + x.porcentaje, 0)}%`;
    };
    pintar(
      "#dash-estrella-clientes",
      "#dash-estrella-clientes-total",
      datos.clientesEstrella,
    );
    pintar(
      "#dash-estrella-productos",
      "#dash-estrella-productos-total",
      datos.productosEstrella,
    );
  }

  function iniciales(nombre) {
    const p = nombre.trim().split(/\s+/);
    return (
      ((p[0] || "")[0] || "").toUpperCase() +
      ((p[1] || "")[0] || "").toUpperCase()
    );
  }

  function renderFrecuentes(datos) {
    const estrellas = new Set(datos.clientesEstrella.map((c) => c.nombre));
    $("#dash-frecuentes").innerHTML = datos.clientesFrecuentes.length
      ? datos.clientesFrecuentes
          .map((c) => {
            const veces = `${c.compras} ${c.compras === 1 ? "compra" : "compras"}`;
            return `<div class="customer-item${estrellas.has(c.nombre) ? " customer-highlight" : ""}">
                    <span class="avatar" aria-hidden="true">${esc(iniciales(c.nombre))}</span>
                    <span class="cust-name" title="${esc(c.nombre)}">${esc(c.nombre)}</span>
                    <span class="compras-tag">${esc(veces)}</span>
                  </div>`;
          })
          .join("")
      : '<div class="customer-item star-empty">Sin datos</div>';
  }

  function renderGastos(datos) {
    const g = datos.gastos;
    const total = g.garantia + g.insumos;
    const ingresos = datos.kpis.ingresos.valor;
    $("#dash-gasto-garantia").innerHTML = dineroUnidad(g.garantia);
    $("#dash-gasto-insumos").innerHTML = dineroUnidad(g.insumos);
    $("#dash-gasto-total").innerHTML = dineroUnidad(total);
    $("#dash-gasto-pct").textContent =
      `${limpio(ingresos ? (total / ingresos) * 100 : 0, 0)}%`;
  }

  function renderMejorVendidos(datos) {
    const items = datos.mejorVendidos.slice(0, 10);
    const item = (
      p,
      i,
    ) => `<div class="product-item${i < 3 ? ` product-item-${i + 1}` : ""}" title="${esc(p.nombre)}: ${miles(p.unidades)} unidades">
              <span class="best-rank">${i + 1}</span>
              <span class="best-name">${esc(p.nombre)}</span>
              <span class="best-units">${miles(p.unidades)}</span>
            </div>`;
    $("#dash-vendidos-a").innerHTML = items
      .slice(0, 5)
      .map((p, i) => item(p, i))
      .join("");
    $("#dash-vendidos-b").innerHTML = items
      .slice(5, 10)
      .map((p, i) => item(p, i + 5))
      .join("");
  }

  /* ---------- 7. Gráfica de ingresos (Chart.js) ---------- */
  let grafico = null;
  let titulosGrafico = [];

  function etiquetas(periodo, n) {
    const hoy = new Date();
    const ejeX = [],
      titulos = [];
    for (let i = 0; i < n; i++) {
      if (periodo === "hoy") {
        ejeX.push(`${8 + i}:00`);
        titulos.push(`${8 + i}:00 a ${9 + i}:00`);
      } else if (periodo === "anio") {
        const d = new Date(hoy.getFullYear(), hoy.getMonth() - (n - 1 - i), 1);
        const mes = MESES[d.getMonth()];
        ejeX.push(mes.charAt(0).toUpperCase() + mes.slice(1));
        titulos.push(`${mes} ${d.getFullYear()}`);
      } else {
        const d = new Date(hoy);
        d.setDate(hoy.getDate() - (n - 1 - i));
        ejeX.push(
          periodo === "7d"
            ? `${DIAS[d.getDay()]} ${d.getDate()}`
            : `${d.getDate()}`,
        );
        titulos.push(
          `${DIAS[d.getDay()]} ${d.getDate()} ${MESES[d.getMonth()]}`,
        );
      }
    }
    return { ejeX, titulos };
  }

  // Ejes en notación K / M, como la gráfica original (200K, 150K, ...)
  function ejeCorto(v) {
    if (v >= 1e6) return `${limpio(v / 1e6, 1)}M`;
    if (v >= 1e3) return `${limpio(v / 1e3, 1)}K`;
    return String(v);
  }

  function crearGrafico(canvas) {
    if (typeof Chart === "undefined") return null;
    if (
      window.matchMedia &&
      window.matchMedia("(prefers-reduced-motion: reduce)").matches
    )
      Chart.defaults.animation = false;
    Chart.defaults.font.family =
      '"Inter-Regular", Helvetica, Arial, sans-serif';
    Chart.defaults.font.size = 12;
    Chart.defaults.color = "#020f1e80";

    // Línea/área: cada serie con su color de línea y un relleno suave del mismo color.
    const serie = (label, color, fill, order, extra = {}) => ({
      label,
      data: [],
      borderColor: color,
      backgroundColor: fill,
      borderWidth: 2,
      fill: true,
      tension: 0.35,
      pointRadius: 0,
      pointHoverRadius: 4,
      pointHoverBackgroundColor: color,
      pointHitRadius: 10,
      order,
      ...extra,
    });
    return new Chart(canvas.getContext("2d"), {
      type: "line",
      data: {
        labels: [],
        datasets: [
          serie("Ventas", COLOR_VENTAS, "#2563eb14", 2),
          serie("Servicios", COLOR_SERVICIOS, "#f4843020", 1),
          // Total: discontinua, sin relleno y dibujada por encima (order 0) para que no la tapen las áreas
          serie("Total", COLOR_TOTAL, "transparent", 0, {
            fill: false,
            borderDash: [7, 5],
            borderWidth: 2.5,
          }),
        ],
      },
      options: {
        responsive: true,
        maintainAspectRatio: false,
        interaction: { mode: "index", intersect: false },
        plugins: {
          legend: { display: false },
          tooltip: {
            backgroundColor: "#020f1e",
            padding: 12,
            cornerRadius: 10,
            boxPadding: 4,
            callbacks: {
              title: (items) =>
                titulosGrafico[items[0].dataIndex] || items[0].label,
              label: (c) =>
                ` ${c.dataset.label}: ${dineroCompleto(c.parsed.y)}`,
            },
          },
        },
        scales: {
          x: {
            grid: { display: false },
            border: { display: false },
            ticks: { maxRotation: 0, autoSkip: true, maxTicksLimit: 12 },
          },
          y: {
            beginAtZero: true,
            grid: { color: "#020f1e33" },
            border: { display: false },
            ticks: { maxTicksLimit: 5, callback: ejeCorto },
          },
        },
      },
    });
  }

  function renderGrafico(datos, periodo) {
    const ventas = datos.ingresos.ventas,
      servicios = datos.ingresos.servicios;
    const total = ventas.map((v, i) => v + servicios[i]);
    const { ejeX, titulos } = etiquetas(periodo, ventas.length);
    titulosGrafico = titulos;

    const sum = (a) => a.reduce((x, y) => x + y, 0);
    const canvas = $("#dash-chart");
    canvas.setAttribute(
      "aria-label",
      `Gráfica de ingresos por ${PERIODOS[periodo].por}. Ventas ${dineroCompleto(sum(ventas))}, servicios ${dineroCompleto(sum(servicios))}, total ${dineroCompleto(sum(total))}.`,
    );

    if (!grafico) grafico = crearGrafico(canvas);
    if (!grafico) {
      $("#dash-chart-wrap").classList.add("is-error");
      return;
    }
    grafico.data.labels = ejeX;
    grafico.data.datasets[0].data = ventas;
    grafico.data.datasets[1].data = servicios;
    grafico.data.datasets[2].data = total;
    grafico.update();
  }

  /* ---------- 8. Carga y eventos ---------- */
  let periodoActual = "mes";
  let solicitud = 0;

  async function cargar(periodo) {
    const id = ++solicitud;
    raiz.setAttribute("aria-busy", "true");
    let datos = null;
    try {
      datos = await obtenerDatosDashboard(periodo);
    } catch (e) {
      datos = null;
    }
    if (id !== solicitud) return; // llegó una respuesta más nueva
    raiz.removeAttribute("aria-busy");
    if (!datos) {
      $("#dash-error").hidden = false;
      return;
    }
    $("#dash-error").hidden = true;
    periodoActual = periodo;
    raiz.querySelectorAll("[data-periodo]").forEach((b) => {
      const activo = b.dataset.periodo === periodo;
      b.classList.toggle("active", activo);
      b.setAttribute("aria-pressed", String(activo));
    });
    renderKpis(datos, periodo);
    renderGrafico(datos, periodo);
    renderEstrellas(datos);
    renderFrecuentes(datos);
    renderGastos(datos);
    renderMejorVendidos(datos);
  }

  window.cambiarPeriodoDashboard = function (periodo) {
    cargar(PERIODOS[periodo] ? periodo : periodoActual);
  };

  window.alternarSerieIngresos = function (indice, boton) {
    if (!grafico) return;
    const visible = grafico.isDatasetVisible(indice);
    grafico.setDatasetVisibility(indice, !visible);
    if (boton) boton.setAttribute("aria-pressed", String(!visible));
    grafico.update();
  };

  function iniciarDashboard() {
    cargar(periodoActual);
  }

  if (document.readyState === "loading") {
    document.addEventListener("DOMContentLoaded", iniciarDashboard);
  } else {
    iniciarDashboard();
  }
})();

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
