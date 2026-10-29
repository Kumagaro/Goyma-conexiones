// Orden por fecha de los listados: el encabezado alterna ascendente y
// descendente (por defecto, el más reciente primero, como lo envía el
// backend). El orden se guarda en el propio encabezado.
export function isAscending(headerId) {
  return document.getElementById(headerId)?.dataset.sortOrder === "asc";
}

export function sortByDate(items, headerId, dateKey = "date") {
  const ascending = isAscending(headerId);
  return [...items].sort((left, right) => {
    const order =
      String(left?.[dateKey] ?? "").localeCompare(
        String(right?.[dateKey] ?? ""),
      ) || Number(left?.id ?? 0) - Number(right?.id ?? 0);
    return ascending ? order : -order;
  });
}

export function bindDateSort(headerId, iconKey, rerender, claim) {
  const header = document.getElementById(headerId);
  if (!claim(header)) return;
  header.addEventListener("click", () => {
    header.dataset.sortOrder =
      header.dataset.sortOrder === "asc" ? "desc" : "asc";
    // dashboard.js solo actualiza el icono del encabezado.
    if (typeof window.toggleOrdenFecha === "function") {
      window.toggleOrdenFecha(iconKey, rerender);
    } else {
      rerender();
    }
  });
}
