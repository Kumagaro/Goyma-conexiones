import { registerDraftOpener } from "../utils/drafts.js";
import { openFormDialog } from "./formDialog.js";
import { createClient } from "../services/clientsService.js";
import { createSupplier } from "../services/suppliersService.js";

// Alta rápida de cliente o proveedor desde una factura (CL-06, PV-06): los
// mismos campos que los formularios de Contactos; el backend valida igual que
// en CL-01/PV-01. Devuelve el contacto creado o null si se cancela.

const KINDS = {
  client: {
    title: "Nuevo cliente",
    draftKey: "quickClient",
    noun: "cliente",
    create: (payload) => createClient(payload),
    extraFields: [{ name: "invoiceInfo", label: "Información de Factura" }],
  },
  supplier: {
    title: "Nuevo proveedor",
    draftKey: "quickSupplier",
    noun: "proveedor",
    create: (payload) => createSupplier(payload),
    extraFields: [],
  },
};

export function openQuickContactDialog({ kind, initialText = "" }) {
  const config = KINDS[kind];
  // Lo tecleado en el buscador se ofrece como nombre y apellidos.
  const [firstWord = "", ...rest] = String(initialText).trim().split(/\s+/);
  return openFormDialog({
    title: config.title,
    draftKey: config.draftKey,
    submitText: `Crear ${config.noun}`,
    missingMessage: `Completa los campos obligatorios del ${config.noun}.`,
    fields: [
      {
        name: "firstName",
        label: "Nombre(s)",
        required: true,
        value: firstWord,
      },
      {
        name: "lastName",
        label: "Apellidos",
        required: true,
        value: rest.join(" "),
      },
      {
        name: "nationalId",
        label: "Número de Carnet de Identidad",
        required: true,
      },
      {
        name: "phone",
        label: "Número de Teléfono",
        type: "tel",
        required: true,
      },
      { name: "company", label: "Empresa" },
      { name: "position", label: "Cargo" },
      { name: "email", label: "Correo", type: "email" },
      { name: "address", label: "Dirección" },
      ...config.extraFields,
    ],
    onSubmit: (payload) => config.create(payload),
  });
}

registerDraftOpener("quickClient", () =>
  openQuickContactDialog({ kind: "client" }),
);
registerDraftOpener("quickSupplier", () =>
  openQuickContactDialog({ kind: "supplier" }),
);
