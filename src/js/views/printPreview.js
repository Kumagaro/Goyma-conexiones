// Vista previa A4 de una factura (spec-04 IM-01, D-06): se monta en
// #print-area y, desde ella, se abre el diálogo de impresión del sistema
// (impresora o "Guardar como PDF"). css/print.css oculta todo lo demás al
// imprimir. Sin estado de la factura (RF-129).
import { el } from "../utils/dom.js";
import { formatMoney } from "../utils/formatters.js";

let opener = null;

function closePreview() {
  const area = document.getElementById("print-area");
  if (area) area.hidden = true;
  document.removeEventListener("keydown", onKeydown);
  if (opener?.isConnected) opener.focus();
}

function onKeydown(event) {
  if (event.key === "Escape") closePreview();
}

function companyHeader(company) {
  const lines = [
    company?.address,
    company?.phone ? `Teléfono: ${company.phone}` : null,
    company?.taxId ? `NIT: ${company.taxId}` : null,
  ].filter(Boolean);
  return el(
    "header",
    { class: "print-company" },
    company?.logoDataUrl
      ? el("img", {
          class: "print-logo",
          src: company.logoDataUrl,
          alt: "Logo de la empresa",
        })
      : null,
    el(
      "div",
      {},
      company?.name
        ? el("div", { class: "print-company-name" }, company.name)
        : null,
      lines.map((line) => el("div", {}, line)),
    ),
  );
}

function contactBlock(contact) {
  if (!contact) return null;
  return el(
    "section",
    { class: "print-contact" },
    el("div", { class: "print-strong" }, `${contact.label}: ${contact.name}`),
    [
      contact.company,
      contact.nationalId ? `CI: ${contact.nationalId}` : null,
      contact.phone ? `Teléfono: ${contact.phone}` : null,
      contact.email ? `Correo: ${contact.email}` : null,
    ]
      .filter(Boolean)
      .map((line) => el("div", {}, line)),
  );
}

function linesTable(model) {
  return el(
    "table",
    { class: "print-lines" },
    el(
      "thead",
      {},
      el(
        "tr",
        {},
        ["Producto", "Cantidad", "Unidad", model.priceLabel, "Subtotal"].map(
          (title) => el("th", {}, title),
        ),
      ),
    ),
    el(
      "tbody",
      {},
      (model.lines ?? []).map((line) =>
        el(
          "tr",
          {},
          el("td", {}, line.name),
          el("td", {}, line.quantity),
          el("td", {}, line.unit || "—"),
          el("td", {}, formatMoney(line.unitPriceCents)),
          el("td", {}, formatMoney(line.subtotalCents)),
        ),
      ),
    ),
  );
}

/** Construye la hoja A4 con el modelo de getInvoicePrintModel. */
export function renderPrintSheet(model) {
  return el(
    "article",
    { class: "print-sheet" },
    companyHeader(model.company),
    el("h1", { class: "print-title" }, model.title),
    el(
      "div",
      { class: "print-meta" },
      `No. ${model.number}    Fecha: ${model.date}`,
    ),
    model.reservedUntil
      ? el(
          "div",
          { class: "print-meta" },
          `Reservada hasta: ${model.reservedUntil}`,
        )
      : null,
    model.associated
      ? el(
          "div",
          { class: "print-meta" },
          `Factura asociada: ${model.associated}`,
        )
      : null,
    contactBlock(model.contact),
    linesTable(model),
    el(
      "div",
      { class: "print-total" },
      `Total: ${formatMoney(model.totalCents)}`,
    ),
    model.notes
      ? el(
          "section",
          { class: "print-notes" },
          el("div", { class: "print-strong" }, `${model.notesLabel}:`),
          el("p", {}, model.notes),
        )
      : null,
  );
}

export function openPrintPreview(model) {
  opener = document.activeElement;
  let area = document.getElementById("print-area");
  if (!area) {
    area = el("div", {
      id: "print-area",
      role: "dialog",
      "aria-modal": "true",
      "aria-label": "Vista previa de impresión",
    });
    document.body.append(area);
  }
  const printButton = el(
    "button",
    { type: "button", class: "btn-primary" },
    "Imprimir",
  );
  printButton.addEventListener("click", () => window.print());
  const closeButton = el(
    "button",
    { type: "button", class: "btn-secondary" },
    "Cerrar",
  );
  closeButton.addEventListener("click", closePreview);
  area.replaceChildren(
    el("div", { class: "print-toolbar" }, closeButton, printButton),
    renderPrintSheet(model),
  );
  area.hidden = false;
  document.addEventListener("keydown", onKeydown);
  printButton.focus();
  return area;
}
