const moneyFormatter = new Intl.NumberFormat("es-ES", {
  style: "currency",
  currency: "USD",
  minimumFractionDigits: 2,
  maximumFractionDigits: 2,
});

export function formatMoney(cents) {
  if (!Number.isSafeInteger(cents)) return "—";
  return moneyFormatter.format(cents / 100);
}

export function formatId(id) {
  if (id === null || id === undefined || id === "") return "—";
  return `#${String(id).padStart(5, "0")}`;
}

export function formatCalendarDate(dateText) {
  if (typeof dateText !== "string") return "—";
  const match = /^(\d{4})-(\d{2})-(\d{2})$/.exec(dateText);
  if (!match) return "—";
  const [, year, month, day] = match;
  return `${day}/${month}/${year}`;
}

export function formatLocalTimestamp(timestamp) {
  if (typeof timestamp !== "string") return "—";
  const date = new Date(timestamp);
  if (Number.isNaN(date.getTime())) return "—";

  const twoDigits = (value) => String(value).padStart(2, "0");
  return `${twoDigits(date.getDate())}/${twoDigits(date.getMonth() + 1)}/${date.getFullYear()} ${twoDigits(date.getHours())}:${twoDigits(date.getMinutes())}`;
}

export function formatRemainingDays(days) {
  if (!Number.isInteger(days) || days < 0) return "—";
  return `${days} ${days === 1 ? "día" : "días"}`;
}

export function formatDecimalText(value) {
  return typeof value === "string" ? value : "—";
}
