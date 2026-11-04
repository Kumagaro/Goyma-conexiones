import { afterEach, describe, expect, it, vi } from "vitest";

import { invoke, setSessionExpiredRedirect } from "../invoke.js";

describe("invoke Tauri", () => {
  afterEach(() => {
    delete window.__TAURI__;
    setSessionExpiredRedirect(null);
    vi.restoreAllMocks();
  });

  it("TO-02: emite command:success solo tras un comando de escritura correcto", async () => {
    const seen = [];
    const listener = (event) => seen.push(event.detail.command);
    window.addEventListener("command:success", listener);
    const tauriInvoke = vi
      .fn()
      .mockResolvedValueOnce([])
      .mockResolvedValueOnce(3)
      .mockResolvedValueOnce({ status: "active" })
      .mockResolvedValueOnce({ invoiceId: 1 })
      .mockRejectedValueOnce({ code: "validationFailed", message: "No" });
    window.__TAURI__ = { core: { invoke: tauriInvoke } };

    await invoke("list_notifications");
    await invoke("count_unread_notifications");
    await invoke("touch_session");
    await invoke("create_sale_invoice", { input: {} });
    await expect(invoke("delete_notification")).rejects.toMatchObject({
      code: "validationFailed",
    });
    window.removeEventListener("command:success", listener);

    expect(seen).toEqual(["create_sale_invoice"]);
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
