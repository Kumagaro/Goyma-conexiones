const numberFormatter = new Intl.NumberFormat("es-ES", {
  minimumFractionDigits: 2,
  maximumFractionDigits: 2,
  useGrouping: "always",
});

const quantityFormatter = new Intl.NumberFormat("es-ES", {
  maximumFractionDigits: 3,
  useGrouping: "always",
});

export function formatMoney(cents) {
  if (!Number.isSafeInteger(cents)) return "—";
  return `${numberFormatter.format(cents / 100)} USD`;
}

export function formatId(id) {
  if (id === null || id === undefined || id === "") return "—";
  return `#${String(id).padStart(5, "0")}`;
}

export function formatDate(dateText) {
  if (typeof dateText !== "string") return "—";
  const match = /^(\d{4})-(\d{2})-(\d{2})$/.exec(dateText);
  if (!match) return "—";
  const [, year, month, day] = match;
  return `${day}/${month}/${year}`;
}

export function formatCalendarDate(dateText) {
  return formatDate(dateText);
}

export function formatQuantity(milli) {
  if (!Number.isSafeInteger(milli)) return "—";
  return quantityFormatter.format(milli / 1000);
}

export function formatTimestamp(timestamp) {
  if (typeof timestamp !== "string") return "—";
  const date = new Date(timestamp);
  if (Number.isNaN(date.getTime())) return "—";

  const twoDigits = (value) => String(value).padStart(2, "0");
  return `${twoDigits(date.getDate())}/${twoDigits(date.getMonth() + 1)}/${date.getFullYear()} ${twoDigits(date.getHours())}:${twoDigits(date.getMinutes())}`;
}

export function formatLocalTimestamp(timestamp) {
  return formatTimestamp(timestamp);
}

export function formatRemainingDays(days) {
  if (!Number.isInteger(days) || days < 0) return "—";
  return `${days} ${days === 1 ? "día" : "días"}`;
}

export function formatDecimalText(value) {
  return typeof value === "string" ? value : "—";
}
