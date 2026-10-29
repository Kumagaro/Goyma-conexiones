// Construcción de DOM sin innerHTML (spec-04 SG-01): el texto siempre entra
// como nodo de texto, nunca como HTML.

/**
 * Crea un elemento. `props` admite `class`, `dataset`, `style` (texto CSS) y
 * cualquier otro atributo; los hijos pueden ser nodos o texto.
 */
export function el(tag, props = {}, ...children) {
  const node = document.createElement(tag);
  for (const [key, value] of Object.entries(props)) {
    if (value == null || value === false) continue;
    if (key === "class") node.className = value;
    else if (key === "dataset") Object.assign(node.dataset, value);
    else if (key === "style") node.style.cssText = value;
    else node.setAttribute(key, value === true ? "" : String(value));
  }
  node.append(...children.flat().filter((child) => child != null));
  return node;
}

/** Sustituye el cuerpo de una tabla por una fila de aviso a todo lo ancho. */
export function messageRow(body, text, colspan) {
  body.replaceChildren(
    el("tr", {}, el("td", { colspan, class: "empty-row" }, text)),
  );
}

/** Sustituye las opciones de un select; cada opción es `[valor, texto]`. */
export function setOptions(select, options) {
  select.replaceChildren(
    ...options.map(([value, text]) => el("option", { value }, text)),
  );
}
