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
    const [handlerSource, serviceEntries] = await Promise.all([
      readFile(handlerPath, "utf8"),
      readdir(servicesPath, { withFileTypes: true }),
    ]);
    const serviceSources = await Promise.all(
      serviceEntries
        .filter((entry) => entry.isFile() && entry.name.endsWith("Service.js"))
        .map((entry) => readFile(path.join(servicesPath, entry.name), "utf8")),
    );
    const handler = handlerSource.match(
      /tauri::generate_handler!\[([\s\S]*?)\]/,
    );
    const handlerCommands = [
      ...handler[1].matchAll(/commands::(?:auth|users)::([a-z_]+)/g),
    ]
      .map(([, command]) => command)
      .sort();
    const serviceCommands = serviceSources
      .flatMap((source) => [
        ...source.matchAll(/\binvoke\(\s*["']([a-z_]+)["']/g),
      ])
      .map(([, command]) => command)
      .sort();

    expect(handler).not.toBeNull();
    expect(handlerCommands).toHaveLength(15);
    expect(serviceCommands).toEqual(handlerCommands);
  });
});
