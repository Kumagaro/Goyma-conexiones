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

  // Al entrar a una sección con datos, nos aseguramos de que esté renderizada
  if (targetId === "productos") renderProductos();
  if (targetId === "categorias") renderCategorias();
  if (targetId === "clientes") renderClientes();
  if (targetId === "proveedores") renderProveedores();
  if (targetId === "unidades") {
    renderUnidades();
    renderConversiones();
    actualizarConversorRapido();
  }
  if (targetId === "configuracion") {
    renderMonedas();
    renderHistorialTasas();
    renderRateCards();
  }
  if (targetId === "facturas-general") renderFacturasGeneral();
  if (targetId === "facturas-entrada") renderFacturasEntrada();
  if (targetId === "facturas-venta") renderFacturasVenta();
  if (targetId === "facturas-insumos") renderFacturasInsumos();
  if (targetId === "facturas-garantia") renderFacturasGarantia();
  if (targetId === "facturas-perdidas") renderFacturasPerdidas();
  if (targetId === "facturas-servicios") renderFacturasServicios();
  if (targetId === "notificaciones") renderNotificaciones();
  document
    .querySelector("main.panel")
    .scrollTo?.({ top: 0, behavior: "smooth" });
  window.scrollTo({ top: 0, behavior: "smooth" });
  return false;
}

// ===== Sub-pestañas: Categorñas (Listado / Crear) =====
function showCategoriaTab(tab, btn) {
  document
    .querySelectorAll("#page-categorias .config-subview")
    .forEach((v) => v.classList.remove("active"));
  document.getElementById("categoria-" + tab).classList.add("active");
  document
    .querySelectorAll("#page-categorias .view-tabs .tab")
    .forEach((t) => t.classList.remove("active"));
  if (btn) btn.classList.add("active");
  document.getElementById("categoria-search-bar").style.display =
    tab === "list" ? "flex" : "none";
}

let categoriaVerId = null;

function verCategoria(id) {
  const cat = categoriasData.find((c) => c.id === id);
  if (!cat) return;
  categoriaVerId = id;
  document.getElementById("categoria-ver-id").textContent = "#" + cat.id;
  document.getElementById("categoria-ver-nombre").textContent = cat.nombre;
  document.getElementById("categoria-ver-descripcion").textContent =
    cat.descripcion;
  document.getElementById("categoria-ver-ingresos").textContent = cat.ingresos;
  document.getElementById("categoria-ver-cantidad").textContent = cat.cantidad;

  const productos = productosData.filter((p) => p.categoria === cat.nombre);
  const lista = document.getElementById("categoria-ver-productos");
  lista.innerHTML = productos.length
    ? productos
        .map(
          (p) => `
                    <div class="pv-invoice-card" style="cursor:pointer" onclick="verProducto(${p.id})">
                        <div class="kpi-icon"><svg class="icon"><use href="#icon-box-tick"></use></svg></div>
                        <div>
                            <span class="pv-invoice-label">${p.nombre}</span>
                            <div class="pv-invoice-value">Stock: ${p.stock ?? 0}</div>
                        </div>
                    </div>
                `,
        )
        .join("")
    : '<p class="pv-empty-note">Esta categoría no tiene productos asociados.</p>';

  showCategoriaTab("ver", null);
}

function editarCategoriaDesdeVer() {
  if (categoriaVerId != null) editarCategoria(categoriaVerId);
}

function eliminarCategoriaDesdeVer() {
  if (categoriaVerId == null) return;
  const cat = categoriasData.find((c) => c.id === categoriaVerId);
  if (!cat) return;
  if (
    !confirm(
      `Â¿Eliminar la categorña "${cat.nombre}"? Esta acción no se puede deshacer.`,
    )
  )
    return;
  categoriasData = categoriasData.filter((c) => c.id !== cat.id);
  categoriaVerId = null;
  renderCategorias();
  showCategoriaTab("list", document.getElementById("tab-categoria-list"));
}

function cancelarCategoriaForm() {
  document.getElementById("categoria-form-title").textContent =
    "Datos de Categorña";
  document.getElementById("categoria-nombre").value = "";
  document.getElementById("categoria-descripcion").value = "";
  editingCategoriaId = null;
  renderProductPicker([]);
  showCategoriaTab("list", document.getElementById("tab-categoria-list"));
}

// ===== Sub-pestañas: Clientes (Listado / Crear) =====
function showClienteTab(tab, btn) {
  document
    .querySelectorAll("#page-clientes .config-subview")
    .forEach((v) => v.classList.remove("active"));
  document.getElementById("cliente-" + tab).classList.add("active");
  document
    .querySelectorAll("#page-clientes .view-tabs .tab")
    .forEach((t) => t.classList.remove("active"));
  if (btn) btn.classList.add("active");
  document.getElementById("cliente-search-bar").style.display =
    tab === "list" ? "flex" : "none";
  if (tab === "create" && !editingClienteId) cancelarClienteForm(false);
}

function cancelarClienteForm(volverALista = true) {
  editingClienteId = null;
  document.getElementById("cliente-form-title").textContent =
    "Datos de Cliente";
  [
    "nombres",
    "apellidos",
    "ci",
    "empresa",
    "cargo",
    "telefono",
    "correo",
    "direccion",
    "fecha-entrada",
    "info-factura",
  ].forEach((campo) => {
    const el = document.getElementById("cliente-" + campo);
    if (el) el.value = "";
  });
  document.getElementById("cliente-id-sistema").value = "";
  if (volverALista)
    showClienteTab("list", document.getElementById("tab-cliente-list"));
}

function editarCliente(id) {
  const c = clientesData.find((cl) => cl.id === id);
  if (!c) return;
  editingClienteId = id;
  document.getElementById("cliente-form-title").textContent =
    "Editar Cliente: " + c.nombre;
  document.getElementById("cliente-id-sistema").value = c.id || "";
  document.getElementById("cliente-nombres").value = c.nombres || "";
  document.getElementById("cliente-apellidos").value = c.apellidos || "";
  document.getElementById("cliente-ci").value = c.ci || "";
  document.getElementById("cliente-empresa").value = c.empresa || "";
  document.getElementById("cliente-cargo").value = c.cargo || "";
  document.getElementById("cliente-telefono").value = c.telefono || "";
  document.getElementById("cliente-correo").value = c.correo || "";
  document.getElementById("cliente-direccion").value = c.direccion || "";
  document.getElementById("cliente-fecha-entrada").value = c.fechaEntrada || "";
  document.getElementById("cliente-info-factura").value = c.infoFactura || "";
  showClienteTab("create", document.getElementById("tab-cliente-create"));
}

