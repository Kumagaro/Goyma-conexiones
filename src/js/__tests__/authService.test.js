import { afterEach, describe, expect, it, vi } from "vitest";

describe("servicio de autenticación", () => {
  afterEach(() => {
    delete window.__TAURI__;
    vi.useRealTimers();
    vi.resetModules();
  });

  it("llama los comandos snake_case y limita touchSession a una llamada cada 30 segundos", async () => {
    vi.useFakeTimers();
    vi.setSystemTime(new Date("2026-09-26T12:00:00Z"));
    const tauriInvoke = vi.fn().mockResolvedValue({ status: "active" });
    window.__TAURI__ = { core: { invoke: tauriInvoke } };
    const {
      enableTouchSession,
      getCurrentSession,
      getSessionInfo,
      startActivityListener,
      touchSession,
    } = await import("../services/authService.js");

    await getSessionInfo();
    expect(tauriInvoke).toHaveBeenLastCalledWith("get_session_info", undefined);
    await getCurrentSession();
    expect(tauriInvoke).toHaveBeenLastCalledWith(
      "get_current_session",
      undefined,
    );
    await touchSession();
    expect(tauriInvoke).toHaveBeenLastCalledWith("touch_session", undefined);

    startActivityListener();
    enableTouchSession();
    document.dispatchEvent(new MouseEvent("mousemove"));
    document.dispatchEvent(new MouseEvent("pointermove"));
    expect(tauriInvoke).toHaveBeenCalledTimes(4);

    await vi.advanceTimersByTimeAsync(30_000);
    window.dispatchEvent(new PopStateEvent("popstate"));
    expect(tauriInvoke).toHaveBeenCalledTimes(5);
    expect(tauriInvoke).toHaveBeenLastCalledWith("touch_session", undefined);
  });

  it("invoca login con credenciales y la opción de persistencia", async () => {
    const sessionInfo = {
      username: "admin",
      role: "administrador",
      rememberSession: true,
    };
    const tauriInvoke = vi.fn().mockResolvedValue(sessionInfo);
    window.__TAURI__ = { core: { invoke: tauriInvoke } };
    const { login } = await import("../services/authService.js");

    await expect(login("Admin", "Admin123!", true)).resolves.toEqual(
      sessionInfo,
    );
    expect(tauriInvoke).toHaveBeenCalledWith("login", {
      username: "Admin",
      password: "Admin123!",
      rememberSession: true,
    });
  });

  it("invoca el comando logout sin argumentos", async () => {
    const tauriInvoke = vi.fn().mockResolvedValue(undefined);
    window.__TAURI__ = { core: { invoke: tauriInvoke } };
    const { logout } = await import("../services/authService.js");

    await expect(logout()).resolves.toBeUndefined();
    expect(tauriInvoke).toHaveBeenCalledWith("logout", undefined);
  });
});
