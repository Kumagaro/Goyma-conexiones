// Borradores de formularios (spec-04 BR-01, BR-02): se guardan en la base
// mientras el usuario escribe. Un fallo al guardar no interrumpe el formulario.

import { confirmDialog } from "../components/confirmDialog.js";
import { deleteDraft, getDraft, saveDraft } from "../services/draftsService.js";

const SAVE_DELAY_MS = 1000;
const SKIPPED_TYPES = new Set([
  "password",
  "file",
  "submit",
  "button",
  "reset",
]);
// Un campo de contraseña pasa a type=text con el botón de mostrarla.
const SECRET_NAME = /pass|contrase/i;

const openers = new Map();

/** Cómo llegar a un formulario desde la sección "Borradores" (BR-03). */
export function registerDraftOpener(formKey, open) {
  openers.set(formKey, open);
}

export function openDraft(formKey, recordId) {
  return openers.get(formKey)?.(recordId);
}

function quietly(action) {
  return Promise.resolve()
    .then(action)
    .catch(() => {});
}

const fieldKey = (control) => control.name || control.id;

function isDraftField(control) {
  const key = fieldKey(control);
  return (
    Boolean(key) &&
    !SKIPPED_TYPES.has(control.type) &&
    !SECRET_NAME.test(key) &&
    !control.disabled &&
    !control.readOnly &&
    // Lo elegido en un combobox lo guarda cada vista con su `serialize`.
    control.getAttribute("role") !== "combobox"
  );
}

/** Valores del formulario, sin contraseñas ni archivos (BR-01). */
export function serializeForm(form) {
  const values = {};
  for (const control of form.elements) {
    if (!isDraftField(control)) continue;
    const isCheck = control.type === "checkbox" || control.type === "radio";
    values[fieldKey(control)] = isCheck ? control.checked : control.value;
  }
  return values;
}

export function restoreForm(form, values) {
  for (const control of form.elements) {
    const key = fieldKey(control);
    if (!isDraftField(control) || !(key in values)) continue;
    if (typeof values[key] === "boolean") control.checked = values[key];
    else control.value = values[key];
    // Lo que depende del campo (vistas previas, secciones) se actualiza solo.
    control.dispatchEvent(new Event("input", { bubbles: true }));
    control.dispatchEvent(new Event("change", { bubbles: true }));
  }
}

/** Pregunta si recuperar el borrador; devuelve sus valores o null (BR-02). */
export async function askForDraft(formKey, recordId = null) {
  let draft = null;
  try {
    draft = await getDraft(formKey, recordId);
  } catch {
    return null;
  }
  if (!draft?.payload) return null;
  const decision = await confirmDialog({
    title: "Formulario incompleto",
    message: "Tienes un formulario incompleto. ¿Deseas continuar?",
    confirmText: "Continuar",
    cancelText: "Descartar",
    confirmClass: "btn-primary",
    // Cerrar el diálogo no es descartar: el borrador sigue ahí.
    dismissValue: null,
  });
  if (decision === true) return JSON.parse(draft.payload);
  if (decision === false) await quietly(() => deleteDraft(formKey, recordId));
  return null;
}

/**
 * Guarda el formulario un segundo después de la última tecla. `clear()` se
 * llama al guardar o cancelar, y `offer()` al abrirlo.
 */
export function attachDraft(
  form,
  {
    formKey,
    recordId = () => null,
    serialize = () => serializeForm(form),
    restore = (values) => restoreForm(form, values),
  },
) {
  let timer = null;
  let offering = null;

  const schedule = () => {
    clearTimeout(timer);
    const id = recordId();
    const payload = JSON.stringify(serialize());
    timer = setTimeout(() => {
      void quietly(() => saveDraft(formKey, id, payload));
    }, SAVE_DELAY_MS);
  };

  async function ask() {
    const values = await askForDraft(formKey, recordId());
    if (!values) return;
    await restore(values);
    // Restaurar dispara `input` antes de reponer lo que aporta la vista.
    schedule();
  }

  form.addEventListener("input", schedule);
  form.addEventListener("change", schedule);

  return {
    schedule,
    clear() {
      clearTimeout(timer);
      const id = recordId();
      void quietly(() => deleteDraft(formKey, id));
    },
    offer() {
      offering ??= ask().finally(() => {
        offering = null;
      });
      return offering;
    },
  };
}
