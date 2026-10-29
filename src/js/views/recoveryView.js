import {
  recoverPassword,
  verifyRecoveryIdentity,
} from "../services/authService.js";
import { notifyError, notifySuccess } from "../utils/feedback.js";
import { installFormValidation } from "../utils/formValidation.js";

const RECOVERY_ERROR_MESSAGE = "Ocurrió un error inesperado";

let initPromise;

export function initRecoveryView() {
  if (!initPromise) initPromise = initializeRecoveryView();
  return initPromise;
}

function initializeRecoveryView() {
  const recoveryForm = document.getElementById("recoveryForm");
  const changePasswordForm = document.getElementById("changePasswordForm");

  if (recoveryForm) {
    installFormValidation(recoveryForm);
    recoveryForm.addEventListener("submit", handleVerifyIdentity);
  }

  if (changePasswordForm) {
    installFormValidation(changePasswordForm);
    changePasswordForm.addEventListener("submit", handleChangePassword);
  }
}

async function handleVerifyIdentity(event) {
  event.preventDefault();

  const form = event.currentTarget;
  if (!form.checkValidity()) {
    form.reportValidity();
    return;
  }

  const username = document.getElementById("username").value.trim();
  const phone = document.getElementById("phone").value.trim();
  const nationalId = document.getElementById("nationalId").value.trim();
  const submitButton = document.getElementById("verifyRecoveryButton");

  if (submitButton) submitButton.disabled = true;

  try {
    const result = await verifyRecoveryIdentity(username, phone, nationalId);
    if (result?.code) {
      notifyError(result, RECOVERY_ERROR_MESSAGE);
      return;
    }

    form.classList.add("step-hidden");
    document
      .getElementById("changePasswordForm")
      .classList.remove("step-hidden");
  } catch (error) {
    notifyError(
      typeof error?.message === "string" ? error : null,
      RECOVERY_ERROR_MESSAGE,
    );
  } finally {
    if (submitButton) submitButton.disabled = false;
  }
}

async function handleChangePassword(event) {
  event.preventDefault();

  const form = event.currentTarget;
  if (!form.checkValidity()) {
    form.reportValidity();
    return;
  }

  const username = document.getElementById("username").value.trim();
  const phone = document.getElementById("phone").value.trim();
  const nationalId = document.getElementById("nationalId").value.trim();
  const newPassword = document.getElementById("newPassword").value;
  const confirmPassword = document.getElementById("confirmPassword");
  const submitButton = document.getElementById("changePasswordButton");

  confirmPassword.setCustomValidity(
    newPassword === confirmPassword.value ? "" : "Las contraseñas no coinciden",
  );

  if (!form.checkValidity()) {
    form.reportValidity();
    return;
  }

  if (submitButton) submitButton.disabled = true;

  try {
    const result = await recoverPassword(
      username,
      phone,
      nationalId,
      newPassword,
    );
    if (result?.code) {
      notifyError(result, RECOVERY_ERROR_MESSAGE);
      return;
    }

    notifySuccess(
      "Contraseña restablecida correctamente. Redirigiendo al login...",
    );

    setTimeout(() => {
      window.location.replace("./index.html");
    }, 2000);
  } catch (error) {
    notifyError(
      typeof error?.message === "string" ? error : null,
      RECOVERY_ERROR_MESSAGE,
    );
  } finally {
    if (submitButton) submitButton.disabled = false;
  }
}

void initRecoveryView();
