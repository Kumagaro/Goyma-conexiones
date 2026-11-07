import { attachDraft, registerDraftOpener } from "../utils/drafts.js";
import { confirmDialog } from "../components/confirmDialog.js";
import {
  clearFieldErrors,
  notifyError,
  notifySuccess,
  reportError,
  requireFields,
} from "../utils/feedback.js";
import {
  convertQuantity,
  createConversion,
  createUnit,
  deleteConversion,
  deleteUnit,
  listConversions,
  listUnits,
  searchUnits,
  updateConversionFactor,
  updateUnit,
} from "../services/unitsService.js";
import { el, messageRow, setOptions, emptyState } from "../utils/dom.js";

let unitDraft = null;
let conversionDraft = null;
let units = [];
let conversions = [];
let editingUnitId = null;
let editingConversionId = null;
let initPromise = null;
let pendingProductUnitCreation = null;

export function initUnidadesView() {
  if (!initPromise) initPromise = initializeUnidadesView();
  return initPromise;
}

async function initializeUnidadesView() {
  const form = document.getElementById("unidad-form");
  const searchInput = document.getElementById("unidad-search");
  const nameInput = document.getElementById("modal-unidad-nombre");
  const abbreviationInput = document.getElementById("modal-unidad-abreviacion");
  const cancelButton = document.getElementById("modal-unidad-cancelar");
  const closeButton = document.getElementById("modal-unidad-cerrar");
  const modal = document.getElementById("modal-unidad");
  const body = document.getElementById("unidades-body");
  const createButton = document.querySelector('[data-unit-action="create"]');

  const conversionForm = document.getElementById("conversion-form");
  const conversionCancelButton = document.getElementById(
    "modal-conversion-cancelar",
  );
  const conversionModal = document.getElementById("modal-conversion");
  const conversionBody = document.getElementById("conversiones-body");
  const conversionCreateButton = document.querySelector(
    '[data-conversion-action="create"]',
  );

  const conversorOrigen = document.getElementById("conversor-origen");
  const conversorDestino = document.getElementById("conversor-destino");
  const conversorCantidad = document.getElementById("conversor-cantidad");
  const conversorSwap = document.querySelector(".conversor-swap");

  if (form) {
    form.addEventListener("submit", handleUnitSubmit);
    unitDraft = attachDraft(form, {
      formKey: "unit",
      recordId: () => editingUnitId,
    });
  }
  if (searchInput) searchInput.addEventListener("input", handleUnitSearch);
  if (nameInput) nameInput.addEventListener("input", handleUnitNameInput);
  if (abbreviationInput)
    abbreviationInput.addEventListener("input", handleUnitNameInput);
  if (cancelButton) cancelButton.addEventListener("click", cancelUnitModal);
  if (closeButton) closeButton.addEventListener("click", cancelUnitModal);
  if (createButton)
    createButton.addEventListener("click", () => openUnitModal());
  if (body) body.addEventListener("click", handleUnitTableAction);
  if (modal) {
    modal.addEventListener("click", (event) => {
      if (event.target === modal) cancelUnitModal();
    });
  }
  document.addEventListener("keydown", (event) => {
    if (event.key !== "Escape") return;
    if (modal?.classList.contains("active")) cancelUnitModal();
    if (conversionModal?.classList.contains("active")) closeConversionModal();
  });

  if (conversionForm) {
    conversionForm.addEventListener("submit", handleConversionSubmit);
    conversionDraft = attachDraft(conversionForm, {
      formKey: "conversion",
      recordId: () => editingConversionId,
    });
  }
  if (conversionCancelButton)
    conversionCancelButton.addEventListener("click", closeConversionModal);
  if (conversionCreateButton)
    conversionCreateButton.addEventListener("click", () =>
      openConversionModal(),
    );
  if (conversionBody)
    conversionBody.addEventListener("click", handleConversionTableAction);
  if (document.getElementById("modal-conversion-origen")) {
    document
      .getElementById("modal-conversion-origen")
      .addEventListener("change", updateConversionPreview);
  }
  if (document.getElementById("modal-conversion-destino")) {
    document
      .getElementById("modal-conversion-destino")
      .addEventListener("change", updateConversionPreview);
  }
  if (document.getElementById("modal-conversion-factor")) {
    document
      .getElementById("modal-conversion-factor")
      .addEventListener("input", updateConversionPreview);
  }
  if (conversionModal) {
    conversionModal.addEventListener("click", (event) => {
      if (event.target === conversionModal) closeConversionModal();
    });
  }

  if (conversorOrigen)
    conversorOrigen.addEventListener("change", handleConversorChange);
  if (conversorDestino)
    conversorDestino.addEventListener("change", handleConversorChange);
  if (conversorCantidad)
    conversorCantidad.addEventListener("input", handleConversorChange);
  if (conversorSwap)
    conversorSwap.addEventListener("click", handleConversorSwap);

  await loadUnits();
  await loadConversions();
  populateConversorSelectors();
}

