import { confirmDialog } from "../components/confirmDialog.js";
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

  if (form) form.addEventListener("submit", handleUnitSubmit);
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

  if (conversionForm)
    conversionForm.addEventListener("submit", handleConversionSubmit);
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
    body.innerHTML =
      '<tr><td colspan="5" class="empty-row">Cargando unidades…</td></tr>';
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
      body.innerHTML =
        '<tr><td colspan="5" class="empty-row">No se pudieron cargar las unidades.</td></tr>';
    }
    if (feedback) {
      feedback.textContent =
        error?.message || "No se pudieron cargar las unidades.";
      feedback.hidden = false;
    }
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

  if (!name || !abbreviation) {
    showUnitFeedback("El nombre y la abreviación son obligatorios.");
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
    closeUnitModal();
    await loadUnits();
    productCreation?.onCreated?.(createdUnit);
    showUnitFeedback(
      wasEditing
        ? "Unidad actualizada correctamente."
        : "Unidad creada correctamente.",
    );
  } catch (error) {
    showUnitFeedback(
      normalizeUnitFeedbackMessage(
        error?.message || "No se pudo guardar la unidad.",
      ),
    );
  }
}

function showUnitFeedback(message) {
  const feedback = document.getElementById("unidad-feedback");
  if (!feedback) return;
  feedback.textContent = normalizeUnitFeedbackMessage(message);
  feedback.hidden = false;
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
    body.innerHTML =
      '<tr><td colspan="5" class="empty-row">Aún no hay unidades.</td></tr>';
    return;
  }

  body.innerHTML = items
    .map(
      (unit) => `
        <tr data-unit-id="${unit.id}">
          <td>#${String(unit.id).padStart(4, "0")}</td>
          <td>${escapeHtml(String(unit.name ?? ""))}</td>
          <td>${escapeHtml(String(unit.abbreviation ?? ""))}</td>
          <td style="text-align:center">
            <span class="conteo-uso ${Number(unit.productCount ?? 0) > 0 ? "en-uso" : ""}">
              ${Number(unit.productCount ?? 0)}
            </span>
          </td>
          <td>
            <div class="col-acciones">
              <button type="button" class="action-link" data-unit-action="edit" data-unit-id="${unit.id}">
                Editar
              </button>
              <button type="button" class="action-link danger" data-unit-action="delete" data-unit-id="${unit.id}">
                Eliminar
              </button>
            </div>
          </td>
        </tr>
      `,
    )
    .join("");
}

function escapeHtml(value) {
  return String(value)
    .replaceAll("&", "&amp;")
    .replaceAll("<", "&lt;")
    .replaceAll(">", "&gt;")
    .replaceAll('"', "&quot;")
    .replaceAll("'", "&#39;");
}

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
}

export function openUnitModalForProduct(name, onCreated, onCancelled) {
  pendingProductUnitCreation = { onCreated, onCancelled };
  openUnitModal(null, name, true);
}

function cancelUnitModal() {
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

  const confirmed = await confirmDialog(
    productCount > 0 || conversionCount > 0
      ? `La unidad "${unit.name}" tiene ${productCount} producto(s) asignados y ${conversionCount} conversión(es). Si la eliminas, se quitarán esos vínculos. ¿Deseas continuar?`
      : `¿Eliminar la unidad "${unit.name}"?`,
  );

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
    showUnitFeedback(
      `Unidad eliminada. ${affectedProducts} producto(s) quedaron sin unidad y ${deletedConversions} conversión(es) se eliminó(n).`,
    );
  } catch (error) {
    showUnitFeedback(error?.message || "No se pudo eliminar la unidad.");
  }
}

// ----- Conversiones -----

async function loadConversions(searchTerm = "") {
  const body = document.getElementById("conversiones-body");
  const feedback = document.getElementById("conversion-feedback");

  if (body) {
    body.innerHTML =
      '<tr><td colspan="5" class="empty-row">Cargando conversiones…</td></tr>';
  }

  try {
    const response = await listConversions(searchTerm);
    const items = Array.isArray(response) ? response : [];
    conversions = items;
    renderConversionList(items);
    if (feedback) {
      feedback.textContent = "";
      feedback.hidden = true;
    }
  } catch (error) {
    if (body) {
      body.innerHTML =
        '<tr><td colspan="5" class="empty-row">No se pudieron cargar las conversiones.</td></tr>';
    }
    if (feedback) {
      feedback.textContent =
        error?.message || "No se pudieron cargar las conversiones.";
      feedback.hidden = false;
    }
  }
}

