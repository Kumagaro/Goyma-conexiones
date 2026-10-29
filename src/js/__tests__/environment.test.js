import { readdir, readFile } from "node:fs/promises";
import path from "node:path";
import { describe, expect, it } from "vitest";

const sourceRoot = path.resolve(process.cwd(), "src");

async function collectSourceFiles(directory) {
  const entries = await readdir(directory, { withFileTypes: true });
  const nestedFiles = await Promise.all(
    entries.map(async (entry) => {
      const entryPath = path.join(directory, entry.name);
      if (entry.isDirectory()) return collectSourceFiles(entryPath);
      return /\.(?:html|js)$/.test(entry.name) ? [entryPath] : [];
    }),
  );
  return nestedFiles.flat();
}

describe("Vitest con jsdom", () => {
  it("proporciona un documento DOM para las pruebas de interfaz", () => {
    expect(document.createElement("form")).toBeInstanceOf(HTMLFormElement);
  });

  it("no guarda estado de autenticación en storage ni conserva el mock de usuarios", async () => {
    const sourceFiles = await collectSourceFiles(sourceRoot);
    const productionFiles = sourceFiles.filter(
      (filePath) => !filePath.includes(`${path.sep}__tests__${path.sep}`),
    );
    const contents = await Promise.all(
      productionFiles.map((filePath) => readFile(filePath, "utf8")),
    );
    const source = contents.join("\n");
    const dashboard = await readFile(
      path.join(sourceRoot, "js", "dashboard.js"),
      "utf8",
    );

    expect(source).not.toMatch(
      /(?:localStorage|sessionStorage)\.(?:getItem|setItem|removeItem)\(\s*["'](?:username|userRole|sessionToken)["']/,
    );
    expect(source).not.toMatch(/\b(?:userRole|sessionToken)\b/);
    expect(dashboard).not.toMatch(
      /\b(?:usuariosData|nextUsuarioSeq|editingUsuarioId|currentUsername|currentSessionToken)\b/,
    );
    expect(dashboard).not.toMatch(
      /\b(?:clientesData|nextClienteSeq|editingClienteId|clienteVerId|proveedoresData|nextProveedorSeq|editingProveedorId|proveedorVerId|renderClientes|renderProveedores|showClienteTab|showProveedorTab|poblarDatalists|actualizarProveedoresDisponibles)\b/,
    );
    expect(dashboard).not.toMatch(
      /\b(?:facturasEntradaData|nextFacturaEntradaSeq|editingFacturaId|facturaLineItems|nextLineItemId|facturaArchivoActual|facturaVerNumero|renderFacturasEntrada|eliminarFactura|showFacturaEntradaTab|verFactura|eliminarFacturaDesdeVer|prepararNuevaFactura|cancelarFacturaEntradaForm|agregarLineaFactura|agregarLineaDesdeBusqueda|crearProductoRapido|quitarLineaFactura|actualizarLineaFactura|renderLineasFactura|recalcularTotalesFactura|manejarArchivoFactura|manejarDropArchivo|quitarArchivoFactura|guardarFactura)\b/,
    );
  });

  it("las secciones migradas de contactos y facturas no tienen manejadores inline", async () => {
    const html = await readFile(
      path.join(sourceRoot, "dashboard-unificado.html"),
      "utf8",
    );
    const doc = new DOMParser().parseFromString(html, "text/html");
    for (const id of [
      "page-clientes",
      "page-proveedores",
      "page-facturas-general",
      "page-facturas-entrada",
      "page-facturas-venta",
      "page-facturas-servicios",
      "page-facturas-insumos",
      "page-facturas-garantia",
      "page-facturas-perdidas",
    ]) {
      const page = doc.getElementById(id);
      expect(page, id).not.toBeNull();
      const inline = Array.from(page.querySelectorAll("*")).filter((el) =>
        el.getAttributeNames().some((name) => name.startsWith("on")),
      );
      expect(
        inline.map((el) => el.id || el.tagName),
        id,
      ).toEqual([]);
    }
  });

  it("mantiene protegidos los elementos de la spec 1 del dashboard", async () => {
    const dashboardHtml = await readFile(
      path.join(sourceRoot, "dashboard-unificado.html"),
      "utf8",
    );
    const dashboardJs = await readFile(
      path.join(sourceRoot, "js", "dashboard.js"),
      "utf8",
    );

    expect(dashboardHtml).toContain('id="reserved-data-warning"');
    expect(dashboardHtml).toContain('src="./js/views/usuarios.js"');
    expect(dashboardHtml).toContain('src="./js/views/perfil.js"');
    expect(dashboardJs).toMatch(/import\(\s*["']\.\/session\.js["']\s*\)/);
  });

  it("carga los módulos de spec-03 después de spec-02 y antes de dashboard.js", async () => {
    const dashboardHtml = await readFile(
      path.join(sourceRoot, "dashboard-unificado.html"),
      "utf8",
    );
    const parsed = new DOMParser().parseFromString(dashboardHtml, "text/html");
    const moduleSources = Array.from(
      parsed.querySelectorAll('script[type="module"]'),
      (script) => script.getAttribute("src"),
    );
    const featureModules = [
      "./js/views/clientesView.js",
      "./js/views/proveedoresView.js",
      "./js/views/facturasView.js",
      "./js/views/historialView.js",
    ];
    const dashboardIndex = Array.from(parsed.scripts).findIndex(
      (script) => script.getAttribute("src") === "./js/dashboard.js",
    );

    expect(moduleSources.slice(-4)).toEqual(featureModules);
    expect(dashboardIndex).toBeGreaterThan(
      Array.from(parsed.scripts).findIndex(
        (script) =>
          script.getAttribute("src") === "./js/views/historialView.js",
      ),
    );
  });

  it("showPage despacha page:change con el id de la vista destino", async () => {
    const dashboardSource = await readFile(
      path.join(sourceRoot, "js", "dashboard.js"),
      "utf8",
    );
    const knownPagesSource = dashboardSource.match(
      /const knownPages = \[[\s\S]*?\];/,
    )?.[0];
    const showPageSource = dashboardSource.match(
      /^function showPage\(pageId, el, label\) \{[\s\S]*?^\}/m,
    )?.[0];

    expect(knownPagesSource).toBeTruthy();
    expect(showPageSource).toBeTruthy();

    const originalBody = document.body.innerHTML;
    document.body.innerHTML = `
      <nav id="sidebar"><a id="clientes-link"></a></nav>
      <main class="panel">
        <section class="page" id="page-dashboard"></section>
        <section class="page" id="page-clientes"></section>
      </main>
    `;
    document.querySelector("main.panel").scrollTo = () => {};

    const showPage = new Function(
      "document",
      "window",
      "CustomEvent",
      "renderClientes",
      "renderProveedores",
      `${knownPagesSource}\n${showPageSource}\nreturn showPage;`,
    )(
      document,
      { scrollTo() {} },
      document.defaultView.CustomEvent,
      () => {},
      () => {},
    );
    const dispatchedEvents = [];
    document.addEventListener(
      "page:change",
      (event) => dispatchedEvents.push(event),
      { once: true },
    );
    // Las vistas (clientes, proveedores, facturas) escuchan en window.
    const windowEvents = [];
    document.defaultView.addEventListener(
      "page:change",
      (event) => windowEvents.push(event),
      { once: true },
    );

    try {
      showPage("clientes", document.getElementById("clientes-link"));
      expect(windowEvents).toHaveLength(1);
      expect(windowEvents[0].detail).toEqual({ pageId: "clientes" });

      expect(
        document.getElementById("page-clientes").classList.contains("active"),
      ).toBe(true);
      expect(dispatchedEvents).toHaveLength(1);
      expect(dispatchedEvents[0]).toBeInstanceOf(
        document.defaultView.CustomEvent,
      );
      expect(dispatchedEvents[0].detail).toEqual({ pageId: "clientes" });
    } finally {
      document.body.innerHTML = originalBody;
    }
  });

  it("mantiene sincronizados los comandos de servicios y el handler Tauri", async () => {
    const handlerPath = path.resolve(
      sourceRoot,
      "..",
      "src-tauri",
      "src",
      "lib.rs",
    );
    const servicesPath = path.join(sourceRoot, "js", "services");
    const viewsPath = path.join(sourceRoot, "js", "views");
    const [handlerSource, serviceEntries, viewEntries] = await Promise.all([
      readFile(handlerPath, "utf8"),
      readdir(servicesPath, { withFileTypes: true }),
      readdir(viewsPath, { withFileTypes: true }),
    ]);
    const serviceSources = await Promise.all(
      serviceEntries
        .filter((entry) => entry.isFile() && entry.name.endsWith("Service.js"))
        .map((entry) => readFile(path.join(servicesPath, entry.name), "utf8")),
    );
    const viewSources = await Promise.all(
      viewEntries
        .filter((entry) => entry.isFile() && entry.name.endsWith("View.js"))
        .map((entry) => readFile(path.join(viewsPath, entry.name), "utf8")),
    );
    const handler = handlerSource.match(
      /tauri::generate_handler!\[([\s\S]*?)\]/,
    );
    // Sin lista fija de módulos ni recuento literal: todo se deriva de
    // los archivos (tarea 3.9 de la Fase 2).
    const handlerCommands = [
      ...handler[1].matchAll(/commands::[a-z_]+::([a-z_]+)/g),
    ]
      .map(([, command]) => command)
      .sort();
    const serviceCommands = serviceSources
      .flatMap((source) => [
        ...source.matchAll(/\binvoke\(\s*["']([a-z_]+)["']/g),
      ])
      .map(([, command]) => command)
      .sort();
    const viewCommands = viewSources
      .flatMap((source) => [
        ...source.matchAll(/\binvoke\(\s*["']([a-z_]+)["']/g),
      ])
      .map(([, command]) => command)
      .sort();
    // Wrappers adelantados cuyo backend aún no está registrado. Con las
    // Fases 3 y 4 terminadas no queda ninguno; si vuelve a haberlos, se listan.
    const pendingBackend = [];
    // Comandos registrados sin wrapper dedicado todavía. Con §7.6
    // (getProductAssociatedInvoices en historyService.js) no queda ninguno.
    const pendingWrapper = [];

    expect(handler).not.toBeNull();
    expect(handlerCommands.length).toBeGreaterThan(0);
    // Los pendientes siguen pendientes a ambos lados (la lista se
    // mantiene sola: si una fase los resuelve, este test lo exige).
    expect(
      pendingBackend.filter((command) => !serviceCommands.includes(command)),
    ).toEqual([]);
    expect(
      pendingBackend.filter((command) => handlerCommands.includes(command)),
    ).toEqual([]);
    expect(
      pendingWrapper.filter((command) => !handlerCommands.includes(command)),
    ).toEqual([]);
    expect(
      pendingWrapper.filter(
        (command) =>
          serviceCommands.includes(command) || viewCommands.includes(command),
      ),
    ).toEqual([]);
    // Fuera de los pendientes, llamadores (servicios + vistas) y handler
    // coinciden exacto.
    const without = (commands, pending) =>
      commands.filter((command) => !pending.includes(command));
    const callerCommands = [...serviceCommands, ...viewCommands].sort();
    expect(without(callerCommands, pendingBackend)).toEqual(
      without(handlerCommands, pendingWrapper),
    );
    // Todo comando registrado tiene al menos un llamador (servicio o vista).
    const called = new Set(callerCommands);
    expect(handlerCommands.filter((command) => !called.has(command))).toEqual(
      pendingWrapper,
    );
  });
});
