import { clearFieldError, setFieldError } from "./feedback.js";

const messages = {
  username:
    "El nombre de usuario debe tener de 3 a 50 caracteres alfanuméricos (letras y números, sin acentos ni espacios)",
  phone: "El teléfono debe tener el formato +53 seguido de 8 dígitos",
  nationalId: "El carnet de identidad debe tener exactamente 11 dígitos",
  password: "La contraseña debe tener al menos 8 caracteres",
  passwordTooLong: "La contraseña no puede tener más de 72 caracteres",
  required: "Completa este campo",
  phoneTooShort: "El teléfono debe tener exactamente 12 caracteres",
  phoneTooLong: "El teléfono debe tener exactamente 12 caracteres",
  nationalIdTooShort:
    "El carnet de identidad debe tener exactamente 11 dígitos",
  nationalIdTooLong: "El carnet de identidad debe tener exactamente 11 dígitos",
};

const passwordFields = new Set(["password", "newPassword", "confirmPassword"]);

export function installFormValidation(root = document) {
  if (!root?.addEventListener) {
    return () => {};
  }

  let checkingOnBlur = false;

  const handleInvalid = (event) => {
    const field = event.target;
    if (typeof field?.setCustomValidity !== "function") {
      return;
    }

    const message = getValidationMessage(field) || field.validationMessage;
    if (!message) return;
    // El error va junto al campo, no en el globo del navegador (ui-spec §6).
    event.preventDefault();
    field.setCustomValidity(message);
    setFieldError(field, message);
    if (!checkingOnBlur && !document.activeElement?.matches?.(":invalid")) {
      field.focus();
    }
  };

  const clearMessage = (event) => {
    if (typeof event.target?.setCustomValidity === "function") {
      event.target.setCustomValidity("");
      clearFieldError(event.target);
    }
  };

  // Un campo vacío se avisa al guardar: al pasar por él no molesta.
  const checkOnBlur = (event) => {
    const field = event.target;
    if (!field?.willValidate || !field.value) return;
    checkingOnBlur = true;
    field.checkValidity();
    checkingOnBlur = false;
  };

  root.addEventListener("invalid", handleInvalid, true);
  root.addEventListener("input", clearMessage, true);
  root.addEventListener("focusout", checkOnBlur, true);

  return () => {
    root.removeEventListener("invalid", handleInvalid, true);
    root.removeEventListener("input", clearMessage, true);
    root.removeEventListener("focusout", checkOnBlur, true);
  };
}

function getValidationMessage(field) {
  const key = field.dataset.validationField || field.id || field.name;
  const validity = field.validity;

  if (validity.valueMissing) {
    return messages.required;
  }
  const tooShort =
    validity.tooShort ||
    (field.minLength > 0 && field.value.length < field.minLength);
  const tooLong =
    validity.tooLong ||
    (field.maxLength > 0 && field.value.length > field.maxLength);
  if (key === "username" && (tooShort || tooLong || validity.patternMismatch)) {
    return messages.username;
  }
  if (key === "phone" && validity.patternMismatch) {
    return messages.phone;
  }
  if (key === "nationalId" && validity.patternMismatch) {
    return messages.nationalId;
  }
  if (passwordFields.has(key) && tooShort) {
    return messages.password;
  }
  if (passwordFields.has(key) && tooLong) {
    return messages.passwordTooLong;
  }
  if (validity.patternMismatch || tooShort || tooLong) {
    return "Revisa el formato del campo";
  }
  return "";
}
