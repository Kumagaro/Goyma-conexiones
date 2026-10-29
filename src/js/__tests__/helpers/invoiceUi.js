import { vi } from "vitest";
import { readFile } from "node:fs/promises";
import path from "node:path";

// Páginas de facturas del HTML de producción (task-03 §7: sin HTML falso).
export async function loadInvoicePagesHtml() {
  const html = await readFile(
    path.resolve(process.cwd(), "src", "dashboard-unificado.html"),
    "utf8",
  );
  return Array.from(
    new DOMParser()
      .parseFromString(html, "text/html")
      .querySelectorAll('[id^="page-facturas-"]'),
  )
    .map((page) => page.outerHTML)
    .join("");
}

// Escribe en el buscador como el usuario y devuelve las opciones mostradas.
export async function typeInCombobox(inputId, text) {
  const input = document.getElementById(inputId);
  input.focus();
  input.value = text;
  input.dispatchEvent(new Event("input", { bubbles: true }));
  return vi.waitFor(
    () => {
      const options = [
        ...document.querySelectorAll(`#${inputId}-listbox [role="option"]`),
      ];
      if (!options.length) throw new Error(`sin opciones en ${inputId}`);
      return options;
    },
    { timeout: 3000 },
  );
}

// Escribe, espera las coincidencias del backend y elige la que contiene `optionText`.
export async function chooseFromCombobox(inputId, text, optionText = text) {
  const options = await typeInCombobox(inputId, text);
  const option = options.find((item) => item.textContent.includes(optionText));
  if (!option) throw new Error(`no aparece «${optionText}» en ${inputId}`);
  option.click();
  return option;
}

export function setValue(id, value, eventName = "input") {
  const el = document.getElementById(id);
  el.value = value;
  el.dispatchEvent(new Event(eventName, { bubbles: true }));
}

export function submitForm(formId) {
  document
    .getElementById(formId)
    .dispatchEvent(new Event("submit", { bubbles: true, cancelable: true }));
}

export function lineInputs(linesBodyId, field) {
  return [
    ...document.querySelectorAll(
      `#${linesBodyId} [data-line-field="${field}"]`,
    ),
  ];
}

// Toasts visibles de un tipo (spec-04 TO-01).
export function toastsOfType(type) {
  return [...document.querySelectorAll(`#toast-region .toast-${type}`)].filter(
    (toast) => !toast.classList.contains("is-leaving"),
  );
}

// Texto de los toasts visibles (título y mensaje), para las aserciones.
export function toastText() {
  return [...document.querySelectorAll("#toast-region .toast")]
    .filter((toast) => !toast.classList.contains("is-leaving"))
    .map((toast) => toast.textContent)
    .join(" ");
}
