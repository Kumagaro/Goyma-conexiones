import { askForDraft, attachDraft, restoreForm } from "../utils/drafts.js";
// Formulario modal pequeño (alta rápida desde una factura): campos con
// etiqueta visible, foco atrapado y Escape para cancelar; los errores salen
// junto al campo y como toast (spec-04 TO-01). Devuelve lo que resuelva
// `onSubmit`, o null si se cancela.

import {
  clearFieldErrors,
  notifyError,
  requireFields,
} from "../utils/feedback.js";

let nextDialogId = 0;

function buildControl(dialogId, field) {
  const id = `${dialogId}-${field.name}`;
  let control;
  if (field.type === "select") {
    control = document.createElement("select");
    for (const [value, text] of field.options ?? []) {
      const option = document.createElement("option");
      option.value = value;
      option.textContent = text;
      control.append(option);
    }
  } else if (field.type === "textarea") {
    control = document.createElement("textarea");
    control.rows = 3;
  } else {
    control = document.createElement("input");
    control.type = field.type ?? "text";
    if (field.inputMode) control.inputMode = field.inputMode;
  }
  control.id = id;
  control.name = field.name;
  control.required = Boolean(field.required);
  control.value = field.value ?? "";
  return control;
}

export async function openFormDialog({
  title,
  fields,
  submitText,
  missingMessage,
  onSubmit,
  draftKey = null,
}) {
  // Se pregunta antes de abrir: el diálogo de recuperar no se apila sobre este.
  const savedValues = draftKey ? await askForDraft(draftKey) : null;
  const dialogId = `form-dialog-${++nextDialogId}`;
  let draft = null;
  const opener = document.activeElement;

  const overlay = document.createElement("div");
  overlay.className = "modal-overlay active";
  const card = document.createElement("div");
  card.className = "modal-card";
  card.setAttribute("role", "dialog");
  card.setAttribute("aria-modal", "true");
  card.setAttribute("aria-labelledby", `${dialogId}-title`);

  const header = document.createElement("div");
  header.className = "modal-header";
  const heading = document.createElement("h3");
  heading.id = `${dialogId}-title`;
  heading.textContent = title;
  header.append(heading);

  const form = document.createElement("form");
  form.noValidate = true;
  const controls = {};
  for (const field of fields) {
    const wrapper = document.createElement("div");
    wrapper.className = "field";
    const label = document.createElement("label");
    label.htmlFor = `${dialogId}-${field.name}`;
    label.textContent = field.label;
    if (field.required) {
      const mark = document.createElement("span");
      mark.className = "required";
      mark.textContent = " *";
      label.append(mark);
    }
    const control = buildControl(dialogId, field);
    wrapper.append(label, control);
    form.append(wrapper);
    controls[field.name] = control;
  }

  const actions = document.createElement("div");
  actions.className = "form-actions";
  const cancelButton = document.createElement("button");
  cancelButton.type = "button";
  cancelButton.className = "btn-secondary";
  cancelButton.textContent = "Cancelar";
  const saveButton = document.createElement("button");
  saveButton.type = "submit";
  saveButton.className = "btn-primary";
  saveButton.textContent = submitText;
  actions.append(cancelButton, saveButton);
  form.append(actions);

  card.append(header, form);
  overlay.append(card);

  return new Promise((resolve) => {
    let settled = false;
    let saving = false;

    const finish = (result) => {
      if (settled) return;
      settled = true;
      draft?.clear();
      document.removeEventListener("keydown", handleKeydown);
      overlay.remove();
      if (opener instanceof HTMLElement && opener.isConnected) opener.focus();
      resolve(result);
    };

    const handleKeydown = (event) => {
      if (event.key === "Escape") {
        event.preventDefault();
        finish(null);
        return;
      }
      if (event.key !== "Tab") return;
      const focusable = [
        ...card.querySelectorAll(
          "input:not(:disabled), select:not(:disabled), textarea:not(:disabled), button:not(:disabled)",
        ),
      ];
      const index = focusable.indexOf(document.activeElement);
      const direction = event.shiftKey ? -1 : 1;
      event.preventDefault();
      focusable[
        index === -1
          ? 0
          : (index + direction + focusable.length) % focusable.length
      ].focus();
    };

    form.addEventListener("submit", async (event) => {
      event.preventDefault();
      if (saving) return;
      clearFieldErrors(form);
      const required = Object.values(controls).filter(
        (control) => control.required,
      );
      // spec-04 TO-01: el campo vacío se marca junto a él y el resumen, en toast.
      if (!requireFields(required)) {
        notifyError(null, missingMessage);
        return;
      }
      const values = Object.fromEntries(
        Object.entries(controls).map(([name, control]) => [
          name,
          control.value.trim(),
        ]),
      );
      saving = true;
      saveButton.disabled = true;
      try {
        finish((await onSubmit(values)) ?? null);
      } catch (error) {
        notifyError(error, "No se pudo guardar.");
      } finally {
        saving = false;
        saveButton.disabled = false;
      }
    });
    cancelButton.addEventListener("click", () => finish(null));
    overlay.addEventListener("click", (event) => {
      if (event.target === overlay) finish(null);
    });
    document.addEventListener("keydown", handleKeydown);
    document.body.append(overlay);
    if (savedValues) restoreForm(form, savedValues);
    if (draftKey) draft = attachDraft(form, { formKey: draftKey });
    (
      Object.values(controls).find(
        (control) => control.required && !control.value,
      ) ?? Object.values(controls)[0]
    ).focus();
  });
}
