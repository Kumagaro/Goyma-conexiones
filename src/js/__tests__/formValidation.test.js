import { describe, expect, it } from "vitest";

import { installFormValidation } from "../utils/formValidation.js";

describe("mensajes nativos de validación", () => {
  it("muestra el mensaje de teléfono en español y lo limpia al editar", () => {
    const form = document.createElement("form");
    form.innerHTML = '<input id="phone" pattern="^\\+53[0-9]{8}$" required>';
    document.body.append(form);
    installFormValidation(form);
    const phone = form.querySelector("#phone");
    phone.value = "123";

    phone.dispatchEvent(new Event("invalid", { cancelable: true }));
    expect(phone.validationMessage).toBe(
      "El teléfono debe tener el formato +53 seguido de 8 dígitos",
    );

    phone.value = "+5350000000";
    phone.dispatchEvent(new Event("input", { bubbles: true }));
    expect(phone.validationMessage).toBe("");
    form.remove();
  });

  it("muestra los errores de nombre de usuario y carnet en español", () => {
    const form = document.createElement("form");
    form.innerHTML = `
      <input id="username" minlength="3" maxlength="50" required>
      <input id="nationalId" pattern="^[0-9]{11}$" required>
    `;
    document.body.append(form);
    installFormValidation(form);
    const username = form.querySelector("#username");
    const nationalId = form.querySelector("#nationalId");

    username.value = "ab";
    username.dispatchEvent(new Event("invalid", { cancelable: true }));
    expect(username.validationMessage).toBe(
      "El nombre de usuario debe tener de 3 a 50 caracteres alfanuméricos (letras y números, sin acentos ni espacios)",
    );

    nationalId.value = "123";
    nationalId.dispatchEvent(new Event("invalid", { cancelable: true }));
    expect(nationalId.validationMessage).toBe(
      "El carnet de identidad debe tener exactamente 11 dígitos",
    );
    form.remove();
  });
});
