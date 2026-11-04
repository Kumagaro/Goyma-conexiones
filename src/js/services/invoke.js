let sessionExpiredRedirect = null;

const INVALID_ARGUMENTS_MESSAGE = "Los datos enviados no son válidos";
const UNEXPECTED_ERROR_MESSAGE = "Ocurrió un error inesperado";

export function setSessionExpiredRedirect(redirect) {
  sessionExpiredRedirect = redirect;
}

// Comandos que solo leen: no generan notificaciones (spec-04 TO-02).
const READ_ONLY_COMMAND =
  /^(?:get_|list_|search_|count_)|^(?:touch_session|preview_selling_price|convert_quantity|verify_recovery_identity)$/;

export async function invoke(command, args) {
  try {
    const result = await window.__TAURI__.core.invoke(command, args);
    if (result?.code === "sessionExpired") {
      redirectToExpiredLogin();
    }
    if (command === "touch_session" && result?.status === "expired") {
      redirectToExpiredLogin();
    }
    // TO-02 (D-04): tras una escritura, el feed busca notificaciones nuevas.
    if (!READ_ONLY_COMMAND.test(command)) {
      window.dispatchEvent(
        new CustomEvent("command:success", { detail: { command } }),
      );
    }
    return result;
  } catch (error) {
    if (error?.code === "sessionExpired") {
      redirectToExpiredLogin();
    }
    if (isAppError(error)) throw error;

    const errorText = getErrorText(error);
    console.error(`Error en comando Tauri "${command}":`, errorText);
    const message = isArgumentRejection(errorText)
      ? INVALID_ARGUMENTS_MESSAGE
      : UNEXPECTED_ERROR_MESSAGE;
    throw new Error(message);
  }
}

function isAppError(error) {
  return (
    error !== null &&
    typeof error === "object" &&
    typeof error.code === "string" &&
    typeof error.message === "string"
  );
}

function getErrorText(error) {
  if (typeof error === "string") return error;
  if (typeof error?.message === "string") return error.message;

  try {
    return JSON.stringify(error) ?? String(error);
  } catch {
    return String(error);
  }
}

function isArgumentRejection(errorText) {
  return /invalid args?\b|missing required key\b/i.test(errorText);
}

function redirectToExpiredLogin() {
  const destination = "./index.html?reason=expired";
  if (sessionExpiredRedirect) {
    sessionExpiredRedirect(destination);
    return;
  }
  window.location.replace(destination);
}