async function loadUnits(searchTerm = "") {
  const body = document.getElementById("unidades-body");
  const feedback = document.getElementById("unidad-feedback");

  if (body) {
    messageRow(body, "Cargando unidades…", 5);
  }

  try {
    const response = await listUnits(searchTerm);
    const items = Array.isArray(response) ? response : (response?.items ?? []);
    units = items;
    renderUnitList(items);
    if (feedback) {
      feedback.textContent = "";
      feedback.hidden = true;
    }
  } catch (error) {
    if (body) {
      messageRow(body, "No se pudieron cargar las unidades.", 5);
    }
    notifyError(error, "No se pudieron cargar las unidades.");
  }
}

function handleUnitSearch(event) {
  const value = event.currentTarget.value.trim();
  void loadUnits(value);
}

async function handleUnitNameInput() {
  const nameValue =
    document.getElementById("modal-unidad-nombre")?.value?.trim() ?? "";
  const abbreviationValue =
    document.getElementById("modal-unidad-abreviacion")?.value?.trim() ?? "";
  const query = nameValue || abbreviationValue;
  const feedback = document.getElementById("unidad-feedback");

  if (!query) {
    if (feedback) {
      feedback.textContent = "";
      feedback.hidden = true;
    }
    return;
  }

  try {
    const matches = await searchUnits(query);
    const items = Array.isArray(matches) ? matches : [];
    const exact = items.find(
      (item) =>
        item?.isExactMatch ||
        normalizeText(item?.name) === normalizeText(nameValue) ||
        normalizeText(item?.abbreviation) === normalizeText(abbreviationValue),
    );

    if (feedback) {
      if (exact) {
        feedback.textContent = normalizeUnitFeedbackMessage(
          `Ya existe una unidad con ese ${
            normalizeText(nameValue) === normalizeText(exact.name)
              ? "nombre"
              : "abreviación"
          }`,
        );
        feedback.hidden = false;
      } else if (items.length) {
        feedback.textContent = "Coincidencia de unidad disponible.";
        feedback.hidden = false;
      } else {
        feedback.textContent = "";
        feedback.hidden = true;
      }
    }
  } catch {
    if (feedback) {
      feedback.textContent = "";
      feedback.hidden = true;
    }
  }
}

function normalizeText(value) {
  return String(value ?? "")
    .trim()
    .toLowerCase();
}

function normalizeUnitFeedbackMessage(message) {
  const text = String(message ?? "").trim();
  if (!text) return "";
  if (/ya existe una unidad con ese nombre/i.test(text)) {
    return "ya existe una unidad con ese nombre.";
  }
  if (/ya existe una unidad con esa abreviación/i.test(text)) {
    return "ya existe una unidad con esa abreviación.";
  }
  return text;
}

