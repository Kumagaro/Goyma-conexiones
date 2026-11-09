import { attachDraft, registerDraftOpener } from "../utils/drafts.js";
// Configuración con datos reales (spec-04 CF-01 a CF-04). Sustituye a
// monedasData, historialTasasData y rateCardsData del mockup.
import {
  getCompanyProfile,
  getSettings,
  getUserPreferences,
  listExchangeRateHistory,
  listExchangeRates,
  saveCompanyLogo,
  updateCompanyProfile,
  updateExchangeRate,
  updateSettings,
  updateUserPreferences,
} from "../services/settingsService.js";
import {
  CURRENCY_LABELS,
  currenciesWithoutRate,
  parseRate,
} from "../utils/currency.js";
import { bindDateSort, sortByDate } from "../utils/dateSort.js";
import { el, messageRow } from "../utils/dom.js";
import { formatTimestamp } from "../utils/formatters.js";
import {
  clearFieldErrors,
  notifyError,
  notifySuccess,
  notifyWarning,
  reportError,
} from "../utils/feedback.js";

const PAGE_ID = "configuracion";
const CURRENCIES = Object.keys(CURRENCY_LABELS);
const HISTORY_HEADER = "th-fecha-historial-tasas";

let companyDraft = null;
let ratesDraft = null;
let thresholdsDraft = null;
let preferences = { visibleCurrencies: [], pageSize: 20 };
let rates = [];
let history = [];

const byId = (id) => document.getElementById(id);

function claim(node) {
  if (!node || node.dataset.configBound === "1") return false;
  node.dataset.configBound = "1";
  return true;
}

// ---------- Preferencias: tamaño de página y monedas ----------

/** El paginador global (dashboard.js) usa el tamaño guardado del usuario. */
function applyPageSize(size) {
  const select = byId("config-elementos-pagina");
  if (select) select.value = String(size);
  window.establecerTamPagina?.(size);
}

async function savePreferences(next, successMessage) {
  const saved = await updateUserPreferences(next);
  preferences = saved ?? next;
  if (successMessage) notifySuccess(successMessage);
  return preferences;
}

function renderCurrencies() {
  const grid = byId("currency-grid");
  if (!grid) return;
  grid.replaceChildren(
    ...CURRENCIES.map((code) =>
      el(
        "label",
        { class: "currency-item" },
        el(
          "span",
          { class: "code" },
          el("span", { class: "currency-flag" }, code.slice(0, 3)),
          CURRENCY_LABELS[code],
        ),
        el(
          "span",
          { class: "switch" },
          el("input", {
            type: "checkbox",
            dataset: { currency: code },
            checked: preferences.visibleCurrencies.includes(code),
            "aria-label": `Mostrar ${CURRENCY_LABELS[code]}`,
          }),
          el("span", { class: "track" }),
        ),
      ),
    ),
  );
  renderCurrencyWarning();
}

/** CF-01: una moneda elegida sin tasa no se muestra; aquí se avisa. */
function renderCurrencyWarning() {
  const warning = byId("config-moneda-aviso");
  if (!warning) return;
  const missing = currenciesWithoutRate(preferences.visibleCurrencies, rates);
  warning.hidden = missing.length === 0;
  warning.textContent = missing.length
    ? `Sin tasa de cambio: ${missing.map((code) => CURRENCY_LABELS[code]).join(", ")}. No se mostrará hasta que la fijes en la pestaña Tasas.`
    : "";
}

function selectedCurrencies() {
  return [...document.querySelectorAll("#currency-grid [data-currency]")]
    .filter((input) => input.checked)
    .map((input) => input.dataset.currency);
}

// ---------- Tasas e historial ----------

