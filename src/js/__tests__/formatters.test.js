import { describe, expect, it } from "vitest";

import {
  formatCalendarDate,
  formatDecimalText,
  formatId,
  formatLocalTimestamp,
  formatMoney,
  formatRemainingDays,
} from "../utils/formatters.js";

describe("formatters de inventario", () => {
  it("formatea centavos como USD con dos decimales en es-ES", () => {
    const formatted = formatMoney(1250);

    expect(formatted).toContain("12,50");
    expect(formatted).toContain("US$");
    expect(formatMoney(1)).toContain("0,01");
  });

  it("formatea IDs con cinco posiciones mínimas", () => {
    expect(formatId(1)).toBe("#00001");
    expect(formatId(123456)).toBe("#123456");
  });

  it("formatea fechas de calendario sin convertirlas en Date", () => {
    expect(formatCalendarDate("2026-03-01")).toBe("01/03/2026");
    expect(formatCalendarDate(null)).toBe("—");
  });

  it("convierte timestamps UTC a los componentes de hora local", () => {
    const timestamp = "2026-03-01T00:30:00Z";
    const localDate = new Date(timestamp);
    const twoDigits = (value) => String(value).padStart(2, "0");
    const expected = `${twoDigits(localDate.getDate())}/${twoDigits(localDate.getMonth() + 1)}/${localDate.getFullYear()} ${twoDigits(localDate.getHours())}:${twoDigits(localDate.getMinutes())}`;

    expect(formatLocalTimestamp(timestamp)).toBe(expected);
    expect(formatLocalTimestamp(null)).toBe("—");
  });

  it("muestra los días restantes en singular, plural o guion", () => {
    expect(formatRemainingDays(1)).toBe("1 día");
    expect(formatRemainingDays(30)).toBe("30 días");
    expect(formatRemainingDays(null)).toBe("—");
  });

  it("conserva los decimales del backend como texto exacto", () => {
    const decimalText = "999999999,999999";

    expect(formatDecimalText(decimalText)).toBe(decimalText);
  });
});