function guardarCliente(e) {
  e.preventDefault();
  const nombres = document.getElementById("cliente-nombres").value.trim();
  const apellidos = document.getElementById("cliente-apellidos").value.trim();
  const ci = document.getElementById("cliente-ci").value.trim();
  const telefono = document.getElementById("cliente-telefono").value.trim();
  if (!nombres || !apellidos || !ci || !telefono) return false;

  const datos = {
    nombres,
    apellidos,
    nombre: nombres + " " + apellidos,
    ci,
    empresa: document.getElementById("cliente-empresa").value.trim(),
    cargo: document.getElementById("cliente-cargo").value.trim(),
    telefono,
    correo: document.getElementById("cliente-correo").value.trim(),
    direccion: document.getElementById("cliente-direccion").value.trim(),
    fechaEntrada: document.getElementById("cliente-fecha-entrada").value,
    infoFactura: document.getElementById("cliente-info-factura").value.trim(),
  };

  if (editingClienteId) {
    const c = clientesData.find((cl) => cl.id === editingClienteId);
    if (c) Object.assign(c, datos);
  } else {
    clientesData.push(Object.assign({ id: "#" + nextClienteSeq++ }, datos));
  }
  cancelarClienteForm();
  renderClientes();
  poblarDatalists();
  return false;
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
// ============ Datos de ejemplo (arreglos JS) =========
// DATOS DE PRUEBA - PLACEHOLDER
// Estos datos serán reemplazados por llamadas al backend cuando se implementen
// los módulos correspondientes de categorías.
// =====================================================
let categoriasData = [
  {
    id: 1,
    nombre: "Laptop",
    descripcion: "Electrónicos / Portátiles",
    cantidad: 20,
    ingresos: "15 000",
  },
  {
    id: 2,
    nombre: "Celulares",
    descripcion: "Electrónicos / Móviles",
    cantidad: 35,
    ingresos: "28 500",
  },
  {
    id: 3,
    nombre: "Accesorios",
    descripcion: "Electrónicos / Periféricos",
    cantidad: 60,
    ingresos: "9 200",
  },
  {
    id: 4,
    nombre: "Electrodomésticos",
    descripcion: "Hogar / Cocina",
    cantidad: 18,
    ingresos: "22 000",
  },
  {
    id: 5,
    nombre: "Muebles",
    descripcion: "Hogar / Sala",
    cantidad: 12,
    ingresos: "11 800",
  },
];
let nextCategoriaId = categoriasData.length + 1;
let editingCategoriaId = null;

// =====================================================
// ============ Unidades de Medida y Conversiones ======
// =====================================================
let unidadesData = [
  { id: 1, nombre: "Unidad", abreviacion: "und" },
  { id: 2, nombre: "Caja", abreviacion: "cj" },
  { id: 3, nombre: "Paquete", abreviacion: "paq" },
  { id: 4, nombre: "Kilogramo", abreviacion: "kg" },
  { id: 5, nombre: "Gramo", abreviacion: "g" },
  { id: 6, nombre: "Litro", abreviacion: "L" },
  { id: 7, nombre: "Mililitro", abreviacion: "ml" },
  { id: 8, nombre: "Metro", abreviacion: "m" },
  { id: 9, nombre: "Centí­metro", abreviacion: "cm" },
];
let nextUnidadId = 10;
let editingUnidadId = null;

let conversionesData = [
  { id: 1, origenId: 2, destinoId: 1, factor: 12 }, // 1 Caja = 12 Unidad
  { id: 2, origenId: 4, destinoId: 5, factor: 1000 }, // 1 Kilogramo = 1000 Gramo
  { id: 3, origenId: 6, destinoId: 7, factor: 1000 }, // 1 Litro = 1000 Mililitro
  { id: 4, origenId: 8, destinoId: 9, factor: 100 }, // 1 Metro = 100 Centí­metro
];
let nextConversionId = 5;
let editingConversionId = null;

function buscarUnidad(id) {
  return unidadesData.find((u) => u.id === id);
}

// Lista de nombres de unidades reales creadas en el sistema (fuente única para
// todos los selects de unidad de la app: Productos y las lí­neas de cada tipo de factura)
function nombresUnidad() {
  return unidadesData
    .map((u) => u.nombre)
    .slice()
    .sort((a, b) => a.localeCompare(b, "es"));
}

function opcionesUnidadHTML(seleccionado) {
  return nombresUnidad()
    .map((u) => `<option ${u === seleccionado ? "selected" : ""}>${u}</option>`)
    .join("");
}

// Motor de conversión: recorre el grafo de conversiones (BFS) para encontrar el
// factor entre dos unidades cualquiera, incluso encadenando varias conversiones.
function obtenerFactorConversion(origenId, destinoId) {
  if (origenId === destinoId) return 1;
  const visitados = new Set([origenId]);
  const cola = [{ id: origenId, factor: 1 }];
  while (cola.length) {
    const actual = cola.shift();
    for (const c of conversionesData) {
      let vecinoId = null,
        factorPaso = null;
      if (c.origenId === actual.id) {
        vecinoId = c.destinoId;
        factorPaso = c.factor;
      } else if (c.destinoId === actual.id) {
        vecinoId = c.origenId;
        factorPaso = 1 / c.factor;
      }
      if (vecinoId !== null && !visitados.has(vecinoId)) {
        const nuevoFactor = actual.factor * factorPaso;
        if (vecinoId === destinoId) return nuevoFactor;
        visitados.add(vecinoId);
        cola.push({ id: vecinoId, factor: nuevoFactor });
      }
    }
  }
  return null; // No existe una ruta de conversión conocida entre estas unidades
}

function formatearNumeroUnidad(n) {
  if (!isFinite(n)) return ":";
  const redondeado = Math.round(n * 10000) / 10000;
  return redondeado.toLocaleString("es-ES", { maximumFractionDigits: 4 });
}

function renderUnidades() {
  const body = document.getElementById("unidades-body");
  if (!body) return;
  const query = (
    document.getElementById("unidad-search")?.value || ""
  ).toLowerCase();
  const rows = unidadesData.filter(
    (u) =>
      u.nombre.toLowerCase().includes(query) ||
      u.abreviacion.toLowerCase().includes(query),
  );

  if (rows.length === 0) {
    body.innerHTML =
      '<tr><td colspan="5" class="empty-row">No se encontraron unidades que coincidan con la búsqueda.</td></tr>';
    return;
  }

  body.innerHTML = rows
    .map((u) => {
      const enUso = productosData.filter((p) => p.unidad === u.nombre).length;
      return `
                <tr>
                    <td data-label="ID">#${String(u.id).padStart(4, "0")}</td>
                    <td data-label="Nombre">${u.nombre}</td>
                    <td data-label="Abreviación">${u.abreviacion}</td>
                    <td data-label="Productos con Unidad" style="text-align:center">
                        <span class="conteo-uso ${enUso > 0 ? "en-uso" : ""}">${enUso}</span>
                    </td>
                    <td>
                        <div class="col-acciones">
                            <button class="action-link" onclick="abrirModalUnidad(${u.id})">
                                <svg class="icon" style="width:16px;height:16px"><use href="#icon-edit"></use></svg>
                                Editar
                            </button>
                            <button class="action-link danger" onclick="eliminarUnidad(${u.id})">
                                <svg class="icon" style="width:16px;height:16px"><use href="#icon-trash"></use></svg>
                                Eliminar
                            </button>
                        </div>
                    </td>
                </tr>
            `;
    })
    .join("");
}

function renderConversiones() {
  const body = document.getElementById("conversiones-body");
  if (!body) return;
  const query = (
    document.getElementById("unidad-search")?.value || ""
  ).toLowerCase();
  const rows = conversionesData.filter((c) => {
    const o = buscarUnidad(c.origenId),
      d = buscarUnidad(c.destinoId);
    if (!o || !d) return false;
    return (
      o.nombre.toLowerCase().includes(query) ||
      d.nombre.toLowerCase().includes(query) ||
      o.abreviacion.toLowerCase().includes(query) ||
      d.abreviacion.toLowerCase().includes(query)
    );
  });

  if (rows.length === 0) {
    body.innerHTML =
      '<tr><td colspan="5" class="empty-row">No se encontraron conversiones que coincidan con la búsqueda.</td></tr>';
    return;
  }

  body.innerHTML = rows
    .map((c) => {
      const o = buscarUnidad(c.origenId),
        d = buscarUnidad(c.destinoId);
      if (!o || !d) return "";
      return `
                <tr>
                    <td data-label="Unidad de Origen">${o.nombre} (${o.abreviacion})</td>
                    <td data-label="Unidad de Destino">${d.nombre} (${d.abreviacion})</td>
                    <td data-label="Factor" style="text-align:center">${formatearNumeroUnidad(c.factor)}</td>
                    <td data-label="Equivalencia">1 ${o.abreviacion} = ${formatearNumeroUnidad(c.factor)} ${d.abreviacion}</td>
                    <td>
                        <div class="col-acciones">
                            <button class="action-link" onclick="abrirModalConversion(${c.id})">
                                <svg class="icon" style="width:16px;height:16px"><use href="#icon-edit"></use></svg>
                                Editar
                            </button>
                            <button class="action-link danger" onclick="eliminarConversion(${c.id})">
                                <svg class="icon" style="width:16px;height:16px"><use href="#icon-trash"></use></svg>
                                Eliminar
                            </button>
                        </div>
                    </td>
                </tr>
            `;
    })
    .join("");
}

// ----- Modal: Unidad de Medida -----
function abrirModalUnidad(id) {
  editingUnidadId = id || null;
  const titulo = document.getElementById("modal-unidad-title");
  const nombreInput = document.getElementById("modal-unidad-nombre");
  const abrevInput = document.getElementById("modal-unidad-abreviacion");
  if (id) {
    const u = buscarUnidad(id);
    if (!u) return;
    titulo.textContent = "Editar Unidad: " + u.nombre;
    nombreInput.value = u.nombre;
    abrevInput.value = u.abreviacion;
  } else {
    titulo.textContent = "Nueva Unidad de Medida";
    nombreInput.value = "";
    abrevInput.value = "";
  }
  const overlay = document.getElementById("modal-unidad");
  overlay.classList.add("active");
  setTimeout(() => nombreInput.focus(), 250);
}

function cerrarModalUnidad() {
  document.getElementById("modal-unidad").classList.remove("active");
  editingUnidadId = null;
}

function guardarUnidad(e) {
  e.preventDefault();
  const nombre = document.getElementById("modal-unidad-nombre").value.trim();
  const abreviacion = document
    .getElementById("modal-unidad-abreviacion")
    .value.trim();
  if (!nombre || !abreviacion) return false;

  const duplicado = unidadesData.some(
    (u) =>
      u.nombre.toLowerCase() === nombre.toLowerCase() &&
      u.id !== editingUnidadId,
  );
  if (duplicado) {
    alert(`Ya existe una unidad llamada "${nombre}".`);
    return false;
  }

  if (editingUnidadId) {
    const u = buscarUnidad(editingUnidadId);
    if (u) {
      const nombreAnterior = u.nombre;
      u.nombre = nombre;
      u.abreviacion = abreviacion;
      // Si el nombre cambió, actualizamos las referencias existentes por texto
      if (nombreAnterior !== nombre) {
        productosData.forEach((p) => {
          if (p.unidad === nombreAnterior) p.unidad = nombre;
        });
      }
    }
  } else {
    unidadesData.push({ id: nextUnidadId++, nombre, abreviacion });
  }
  cerrarModalUnidad();
  renderUnidades();
  renderConversiones();
  poblarSelectUnidadesProducto(
    document.getElementById("producto-unidad")?.value,
  );
  poblarSelectoresConversor();
  return false;
}

function eliminarUnidad(id) {
  const u = buscarUnidad(id);
  if (!u) return;
  const enUsoPorProductos = productosData.filter(
    (p) => p.unidad === u.nombre,
  ).length;
  const enUsoPorConversiones = conversionesData.filter(
    (c) => c.origenId === id || c.destinoId === id,
  ).length;
  let mensaje = `Â¿Eliminar la unidad "${u.nombre}"? Esta acción no se puede deshacer.`;
  if (enUsoPorProductos > 0 || enUsoPorConversiones > 0) {
    mensaje =
      `La unidad "${u.nombre}" está en uso por ${enUsoPorProductos} producto(s) y ${enUsoPorConversiones} conversión(es). ` +
      `Si la eliminas, esas conversiones también se eliminarán. Â¿Deseas continuar?`;
  }
  if (!confirm(mensaje)) return;
  unidadesData = unidadesData.filter((u2) => u2.id !== id);
  conversionesData = conversionesData.filter(
    (c) => c.origenId !== id && c.destinoId !== id,
  );
  renderUnidades();
  renderConversiones();
  poblarSelectUnidadesProducto();
  poblarSelectoresConversor();
}

// ----- Modal: Conversión -----
function poblarSelectoresModalConversion(origenSel, destinoSel) {
  const opciones = unidadesData
    .map(
      (u) => `<option value="${u.id}">${u.nombre} (${u.abreviacion})</option>`,
    )
    .join("");
  const selOrigen = document.getElementById("modal-conversion-origen");
  const selDestino = document.getElementById("modal-conversion-destino");
  selOrigen.innerHTML = opciones;
  selDestino.innerHTML = opciones;
  if (origenSel) selOrigen.value = origenSel;
  if (destinoSel) selDestino.value = destinoSel;
}

function actualizarPreviaConversion() {
  const preview = document.getElementById("modal-conversion-preview");
  const origenId = Number(
    document.getElementById("modal-conversion-origen").value,
  );
  const destinoId = Number(
    document.getElementById("modal-conversion-destino").value,
  );
  const factor = parseFloat(
    document.getElementById("modal-conversion-factor").value,
  );
  const o = buscarUnidad(origenId),
    d = buscarUnidad(destinoId);
  if (!o || !d) {
    preview.textContent = "Selecciona ambas unidades.";
    return;
  }
  if (origenId === destinoId) {
    preview.textContent = "La unidad de origen y destino deben ser diferentes.";
    return;
  }
  if (!factor || factor <= 0) {
    preview.textContent = `1 ${o.abreviacion} =: ${d.abreviacion}`;
    return;
  }
  preview.textContent = `1 ${o.abreviacion} = ${formatearNumeroUnidad(factor)} ${d.abreviacion}`;
}

function abrirModalConversion(id) {
  editingConversionId = id || null;
  const titulo = document.getElementById("modal-conversion-title");
  const factorInput = document.getElementById("modal-conversion-factor");
  if (id) {
    const c = conversionesData.find((x) => x.id === id);
    if (!c) return;
    titulo.textContent = "Editar Conversión";
    poblarSelectoresModalConversion(c.origenId, c.destinoId);
    factorInput.value = c.factor;
  } else {
    titulo.textContent = "Nueva Conversión";
    poblarSelectoresModalConversion();
    factorInput.value = "";
  }
  actualizarPreviaConversion();
  const overlay = document.getElementById("modal-conversion");
  overlay.classList.add("active");
}

function cerrarModalConversion() {
  document.getElementById("modal-conversion").classList.remove("active");
  editingConversionId = null;
}

function guardarConversion(e) {
  e.preventDefault();
  const origenId = Number(
    document.getElementById("modal-conversion-origen").value,
  );
  const destinoId = Number(
    document.getElementById("modal-conversion-destino").value,
  );
  const factor = parseFloat(
    document.getElementById("modal-conversion-factor").value,
  );

  if (origenId === destinoId) {
    alert("La unidad de origen y destino deben ser diferentes.");
    return false;
  }
  if (!factor || factor <= 0) return false;

  const duplicado = conversionesData.some(
    (c) =>
      c.id !== editingConversionId &&
      ((c.origenId === origenId && c.destinoId === destinoId) ||
        (c.origenId === destinoId && c.destinoId === origenId)),
  );
  if (duplicado) {
    alert("Ya existe una conversión definida entre estas dos unidades.");
    return false;
  }

  if (editingConversionId) {
    const c = conversionesData.find((x) => x.id === editingConversionId);
    if (c) Object.assign(c, { origenId, destinoId, factor });
  } else {
    conversionesData.push({
      id: nextConversionId++,
      origenId,
      destinoId,
      factor,
    });
  }
  cerrarModalConversion();
  renderConversiones();
  actualizarConversorRapido();
  return false;
}

function eliminarConversion(id) {
  const c = conversionesData.find((x) => x.id === id);
  if (!c) return;
  if (!confirm("Â¿Eliminar esta conversión? Esta acción no se puede deshacer."))
    return;
  conversionesData = conversionesData.filter((x) => x.id !== id);
  renderConversiones();
  actualizarConversorRapido();
}

// ----- Conversor Rápido -----
function poblarSelectoresConversor() {
  const selOrigen = document.getElementById("conversor-origen");
  const selDestino = document.getElementById("conversor-destino");
  if (!selOrigen || !selDestino) return;
  const valorOrigenPrevio = selOrigen.value,
    valorDestinoPrevio = selDestino.value;
  const opciones = unidadesData
    .map(
      (u) => `<option value="${u.id}">${u.nombre} (${u.abreviacion})</option>`,
    )
    .join("");
  selOrigen.innerHTML = opciones;
  selDestino.innerHTML = opciones;
  if (
    valorOrigenPrevio &&
    unidadesData.some((u) => String(u.id) === valorOrigenPrevio)
  )
    selOrigen.value = valorOrigenPrevio;
  if (
    valorDestinoPrevio &&
    unidadesData.some((u) => String(u.id) === valorDestinoPrevio)
  ) {
    selDestino.value = valorDestinoPrevio;
  } else if (unidadesData[1]) {
    selDestino.value = unidadesData[1].id;
  }
}

function actualizarConversorRapido() {
  const resultado = document.getElementById("conversor-resultado");
  if (!resultado) return;
  if (!document.getElementById("conversor-origen").options.length)
    poblarSelectoresConversor();

  const cantidad =
    parseFloat(document.getElementById("conversor-cantidad").value) || 0;
  const origenId = Number(document.getElementById("conversor-origen").value);
  const destinoId = Number(document.getElementById("conversor-destino").value);
  const o = buscarUnidad(origenId),
    d = buscarUnidad(destinoId);
  if (!o || !d) return;

  if (origenId === destinoId) {
    resultado.classList.remove("sin-ruta");
    resultado.textContent = `${formatearNumeroUnidad(cantidad)} ${o.abreviacion} = ${formatearNumeroUnidad(cantidad)} ${d.abreviacion}`;
    return;
  }

  const factor = obtenerFactorConversion(origenId, destinoId);
  if (factor === null) {
    resultado.classList.add("sin-ruta");
    resultado.innerHTML = `No hay una conversión definida entre <strong>${o.nombre}</strong> y <strong>${d.nombre}</strong>. Crea una en la tabla de Conversiones.`;
    return;
  }
  resultado.classList.remove("sin-ruta");
  resultado.textContent = `${formatearNumeroUnidad(cantidad)} ${o.abreviacion} = ${formatearNumeroUnidad(cantidad * factor)} ${d.abreviacion}`;
}

function invertirConversorRapido() {
  const selOrigen = document.getElementById("conversor-origen");
  const selDestino = document.getElementById("conversor-destino");
  const tmp = selOrigen.value;
  selOrigen.value = selDestino.value;
  selDestino.value = tmp;
  actualizarConversorRapido();
}

// Cierra los modales del dashboard al presionar Escape.
document.addEventListener("keydown", (e) => {
  if (e.key !== "Escape") return;
  if (document.getElementById("modal-unidad")?.classList.contains("active"))
    cerrarModalUnidad();
  if (document.getElementById("modal-conversion")?.classList.contains("active"))
    cerrarModalConversion();
});

// =====================================================
// ========================= Productos =================
// DATOS DE PRUEBA - PLACEHOLDER
// Estos datos serán reemplazados por llamadas al backend cuando se implemente
// el módulo de productos.
// =====================================================
let productosData = [
  {
    id: 1,
    nombre: "Laptop HP 15",
    descripcion: "Laptop de uso general, 8GB RAM / 256GB SSD.",
    categoria: "Laptop",
    unidad: "Unidad",
    precioEntrada: 380,
    precioVenta: 500,
    stock: 12,
    caducidad: null,
  },
  {
    id: 2,
    nombre: "Mouse Inalámbrico",
    descripcion: "Mouse óptico inalámbrico 2.4GHz.",
    categoria: "Accesorios",
    unidad: "Unidad",
    precioEntrada: 8,
    precioVenta: 15,
    stock: 46,
    caducidad: null,
  },
  {
    id: 3,
    nombre: "Teclado Mecánico",
    descripcion: "Teclado mecánico retroiluminado.",
    categoria: "Accesorios",
    unidad: "Unidad",
    precioEntrada: 22,
    precioVenta: 40,
    stock: 28,
    caducidad: null,
  },
  {
    id: 4,
    nombre: 'Monitor 24"',
    descripcion: "Monitor LED Full HD 24 pulgadas.",
    categoria: "Accesorios",
    unidad: "Unidad",
    precioEntrada: 90,
    precioVenta: 130,
    stock: 15,
    caducidad: null,
  },
  {
    id: 5,
    nombre: "Silla de Oficina",
    descripcion: "Silla ergonómica con soporte lumbar.",
    categoria: "Muebles",
    unidad: "Unidad",
    precioEntrada: 60,
    precioVenta: 95,
    stock: 9,
    caducidad: null,
  },
  {
    id: 6,
    nombre: "Cámara Web",
    descripcion: "Cámara web HD 1080p con micrófono.",
    categoria: "Accesorios",
    unidad: "Unidad",
    precioEntrada: 14,
    precioVenta: 25,
    stock: 33,
    caducidad: null,
  },
  {
    id: 7,
    nombre: "Router WiFi",
    descripcion: "Router doble banda AC1200.",
    categoria: "Electrodomésticos",
    unidad: "Unidad",
    precioEntrada: 25,
    precioVenta: 45,
    stock: 20,
    caducidad: null,
  },
  {
    id: 8,
    nombre: "Disco SSD 1TB",
    descripcion: "Unidad de estado sólido SATA III.",
    categoria: "Accesorios",
    unidad: "Unidad",
    precioEntrada: 55,
    precioVenta: 80,
    stock: 24,
    caducidad: null,
  },
];
let nextProductoId = productosData.length + 1;
let editingProductoId = null;
let productosDisponibles = [];

function actualizarProductosDisponibles() {
  productosDisponibles = productosData.map((p) => p.nombre);
  poblarDatalists();
}

function poblarSelectCategoriasProducto(seleccionar) {
  const sel = document.getElementById("producto-categoria");
  if (!sel) return;
  sel.innerHTML = categoriasData
    .map((c) => `<option value="${c.nombre}">${c.nombre}</option>`)
    .join("");
  if (seleccionar) sel.value = seleccionar;
}

function poblarSelectUnidadesProducto(seleccionar) {
  const sel = document.getElementById("producto-unidad");
  if (!sel) return;
  sel.innerHTML = opcionesUnidadHTML(seleccionar);
  if (seleccionar) sel.value = seleccionar;
}

function agregarUnidadRapida() {
  const nombre = prompt(
    "Nombre de la nueva unidad de medida (Ej. Metro, Galón, Docena):",
  );
  if (nombre === null) return;
  const limpio = nombre.trim();
  if (!limpio) return;
  if (
    !unidadesData.some((u) => u.nombre.toLowerCase() === limpio.toLowerCase())
  ) {
    const abrevSugerida = limpio.slice(0, 3).toLowerCase();
    const abrev = prompt(
      `Abreviación para "${limpio}" (Ej. m, gal, doc):`,
      abrevSugerida,
    );
    if (abrev === null) return;
    unidadesData.push({
      id: nextUnidadId++,
      nombre: limpio,
      abreviacion: abrev.trim() || abrevSugerida,
    });
    poblarSelectoresConversor();
    if (
      document.getElementById("page-unidades")?.classList.contains("active")
    ) {
      renderUnidades();
    }
  }
  poblarSelectUnidadesProducto(limpio);
}

function irCrearCategoria() {
  showPage(
    "categorias",
    document.querySelector('#sidebar a[data-page="categorias"]'),
  );
  showCategoriaTab("create", document.getElementById("tab-categoria-create"));
}

function irCrearProducto() {
  showPage(
    "productos",
    document.querySelector('#sidebar a[data-page="productos"]'),
  );
  showProductoTab("create", document.getElementById("tab-producto-create"));
}

function toggleCaducidadProducto() {
  const tipo = document.getElementById("producto-caducidad-tipo").value;
  const wrap = document.getElementById("producto-caducidad-fecha-wrap");
  if (tipo === "fecha") {
    wrap.classList.remove("field-oculto");
  } else {
    wrap.classList.add("field-oculto");
    document.getElementById("producto-caducidad-fecha").value = "";
  }
}

function showProductoTab(tab, btn) {
  document
    .querySelectorAll("#page-productos .config-subview")
    .forEach((v) => v.classList.remove("active"));
  document.getElementById("producto-" + tab).classList.add("active");
  document
    .querySelectorAll("#page-productos .view-tabs .tab")
    .forEach((t) => t.classList.remove("active"));
  if (btn) btn.classList.add("active");
  document.getElementById("producto-search-bar").style.display =
    tab === "list" ? "flex" : "none";
  if (tab === "create" && !editingProductoId) prepararNuevoProducto();
  if (tab === "historial") {
    poblarSelectHistorialProducto();
    renderHistorialProducto();
  }
}

function prepararNuevoProducto() {
  editingProductoId = null;
  document.getElementById("producto-form-title").textContent =
    "Crear Nuevo Producto";
  document.getElementById("producto-nombre").value = "";
  document.getElementById("producto-id-sistema").value = "";
  document.getElementById("producto-descripcion").value = "";
  poblarSelectCategoriasProducto();
  poblarSelectUnidadesProducto();
  document.getElementById("producto-precio-entrada").value = "";
  document.getElementById("producto-precio-venta").value = "";
  document.getElementById("producto-stock").value = "";
  document.getElementById("producto-caducidad-tipo").value = "no-aplica";
  document.getElementById("producto-caducidad-fecha").value = "";
  document.getElementById("producto-factura-compra").value = "";
  document.getElementById("producto-factura-venta").value = "";
  toggleCaducidadProducto();
  actualizarGananciaProducto();
}

function cancelarProductoForm() {
  editingProductoId = null;
  showProductoTab("list", document.getElementById("tab-producto-list"));
}

function editarProducto(id) {
  const p = productosData.find((x) => x.id === id);
  if (!p) return;
  editingProductoId = id;
  document.getElementById("producto-form-title").textContent =
    "Editar Producto: " + p.nombre;
  document.getElementById("producto-nombre").value = p.nombre;
  document.getElementById("producto-id-sistema").value = formatearIdProducto(
    p.id,
  );
  document.getElementById("producto-descripcion").value = p.descripcion || "";
  poblarSelectCategoriasProducto(p.categoria);
  poblarSelectUnidadesProducto(p.unidad);
  document.getElementById("producto-precio-entrada").value =
    p.precioEntrada || "";
  document.getElementById("producto-precio-venta").value = p.precioVenta || "";
  document.getElementById("producto-stock").value = p.stock || "";
  document.getElementById("producto-caducidad-tipo").value = p.caducidad
    ? "fecha"
    : "no-aplica";
  document.getElementById("producto-caducidad-fecha").value = p.caducidad || "";
  toggleCaducidadProducto();
  actualizarGananciaProducto();
  const facturas = obtenerFacturasProducto(p.nombre);
  document.getElementById("producto-factura-compra").value =
    facturas.compra.join(", ");
  document.getElementById("producto-factura-venta").value =
    facturas.venta.join(", ");
  showProductoTab("create", document.getElementById("tab-producto-create"));
}

// Calcula el % de ganancia en vivo a partir del precio de entrada y venta
function actualizarGananciaProducto() {
  const campo = document.getElementById("producto-ganancia");
  if (!campo) return;
  const entrada = parseFloat(
    document.getElementById("producto-precio-entrada").value,
  );
  const venta = parseFloat(
    document.getElementById("producto-precio-venta").value,
  );
  if (!entrada || entrada <= 0 || isNaN(venta)) {
    campo.value = "";
    return;
  }
  const ganancia = ((venta - entrada) / entrada) * 100;
  campo.value = ganancia.toFixed(1) + "%";
}

// Busca en qué facturas de entrada/venta ya guardadas aparece un producto
function obtenerFacturasProducto(nombreProducto) {
  const nombre = (nombreProducto || "").trim().toLowerCase();
  const compra = facturasEntradaData
    .filter((f) =>
      (f.productos || []).some((np) => np.trim().toLowerCase() === nombre),
    )
    .map((f) => f.numero);
  const venta = facturasVentaData
    .filter((f) =>
      (f.productos || []).some((np) => np.trim().toLowerCase() === nombre),
    )
    .map((f) => f.numero);
  return { compra, venta };
}

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

// ===== Historial de movimientos de un Producto =====
function poblarSelectHistorialProducto(nombreSeleccionado) {
  const select = document.getElementById("historial-producto-select");
  if (!select) return;
  const valorPrevio = nombreSeleccionado || select.value;
  select.innerHTML =
    '<option value="">Selecciona un producto...</option>' +
    productosData
      .map(
        (p) =>
          `<option value="${p.nombre.replace(/"/g, "&quot;")}">${p.nombre}</option>`,
      )
      .join("");
  if (valorPrevio) select.value = valorPrevio;
}

function obtenerMovimientosProducto(nombreProducto) {
  const nombre = (nombreProducto || "").trim().toLowerCase();
  const movimientos = [];

  facturasEntradaData.forEach((f) => {
    (f.lineas || [])
      .filter((li) => li.producto.trim().toLowerCase() === nombre)
      .forEach((li) => {
        movimientos.push({
          fecha: f.fechaEmision,
          tipo: "Entrada",
          cantidad: li.cantidad,
          precioUnitario: li.costo,
          factura: f.numero,
          usuario: f.responsable || ":",
        });
      });
  });
  facturasVentaData.forEach((f) => {
    (f.lineas || [])
      .filter((li) => li.producto.trim().toLowerCase() === nombre)
      .forEach((li) => {
        movimientos.push({
          fecha: f.fechaEmision,
          tipo: "Salida (Venta)",
          cantidad: li.cantidad,
          precioUnitario: li.costo,
          factura: f.numero,
          usuario: f.cliente || ":",
        });
      });
  });
  facturasPerdidasData.forEach((f) => {
    if (f.producto.trim().toLowerCase() === nombre) {
      movimientos.push({
        fecha: f.fecha,
        tipo: "Pérdida",
        cantidad: f.cantidad,
        precioUnitario: f.costo,
        factura: f.numero,
        usuario: ":",
      });
    }
  });

  // Orden cronológico ascendente (más antiguo primero) para poder reconstruir el stock resultante.
  movimientos.sort(
    (a, b) => fechaAValorOrdenable(a.fecha) - fechaAValorOrdenable(b.fecha),
  );

  // Calcula la cantidad que quedaba en stock justo después de cada movimiento: se ancla en el
  // stock ACTUAL del producto (el más reciente) y se reconstruye hacia atrás en el tiempo,
  // deshaciendo cada movimiento (restando las entradas, sumando de vuelta salidas/pérdidas).
  const producto = productosData.find(
    (p) => p.nombre.trim().toLowerCase() === nombre,
  );
  let stockAcumulado = producto ? producto.stock || 0 : 0;
  for (let i = movimientos.length - 1; i >= 0; i--) {
    movimientos[i].stockResultante = stockAcumulado;
    const efecto =
      movimientos[i].tipo === "Entrada"
        ? movimientos[i].cantidad
        : -movimientos[i].cantidad;
    stockAcumulado -= efecto;
  }

  return movimientos;
}

function renderHistorialProducto() {
  const body = document.getElementById("historial-producto-body");
  if (!body) return;
  const nombreProducto =
    document.getElementById("historial-producto-select")?.value || "";
  const desde =
    document.getElementById("historial-producto-desde")?.value || "";
  const hasta =
    document.getElementById("historial-producto-hasta")?.value || "";
  const tipoFiltro =
    document.getElementById("historial-producto-tipo")?.value || "";

  if (!nombreProducto) {
    body.innerHTML = "";
    document.getElementById("historial-producto-list").style.display = "none";
    if (!document.getElementById("historial-producto-vacio")) {
      document
        .getElementById("historial-producto-list")
        .insertAdjacentHTML(
          "afterend",
          '<div id="historial-producto-vacio" class="historial-vacio-selector">Selecciona un producto para consultar su historial de movimientos.</div>',
        );
    }
    return;
  }
  document.getElementById("historial-producto-list").style.display = "";
  document.getElementById("historial-producto-vacio")?.remove();

  const filasFiltradas = obtenerMovimientosProducto(nombreProducto).filter(
    (m) => {
      const cumpleTipo = !tipoFiltro || m.tipo === tipoFiltro;
      const cumpleDesde = !desde || (m.fecha || "") >= desde;
      const cumpleHasta = !hasta || (m.fecha || "") <= hasta;
      return cumpleTipo && cumpleDesde && cumpleHasta;
    },
  );
  const filas = ordenarPorFecha(filasFiltradas, "historialProducto", "fecha");

  if (filas.length === 0) {
    body.innerHTML =
      '<tr><td colspan="8" class="empty-row">No hay movimientos registrados para este producto en el perí­odo/tipo seleccionado.</td></tr>';
    return;
  }

  const claseTipo = {
    Entrada: "mov-entrada",
    "Salida (Venta)": "mov-salida",
    Pérdida: "mov-perdida",
  };
  body.innerHTML = filas
    .map(
      (m) => `
                <tr>
                    <td data-label="Fecha">${formatearFecha(m.fecha)}</td>
                    <td data-label="Tipo de Movimiento"><span class="mov-tipo ${claseTipo[m.tipo] || ""}">${m.tipo}</span></td>
                    <td data-label="Cantidad" style="text-align:center">${m.cantidad}</td>
                    <td data-label="Stock Resultante" style="text-align:center">${m.stockResultante}</td>
                    <td data-label="Precio Unitario" style="text-align:right">${(m.precioUnitario || 0).toLocaleString("es-ES", { minimumFractionDigits: 2, maximumFractionDigits: 2 })} USD</td>
                    <td data-label="Precio Total" style="text-align:right">${((m.cantidad || 0) * (m.precioUnitario || 0)).toLocaleString("es-ES", { minimumFractionDigits: 2, maximumFractionDigits: 2 })} USD</td>
                    <td data-label="Factura">${m.factura}</td>
                    <td data-label="Usuario">${m.usuario}</td>
                </tr>
            `,
    )
    .join("");
}

function verHistorialProducto(nombre) {
  showPage(
    "productos",
    document.querySelector('#sidebar a[data-page="productos"]'),
  );
  poblarSelectHistorialProducto(nombre);
  showProductoTab(
    "historial",
    document.getElementById("tab-producto-historial"),
  );
}

function eliminarProducto(id) {
  const p = productosData.find((x) => x.id === id);
  if (!p) return;
  if (
    !confirm(
      `Â¿Eliminar el producto "${p.nombre}"? Esta acción no se puede deshacer.`,
    )
  )
    return;
  productosData = productosData.filter((x) => x.id !== id);
  actualizarProductosDisponibles();
  renderProductos();
}

function guardarProducto(e) {
  e.preventDefault();
  const nombre = document.getElementById("producto-nombre").value.trim();
  const descripcion = document
    .getElementById("producto-descripcion")
    .value.trim();
  const categoria = document.getElementById("producto-categoria").value;
  const unidad = document.getElementById("producto-unidad").value;
  const precioEntrada =
    parseFloat(document.getElementById("producto-precio-entrada").value) || 0;
  const precioVenta =
    parseFloat(document.getElementById("producto-precio-venta").value) || 0;
  const stock =
    parseFloat(document.getElementById("producto-stock").value) || 0;
  const tieneCaducidad =
    document.getElementById("producto-caducidad-tipo").value === "fecha";
  const caducidad = tieneCaducidad
    ? document.getElementById("producto-caducidad-fecha").value
    : null;

  if (!nombre || !categoria || !unidad) return false;

  if (editingProductoId) {
    const p = productosData.find((x) => x.id === editingProductoId);
    if (p)
      Object.assign(p, {
        nombre,
        descripcion,
        categoria,
        unidad,
        precioEntrada,
        precioVenta,
        stock,
        caducidad,
      });
  } else {
    productosData.push({
      id: nextProductoId++,
      nombre,
      descripcion,
      categoria,
      unidad,
      precioEntrada,
      precioVenta,
      stock,
      caducidad,
    });
  }

  actualizarProductosDisponibles();
  cancelarProductoForm();
  renderProductos();
  return false;
}

function formatearIdProducto(id) {
  return "#" + String(id).padStart(5, "0");
}

function renderProductos() {
  const body = document.getElementById("productos-body");
  if (!body) return;
  const query = (
    document.getElementById("producto-search")?.value || ""
  ).toLowerCase();
  const rows = productosData.filter(
    (p) =>
      p.nombre.toLowerCase().includes(query) ||
      p.categoria.toLowerCase().includes(query),
  );

  if (rows.length === 0) {
    body.innerHTML =
      '<tr><td colspan="8" class="empty-row">No se encontraron productos que coincidan con la búsqueda.</td></tr>';
    return;
  }

  body.innerHTML = rows
    .map((p) => {
      const valorTotal = (p.precioVenta || 0) * (p.stock || 0);
      return `
                <tr class="cat-row" onclick="verProducto(${p.id})" style="cursor:pointer">
                    <td data-label="ID">${formatearIdProducto(p.id)}</td>
                    <td data-label="Nombre">${p.nombre}</td>
                    <td data-label="Categorí­a">${p.categoria}</td>
                    <td data-label="Unidad">${p.unidad}</td>
                    <td data-label="Precio de Venta" style="text-align:right">${p.precioVenta.toLocaleString("es-ES", { minimumFractionDigits: 2, maximumFractionDigits: 2 })} USD</td>
                    <td data-label="Precio de Entrada" style="text-align:right">${(p.precioEntrada || 0).toLocaleString("es-ES", { minimumFractionDigits: 2, maximumFractionDigits: 2 })} USD</td>
                    <td data-label="Disponible en Stock" style="text-align:center">
                        <span class="stock-badge ${p.stock <= 5 ? "stock-bajo" : ""}">${p.stock}</span>
                    </td>
                    <td data-label="Valor Total" style="text-align:right">${valorTotal.toLocaleString("es-ES", { minimumFractionDigits: 2, maximumFractionDigits: 2 })} USD</td>
                </tr>
            `;
    })
    .join("");
}

// ===== Vista de solo lectura: Ver Información de Producto =====
let productoVerId = null;

function verProducto(id) {
  const p = productosData.find((x) => x.id === id);
  if (!p) return;
  productoVerId = id;
  const facturas = obtenerFacturasProducto(p.nombre);
  const entrada = parseFloat(p.precioEntrada);
  const venta = parseFloat(p.precioVenta);
  const gananciaNum =
    entrada > 0 && !isNaN(venta) ? ((venta - entrada) / entrada) * 100 : null;
  const ganancia = gananciaNum !== null ? gananciaNum.toFixed(1) + "%" : ":";
  const stock = p.stock ?? 0;

  document.getElementById("producto-ver-nombre").textContent = p.nombre || ":";
  document.getElementById("producto-ver-id").textContent = formatearIdProducto(
    p.id,
  );
  document.getElementById("producto-ver-descripcion").textContent =
    p.descripcion || "Sin descripción registrada.";
  document.getElementById("producto-ver-categoria").textContent =
    p.categoria || ":";
  document.getElementById("producto-ver-unidad").textContent = p.unidad || ":";
  document.getElementById("producto-ver-precio-entrada").textContent =
    (p.precioEntrada || 0).toLocaleString("es-ES", {
      minimumFractionDigits: 2,
      maximumFractionDigits: 2,
    }) + " USD";
  document.getElementById("producto-ver-precio-venta").textContent = (
    p.precioVenta || 0
  ).toLocaleString("es-ES", {
    minimumFractionDigits: 2,
    maximumFractionDigits: 2,
  });
  document.getElementById("producto-ver-ganancia").textContent = ganancia;
  document.getElementById("producto-ver-stock").textContent = stock;
  document.getElementById("producto-ver-caducidad").textContent = p.caducidad
    ? formatearFecha(p.caducidad)
    : "No Aplica";
  document.getElementById("producto-ver-factura-compra").textContent = facturas
    .compra.length
    ? facturas.compra.join(", ")
    : "Sin facturas de entrada asociadas";
  document.getElementById("producto-ver-factura-venta").textContent = facturas
    .venta.length
    ? facturas.venta.join(", ")
    : "Sin facturas de venta asociadas";

  // Tono de la tarjeta de % de ganancia (verde si gana, rojo si pierde)
  const gananciaCard = document
    .getElementById("producto-ver-ganancia")
    .closest(".kpi-hero");
  gananciaCard.classList.remove("tone-green", "tone-orange");
  gananciaCard.classList.add(
    gananciaNum !== null && gananciaNum < 0 ? "tone-orange" : "tone-green",
  );

  // Estado del stock: badge + tono de la tarjeta secundaria
  const stockTag = document.getElementById("producto-ver-stock-tag");
  const stockTagTxt = document.getElementById("producto-ver-stock-tag-txt");
  const stockCard = document.getElementById("producto-ver-stock-card");
  stockTag.classList.remove("tone-green", "tone-orange", "tone-red");
  stockCard.classList.remove("tone-green", "tone-orange", "tone-red");
  let stockTono = "tone-green",
    stockTexto = "Disponible";
  if (stock <= 0) {
    stockTono = "tone-red";
    stockTexto = "Agotado";
  } else if (stock <= 5) {
    stockTono = "tone-orange";
    stockTexto = "Stock Bajo";
  }
  stockTag.classList.add(stockTono);
  stockCard.classList.add(stockTono);
  stockTagTxt.textContent = stockTexto;

  // Abrir "Descripción" solo si el producto tiene una; si no, mantenerla cerrada
  document.getElementById("producto-ver-desc-section").open = !!p.descripcion;

  showProductoTab("ver", null);
}

function editarProductoDesdeVer() {
  if (productoVerId != null) editarProducto(productoVerId);
}

function historialProductoDesdeVer() {
  if (productoVerId == null) return;
  const p = productosData.find((x) => x.id === productoVerId);
  if (!p) return;
  verHistorialProducto(p.nombre);
}

function eliminarProductoDesdeVer() {
  if (productoVerId == null) return;
  const idPrevio = productoVerId;
  eliminarProducto(productoVerId);
  if (!productosData.find((p) => p.id === idPrevio)) {
    productoVerId = null;
    showProductoTab("list", document.getElementById("tab-producto-list"));
  }
}

// DATOS DE PRUEBA - PLACEHOLDER
// Estos datos serán reemplazados por llamadas al backend cuando se implemente
// el módulo de clientes.
let clientesData = [
  {
    id: "#2632",
    nombres: "Tatiana",
    apellidos: "Fonseca",
    nombre: "Tatiana Fonseca",
    ci: "90010112345",
    empresa: "Tecnopage",
    cargo: "Gerente de Compras",
    telefono: "+53 50000000",
    correo: "tatiana@tecnopage.com",
    direccion: "",
    fechaEntrada: "",
    infoFactura: "",
  },
  {
    id: "#2633",
    nombres: "Manuel",
    apellidos: "Peña",
    nombre: "Manuel Peña",
    ci: "",
    empresa: "Lulugrafia",
    cargo: "",
    telefono: "+53 50112233",
    correo: "",
    direccion: "",
    fechaEntrada: "",
    infoFactura: "",
  },
  {
    id: "#2634",
    nombres: "Laura",
    apellidos: "Martinez",
    nombre: "Laura Martinez",
    ci: "",
    empresa: "Lulugrafia",
    cargo: "",
    telefono: "+53 50223344",
    correo: "",
    direccion: "",
    fechaEntrada: "",
    infoFactura: "",
  },
  {
    id: "#2635",
    nombres: "Ana",
    apellidos: "Rizo",
    nombre: "Ana Rizo",
    ci: "",
    empresa: "Plastideco",
    cargo: "",
    telefono: "+53 50334455",
    correo: "",
    direccion: "",
    fechaEntrada: "",
    infoFactura: "",
  },
  {
    id: "#2636",
    nombres: "Jose",
    apellidos: "Fernández",
    nombre: "Jose Fernández",
    ci: "",
    empresa: "Plastideco",
    cargo: "",
    telefono: "+53 50445566",
    correo: "",
    direccion: "",
    fechaEntrada: "",
    infoFactura: "",
  },
  {
    id: "#2637",
    nombres: "Enrique",
    apellidos: "Lopez",
    nombre: "Enrique Lopez",
    ci: "",
    empresa: "Tecnopage",
    cargo: "",
    telefono: "+53 50556677",
    correo: "",
    direccion: "",
    fechaEntrada: "",
    infoFactura: "",
  },
  {
    id: "#2638",
    nombres: "Lorenzo",
    apellidos: "Espinosa",
    nombre: "Lorenzo Espinosa",
    ci: "",
    empresa: "Tecnopage",
    cargo: "",
    telefono: "+53 50667788",
    correo: "",
    direccion: "",
    fechaEntrada: "",
    infoFactura: "",
  },
];
let nextClienteSeq = 2639;
let editingClienteId = null;

// =====================================================
// ======================= Proveedores ==================
// DATOS DE PRUEBA - PLACEHOLDER
// Estos datos serán reemplazados por llamadas al backend cuando se implemente
// el módulo de proveedores.
// =====================================================
let proveedoresData = [
  {
    id: "#P-001",
    nombres: "Roberto",
    apellidos: "Suárez",
    nombre: "Roberto Suárez",
    ci: "",
    empresa: "Tecnosum S.A.",
    cargo: "Gerente de Ventas",
    telefono: "+53 52000001",
    correo: "",
    direccion: "",
  },
  {
    id: "#P-002",
    nombres: "Yolanda",
    apellidos: "Reyes",
    nombre: "Yolanda Reyes",
    ci: "",
    empresa: "Distribuidora Caribe",
    cargo: "",
    telefono: "+53 52000002",
    correo: "",
    direccion: "",
  },
  {
    id: "#P-003",
    nombres: "Miguel",
    apellidos: "Torres",
    nombre: "Miguel Torres",
    ci: "",
    empresa: "Importadora del Golfo",
    cargo: "",
    telefono: "+53 52000003",
    correo: "",
    direccion: "",
  },
  {
    id: "#P-004",
    nombres: "Carla",
    apellidos: "Dí­az",
    nombre: "Carla Dí­az",
    ci: "",
    empresa: "ElectroSuministros SRL",
    cargo: "",
    telefono: "+53 52000004",
    correo: "",
    direccion: "",
  },
  {
    id: "#P-005",
    nombres: "Pedro",
    apellidos: "Gómez",
    nombre: "Pedro Gómez",
    ci: "",
    empresa: "Comercial Habana",
    cargo: "",
    telefono: "+53 52000005",
    correo: "",
    direccion: "",
  },
  {
    id: "#P-006",
    nombres: "Sandra",
    apellidos: "Iglesias",
    nombre: "Sandra Iglesias",
    ci: "",
    empresa: "Global Parts Import",
    cargo: "",
    telefono: "+53 52000006",
    correo: "",
    direccion: "",
  },
];
let nextProveedorSeq = 7;
let editingProveedorId = null;
let proveedoresDisponibles = [];

function actualizarProveedoresDisponibles() {
  proveedoresDisponibles = proveedoresData.map((p) => p.empresa || p.nombre);
  poblarDatalists();
}

function showProveedorTab(tab, btn) {
  document
    .querySelectorAll("#page-proveedores .config-subview")
    .forEach((v) => v.classList.remove("active"));
  document.getElementById("proveedor-" + tab).classList.add("active");
  document
    .querySelectorAll("#page-proveedores .view-tabs .tab")
    .forEach((t) => t.classList.remove("active"));
  if (btn) btn.classList.add("active");
  document.getElementById("proveedor-search-bar").style.display =
    tab === "list" ? "flex" : "none";
  if (tab === "create" && !editingProveedorId) cancelarProveedorForm(false);
  if (tab === "historial") {
    poblarSelectHistorialProveedor();
    renderHistorialProveedor();
  }
}

function cancelarProveedorForm(volverALista = true) {
  editingProveedorId = null;
  document.getElementById("proveedor-form-title").textContent =
    "Datos de Proveedor";
  [
    "nombres",
    "apellidos",
    "ci",
    "empresa",
    "cargo",
    "telefono",
    "correo",
    "direccion",
  ].forEach((campo) => {
    const el = document.getElementById("proveedor-" + campo);
    if (el) el.value = "";
  });
  document.getElementById("proveedor-id-sistema").value = "";
  if (volverALista)
    showProveedorTab("list", document.getElementById("tab-proveedor-list"));
}

function editarProveedor(id) {
  const p = proveedoresData.find((pr) => pr.id === id);
  if (!p) return;
  editingProveedorId = id;
  document.getElementById("proveedor-form-title").textContent =
    "Editar Proveedor: " + p.nombre;
  document.getElementById("proveedor-id-sistema").value = p.id || "";
  document.getElementById("proveedor-nombres").value = p.nombres || "";
  document.getElementById("proveedor-apellidos").value = p.apellidos || "";
  document.getElementById("proveedor-ci").value = p.ci || "";
  document.getElementById("proveedor-empresa").value = p.empresa || "";
  document.getElementById("proveedor-cargo").value = p.cargo || "";
  document.getElementById("proveedor-telefono").value = p.telefono || "";
  document.getElementById("proveedor-correo").value = p.correo || "";
  document.getElementById("proveedor-direccion").value = p.direccion || "";
  showProveedorTab("create", document.getElementById("tab-proveedor-create"));
}

function guardarProveedor(e) {
  e.preventDefault();
  const nombres = document.getElementById("proveedor-nombres").value.trim();
  const apellidos = document.getElementById("proveedor-apellidos").value.trim();
  const ci = document.getElementById("proveedor-ci").value.trim();
  const telefono = document.getElementById("proveedor-telefono").value.trim();
  if (!nombres || !apellidos || !ci || !telefono) return false;

  const datos = {
    nombres,
    apellidos,
    nombre: nombres + " " + apellidos,
    ci,
    empresa: document.getElementById("proveedor-empresa").value.trim(),
    cargo: document.getElementById("proveedor-cargo").value.trim(),
    telefono,
    correo: document.getElementById("proveedor-correo").value.trim(),
    direccion: document.getElementById("proveedor-direccion").value.trim(),
  };

  if (editingProveedorId) {
    const p = proveedoresData.find((pr) => pr.id === editingProveedorId);
    if (p) Object.assign(p, datos);
  } else {
    proveedoresData.push(
      Object.assign(
        { id: "#P-" + String(nextProveedorSeq++).padStart(3, "0") },
        datos,
      ),
    );
  }
  actualizarProveedoresDisponibles();
  cancelarProveedorForm();
  renderProveedores();
  return false;
}

function renderProveedores() {
  const body = document.getElementById("proveedores-body");
  if (!body) return;
  const query = (
    document.getElementById("proveedor-search")?.value || ""
  ).toLowerCase();
  const rows = proveedoresData.filter(
    (p) =>
      p.nombre.toLowerCase().includes(query) ||
      (p.empresa || "").toLowerCase().includes(query) ||
      p.id.toLowerCase().includes(query),
  );

  if (rows.length === 0) {
    body.innerHTML =
      '<tr><td colspan="5" class="empty-row">No se encontraron proveedores que coincidan con la búsqueda.</td></tr>';
    return;
  }

  body.innerHTML = rows
    .map(
      (p) => `
                <tr class="cat-row" onclick="verProveedor('${p.id}')" style="cursor:pointer">
                    <td data-label="ID">${p.id}</td>
                    <td data-label="Nombre">${p.nombre}</td>
                    <td data-label="Empresa">${p.empresa || ""}</td>
                    <td data-label="Teléfono">${p.telefono}</td>
                    <td>
                        <div class="col-acciones">
                            <button class="action-link" onclick="event.stopPropagation(); editarProveedor('${p.id}')">
                                <svg class="icon" style="width:16px;height:16px"><use href="#icon-edit"></use></svg>
                                Editar
                            </button>
                            <button class="action-link" onclick="event.stopPropagation(); verHistorialProveedor('${p.empresa ? p.empresa.replace(/'/g, "\\'") : p.id}')">
                                <svg class="icon" style="width:16px;height:16px"><use href="#icon-history"></use></svg>
                                Historial
                            </button>
                            <button class="action-link danger" onclick="event.stopPropagation(); eliminarProveedor('${p.id}')">
                                <svg class="icon" style="width:16px;height:16px"><use href="#icon-trash"></use></svg>
                                Eliminar
                            </button>
                        </div>
                    </td>
                </tr>
            `,
    )
    .join("");
}

function eliminarProveedor(id) {
  const p = proveedoresData.find((pr) => pr.id === id);
  if (!p) return;
  if (
    !confirm(
      `Â¿Eliminar al proveedor "${p.nombre}"? Esta acción no se puede deshacer.`,
    )
  )
    return;
  proveedoresData = proveedoresData.filter((pr) => pr.id !== id);
  actualizarProveedoresDisponibles();
  if (proveedorVerId === id) proveedorVerId = null;
  renderProveedores();
}

// ===== Vista de solo lectura: Ver Información de Proveedor =====
let proveedorVerId = null;

function calcularResumenFacturasProveedor(nombreEmpresaProveedor) {
  const facturas = facturasEntradaData.filter(
    (f) => f.proveedor === nombreEmpresaProveedor,
  );
  const total = facturas.reduce((sum, f) => sum + (f.total || 0), 0);
  const pendiente = facturas
    .filter((f) => f.estado === "Por Pagar")
    .reduce((sum, f) => sum + (f.total || 0), 0);
  const ordenadas = [...facturas].sort(
    (a, b) => new Date(b.fechaEmision) - new Date(a.fechaEmision),
  );
  const ultimaCompra = ordenadas.length ? ordenadas[0].fechaEmision : null;

  return { facturas: ordenadas, total, pendiente, ultimaCompra };
}

function verProveedor(id) {
  const p = proveedoresData.find((pr) => pr.id === id);
  if (!p) return;
  proveedorVerId = id;
  const resumen = calcularResumenFacturasProveedor(p.empresa);

  document.getElementById("proveedor-ver-id").textContent = p.id || ":";
  document.getElementById("proveedor-ver-nombre-completo").textContent =
    p.nombre || ":";
  document.getElementById("proveedor-ver-ci").textContent =
    p.ci || "No registrado";
  document.getElementById("proveedor-ver-direccion").textContent =
    p.direccion || "No registrada";

  document.getElementById("proveedor-ver-empresa").textContent =
    p.empresa || "Sin empresa";
  document.getElementById("proveedor-ver-cargo").textContent =
    p.cargo || "Sin cargo registrado";
  document.getElementById("proveedor-ver-cargo-tag").style.display = p.cargo
    ? "inline-flex"
    : "none";
  document.getElementById("proveedor-ver-empresa-cargo").textContent =
    [p.empresa, p.cargo].filter(Boolean).join(" — ") || "No registrado";

  const telChip = document.getElementById("proveedor-ver-telefono-chip");
  document.getElementById("proveedor-ver-telefono").textContent =
    p.telefono || "No registrado";
  telChip.href = p.telefono ? "tel:" + p.telefono.replace(/\s+/g, "") : "#";
  telChip.classList.toggle("is-empty", !p.telefono);

  const correoChip = document.getElementById("proveedor-ver-correo-chip");
  document.getElementById("proveedor-ver-correo").textContent =
    p.correo || "No registrado";
  correoChip.href = p.correo ? "mailto:" + p.correo : "#";
  correoChip.classList.toggle("is-empty", !p.correo);

  // Resumen ejecutivo de compras
  document.getElementById("proveedor-ver-total-comprado").textContent =
    resumen.total.toLocaleString("es-ES", {
      minimumFractionDigits: 2,
      maximumFractionDigits: 2,
    });
  document.getElementById("proveedor-ver-saldo-pendiente").textContent =
    resumen.pendiente.toLocaleString("es-ES", {
      minimumFractionDigits: 2,
      maximumFractionDigits: 2,
    });
  document.getElementById("proveedor-ver-num-facturas").textContent =
    resumen.facturas.length;
  document.getElementById("proveedor-ver-ultima-compra").textContent =
    resumen.ultimaCompra ? formatearFecha(resumen.ultimaCompra) : "Sin compras";

  const saldoCard = document.getElementById("proveedor-ver-saldo-card");
  saldoCard.classList.remove("tone-green", "tone-orange");
  saldoCard.classList.add(resumen.pendiente > 0 ? "tone-orange" : "tone-green");

  // Historial de facturas de entrada de este proveedor
  const histEl = document.getElementById("proveedor-ver-historial-facturas");
  if (resumen.facturas.length === 0) {
    histEl.innerHTML =
      '<span class="pv-empty-note">Este proveedor aún no tiene facturas de entrada registradas.</span>';
  } else {
    const toneEstado = (estado) =>
      estado === "Pagada" ? "tone-green" : "tone-orange";
    histEl.innerHTML = resumen.facturas
      .map(
        (f) => `
                    <div class="pv-invoice-row">
                        <div class="pv-invoice-row-main">
                            <span class="pv-invoice-row-num">${f.numero}</span>
                            <span class="pv-invoice-row-fecha">${formatearFecha(f.fechaEmision)}</span>
                        </div>
                        <div class="pv-invoice-row-right">
                            <span class="kpi-tag ${toneEstado(f.estado)}">${f.estado}</span>
                            <span class="pv-invoice-row-total">${(f.total || 0).toLocaleString("es-ES", { minimumFractionDigits: 2, maximumFractionDigits: 2 })} USD</span>
                        </div>
                    </div>
                `,
      )
      .join("");
  }

  showProveedorTab("ver", null);
}

function editarProveedorDesdeVer() {
  if (proveedorVerId != null) editarProveedor(proveedorVerId);
}

function eliminarProveedorDesdeVer() {
  if (proveedorVerId == null) return;
  const idPrevio = proveedorVerId;
  eliminarProveedor(proveedorVerId);
  if (!proveedoresData.find((pr) => pr.id === idPrevio)) {
    proveedorVerId = null;
    showProveedorTab("list", document.getElementById("tab-proveedor-list"));
  }
}

function irCrearProveedor() {
  showPage(
    "proveedores",
    document.querySelector('#sidebar a[data-page="proveedores"]'),
  );
  showProveedorTab("create", document.getElementById("tab-proveedor-create"));
}

// ===== Historial de compras de un Proveedor =====
function poblarSelectHistorialProveedor(claveSeleccionada) {
  const select = document.getElementById("historial-proveedor-select");
  if (!select) return;
  const valorPrevio = claveSeleccionada || select.value;
  select.innerHTML =
    '<option value="">Selecciona un proveedor...</option>' +
    proveedoresData
      .map((p) => {
        const clave = p.empresa || p.id;
        return `<option value="${clave.replace(/"/g, "&quot;")}">${p.empresa || p.nombre}</option>`;
      })
      .join("");
  if (valorPrevio) select.value = valorPrevio;
}

function obtenerMovimientosProveedor(claveProveedor) {
  const clave = (claveProveedor || "").trim().toLowerCase();
  return facturasEntradaData
    .filter((f) => (f.proveedor || "").trim().toLowerCase() === clave)
    .map((f) => {
      const cantidadProductos =
        f.lineas && f.lineas.length
          ? f.lineas.reduce((s, li) => s + li.cantidad, 0)
          : (f.productos || []).length;
      return {
        fecha: f.fechaEmision,
        estado: f.estado,
        cantidadProductos,
        precioUnitario: cantidadProductos ? f.total / cantidadProductos : 0,
        precioTotal: f.total,
        factura: f.numero,
        responsable: f.responsable || ":",
      };
    })
    .sort((a, b) => (b.fecha || "").localeCompare(a.fecha || ""));
}

function renderHistorialProveedor() {
  const body = document.getElementById("historial-proveedor-body");
  if (!body) return;
  const claveProveedor =
    document.getElementById("historial-proveedor-select")?.value || "";
  const desde =
    document.getElementById("historial-proveedor-desde")?.value || "";
  const hasta =
    document.getElementById("historial-proveedor-hasta")?.value || "";

  if (!claveProveedor) {
    body.innerHTML = "";
    document.getElementById("historial-proveedor-list").style.display = "none";
    if (!document.getElementById("historial-proveedor-vacio")) {
      document
        .getElementById("historial-proveedor-list")
        .insertAdjacentHTML(
          "afterend",
          '<div id="historial-proveedor-vacio" class="historial-vacio-selector">Selecciona un proveedor para consultar su historial de compras.</div>',
        );
    }
    return;
  }
  document.getElementById("historial-proveedor-list").style.display = "";
  document.getElementById("historial-proveedor-vacio")?.remove();

  const filasFiltradas = obtenerMovimientosProveedor(claveProveedor).filter(
    (m) => {
      const cumpleDesde = !desde || (m.fecha || "") >= desde;
      const cumpleHasta = !hasta || (m.fecha || "") <= hasta;
      return cumpleDesde && cumpleHasta;
    },
  );
  const filas = ordenarPorFecha(filasFiltradas, "historialProveedor", "fecha");

  if (filas.length === 0) {
    body.innerHTML =
      '<tr><td colspan="7" class="empty-row">No hay compras registradas a este proveedor en el perí­odo seleccionado.</td></tr>';
    return;
  }

  const estadoClase = {
    "Por Pagar": "status-pendiente",
    Pagada: "status-pagada",
  };
  body.innerHTML = filas
    .map(
      (m) => `
                <tr>
                    <td data-label="Fecha">${formatearFecha(m.fecha)}</td>
                    <td data-label="Estado"><span class="status-badge ${estadoClase[m.estado] || ""}">${m.estado}</span></td>
                    <td data-label="Cantidad Productos" style="text-align:center">${m.cantidadProductos}</td>
                    <td data-label="Precio Unitario" style="text-align:right">${m.precioUnitario.toLocaleString("es-ES", { minimumFractionDigits: 2, maximumFractionDigits: 2 })} USD</td>
                    <td data-label="Precio Total" style="text-align:right">${m.precioTotal.toLocaleString("es-ES", { minimumFractionDigits: 2, maximumFractionDigits: 2 })} USD</td>
                    <td data-label="Factura">${m.factura}</td>
                    <td data-label="Responsable">${m.responsable}</td>
                </tr>
            `,
    )
    .join("");
}

function verHistorialProveedor(clave) {
  showPage(
    "proveedores",
    document.querySelector('#sidebar a[data-page="proveedores"]'),
  );
  poblarSelectHistorialProveedor(clave);
  showProveedorTab(
    "historial",
    document.getElementById("tab-proveedor-historial"),
  );
}

// DATOS DE PRUEBA - PLACEHOLDER
// Estos datos serán reemplazados por llamadas al backend cuando se implemente
// el módulo de facturas de entrada.
let facturasEntradaData = [
  {
    numero: "E1.001",
    proveedor: "Tecnosum S.A.",
    fechaEmision: "2027-01-01",
    total: 1250,
    subtotal: 1250,
    descuentoPct: 0,
    impuestoPct: 0,
    observaciones: "",
    descripcion: "",
    archivoNombre: null,
    estado: "Por Pagar",
    metodoPago: "Efectivo USD",
    productos: ["Laptop HP 15", "Mouse Inalámbrico"],
    responsable: "Amanda Hernández",
    lineas: [
      { producto: "Laptop HP 15", cantidad: 3, unidad: "Unidad", costo: 380 },
      {
        producto: "Mouse Inalámbrico",
        cantidad: 10,
        unidad: "Unidad",
        costo: 8,
      },
    ],
  },
  {
    numero: "E1.002",
    proveedor: "Distribuidora Caribe",
    fechaEmision: "2027-01-04",
    total: 830.5,
    subtotal: 830.5,
    descuentoPct: 0,
    impuestoPct: 0,
    observaciones: "",
    descripcion: "",
    archivoNombre: null,
    estado: "Pagada",
    metodoPago: "Efectivo USD",
    productos: ["Teclado Mecánico"],
    responsable: "Raúl Gómez",
    lineas: [
      {
        producto: "Teclado Mecánico",
        cantidad: 25,
        unidad: "Unidad",
        costo: 22,
      },
    ],
  },
  {
    numero: "E1.003",
    proveedor: "Importadora del Golfo",
    fechaEmision: "2026-12-10",
    total: 2140,
    subtotal: 2140,
    descuentoPct: 0,
    impuestoPct: 0,
    observaciones: "",
    descripcion: "",
    archivoNombre: null,
    estado: "Por Pagar",
    metodoPago: "Efectivo USD",
    productos: ['Monitor 24"', "Cámara Web"],
    responsable: "Amanda Hernández",
    lineas: [
      { producto: 'Monitor 24"', cantidad: 15, unidad: "Unidad", costo: 90 },
      { producto: "Cámara Web", cantidad: 30, unidad: "Unidad", costo: 14 },
    ],
  },
];
let nextFacturaEntradaSeq = 4;
let editingFacturaId = null;
let facturaLineItems = [];
let nextLineItemId = 1;
let facturaArchivoActual = null;

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
// =================== Renderizado ====================
// =====================================================
function renderCategorias() {
  const body = document.getElementById("categorias-body");
  if (!body) return;
  const query = (
    document.getElementById("categoria-search")?.value || ""
  ).toLowerCase();
  const rows = categoriasData.filter(
    (c) =>
      c.nombre.toLowerCase().includes(query) ||
      c.descripcion.toLowerCase().includes(query),
  );

  if (rows.length === 0) {
    body.innerHTML =
      '<tr><td colspan="5" class="empty-row">No se encontraron categorí­as que coincidan con la búsqueda.</td></tr>';
    return;
  }

  body.innerHTML = rows
    .map(
      (c) => `
                <tr class="cat-row" onclick="verCategoria(${c.id})" style="cursor:pointer">
                    <td data-label="Nombre">${c.nombre}</td>
                    <td data-label="Descripción">${c.descripcion}</td>
                    <td data-label="Cantidad" style="text-align:center">${c.cantidad}</td>
                    <td data-label="Ingresos" style="text-align:center">${c.ingresos}</td>
                    <td>
                        <div class="col-acciones">
                            <button class="action-link" onclick="event.stopPropagation(); editarCategoria(${c.id})">
                                <svg class="icon" style="width:16px;height:16px"><use href="#icon-edit"></use></svg>
                                Editar
                            </button>
                            <button class="action-link danger" onclick="event.stopPropagation(); eliminarCategoria(${c.id})">
                                <svg class="icon" style="width:16px;height:16px"><use href="#icon-trash"></use></svg>
                                Eliminar
                            </button>
                        </div>
                    </td>
                </tr>
            `,
    )
    .join("");
}

function renderProductPicker(seleccionados) {
  const picker = document.getElementById("categoria-productos-picker");
  if (!picker) return;
  if (!seleccionados || seleccionados.length === 0)
    seleccionados = [productosDisponibles[0]];
  picker.innerHTML = seleccionados
    .map(
      (prod, i) => `
                <div class="picker-row">
                    <select onchange="void(0)">
                        ${productosDisponibles.map((p) => `<option ${p === prod ? "selected" : ""}>${p}</option>`).join("")}
                    </select>
                    <button type="button" class="remove-btn" onclick="quitarProductoPicker(${i})" aria-label="Quitar producto">
                        <svg class="icon" style="width:16px;height:16px"><use href="#icon-close"></use></svg>
                    </button>
                </div>
            `,
    )
    .join("");
}

function agregarProductoPicker() {
  const picker = document.getElementById("categoria-productos-picker");
  const actuales = Array.from(picker.querySelectorAll("select")).map(
    (s) => s.value,
  );
  actuales.push(productosDisponibles[0]);
  renderProductPicker(actuales);
}

function quitarProductoPicker(index) {
  const picker = document.getElementById("categoria-productos-picker");
  const actuales = Array.from(picker.querySelectorAll("select")).map(
    (s) => s.value,
  );
  actuales.splice(index, 1);
  renderProductPicker(actuales);
}

function editarCategoria(id) {
  const cat = categoriasData.find((c) => c.id === id);
  if (!cat) return;
  editingCategoriaId = id;
  document.getElementById("categoria-form-title").textContent =
    "Editar Categorí­a: " + cat.nombre;
  document.getElementById("categoria-nombre").value = cat.nombre;
  document.getElementById("categoria-descripcion").value = cat.descripcion;
  renderProductPicker([]);
  showCategoriaTab("create", document.getElementById("tab-categoria-create"));
}

function eliminarCategoria(id) {
  const cat = categoriasData.find((c) => c.id === id);
  if (!cat) return;
  if (
    !confirm(
      `Â¿Eliminar la categorí­a "${cat.nombre}"? Esta acción no se puede deshacer.`,
    )
  )
    return;
  categoriasData = categoriasData.filter((c) => c.id !== id);
  renderCategorias();
}

function guardarCategoria(e) {
  e.preventDefault();
  const nombre = document.getElementById("categoria-nombre").value.trim();
  const descripcion = document
    .getElementById("categoria-descripcion")
    .value.trim();
  if (!nombre || !descripcion) return false;

  if (editingCategoriaId) {
    const cat = categoriasData.find((c) => c.id === editingCategoriaId);
    if (cat) {
      cat.nombre = nombre;
      cat.descripcion = descripcion;
    }
  } else {
    categoriasData.push({
      id: nextCategoriaId++,
      nombre,
      descripcion,
      cantidad: 0,
      ingresos: "0",
    });
  }
  cancelarCategoriaForm();
  renderCategorias();
  return false;
}

function renderClientes() {
  const body = document.getElementById("clientes-body");
  if (!body) return;
  const query = (
    document.getElementById("cliente-search")?.value || ""
  ).toLowerCase();
  const rows = clientesData.filter(
    (c) =>
      c.nombre.toLowerCase().includes(query) ||
      c.empresa.toLowerCase().includes(query) ||
      c.id.toLowerCase().includes(query),
  );

  if (rows.length === 0) {
    body.innerHTML =
      '<tr><td colspan="5" class="empty-row">No se encontraron clientes que coincidan con la búsqueda.</td></tr>';
    return;
  }

  body.innerHTML = rows
    .map(
      (c) => `
                <tr class="cat-row" onclick="verCliente('${c.id}')" style="cursor:pointer">
                    <td data-label="ID">${c.id}</td>
                    <td data-label="Nombre">${c.nombre}</td>
                    <td data-label="Empresa">${c.empresa}</td>
                    <td data-label="Teléfono">${c.telefono}</td>
                    <td>
                        <div class="col-acciones">
                            <button class="action-link" onclick="event.stopPropagation(); editarCliente('${c.id}')">
                                <svg class="icon" style="width:16px;height:16px"><use href="#icon-edit"></use></svg>
                                Editar
                            </button>
                            <button class="action-link danger" onclick="event.stopPropagation(); eliminarCliente('${c.id}')">
                                <svg class="icon" style="width:16px;height:16px"><use href="#icon-trash"></use></svg>
                                Eliminar
                            </button>
                        </div>
                    </td>
                </tr>
            `,
    )
    .join("");
}

function eliminarCliente(id) {
  const cli = clientesData.find((c) => c.id === id);
  if (!cli) return;
  if (
    !confirm(
      `Â¿Eliminar al cliente "${cli.nombre}"? Esta acción no se puede deshacer.`,
    )
  )
    return;
  const idx = clientesData.findIndex((c) => c.id === id);
  clientesData.splice(idx, 1);
  if (clienteVerId === id) clienteVerId = null;
  renderClientes();
}

// ===== Vista de solo lectura: Ver Información de Cliente =====
let clienteVerId = null;

function calcularResumenFacturasCliente(nombreCliente) {
  const facturas = [
    ...facturasVentaData.map((f) => ({ ...f, origen: "Venta" })),
    ...facturasServiciosData.map((f) => ({ ...f, origen: "Servicio" })),
  ].filter((f) => f.cliente === nombreCliente);

  const total = facturas.reduce((sum, f) => sum + (f.total || 0), 0);
  const pendiente = facturas
    .filter((f) => f.estado === "Por Pagar")
    .reduce((sum, f) => sum + (f.total || 0), 0);
  const ordenadas = [...facturas].sort(
    (a, b) => new Date(b.fechaEmision) - new Date(a.fechaEmision),
  );
  const ultimaCompra = ordenadas.length ? ordenadas[0].fechaEmision : null;

  return { facturas: ordenadas, total, pendiente, ultimaCompra };
}

function verCliente(id) {
  const c = clientesData.find((cl) => cl.id === id);
  if (!c) return;
  clienteVerId = id;
  const resumen = calcularResumenFacturasCliente(c.nombre);

  document.getElementById("cliente-ver-id").textContent = c.id || ":";
  document.getElementById("cliente-ver-nombre-completo").textContent =
    c.nombre || ":";
  document.getElementById("cliente-ver-ci").textContent =
    c.ci || "No registrado";
  document.getElementById("cliente-ver-direccion").textContent =
    c.direccion || "No registrada";
  document.getElementById("cliente-ver-fecha-entrada").textContent =
    c.fechaEntrada ? formatearFecha(c.fechaEntrada) : "No registrada";

  // Empresa / cargo: badges del encabezado + texto combinado en Perfil
  document.getElementById("cliente-ver-empresa").textContent =
    c.empresa || "Sin empresa";
  document.getElementById("cliente-ver-cargo").textContent =
    c.cargo || "Sin cargo registrado";
  document.getElementById("cliente-ver-cargo-tag").style.display = c.cargo
    ? "inline-flex"
    : "none";
  document.getElementById("cliente-ver-empresa-cargo").textContent =
    [c.empresa, c.cargo].filter(Boolean).join(" — ") || "No registrado";

  // Contacto directo: chips clicables (tel:/mailto:), deshabilitados si no hay dato
  const telChip = document.getElementById("cliente-ver-telefono-chip");
  document.getElementById("cliente-ver-telefono").textContent =
    c.telefono || "No registrado";
  telChip.href = c.telefono ? "tel:" + c.telefono.replace(/\s+/g, "") : "#";
  telChip.classList.toggle("is-empty", !c.telefono);

  const correoChip = document.getElementById("cliente-ver-correo-chip");
  document.getElementById("cliente-ver-correo").textContent =
    c.correo || "No registrado";
  correoChip.href = c.correo ? "mailto:" + c.correo : "#";
  correoChip.classList.toggle("is-empty", !c.correo);

  // Resumen ejecutivo de facturación
  document.getElementById("cliente-ver-total-facturado").textContent =
    resumen.total.toLocaleString("es-ES", {
      minimumFractionDigits: 2,
      maximumFractionDigits: 2,
    });
  document.getElementById("cliente-ver-saldo-pendiente").textContent =
    resumen.pendiente.toLocaleString("es-ES", {
      minimumFractionDigits: 2,
      maximumFractionDigits: 2,
    });
  document.getElementById("cliente-ver-num-facturas").textContent =
    resumen.facturas.length;
  document.getElementById("cliente-ver-ultima-compra").textContent =
    resumen.ultimaCompra ? formatearFecha(resumen.ultimaCompra) : "Sin compras";

  // Tono de la tarjeta de saldo: verde si está al día, rojo si tiene deuda
  const saldoCard = document.getElementById("cliente-ver-saldo-card");
  saldoCard.classList.remove("tone-green", "tone-orange");
  saldoCard.classList.add(resumen.pendiente > 0 ? "tone-orange" : "tone-green");

  // Nota legada de "Información de Factura" (texto libre), si existe
  const notaEl = document.getElementById("cliente-ver-info-factura-nota");
  notaEl.textContent = c.infoFactura || "";
  notaEl.style.display = c.infoFactura ? "block" : "none";

  // Historial de facturas (ventas + servicios) de este cliente
  const histEl = document.getElementById("cliente-ver-historial-facturas");
  if (resumen.facturas.length === 0) {
    histEl.innerHTML =
      '<span class="pv-empty-note">Este cliente aún no tiene facturas de venta o servicio registradas.</span>';
  } else {
    histEl.innerHTML = resumen.facturas
      .map(
        (f) => `
                    <div class="pv-invoice-row">
                        <div class="pv-invoice-row-main">
                            <span class="pv-invoice-row-num">${f.numero} · ${f.origen}</span>
                            <span class="pv-invoice-row-fecha">${formatearFecha(f.fechaEmision)}</span>
                        </div>
                        <div class="pv-invoice-row-right">
                            <span class="kpi-tag ${f.estado === "Pagada" ? "tone-green" : "tone-orange"}">${f.estado}</span>
                            <span class="pv-invoice-row-total">${(f.total || 0).toLocaleString("es-ES", { minimumFractionDigits: 2, maximumFractionDigits: 2 })} USD</span>
                        </div>
                    </div>
                `,
      )
      .join("");
  }

  showClienteTab("ver", null);
}

function editarClienteDesdeVer() {
  if (clienteVerId != null) editarCliente(clienteVerId);
}

function eliminarClienteDesdeVer() {
  if (clienteVerId == null) return;
  eliminarCliente(clienteVerId);
  if (clienteVerId == null)
    showClienteTab("list", document.getElementById("tab-cliente-list"));
}

// =====================================================
// ======================= Usuarios ====================
// =====================================================

// =====================================================
// ================ Facturas de Entrada ================
// =====================================================
function poblarDatalists() {
  const dlProv = document.getElementById("proveedores-datalist");
  if (dlProv)
    dlProv.innerHTML = proveedoresDisponibles
      .map((p) => `<option value="${p}"></option>`)
      .join("");
  const dlProd = document.getElementById("productos-datalist");
  if (dlProd)
    dlProd.innerHTML = productosDisponibles
      .map((p) => `<option value="${p}"></option>`)
      .join("");
  const dlCli = document.getElementById("clientes-datalist");
  if (dlCli)
    dlCli.innerHTML = clientesData
      .map(
        (c) =>
          `<option value="${c.nombre}">${c.empresa ? c.empresa : ""}</option>`,
      )
      .join("");
  const dlIns = document.getElementById("insumos-datalist");
  if (dlIns)
    dlIns.innerHTML = insumosDisponibles
      .map((p) => `<option value="${p}"></option>`)
      .join("");
}

function renderFacturasEntrada() {
  const body = document.getElementById("facturas-entrada-body");
  if (!body) return;
  const query = (
    document.getElementById("factura-entrada-search")?.value || ""
  ).toLowerCase();
  const rows = ordenarPorFecha(
    facturasEntradaData.filter(
      (f) =>
        f.proveedor.toLowerCase().includes(query) ||
        f.numero.toLowerCase().includes(query),
    ),
    "facturasEntrada",
    "fechaEmision",
  );

  if (rows.length === 0) {
    body.innerHTML =
      '<tr><td colspan="6" class="empty-row">No se encontraron facturas que coincidan con la búsqueda.</td></tr>';
    return;
  }

  const estadoClase = {
    "Por Pagar": "status-pendiente",
    Pagada: "status-pagada",
  };
  body.innerHTML = rows
    .map(
      (f) => `
                <tr class="cat-row" onclick="verFactura('${f.numero}')" style="cursor:pointer">
                    <td data-label="No. Factura">${f.numero}</td>
                    <td data-label="Proveedor">${f.proveedor}</td>
                    <td data-label="Emisión">${formatearFecha(f.fechaEmision)}</td>
                    <td data-label="Total" style="text-align:right">${f.total.toLocaleString("es-ES", { minimumFractionDigits: 2, maximumFractionDigits: 2 })} USD</td>
                    <td data-label="Estado"><span class="status-badge ${estadoClase[f.estado] || ""}">${f.estado}</span></td>
                    <td>
                        <div class="col-acciones">
                            <button class="action-link" onclick="event.stopPropagation(); alert('La edición de facturas ya guardadas estará disponible próximamente.')">
                                <svg class="icon" style="width:16px;height:16px"><use href="#icon-edit"></use></svg>
                                Editar
                            </button>
                            <button class="action-link danger" onclick="event.stopPropagation(); eliminarFactura('${f.numero}')">
                                <svg class="icon" style="width:16px;height:16px"><use href="#icon-trash"></use></svg>
                                Eliminar
                            </button>
                        </div>
                    </td>
                </tr>
            `,
    )
    .join("");
}

function formatearFecha(iso) {
  if (!iso) return ":";
  const [y, m, d] = iso.split("-");
  return `${d}/${m}/${y}`;
}

function eliminarFactura(numero) {
  const f = facturasEntradaData.find((x) => x.numero === numero);
  if (!f) return;
  if (
    !confirm(
      `Â¿Eliminar la factura "${f.numero}" de ${f.proveedor}? Esta acción no se puede deshacer.`,
    )
  )
    return;
  facturasEntradaData = facturasEntradaData.filter((x) => x.numero !== numero);
  renderFacturasEntrada();
}

function showFacturaEntradaTab(tab, btn) {
  document
    .querySelectorAll("#page-facturas-entrada .config-subview")
    .forEach((v) => v.classList.remove("active"));
  document.getElementById("factura-entrada-" + tab).classList.add("active");
  document
    .querySelectorAll("#page-facturas-entrada .view-tabs .tab")
    .forEach((t) => t.classList.remove("active"));
  if (btn) btn.classList.add("active");
  document.getElementById("factura-entrada-search-bar").style.display =
    tab === "list" ? "flex" : "none";
  if (tab === "create" && !editingFacturaId) prepararNuevaFactura();
}

let facturaVerNumero = null;

function verFactura(numero) {
  const f = facturasEntradaData.find((x) => x.numero === numero);
  if (!f) return;
  facturaVerNumero = numero;

  document.getElementById("factura-ver-titulo").textContent =
    "Factura de Entrada " + f.numero;
  document.getElementById("factura-ver-numero-badge").textContent = f.numero;
  document.getElementById("factura-ver-proveedor").textContent = f.proveedor;
  document.getElementById("factura-ver-numero").textContent = f.numero;
  document.getElementById("factura-ver-emision").textContent = formatearFecha(
    f.fechaEmision,
  );
  const estadoTono = { "Por Pagar": "tone-orange", Pagada: "tone-green" };
  document.getElementById("factura-ver-estado-txt").textContent = f.estado;
  document.getElementById("factura-ver-estado-hero").textContent = f.estado;
  document
    .getElementById("factura-ver-estado-tag")
    .classList.remove("tone-orange", "tone-green", "tone-red");
  document
    .getElementById("factura-ver-estado-tag")
    .classList.add(estadoTono[f.estado] || "tone-orange");
  const estadoCard = document.getElementById("factura-ver-estado-card");
  estadoCard.classList.remove("tone-orange", "tone-green", "tone-red");
  estadoCard.classList.add(estadoTono[f.estado] || "tone-orange");
  document.getElementById("factura-ver-metodo").textContent =
    f.metodoPago || ":";
  document.getElementById("factura-ver-observaciones").textContent =
    f.observaciones || ":";
  document.getElementById("factura-ver-descripcion").textContent =
    f.descripcion || ":";
  document.getElementById("factura-ver-archivo").textContent =
    f.archivoNombre || `Factura de Compra ${f.numero}.PDF`;

  const cuerpo = document.getElementById("factura-ver-productos");
  cuerpo.innerHTML = (f.lineas || [])
    .map(
      (li, i) => `
                <div class="li-row li-body li-body-readonly">
                    <div>${i + 1}</div>
                    <div>${li.producto}</div>
                    <div>${li.cantidad}</div>
                    <div>${li.unidad || "Unidad"}</div>
                    <div class="li-costo">$ ${li.costo.toLocaleString("es-ES", { minimumFractionDigits: 2, maximumFractionDigits: 2 })} USD</div>
                    <div class="li-subtotal">${(li.cantidad * li.costo).toLocaleString("es-ES", { minimumFractionDigits: 2, maximumFractionDigits: 2 })} USD</div>
                </div>
            `,
    )
    .join("");

  const fmt = (n) =>
    n.toLocaleString("es-ES", {
      minimumFractionDigits: 2,
      maximumFractionDigits: 2,
    }) + " USD";
  const descuentoMonto = f.subtotal * ((f.descuentoPct || 0) / 100);
  const impuestoMonto =
    (f.subtotal - descuentoMonto) * ((f.impuestoPct || 0) / 100);
  document.getElementById("factura-ver-subtotal").textContent = fmt(f.subtotal);
  document.getElementById("factura-ver-subtotal-resumen").textContent = fmt(
    f.subtotal,
  );
  document.getElementById("factura-ver-descuento").textContent =
    fmt(descuentoMonto) + ` (${f.descuentoPct || 0}%)`;
  document.getElementById("factura-ver-impuestos").textContent =
    fmt(impuestoMonto) + ` (${f.impuestoPct || 0}%)`;
  document.getElementById("factura-ver-total").textContent = fmt(f.total);
  document.getElementById("factura-ver-total-resumen").textContent = fmt(
    f.total,
  );

  showFacturaEntradaTab("ver", null);
}

function eliminarFacturaDesdeVer() {
  if (!facturaVerNumero) return;
  const numero = facturaVerNumero;
  const f = facturasEntradaData.find((x) => x.numero === numero);
  if (!f) return;
  if (
    !confirm(
      `Â¿Eliminar la factura "${f.numero}" de ${f.proveedor}? Esta acción no se puede deshacer.`,
    )
  )
    return;
  facturasEntradaData = facturasEntradaData.filter((x) => x.numero !== numero);
  facturaVerNumero = null;
  renderFacturasEntrada();
  showFacturaEntradaTab(
    "list",
    document.getElementById("tab-factura-entrada-list"),
  );
}

function exportarFacturaPDF(numero) {
  const f = facturasEntradaData.find((x) => x.numero === numero);
  if (!f || !window.jspdf) return;
  const { jsPDF } = window.jspdf;
  const doc = new jsPDF();
  let y = 20;
  doc.setFontSize(16);
  doc.text(`Factura de Entrada ${f.numero}`, 14, y);
  y += 10;
  doc.setFontSize(11);
  doc.text(`Proveedor: ${f.proveedor}`, 14, y);
  y += 7;
  doc.text(`Fecha de Emisión: ${formatearFecha(f.fechaEmision)}`, 14, y);
  y += 7;
  doc.text(`Estado: ${f.estado}`, 14, y);
  y += 7;
  doc.text(`Método de Pago: ${f.metodoPago || ":"}`, 14, y);
  y += 10;

  doc.setFontSize(12);
  doc.text("Productos", 14, y);
  y += 7;
  doc.setFontSize(10);
  doc.text("No.", 14, y);
  doc.text("Producto", 26, y);
  doc.text("Cant.", 110, y);
  doc.text("Unidad", 130, y);
  doc.text("Costo", 155, y);
  doc.text("Subtotal", 175, y);
  y += 5;
  doc.line(14, y, 196, y);
  y += 6;
  (f.lineas || []).forEach((li, i) => {
    doc.text(String(i + 1), 14, y);
    doc.text(String(li.producto).slice(0, 35), 26, y);
    doc.text(String(li.cantidad), 110, y);
    doc.text(String(li.unidad || "Unidad"), 130, y);
    doc.text(li.costo.toFixed(2), 155, y);
    doc.text((li.cantidad * li.costo).toFixed(2), 175, y);
    y += 7;
  });

  y += 5;
  const descuentoMonto = f.subtotal * ((f.descuentoPct || 0) / 100);
  const impuestoMonto =
    (f.subtotal - descuentoMonto) * ((f.impuestoPct || 0) / 100);
  doc.setFontSize(11);
  doc.text(`Subtotal: ${f.subtotal.toFixed(2)} USD`, 140, y);
  y += 7;
  doc.text(
    `Descuento (${f.descuentoPct || 0}%): ${descuentoMonto.toFixed(2)} USD`,
    140,
    y,
  );
  y += 7;
  doc.text(
    `Impuestos (${f.impuestoPct || 0}%): ${impuestoMonto.toFixed(2)} USD`,
    140,
    y,
  );
  y += 7;
  doc.setFontSize(12);
  doc.text(`Total: ${f.total.toFixed(2)} USD`, 140, y);
  y += 10;

  if (f.observaciones) {
    doc.setFontSize(10);
    doc.text(`Observaciones: ${f.observaciones}`, 14, y);
    y += 7;
  }
  if (f.descripcion) {
    doc.setFontSize(10);
    doc.text(`Descripción: ${f.descripcion}`, 14, y);
  }

  doc.save(`Factura_${f.numero}.pdf`);
}

function prepararNuevaFactura() {
  editingFacturaId = null;
  document.getElementById("factura-form-title").textContent =
    "Nueva Factura de Entrada";
  document.getElementById("factura-numero").value =
    "E1." + String(nextFacturaEntradaSeq).padStart(3, "0");
  document.getElementById("factura-proveedor").value = "";
  document.getElementById("factura-fecha-emision").value = new Date()
    .toISOString()
    .slice(0, 10);
  document.getElementById("factura-estado").value = "Por Pagar";
  document.getElementById("factura-metodo-pago").value = "Efectivo USD";
  document.getElementById("factura-observaciones").value = "";
  document.getElementById("factura-descripcion").value = "";
  document.getElementById("factura-descuento").value = 0;
  document.getElementById("factura-impuesto").value = 0;
  document.getElementById("factura-buscar-producto").value = "";
  quitarArchivoFactura();
  facturaLineItems = [];
  agregarLineaFactura();
  renderLineasFactura();
}

function cancelarFacturaEntradaForm() {
  editingFacturaId = null;
  showFacturaEntradaTab(
    "list",
    document.getElementById("tab-factura-entrada-list"),
  );
}

// ---- Lí­neas de producto de la factura ----
function agregarLineaFactura(nombreProducto = "") {
  facturaLineItems.push({
    id: nextLineItemId++,
    producto: nombreProducto,
    cantidad: 1,
    unidad: "Unidad",
    costo: 0,
  });
  renderLineasFactura();
}

function agregarLineaDesdeBusqueda() {
  const input = document.getElementById("factura-buscar-producto");
  const nombre = input.value.trim();
  if (!nombre) {
    input.focus();
    return;
  }
  agregarLineaFactura(nombre);
  input.value = "";
}

function crearProductoRapido() {
  irCrearProducto();
}

function quitarLineaFactura(id) {
  facturaLineItems = facturaLineItems.filter((li) => li.id !== id);
  renderLineasFactura();
}

function actualizarLineaFactura(id, campo, valor) {
  const linea = facturaLineItems.find((li) => li.id === id);
  if (!linea) return;
  linea[campo] =
    campo === "cantidad" || campo === "costo" ? parseFloat(valor) || 0 : valor;
  renderLineasFactura(true);
}

function renderLineasFactura(soloTotales = false) {
  const body = document.getElementById("factura-lineas-body");
  if (body && !soloTotales) {
    if (facturaLineItems.length === 0) {
      body.innerHTML =
        '<div class="li-empty">Añade productos con el buscador o el botón "Agregar Fila".</div>';
    } else {
      body.innerHTML = facturaLineItems
        .map(
          (li, i) => `
                        <div class="li-row li-body">
                            <div>${i + 1}</div>
                            <div><input type="text" value="${li.producto.replace(/"/g, "&quot;")}" placeholder="Nombre del producto" oninput="actualizarLineaFactura(${li.id}, 'producto', this.value)" /></div>
                            <div><input type="number" min="0" step="1" value="${li.cantidad}" oninput="actualizarLineaFactura(${li.id}, 'cantidad', this.value)" /></div>
                            <div>
                                <select onchange="actualizarLineaFactura(${li.id}, 'unidad', this.value)">
                                    ${opcionesUnidadHTML(li.unidad)}
                                </select>
                            </div>
                            <div class="li-costo">
                                $ <input type="number" min="0" step="0.01" value="${li.costo}" oninput="actualizarLineaFactura(${li.id}, 'costo', this.value)" /> USD
                            </div>
                            <div class="li-subtotal">${(li.cantidad * li.costo).toLocaleString("es-ES", { minimumFractionDigits: 2, maximumFractionDigits: 2 })} USD</div>
                            <div>
                                <button type="button" class="li-remove-btn" onclick="quitarLineaFactura(${li.id})" aria-label="Quitar lí­nea">
                                    <svg class="icon" style="width:16px;height:16px"><use href="#icon-trash"></use></svg>
                                </button>
                            </div>
                        </div>
                    `,
        )
        .join("");
    }
  }
  recalcularTotalesFactura();
}

function recalcularTotalesFactura() {
  const subtotal = facturaLineItems.reduce(
    (sum, li) => sum + li.cantidad * li.costo,
    0,
  );
  const descuentoPct =
    parseFloat(document.getElementById("factura-descuento")?.value) || 0;
  const impuestoPct =
    parseFloat(document.getElementById("factura-impuesto")?.value) || 0;
  const descuentoMonto = subtotal * (descuentoPct / 100);
  const impuestoMonto = (subtotal - descuentoMonto) * (impuestoPct / 100);
  const total = subtotal - descuentoMonto + impuestoMonto;

  const fmt = (n) =>
    n.toLocaleString("es-ES", {
      minimumFractionDigits: 2,
      maximumFractionDigits: 2,
    }) + " USD";
  const elSub = document.getElementById("factura-subtotal");
  const elTotal = document.getElementById("factura-total");
  if (elSub) elSub.textContent = fmt(subtotal);
  if (elTotal) elTotal.textContent = fmt(total);
}

// ---- Adjuntar archivo (drag & drop) ----
function manejarArchivoFactura(file) {
  if (!file) return;
  facturaArchivoActual = file;
  const dz = document.getElementById("factura-dropzone");
  dz.classList.add("has-file");
  dz.classList.remove("dragover");
  document.getElementById("factura-archivo-nombre").textContent = file.name;
}

function manejarDropArchivo(e) {
  e.preventDefault();
  const dz = document.getElementById("factura-dropzone");
  dz.classList.remove("dragover");
  const file = e.dataTransfer.files && e.dataTransfer.files[0];
  if (file) manejarArchivoFactura(file);
}

function quitarArchivoFactura() {
  facturaArchivoActual = null;
  const dz = document.getElementById("factura-dropzone");
  if (dz) dz.classList.remove("has-file");
  const input = document.getElementById("factura-archivo-input");
  if (input) input.value = "";
  const nombre = document.getElementById("factura-archivo-nombre");
  if (nombre) nombre.textContent = "";
}

// ---- Guardar factura ----
async function guardarFactura(e) {
  e.preventDefault();
  const proveedor = document.getElementById("factura-proveedor").value.trim();
  const fechaEmision = document.getElementById("factura-fecha-emision").value;
  const productosValidos = facturaLineItems.filter(
    (li) => li.producto.trim() !== "",
  );

  if (!proveedor || !fechaEmision) return false;
  if (productosValidos.length === 0) {
    alert("Agrega al menos un producto a la factura antes de guardarla.");
    return false;
  }

  const subtotal = productosValidos.reduce(
    (sum, li) => sum + li.cantidad * li.costo,
    0,
  );
  const descuentoPct =
    parseFloat(document.getElementById("factura-descuento").value) || 0;
  const impuestoPct =
    parseFloat(document.getElementById("factura-impuesto").value) || 0;
  const descuentoMonto = subtotal * (descuentoPct / 100);
  const total = (subtotal - descuentoMonto) * (1 + impuestoPct / 100);
  const { getSession } = await import("./session.js");

  facturasEntradaData.push({
    numero: document.getElementById("factura-numero").value,
    proveedor,
    fechaEmision,
    total,
    subtotal,
    descuentoPct,
    impuestoPct,
    observaciones: document
      .getElementById("factura-observaciones")
      .value.trim(),
    descripcion: document.getElementById("factura-descripcion").value.trim(),
    archivoNombre: facturaArchivoActual ? facturaArchivoActual.name : null,
    estado: document.getElementById("factura-estado").value,
    metodoPago: document.getElementById("factura-metodo-pago").value,
    productos: productosValidos.map((li) => li.producto),
    responsable: getSession()?.username || ":",
    lineas: productosValidos.map((li) => ({
      producto: li.producto,
      cantidad: li.cantidad,
      unidad: li.unidad,
      costo: li.costo,
    })),
  });
  nextFacturaEntradaSeq++;
  notificarFactura(
    "Entrada",
    document.getElementById("factura-numero").value,
    proveedor,
    total,
    document.getElementById("factura-estado").value,
  );

  cancelarFacturaEntradaForm();
  renderFacturasEntrada();
  return false;
}

// =====================================================
// ================= Facturas de Venta ==================
// =====================================================
let facturasVentaData = [
  {
    numero: "V1.001",
    cliente: "Ana Rizo",
    fechaEmision: "2027-01-05",
    total: 1250,
    subtotal: 1250,
    descuentoPct: 0,
    impuestoPct: 0,
    metodoPago: "Efectivo USD",
    descripcion: "",
    estado: "Pagada",
    productos: ["Laptop HP 15"],
    lineas: [
      { producto: "Laptop HP 15", cantidad: 2, costo: 500, unidad: "Unidad" },
    ],
  },
  {
    numero: "V1.002",
    cliente: "Enrique Lopez",
    fechaEmision: "2027-01-11",
    total: 275,
    subtotal: 275,
    descuentoPct: 0,
    impuestoPct: 0,
    metodoPago: "Transferencia USD",
    descripcion: "",
    estado: "Por Pagar",
    productos: ["Mouse Inalámbrico", "Teclado Mecánico"],
    lineas: [
      {
        producto: "Mouse Inalámbrico",
        cantidad: 5,
        costo: 15,
        unidad: "Unidad",
      },
      {
        producto: "Teclado Mecánico",
        cantidad: 5,
        costo: 40,
        unidad: "Unidad",
      },
    ],
  },
];
let nextFacturaVentaSeq = 3;
let editingFacturaVentaId = null;
let facturaVentaLineItems = [];
let nextVentaLineItemId = 1;

function renderFacturasVenta() {
  const body = document.getElementById("facturas-venta-body");
  if (!body) return;
  const query = (
    document.getElementById("factura-venta-search")?.value || ""
  ).toLowerCase();
  const rows = ordenarPorFecha(
    facturasVentaData.filter(
      (f) =>
        f.cliente.toLowerCase().includes(query) ||
        f.numero.toLowerCase().includes(query),
    ),
    "facturasVenta",
    "fechaEmision",
  );

  if (rows.length === 0) {
    body.innerHTML =
      '<tr><td colspan="6" class="empty-row">No se encontraron facturas que coincidan con la búsqueda.</td></tr>';
    return;
  }

  const estadoClase = {
    "Por Pagar": "status-pendiente",
    Pagada: "status-pagada",
    Vencida: "status-vencida",
  };
  body.innerHTML = rows
    .map(
      (f) => `
                <tr class="cat-row" onclick="verFacturaVenta('${f.numero}')" style="cursor:pointer">
                    <td data-label="No. Factura">${f.numero}</td>
                    <td data-label="Cliente">${f.cliente}</td>
                    <td data-label="Emisión">${formatearFecha(f.fechaEmision)}</td>
                    <td data-label="Total" style="text-align:right">${f.total.toLocaleString("es-ES", { minimumFractionDigits: 2, maximumFractionDigits: 2 })} USD</td>
                    <td data-label="Estado"><span class="status-badge ${estadoClase[f.estado] || ""}">${f.estado}</span></td>
                    <td>
                        <div class="col-acciones">
                            <button class="action-link" onclick="event.stopPropagation(); alert('La edición de facturas ya guardadas estará disponible próximamente.')">
                                <svg class="icon" style="width:16px;height:16px"><use href="#icon-edit"></use></svg>
                                Editar
                            </button>
                            <button class="action-link danger" onclick="event.stopPropagation(); eliminarFacturaVenta('${f.numero}')">
                                <svg class="icon" style="width:16px;height:16px"><use href="#icon-trash"></use></svg>
                                Eliminar
                            </button>
                        </div>
                    </td>
                </tr>
            `,
    )
    .join("");
}

function eliminarFacturaVenta(numero) {
  const f = facturasVentaData.find((x) => x.numero === numero);
  if (!f) return;
  if (
    !confirm(
      `Â¿Eliminar la factura "${f.numero}" de ${f.cliente}? Esta acción no se puede deshacer.`,
    )
  )
    return;
  facturasVentaData = facturasVentaData.filter((x) => x.numero !== numero);
  if (facturaVentaVerNumero === numero) facturaVentaVerNumero = null;
  renderFacturasVenta();
}

// ===== Vista de solo lectura: Ver Factura de Venta =====
let facturaVentaVerNumero = null;

function verFacturaVenta(numero) {
  const f = facturasVentaData.find((x) => x.numero === numero);
  if (!f) return;
  facturaVentaVerNumero = numero;

  document.getElementById("factura-venta-ver-titulo").textContent =
    "Factura de Venta " + f.numero;
  document.getElementById("factura-venta-ver-numero-badge").textContent =
    f.numero;
  document.getElementById("factura-venta-ver-cliente").textContent = f.cliente;
  document.getElementById("factura-venta-ver-numero").textContent = f.numero;
  document.getElementById("factura-venta-ver-emision").textContent =
    formatearFecha(f.fechaEmision);
  const estadoTono = {
    "Por Pagar": "tone-orange",
    Pagada: "tone-green",
    Vencida: "tone-red",
  };
  document.getElementById("factura-venta-ver-estado-txt").textContent =
    f.estado;
  document.getElementById("factura-venta-ver-estado-hero").textContent =
    f.estado;
  document
    .getElementById("factura-venta-ver-estado-tag")
    .classList.remove("tone-orange", "tone-green", "tone-red");
  document
    .getElementById("factura-venta-ver-estado-tag")
    .classList.add(estadoTono[f.estado] || "tone-orange");
  const estadoCardVenta = document.getElementById(
    "factura-venta-ver-estado-card",
  );
  estadoCardVenta.classList.remove("tone-orange", "tone-green", "tone-red");
  estadoCardVenta.classList.add(estadoTono[f.estado] || "tone-orange");
  document.getElementById("factura-venta-ver-metodo").textContent =
    f.metodoPago || ":";
  document.getElementById("factura-venta-ver-descripcion").textContent =
    f.descripcion || ":";

  const cuerpo = document.getElementById("factura-venta-ver-productos");
  cuerpo.innerHTML =
    (f.lineas || [])
      .map(
        (li, i) => `
                <div class="li-row li-body li-body-readonly">
                    <div>${i + 1}</div>
                    <div>${li.producto}</div>
                    <div>${li.cantidad}</div>
                    <div>${li.unidad || "Unidad"}</div>
                    <div class="li-costo">$ ${li.costo.toLocaleString("es-ES", { minimumFractionDigits: 2, maximumFractionDigits: 2 })} USD</div>
                    <div class="li-subtotal">${(li.cantidad * li.costo).toLocaleString("es-ES", { minimumFractionDigits: 2, maximumFractionDigits: 2 })} USD</div>
                </div>
            `,
      )
      .join("") ||
    '<div class="li-empty">Esta factura no tiene productos registrados.</div>';

  const fmt = (n) =>
    n.toLocaleString("es-ES", {
      minimumFractionDigits: 2,
      maximumFractionDigits: 2,
    }) + " USD";
  const subtotal =
    f.subtotal ??
    (f.lineas || []).reduce((sum, li) => sum + li.cantidad * li.costo, 0);
  const descuentoMonto = subtotal * ((f.descuentoPct || 0) / 100);
  const impuestoMonto =
    (subtotal - descuentoMonto) * ((f.impuestoPct || 0) / 100);
  document.getElementById("factura-venta-ver-subtotal").textContent =
    fmt(subtotal);
  document.getElementById("factura-venta-ver-subtotal-resumen").textContent =
    fmt(subtotal);
  document.getElementById("factura-venta-ver-descuento").textContent =
    fmt(descuentoMonto) + ` (${f.descuentoPct || 0}%)`;
  document.getElementById("factura-venta-ver-impuestos").textContent =
    fmt(impuestoMonto) + ` (${f.impuestoPct || 0}%)`;
  document.getElementById("factura-venta-ver-total").textContent = fmt(f.total);
  document.getElementById("factura-venta-ver-total-resumen").textContent = fmt(
    f.total,
  );

  showFacturaVentaTab("ver", null);
}

function eliminarFacturaVentaDesdeVer() {
  if (!facturaVentaVerNumero) return;
  const numero = facturaVentaVerNumero;
  const f = facturasVentaData.find((x) => x.numero === numero);
  if (!f) return;
  if (
    !confirm(
      `Â¿Eliminar la factura "${f.numero}" de ${f.cliente}? Esta acción no se puede deshacer.`,
    )
  )
    return;
  facturasVentaData = facturasVentaData.filter((x) => x.numero !== numero);
  facturaVentaVerNumero = null;
  renderFacturasVenta();
  showFacturaVentaTab(
    "list",
    document.getElementById("tab-factura-venta-list"),
  );
}

function exportarFacturaVentaPDF(numero) {
  const f = facturasVentaData.find((x) => x.numero === numero);
  if (!f || !window.jspdf) return;
  const { jsPDF } = window.jspdf;
  const doc = new jsPDF();
  let y = 20;
  doc.setFontSize(16);
  doc.text(`Factura de Venta ${f.numero}`, 14, y);
  y += 10;
  doc.setFontSize(11);
  doc.text(`Cliente: ${f.cliente}`, 14, y);
  y += 7;
  doc.text(`Fecha de Emisión: ${formatearFecha(f.fechaEmision)}`, 14, y);
  y += 7;
  doc.text(`Estado: ${f.estado}`, 14, y);
  y += 7;
  doc.text(`Método de Pago: ${f.metodoPago || ":"}`, 14, y);
  y += 10;

  doc.setFontSize(12);
  doc.text("Productos", 14, y);
  y += 7;
  doc.setFontSize(10);
  doc.text("No.", 14, y);
  doc.text("Producto", 26, y);
  doc.text("Cant.", 110, y);
  doc.text("Unidad", 130, y);
  doc.text("Precio", 155, y);
  doc.text("Subtotal", 175, y);
  y += 5;
  doc.line(14, y, 196, y);
  y += 6;
  (f.lineas || []).forEach((li, i) => {
    doc.text(String(i + 1), 14, y);
    doc.text(String(li.producto).slice(0, 35), 26, y);
    doc.text(String(li.cantidad), 110, y);
    doc.text(String(li.unidad || "Unidad"), 130, y);
    doc.text(li.costo.toFixed(2), 155, y);
    doc.text((li.cantidad * li.costo).toFixed(2), 175, y);
    y += 7;
  });

  y += 5;
  const subtotal =
    f.subtotal ??
    (f.lineas || []).reduce((sum, li) => sum + li.cantidad * li.costo, 0);
  const descuentoMonto = subtotal * ((f.descuentoPct || 0) / 100);
  const impuestoMonto =
    (subtotal - descuentoMonto) * ((f.impuestoPct || 0) / 100);
  doc.setFontSize(11);
  doc.text(`Subtotal: ${subtotal.toFixed(2)} USD`, 140, y);
  y += 7;
  doc.text(
    `Descuento (${f.descuentoPct || 0}%): ${descuentoMonto.toFixed(2)} USD`,
    140,
    y,
  );
  y += 7;
  doc.text(
    `Impuestos (${f.impuestoPct || 0}%): ${impuestoMonto.toFixed(2)} USD`,
    140,
    y,
  );
  y += 7;
  doc.setFontSize(12);
  doc.text(`Total: ${f.total.toFixed(2)} USD`, 140, y);
  y += 10;

  if (f.descripcion) {
    doc.setFontSize(10);
    doc.text(`Descripción: ${f.descripcion}`, 14, y);
  }

  doc.save(`Factura_${f.numero}.pdf`);
}

function showFacturaVentaTab(tab, btn) {
  document
    .querySelectorAll("#page-facturas-venta .config-subview")
    .forEach((v) => v.classList.remove("active"));
  document.getElementById("factura-venta-" + tab).classList.add("active");
  document
    .querySelectorAll("#page-facturas-venta .view-tabs .tab")
    .forEach((t) => t.classList.remove("active"));
  if (btn) btn.classList.add("active");
  document.getElementById("factura-venta-search-bar").style.display =
    tab === "list" ? "flex" : "none";
  if (tab === "create" && !editingFacturaVentaId) prepararNuevaFacturaVenta();
}

function prepararNuevaFacturaVenta() {
  editingFacturaVentaId = null;
  document.getElementById("factura-venta-form-title").textContent =
    "Nueva Factura de Venta";
  document.getElementById("factura-venta-cliente").value = "";
  document.getElementById("factura-venta-numero").value =
    "V1." + String(nextFacturaVentaSeq).padStart(3, "0");
  document.getElementById("factura-venta-fecha-emision").value = new Date()
    .toISOString()
    .slice(0, 10);
  document.getElementById("factura-venta-estado").value = "Por Pagar";
  document.getElementById("factura-venta-metodo-pago").value = "Efectivo USD";
  document.getElementById("factura-venta-descripcion").value = "";
  document.getElementById("factura-venta-descuento").value = 0;
  document.getElementById("factura-venta-impuesto").value = 0;
  document.getElementById("factura-venta-buscar-producto").value = "";
  facturaVentaLineItems = [];
  agregarLineaVenta();
  renderLineasVenta();
}

function cancelarFacturaVentaForm() {
  editingFacturaVentaId = null;
  showFacturaVentaTab(
    "list",
    document.getElementById("tab-factura-venta-list"),
  );
}

// ---- Lí­neas de producto de la venta (con autocompletado de precio) ----
function agregarLineaVenta(nombreProducto = "") {
  const productoRef = productosData.find(
    (p) => p.nombre.toLowerCase() === nombreProducto.trim().toLowerCase(),
  );
  facturaVentaLineItems.push({
    id: nextVentaLineItemId++,
    producto: nombreProducto,
    cantidad: 1,
    unidad: productoRef ? productoRef.unidad : "Unidad",
    precio: productoRef ? productoRef.precioVenta : 0,
    autofill: !!productoRef,
  });
  renderLineasVenta();
}

function agregarLineaDesdeBusquedaVenta() {
  const input = document.getElementById("factura-venta-buscar-producto");
  const nombre = input.value.trim();
  if (!nombre) {
    input.focus();
    return;
  }
  agregarLineaVenta(nombre);
  input.value = "";
}

function crearProductoVentaRapido() {
  irCrearProducto();
}

function quitarLineaVenta(id) {
  facturaVentaLineItems = facturaVentaLineItems.filter((li) => li.id !== id);
  renderLineasVenta();
}

function actualizarLineaVenta(id, campo, valor) {
  const linea = facturaVentaLineItems.find((li) => li.id === id);
  if (!linea) return;

  if (campo === "producto") {
    linea.producto = valor;
    const productoRef = productosData.find(
      (p) => p.nombre.toLowerCase() === valor.trim().toLowerCase(),
    );
    if (productoRef) {
      linea.unidad = productoRef.unidad;
      linea.precio = productoRef.precioVenta;
      linea.autofill = true;
    } else {
      linea.autofill = false;
    }
  } else if (campo === "cantidad" || campo === "precio") {
    linea[campo] = parseFloat(valor) || 0;
    linea.autofill = false;
  } else {
    linea[campo] = valor;
  }
  renderLineasVenta();
}

function renderLineasVenta(soloTotales = false) {
  const body = document.getElementById("factura-venta-lineas-body");
  if (body && !soloTotales) {
    if (facturaVentaLineItems.length === 0) {
      body.innerHTML =
        '<div class="li-empty">Añade productos con el buscador o el botón "Agregar Fila".</div>';
    } else {
      body.innerHTML = facturaVentaLineItems
        .map(
          (li, i) => `
                        <div class="li-row li-body">
                            <div>${i + 1}</div>
                            <div><input type="text" list="productos-datalist" value="${li.producto.replace(/"/g, "&quot;")}" placeholder="Nombre del producto" oninput="actualizarLineaVenta(${li.id}, 'producto', this.value)" /></div>
                            <div><input type="number" min="0" step="1" value="${li.cantidad}" oninput="actualizarLineaVenta(${li.id}, 'cantidad', this.value)" /></div>
                            <div>
                                <select onchange="actualizarLineaVenta(${li.id}, 'unidad', this.value)">
                                    ${opcionesUnidadHTML(li.unidad)}
                                </select>
                            </div>
                            <div class="li-costo${li.autofill ? " li-autofill" : ""}">
                                $ <input type="number" min="0" step="0.01" value="${li.precio}" oninput="actualizarLineaVenta(${li.id}, 'precio', this.value)" /> USD
                            </div>
                            <div class="li-subtotal">${(li.cantidad * li.precio).toLocaleString("es-ES", { minimumFractionDigits: 2, maximumFractionDigits: 2 })} USD</div>
                            <div>
                                <button type="button" class="li-remove-btn" onclick="quitarLineaVenta(${li.id})" aria-label="Quitar lí­nea">
                                    <svg class="icon" style="width:16px;height:16px"><use href="#icon-trash"></use></svg>
                                </button>
                            </div>
                        </div>
                    `,
        )
        .join("");
    }
  }
  recalcularTotalesVenta();
}

function recalcularTotalesVenta() {
  const subtotal = facturaVentaLineItems.reduce(
    (sum, li) => sum + li.cantidad * li.precio,
    0,
  );
  const descuentoPct =
    parseFloat(document.getElementById("factura-venta-descuento")?.value) || 0;
  const impuestoPct =
    parseFloat(document.getElementById("factura-venta-impuesto")?.value) || 0;
  const descuentoMonto = subtotal * (descuentoPct / 100);
  const impuestoMonto = (subtotal - descuentoMonto) * (impuestoPct / 100);
  const total = subtotal - descuentoMonto + impuestoMonto;

  const fmt = (n) =>
    n.toLocaleString("es-ES", {
      minimumFractionDigits: 2,
      maximumFractionDigits: 2,
    }) + " USD";
  const elSub = document.getElementById("factura-venta-subtotal");
  const elTotal = document.getElementById("factura-venta-total");
  if (elSub) elSub.textContent = fmt(subtotal);
  if (elTotal) elTotal.textContent = fmt(total);
}

function guardarFacturaVenta(e) {
  e.preventDefault();
  const cliente = document.getElementById("factura-venta-cliente").value.trim();
  const fechaEmision = document.getElementById(
    "factura-venta-fecha-emision",
  ).value;
  const productosValidos = facturaVentaLineItems.filter(
    (li) => li.producto.trim() && li.cantidad > 0,
  );

  if (!cliente || !fechaEmision) return false;
  if (productosValidos.length === 0) {
    alert("Agrega al menos un producto a la factura antes de guardarla.");
    return false;
  }

  const subtotal = productosValidos.reduce(
    (sum, li) => sum + li.cantidad * li.precio,
    0,
  );
  const descuentoPct =
    parseFloat(document.getElementById("factura-venta-descuento").value) || 0;
  const impuestoPct =
    parseFloat(document.getElementById("factura-venta-impuesto").value) || 0;
  const descuentoMonto = subtotal * (descuentoPct / 100);
  const total = (subtotal - descuentoMonto) * (1 + impuestoPct / 100);

  facturasVentaData.push({
    numero: document.getElementById("factura-venta-numero").value,
    cliente,
    fechaEmision,
    total,
    subtotal,
    descuentoPct,
    impuestoPct,
    metodoPago: document.getElementById("factura-venta-metodo-pago").value,
    descripcion: document
      .getElementById("factura-venta-descripcion")
      .value.trim(),
    estado: document.getElementById("factura-venta-estado").value,
    productos: productosValidos.map((li) => li.producto),
    lineas: productosValidos.map((li) => ({
      producto: li.producto,
      cantidad: li.cantidad,
      costo: li.precio,
    })),
  });
  nextFacturaVentaSeq++;
  notificarFactura(
    "Venta",
    document.getElementById("factura-venta-numero").value,
    cliente,
    total,
    document.getElementById("factura-venta-estado").value,
  );

  cancelarFacturaVentaForm();
  renderFacturasVenta();
  return false;
}

// =====================================================
// =============== Facturas de Servicios ================
// =====================================================
let facturasServiciosData = [
  {
    numero: "S1.001",
    cliente: "Lorenzo Espinosa",
    fechaEmision: "2027-01-04",
    total: 950,
    subtotal: 950,
    descuentoPct: 0,
    impuestoPct: 0,
    metodoPago: "Efectivo USD",
    descripcion: "",
    productos: [],
    lineas: [],
    estado: "Pagada",
  },
  {
    numero: "S1.002",
    cliente: "Ana Rizo",
    fechaEmision: "2027-01-08",
    total: 320,
    subtotal: 320,
    descuentoPct: 0,
    impuestoPct: 0,
    metodoPago: "Efectivo USD",
    descripcion: "",
    productos: [],
    lineas: [],
    estado: "Por Pagar",
  },
];
let nextFacturaServicioSeq = 3;
let editingFacturaServicioId = null;
let facturaServicioLineItems = [];
let nextServicioLineItemId = 1;

function renderFacturasServicios() {
  const body = document.getElementById("facturas-servicios-body");
  if (!body) return;
  const query = (
    document.getElementById("factura-servicios-search")?.value || ""
  ).toLowerCase();
  const rows = ordenarPorFecha(
    facturasServiciosData.filter(
      (f) =>
        f.cliente.toLowerCase().includes(query) ||
        f.numero.toLowerCase().includes(query),
    ),
    "facturasServicios",
    "fechaEmision",
  );

  if (rows.length === 0) {
    body.innerHTML =
      '<tr><td colspan="6" class="empty-row">No se encontraron facturas que coincidan con la búsqueda.</td></tr>';
    return;
  }

  const estadoClase = {
    "Por Pagar": "status-pendiente",
    Pagada: "status-pagada",
    Vencida: "status-vencida",
  };
  body.innerHTML = rows
    .map(
      (f) => `
                <tr class="cat-row" onclick="verFacturaServicio('${f.numero}')" style="cursor:pointer">
                    <td data-label="No. Factura">${f.numero}</td>
                    <td data-label="Cliente">${f.cliente}</td>
                    <td data-label="Emisión">${formatearFecha(f.fechaEmision)}</td>
                    <td data-label="Total" style="text-align:right">${f.total.toLocaleString("es-ES", { minimumFractionDigits: 2, maximumFractionDigits: 2 })} USD</td>
                    <td data-label="Estado"><span class="status-badge ${estadoClase[f.estado] || ""}">${f.estado}</span></td>
                    <td>
                        <div class="col-acciones">
                            <button class="action-link" onclick="event.stopPropagation(); alert('La edición de facturas ya guardadas estará disponible próximamente.')">
                                <svg class="icon" style="width:16px;height:16px"><use href="#icon-edit"></use></svg>
                                Editar
                            </button>
                            <button class="action-link danger" onclick="event.stopPropagation(); eliminarFacturaServicio('${f.numero}')">
                                <svg class="icon" style="width:16px;height:16px"><use href="#icon-trash"></use></svg>
                                Eliminar
                            </button>
                        </div>
                    </td>
                </tr>
            `,
    )
    .join("");
}

function eliminarFacturaServicio(numero) {
  const f = facturasServiciosData.find((x) => x.numero === numero);
  if (!f) return;
  if (
    !confirm(
      `Â¿Eliminar la factura "${f.numero}" de ${f.cliente}? Esta acción no se puede deshacer.`,
    )
  )
    return;
  facturasServiciosData = facturasServiciosData.filter(
    (x) => x.numero !== numero,
  );
  renderFacturasServicios();
}

function showFacturaServiciosTab(tab, btn) {
  document
    .querySelectorAll("#page-facturas-servicios .config-subview")
    .forEach((v) => v.classList.remove("active"));
  document.getElementById("factura-servicios-" + tab).classList.add("active");
  document
    .querySelectorAll("#page-facturas-servicios .view-tabs .tab")
    .forEach((t) => t.classList.remove("active"));
  if (btn) btn.classList.add("active");
  document.getElementById("factura-servicios-search-bar").style.display =
    tab === "list" ? "flex" : "none";
  if (tab === "create" && !editingFacturaServicioId)
    prepararNuevaFacturaServicio();
}

let facturaServicioVerNumero = null;

function verFacturaServicio(numero) {
  const f = facturasServiciosData.find((x) => x.numero === numero);
  if (!f) return;
  facturaServicioVerNumero = numero;

  document.getElementById("factura-servicio-ver-titulo").textContent =
    "Factura de Servicios " + f.numero;
  document.getElementById("factura-servicio-ver-numero-badge").textContent =
    f.numero;
  document.getElementById("factura-servicio-ver-cliente").textContent =
    f.cliente;
  document.getElementById("factura-servicio-ver-numero").textContent = f.numero;
  document.getElementById("factura-servicio-ver-emision").textContent =
    formatearFecha(f.fechaEmision);
  const estadoTono = {
    "Por Pagar": "tone-orange",
    Pagada: "tone-green",
    Vencida: "tone-red",
  };
  document.getElementById("factura-servicio-ver-estado-txt").textContent =
    f.estado;
  document.getElementById("factura-servicio-ver-estado-hero").textContent =
    f.estado;
  document
    .getElementById("factura-servicio-ver-estado-tag")
    .classList.remove("tone-orange", "tone-green", "tone-red");
  document
    .getElementById("factura-servicio-ver-estado-tag")
    .classList.add(estadoTono[f.estado] || "tone-orange");
  const estadoCardServicio = document.getElementById(
    "factura-servicio-ver-estado-card",
  );
  estadoCardServicio.classList.remove("tone-orange", "tone-green", "tone-red");
  estadoCardServicio.classList.add(estadoTono[f.estado] || "tone-orange");
  document.getElementById("factura-servicio-ver-metodo").textContent =
    f.metodoPago || ":";
  document.getElementById("factura-servicio-ver-descripcion").textContent =
    f.descripcion || ":";

  const cuerpo = document.getElementById("factura-servicio-ver-productos");
  cuerpo.innerHTML =
    (f.lineas || [])
      .map(
        (li, i) => `
                <div class="li-row li-body li-body-readonly">
                    <div>${i + 1}</div>
                    <div>${li.producto}</div>
                    <div>${li.cantidad}</div>
                    <div>${li.unidad || "Unidad"}</div>
                    <div class="li-costo">$ ${li.costo.toLocaleString("es-ES", { minimumFractionDigits: 2, maximumFractionDigits: 2 })} USD</div>
                    <div class="li-subtotal">${(li.cantidad * li.costo).toLocaleString("es-ES", { minimumFractionDigits: 2, maximumFractionDigits: 2 })} USD</div>
                </div>
            `,
      )
      .join("") ||
    '<div class="li-empty">Esta factura no tiene productos registrados.</div>';

  const fmt = (n) =>
    n.toLocaleString("es-ES", {
      minimumFractionDigits: 2,
      maximumFractionDigits: 2,
    }) + " USD";
  const descuentoMonto = f.subtotal * ((f.descuentoPct || 0) / 100);
  const impuestoMonto =
    (f.subtotal - descuentoMonto) * ((f.impuestoPct || 0) / 100);
  document.getElementById("factura-servicio-ver-subtotal").textContent = fmt(
    f.subtotal,
  );
  document.getElementById("factura-servicio-ver-subtotal-resumen").textContent =
    fmt(f.subtotal);
  document.getElementById("factura-servicio-ver-descuento").textContent =
    fmt(descuentoMonto) + ` (${f.descuentoPct || 0}%)`;
  document.getElementById("factura-servicio-ver-impuestos").textContent =
    fmt(impuestoMonto) + ` (${f.impuestoPct || 0}%)`;
  document.getElementById("factura-servicio-ver-total").textContent = fmt(
    f.total,
  );
  document.getElementById("factura-servicio-ver-total-resumen").textContent =
    fmt(f.total);

  showFacturaServiciosTab("ver", null);
}

function eliminarFacturaServicioDesdeVer() {
  if (!facturaServicioVerNumero) return;
  const numero = facturaServicioVerNumero;
  const f = facturasServiciosData.find((x) => x.numero === numero);
  if (!f) return;
  if (
    !confirm(
      `Â¿Eliminar la factura "${f.numero}" de ${f.cliente}? Esta acción no se puede deshacer.`,
    )
  )
    return;
  facturasServiciosData = facturasServiciosData.filter(
    (x) => x.numero !== numero,
  );
  facturaServicioVerNumero = null;
  renderFacturasServicios();
  showFacturaServiciosTab(
    "list",
    document.getElementById("tab-factura-servicios-list"),
  );
}

function exportarFacturaServicioPDF(numero) {
  const f = facturasServiciosData.find((x) => x.numero === numero);
  if (!f || !window.jspdf) return;
  const { jsPDF } = window.jspdf;
  const doc = new jsPDF();
  let y = 20;
  doc.setFontSize(16);
  doc.text(`Factura de Servicios ${f.numero}`, 14, y);
  y += 10;
  doc.setFontSize(11);
  doc.text(`Cliente: ${f.cliente}`, 14, y);
  y += 7;
  doc.text(`Fecha de Emisión: ${formatearFecha(f.fechaEmision)}`, 14, y);
  y += 7;
  doc.text(`Estado: ${f.estado}`, 14, y);
  y += 7;
  doc.text(`Método de Pago: ${f.metodoPago || ":"}`, 14, y);
  y += 10;

  doc.setFontSize(12);
  doc.text("Productos / Insumos", 14, y);
  y += 7;
  doc.setFontSize(10);
  doc.text("No.", 14, y);
  doc.text("Producto / Insumo", 26, y);
  doc.text("Cant.", 110, y);
  doc.text("Unidad", 130, y);
  doc.text("Costo", 155, y);
  doc.text("Subtotal", 175, y);
  y += 5;
  doc.line(14, y, 196, y);
  y += 6;
  (f.lineas || []).forEach((li, i) => {
    doc.text(String(i + 1), 14, y);
    doc.text(String(li.producto).slice(0, 35), 26, y);
    doc.text(String(li.cantidad), 110, y);
    doc.text(String(li.unidad || "Unidad"), 130, y);
    doc.text(li.costo.toFixed(2), 155, y);
    doc.text((li.cantidad * li.costo).toFixed(2), 175, y);
    y += 7;
  });

  y += 5;
  const descuentoMonto = f.subtotal * ((f.descuentoPct || 0) / 100);
  const impuestoMonto =
    (f.subtotal - descuentoMonto) * ((f.impuestoPct || 0) / 100);
  doc.setFontSize(11);
  doc.text(`Subtotal: ${f.subtotal.toFixed(2)} USD`, 140, y);
  y += 7;
  doc.text(
    `Descuento (${f.descuentoPct || 0}%): ${descuentoMonto.toFixed(2)} USD`,
    140,
    y,
  );
  y += 7;
  doc.text(
    `Impuestos (${f.impuestoPct || 0}%): ${impuestoMonto.toFixed(2)} USD`,
    140,
    y,
  );
  y += 7;
  doc.setFontSize(12);
  doc.text(`Total: ${f.total.toFixed(2)} USD`, 140, y);
  y += 10;

  if (f.descripcion) {
    doc.setFontSize(10);
    doc.text(`Descripción: ${f.descripcion}`, 14, y);
  }

  doc.save(`Factura_${f.numero}.pdf`);
}

function prepararNuevaFacturaServicio() {
  editingFacturaServicioId = null;
  document.getElementById("factura-servicio-form-title").textContent =
    "Nueva Factura de Servicios";
  document.getElementById("factura-servicio-numero").value =
    "S1." + String(nextFacturaServicioSeq).padStart(3, "0");
  document.getElementById("factura-servicio-cliente").value = "";
  document.getElementById("factura-servicio-fecha-emision").value = new Date()
    .toISOString()
    .slice(0, 10);
  document.getElementById("factura-servicio-estado").value = "Por Pagar";
  document.getElementById("factura-servicio-metodo-pago").value =
    "Efectivo USD";
  document.getElementById("factura-servicio-descripcion").value = "";
  document.getElementById("factura-servicio-descuento").value = 0;
  document.getElementById("factura-servicio-impuesto").value = 0;
  document.getElementById("factura-servicio-buscar").value = "";
  facturaServicioLineItems = [];
  agregarLineaServicio();
  renderLineasServicio();
}

function cancelarFacturaServiciosForm() {
  editingFacturaServicioId = null;
  showFacturaServiciosTab(
    "list",
    document.getElementById("tab-factura-servicios-list"),
  );
}

function agregarLineaServicio(nombreProducto = "") {
  facturaServicioLineItems.push({
    id: nextServicioLineItemId++,
    producto: nombreProducto,
    cantidad: 1,
    unidad: "Unidad",
    costo: 0,
  });
  renderLineasServicio();
}

function agregarLineaDesdeBusquedaServicio() {
  const input = document.getElementById("factura-servicio-buscar");
  const nombre = input.value.trim();
  if (!nombre) {
    input.focus();
    return;
  }
  agregarLineaServicio(nombre);
  input.value = "";
}

function crearInsumoServicioRapido() {
  alert(
    "La gestión completa de Insumos estará disponible próximamente. Se agregó una fila en blanco para completarla manualmente.",
  );
  agregarLineaServicio();
}

function quitarLineaServicio(id) {
  facturaServicioLineItems = facturaServicioLineItems.filter(
    (li) => li.id !== id,
  );
  renderLineasServicio();
}

function actualizarLineaServicio(id, campo, valor) {
  const linea = facturaServicioLineItems.find((li) => li.id === id);
  if (!linea) return;
  linea[campo] =
    campo === "cantidad" || campo === "costo" ? parseFloat(valor) || 0 : valor;
  renderLineasServicio(true);
}

function renderLineasServicio(soloTotales = false) {
  const body = document.getElementById("factura-servicio-lineas-body");
  if (body && !soloTotales) {
    if (facturaServicioLineItems.length === 0) {
      body.innerHTML =
        '<div class="li-empty">Añade insumos con el buscador o el botón "Agregar Fila".</div>';
    } else {
      body.innerHTML = facturaServicioLineItems
        .map(
          (li, i) => `
                        <div class="li-row li-body">
                            <div>${i + 1}</div>
                            <div><input type="text" value="${li.producto.replace(/"/g, "&quot;")}" placeholder="Nombre del insumo" oninput="actualizarLineaServicio(${li.id}, 'producto', this.value)" /></div>
                            <div><input type="number" min="0" step="1" value="${li.cantidad}" oninput="actualizarLineaServicio(${li.id}, 'cantidad', this.value)" /></div>
                            <div>
                                <select onchange="actualizarLineaServicio(${li.id}, 'unidad', this.value)">
                                    ${opcionesUnidadHTML(li.unidad)}
                                </select>
                            </div>
                            <div class="li-costo">
                                $ <input type="number" min="0" step="0.01" value="${li.costo}" oninput="actualizarLineaServicio(${li.id}, 'costo', this.value)" /> USD
                            </div>
                            <div class="li-subtotal">${(li.cantidad * li.costo).toLocaleString("es-ES", { minimumFractionDigits: 2, maximumFractionDigits: 2 })} USD</div>
                            <div>
                                <button type="button" class="li-remove-btn" onclick="quitarLineaServicio(${li.id})" aria-label="Quitar lí­nea">
                                    <svg class="icon" style="width:16px;height:16px"><use href="#icon-trash"></use></svg>
                                </button>
                            </div>
                        </div>
                    `,
        )
        .join("");
    }
  }
  recalcularTotalesServicio();
}

function recalcularTotalesServicio() {
  const subtotal = facturaServicioLineItems.reduce(
    (sum, li) => sum + li.cantidad * li.costo,
    0,
  );
  const descuentoPct =
    parseFloat(document.getElementById("factura-servicio-descuento")?.value) ||
    0;
  const impuestoPct =
    parseFloat(document.getElementById("factura-servicio-impuesto")?.value) ||
    0;
  const descuentoMonto = subtotal * (descuentoPct / 100);
  const impuestoMonto = (subtotal - descuentoMonto) * (impuestoPct / 100);
  const total = subtotal - descuentoMonto + impuestoMonto;

  const fmt = (n) =>
    n.toLocaleString("es-ES", {
      minimumFractionDigits: 2,
      maximumFractionDigits: 2,
    }) + " USD";
  const elSub = document.getElementById("factura-servicio-subtotal");
  const elTotal = document.getElementById("factura-servicio-total");
  if (elSub) elSub.textContent = fmt(subtotal);
  if (elTotal) elTotal.textContent = fmt(total);
}

function guardarFacturaServicio(e) {
  e.preventDefault();
  const cliente = document
    .getElementById("factura-servicio-cliente")
    .value.trim();
  const fechaEmision = document.getElementById(
    "factura-servicio-fecha-emision",
  ).value;
  const productosValidos = facturaServicioLineItems.filter(
    (li) => li.producto.trim() !== "",
  );

  if (!cliente || !fechaEmision) return false;
  if (productosValidos.length === 0) {
    alert("Agrega al menos un insumo a la factura antes de guardarla.");
    return false;
  }

  const subtotal = productosValidos.reduce(
    (sum, li) => sum + li.cantidad * li.costo,
    0,
  );
  const descuentoPct =
    parseFloat(document.getElementById("factura-servicio-descuento").value) ||
    0;
  const impuestoPct =
    parseFloat(document.getElementById("factura-servicio-impuesto").value) || 0;
  const descuentoMonto = subtotal * (descuentoPct / 100);
  const total = (subtotal - descuentoMonto) * (1 + impuestoPct / 100);

  facturasServiciosData.push({
    numero: document.getElementById("factura-servicio-numero").value,
    cliente,
    fechaEmision,
    total,
    subtotal,
    descuentoPct,
    impuestoPct,
    metodoPago: document.getElementById("factura-servicio-metodo-pago").value,
    descripcion: document
      .getElementById("factura-servicio-descripcion")
      .value.trim(),
    estado: document.getElementById("factura-servicio-estado").value,
    productos: productosValidos.map((li) => li.producto),
    lineas: productosValidos.map((li) => ({
      producto: li.producto,
      cantidad: li.cantidad,
      unidad: li.unidad,
      costo: li.costo,
    })),
  });
  nextFacturaServicioSeq++;
  notificarFactura(
    "Servicio",
    document.getElementById("factura-servicio-numero").value,
    cliente,
    total,
    document.getElementById("factura-servicio-estado").value,
  );

  cancelarFacturaServiciosForm();
  renderFacturasServicios();
  return false;
}

// =====================================================
// =============== Facturas de Insumos =================
// =====================================================
const insumosDisponibles = [
  "Cinta Aislante",
  "Brida",
  "Guantes de Trabajo",
  "Tornillos",
  "Silicona",
  "Lubricante",
  "Cable UTP",
  "Conectores RJ45",
];

let facturasInsumosData = [
  {
    numero: "I1.001",
    cliente: "Tatiana Fonseca",
    fechaEmision: "2027-01-02",
    total: 45.5,
    subtotal: 45.5,
    descuentoPct: 0,
    impuestoPct: 0,
    metodoPago: "Efectivo USD",
    descripcion: "",
    archivoNombre: null,
    productos: [],
    lineas: [],
    estado: "Pagada",
  },
  {
    numero: "I1.002",
    cliente: "Manuel Peña",
    fechaEmision: "2027-01-06",
    total: 120,
    subtotal: 120,
    descuentoPct: 0,
    impuestoPct: 0,
    metodoPago: "Efectivo USD",
    descripcion: "",
    archivoNombre: null,
    productos: [],
    lineas: [],
    estado: "Por Pagar",
  },
];
let nextFacturaInsumoSeq = 3;
let editingFacturaInsumoId = null;
let facturaInsumoLineItems = [];
let nextInsumoLineItemId = 1;
let facturaInsumoArchivoActual = null;

function irCrearCliente() {
  showPage(
    "clientes",
    document.querySelector('#sidebar a[data-page="clientes"]'),
  );
  showClienteTab("create", document.getElementById("tab-cliente-create"));
}

function renderFacturasInsumos() {
  const body = document.getElementById("facturas-insumos-body");
  if (!body) return;
  const query = (
    document.getElementById("factura-insumos-search")?.value || ""
  ).toLowerCase();
  const rows = ordenarPorFecha(
    facturasInsumosData.filter(
      (f) =>
        f.cliente.toLowerCase().includes(query) ||
        f.numero.toLowerCase().includes(query),
    ),
    "facturasInsumos",
    "fechaEmision",
  );

  if (rows.length === 0) {
    body.innerHTML =
      '<tr><td colspan="6" class="empty-row">No se encontraron facturas que coincidan con la búsqueda.</td></tr>';
    return;
  }

  const estadoClase = {
    "Por Pagar": "status-pendiente",
    Pagada: "status-pagada",
    Vencida: "status-vencida",
  };
  body.innerHTML = rows
    .map(
      (f) => `
                <tr class="cat-row" onclick="verFacturaInsumo('${f.numero}')" style="cursor:pointer">
                    <td data-label="No. Factura">${f.numero}</td>
                    <td data-label="Cliente">${f.cliente}</td>
                    <td data-label="Emisión">${formatearFecha(f.fechaEmision)}</td>
                    <td data-label="Total" style="text-align:right">${f.total.toLocaleString("es-ES", { minimumFractionDigits: 2, maximumFractionDigits: 2 })} USD</td>
                    <td data-label="Estado"><span class="status-badge ${estadoClase[f.estado] || ""}">${f.estado}</span></td>
                    <td>
                        <div class="col-acciones">
                            <button class="action-link" onclick="event.stopPropagation(); alert('La edición de facturas ya guardadas estará disponible próximamente.')">
                                <svg class="icon" style="width:16px;height:16px"><use href="#icon-edit"></use></svg>
                                Editar
                            </button>
                            <button class="action-link danger" onclick="event.stopPropagation(); eliminarFacturaInsumo('${f.numero}')">
                                <svg class="icon" style="width:16px;height:16px"><use href="#icon-trash"></use></svg>
                                Eliminar
                            </button>
                        </div>
                    </td>
                </tr>
            `,
    )
    .join("");
}

function eliminarFacturaInsumo(numero) {
  const f = facturasInsumosData.find((x) => x.numero === numero);
  if (!f) return;
  if (
    !confirm(
      `Â¿Eliminar la factura "${f.numero}" de ${f.cliente}? Esta acción no se puede deshacer.`,
    )
  )
    return;
  facturasInsumosData = facturasInsumosData.filter((x) => x.numero !== numero);
  renderFacturasInsumos();
}

function showFacturaInsumosTab(tab, btn) {
  document
    .querySelectorAll("#page-facturas-insumos .config-subview")
    .forEach((v) => v.classList.remove("active"));
  document.getElementById("factura-insumos-" + tab).classList.add("active");
  document
    .querySelectorAll("#page-facturas-insumos .view-tabs .tab")
    .forEach((t) => t.classList.remove("active"));
  if (btn) btn.classList.add("active");
  document.getElementById("factura-insumos-search-bar").style.display =
    tab === "list" ? "flex" : "none";
  if (tab === "create" && !editingFacturaInsumoId) prepararNuevaFacturaInsumo();
}

let facturaInsumoVerNumero = null;

function verFacturaInsumo(numero) {
  const f = facturasInsumosData.find((x) => x.numero === numero);
  if (!f) return;
  facturaInsumoVerNumero = numero;

  document.getElementById("factura-insumo-ver-titulo").textContent =
    "Factura de Gasto de Insumos " + f.numero;
  document.getElementById("factura-insumo-ver-numero-badge").textContent =
    f.numero;
  document.getElementById("factura-insumo-ver-cliente").textContent = f.cliente;
  document.getElementById("factura-insumo-ver-numero").textContent = f.numero;
  document.getElementById("factura-insumo-ver-emision").textContent =
    formatearFecha(f.fechaEmision);
  const estadoTono = {
    "Por Pagar": "tone-orange",
    Pagada: "tone-green",
    Vencida: "tone-red",
  };
  document.getElementById("factura-insumo-ver-estado-txt").textContent =
    f.estado;
  document.getElementById("factura-insumo-ver-estado-hero").textContent =
    f.estado;
  document
    .getElementById("factura-insumo-ver-estado-tag")
    .classList.remove("tone-orange", "tone-green", "tone-red");
  document
    .getElementById("factura-insumo-ver-estado-tag")
    .classList.add(estadoTono[f.estado] || "tone-orange");
  const estadoCardInsumo = document.getElementById(
    "factura-insumo-ver-estado-card",
  );
  estadoCardInsumo.classList.remove("tone-orange", "tone-green", "tone-red");
  estadoCardInsumo.classList.add(estadoTono[f.estado] || "tone-orange");
  document.getElementById("factura-insumo-ver-metodo").textContent =
    f.metodoPago || ":";
  document.getElementById("factura-insumo-ver-asociada").textContent =
    f.archivoNombre || "Sin archivo adjunto";
  document.getElementById("factura-insumo-ver-descripcion").textContent =
    f.descripcion || ":";

  const cuerpo = document.getElementById("factura-insumo-ver-productos");
  cuerpo.innerHTML =
    (f.lineas || [])
      .map(
        (li, i) => `
                <div class="li-row li-body li-body-readonly">
                    <div>${i + 1}</div>
                    <div>${li.producto}</div>
                    <div>${li.cantidad}</div>
                    <div>${li.unidad || "Unidad"}</div>
                    <div class="li-costo">$ ${li.costo.toLocaleString("es-ES", { minimumFractionDigits: 2, maximumFractionDigits: 2 })} USD</div>
                    <div class="li-subtotal">${(li.cantidad * li.costo).toLocaleString("es-ES", { minimumFractionDigits: 2, maximumFractionDigits: 2 })} USD</div>
                </div>
            `,
      )
      .join("") ||
    '<div class="li-empty">Esta factura no tiene insumos registrados.</div>';

  const fmt = (n) =>
    n.toLocaleString("es-ES", {
      minimumFractionDigits: 2,
      maximumFractionDigits: 2,
    }) + " USD";
  const descuentoMonto = f.subtotal * ((f.descuentoPct || 0) / 100);
  const impuestoMonto =
    (f.subtotal - descuentoMonto) * ((f.impuestoPct || 0) / 100);
  document.getElementById("factura-insumo-ver-subtotal").textContent = fmt(
    f.subtotal,
  );
  document.getElementById("factura-insumo-ver-descuento").textContent =
    fmt(descuentoMonto) + ` (${f.descuentoPct || 0}%)`;
  document.getElementById("factura-insumo-ver-impuestos").textContent =
    fmt(impuestoMonto) + ` (${f.impuestoPct || 0}%)`;
  document.getElementById("factura-insumo-ver-total").textContent = fmt(
    f.total,
  );
  document.getElementById("factura-insumo-ver-total-resumen").textContent = fmt(
    f.total,
  );

  showFacturaInsumosTab("ver", null);
}

function eliminarFacturaInsumoDesdeVer() {
  if (!facturaInsumoVerNumero) return;
  const numero = facturaInsumoVerNumero;
  const f = facturasInsumosData.find((x) => x.numero === numero);
  if (!f) return;
  if (
    !confirm(
      `Â¿Eliminar la factura "${f.numero}" de ${f.cliente}? Esta acción no se puede deshacer.`,
    )
  )
    return;
  facturasInsumosData = facturasInsumosData.filter((x) => x.numero !== numero);
  facturaInsumoVerNumero = null;
  renderFacturasInsumos();
  showFacturaInsumosTab(
    "list",
    document.getElementById("tab-factura-insumos-list"),
  );
}

function exportarFacturaInsumoPDF(numero) {
  const f = facturasInsumosData.find((x) => x.numero === numero);
  if (!f || !window.jspdf) return;
  const { jsPDF } = window.jspdf;
  const doc = new jsPDF();
  let y = 20;
  doc.setFontSize(16);
  doc.text(`Factura de Gasto de Insumos ${f.numero}`, 14, y);
  y += 10;
  doc.setFontSize(11);
  doc.text(`Cliente: ${f.cliente}`, 14, y);
  y += 7;
  doc.text(`Fecha de Emisión: ${formatearFecha(f.fechaEmision)}`, 14, y);
  y += 7;
  doc.text(`Estado: ${f.estado}`, 14, y);
  y += 7;
  doc.text(`Método de Pago: ${f.metodoPago || ":"}`, 14, y);
  y += 7;
  doc.text(
    `Factura de Servicio Asociada: ${f.archivoNombre || "Sin archivo adjunto"}`,
    14,
    y,
  );
  y += 10;

  doc.setFontSize(12);
  doc.text("Productos / Insumos", 14, y);
  y += 7;
  doc.setFontSize(10);
  doc.text("No.", 14, y);
  doc.text("Producto / Insumo", 26, y);
  doc.text("Cant.", 110, y);
  doc.text("Unidad", 130, y);
  doc.text("Costo", 155, y);
  doc.text("Subtotal", 175, y);
  y += 5;
  doc.line(14, y, 196, y);
  y += 6;
  (f.lineas || []).forEach((li, i) => {
    doc.text(String(i + 1), 14, y);
    doc.text(String(li.producto).slice(0, 35), 26, y);
    doc.text(String(li.cantidad), 110, y);
    doc.text(String(li.unidad || "Unidad"), 130, y);
    doc.text(li.costo.toFixed(2), 155, y);
    doc.text((li.cantidad * li.costo).toFixed(2), 175, y);
    y += 7;
  });

  y += 5;
  const descuentoMonto = f.subtotal * ((f.descuentoPct || 0) / 100);
  const impuestoMonto =
    (f.subtotal - descuentoMonto) * ((f.impuestoPct || 0) / 100);
  doc.setFontSize(11);
  doc.text(`Subtotal: ${f.subtotal.toFixed(2)} USD`, 140, y);
  y += 7;
  doc.text(
    `Descuento (${f.descuentoPct || 0}%): ${descuentoMonto.toFixed(2)} USD`,
    140,
    y,
  );
  y += 7;
  doc.text(
    `Impuestos (${f.impuestoPct || 0}%): ${impuestoMonto.toFixed(2)} USD`,
    140,
    y,
  );
  y += 7;
  doc.setFontSize(12);
  doc.text(`Total: ${f.total.toFixed(2)} USD`, 140, y);
  y += 10;

  if (f.descripcion) {
    doc.setFontSize(10);
    doc.text(`Descripción: ${f.descripcion}`, 14, y);
  }

  doc.save(`Factura_${f.numero}.pdf`);
}

function prepararNuevaFacturaInsumo() {
  editingFacturaInsumoId = null;
  document.getElementById("factura-insumo-form-title").textContent =
    "Nueva Factura de Gasto de Insumos";
  document.getElementById("factura-insumo-numero").value =
    "I1." + String(nextFacturaInsumoSeq).padStart(3, "0");
  document.getElementById("factura-insumo-cliente").value = "";
  document.getElementById("factura-insumo-fecha-emision").value = new Date()
    .toISOString()
    .slice(0, 10);
  document.getElementById("factura-insumo-estado").value = "Por Pagar";
  document.getElementById("factura-insumo-metodo-pago").value = "Efectivo USD";
  document.getElementById("factura-insumo-descripcion").value = "";
  document.getElementById("factura-insumo-descuento").value = 0;
  document.getElementById("factura-insumo-impuesto").value = 0;
  document.getElementById("factura-insumo-buscar").value = "";
  quitarArchivoInsumo();
  facturaInsumoLineItems = [];
  agregarLineaInsumo();
  renderLineasInsumo();
}

function cancelarFacturaInsumosForm() {
  editingFacturaInsumoId = null;
  showFacturaInsumosTab(
    "list",
    document.getElementById("tab-factura-insumos-list"),
  );
}

function agregarLineaInsumo(nombreProducto = "") {
  facturaInsumoLineItems.push({
    id: nextInsumoLineItemId++,
    producto: nombreProducto,
    cantidad: 1,
    unidad: "Unidad",
    costo: 0,
  });
  renderLineasInsumo();
}

function agregarLineaDesdeBusquedaInsumo() {
  const input = document.getElementById("factura-insumo-buscar");
  const nombre = input.value.trim();
  if (!nombre) {
    input.focus();
    return;
  }
  agregarLineaInsumo(nombre);
  input.value = "";
}

function crearInsumoRapido() {
  alert(
    "La gestión completa de Insumos estará disponible próximamente. Se agregó una fila en blanco para completarla manualmente.",
  );
  agregarLineaInsumo();
}

function quitarLineaInsumo(id) {
  facturaInsumoLineItems = facturaInsumoLineItems.filter((li) => li.id !== id);
  renderLineasInsumo();
}

function actualizarLineaInsumo(id, campo, valor) {
  const linea = facturaInsumoLineItems.find((li) => li.id === id);
  if (!linea) return;
  linea[campo] =
    campo === "cantidad" || campo === "costo" ? parseFloat(valor) || 0 : valor;
  renderLineasInsumo(true);
}

function renderLineasInsumo(soloTotales = false) {
  const body = document.getElementById("factura-insumo-lineas-body");
  if (body && !soloTotales) {
    if (facturaInsumoLineItems.length === 0) {
      body.innerHTML =
        '<div class="li-empty">Añade insumos con el buscador o el botón "Agregar Fila".</div>';
    } else {
      body.innerHTML = facturaInsumoLineItems
        .map(
          (li, i) => `
                        <div class="li-row li-body">
                            <div>${i + 1}</div>
                            <div><input type="text" value="${li.producto.replace(/"/g, "&quot;")}" placeholder="Nombre del insumo" oninput="actualizarLineaInsumo(${li.id}, 'producto', this.value)" /></div>
                            <div><input type="number" min="0" step="1" value="${li.cantidad}" oninput="actualizarLineaInsumo(${li.id}, 'cantidad', this.value)" /></div>
                            <div>
                                <select onchange="actualizarLineaInsumo(${li.id}, 'unidad', this.value)">
                                    ${opcionesUnidadHTML(li.unidad)}
                                </select>
                            </div>
                            <div class="li-costo">
                                $ <input type="number" min="0" step="0.01" value="${li.costo}" oninput="actualizarLineaInsumo(${li.id}, 'costo', this.value)" /> USD
                            </div>
                            <div class="li-subtotal">${(li.cantidad * li.costo).toLocaleString("es-ES", { minimumFractionDigits: 2, maximumFractionDigits: 2 })} USD</div>
                            <div>
                                <button type="button" class="li-remove-btn" onclick="quitarLineaInsumo(${li.id})" aria-label="Quitar lí­nea">
                                    <svg class="icon" style="width:16px;height:16px"><use href="#icon-trash"></use></svg>
                                </button>
                            </div>
                        </div>
                    `,
        )
        .join("");
    }
  }
  recalcularTotalesInsumo();
}

function recalcularTotalesInsumo() {
  const subtotal = facturaInsumoLineItems.reduce(
    (sum, li) => sum + li.cantidad * li.costo,
    0,
  );
  const descuentoPct =
    parseFloat(document.getElementById("factura-insumo-descuento")?.value) || 0;
  const impuestoPct =
    parseFloat(document.getElementById("factura-insumo-impuesto")?.value) || 0;
  const descuentoMonto = subtotal * (descuentoPct / 100);
  const impuestoMonto = (subtotal - descuentoMonto) * (impuestoPct / 100);
  const total = subtotal - descuentoMonto + impuestoMonto;

  const fmt = (n) =>
    n.toLocaleString("es-ES", {
      minimumFractionDigits: 2,
      maximumFractionDigits: 2,
    }) + " USD";
  const elSub = document.getElementById("factura-insumo-subtotal");
  const elTotal = document.getElementById("factura-insumo-total");
  if (elSub) elSub.textContent = fmt(subtotal);
  if (elTotal) elTotal.textContent = fmt(total);
}

function manejarArchivoInsumo(file) {
  if (!file) return;
  facturaInsumoArchivoActual = file;
  const dz = document.getElementById("factura-insumo-dropzone");
  dz.classList.add("has-file");
  dz.classList.remove("dragover");
  document.getElementById("factura-insumo-archivo-nombre").textContent =
    file.name;
}

function manejarDropArchivoInsumo(e) {
  e.preventDefault();
  const dz = document.getElementById("factura-insumo-dropzone");
  dz.classList.remove("dragover");
  const file = e.dataTransfer.files && e.dataTransfer.files[0];
  if (file) manejarArchivoInsumo(file);
}

function quitarArchivoInsumo() {
  facturaInsumoArchivoActual = null;
  const dz = document.getElementById("factura-insumo-dropzone");
  if (dz) dz.classList.remove("has-file");
  const input = document.getElementById("factura-insumo-archivo-input");
  if (input) input.value = "";
  const nombre = document.getElementById("factura-insumo-archivo-nombre");
  if (nombre) nombre.textContent = "";
}

function guardarFacturaInsumo(e) {
  e.preventDefault();
  const cliente = document
    .getElementById("factura-insumo-cliente")
    .value.trim();
  const fechaEmision = document.getElementById(
    "factura-insumo-fecha-emision",
  ).value;
  const productosValidos = facturaInsumoLineItems.filter(
    (li) => li.producto.trim() !== "",
  );

  if (!cliente || !fechaEmision) return false;
  if (productosValidos.length === 0) {
    alert("Agrega al menos un insumo a la factura antes de guardarla.");
    return false;
  }

  const subtotal = productosValidos.reduce(
    (sum, li) => sum + li.cantidad * li.costo,
    0,
  );
  const descuentoPct =
    parseFloat(document.getElementById("factura-insumo-descuento").value) || 0;
  const impuestoPct =
    parseFloat(document.getElementById("factura-insumo-impuesto").value) || 0;
  const descuentoMonto = subtotal * (descuentoPct / 100);
  const total = (subtotal - descuentoMonto) * (1 + impuestoPct / 100);

  facturasInsumosData.push({
    numero: document.getElementById("factura-insumo-numero").value,
    cliente,
    fechaEmision,
    total,
    subtotal,
    descuentoPct,
    impuestoPct,
    metodoPago: document.getElementById("factura-insumo-metodo-pago").value,
    descripcion: document
      .getElementById("factura-insumo-descripcion")
      .value.trim(),
    archivoNombre: facturaInsumoArchivoActual
      ? facturaInsumoArchivoActual.name
      : null,
    estado: document.getElementById("factura-insumo-estado").value,
    productos: productosValidos.map((li) => li.producto),
    lineas: productosValidos.map((li) => ({
      producto: li.producto,
      cantidad: li.cantidad,
      unidad: li.unidad,
      costo: li.costo,
    })),
  });
  nextFacturaInsumoSeq++;
  notificarFactura(
    "Insumo",
    document.getElementById("factura-insumo-numero").value,
    cliente,
    total,
    document.getElementById("factura-insumo-estado").value,
  );

  cancelarFacturaInsumosForm();
  renderFacturasInsumos();
  return false;
}

// =====================================================
// =============== Facturas de Garantí­a ================
// =====================================================
let facturasGarantiaData = [
  {
    numero: "G1.001",
    cliente: "Ana Rizo",
    fechaEmision: "2027-01-03",
    total: 0,
    subtotal: 0,
    descuentoPct: 0,
    impuestoPct: 0,
    metodoPago: "Efectivo USD",
    descripcion: "",
    archivoNombre: null,
    productos: [],
    lineas: [],
    estado: "Pagada",
  },
];
let nextFacturaGarantiaSeq = 2;
let editingFacturaGarantiaId = null;
let facturaGarantiaLineItems = [];
let nextGarantiaLineItemId = 1;
let facturaGarantiaArchivoActual = null;

function renderFacturasGarantia() {
  const body = document.getElementById("facturas-garantia-body");
  if (!body) return;
  const query = (
    document.getElementById("factura-garantia-search")?.value || ""
  ).toLowerCase();
  const rows = ordenarPorFecha(
    facturasGarantiaData.filter(
      (f) =>
        f.cliente.toLowerCase().includes(query) ||
        f.numero.toLowerCase().includes(query),
    ),
    "facturasGarantia",
    "fechaEmision",
  );

  if (rows.length === 0) {
    body.innerHTML =
      '<tr><td colspan="6" class="empty-row">No se encontraron facturas que coincidan con la búsqueda.</td></tr>';
    return;
  }

  const estadoClase = {
    "Por Pagar": "status-pendiente",
    Pagada: "status-pagada",
    Vencida: "status-vencida",
  };
  body.innerHTML = rows
    .map(
      (f) => `
                <tr class="cat-row" onclick="verFacturaGarantia('${f.numero}')" style="cursor:pointer">
                    <td data-label="No. Factura">${f.numero}</td>
                    <td data-label="Cliente">${f.cliente}</td>
                    <td data-label="Emisión">${formatearFecha(f.fechaEmision)}</td>
                    <td data-label="Total" style="text-align:right">${f.total.toLocaleString("es-ES", { minimumFractionDigits: 2, maximumFractionDigits: 2 })} USD</td>
                    <td data-label="Estado"><span class="status-badge ${estadoClase[f.estado] || ""}">${f.estado}</span></td>
                    <td>
                        <div class="col-acciones">
                            <button class="action-link" onclick="event.stopPropagation(); alert('La edición de facturas ya guardadas estará disponible próximamente.')">
                                <svg class="icon" style="width:16px;height:16px"><use href="#icon-edit"></use></svg>
                                Editar
                            </button>
                            <button class="action-link danger" onclick="event.stopPropagation(); eliminarFacturaGarantia('${f.numero}')">
                                <svg class="icon" style="width:16px;height:16px"><use href="#icon-trash"></use></svg>
                                Eliminar
                            </button>
                        </div>
                    </td>
                </tr>
            `,
    )
    .join("");
}

function eliminarFacturaGarantia(numero) {
  const f = facturasGarantiaData.find((x) => x.numero === numero);
  if (!f) return;
  if (
    !confirm(
      `Â¿Eliminar la factura "${f.numero}" de ${f.cliente}? Esta acción no se puede deshacer.`,
    )
  )
    return;
  facturasGarantiaData = facturasGarantiaData.filter(
    (x) => x.numero !== numero,
  );
  renderFacturasGarantia();
}

function showFacturaGarantiaTab(tab, btn) {
  document
    .querySelectorAll("#page-facturas-garantia .config-subview")
    .forEach((v) => v.classList.remove("active"));
  document.getElementById("factura-garantia-" + tab).classList.add("active");
  document
    .querySelectorAll("#page-facturas-garantia .view-tabs .tab")
    .forEach((t) => t.classList.remove("active"));
  if (btn) btn.classList.add("active");
  document.getElementById("factura-garantia-search-bar").style.display =
    tab === "list" ? "flex" : "none";
  if (tab === "create" && !editingFacturaGarantiaId)
    prepararNuevaFacturaGarantia();
}

let facturaGarantiaVerNumero = null;

function verFacturaGarantia(numero) {
  const f = facturasGarantiaData.find((x) => x.numero === numero);
  if (!f) return;
  facturaGarantiaVerNumero = numero;

  document.getElementById("factura-garantia-ver-titulo").textContent =
    "Factura de Gasto de Garantí­a " + f.numero;
  document.getElementById("factura-garantia-ver-numero-badge").textContent =
    f.numero;
  document.getElementById("factura-garantia-ver-cliente").textContent =
    f.cliente;
  document.getElementById("factura-garantia-ver-numero").textContent = f.numero;
  document.getElementById("factura-garantia-ver-emision").textContent =
    formatearFecha(f.fechaEmision);
  const estadoTono = {
    "Por Pagar": "tone-orange",
    Pagada: "tone-green",
    Vencida: "tone-red",
  };
  document.getElementById("factura-garantia-ver-estado-txt").textContent =
    f.estado;
  document.getElementById("factura-garantia-ver-estado-hero").textContent =
    f.estado;
  document
    .getElementById("factura-garantia-ver-estado-tag")
    .classList.remove("tone-orange", "tone-green", "tone-red");
  document
    .getElementById("factura-garantia-ver-estado-tag")
    .classList.add(estadoTono[f.estado] || "tone-orange");
  const estadoCardGarantia = document.getElementById(
    "factura-garantia-ver-estado-card",
  );
  estadoCardGarantia.classList.remove("tone-orange", "tone-green", "tone-red");
  estadoCardGarantia.classList.add(estadoTono[f.estado] || "tone-orange");
  document.getElementById("factura-garantia-ver-metodo").textContent =
    f.metodoPago || ":";
  document.getElementById("factura-garantia-ver-original").textContent =
    f.archivoNombre || "Sin archivo adjunto";
  document.getElementById("factura-garantia-ver-descripcion").textContent =
    f.descripcion || ":";

  const cuerpo = document.getElementById("factura-garantia-ver-productos");
  cuerpo.innerHTML =
    (f.lineas || [])
      .map(
        (li, i) => `
                <div class="li-row li-body li-body-readonly">
                    <div>${i + 1}</div>
                    <div>${li.producto}</div>
                    <div>${li.cantidad}</div>
                    <div>${li.unidad || "Unidad"}</div>
                    <div class="li-costo">$ ${li.costo.toLocaleString("es-ES", { minimumFractionDigits: 2, maximumFractionDigits: 2 })} USD</div>
                    <div class="li-subtotal">${(li.cantidad * li.costo).toLocaleString("es-ES", { minimumFractionDigits: 2, maximumFractionDigits: 2 })} USD</div>
                </div>
            `,
      )
      .join("") ||
    '<div class="li-empty">Esta factura no tiene productos registrados.</div>';

  const fmt = (n) =>
    n.toLocaleString("es-ES", {
      minimumFractionDigits: 2,
      maximumFractionDigits: 2,
    }) + " USD";
  const descuentoMonto = f.subtotal * ((f.descuentoPct || 0) / 100);
  const impuestoMonto =
    (f.subtotal - descuentoMonto) * ((f.impuestoPct || 0) / 100);
  document.getElementById("factura-garantia-ver-subtotal").textContent = fmt(
    f.subtotal,
  );
  document.getElementById("factura-garantia-ver-descuento").textContent =
    fmt(descuentoMonto) + ` (${f.descuentoPct || 0}%)`;
  document.getElementById("factura-garantia-ver-impuestos").textContent =
    fmt(impuestoMonto) + ` (${f.impuestoPct || 0}%)`;
  document.getElementById("factura-garantia-ver-total").textContent = fmt(
    f.total,
  );
  document.getElementById("factura-garantia-ver-total-resumen").textContent =
    fmt(f.total);

  showFacturaGarantiaTab("ver", null);
}

function eliminarFacturaGarantiaDesdeVer() {
  if (!facturaGarantiaVerNumero) return;
  const numero = facturaGarantiaVerNumero;
  const f = facturasGarantiaData.find((x) => x.numero === numero);
  if (!f) return;
  if (
    !confirm(
      `Â¿Eliminar la factura "${f.numero}" de ${f.cliente}? Esta acción no se puede deshacer.`,
    )
  )
    return;
  facturasGarantiaData = facturasGarantiaData.filter(
    (x) => x.numero !== numero,
  );
  facturaGarantiaVerNumero = null;
  renderFacturasGarantia();
  showFacturaGarantiaTab(
    "list",
    document.getElementById("tab-factura-garantia-list"),
  );
}

function exportarFacturaGarantiaPDF(numero) {
  const f = facturasGarantiaData.find((x) => x.numero === numero);
  if (!f || !window.jspdf) return;
  const { jsPDF } = window.jspdf;
  const doc = new jsPDF();
  let y = 20;
  doc.setFontSize(16);
  doc.text(`Factura de Gasto de Garantí­a ${f.numero}`, 14, y);
  y += 10;
  doc.setFontSize(11);
  doc.text(`Cliente: ${f.cliente}`, 14, y);
  y += 7;
  doc.text(`Fecha de Emisión: ${formatearFecha(f.fechaEmision)}`, 14, y);
  y += 7;
  doc.text(`Estado: ${f.estado}`, 14, y);
  y += 7;
  doc.text(`Método de Pago: ${f.metodoPago || ":"}`, 14, y);
  y += 7;
  doc.text(
    `Factura Original Asociada: ${f.archivoNombre || "Sin archivo adjunto"}`,
    14,
    y,
  );
  y += 10;

  doc.setFontSize(12);
  doc.text("Productos / Insumos", 14, y);
  y += 7;
  doc.setFontSize(10);
  doc.text("No.", 14, y);
  doc.text("Producto / Insumo", 26, y);
  doc.text("Cant.", 110, y);
  doc.text("Unidad", 130, y);
  doc.text("Costo", 155, y);
  doc.text("Subtotal", 175, y);
  y += 5;
  doc.line(14, y, 196, y);
  y += 6;
  (f.lineas || []).forEach((li, i) => {
    doc.text(String(i + 1), 14, y);
    doc.text(String(li.producto).slice(0, 35), 26, y);
    doc.text(String(li.cantidad), 110, y);
    doc.text(String(li.unidad || "Unidad"), 130, y);
    doc.text(li.costo.toFixed(2), 155, y);
    doc.text((li.cantidad * li.costo).toFixed(2), 175, y);
    y += 7;
  });

  y += 5;
  const descuentoMonto = f.subtotal * ((f.descuentoPct || 0) / 100);
  const impuestoMonto =
    (f.subtotal - descuentoMonto) * ((f.impuestoPct || 0) / 100);
  doc.setFontSize(11);
  doc.text(`Subtotal: ${f.subtotal.toFixed(2)} USD`, 140, y);
  y += 7;
  doc.text(
    `Descuento (${f.descuentoPct || 0}%): ${descuentoMonto.toFixed(2)} USD`,
    140,
    y,
  );
  y += 7;
  doc.text(
    `Impuestos (${f.impuestoPct || 0}%): ${impuestoMonto.toFixed(2)} USD`,
    140,
    y,
  );
  y += 7;
  doc.setFontSize(12);
  doc.text(`Total: ${f.total.toFixed(2)} USD`, 140, y);
  y += 10;

  if (f.descripcion) {
    doc.setFontSize(10);
    doc.text(`Descripción: ${f.descripcion}`, 14, y);
  }

  doc.save(`Factura_${f.numero}.pdf`);
}

function prepararNuevaFacturaGarantia() {
  editingFacturaGarantiaId = null;
  document.getElementById("factura-garantia-form-title").textContent =
    "Nueva Factura de Gasto de Garantí­a";
  document.getElementById("factura-garantia-numero").value =
    "G1." + String(nextFacturaGarantiaSeq).padStart(3, "0");
  document.getElementById("factura-garantia-cliente").value = "";
  document.getElementById("factura-garantia-fecha-emision").value = new Date()
    .toISOString()
    .slice(0, 10);
  document.getElementById("factura-garantia-estado").value = "Por Pagar";
  document.getElementById("factura-garantia-metodo-pago").value =
    "Efectivo USD";
  document.getElementById("factura-garantia-descripcion").value = "";
  document.getElementById("factura-garantia-descuento").value = 0;
  document.getElementById("factura-garantia-impuesto").value = 0;
  document.getElementById("factura-garantia-buscar").value = "";
  quitarArchivoGarantia();
  facturaGarantiaLineItems = [];
  agregarLineaGarantia();
  renderLineasGarantia();
}

function cancelarFacturaGarantiaForm() {
  editingFacturaGarantiaId = null;
  showFacturaGarantiaTab(
    "list",
    document.getElementById("tab-factura-garantia-list"),
  );
}

function agregarLineaGarantia(nombreProducto = "") {
  facturaGarantiaLineItems.push({
    id: nextGarantiaLineItemId++,
    producto: nombreProducto,
    cantidad: 1,
    unidad: "Unidad",
    costo: 0,
  });
  renderLineasGarantia();
}

function agregarLineaDesdeBusquedaGarantia() {
  const input = document.getElementById("factura-garantia-buscar");
  const nombre = input.value.trim();
  if (!nombre) {
    input.focus();
    return;
  }
  agregarLineaGarantia(nombre);
  input.value = "";
}

function crearProductoGarantiaRapido() {
  irCrearProducto();
}

function quitarLineaGarantia(id) {
  facturaGarantiaLineItems = facturaGarantiaLineItems.filter(
    (li) => li.id !== id,
  );
  renderLineasGarantia();
}

function actualizarLineaGarantia(id, campo, valor) {
  const linea = facturaGarantiaLineItems.find((li) => li.id === id);
  if (!linea) return;
  linea[campo] =
    campo === "cantidad" || campo === "costo" ? parseFloat(valor) || 0 : valor;
  renderLineasGarantia(true);
}

function renderLineasGarantia(soloTotales = false) {
  const body = document.getElementById("factura-garantia-lineas-body");
  if (body && !soloTotales) {
    if (facturaGarantiaLineItems.length === 0) {
      body.innerHTML =
        '<div class="li-empty">Añade productos con el buscador o el botón "Agregar Fila".</div>';
    } else {
      body.innerHTML = facturaGarantiaLineItems
        .map(
          (li, i) => `
                        <div class="li-row li-body">
                            <div>${i + 1}</div>
                            <div><input type="text" value="${li.producto.replace(/"/g, "&quot;")}" placeholder="Nombre del producto" oninput="actualizarLineaGarantia(${li.id}, 'producto', this.value)" /></div>
                            <div><input type="number" min="0" step="1" value="${li.cantidad}" oninput="actualizarLineaGarantia(${li.id}, 'cantidad', this.value)" /></div>
                            <div>
                                <select onchange="actualizarLineaGarantia(${li.id}, 'unidad', this.value)">
                                    ${opcionesUnidadHTML(li.unidad)}
                                </select>
                            </div>
                            <div class="li-costo">
                                $ <input type="number" min="0" step="0.01" value="${li.costo}" oninput="actualizarLineaGarantia(${li.id}, 'costo', this.value)" /> USD
                            </div>
                            <div class="li-subtotal">${(li.cantidad * li.costo).toLocaleString("es-ES", { minimumFractionDigits: 2, maximumFractionDigits: 2 })} USD</div>
                            <div>
                                <button type="button" class="li-remove-btn" onclick="quitarLineaGarantia(${li.id})" aria-label="Quitar lí­nea">
                                    <svg class="icon" style="width:16px;height:16px"><use href="#icon-trash"></use></svg>
                                </button>
                            </div>
                        </div>
                    `,
        )
        .join("");
    }
  }
  recalcularTotalesGarantia();
}

function recalcularTotalesGarantia() {
  const subtotal = facturaGarantiaLineItems.reduce(
    (sum, li) => sum + li.cantidad * li.costo,
    0,
  );
  const descuentoPct =
    parseFloat(document.getElementById("factura-garantia-descuento")?.value) ||
    0;
  const impuestoPct =
    parseFloat(document.getElementById("factura-garantia-impuesto")?.value) ||
    0;
  const descuentoMonto = subtotal * (descuentoPct / 100);
  const impuestoMonto = (subtotal - descuentoMonto) * (impuestoPct / 100);
  const total = subtotal - descuentoMonto + impuestoMonto;

  const fmt = (n) =>
    n.toLocaleString("es-ES", {
      minimumFractionDigits: 2,
      maximumFractionDigits: 2,
    }) + " USD";
  const elSub = document.getElementById("factura-garantia-subtotal");
  const elTotal = document.getElementById("factura-garantia-total");
  if (elSub) elSub.textContent = fmt(subtotal);
  if (elTotal) elTotal.textContent = fmt(total);
}

function manejarArchivoGarantia(file) {
  if (!file) return;
  facturaGarantiaArchivoActual = file;
  const dz = document.getElementById("factura-garantia-dropzone");
  dz.classList.add("has-file");
  dz.classList.remove("dragover");
  document.getElementById("factura-garantia-archivo-nombre").textContent =
    file.name;
}

function manejarDropArchivoGarantia(e) {
  e.preventDefault();
  const dz = document.getElementById("factura-garantia-dropzone");
  dz.classList.remove("dragover");
  const file = e.dataTransfer.files && e.dataTransfer.files[0];
  if (file) manejarArchivoGarantia(file);
}

function quitarArchivoGarantia() {
  facturaGarantiaArchivoActual = null;
  const dz = document.getElementById("factura-garantia-dropzone");
  if (dz) dz.classList.remove("has-file");
  const input = document.getElementById("factura-garantia-archivo-input");
  if (input) input.value = "";
  const nombre = document.getElementById("factura-garantia-archivo-nombre");
  if (nombre) nombre.textContent = "";
}

function guardarFacturaGarantia(e) {
  e.preventDefault();
  const cliente = document
    .getElementById("factura-garantia-cliente")
    .value.trim();
  const fechaEmision = document.getElementById(
    "factura-garantia-fecha-emision",
  ).value;
  const productosValidos = facturaGarantiaLineItems.filter(
    (li) => li.producto.trim() !== "",
  );

  if (!cliente || !fechaEmision) return false;
  if (productosValidos.length === 0) {
    alert("Agrega al menos un producto a la factura antes de guardarla.");
    return false;
  }

  const subtotal = productosValidos.reduce(
    (sum, li) => sum + li.cantidad * li.costo,
    0,
  );
  const descuentoPct =
    parseFloat(document.getElementById("factura-garantia-descuento").value) ||
    0;
  const impuestoPct =
    parseFloat(document.getElementById("factura-garantia-impuesto").value) || 0;
  const descuentoMonto = subtotal * (descuentoPct / 100);
  const total = (subtotal - descuentoMonto) * (1 + impuestoPct / 100);

  facturasGarantiaData.push({
    numero: document.getElementById("factura-garantia-numero").value,
    cliente,
    fechaEmision,
    total,
    subtotal,
    descuentoPct,
    impuestoPct,
    metodoPago: document.getElementById("factura-garantia-metodo-pago").value,
    descripcion: document
      .getElementById("factura-garantia-descripcion")
      .value.trim(),
    archivoNombre: facturaGarantiaArchivoActual
      ? facturaGarantiaArchivoActual.name
      : null,
    estado: document.getElementById("factura-garantia-estado").value,
    productos: productosValidos.map((li) => li.producto),
    lineas: productosValidos.map((li) => ({
      producto: li.producto,
      cantidad: li.cantidad,
      unidad: li.unidad,
      costo: li.costo,
    })),
  });
  nextFacturaGarantiaSeq++;
  notificarFactura(
    "Garantí­a",
    document.getElementById("factura-garantia-numero").value,
    cliente,
    total,
    document.getElementById("factura-garantia-estado").value,
  );

  cancelarFacturaGarantiaForm();
  renderFacturasGarantia();
  return false;
}

// =====================================================
// =============== Facturas de Pérdidas ================
// =====================================================
let facturasPerdidasData = [
  {
    numero: "P1.001",
    producto: "Mouse Inalámbrico",
    cantidad: 3,
    costo: 15,
    fecha: "2027-01-05",
    descripcion: "Dañados durante el transporte desde el almacén.",
  },
  {
    numero: "P1.002",
    producto: 'Monitor 24"',
    cantidad: 1,
    costo: 90,
    fecha: "2027-01-09",
    descripcion: "Pantalla rota por caí­da accidental.",
  },
];
let nextFacturaPerdidaSeq = 3;
let editingFacturaPerdidaId = null;

function renderFacturasPerdidas() {
  const body = document.getElementById("facturas-perdidas-body");
  if (!body) return;
  const query = (
    document.getElementById("factura-perdidas-search")?.value || ""
  ).toLowerCase();
  const rows = ordenarPorFecha(
    facturasPerdidasData.filter(
      (f) =>
        f.producto.toLowerCase().includes(query) ||
        f.numero.toLowerCase().includes(query),
    ),
    "facturasPerdidas",
    "fecha",
  );

  if (rows.length === 0) {
    body.innerHTML =
      '<tr><td colspan="6" class="empty-row">No se encontraron facturas que coincidan con la búsqueda.</td></tr>';
    return;
  }

  body.innerHTML = rows
    .map(
      (f) => `
                <tr class="cat-row" onclick="verFacturaPerdida('${f.numero}')" style="cursor:pointer">
                    <td data-label="No. Factura">${f.numero}</td>
                    <td data-label="Producto">${f.producto}</td>
                    <td data-label="Cantidad" style="text-align:center">${f.cantidad}</td>
                    <td data-label="Costo Asociado" style="text-align:right">${f.costo.toLocaleString("es-ES", { minimumFractionDigits: 2, maximumFractionDigits: 2 })} USD</td>
                    <td data-label="Fecha">${formatearFecha(f.fecha)}</td>
                    <td>
                        <div class="col-acciones">
                            <button class="action-link" onclick="event.stopPropagation(); alert('La edición de facturas ya guardadas estará disponible próximamente.')">
                                <svg class="icon" style="width:16px;height:16px"><use href="#icon-edit"></use></svg>
                                Editar
                            </button>
                            <button class="action-link danger" onclick="event.stopPropagation(); eliminarFacturaPerdida('${f.numero}')">
                                <svg class="icon" style="width:16px;height:16px"><use href="#icon-trash"></use></svg>
                                Eliminar
                            </button>
                        </div>
                    </td>
                </tr>
            `,
    )
    .join("");
}

function eliminarFacturaPerdida(numero) {
  const f = facturasPerdidasData.find((x) => x.numero === numero);
  if (!f) return;
  if (
    !confirm(
      `Â¿Eliminar la factura "${f.numero}" de ${f.producto}? Esta acción no se puede deshacer.`,
    )
  )
    return;
  facturasPerdidasData = facturasPerdidasData.filter(
    (x) => x.numero !== numero,
  );
  renderFacturasPerdidas();
}

function showFacturaPerdidasTab(tab, btn) {
  document
    .querySelectorAll("#page-facturas-perdidas .config-subview")
    .forEach((v) => v.classList.remove("active"));
  document.getElementById("factura-perdidas-" + tab).classList.add("active");
  document
    .querySelectorAll("#page-facturas-perdidas .view-tabs .tab")
    .forEach((t) => t.classList.remove("active"));
  if (btn) btn.classList.add("active");
  document.getElementById("factura-perdidas-search-bar").style.display =
    tab === "list" ? "flex" : "none";
  if (tab === "create" && !editingFacturaPerdidaId)
    prepararNuevaFacturaPerdida();
}

let facturaPerdidaVerNumero = null;

function verFacturaPerdida(numero) {
  const f = facturasPerdidasData.find((x) => x.numero === numero);
  if (!f) return;
  facturaPerdidaVerNumero = numero;

  const fmtP = (n) =>
    n.toLocaleString("es-ES", {
      minimumFractionDigits: 2,
      maximumFractionDigits: 2,
    }) + " USD";
  document.getElementById("factura-perdida-ver-titulo").textContent =
    "Factura de Pérdidas " + f.numero;
  document.getElementById("factura-perdida-ver-numero-badge").textContent =
    f.numero;
  document.getElementById("factura-perdida-ver-producto-tag").textContent =
    f.producto;
  document.getElementById("factura-perdida-ver-costo-total").textContent = fmtP(
    f.cantidad * f.costo,
  );
  document.getElementById("factura-perdida-ver-cantidad").textContent =
    f.cantidad;
  document.getElementById("factura-perdida-ver-producto").textContent =
    f.producto;
  document.getElementById("factura-perdida-ver-costo").textContent = fmtP(
    f.costo,
  );
  document.getElementById("factura-perdida-ver-fecha").textContent =
    formatearFecha(f.fecha);
  document.getElementById("factura-perdida-ver-descripcion").textContent =
    f.descripcion || ":";
  document.getElementById("factura-perdida-ver-desc-section").open =
    !!f.descripcion;

  showFacturaPerdidasTab("ver", null);
}

function eliminarFacturaPerdidaDesdeVer() {
  if (!facturaPerdidaVerNumero) return;
  const numero = facturaPerdidaVerNumero;
  const f = facturasPerdidasData.find((x) => x.numero === numero);
  if (!f) return;
  if (
    !confirm(
      `Â¿Eliminar la factura "${f.numero}" de ${f.producto}? Esta acción no se puede deshacer.`,
    )
  )
    return;
  facturasPerdidasData = facturasPerdidasData.filter(
    (x) => x.numero !== numero,
  );
  facturaPerdidaVerNumero = null;
  renderFacturasPerdidas();
  showFacturaPerdidasTab(
    "list",
    document.getElementById("tab-factura-perdidas-list"),
  );
}

function exportarFacturaPerdidaPDF(numero) {
  const f = facturasPerdidasData.find((x) => x.numero === numero);
  if (!f || !window.jspdf) return;
  const { jsPDF } = window.jspdf;
  const doc = new jsPDF();
  let y = 20;
  doc.setFontSize(16);
  doc.text(`Factura de Pérdidas ${f.numero}`, 14, y);
  y += 12;
  doc.setFontSize(11);
  doc.text(`Producto: ${f.producto}`, 14, y);
  y += 7;
  doc.text(`Cantidad de Pérdidas: ${f.cantidad}`, 14, y);
  y += 7;
  doc.text(`Costo Asociado: ${f.costo.toFixed(2)} USD`, 14, y);
  y += 7;
  doc.text(`Costo Total: ${(f.cantidad * f.costo).toFixed(2)} USD`, 14, y);
  y += 7;
  doc.text(`Fecha: ${formatearFecha(f.fecha)}`, 14, y);
  y += 10;

  if (f.descripcion) {
    doc.setFontSize(12);
    doc.text("Descripción del Motivo de la Pérdida", 14, y);
    y += 8;
    doc.setFontSize(10);
    const lineas = doc.splitTextToSize(f.descripcion, 180);
    doc.text(lineas, 14, y);
  }

  doc.save(`Factura_${f.numero}.pdf`);
}

function prepararNuevaFacturaPerdida() {
  editingFacturaPerdidaId = null;
  document.getElementById("factura-perdida-form-title").textContent =
    "Nueva Factura de Pérdidas";
  document.getElementById("factura-perdida-producto").value = "";
  document.getElementById("factura-perdida-cantidad").value = 1;
  document.getElementById("factura-perdida-costo").value = "";
  document.getElementById("factura-perdida-fecha").value = new Date()
    .toISOString()
    .slice(0, 10);
  document.getElementById("factura-perdida-descripcion").value = "";
}

function cancelarFacturaPerdidasForm() {
  editingFacturaPerdidaId = null;
  showFacturaPerdidasTab(
    "list",
    document.getElementById("tab-factura-perdidas-list"),
  );
}

function guardarFacturaPerdida(e) {
  e.preventDefault();
  const producto = document
    .getElementById("factura-perdida-producto")
    .value.trim();
  const cantidad =
    parseFloat(document.getElementById("factura-perdida-cantidad").value) || 0;
  const costo =
    parseFloat(document.getElementById("factura-perdida-costo").value) || 0;
  const fecha = document.getElementById("factura-perdida-fecha").value;

  if (!producto || !fecha || cantidad <= 0) return false;

  facturasPerdidasData.push({
    numero: "P1." + String(nextFacturaPerdidaSeq).padStart(3, "0"),
    producto,
    cantidad,
    costo,
    fecha,
    descripcion: document
      .getElementById("factura-perdida-descripcion")
      .value.trim(),
  });
  nextFacturaPerdidaSeq++;
  crearNotificacion(
    "factura",
    "Pérdida Registrada",
    `Se registró una pérdida de "${producto}" (${cantidad} uds.) por un valor de ${(cantidad * costo).toLocaleString("es-ES", { minimumFractionDigits: 2 })} USD.`,
  );

  cancelarFacturaPerdidasForm();
  renderFacturasPerdidas();
  return false;
}

function renderMonedas() {
  const grid = document.getElementById("currency-grid");
  if (!grid || grid.dataset.rendered) return;
  grid.innerHTML = monedasData
    .map(
      (m, i) => `
                <label class="currency-item">
                    <span class="code"><span class="currency-flag">${m.codigo.slice(0, 3)}</span>${m.codigo}</span>
                    <span class="switch">
                        <input type="checkbox" ${m.activo ? "checked" : ""} onchange="monedasData[${i}].activo = this.checked">
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
                        <input type="text" placeholder="0.000" oninput="actualizarEquivalencia(this)">
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
// =============== Facturas - Vista General ============
// =====================================================
const facturaTipoInfo = {
  Entrada: {
    page: "facturas-entrada",
    searchId: "factura-entrada-search",
    tabListId: "tab-factura-entrada-list",
    render: () => renderFacturasEntrada(),
  },
  Venta: {
    page: "facturas-venta",
    searchId: "factura-venta-search",
    tabListId: "tab-factura-venta-list",
    render: () => renderFacturasVenta(),
  },
  Servicio: {
    page: "facturas-servicios",
    searchId: "factura-servicios-search",
    tabListId: "tab-factura-servicios-list",
    render: () => renderFacturasServicios(),
  },
  "Garantí­a": {
    page: "facturas-garantia",
    searchId: "factura-garantia-search",
    tabListId: "tab-factura-garantia-list",
    render: () => renderFacturasGarantia(),
  },
  Insumo: {
    page: "facturas-insumos",
    searchId: "factura-insumos-search",
    tabListId: "tab-factura-insumos-list",
    render: () => renderFacturasInsumos(),
  },
  Pérdida: {
    page: "facturas-perdidas",
    searchId: "factura-perdidas-search",
    tabListId: "tab-factura-perdidas-list",
    render: () => renderFacturasPerdidas(),
  },
};

function obtenerFacturasCombinadas() {
  const combinadas = [];
  facturasEntradaData.forEach((f) =>
    combinadas.push({
      numero: f.numero,
      tipo: "Entrada",
      tercero: f.proveedor,
      fecha: f.fechaEmision,
      total: f.total,
      estado: f.estado,
    }),
  );
  facturasVentaData.forEach((f) =>
    combinadas.push({
      numero: f.numero,
      tipo: "Venta",
      tercero: f.cliente,
      fecha: f.fechaEmision,
      total: f.total,
      estado: f.estado,
    }),
  );
  facturasServiciosData.forEach((f) =>
    combinadas.push({
      numero: f.numero,
      tipo: "Servicio",
      tercero: f.cliente,
      fecha: f.fechaEmision,
      total: f.total,
      estado: f.estado,
    }),
  );
  facturasInsumosData.forEach((f) =>
    combinadas.push({
      numero: f.numero,
      tipo: "Insumo",
      tercero: f.cliente,
      fecha: f.fechaEmision,
      total: f.total,
      estado: f.estado,
    }),
  );
  facturasGarantiaData.forEach((f) =>
    combinadas.push({
      numero: f.numero,
      tipo: "Garantí­a",
      tercero: f.cliente,
      fecha: f.fechaEmision,
      total: f.total,
      estado: f.estado,
    }),
  );
  facturasPerdidasData.forEach((f) =>
    combinadas.push({
      numero: f.numero,
      tipo: "Pérdida",
      tercero: f.producto,
      fecha: f.fecha,
      total: f.cantidad * f.costo,
      estado: "N/A",
    }),
  );
  return ordenarPorFecha(combinadas, "facturasGeneral", "fecha");
}

function renderFacturasGeneral() {
  const body = document.getElementById("facturas-general-body");
  if (!body) return;
  const query = (
    document.getElementById("factura-general-search")?.value || ""
  ).toLowerCase();
  const tipoFiltro =
    document.getElementById("factura-general-tipo")?.value || "";
  const estadoFiltro =
    document.getElementById("factura-general-estado")?.value || "";

  const rows = obtenerFacturasCombinadas().filter((f) => {
    const coincideBusqueda =
      f.numero.toLowerCase().includes(query) ||
      (f.tercero || "").toLowerCase().includes(query);
    const coincideTipo = !tipoFiltro || f.tipo === tipoFiltro;
    const coincideEstado = !estadoFiltro || f.estado === estadoFiltro;
    return coincideBusqueda && coincideTipo && coincideEstado;
  });

  if (rows.length === 0) {
    body.innerHTML =
      '<tr><td colspan="7" class="empty-row">No se encontraron facturas que coincidan con los filtros.</td></tr>';
    return;
  }

  const estadoClase = {
    "Por Pagar": "status-pendiente",
    Pagada: "status-pagada",
    Vencida: "status-vencida",
  };
  body.innerHTML = rows
    .map(
      (f) => `
                <tr>
                    <td data-label="No. Factura">${f.numero}</td>
                    <td data-label="Tipo">${f.tipo}</td>
                    <td data-label="Cliente / Proveedor">${f.tercero || ":"}</td>
                    <td data-label="Fecha">${formatearFecha(f.fecha)}</td>
                    <td data-label="Total" style="text-align:right">${(f.total || 0).toLocaleString("es-ES", { minimumFractionDigits: 2, maximumFractionDigits: 2 })} USD</td>
                    <td data-label="Estado">${f.estado === "N/A" ? ":" : `<span class="status-badge ${estadoClase[f.estado] || ""}">${f.estado}</span>`}</td>
                    <td>
                        <div class="col-acciones">
                            <button class="action-link" onclick="irAFacturaDesdeGeneral('${f.tipo}', '${f.numero}')">
                                <svg class="icon" style="width:16px;height:16px"><use href="#icon-eye"></use></svg>
                                Ver
                            </button>
                        </div>
                    </td>
                </tr>
            `,
    )
    .join("");
}

function irAFacturaDesdeGeneral(tipo, numero) {
  const info = facturaTipoInfo[tipo];
  if (!info) return;
  showPage(
    info.page,
    document.querySelector(`#sidebar a[onclick*="'${info.page}'"]`),
  );
  const tabBtn = document.getElementById(info.tabListId);
  const tabFn = {
    "facturas-entrada": showFacturaEntradaTab,
    "facturas-venta": showFacturaVentaTab,
    "facturas-servicios": showFacturaServiciosTab,
    "facturas-garantia": showFacturaGarantiaTab,
    "facturas-insumos": showFacturaInsumosTab,
    "facturas-perdidas": showFacturaPerdidasTab,
  }[info.page];
  if (tabFn && tabBtn) tabFn("list", tabBtn);
  const searchInput = document.getElementById(info.searchId);
  if (searchInput) {
    searchInput.value = numero;
  }
  info.render();
}

// =====================================================
// ===================== Notificaciones =================
// =====================================================
let notificacionesData = [];
let nextNotificacionId = 1;
let notifTabActual = "todas";

const notifIconos = {
  factura: "icon-receipt",
  pago: "icon-alert",
  tasa: "icon-coins",
  usuario: "icon-user-circle",
};

function crearNotificacion(tipo, titulo, descripcion, fechaISO) {
  const ahora = new Date();
  notificacionesData.unshift({
    id: nextNotificacionId++,
    tipo,
    titulo,
    descripcion,
    fechaISO: fechaISO || ahora.toISOString().slice(0, 10),
    hora: ahora.toLocaleTimeString("es-ES", {
      hour: "2-digit",
      minute: "2-digit",
    }),
    leida: false,
  });
  renderNotificaciones();
  actualizarBadgesNotificaciones();
}

// Genera la notificación correspondiente al guardar una factura de cualquier tipo:
// si el estado elegido es "Vencida" se notifica como pago vencido, si no, como factura registrada.
function notificarFactura(tipoFactura, numero, tercero, total, estado) {
  const totalFmt = (total || 0).toLocaleString("es-ES", {
    minimumFractionDigits: 2,
    maximumFractionDigits: 2,
  });
  if (estado === "Vencida") {
    crearNotificacion(
      "pago",
      "Pago Vencido",
      `La factura de ${tipoFactura} ${numero} de "${tercero}" (${totalFmt} USD) está marcada como vencida.`,
    );
  } else {
    crearNotificacion(
      "factura",
      `Factura de ${tipoFactura} Registrada`,
      `Se registró la factura ${numero} de "${tercero}" por ${totalFmt} USD.`,
    );
  }
}

function guardarTasasCambioDemo() {
  crearNotificacion(
    "tasa",
    "Tasa de Cambio Actualizada",
    "Se guardaron nuevas tasas de cambio para las monedas activas del sistema.",
  );
  alert("Tasas de cambio guardadas (demo).");
}

function showNotifTab(tab, btn) {
  notifTabActual = tab;
  document
    .querySelectorAll("#page-notificaciones .view-tabs .tab")
    .forEach((t) => t.classList.remove("active"));
  btn.classList.add("active");
  renderNotificaciones();
}

function marcarNotificacionLeida(id) {
  const n = notificacionesData.find((x) => x.id === id);
  if (n && !n.leida) {
    n.leida = true;
    renderNotificaciones();
    actualizarBadgesNotificaciones();
  }
}

function marcarTodasNotificacionesLeidas() {
  notificacionesData.forEach((n) => (n.leida = true));
  renderNotificaciones();
  actualizarBadgesNotificaciones();
}

function actualizarBadgesNotificaciones() {
  const noLeidas = notificacionesData.filter((n) => !n.leida).length;
  const sidebarBadge = document.getElementById("sidebar-notif-badge");
  const topbarBadge = document.getElementById("topbar-notif-badge");
  const tabBadge = document.getElementById("notif-unread-count");
  [sidebarBadge, topbarBadge, tabBadge].forEach((el) => {
    if (!el) return;
    if (noLeidas > 0) {
      el.textContent = noLeidas > 99 ? "99+" : noLeidas;
      el.style.display = "";
    } else {
      el.style.display = "none";
    }
  });
}

function formatearFechaHoraNotif(fechaISO) {
  const hoy = new Date().toISOString().slice(0, 10);
  const ayer = new Date(Date.now() - 86400000).toISOString().slice(0, 10);
  if (fechaISO === hoy) return "Hoy";
  if (fechaISO === ayer) return "Ayer";
  return formatearFecha(fechaISO);
}

function renderNotificaciones() {
  const cont = document.getElementById("notificaciones-list");
  if (!cont) return;
  const query = (
    document.getElementById("notificacion-search")?.value || ""
  ).toLowerCase();
  const fechaFiltro =
    document.getElementById("notificacion-fecha")?.value || "";

  const rows = notificacionesData.filter((n) => {
    const coincideTab = notifTabActual === "todas" || !n.leida;
    const coincideBusqueda =
      n.titulo.toLowerCase().includes(query) ||
      n.descripcion.toLowerCase().includes(query);
    const coincideFecha = !fechaFiltro || n.fechaISO === fechaFiltro;
    return coincideTab && coincideBusqueda && coincideFecha;
  });

  if (rows.length === 0) {
    cont.innerHTML =
      '<div class="empty-row">No hay notificaciones que coincidan con los filtros.</div>';
    return;
  }

  cont.innerHTML = rows
    .map(
      (n) => `
                <div class="notif-row notif-tipo-${n.tipo} ${n.leida ? "" : "no-leida"}" onclick="marcarNotificacionLeida(${n.id})">
                    <div class="notif-icon">
                        <svg class="icon" style="width:26px;height:26px"><use href="#${notifIconos[n.tipo] || "icon-bell"}"></use></svg>
                    </div>
                    <div class="notif-body">
                        <div class="notif-title">${n.titulo} ${n.leida ? "" : '<span class="notif-dot"></span>'}</div>
                        <div class="notif-desc">${n.descripcion}</div>
                    </div>
                    <div class="notif-time">${formatearFechaHoraNotif(n.fechaISO)} Â· ${n.hora}</div>
                </div>
            `,
    )
    .join("");
}

// =====================================================
// ========================= Perfil =====================
// =====================================================
// Estado inicial
actualizarProductosDisponibles();
actualizarProveedoresDisponibles();
renderProductos();
renderProveedores();
renderProductPicker([]);
renderCategorias();
renderClientes();
renderUnidades();
renderConversiones();
poblarSelectoresConversor();
actualizarConversorRapido();
poblarDatalists();
renderFacturasEntrada();
renderFacturasVenta();
renderFacturasServicios();
renderFacturasInsumos();
renderFacturasGarantia();
renderFacturasPerdidas();
renderFacturasGeneral();
inicializarIconosOrdenFecha();

// Notificaciones iniciales: se generan a partir de facturas que ya nacen "Vencida" en los datos de ejemplo
[
  ...facturasEntradaData.map((f) => ({
    ...f,
    tipo: "Entrada",
    tercero: f.proveedor,
  })),
  ...facturasVentaData.map((f) => ({
    ...f,
    tipo: "Venta",
    tercero: f.cliente,
  })),
  ...facturasServiciosData.map((f) => ({
    ...f,
    tipo: "Servicio",
    tercero: f.cliente,
  })),
  ...facturasInsumosData.map((f) => ({
    ...f,
    tipo: "Insumo",
    tercero: f.cliente,
  })),
  ...facturasGarantiaData.map((f) => ({
    ...f,
    tipo: "Garantí­a",
    tercero: f.cliente,
  })),
]
  .filter((f) => f.estado === "Vencida")
  .forEach((f) => {
    crearNotificacion(
      "pago",
      "Pago Vencido",
      `La factura de ${f.tipo} ${f.numero} de "${f.tercero}" (${(f.total || 0).toLocaleString("es-ES", { minimumFractionDigits: 2 })} USD) está marcada como vencida.`,
      f.fechaEmision,
    );
  });
crearNotificacion(
  "factura",
  "Bienvenido al Sistema",
  "Este es tu panel de notificaciones. Aquí­ verás avisos de facturas, pagos vencidos, tasas de cambio y nuevos usuarios.",
);
notificacionesData.forEach((n, i) => {
  if (i > 0) n.leida = true;
}); // solo la más reciente queda como no leí­da al iniciar
actualizarBadgesNotificaciones();
renderNotificaciones();

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

  const aviso = document.getElementById("config-paginacion-feedback");
  if (aviso) {
    aviso.textContent = `Se mostrarán ${nuevo} elementos por página en todas las tablas.`;
    aviso.classList.add("is-visible");
    clearTimeout(cambiarTamPagina._t);
    cambiarTamPagina._t = setTimeout(
      () => aviso.classList.remove("is-visible"),
      2600,
    );
  }
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
    pager.innerHTML = "";
    pager.hidden = true;
    return;
  }
  pager.hidden = false;

  const desde = inicio + 1;
  const hasta = Math.min(inicio + PAGINACION_TAM, total);
  const botones = numerosPaginacion(pagina, totalPaginas)
    .map((n) => {
      if (n === "…") return '<span class="pager-gap">…</span>';
      return (
        `<button type="button" class="pager-num ${n === pagina ? "is-active" : ""}" ` +
        `onclick="irAPagina('${cfg.id}', ${n})" ${n === pagina ? 'aria-current="page"' : ""}>${n}</button>`
      );
    })
    .join("");

  pager.innerHTML = `
                <span class="pager-info">Mostrando ${desde}–${hasta} de ${total}</span>
                <div class="pager-controls">
                    <button type="button" class="pager-btn" onclick="irAPagina('${cfg.id}', ${pagina - 1})" ${pagina === 1 ? "disabled" : ""} aria-label="Página anterior">
                        <svg class="icon" style="width:16px;height:16px" aria-hidden="true"><use href="#icon-chevron-left"></use></svg>
                        Anterior
                    </button>
                    ${botones}
                    <button type="button" class="pager-btn" onclick="irAPagina('${cfg.id}', ${pagina + 1})" ${pagina === totalPaginas ? "disabled" : ""} aria-label="Página siguiente">
                        Siguiente
                        <svg class="icon pager-chevron-next" style="width:16px;height:16px" aria-hidden="true"><use href="#icon-chevron-left"></use></svg>
                    </button>
                </div>
            `;
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
// Solo se exponen dos funciones globales (las usan los onclick del HTML):
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

  // El estado del inventario es una "foto" de hoy: no cambia con el período.
  const INVENTARIO_EJEMPLO = {
    agotados: {
      total: 5,
      sinCategoria: 4,
      categorias: ["Accesorios", "Laptop", "Muebles"],
    },
    lentoMovimiento: [
      { nombre: "Silla de Oficina", dias: 125, stock: 2 },
      { nombre: "Impresora HP Láser", dias: 93, stock: 8 },
      { nombre: "Soporte para Laptop", dias: 60, stock: 25 },
      { nombre: "Cámara Web", dias: 29, stock: 7 },
    ],
  };

  /* ---------- 2. Conexión con el backend ----------
             Cuando tengas API, reemplaza el cuerpo de estas funciones por un fetch que devuelva
             la misma forma de objeto, por ejemplo:
               return fetch('/api/dashboard?periodo=' + periodo).then(r => r.json()); */
  async function obtenerDatosDashboard(periodo) {
    return DATOS_EJEMPLO[periodo];
  }

  async function obtenerInventarioDashboard() {
    return INVENTARIO_EJEMPLO;
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

  function renderInventario(inv) {
    const a = inv.agotados;
    $("#dash-agotados-total").textContent = a.total;
    $("#dash-agotados-categorias").innerHTML = a.categorias
      .slice(0, 3)
      .map((c) => `<span>${esc(c)}</span>`)
      .join("");
    $("#dash-agotados-sin-categoria").textContent = a.sinCategoria;
    $("#dash-lento").innerHTML = inv.lentoMovimiento
      .map((p) => {
        const clase =
          p.dias >= 100
            ? ' class="days-red"'
            : p.dias >= 60
              ? ' class="days-orange"'
              : "";
        return `<tr><td>${esc(p.nombre)}</td><td${clase}>${p.dias}</td><td>${miles(p.stock)}</td></tr>`;
      })
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
    obtenerInventarioDashboard()
      .then(renderInventario)
      .catch(() => {});
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
