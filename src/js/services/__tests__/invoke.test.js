import { afterEach, describe, expect, it, vi } from "vitest";

import { invoke, setSessionExpiredRedirect } from "../invoke.js";

describe("invoke Tauri", () => {
  afterEach(() => {
    delete window.__TAURI__;
    setSessionExpiredRedirect(null);
    vi.restoreAllMocks();
  });

  it("invoca comandos mediante la API global de Tauri v2", async () => {
    const tauriInvoke = vi.fn().mockResolvedValue({ username: "admin" });
    window.__TAURI__ = { core: { invoke: tauriInvoke } };

    await expect(invoke("get_session_info")).resolves.toEqual({
      username: "admin",
    });
    expect(tauriInvoke).toHaveBeenCalledWith("get_session_info", undefined);
  });

  it("redirige si sessionExpired llega como respuesta y retorna el resultado intacto", async () => {
    const response = {
      code: "sessionExpired",
      message: "Tu sesión ya no es válida",
    };
    const tauriInvoke = vi.fn().mockResolvedValue(response);
    const redirect = vi.fn();
    window.__TAURI__ = { core: { invoke: tauriInvoke } };
    setSessionExpiredRedirect(redirect);

    await expect(invoke("get_session_info")).resolves.toEqual(response);
    expect(redirect).toHaveBeenCalledWith("./index.html?reason=expired");
  });

  it("redirige ante sessionExpired y preserva otros errores", async () => {
    const redirect = vi.fn();
    const tauriInvoke = vi
      .fn()
      .mockRejectedValueOnce({ code: "sessionExpired", message: "expirada" })
      .mockRejectedValueOnce({ code: "internal", message: "interno" });
    window.__TAURI__ = { core: { invoke: tauriInvoke } };
    setSessionExpiredRedirect(redirect);

    await expect(invoke("get_session_info")).rejects.toMatchObject({
      code: "sessionExpired",
    });
    await expect(invoke("get_session_info")).rejects.toMatchObject({
      code: "internal",
    });
    expect(redirect).toHaveBeenCalledTimes(1);
  });

  it("traduce el rechazo de argumentos de Tauri y registra comando y causa", async () => {
    const tauriError = "invalid args `input` for command `create_product`";
    const tauriInvoke = vi.fn().mockRejectedValue(tauriError);
    const consoleError = vi
      .spyOn(console, "error")
      .mockImplementation(() => {});
    window.__TAURI__ = { core: { invoke: tauriInvoke } };

    await expect(invoke("create_product")).rejects.toMatchObject({
      message: "Los datos enviados no son válidos",
    });
    expect(consoleError).toHaveBeenCalledWith(
      expect.stringContaining("create_product"),
      expect.stringContaining(tauriError),
    );
  });

  it("convierte cualquier otro error sin forma AppError al mensaje genérico", async () => {
    const tauriInvoke = vi.fn().mockRejectedValue(new Error("IPC unavailable"));
    const consoleError = vi
      .spyOn(console, "error")
      .mockImplementation(() => {});
    window.__TAURI__ = { core: { invoke: tauriInvoke } };

    await expect(invoke("list_products")).rejects.toMatchObject({
      message: "Ocurrió un error inesperado",
    });
    expect(consoleError).toHaveBeenCalledWith(
      expect.stringContaining("list_products"),
      expect.stringContaining("IPC unavailable"),
    );
  });

  it("preserva intacto un AppError y no lo registra como error de Tauri", async () => {
    const appError = {
      code: "validationFailed",
      message: "La fecha de caducidad no es válida",
    };
    const tauriInvoke = vi.fn().mockRejectedValue(appError);
    const consoleError = vi
      .spyOn(console, "error")
      .mockImplementation(() => {});
    window.__TAURI__ = { core: { invoke: tauriInvoke } };

    await expect(invoke("create_product")).rejects.toBe(appError);
    expect(consoleError).not.toHaveBeenCalled();
  });
});