function renderRateCards() {
  const wrap = byId("rate-cards");
  if (!wrap) return;
  const current = new Map(rates.map((rate) => [rate.currency, rate]));
  wrap.replaceChildren(
    ...CURRENCIES.map((code, index) => {
      const rate = current.get(code);
      const result = el("span", { class: "rate-result" });
      const input = el("input", {
        type: "text",
        inputmode: "decimal",
        id: `config-tasa-${code}`,
        placeholder: "0,000000",
        "aria-label": `Tasa de ${CURRENCY_LABELS[code]}`,
        dataset: { currency: code },
      });
      input.value = rate?.rate ?? "";
      const showResult = () => {
        const value = parseRate(input.value);
        result.textContent = value
          ? `1 USD = ${input.value.trim()} ${CURRENCY_LABELS[code]}`
          : `1 USD = — ${CURRENCY_LABELS[code]}`;
      };
      input.addEventListener("input", showResult);
      showResult();
      return el(
        "div",
        { class: "rate-card" },
        el(
          "div",
          { class: "rate-card-head" },
          el("span", { class: "rate-step" }, String(index + 1)),
          el(
            "div",
            {},
            el("div", { class: "code" }, CURRENCY_LABELS[code]),
            el(
              "div",
              { class: "desc" },
              rate
                ? `Actualizada por ${rate.updatedByName}`
                : "Sin tasa configurada",
            ),
          ),
        ),
        el(
          "div",
          { class: "rate-row" },
          el("label", { for: input.id }, "Tasa"),
          input,
          result,
        ),
      );
    }),
  );
}

function rateText(rate, currency) {
  return rate
    ? `1 USD = ${rate} ${CURRENCY_LABELS[currency] ?? currency}`
    : "—";
}

function renderHistory() {
  const body = byId("historial-tasas-body");
  if (!body) return;
  if (!history.length) {
    messageRow(body, "Aún no hay cambios de tasas.", 6);
    return;
  }
  body.replaceChildren(
    ...sortByDate(history, HISTORY_HEADER, "createdAt").map((change) =>
      el(
        "tr",
        {},
        el("td", {}, formatTimestamp(change.createdAt)),
        el("td", {}, CURRENCY_LABELS[change.currency] ?? change.currency),
        el("td", {}, rateText(change.previousRate, change.currency)),
        el("td", {}, rateText(change.newRate, change.currency)),
        el("td", {}, "USD"),
        el("td", {}, change.userName),
      ),
    ),
  );
}

async function loadRates() {
  [rates, history] = await Promise.all([
    listExchangeRates().then((items) => items ?? []),
    listExchangeRateHistory().then((items) => items ?? []),
  ]);
  renderRateCards();
  renderHistory();
  renderCurrencyWarning();
}

const rateInputs = () => [
  ...document.querySelectorAll("#rate-cards [data-currency]"),
];

function serializeRates() {
  return Object.fromEntries(
    rateInputs().map((input) => [input.dataset.currency, input.value]),
  );
}

function restoreRates(values) {
  for (const input of rateInputs()) {
    if (!(input.dataset.currency in values)) continue;
    input.value = values[input.dataset.currency];
    input.dispatchEvent(new Event("input", { bubbles: true }));
  }
}

/** Guarda solo las tasas que cambiaron; cada una deja su historial. */
async function saveRates() {
  const current = new Map(rates.map((rate) => [rate.currency, rate.rate]));
  const changed = rateInputs()
    .map((input) => [input.dataset.currency, input.value.trim(), input])
    .filter(([code, value]) => value && value !== current.get(code));
  if (!changed.length) {
    notifyWarning("No hay tasas nuevas que guardar.");
    return;
  }
  clearFieldErrors(byId("rate-cards"));
  let saved = 0;
  for (const [code, value, input] of changed) {
    try {
      await updateExchangeRate(code, value);
      saved += 1;
    } catch (error) {
      reportError(error, "No se pudo guardar la tasa.", [[/tasa/i, input]]);
    }
  }
  if (saved === changed.length) ratesDraft?.clear();
  if (saved) {
    notifySuccess(
      saved === 1
        ? "Tasa de cambio guardada."
        : `${saved} tasas de cambio guardadas.`,
    );
  }
  await loadRates();
}

// ---------- Umbrales ----------

async function loadThresholds() {
  const settings = await getSettings();
  byId("config-umbral-facturas").value = String(settings.staleInvoiceDays);
  byId("config-umbral-respaldo").value = String(settings.backupReminderDays);
}

const THRESHOLD_FIELDS = [
  [/factura pendiente/i, "config-umbral-facturas"],
  [/respaldo/i, "config-umbral-respaldo"],
];