async function handleUnitSubmit(event) {
  event.preventDefault();

  const form = event.currentTarget;
  const nameInput = form.querySelector("#modal-unidad-nombre");
  const abbreviationInput = form.querySelector("#modal-unidad-abreviacion");
  const name = nameInput?.value?.trim();
  const abbreviation = abbreviationInput?.value?.trim();

  clearFieldErrors(form);
  if (!requireFields([nameInput, abbreviationInput])) {
    notifyError(null, "El nombre y la abreviación son obligatorios.");
    return;
  }

  try {
    const wasEditing = editingUnitId !== null;
    let createdUnit = null;
    if (wasEditing) {
      await updateUnit(editingUnitId, name, abbreviation);
    } else {
      createdUnit = await createUnit(name, abbreviation);
    }

    const productCreation = wasEditing ? null : pendingProductUnitCreation;
    pendingProductUnitCreation = null;
    unitDraft?.clear();
    closeUnitModal();
    await loadUnits();
    productCreation?.onCreated?.(createdUnit);
    notifySuccess(`Unidad «${name}» ${wasEditing ? "actualizada" : "creada"}.`);
  } catch (error) {
    reportError(error, "No se pudo guardar la unidad.", [
      [/abreviación/i, abbreviationInput],
      [/nombre/i, nameInput],
    ]);
  }
}

function handleUnitTableAction(event) {
  const editButton = event.target.closest("button[data-unit-action='edit']");
  if (editButton) {
    const id = Number(editButton.dataset.unitId);
    if (Number.isFinite(id)) openUnitModal(id);
    return;
  }

  const deleteButton = event.target.closest(
    "button[data-unit-action='delete']",
  );
  if (deleteButton) {
    const id = Number(deleteButton.dataset.unitId);
    if (Number.isFinite(id)) void handleDeleteUnit(id);
  }
}

function renderUnitList(items) {
  const body = document.getElementById("unidades-body");
  if (!body) return;

  if (!items.length) {
    emptyState(body, {
      empty: "Aún no hay unidades.",
      filtered: Boolean(document.getElementById("unidad-search")?.value.trim()),
      action: { label: "Crear unidad", onClick: () => openUnitModal() },
    });
    return;
  }

  body.replaceChildren(
    ...items.map((unit) => {
      const count = Number(unit.productCount ?? 0);
      return el(
        "tr",
        { dataset: { unitId: unit.id } },
        el("td", {}, `#${String(unit.id).padStart(4, "0")}`),
        el("td", {}, String(unit.name ?? "")),
        el("td", {}, String(unit.abbreviation ?? "")),
        el(
          "td",
          { style: "text-align:center" },
          el(
            "span",
            { class: count > 0 ? "conteo-uso en-uso" : "conteo-uso" },
            String(count),
          ),
        ),
        el(
          "td",
          {},
          el(
            "div",
            { class: "col-acciones" },
            ROW_ACTIONS.map(([action, label, extra]) =>
              el(
                "button",
                {
                  type: "button",
                  class: `action-link${extra}`,
                  dataset: { unitAction: action, unitId: unit.id },
                },
                label,
              ),
            ),
          ),
        ),
      );
    }),
  );
}

const ROW_ACTIONS = [
  ["edit", "Editar", ""],
  ["delete", "Eliminar", " danger"],
];

function openUnitModal(unitId = null, initialName = "", forProduct = false) {
  const modal = document.getElementById("modal-unidad");
  const title = document.getElementById("modal-unidad-title");
  const nameInput = document.getElementById("modal-unidad-nombre");
  const abbreviationInput = document.getElementById("modal-unidad-abreviacion");

  if (!forProduct) pendingProductUnitCreation = null;
  editingUnitId = unitId;

  if (unitId !== null) {
    const unit = units.find((item) => item.id === unitId);
    if (!unit) return;
    if (title) title.textContent = `Editar Unidad: ${unit.name}`;
    if (nameInput) nameInput.value = unit.name ?? "";
    if (abbreviationInput) abbreviationInput.value = unit.abbreviation ?? "";
  } else {
    if (title) title.textContent = "Nueva Unidad de Medida";
    if (nameInput) nameInput.value = initialName;
    if (abbreviationInput) abbreviationInput.value = "";
  }

  if (modal) modal.classList.add("active");
  if (nameInput) setTimeout(() => nameInput.focus(), 100);
  void unitDraft?.offer();
}

registerDraftOpener("unit", async (unitId) => {
  window.showPage?.(
    "unidades",
    document.querySelector('#sidebar [data-page="unidades"]'),
  );
  await loadUnits();
  openUnitModal(unitId ?? null);
});

