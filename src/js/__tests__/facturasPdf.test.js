import { beforeAll, describe, expect, it } from "vitest";
import { readFile } from "node:fs/promises";
import path from "node:path";

// dashboard.js es un script clásico: se extrae el bloque de exportación del
// fuente real y se ejecuta con un jsPDF falso que registra lo que se escribe.
let exporters;
let blockSource = "";

class FakePdf {
  static last = null;
  constructor() {
    this.texts = [];
    this.savedAs = null;
    FakePdf.last = this;
  }
  setFontSize() {}
  line() {}
  splitTextToSize(text) {
    return [text];
  }
  text(value) {
    this.texts.push(...(Array.isArray(value) ? value : [value]));
  }
  save(name) {
    this.savedAs = name;
  }
}

const NAMES = [
  "exportarFacturaPDF",
  "exportarFacturaVentaPDF",
  "exportarFacturaServicioPDF",
  "exportarFacturaInsumoPDF",
  "exportarFacturaGarantiaPDF",
  "exportarFacturaPerdidaPDF",
];

beforeAll(async () => {
  const source = await readFile(
    path.resolve(process.cwd(), "src", "js", "dashboard.js"),
    "utf8",
  );
  blockSource = source.match(
    /\/\/ ===== Exportación de facturas a PDF[\s\S]*?\/\/ ===== Fin de la exportación de facturas a PDF =====/,
  )?.[0];
  const fechaSource = source.match(
    /^function formatearFecha\(iso\) \{[\s\S]*?^\}/m,
  )?.[0];
  exporters = new Function(
    "window",
    `${fechaSource}\n${blockSource}\nreturn { ${NAMES.join(", ")} };`,
  )({ jspdf: { jsPDF: FakePdf } });
});

// es-ES no agrupa los miles con cuatro cifras: se usa la misma regla.
const money = (cents) =>
  (cents / 100).toLocaleString("es-ES", {
    minimumFractionDigits: 2,
    maximumFractionDigits: 2,
  });
const TOTAL = money(123_450);

function detail(overrides = {}) {
  return {
    id: 7,
    number: "X-00007",
    invoiceType: "V",
    status: "completed",
    paymentMethod: "transfer",
    contactFullName: "Ana Pérez",
    date: "2026-02-10",
    totalCents: 123_450,
    notes: "Entregar por la tarde",
    associatedInvoice: { number: "S-00003", invoiceType: "S" },
    lines: [
      {
        productName: "Cable",
        quantity: "2,5",
        unitAbbreviation: "m",
        unitPriceCents: 49_380,
        subtotalCents: 123_450,
      },
    ],
    ...overrides,
  };
}

function exportWith(name, dto) {
  FakePdf.last = null;
  const result = exporters[name](dto);
  return { result, pdf: FakePdf.last, text: FakePdf.last?.texts.join("\n") };
}

describe("exportarFactura*PDF leen el DTO de getInvoice", () => {
  it("el bloque existe con las seis funciones y no lee arreglos simulados", () => {
    expect(blockSource).toBeTruthy();
    for (const name of NAMES) {
      expect(typeof exporters[name], name).toBe("function");
    }
    expect(blockSource).not.toMatch(/facturas\w*Data|\.find\(/);
  });

  it.each([
    [
      "exportarFacturaPDF",
      "Factura de Entrada",
      "Proveedor: Ana Pérez",
      "Estado: Pagada",
    ],
    [
      "exportarFacturaVentaPDF",
      "Factura de Venta",
      "Cliente: Ana Pérez",
      "Estado: Cobrada",
    ],
    [
      "exportarFacturaServicioPDF",
      "Factura de Servicio",
      "Cliente: Ana Pérez",
      "Estado: Cobrada",
    ],
  ])(
    "%s pinta contacto, estado, método de pago, líneas y total",
    (name, title, contact, status) => {
      const { result, pdf, text } = exportWith(name, detail());
      expect(result).toBe(true);
      expect(text).toContain(`${title} X-00007`);
      expect(text).toContain(contact);
      expect(text).toContain(status);
      expect(text).toContain("Método de Pago: Transferencia");
      expect(text).toContain("Cable");
      expect(text).toContain(TOTAL);
      expect(text).toContain("Entregar por la tarde");
      expect(text).toContain("Fecha de Emisión: 10/02/2026");
      expect(text).not.toMatch(/Descuento|Impuestos/);
      expect(pdf.savedAs).toBe("Factura_X-00007.pdf");
    },
  );

  it.each([
    [
      "exportarFacturaGarantiaPDF",
      "Factura de Gasto de Garantía",
      "Factura Original: S-00003 · Servicio",
    ],
    [
      "exportarFacturaInsumoPDF",
      "Factura de Gasto de Insumos",
      "Factura de Servicio Asociada: S-00003 · Servicio",
    ],
  ])(
    "%s muestra la factura asociada, estado y costo total, sin método de pago",
    (name, title, associated) => {
      const { text } = exportWith(name, detail());
      expect(text).toContain(`${title} X-00007`);
      expect(text).toContain(associated);
      expect(text).toContain("Estado: Completada");
      expect(text).toContain(`Costo Total: ${TOTAL} USD`);
      expect(text).not.toContain("Método de Pago");
    },
  );

  it("exportarFacturaPerdidaPDF muestra producto, cantidad, costo real y motivo, sin cliente ni estado", () => {
    const { text, pdf } = exportWith(
      "exportarFacturaPerdidaPDF",
      detail({ status: null, contactFullName: null, paymentMethod: null }),
    );
    expect(text).toContain("Factura de Pérdida X-00007");
    expect(text).toContain("Cable");
    expect(text).toContain("2,5");
    expect(text).toContain(money(49_380));
    expect(text).toContain(`Costo Total: ${TOTAL} USD`);
    expect(text).toContain("Motivo de la Pérdida: Entregar por la tarde");
    expect(text).not.toMatch(/Cliente|Estado|Método de Pago/);
    expect(pdf.savedAs).toBe("Factura_X-00007.pdf");
  });

  it.each(NAMES)(
    "%s ignora un número suelto (ya no hay arreglos que consultar)",
    (name) => {
      const { result, pdf } = exportWith(name, "V-00001");
      expect(result).toBe(false);
      expect(pdf).toBeNull();
    },
  );
});
