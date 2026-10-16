// Textos visibles de los códigos de factura que devuelve el backend.

export const INVOICE_TYPE_LABELS = {
  E: "Entrada",
  V: "Venta",
  S: "Servicio",
  GG: "Garantía",
  GI: "Insumo",
  P: "Pérdida",
};

export const ENTRY_STATUS_LABELS = {
  pending: "Por Pagar",
  completed: "Pagada",
  cancelled: "Cancelada",
};

export const SALE_STATUS_LABELS = {
  pending: "Por Cobrar",
  completed: "Cobrada",
  cancelled: "Cancelada",
};

export const ASSOCIATED_STATUS_LABELS = {
  pending: "Pendiente",
  completed: "Completada",
  cancelled: "Cancelada",
};

// El texto del estado depende del tipo (§7.5); las pérdidas no tienen estado.
export function invoiceStatusLabel(type, status) {
  if (type === "P" || !status) return "—";
  const labels =
    type === "E"
      ? ENTRY_STATUS_LABELS
      : type === "V" || type === "S"
        ? SALE_STATUS_LABELS
        : ASSOCIATED_STATUS_LABELS;
  return labels[status] ?? status;
}