registerDraftOpener("conversion", async (conversionId) => {
  window.showPage?.(
    "unidades",
    document.querySelector('#sidebar [data-page="unidades"]'),
  );
  await Promise.all([loadUnits(), loadConversions()]);
  openConversionModal(conversionId ?? null);
});

export function openUnitModalForProduct(name, onCreated, onCancelled) {
  pendingProductUnitCreation = { onCreated, onCancelled };
  openUnitModal(null, name, true);
}

function cancelUnitModal() {
  unitDraft?.clear();
  const pendingCreation = pendingProductUnitCreation;
  pendingProductUnitCreation = null;
  closeUnitModal();
  pendingCreation?.onCancelled?.();
}

function closeUnitModal() {
  const modal = document.getElementById("modal-unidad");
  const form = document.getElementById("unidad-form");
  editingUnitId = null;

  if (form) form.reset();
  if (modal) modal.classList.remove("active");
}

async function handleDeleteUnit(unitId) {
  const unit = units.find((item) => item.id === unitId);
  if (!unit) return;

  const productCount = Number(unit.productCount ?? 0);
  const conversionCount = Number(unit.conversionCount ?? 0);

  const confirmed = await confirmDialog({
    title: "Eliminar unidad",
    message:
      productCount > 0 || conversionCount > 0
        ? `La unidad "${unit.name}" tiene ${productCount} producto(s) asignados y ${conversionCount} conversión(es). Si la eliminas, se quitarán esos vínculos. ¿Deseas continuar?`
        : `¿Eliminar la unidad "${unit.name}"?`,
    confirmText: "Eliminar",
  });

  if (!confirmed) return;

  try {
    const result = await deleteUnit(unitId);
    const affectedProducts = Number(result?.affectedProducts ?? productCount);
    const deletedConversions = Number(
      result?.deletedConversions ?? conversionCount,
    );
    await loadUnits();
    await loadConversions();
    populateConversorSelectors();
    notifySuccess(
      `Unidad «${unit.name}» eliminada. ${affectedProducts} producto(s) quedaron sin unidad y ${deletedConversions} conversión(es) se eliminó(n).`,
    );
  } catch (error) {
    notifyError(error, "No se pudo eliminar la unidad.");
  }
}

// ----- Conversiones -----

async function loadConversions(searchTerm = "") {
  const body = document.getElementById("conversiones-body");

  if (body) {
    messageRow(body, "Cargando conversiones…", 5);
  }

  try {
    const response = await listConversions(searchTerm);
    const items = Array.isArray(response) ? response : [];
    conversions = items;
    renderConversionList(items);
  } catch (error) {
    if (body) {
      messageRow(body, "No se pudieron cargar las conversiones.", 5);
    }
    notifyError(error, "No se pudieron cargar las conversiones.");
  }
}

function renderConversionList(items) {
  const body = document.getElementById("conversiones-body");
  if (!body) return;

  if (!items.length) {
    emptyState(body, {
      empty: "Aún no hay conversiones.",
      filtered: Boolean(document.getElementById("unidad-search")?.value.trim()),
      action: {
        label: "Crear conversión",
        onClick: () => openConversionModal(),
      },
    });
    return;
  }

  const unitLabel = (unit) =>
    `${String(unit.name ?? "")} (${String(unit.abbreviation ?? "")})`;
  body.replaceChildren(
    ...items.map((conversion) =>
      el(
        "tr",
        { dataset: { conversionId: conversion.id } },
        el("td", {}, `#${String(conversion.id).padStart(4, "0")}`),
        el("td", {}, unitLabel(conversion.fromUnit)),
        el("td", {}, unitLabel(conversion.toUnit)),
        el(
          "td",
          { style: "text-align:center" },
          String(conversion.factor ?? ""),
        ),
        el(
          "td",
          {},
          el(
            "div",
            { class: "col-acciones" },
            ROW_ACTIONS.map(([action, label, extra]) =>
              el(
                "button",
                {
                  type: "button",
                  class: `action-link${extra}`,
                  dataset: {
                    conversionAction: action,
                    conversionId: conversion.id,
                  },
                },
                label,
              ),
            ),
          ),
        ),
      ),
    ),
  );
}

