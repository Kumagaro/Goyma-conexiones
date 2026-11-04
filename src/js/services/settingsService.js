import { invoke } from "./invoke.js";

export function getSettings() {
  return invoke("get_settings");
}

// input: { lowStockThreshold, staleInvoiceDays, backupReminderDays } (CF-04).
export function updateSettings(input) {
  return invoke("update_settings", { input });
}

export function getUserPreferences() {
  return invoke("get_user_preferences");
}

// input: { visibleCurrencies: string[], pageSize } (CF-01, CF-04).
export function updateUserPreferences(input) {
  return invoke("update_user_preferences", { input });
}

export function listExchangeRates() {
  return invoke("list_exchange_rates");
}

export function updateExchangeRate(currency, rate) {
  return invoke("update_exchange_rate", { currency, rate });
}

export function listExchangeRateHistory() {
  return invoke("list_exchange_rate_history");
}

export function getCompanyProfile() {
  return invoke("get_company_profile");
}

// input: { name, address, phone, taxId } (CF-03).
export function updateCompanyProfile(input) {
  return invoke("update_company_profile", { input });
}

// jpeg: bytes del JPEG (D-17) o null para quitar el logo.
export function saveCompanyLogo(jpeg) {
  return invoke("save_company_logo", { jpeg });
}