function renderConversionList(items) {
  const body = document.getElementById("conversiones-body");
  if (!body) return;

  if (!items.length) {
    body.innerHTML =
      '<tr><td colspan="5" class="empty-row">Aún no hay conversiones.</td></tr>';
    return;
  }

  body.innerHTML = items
    .map(
      (conversion) => `
        <tr data-conversion-id="${conversion.id}">
          <td>#${String(conversion.id).padStart(4, "0")}</td>
          <td>${escapeHtml(String(conversion.fromUnit.name ?? ""))} (${escapeHtml(String(conversion.fromUnit.abbreviation ?? ""))})</td>
          <td>${escapeHtml(String(conversion.toUnit.name ?? ""))} (${escapeHtml(String(conversion.toUnit.abbreviation ?? ""))})</td>
          <td style="text-align:center">${escapeHtml(String(conversion.factor ?? ""))}</td>
          <td>
            <div class="col-acciones">
              <button type="button" class="action-link" data-conversion-action="edit" data-conversion-id="${conversion.id}">
                Editar
              </button>
              <button type="button" class="action-link danger" data-conversion-action="delete" data-conversion-id="${conversion.id}">
                Eliminar
              </button>
            </div>
          </td>
        </tr>
      `,
    )
    .join("");
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

function closeConversionModal() {
  const modal = document.getElementById("modal-conversion");
  const form = document.getElementById("conversion-form");
  editingConversionId = null;

  if (form) form.reset();
  if (modal) modal.classList.remove("active");
}

function populateConversionSelectors(origenSelect, destinoSelect) {
  if (!origenSelect || !destinoSelect) return;

  const options = units
    .map(
      (unit) => `
        <option value="${unit.id}">${escapeHtml(unit.name)} (${escapeHtml(unit.abbreviation)})</option>
      `,
    )
    .join("");

  origenSelect.innerHTML = options;
  destinoSelect.innerHTML = options;
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
    showConversionFeedback("Selecciona ambas unidades y el factor.");
    return;
  }

  try {
    if (editingConversionId !== null) {
      await updateConversionFactor(editingConversionId, factor);
    } else {
      await createConversion(fromUnitId, toUnitId, factor);
    }

    closeConversionModal();
    await loadConversions();
    showConversionFeedback(
      editingConversionId === null
        ? "Conversión creada correctamente."
        : "Conversión actualizada correctamente.",
    );
  } catch (error) {
    showConversionFeedback(
      error?.message || "No se pudo guardar la conversión.",
    );
  }
}

function showConversionFeedback(message) {
  const feedback = document.getElementById("conversion-feedback");
  if (!feedback) return;
  feedback.textContent = message;
  feedback.hidden = false;
}

async function handleDeleteConversion(conversionId) {
  const conversion = conversions.find((item) => item.id === conversionId);
  if (!conversion) return;

  const confirmed = await confirmDialog(
    `¿Eliminar la conversión entre "${conversion.fromUnit.name}" y "${conversion.toUnit.name}"? Esta acción no se puede deshacer.`,
  );

  if (!confirmed) return;

  try {
    await deleteConversion(conversionId);
    await loadConversions();
    showConversionFeedback("Conversión eliminada correctamente.");
  } catch (error) {
    showConversionFeedback(
      error?.message || "No se pudo eliminar la conversión.",
    );
  }
}

// ----- Conversor Rápido -----

function populateConversorSelectors() {
  const origenSelect = document.getElementById("conversor-origen");
  const destinoSelect = document.getElementById("conversor-destino");

  if (!origenSelect || !destinoSelect) return;

  const options = units
    .map(
      (unit) => `
        <option value="${unit.id}">${escapeHtml(unit.name)} (${escapeHtml(unit.abbreviation)})</option>
      `,
    )
    .join("");

  origenSelect.innerHTML = options;
  destinoSelect.innerHTML = options;

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
    resultado.textContent = response.result;
    resultado.classList.remove("sin-ruta");
  } catch (error) {
    resultado.textContent =
      error?.message || "No se pudo calcular la conversión.";
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