function handleConversionTableAction(event) {
  const editButton = event.target.closest(
    "button[data-conversion-action='edit']",
  );
  if (editButton) {
    const id = Number(editButton.dataset.conversionId);
    if (Number.isFinite(id)) openConversionModal(id);
    return;
  }

  const deleteButton = event.target.closest(
    "button[data-conversion-action='delete']",
  );
  if (deleteButton) {
    const id = Number(deleteButton.dataset.conversionId);
    if (Number.isFinite(id)) void handleDeleteConversion(id);
  }
}

function openConversionModal(conversionId = null) {
  const modal = document.getElementById("modal-conversion");
  const title = document.getElementById("modal-conversion-title");
  const origenSelect = document.getElementById("modal-conversion-origen");
  const destinoSelect = document.getElementById("modal-conversion-destino");
  const factorInput = document.getElementById("modal-conversion-factor");
  const preview = document.getElementById("modal-conversion-preview");

  editingConversionId = conversionId;

  if (conversionId !== null) {
    const conversion = conversions.find((item) => item.id === conversionId);
    if (!conversion) return;
    if (title) title.textContent = `Editar Conversión`;
    if (origenSelect) {
      origenSelect.value = conversion.fromUnit.id;
      origenSelect.disabled = true;
    }
    if (destinoSelect) {
      destinoSelect.value = conversion.toUnit.id;
      destinoSelect.disabled = true;
    }
    if (factorInput) factorInput.value = conversion.factor ?? "";
    if (preview) {
      preview.textContent = `1 ${conversion.fromUnit.abbreviation} = ${conversion.factor} ${conversion.toUnit.abbreviation}`;
    }
  } else {
    if (title) title.textContent = "Nueva Conversión";
    if (origenSelect) {
      origenSelect.value = "";
      origenSelect.disabled = false;
    }
    if (destinoSelect) {
      destinoSelect.value = "";
      destinoSelect.disabled = false;
    }
    if (factorInput) factorInput.value = "";
    if (preview) preview.textContent = "—";
  }

  populateConversionSelectors(origenSelect, destinoSelect);
  updateConversionPreview();

  if (modal) modal.classList.add("active");
  if (factorInput) setTimeout(() => factorInput.focus(), 100);
  void conversionDraft?.offer();
}

function updateConversionPreview() {
  const origenSelect = document.getElementById("modal-conversion-origen");
  const destinoSelect = document.getElementById("modal-conversion-destino");
  const factorInput = document.getElementById("modal-conversion-factor");
  const preview = document.getElementById("modal-conversion-preview");

  if (!origenSelect || !destinoSelect || !factorInput || !preview) return;

  const fromUnitId = Number(origenSelect.value);
  const toUnitId = Number(destinoSelect.value);
  const factor = factorInput.value.trim();

  const fromUnit = units.find((unit) => unit.id === fromUnitId);
  const toUnit = units.find((unit) => unit.id === toUnitId);

  if (!fromUnit || !toUnit || !factor) {
    preview.textContent = "1 unidad de origen = — unidad de destino";
    return;
  }

  const fromLabel = fromUnit.abbreviation || fromUnit.name || "unidad";
  const toLabel = toUnit.abbreviation || toUnit.name || "unidad";
  preview.textContent = `1 ${fromLabel} = ${factor} ${toLabel}`;
}

function conversionLabel(fromUnitId, toUnitId) {
  const unitName = (id) => units.find((unit) => unit.id === id)?.name ?? "";
  return `${unitName(fromUnitId)} → ${unitName(toUnitId)}`;
}

function closeConversionModal() {
  conversionDraft?.clear();
  const modal = document.getElementById("modal-conversion");
  const form = document.getElementById("conversion-form");
  editingConversionId = null;

  if (form) form.reset();
  if (modal) modal.classList.remove("active");
}

function populateConversionSelectors(origenSelect, destinoSelect) {
  if (!origenSelect || !destinoSelect) return;

  const options = units.map((unit) => [
    unit.id,
    `${unit.name} (${unit.abbreviation})`,
  ]);
  setOptions(origenSelect, options);
  setOptions(destinoSelect, options);
}

