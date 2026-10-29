import { createClient } from "../services/clientsService.js";
import { createSupplier } from "../services/suppliersService.js";

// Alta rápida de cliente o proveedor desde una factura (CL-06, PV-06): los
// mismos campos que los formularios de Contactos; el backend valida igual que
// en CL-01/PV-01. Devuelve el contacto creado o null si se cancela.

const KINDS = {
  client: {
    title: "Nuevo cliente",
    noun: "cliente",
    create: (payload) => createClient(payload),
    extraFields: [["invoiceInfo", "Información de Factura", "text"]],
  },
  supplier: {
    title: "Nuevo proveedor",
    noun: "proveedor",
    create: (payload) => createSupplier(payload),
    extraFields: [],
  },
};

const BASE_FIELDS = [
  ["firstName", "Nombre(s)", "text", true],
  ["lastName", "Apellidos", "text", true],
  ["nationalId", "Número de Carnet de Identidad", "text", true],
  ["phone", "Número de Teléfono", "tel", true],
  ["company", "Empresa", "text"],
  ["position", "Cargo", "text"],
  ["email", "Correo", "email"],
  ["address", "Dirección", "text"],
];

let nextDialogId = 0;

export function openQuickContactDialog({ kind, initialText = "" }) {
  const config = KINDS[kind];
  const dialogId = `quick-${kind}-${++nextDialogId}`;
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
  heading.textContent = config.title;
  header.append(heading);

  const form = document.createElement("form");
  form.noValidate = true;
  const inputs = {};
  for (const [name, label, type, required] of [
    ...BASE_FIELDS,
    ...config.extraFields,
  ]) {
    const field = document.createElement("div");
    field.className = "field";
    const labelElement = document.createElement("label");
    labelElement.htmlFor = `${dialogId}-${name}`;
    labelElement.textContent = label;
    if (required) {
      const mark = document.createElement("span");
      mark.className = "required";
      mark.textContent = " *";
      labelElement.append(mark);
    }
    const input = document.createElement("input");
    input.id = `${dialogId}-${name}`;
    input.name = name;
    input.type = type;
    input.required = Boolean(required);
    field.append(labelElement, input);
    form.append(field);
    inputs[name] = input;
  }

  // Lo tecleado en el buscador se ofrece como nombre y apellidos.
  const [firstWord = "", ...rest] = String(initialText).trim().split(/\s+/);
  inputs.firstName.value = firstWord;
  inputs.lastName.value = rest.join(" ");

  const feedback = document.createElement("p");
  feedback.className = "form-feedback";
  feedback.setAttribute("role", "alert");
  feedback.hidden = true;

  const actions = document.createElement("div");
  actions.className = "form-actions";
  const cancelButton = document.createElement("button");
  cancelButton.type = "button";
  cancelButton.className = "btn-secondary";
  cancelButton.textContent = "Cancelar";
  const saveButton = document.createElement("button");
  saveButton.type = "submit";
  saveButton.className = "btn-primary";
  saveButton.textContent = `Crear ${config.noun}`;
  actions.append(cancelButton, saveButton);
  form.append(feedback, actions);

  card.append(header, form);
  overlay.append(card);

  return new Promise((resolve) => {
    let settled = false;
    let saving = false;

    const finish = (contact) => {
      if (settled) return;
      settled = true;
      document.removeEventListener("keydown", handleKeydown);
      overlay.remove();
      if (opener instanceof HTMLElement && opener.isConnected) opener.focus();
      resolve(contact);
    };

    const showError = (message) => {
      feedback.textContent = message;
      feedback.hidden = false;
    };

    const handleKeydown = (event) => {
      if (event.key === "Escape") {
        event.preventDefault();
        finish(null);
        return;
      }
      if (event.key !== "Tab") return;
      const focusable = [
        ...card.querySelectorAll("input:not(:disabled), button:not(:disabled)"),
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
      const payload = Object.fromEntries(
        Object.entries(inputs).map(([name, input]) => [
          name,
          input.value.trim(),
        ]),
      );
      const missing = Object.values(inputs).find(
        (input) => input.required && !input.value.trim(),
      );
      if (missing) {
        showError(`Completa los campos obligatorios del ${config.noun}.`);
        missing.focus();
        return;
      }
      saving = true;
      saveButton.disabled = true;
      try {
        const created = await config.create(payload);
        finish(created ?? null);
      } catch (error) {
        showError(error?.message || `No se pudo crear el ${config.noun}.`);
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
    (
      Object.values(inputs).find((input) => input.required && !input.value) ??
      inputs.firstName
    ).focus();
  });
}
