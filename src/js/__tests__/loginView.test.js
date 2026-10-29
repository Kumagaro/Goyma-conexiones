import { afterEach, describe, expect, it, vi } from "vitest";
import { toastText } from "./helpers/invoiceUi.js";

function renderLoginForm() {
  document.body.innerHTML = `
    <form id="loginForm" novalidate>
      <input id="username" minlength="3" maxlength="50" required />
      <input id="password" minlength="8" maxlength="128" required />
      <input id="rememberSession" type="checkbox" />
      <button id="loginButton" type="submit">Iniciar Sesión</button>
      <div id="loginFeedback" role="status"></div>
    </form>
  `;
}

describe("vista de inicio de sesión", () => {
  afterEach(() => {
    document.body.innerHTML = "";
    delete window.__TAURI__;
    window.history.replaceState({}, "", "/");
    vi.resetModules();
  });

  it("muestra en español el error de username y no invoca Tauri", async () => {
    renderLoginForm();
    const tauriInvoke = vi.fn();
    window.__TAURI__ = { core: { invoke: tauriInvoke } };
    const loginView = await import("../views/loginView.js");
    await loginView.initLoginView();
    const username = document.getElementById("username");
    expect(toastText()).toBe("");
    username.value = "ab";
    document.getElementById("password").value = "ValidPass1!";
    username.dispatchEvent(new Event("invalid", { cancelable: true }));

    document
      .getElementById("loginForm")
      .dispatchEvent(new Event("submit", { bubbles: true, cancelable: true }));

    expect(username.validationMessage).toBe(
      "El nombre de usuario debe tener de 3 a 50 caracteres alfanuméricos (letras y números, sin acentos ni espacios)",
    );
    expect(tauriInvoke).toHaveBeenCalledTimes(1);
    expect(tauriInvoke).toHaveBeenCalledWith("get_current_session", undefined);
  });

  it("muestra el mensaje de credenciales incorrectas que devuelve Rust", async () => {
    renderLoginForm();
    window.__TAURI__ = {
      core: {
        invoke: vi.fn().mockRejectedValue({
          code: "invalidCredentials",
          message: "El nombre de usuario o la contraseña son incorrectos",
        }),
      },
    };
    const loginView = await import("../views/loginView.js");
    await loginView.initLoginView();
    document.getElementById("username").value = "admin";
    document.getElementById("password").value = "incorrecta";

    document
      .getElementById("loginForm")
      .dispatchEvent(new Event("submit", { bubbles: true, cancelable: true }));

    await vi.waitFor(() => {
      expect(toastText()).toContain(
        "El nombre de usuario o la contraseña son incorrectos",
      );
    });
  });

  it("envía Mantener sesión activa y redirige al dashboard al autenticar", async () => {
    renderLoginForm();
    const tauriInvoke = vi.fn((command) => {
      if (command === "get_current_session") return Promise.resolve(null);
      return Promise.resolve({
        username: "admin",
        role: "administrador",
        rememberSession: true,
      });
    });
    window.__TAURI__ = { core: { invoke: tauriInvoke } };
    const loginView = await import("../views/loginView.js");
    const redirect = vi.fn();
    loginView.setDashboardRedirect(redirect);
    await loginView.initLoginView();
    expect(document.getElementById("loginForm").hidden).toBe(false);
    document.getElementById("username").value = "admin";
    document.getElementById("password").value = "Admin123!";
    document.getElementById("rememberSession").checked = true;

    document
      .getElementById("loginForm")
      .dispatchEvent(new Event("submit", { bubbles: true, cancelable: true }));

    await vi.waitFor(() =>
      expect(redirect).toHaveBeenCalledWith("./dashboard-unificado.html"),
    );
    expect(tauriInvoke).toHaveBeenCalledWith("login", {
      username: "admin",
      password: "Admin123!",
      rememberSession: true,
    });
  });

  it("redirige al dashboard antes de mostrar el login si hay sesión persistida", async () => {
    renderLoginForm();
    const session = {
      username: "admin",
      role: "administrador",
      rememberSession: true,
    };
    let resolveCurrentSession;
    const tauriInvoke = vi.fn(
      () =>
        new Promise((resolve) => {
          resolveCurrentSession = resolve;
        }),
    );
    window.__TAURI__ = { core: { invoke: tauriInvoke } };
    const loginView = await import("../views/loginView.js");
    const redirect = vi.fn();
    loginView.setDashboardRedirect(redirect);

    const initialization = loginView.initLoginView();
    resolveCurrentSession(session);
    await initialization;

    expect(tauriInvoke).toHaveBeenCalledWith("get_current_session", undefined);
    expect(redirect).toHaveBeenCalledWith("./dashboard-unificado.html");
    expect(document.getElementById("loginForm").hidden).toBe(true);
  });

  it("muestra el aviso de sesión expirada al regresar al login", async () => {
    window.history.replaceState({}, "", "/?reason=expired");
    renderLoginForm();
    window.__TAURI__ = {
      core: { invoke: vi.fn().mockResolvedValue(null) },
    };
    const loginView = await import("../views/loginView.js");

    await loginView.initLoginView();

    expect(toastText()).toContain(
      "Tu sesión ya no es válida. Inicia sesión de nuevo",
    );
    expect(document.getElementById("loginForm").hidden).toBe(false);
  });
});
