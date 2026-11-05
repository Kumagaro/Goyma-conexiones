import { invoke } from "./invoke.js";

export function exportCsv(kind, filters, destinationPath) {
  return invoke("export_csv", { kind, filters, destinationPath });
}

export function importCsv(kind, sourcePath) {
  return invoke("import_csv", { kind, sourcePath });
}
