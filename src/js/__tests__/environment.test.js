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

  it("TO-01: ninguna vista de producción usa .form-feedback ni contenedores de resultado generales", async () => {
    const sourceFiles = await collectSourceFiles(sourceRoot);
    const productionFiles = sourceFiles.filter(
      (filePath) => !filePath.includes(`${path.sep}__tests__${path.sep}`),
    );
    const contents = await Promise.all(
      productionFiles.map((filePath) => readFile(filePath, "utf8")),
    );
    const source = contents.join("\n");

    expect(source).not.toMatch(/form-feedback/);
    expect(source).not.toMatch(/cleanupWarning\.js|cleanup-warning/);
    // Los avisos en vivo de categoría y unidad y la vista previa del precio
    // son ayudas junto al campo, no resultados: se conservan.
    for (const id of [
      "productos-feedback",
      "producto-condiciones-feedback",
      "producto-feedback",
      "producto-detalle-feedback",
      "usuarios-feedback",
      "usuario-feedback",
      "perfil-feedback",
      "historial-feedback",
      "config-paginacion-feedback",
    ]) {
      expect(source, id).not.toContain(`"${id}"`);
    }
  });

  it("SG-01: ni el HTML ni las plantillas del JS de producción tienen atributos on*", async () => {
    for (const page of [
      "dashboard-unificado.html",
      "index.html",
      "recovery.html",
    ]) {
      const html = await readFile(path.join(sourceRoot, page), "utf8");
      expect(html.match(/\son[a-z]+\s*=/gi) ?? [], page).toEqual([]);
    }
    const sourceFiles = await collectSourceFiles(path.join(sourceRoot, "js"));
    const productionJs = sourceFiles.filter(
      (filePath) =>
        filePath.endsWith(".js") &&
        !filePath.includes(`${path.sep}__tests__${path.sep}`) &&
        !filePath.includes(`${path.sep}libs${path.sep}`),
    );
    for (const filePath of productionJs) {
      const source = await readFile(filePath, "utf8");
      expect(
        source.match(/\son[a-z]+="/g) ?? [],
        path.basename(filePath),
      ).toEqual([]);
    }
  });

  it("SG-01: tauri.conf.json fija la CSP de plan-04 y el HTML no trae scripts inline ni recursos externos", async () => {
    const config = JSON.parse(
      await readFile(
        path.resolve(process.cwd(), "src-tauri", "tauri.conf.json"),
        "utf8",
      ),
    );
    expect(config.app.security.csp).toBe(
      "default-src 'self'; script-src 'self'; style-src 'self' 'unsafe-inline'; img-src 'self' data:; connect-src ipc: http://ipc.localhost; object-src 'none'; base-uri 'none'; form-action 'none'",
    );
    for (const page of [
      "dashboard-unificado.html",
      "index.html",
      "recovery.html",
    ]) {
      const doc = new DOMParser().parseFromString(
        await readFile(path.join(sourceRoot, page), "utf8"),
        "text/html",
      );
      const inline = [...doc.querySelectorAll("script")].filter(
        (script) => !script.getAttribute("src"),
      );
      expect(inline, page).toHaveLength(0);
      const external = [...doc.querySelectorAll("[src], link[href]")]
        .map((node) => node.getAttribute("src") ?? node.getAttribute("href"))
        .filter((url) => /^(?:https?:)?\/\//i.test(url));
      expect(external, page).toEqual([]);
      // IM-02: el PDF lo genera el backend; jsPDF se retiró.
      expect(
        [...doc.querySelectorAll("script[src]")].some((script) =>
          /jspdf/i.test(script.getAttribute("src")),
        ),
        page,
      ).toBe(false);
    }
    const libs = await readdir(path.join(sourceRoot, "js", "libs"));
    expect(libs.some((name) => /jspdf/i.test(name))).toBe(false);
  });

  it("SG-01: ningún innerHTML del JS de producción recibe una plantilla o concatenación", async () => {
    const sourceFiles = await collectSourceFiles(path.join(sourceRoot, "js"));
    const productionJs = sourceFiles.filter(
      (filePath) =>
        filePath.endsWith(".js") &&
        !filePath.includes(`${path.sep}__tests__${path.sep}`) &&
        !filePath.includes(`${path.sep}libs${path.sep}`),
    );
    const violations = [];
    for (const filePath of productionJs) {
      const name = path.basename(filePath);
      const source = (await readFile(filePath, "utf8")).replace(/\r\n/g, "\n");
      for (const match of source.matchAll(/innerHTML\s*\+?=\s*/g)) {
        const rest = source.slice(match.index + match[0].length);
        // Un literal fijo (sin datos) sí se admite.
        if (/^(['"])(?:\\.|(?!\1)[^\n])*\1\s*;/.test(rest)) continue;
        const lineStart = source.lastIndexOf("\n", match.index) + 1;
        const line = source
          .slice(lineStart, source.indexOf("\n", match.index))
          .trim();
        violations.push(`${name}: ${line}`);
      }
    }
    expect(violations).toEqual([]);
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

    // spec-04: notificaciones y configuración van tras los módulos de spec-03.
    expect(moduleSources.slice(-9)).toEqual([
      ...featureModules,
      "./js/views/notificacionesView.js",
      "./js/views/configuracionView.js",
      "./js/views/borradoresView.js",
      "./js/views/actividadView.js",
      "./js/views/datosView.js",
    ]);
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
    const handlerCommands = [...handler[1].matchAll(/(?:[a-z_]+::)+([a-z_]+)/g)]
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
