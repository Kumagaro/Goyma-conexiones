import { afterEach, describe, expect, it, vi } from "vitest";
import { toastText } from "./helpers/invoiceUi.js";

describe("vista de perfil", () => {
  afterEach(() => {
    document.body.innerHTML = "";
    delete window.__TAURI__;
    delete window.showPage;
    vi.resetModules();
  });

  it.each([true, false])(
    "renderiza los datos propios y la advertencia reservada (%s)",
    async (hasReservedContactData) => {
      document.body.innerHTML = `
      <div class="usuario" role="button" tabindex="0"></div>
      <nav id="sidebar"><a data-page="perfil"></a></nav>
      <div id="reserved-data-warning" class="feedback-inline"></div>
      <form id="profile-form">
        <input id="perfil-username" />
        <input id="perfil-role" />
        <input id="perfil-telefono" />
        <input id="perfil-national-id" />
      </form>
      <div id="modal-ver-perfil">
        <span id="verperfil-username"></span>
        <span id="verperfil-rol"></span>
        <span id="verperfil-rol-badge"></span>
        <span id="verperfil-telefono"></span>
        <span id="verperfil-national-id"></span>
        <button data-profile-action="close"></button>
        <button data-profile-action="edit"></button>
      </div>
    `;
      const profile = {
        username: "admin",
        role: "administrador",
        phone: "+5300000000",
        nationalId: "00000000000",
        hasReservedContactData,
      };
      const invoke = vi.fn((command) => {
        if (command === "get_session_info") {
          return Promise.resolve({
            username: "admin",
            role: "administrador",
            rememberSession: true,
          });
        }
        if (command === "get_user_profile") return Promise.resolve(profile);
        throw new Error(`Unexpected command: ${command}`);
      });
      window.__TAURI__ = {
        core: { invoke },
      };
      window.showPage = vi.fn();

      const profileView = await import("../views/perfil.js");
      await profileView.initProfileView();

      expect(invoke).toHaveBeenCalledWith("get_user_profile", undefined);
      expect(document.getElementById("perfil-username").value).toBe("admin");
      expect(document.getElementById("perfil-role").value).toBe(
        "Administrador",
      );
      expect(document.getElementById("perfil-telefono").value).toBe(
        "+5300000000",
      );
      expect(document.getElementById("perfil-national-id").value).toBe(
        "00000000000",
      );
      expect(
        document
          .getElementById("reserved-data-warning")
          .classList.contains("is-visible"),
      ).toBe(hasReservedContactData);

      document.querySelector(".usuario").click();

      expect(
        document
          .getElementById("modal-ver-perfil")
          .classList.contains("active"),
      ).toBe(true);
      expect(document.getElementById("verperfil-username").textContent).toBe(
        "admin",
      );
      expect(document.getElementById("verperfil-rol").textContent).toBe(
        "Administrador",
      );
      expect(document.getElementById("verperfil-telefono").textContent).toBe(
        "+5300000000",
      );
      expect(document.getElementById("verperfil-national-id").textContent).toBe(
        "00000000000",
      );

      document.querySelector("[data-profile-action='edit']").click();

      expect(
        document
          .getElementById("modal-ver-perfil")
          .classList.contains("active"),
      ).toBe(false);
      expect(window.showPage).toHaveBeenCalledWith(
        "perfil",
        document.querySelector('#sidebar a[data-page="perfil"]'),
      );
    },
  );

  it("guarda el perfil sin enviar rol y refresca la sesión para el saludo", async () => {
    document.body.innerHTML = `
      <div id="page-perfil">
        <p id="perfil-feedback" class="feedback-inline"></p>
        <div id="reserved-data-warning" class="feedback-inline is-visible"></div>
        <form id="profile-form">
          <input id="perfil-username" name="username" data-validation-field="username" minlength="3" maxlength="50" pattern="[A-Za-z0-9]{3,50}" required value="admin" />
          <input id="perfil-role" readonly value="Administrador" />
          <input id="perfil-telefono" name="phone" data-validation-field="phone" pattern="\\+53[0-9]{8}" required value="+5300000000" />
          <input id="perfil-national-id" name="nationalId" data-validation-field="nationalId" pattern="[0-9]{11}" required value="00000000000" />
          <button type="submit">Guardar</button>
        </form>
      </div>
      <div id="modal-ver-perfil">
        <span id="verperfil-username"></span>
        <span id="verperfil-rol"></span>
        <span id="verperfil-rol-badge"></span>
        <span id="verperfil-telefono"></span>
        <span id="verperfil-national-id"></span>
      </div>
      <div id="dashboard-greeting"></div>
    `;
    let sessionRefreshCount = 0;
    const currentProfile = {
      username: "admin",
      role: "administrador",
      phone: "+5300000000",
      nationalId: "00000000000",
      hasReservedContactData: true,
    };
    const updatedProfile = {
      ...currentProfile,
      username: "newadmin",
      phone: "+5350000001",
      nationalId: "12345678901",
      hasReservedContactData: false,
    };
    const invoke = vi.fn((command) => {
      if (command === "get_session_info") {
        sessionRefreshCount += 1;
        return Promise.resolve({
          username: sessionRefreshCount === 1 ? "admin" : "newadmin",
          role: "administrador",
          rememberSession: true,
        });
      }
      if (command === "get_user_profile")
        return Promise.resolve(currentProfile);
      if (command === "update_own_profile")
        return Promise.resolve(updatedProfile);
      throw new Error(`Unexpected command: ${command}`);
    });
    window.__TAURI__ = { core: { invoke } };

    const profileView = await import("../views/perfil.js");
    await profileView.initProfileView();
    const { onSessionChange } = await import("../session.js");
    const greeting = document.getElementById("dashboard-greeting");
    onSessionChange(({ username }) => {
      greeting.textContent = `Bienvenido de nuevo, ${username}.`;
    });

    document.getElementById("perfil-username").value = "NewAdmin";
    document.getElementById("perfil-telefono").value = "+5350000001";
    document.getElementById("perfil-national-id").value = "12345678901";
    document
      .getElementById("profile-form")
      .dispatchEvent(new Event("submit", { bubbles: true, cancelable: true }));

    await vi.waitFor(() =>
      expect(invoke).toHaveBeenCalledWith("update_own_profile", {
        username: "NewAdmin",
        phone: "+5350000001",
        nationalId: "12345678901",
      }),
    );
    await vi.waitFor(() =>
      expect(greeting.textContent).toBe("Bienvenido de nuevo, newadmin."),
    );
    expect(sessionRefreshCount).toBe(2);
    expect(document.getElementById("perfil-username").value).toBe("newadmin");
    expect(document.getElementById("perfil-telefono").value).toBe(
      "+5350000001",
    );
    expect(document.getElementById("perfil-national-id").value).toBe(
      "12345678901",
    );
    expect(
      document
        .getElementById("reserved-data-warning")
        .classList.contains("is-visible"),
    ).toBe(false);
  });

  it("no llama al backend si el username no pasa la validación HTML5", async () => {
    document.body.innerHTML = `
      <div id="page-perfil">
        <p id="perfil-feedback" class="feedback-inline"></p>
        <form id="profile-form">
          <input id="perfil-username" name="username" data-validation-field="username" minlength="3" maxlength="50" pattern="[A-Za-z0-9]{3,50}" required value="admin" />
          <input id="perfil-role" readonly value="Administrador" />
          <input id="perfil-telefono" name="phone" data-validation-field="phone" pattern="\\+53[0-9]{8}" required value="+5350000001" />
          <input id="perfil-national-id" name="nationalId" data-validation-field="nationalId" pattern="[0-9]{11}" required value="12345678901" />
          <button type="submit">Guardar</button>
        </form>
      </div>
    `;
    const invoke = vi.fn((command) => {
      if (command === "get_session_info") {
        return Promise.resolve({
          username: "admin",
          role: "administrador",
          rememberSession: true,
        });
      }
      if (command === "get_user_profile") {
        return Promise.resolve({
          username: "admin",
          role: "administrador",
          phone: "+5350000001",
          nationalId: "12345678901",
          hasReservedContactData: false,
        });
      }
      return Promise.reject(new Error(`Unexpected command: ${command}`));
    });
    window.__TAURI__ = { core: { invoke } };

    const profileView = await import("../views/perfil.js");
    await profileView.initProfileView();
    document.getElementById("perfil-username").value = "ab";
    document
      .getElementById("profile-form")
      .dispatchEvent(new Event("submit", { bubbles: true, cancelable: true }));

    expect(invoke).not.toHaveBeenCalledWith(
      "update_own_profile",
      expect.anything(),
    );
    expect(document.getElementById("perfil-username").validationMessage).toBe(
      "El nombre de usuario debe tener de 3 a 50 caracteres alfanuméricos (letras y números, sin acentos ni espacios)",
    );
  });

  it("cambia contraseña y compara la confirmación solo en la interfaz", async () => {
    document.body.innerHTML = `
      <div id="page-perfil">
        <p id="perfil-feedback" class="feedback-inline"></p>
        <form id="profile-form"></form>
        <form id="profile-password-form">
          <input id="perfil-pass-actual" name="currentPassword" required />
          <input id="perfil-pass-nueva" name="newPassword" data-validation-field="password" minlength="8" maxlength="72" required />
          <input id="perfil-pass-confirmar" name="confirmPassword" data-validation-field="password" minlength="8" maxlength="72" required />
          <button type="submit">Cambiar</button>
        </form>
      </div>
    `;
    const invoke = vi.fn((command) => {
      if (command === "get_session_info") {
        return Promise.resolve({
          username: "admin",
          role: "administrador",
          rememberSession: true,
        });
      }
      if (command === "get_user_profile") {
        return Promise.resolve({
          username: "admin",
          role: "administrador",
          phone: "+5350000001",
          nationalId: "12345678901",
          hasReservedContactData: false,
        });
      }
      if (command === "change_own_password") return Promise.resolve(null);
      throw new Error(`Unexpected command: ${command}`);
    });
    window.__TAURI__ = { core: { invoke } };

    const profileView = await import("../views/perfil.js");
    await profileView.initProfileView();
    const passwordForm = document.getElementById("profile-password-form");
    passwordForm.elements.namedItem("currentPassword").value = "CurrentPass1!";
    passwordForm.elements.namedItem("newPassword").value = "NewPassword2!";
    passwordForm.elements.namedItem("confirmPassword").value = "NewPassword2!";
    passwordForm.dispatchEvent(
      new Event("submit", { bubbles: true, cancelable: true }),
    );

    await vi.waitFor(() =>
      expect(invoke).toHaveBeenCalledWith("change_own_password", {
        currentPassword: "CurrentPass1!",
        newPassword: "NewPassword2!",
      }),
    );
    await vi.waitFor(() =>
      expect(passwordForm.elements.namedItem("currentPassword").value).toBe(""),
    );
    expect(invoke).not.toHaveBeenCalledWith(
      "change_own_password",
      expect.objectContaining({ confirmPassword: expect.anything() }),
    );
    expect(passwordForm.elements.namedItem("newPassword").value).toBe("");
    expect(passwordForm.elements.namedItem("confirmPassword").value).toBe("");
    expect(toastText()).toContain("La contraseña se cambió correctamente.");
  });

  it("no cambia contraseña si la confirmación no coincide", async () => {
    document.body.innerHTML = `
      <div id="page-perfil">
        <p id="perfil-feedback" class="feedback-inline"></p>
        <form id="profile-form"></form>
        <form id="profile-password-form">
          <input id="perfil-pass-actual" name="currentPassword" required value="CurrentPass1!" />
          <input id="perfil-pass-nueva" name="newPassword" data-validation-field="password" minlength="8" maxlength="72" required value="NewPassword2!" />
          <input id="perfil-pass-confirmar" name="confirmPassword" data-validation-field="password" minlength="8" maxlength="72" required value="DifferentPass3!" />
        </form>
      </div>
    `;
    const invoke = vi.fn((command) => {
      if (command === "get_session_info") {
        return Promise.resolve({
          username: "admin",
          role: "administrador",
          rememberSession: true,
        });
      }
      if (command === "get_user_profile") {
        return Promise.resolve({
          username: "admin",
          role: "administrador",
          phone: "+5350000001",
          nationalId: "12345678901",
          hasReservedContactData: false,
        });
      }
      return Promise.reject(new Error(`Unexpected command: ${command}`));
    });
    window.__TAURI__ = { core: { invoke } };

    const profileView = await import("../views/perfil.js");
    await profileView.initProfileView();
    const confirmation = document.getElementById("perfil-pass-confirmar");
    document
      .getElementById("profile-password-form")
      .dispatchEvent(new Event("submit", { bubbles: true, cancelable: true }));

    expect(invoke).not.toHaveBeenCalledWith(
      "change_own_password",
      expect.anything(),
    );
    expect(confirmation.validationMessage).toBe("Las contraseñas no coinciden");
  });
});
