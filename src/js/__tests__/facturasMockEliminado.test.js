import { beforeAll, describe, expect, it, onTestFinished, vi } from "vitest";
import { readFile } from "node:fs/promises";
import path from "node:path";

vi.mock("../services/invoicesService.js", () => ({
  listInvoices: vi.fn(() => Promise.resolve([])),
  getInvoice: vi.fn(),
  createEntryInvoice: vi.fn(),
  createSaleInvoice: vi.fn(),
  createServiceInvoice: vi.fn(),
  createGuaranteeInvoice: vi.fn(),
  createInstallationInvoice: vi.fn(),
  createLossInvoice: vi.fn(),
  saveInvoiceAttachment: vi.fn(),
  downloadInvoiceAttachment: vi.fn(),
  deleteInvoice: vi.fn(),
  changeInvoiceStatus: vi.fn(),
}));

vi.mock("../services/clientsService.js", () => ({
  searchClients: vi.fn(() => Promise.resolve([])),
  getClientDetail: vi.fn(),
}));

vi.mock("../services/suppliersService.js", () => ({
  searchSuppliers: vi.fn(() => Promise.resolve([])),
}));

vi.mock("../components/confirmDialog.js", () => ({
  confirmDialog: vi.fn(() => Promise.resolve(true)),
}));

// Identificadores de spec-03 §9.X.4.a de las facturas de salida.
const REMOVED = [
  "facturasVentaData",
  "nextFacturaVentaSeq",
  "editingFacturaVentaId",
  "facturaVentaLineItems",
  "nextVentaLineItemId",
  "eliminarFacturaVenta",
  "facturaVentaVerNumero",
  "eliminarFacturaVentaDesdeVer",
  "prepararNuevaFacturaVenta",
  "cancelarFacturaVentaForm",
  "renderLineasVenta",
  "recalcularTotalesVenta",
  "facturasServiciosData",
  "nextFacturaServicioSeq",
  "editingFacturaServicioId",
  "facturaServicioLineItems",
  "nextServicioLineItemId",
  "eliminarFacturaServicio",
  "facturaServicioVerNumero",
  "eliminarFacturaServicioDesdeVer",
  "prepararNuevaFacturaServicio",
  "cancelarFacturaServiciosForm",
  "agregarLineaServicio",
  "agregarLineaDesdeBusquedaServicio",
  "crearInsumoServicioRapido",
  "quitarLineaServicio",
  "actualizarLineaServicio",
  "renderLineasServicio",
  "recalcularTotalesServicio",
  "facturasInsumosData",
  "nextFacturaInsumoSeq",
  "editingFacturaInsumoId",
  "facturaInsumoLineItems",
  "nextInsumoLineItemId",
  "facturaInsumoArchivoActual",
  "insumosDisponibles",
  "eliminarFacturaInsumo",
  "facturaInsumoVerNumero",
  "eliminarFacturaInsumoDesdeVer",
  "prepararNuevaFacturaInsumo",
  "cancelarFacturaInsumosForm",
  "agregarLineaInsumo",
  "agregarLineaDesdeBusquedaInsumo",
  "crearInsumoRapido",
  "quitarLineaInsumo",
  "actualizarLineaInsumo",
  "renderLineasInsumo",
  "recalcularTotalesInsumo",
  "manejarArchivoInsumo",
  "manejarDropArchivoInsumo",
  "quitarArchivoInsumo",
  "facturasGarantiaData",
  "nextFacturaGarantiaSeq",
  "editingFacturaGarantiaId",
  "facturaGarantiaLineItems",
  "nextGarantiaLineItemId",
  "facturaGarantiaArchivoActual",
  "eliminarFacturaGarantia",
  "facturaGarantiaVerNumero",
  "eliminarFacturaGarantiaDesdeVer",
  "prepararNuevaFacturaGarantia",
  "cancelarFacturaGarantiaForm",
  "agregarLineaGarantia",
  "agregarLineaDesdeBusquedaGarantia",
  "crearProductoGarantiaRapido",
  "quitarLineaGarantia",
  "actualizarLineaGarantia",
  "renderLineasGarantia",
  "recalcularTotalesGarantia",
  "manejarArchivoGarantia",
  "manejarDropArchivoGarantia",
  "quitarArchivoGarantia",
  "facturasPerdidasData",
  "nextFacturaPerdidaSeq",
  "editingFacturaPerdidaId",
  "eliminarFacturaPerdida",
  "facturaPerdidaVerNumero",
  "eliminarFacturaPerdidaDesdeVer",
  "prepararNuevaFacturaPerdida",
  "cancelarFacturaPerdidasForm",
  "facturaTipoInfo",
  "obtenerFacturasCombinadas",
  "opcionesUnidadHTML",
  "notificarFactura",
];

