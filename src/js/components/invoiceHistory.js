import { formatDate, formatMoney } from "../utils/formatters.js";
import {
  INVOICE_TYPE_LABELS,
  invoiceStatusLabel,
} from "../utils/invoiceLabels.js";

// Historial de facturas de un contacto (CL-03, PV-03): número, tipo, fecha,
// total y estado; "Ver" abre el detalle con la navegación de facturasView.js.
export function renderInvoiceHistory(container, invoices, emptyText) {
  if (!container) return;
  const items = Array.isArray(invoices) ? invoices : [];
  if (!items.length) {
    const empty = document.createElement("p");
    empty.className = "pv-desc-text";
    empty.textContent = emptyText;
    container.replaceChildren(empty);
    return;
  }

  const table = document.createElement("table");
  table.className = "list-table";
  const headRow = document.createElement("tr");
  for (const label of [
    "No. Factura",
    "Tipo",
    "Fecha",
    "Total",
    "Estado",
    "Acciones",
  ]) {
    const th = document.createElement("th");
    th.textContent = label;
    if (label === "Total") th.style.textAlign = "right";
    headRow.append(th);
  }
  const thead = document.createElement("thead");
  thead.append(headRow);

  const tbody = document.createElement("tbody");
  for (const invoice of items) {
    const type = invoice?.invoiceType ?? "";
    const row = document.createElement("tr");
    const cells = [
      invoice?.number ?? "—",
      INVOICE_TYPE_LABELS[type] ?? type,
      invoice?.date ? formatDate(invoice.date) : "—",
      formatMoney(invoice?.totalCents ?? 0),
      invoiceStatusLabel(type, invoice?.status),
    ].map((text) => {
      const cell = document.createElement("td");
      cell.textContent = text;
      return cell;
    });
    cells[3].style.textAlign = "right";

    const actions = document.createElement("td");
    const view = document.createElement("button");
    view.type = "button";
    view.className = "action-link";
    view.textContent = "Ver";
    view.setAttribute("aria-label", `Ver factura ${invoice?.number ?? ""}`);
    view.addEventListener("click", () => {
      window.irAFacturaDesdeGeneral?.(type, Number(invoice?.id));
    });
    actions.append(view);
    row.append(...cells, actions);
    tbody.append(row);
  }

  table.append(thead, tbody);
  container.replaceChildren(table);
}
