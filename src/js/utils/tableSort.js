// Ordenar cualquier columna de una tabla pulsando su encabezado. Las columnas
// de fecha con orden propio (th-fecha-*) y las de acciones quedan fuera.

const DATE = /^(\d{2})\/(\d{2})\/(\d{4})/;
const NUMBER = /^#?-?[\d.]*\d(,\d+)?(\s.*)?$/;

export function cellValue(text) {
  const value = text.trim();
  const date = DATE.exec(value);
  if (date) return Number(`${date[3]}${date[2]}${date[1]}`);
  if (NUMBER.test(value)) {
    return Number(
      value
        .replace(/^#/, "")
        .split(/\s/)[0]
        .replace(/\./g, "")
        .replace(",", "."),
    );
  }
  return value;
}

function compare(left, right) {
  if (typeof left === "number" && typeof right === "number") {
    return left - right;
  }
  return String(left).localeCompare(String(right), "es", { numeric: true });
}

function isSortable(th) {
  return (
    !th.id.startsWith("th-fecha") &&
    !th.querySelector("button") &&
    th.textContent.trim() !== "" &&
    th.textContent.trim() !== "Acciones"
  );
}

function sortBy(th) {
  const table = th.closest("table");
  const body = table.tBodies[0];
  const rows = [...body.rows];
  if (rows.some((row) => row.querySelector(".empty-row"))) return;

  const ascending = th.getAttribute("aria-sort") !== "ascending";
  const index = th.cellIndex;
  const value = (row) => cellValue(row.cells[index]?.textContent ?? "");
  rows.sort(
    (left, right) => compare(value(left), value(right)) * (ascending ? 1 : -1),
  );
  table
    .querySelectorAll("thead th[aria-sort]")
    .forEach((header) => header.removeAttribute("aria-sort"));
  th.setAttribute("aria-sort", ascending ? "ascending" : "descending");
  body.append(...rows);
}

export function installTableSort(root = document) {
  for (const th of root.querySelectorAll(".list-table thead th")) {
    if (isSortable(th)) th.tabIndex = 0;
  }
  const headerOf = (event) => {
    const th = event.target.closest?.(".list-table thead th");
    return th && isSortable(th) ? th : null;
  };
  root.addEventListener("click", (event) => {
    const th = headerOf(event);
    if (th) sortBy(th);
  });
  root.addEventListener("keydown", (event) => {
    const th = event.key === "Enter" ? headerOf(event) : null;
    if (th) sortBy(th);
  });
}