async function handleConversionSubmit(event) {
  event.preventDefault();

  const form = event.currentTarget;
  const origenSelect = form.querySelector("#modal-conversion-origen");
  const destinoSelect = form.querySelector("#modal-conversion-destino");
  const factorInput = form.querySelector("#modal-conversion-factor");

  const fromUnitId = Number(origenSelect?.value);
  const toUnitId = Number(destinoSelect?.value);
  const factor = factorInput?.value?.trim();

  if (!Number.isFinite(fromUnitId) || !Number.isFinite(toUnitId) || !factor) {
    notifyError(null, "Selecciona ambas unidades y el factor.");
    return;
  }

  // closeConversionModal limpia editingConversionId: se guarda antes.
  const wasEditing = editingConversionId !== null;
  try {
    if (wasEditing) {
      await updateConversionFactor(editingConversionId, factor);
    } else {
      await createConversion(fromUnitId, toUnitId, factor);
    }

    closeConversionModal();
    await loadConversions();
    notifySuccess(
      `Conversión ${conversionLabel(fromUnitId, toUnitId)} ${wasEditing ? "actualizada" : "creada"}.`,
    );
  } catch (error) {
    reportError(error, "No se pudo guardar la conversión.", [
      [/factor/i, factorInput],
    ]);
  }
}

async function handleDeleteConversion(conversionId) {
  const conversion = conversions.find((item) => item.id === conversionId);
  if (!conversion) return;

  const confirmed = await confirmDialog({
    title: "Eliminar conversión",
    message: `¿Eliminar la conversión entre "${conversion.fromUnit.name}" y "${conversion.toUnit.name}"? Esta acción no se puede deshacer.`,
    confirmText: "Eliminar",
  });

  if (!confirmed) return;

  try {
    await deleteConversion(conversionId);
    await loadConversions();
    notifySuccess(
      `Conversión ${conversion.fromUnit.name} → ${conversion.toUnit.name} eliminada.`,
    );
  } catch (error) {
    notifyError(error, "No se pudo eliminar la conversión.");
  }
}

// ----- Conversor Rápido -----

function populateConversorSelectors() {
  const origenSelect = document.getElementById("conversor-origen");
  const destinoSelect = document.getElementById("conversor-destino");

  if (!origenSelect || !destinoSelect) return;

  const options = units.map((unit) => [
    unit.id,
    `${unit.name} (${unit.abbreviation})`,
  ]);
  setOptions(origenSelect, options);
  setOptions(destinoSelect, options);

  if (units[0]) origenSelect.value = units[0].id;
  if (units[1]) destinoSelect.value = units[1].id;

  handleConversorChange();
}

async function handleConversorChange() {
  const origenSelect = document.getElementById("conversor-origen");
  const destinoSelect = document.getElementById("conversor-destino");
  const cantidadInput = document.getElementById("conversor-cantidad");
  const resultado = document.getElementById("conversor-resultado");

  if (!origenSelect || !destinoSelect || !cantidadInput || !resultado) return;

  const fromUnitId = Number(origenSelect.value);
  const toUnitId = Number(destinoSelect.value);
  const quantity = cantidadInput.value.trim();

  if (!quantity) {
    resultado.textContent = "—";
    resultado.classList.remove("sin-ruta");
    return;
  }

  if (fromUnitId === toUnitId) {
    resultado.textContent = quantity;
    resultado.classList.remove("sin-ruta");
    return;
  }

  try {
    const response = await convertQuantity(fromUnitId, toUnitId, quantity);
    resultado.textContent = response?.result ?? "—";
    resultado.classList.remove("sin-ruta");
  } catch (error) {
    // Solo los errores del backend traen un texto apto para el usuario.
    resultado.textContent = error?.code
      ? error.message
      : "No se pudo calcular la conversión.";
    resultado.classList.add("sin-ruta");
  }
}

function handleConversorSwap() {
  const origenSelect = document.getElementById("conversor-origen");
  const destinoSelect = document.getElementById("conversor-destino");

  if (!origenSelect || !destinoSelect) return;

  const temp = origenSelect.value;
  origenSelect.value = destinoSelect.value;
  destinoSelect.value = temp;

  handleConversorChange();
}

void initUnidadesView();
