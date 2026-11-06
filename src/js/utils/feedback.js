import { showToast } from "../components/toast.js";

// Respuestas de la aplicación (spec-04 TO-01): los resultados, los errores
// generales y los avisos van en toast; los errores de un campo concreto
// también se muestran junto al campo.

export function notifySuccess(message, title) {
  return showToast({ type: "success", title, message });
}

export function notifyInfo(message, title) {
  return showToast({ type: "info", title, message });
}

// Vacío o null no muestra nada: así se usa con `expiredCleanupWarning`.
export function notifyWarning(message, title) {
  if (!message) return null;
  return showToast({ type: "warning", title, message });
}

// invoke.js ya deja en `message` un texto en español apto para el usuario
// (el del backend o uno genérico); si no llega ninguno, se usa el de la vista.
export function notifyError(error, fallback) {
  return showToast({ type: "error", message: error?.message || fallback });
}

function errorIdFor(input) {
  return `${input.id || input.name}-error`;
}

export function clearFieldError(input) {
  if (!input) return;
  const id = errorIdFor(input);
  document.getElementById(id)?.remove();
  input.removeAttribute("aria-invalid");
  const described = (input.getAttribute("aria-describedby") ?? "")
    .split(/\s+/)
    .filter((token) => token && token !== id);
  if (described.length) {
    input.setAttribute("aria-describedby", described.join(" "));
  } else {
    input.removeAttribute("aria-describedby");
  }
}

export function setFieldError(input, message) {
  if (!input) return;
  const id = errorIdFor(input);
  let error = document.getElementById(id);
  if (!error) {
    error = document.createElement("p");
    error.id = id;
    error.className = "field-error";
    // Junto al control; en un combobox, tras su contenedor.
    const anchor = input.closest(".autocomplete-row, .input-box") ?? input;
    anchor.after(error);
  }
  error.textContent = message;
  input.setAttribute("aria-invalid", "true");
  const described = (input.getAttribute("aria-describedby") ?? "")
    .split(/\s+/)
    .filter(Boolean);
  if (!described.includes(id)) described.push(id);
  input.setAttribute("aria-describedby", described.join(" "));
  // Se limpia en cuanto el usuario corrige.
  input.addEventListener("input", () => clearFieldError(input), { once: true });
}

export function clearFieldErrors(root) {
  root?.querySelectorAll("[aria-invalid='true']").forEach(clearFieldError);
  root?.querySelectorAll(".field-error").forEach((error) => error.remove());
}

const FIELD_ERROR_CODES = new Set(["validationFailed", "duplicateField"]);

function resolveInput(target) {
  return typeof target === "string" ? document.getElementById(target) : target;
}

/**
 * Error de una acción: toast y, si es un rechazo de validación cuyo mensaje
 * nombra un campo, además junto a ese campo. `fields`: [[/carnet/i, "cliente-ci"], …].
 */
export function reportError(error, fallback, fields = []) {
  const toast = notifyError(error, fallback);
  if (FIELD_ERROR_CODES.has(error?.code)) {
    const match = fields.find(([pattern]) => pattern.test(error.message ?? ""));
    if (match) setFieldError(resolveInput(match[1]), error.message);
  }
  return toast;
}

/** Marca los campos obligatorios vacíos; devuelve true si no falta ninguno. */
export function requireFields(targets, message = "Completa este campo") {
  const missing = targets
    .map(resolveInput)
    .filter((input) => input && !String(input.value ?? "").trim());
  missing.forEach((input) => setFieldError(input, message));
  missing[0]?.focus();
  return missing.length === 0;
}
