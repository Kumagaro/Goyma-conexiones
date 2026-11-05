// Sección "Borradores" (spec-04 BR-02, BR-03): lista los formularios
// incompletos del usuario y avisa de ellos al entrar a la aplicación.

import { confirmDialog } from "../components/confirmDialog.js";
import { showToast } from "../components/toast.js";
import { deleteDraft, listDrafts } from "../services/draftsService.js";
import { el, messageRow } from "../utils/dom.js";
import { openDraft } from "../utils/drafts.js";
import { notifyError, notifySuccess } from "../utils/feedback.js";
import { formatId, formatLocalTimestamp } from "../utils/formatters.js";

const PAGE_ID = "borradores";

const FORM_LABELS = {
  product: "Producto",
  category: "Categoría",
  unit: "Unidad",
  conversion: "Conversión",
  client: "Cliente",
  supplier: "Proveedor",
  entryInvoice: "Factura de entrada",
  saleInvoice: "Factura de venta",
  serviceInvoice: "Factura de servicio",
  guaranteeInvoice: "Factura de garantía",
  installationInvoice: "Factura de insumos",
  lossInvoice: "Factura de pérdida",
  user: "Usuario",
  profile: "Mi perfil",
  company: "Datos de la empresa",
  thresholds: "Umbrales",
  quickClient: "Alta rápida de cliente",
  quickSupplier: "Alta rápida de proveedor",
  quickProduct: "Alta rápida de producto",
};

function actionButton(label, extra, onClick) {
  const button = el(
    "button",
    { type: "button", class: `action-link${extra}` },
    label,
  );
  button.addEventListener("click", onClick);
  return button;
}

async function removeDraft({ formKey, recordId }) {
  const confirmed = await confirmDialog({
    title: "Eliminar borrador",
    message: `¿Eliminar el borrador de «${FORM_LABELS[formKey] ?? formKey}»?`,
    confirmText: "Eliminar",
  });
  if (!confirmed) return;
  try {
    await deleteDraft(formKey, recordId);
    notifySuccess("Borrador eliminado.");
    await loadDrafts();
  } catch (error) {
    notifyError(error, "No se pudo eliminar el borrador.");
  }
}

async function openForm({ formKey, recordId }) {
  try {
    await openDraft(formKey, recordId);
  } catch (error) {
    notifyError(error, "No se pudo abrir el formulario.");
  }
}

function draftRow(draft) {
  return el(
    "tr",
    {},
    el("td", {}, FORM_LABELS[draft.formKey] ?? draft.formKey),
    el(
      "td",
      {},
      draft.recordId == null ? "Nuevo" : `Edición ${formatId(draft.recordId)}`,
    ),
    el("td", {}, formatLocalTimestamp(draft.updatedAt)),
    el(
      "td",
      {},
      el(
        "div",
        { class: "row-actions" },
        actionButton("Abrir", "", () => void openForm(draft)),
        actionButton("Eliminar", " danger", () => void removeDraft(draft)),
      ),
    ),
  );
}

export async function loadDrafts() {
  const body = document.getElementById("borradores-body");
  if (!body) return;
  try {
    const drafts = await listDrafts();
    if (drafts?.length) body.replaceChildren(...drafts.map(draftRow));
    else messageRow(body, "No tienes formularios incompletos.", 4);
  } catch {
    messageRow(body, "No se pudieron cargar los borradores.", 4);
  }
}

function goToDrafts() {
  window.showPage?.(
    PAGE_ID,
    document.querySelector(`#sidebar [data-page="${PAGE_ID}"]`),
  );
}

async function announceDrafts() {
  let drafts = [];
  try {
    drafts = (await listDrafts()) ?? [];
  } catch {
    return;
  }
  if (!drafts.length) return;
  showToast({
    type: "info",
    message:
      drafts.length === 1
        ? "Tienes 1 formulario incompleto"
        : `Tienes ${drafts.length} formularios incompletos`,
    action: { label: "Ver borradores", onClick: goToDrafts },
  });
}

export async function initBorradoresView() {
  if (!document.getElementById(`page-${PAGE_ID}`)) return;
  if (!window.__borradoresPageChangeBound) {
    window.addEventListener("page:change", (event) => {
      if (event.detail?.pageId === PAGE_ID) void loadDrafts();
    });
    window.__borradoresPageChangeBound = true;
  }
  await announceDrafts();
}

if (document.readyState === "loading") {
  document.addEventListener("DOMContentLoaded", () => {
    void initBorradoresView();
  });
} else {
  void initBorradoresView();
}