async function saveThresholds(event) {
  event.preventDefault();
  const form = event.currentTarget;
  clearFieldErrors(form);
  try {
    await updateSettings({
      staleInvoiceDays: Number(byId("config-umbral-facturas").value),
      backupReminderDays: Number(byId("config-umbral-respaldo").value),
    });
    thresholdsDraft?.clear();
    notifySuccess("Umbrales guardados.");
  } catch (error) {
    reportError(
      error,
      "No se pudieron guardar los umbrales.",
      THRESHOLD_FIELDS,
    );
  }
}

// ---------- Empresa y logo ----------

function renderCompany(profile) {
  byId("config-empresa-nombre").value = profile?.name ?? "";
  byId("config-empresa-direccion").value = profile?.address ?? "";
  byId("config-empresa-telefono").value = profile?.phone ?? "";
  byId("config-empresa-nit").value = profile?.taxId ?? "";
  renderLogo(profile?.logoDataUrl ?? null);
}

function renderLogo(dataUrl) {
  const preview = byId("config-logo-preview");
  if (preview) {
    preview.hidden = !dataUrl;
    if (dataUrl) preview.src = dataUrl;
    else preview.removeAttribute("src");
  }
  const empty = byId("config-logo-empty");
  if (empty) empty.hidden = Boolean(dataUrl);
  const remove = byId("config-logo-quitar");
  if (remove) remove.hidden = !dataUrl;
}

const COMPANY_FIELDS = [
  [/nombre/i, "config-empresa-nombre"],
  [/dirección/i, "config-empresa-direccion"],
  [/teléfono/i, "config-empresa-telefono"],
  [/nit/i, "config-empresa-nit"],
];

async function saveCompany(event) {
  event.preventDefault();
  const form = event.currentTarget;
  clearFieldErrors(form);
  try {
    renderCompany(
      await updateCompanyProfile({
        name: byId("config-empresa-nombre").value,
        address: byId("config-empresa-direccion").value,
        phone: byId("config-empresa-telefono").value,
        taxId: byId("config-empresa-nit").value,
      }),
    );
    companyDraft?.clear();
    notifySuccess("Datos de la empresa guardados.");
  } catch (error) {
    reportError(
      error,
      "No se pudieron guardar los datos de la empresa.",
      COMPANY_FIELDS,
    );
  }
}

const MAX_LOGO_BYTES = 2 * 1024 * 1024;

/** D-17: el PNG o JPG elegido se dibuja en un canvas y se envía en JPEG. */
async function fileToJpeg(file) {
  const url = URL.createObjectURL(file);
  try {
    const image = new Image();
    image.src = url;
    await image.decode();
    const canvas = document.createElement("canvas");
    canvas.width = image.naturalWidth;
    canvas.height = image.naturalHeight;
    const context = canvas.getContext("2d");
    // Fondo blanco: el JPEG no tiene transparencia.
    context.fillStyle = "#ffffff";
    context.fillRect(0, 0, canvas.width, canvas.height);
    context.drawImage(image, 0, 0);
    const blob = await new Promise((resolve) =>
      canvas.toBlob(resolve, "image/jpeg", 0.9),
    );
    if (!blob) throw new Error("conversión fallida");
    return Array.from(new Uint8Array(await blob.arrayBuffer()));
  } finally {
    URL.revokeObjectURL(url);
  }
}

async function chooseLogo(event) {
  const input = event.currentTarget;
  const file = input.files?.[0];
  input.value = "";
  if (!file) return;
  if (!["image/png", "image/jpeg"].includes(file.type)) {
    notifyError(null, "El logo debe ser una imagen PNG o JPG válida");
    return;
  }
  if (file.size > MAX_LOGO_BYTES) {
    notifyError(null, "El logo no puede superar los 2 MB");
    return;
  }
  let jpeg;
  try {
    jpeg = await fileToJpeg(file);
  } catch {
    notifyError(null, "El logo debe ser una imagen PNG o JPG válida");
    return;
  }
  try {
    renderCompany(await saveCompanyLogo(jpeg));
    notifySuccess("Logo actualizado.");
  } catch (error) {
    notifyError(error, "No se pudo guardar el logo.");
  }
}

async function removeLogo() {
  try {
    renderCompany(await saveCompanyLogo(null));
    notifySuccess("Logo quitado.");
  } catch (error) {
    notifyError(error, "No se pudo quitar el logo.");
  }
}

