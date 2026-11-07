// Equivalentes en otras monedas (spec-04 CF-01): solo presentación, con la
// tasa actual (1 USD = tasa) y redondeo a dos decimales. Todo se guarda en
// centavos de USD.

export const CURRENCY_LABELS = {
  EUR: "EUR",
  CUP: "CUP",
  MLC: "MLC",
  SALDO: "Saldo Clásica",
};

/** "670,5" → 670.5; algo que no es una tasa válida → null. */
export function parseRate(rate) {
  const value = Number(String(rate ?? "").replace(",", "."));
  return Number.isFinite(value) && value > 0 ? value : null;
}

// Mismo formato que formatMoney.
const formatter = new Intl.NumberFormat("es-ES", {
  minimumFractionDigits: 2,
  maximumFractionDigits: 2,
  useGrouping: "always",
});

/**
 * Textos "1 234,50 CUP" de las monedas elegidas que tienen tasa; las que no
 * la tienen se omiten (Configuración lo avisa).
 */
export function equivalents(cents, currencies = [], rates = []) {
  if (!Number.isFinite(Number(cents))) return [];
  const byCurrency = new Map(
    rates.map((rate) => [rate.currency, parseRate(rate.rate)]),
  );
  return currencies
    .filter((code) => byCurrency.get(code))
    .map((code) => {
      const amount = (Number(cents) / 100) * byCurrency.get(code);
      return `${formatter.format(Math.round(amount * 100) / 100)} ${CURRENCY_LABELS[code] ?? code}`;
    });
}

/** Monedas elegidas que todavía no tienen tasa configurada. */
export function currenciesWithoutRate(currencies = [], rates = []) {
  const withRate = new Set(
    rates.filter((rate) => parseRate(rate.rate)).map((rate) => rate.currency),
  );
  return currencies.filter((code) => !withRate.has(code));
}