// Nombres que facturasView.js expone en window: dashboard.js ya no los declara.
const NOW_IN_VIEW = [
  "renderFacturasVenta",
  "verFacturaVenta",
  "showFacturaVentaTab",
  "guardarFacturaVenta",
  "renderFacturasServicios",
  "verFacturaServicio",
  "showFacturaServiciosTab",
  "guardarFacturaServicio",
  "renderFacturasInsumos",
  "verFacturaInsumo",
  "showFacturaInsumosTab",
  "guardarFacturaInsumo",
  "renderFacturasGarantia",
  "verFacturaGarantia",
  "showFacturaGarantiaTab",
  "guardarFacturaGarantia",
  "renderFacturasPerdidas",
  "verFacturaPerdida",
  "showFacturaPerdidasTab",
  "guardarFacturaPerdida",
  "renderFacturasGeneral",
  "irAFacturaDesdeGeneral",
];

let dashboardSource = "";
let html = "";

beforeAll(async () => {
  const root = path.resolve(process.cwd(), "src");
  dashboardSource = await readFile(
    path.join(root, "js", "dashboard.js"),
    "utf8",
  );
  html = await readFile(path.join(root, "dashboard-unificado.html"), "utf8");
});

describe("mock de facturas de salida eliminado (tarea 4.5.9)", () => {
  it("dashboard.js no contiene ninguno de los identificadores del mock", () => {
    for (const name of REMOVED) {
      expect(dashboardSource, name).not.toMatch(new RegExp(`\\b${name}\\b`));
    }
    for (const name of NOW_IN_VIEW) {
      expect(dashboardSource, name).not.toMatch(
        new RegExp(`function ${name}\\(`),
      );
    }
  });

  it("no queda el bloque final de notificaciones simuladas ni avisos de facturas", () => {
    expect(dashboardSource).not.toContain("Bienvenido al Sistema");
    expect(dashboardSource).not.toContain("Pago Vencido");
    expect(dashboardSource).not.toMatch(
      /crearNotificacion\(\s*["']factura["']/,
    );
    // crearNotificacion se conserva para la página de notificaciones.
    expect(dashboardSource).toMatch(/^function crearNotificacion\(/m);
  });

  it("las páginas de facturas del HTML no tienen manejadores inline", () => {
    const doc = new DOMParser().parseFromString(html, "text/html");
    for (const page of doc.querySelectorAll('[id^="page-facturas-"]')) {
      expect(
        page.querySelector(
          "[onclick],[oninput],[onchange],[onsubmit],[ondrop]",
        ),
        page.id,
      ).toBeNull();
    }
  });

  it("dashboard.js carga sin errores y la lista general queda vacía con la BD vacía", async () => {
    const doc = new DOMParser().parseFromString(html, "text/html");
    document.body.innerHTML = doc.body.innerHTML;
    // Con isolate:false, los globales que crea el script (showPage, …)
    // llegarían a los demás archivos de test: se quitan al terminar.
    const globalsBefore = new Set(Object.getOwnPropertyNames(window));
    // Igual con los listeners delegados que el script pone en document.
    const documentListeners = [];
    const addListener = document.addEventListener;
    document.addEventListener = function (...args) {
      documentListeners.push(args);
      return addListener.apply(this, args);
    };
    onTestFinished(() => {
      document.addEventListener = addListener;
      for (const args of documentListeners) {
        document.removeEventListener(...args);
      }
      for (const name of Object.getOwnPropertyNames(window)) {
        if (!globalsBefore.has(name)) delete window[name];
      }
      document.body.innerHTML = "";
    });
    // APIs del navegador que jsdom no trae (Chart.js, matchMedia).
    window.Chart = class {
      static defaults = { font: {} };
      constructor(_context, config = {}) {
        this.data = config.data ?? { labels: [], datasets: [] };
        this.options = config.options ?? {};
      }
      destroy() {}
      update() {}
    };
    window.matchMedia = () => ({
      matches: false,
      addEventListener() {},
      removeEventListener() {},
      addListener() {},
      removeListener() {},
    });
    // El vm de los tests no admite import() dinámico en código evaluado: la
    // sesión queda pendiente, como antes de que responda el backend.
    window.__pendingImport = () => new Promise(() => {});
    const source = dashboardSource.replace(
      /\bimport\(/g,
      "window.__pendingImport(",
    );
    // Script clásico: se ejecuta con alcance global como lo hace el navegador.
    expect(() => window.eval(source)).not.toThrow();

    vi.resetModules();
    const { initFacturasView } = await import("../views/facturasView.js");
    await initFacturasView();
    expect(
      document.getElementById("facturas-general-body").textContent,
    ).toContain("Aún no hay facturas");
    // Parsea y evalúa el HTML y dashboard.js completos: con la máquina cargada
    // supera los 5 s por defecto.
  }, 30000);
});
