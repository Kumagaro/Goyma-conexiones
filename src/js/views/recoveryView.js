import {
  recoverPassword,
  verifyRecoveryIdentity,
} from "../services/authService.js";
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
  const feedback = document.getElementById("recoveryFeedback");
  const submitButton = document.getElementById("verifyRecoveryButton");

  if (feedback) feedback.textContent = "";
  if (submitButton) submitButton.disabled = true;

  try {
    const result = await verifyRecoveryIdentity(username, phone, nationalId);
    if (result?.code) {
      if (feedback)
        feedback.textContent = result.message || RECOVERY_ERROR_MESSAGE;
      return;
    }

    form.classList.add("step-hidden");
    document
      .getElementById("changePasswordForm")
      .classList.remove("step-hidden");
  } catch (error) {
    if (feedback) {
      feedback.textContent =
        typeof error?.message === "string"
          ? error.message
          : RECOVERY_ERROR_MESSAGE;
    }
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
  const feedback = document.getElementById("changePasswordFeedback");
  const submitButton = document.getElementById("changePasswordButton");

  confirmPassword.setCustomValidity(
    newPassword === confirmPassword.value ? "" : "Las contraseñas no coinciden",
  );

  if (!form.checkValidity()) {
    form.reportValidity();
    return;
  }

  if (feedback) feedback.textContent = "";
  if (submitButton) submitButton.disabled = true;

  try {
    const result = await recoverPassword(
      username,
      phone,
      nationalId,
      newPassword,
    );
    if (result?.code) {
      if (feedback)
        feedback.textContent = result.message || RECOVERY_ERROR_MESSAGE;
      return;
    }

    if (feedback) {
      feedback.textContent =
        "Contraseña restablecida correctamente. Redirigiendo al login...";
      feedback.classList.add("is-success");
    }

    setTimeout(() => {
      window.location.replace("./index.html");
    }, 2000);
  } catch (error) {
    if (feedback) {
      feedback.textContent =
        typeof error?.message === "string"
          ? error.message
          : RECOVERY_ERROR_MESSAGE;
    }
  } finally {
    if (submitButton) submitButton.disabled = false;
  }
}

void initRecoveryView();
