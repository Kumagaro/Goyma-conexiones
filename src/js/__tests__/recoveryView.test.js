import { afterEach, describe, expect, it, vi } from "vitest";
import { toastText } from "./helpers/invoiceUi.js";
import { beforeEach } from "vitest";

const { verifyRecoveryIdentity, recoverPassword } = vi.hoisted(() => ({
  verifyRecoveryIdentity: vi.fn(),
  recoverPassword: vi.fn(),
}));

vi.mock("../services/authService.js", () => ({
  verifyRecoveryIdentity,
  recoverPassword,
}));

describe("vista de recuperación de contraseña", () => {
  beforeEach(() => {
    vi.useFakeTimers();
  });

  afterEach(() => {
    document.body.innerHTML = "";
    delete window.__TAURI__;
    verifyRecoveryIdentity.mockReset();
    recoverPassword.mockReset();
    vi.resetModules();
    vi.useRealTimers();
  });

  it("verifica identidad con datos correctos y muestra el formulario de cambio de contraseña", async () => {
    document.body.innerHTML = `
      <form id="recoveryForm" novalidate>
        <input id="username" name="username" required />
        <input id="phone" name="phone" required pattern="^\\+53[0-9]{8}$" />
        <input id="nationalId" name="nationalId" required pattern="^[0-9]{11}$" />
        <div id="recoveryFeedback" role="status"></div>
        <button id="verifyRecoveryButton" type="submit">Verificar identidad</button>
      </form>
      <form id="changePasswordForm" class="step-hidden" novalidate>
        <input id="newPassword" name="newPassword" required minlength="8" maxlength="128" />
        <input id="confirmPassword" name="confirmPassword" required minlength="8" maxlength="128" />
        <div id="changePasswordFeedback" role="status"></div>
        <button id="changePasswordButton" type="submit">Restablecer Contraseña</button>
      </form>
    `;
    verifyRecoveryIdentity.mockResolvedValue(undefined);

    const recoveryView = await import("../views/recoveryView.js");
    await recoveryView.initRecoveryView();

    document.getElementById("username").value = "testuser";
    document.getElementById("phone").value = "+5350000001";
    document.getElementById("nationalId").value = "12345678901";
    document
      .getElementById("recoveryForm")
      .dispatchEvent(new Event("submit", { bubbles: true, cancelable: true }));

    await vi.waitFor(() =>
      expect(verifyRecoveryIdentity).toHaveBeenCalledOnce(),
    );
    expect(verifyRecoveryIdentity).toHaveBeenCalledWith(
      "testuser",
      "+5350000001",
      "12345678901",
    );
    expect(
      document.getElementById("recoveryForm").classList.contains("step-hidden"),
    ).toBe(true);
    expect(
      document
        .getElementById("changePasswordForm")
        .classList.contains("step-hidden"),
    ).toBe(false);
  });

  it("muestra error genérico cuando la verificación falla", async () => {
    document.body.innerHTML = `
      <form id="recoveryForm" novalidate>
        <input id="username" name="username" required />
        <input id="phone" name="phone" required pattern="^\\+53[0-9]{8}$" />
        <input id="nationalId" name="nationalId" required pattern="^[0-9]{11}$" />
        <div id="recoveryFeedback" role="status"></div>
        <button id="verifyRecoveryButton" type="submit">Verificar identidad</button>
      </form>
      <form id="changePasswordForm" class="step-hidden" novalidate>
        <input id="newPassword" name="newPassword" required minlength="8" maxlength="128" />
        <input id="confirmPassword" name="confirmPassword" required minlength="8" maxlength="128" />
        <div id="changePasswordFeedback" role="status"></div>
        <button id="changePasswordButton" type="submit">Restablecer Contraseña</button>
      </form>
    `;
    verifyRecoveryIdentity.mockResolvedValue({
      code: "validationFailed",
      message: "Los datos no coinciden",
    });

    const recoveryView = await import("../views/recoveryView.js");
    await recoveryView.initRecoveryView();

    document.getElementById("username").value = "testuser";
    document.getElementById("phone").value = "+5350000001";
    document.getElementById("nationalId").value = "12345678901";
    document
      .getElementById("recoveryForm")
      .dispatchEvent(new Event("submit", { bubbles: true, cancelable: true }));

    await vi.waitFor(() =>
      expect(verifyRecoveryIdentity).toHaveBeenCalledOnce(),
    );
    expect(toastText()).toContain("Los datos no coinciden");
    expect(
      document
        .getElementById("changePasswordForm")
        .classList.contains("step-hidden"),
    ).toBe(true);
  });

  it("restablece contraseña con datos correctos y redirige al login", async () => {
    document.body.innerHTML = `
      <form id="recoveryForm" class="step-hidden" novalidate>
        <input id="username" name="username" required />
        <input id="phone" name="phone" required pattern="^\\+53[0-9]{8}$" />
        <input id="nationalId" name="nationalId" required pattern="^[0-9]{11}$" />
        <div id="recoveryFeedback" role="status"></div>
        <button id="verifyRecoveryButton" type="submit">Verificar identidad</button>
      </form>
      <form id="changePasswordForm" novalidate>
        <input id="newPassword" name="newPassword" required minlength="8" maxlength="128" />
        <input id="confirmPassword" name="confirmPassword" required minlength="8" maxlength="128" />
        <div id="changePasswordFeedback" role="status"></div>
        <button id="changePasswordButton" type="submit">Restablecer Contraseña</button>
      </form>
    `;
    verifyRecoveryIdentity.mockResolvedValue(undefined);
    recoverPassword.mockResolvedValue(undefined);
    const mockReplace = vi.fn();
    Object.defineProperty(window, "location", {
      value: { replace: mockReplace },
      writable: true,
    });

    const recoveryView = await import("../views/recoveryView.js");
    await recoveryView.initRecoveryView();

    document.getElementById("username").value = "testuser";
    document.getElementById("phone").value = "+5350000001";
    document.getElementById("nationalId").value = "12345678901";
    document.getElementById("newPassword").value = "NewPassword2!";
    document.getElementById("confirmPassword").value = "NewPassword2!";
    document
      .getElementById("changePasswordForm")
      .dispatchEvent(new Event("submit", { bubbles: true, cancelable: true }));

    await vi.waitFor(() => expect(recoverPassword).toHaveBeenCalledOnce());
    expect(recoverPassword).toHaveBeenCalledWith(
      "testuser",
      "+5350000001",
      "12345678901",
      "NewPassword2!",
    );
    expect(toastText()).toContain("Contraseña restablecida correctamente");
    vi.advanceTimersByTime(2000);
    expect(mockReplace).toHaveBeenCalledWith("./index.html");
  });

  it("no envía datos de recuperación si el formulario no es válido", async () => {
    document.body.innerHTML = `
      <form id="recoveryForm" novalidate>
        <input id="username" name="username" required />
        <input id="phone" name="phone" required pattern="^\\+53[0-9]{8}$" />
        <input id="nationalId" name="nationalId" required pattern="^[0-9]{11}$" />
        <div id="recoveryFeedback" role="status"></div>
        <button id="verifyRecoveryButton" type="submit">Verificar identidad</button>
      </form>
    `;
    const recoveryView = await import("../views/recoveryView.js");
    await recoveryView.initRecoveryView();

    document.getElementById("username").value = "";
    document
      .getElementById("recoveryForm")
      .dispatchEvent(new Event("submit", { bubbles: true, cancelable: true }));

    expect(verifyRecoveryIdentity).not.toHaveBeenCalled();
  });

  it("valida que las contraseñas coinciden antes de enviar", async () => {
    document.body.innerHTML = `
      <form id="recoveryForm" class="step-hidden" novalidate>
        <input id="username" name="username" required />
        <input id="phone" name="phone" required pattern="^\\+53[0-9]{8}$" />
        <input id="nationalId" name="nationalId" required pattern="^[0-9]{11}$" />
        <div id="recoveryFeedback" role="status"></div>
        <button id="verifyRecoveryButton" type="submit">Verificar identidad</button>
      </form>
      <form id="changePasswordForm" novalidate>
        <input id="newPassword" name="newPassword" required minlength="8" maxlength="128" />
        <input id="confirmPassword" name="confirmPassword" required minlength="8" maxlength="128" />
        <div id="changePasswordFeedback" role="status"></div>
        <button id="changePasswordButton" type="submit">Restablecer Contraseña</button>
      </form>
    `;
    verifyRecoveryIdentity.mockResolvedValue(undefined);

    const recoveryView = await import("../views/recoveryView.js");
    await recoveryView.initRecoveryView();

    document.getElementById("username").value = "testuser";
    document.getElementById("phone").value = "+5350000001";
    document.getElementById("nationalId").value = "12345678901";
    document.getElementById("newPassword").value = "NewPassword2!";
    document.getElementById("confirmPassword").value = "DifferentPassword!";
    document
      .getElementById("changePasswordForm")
      .dispatchEvent(new Event("submit", { bubbles: true, cancelable: true }));

    expect(recoverPassword).not.toHaveBeenCalled();
    expect(document.getElementById("confirmPassword").validationMessage).toBe(
      "Las contraseñas no coinciden",
    );
  });
});