// ---------- Carga y enlaces ----------

export async function loadConfiguration() {
  try {
    const [company, prefs] = await Promise.all([
      getCompanyProfile(),
      getUserPreferences(),
      loadThresholds(),
      loadRates(),
    ]);
    preferences = prefs ?? preferences;
    renderCompany(company);
    renderCurrencies();
    applyPageSize(preferences.pageSize);
    // Uno tras otro: los dos preguntan con un diálogo.
    await companyDraft?.offer();
    await thresholdsDraft?.offer();
    await ratesDraft?.offer();
  } catch (error) {
    notifyError(error, "No se pudo cargar la configuración.");
  }
}

function bindPage() {
  if (claim(byId("config-empresa-form"))) {
    byId("config-empresa-form").addEventListener("submit", saveCompany);
    companyDraft = attachDraft(byId("config-empresa-form"), {
      formKey: "company",
    });
  }
  if (claim(byId("config-umbrales-form"))) {
    byId("config-umbrales-form").addEventListener("submit", saveThresholds);
    thresholdsDraft = attachDraft(byId("config-umbrales-form"), {
      formKey: "thresholds",
    });
  }
  if (claim(byId("rate-cards"))) {
    ratesDraft = attachDraft(byId("rate-cards"), {
      formKey: "rates",
      serialize: serializeRates,
      restore: restoreRates,
    });
  }
  if (claim(byId("config-logo-elegir"))) {
    byId("config-logo-elegir").addEventListener("click", () =>
      byId("config-logo-input")?.click(),
    );
  }
  if (claim(byId("config-logo-input"))) {
    byId("config-logo-input").addEventListener("change", chooseLogo);
  }
  if (claim(byId("config-logo-quitar"))) {
    byId("config-logo-quitar").addEventListener("click", removeLogo);
  }
  if (claim(byId("config-moneda-guardar"))) {
    byId("config-moneda-guardar").addEventListener("click", async () => {
      try {
        await savePreferences(
          { ...preferences, visibleCurrencies: selectedCurrencies() },
          "Monedas visibles guardadas.",
        );
        renderCurrencies();
      } catch (error) {
        notifyError(error, "No se pudieron guardar las monedas.");
      }
    });
  }
  if (claim(byId("config-tasas-guardar"))) {
    byId("config-tasas-guardar").addEventListener(
      "click",
      () => void saveRates(),
    );
  }
  if (claim(byId("config-elementos-pagina"))) {
    byId("config-elementos-pagina").addEventListener(
      "change",
      async (event) => {
        const pageSize = Number(event.currentTarget.value);
        try {
          await savePreferences(
            { ...preferences, pageSize },
            `Se mostrarán ${pageSize} elementos por página en todas las tablas.`,
          );
          applyPageSize(pageSize);
        } catch (error) {
          notifyError(error, "No se pudo guardar el tamaño de página.");
          applyPageSize(preferences.pageSize);
        }
      },
    );
  }
  bindDateSort(HISTORY_HEADER, "historialTasas", renderHistory, claim);
}

export async function initConfiguracionView() {
  if (!byId("page-configuracion")) return;
  bindPage();
  if (!window.__configuracionPageChangeBound) {
    window.addEventListener("page:change", (event) => {
      if (event.detail?.pageId === PAGE_ID) void loadConfiguration();
    });
    window.__configuracionPageChangeBound = true;
  }
  // El tamaño de página del usuario se aplica desde la entrada (CF-04).
  try {
    preferences = (await getUserPreferences()) ?? preferences;
    applyPageSize(preferences.pageSize);
  } catch {
    // Sin preferencias se queda el tamaño por defecto.
  }
}

const openConfiguration = () => {
  window.showPage?.(
    "configuracion",
    document.querySelector('#sidebar [data-page="configuracion"]'),
  );
};
registerDraftOpener("company", openConfiguration);
registerDraftOpener("thresholds", openConfiguration);
registerDraftOpener("rates", openConfiguration);

export const __testHooks = { loadConfiguration, fileToJpeg };

if (document.readyState === "loading") {
  document.addEventListener(
    "DOMContentLoaded",
    () => void initConfiguracionView(),
  );
} else {
  void initConfiguracionView();
}
