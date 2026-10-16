import { afterEach, describe, expect, it, vi } from "vitest";

describe("sesión del dashboard", () => {
  afterEach(() => {
    // Con isolate:false los temporizadores falsos pasarían a otros archivos.
    vi.useRealTimers();
    delete window.__TAURI__;
    vi.resetModules();
  });

  it("memoiza loadSession, expone la sesión y notifica refreshSession", async () => {
    const initial = {
      username: "admin",
      role: "administrador",
      rememberSession: true,
    };
    const updated = { ...initial, username: "admin2" };
    const tauriInvoke = vi
      .fn()
      .mockResolvedValueOnce(initial)
      .mockResolvedValueOnce(updated);
    window.__TAURI__ = { core: { invoke: tauriInvoke } };
    const session = await import("../session.js");
    const subscriber = vi.fn();
    session.onSessionChange(subscriber);

    await Promise.all([session.loadSession(), session.loadSession()]);
    expect(tauriInvoke).toHaveBeenCalledTimes(1);
    expect(session.getSession()).toEqual(initial);

    await session.refreshSession();
    expect(session.getSession()).toEqual(updated);
    expect(subscriber).toHaveBeenCalledWith(updated);
    expect(tauriInvoke).toHaveBeenCalledTimes(2);
  });

  it("trata una respuesta sessionExpired como ausencia de sesión", async () => {
    const redirect = vi.fn();
    const tauriInvoke = vi
      .fn()
      .mockResolvedValue({ code: "sessionExpired", message: "expirada" });
    window.__TAURI__ = { core: { invoke: tauriInvoke } };
    const { setSessionExpiredRedirect } = await import("../services/invoke.js");
    setSessionExpiredRedirect(redirect);
    const session = await import("../session.js");

    await expect(session.loadSession()).resolves.toBeNull();
    expect(redirect).toHaveBeenCalledWith("./index.html?reason=expired");
  });

  it("redirige al login cuando no hay sesión", async () => {
    const redirect = vi.fn();
    window.__TAURI__ = { core: { invoke: vi.fn().mockResolvedValue(null) } };
    const session = await import("../session.js");
    session.setRedirectToLogin(redirect);

    await expect(session.loadSession()).resolves.toBeNull();
    expect(redirect).toHaveBeenCalledWith("./index.html");
  });

  it("activa el toque de sesión tras cargarla y redirige si ya expiró", async () => {
    vi.useFakeTimers();
    const redirect = vi.fn();
    const tauriInvoke = vi.fn((command) => {
      if (command === "get_session_info") {
        return Promise.resolve({
          username: "admin",
          role: "administrador",
          rememberSession: false,
        });
      }
      if (command === "touch_session") {
        return Promise.resolve({ status: "expired" });
      }
      return Promise.resolve(null);
    });
    window.__TAURI__ = { core: { invoke: tauriInvoke } };
    const { setSessionExpiredRedirect } = await import("../services/invoke.js");
    setSessionExpiredRedirect(redirect);
    const session = await import("../session.js");

    await session.loadSession();
    document.dispatchEvent(new MouseEvent("click"));

    await vi.waitFor(() => {
      expect(redirect).toHaveBeenCalledWith("./index.html?reason=expired");
    });
    expect(tauriInvoke).toHaveBeenCalledWith("touch_session", undefined);
  });
});
