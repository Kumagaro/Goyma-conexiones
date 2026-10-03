import { afterEach, describe, expect, it } from "vitest";

import { confirmDialog } from "../components/confirmDialog.js";

describe("confirmDialog", () => {
  afterEach(() => {
    document.body.replaceChildren();
  });

  it("renderiza texto multilínea de forma segura y resuelve true al confirmar", async () => {
    const opener = document.createElement("button");
    opener.textContent = "Eliminar";
    document.body.append(opener);
    opener.focus();

    const result = confirmDialog({
      title: "Confirmar eliminación",
      message:
        "Quedarán 2 productos sin categoría.\nEsta acción no se puede deshacer.",
    });
    const overlay = document.querySelector(".modal-overlay");
    const dialog = overlay.querySelector('[role="dialog"]');

    expect(overlay).not.toBeNull();
    expect(overlay.querySelector(".modal-card")).not.toBeNull();
    expect(dialog.getAttribute("aria-modal")).toBe("true");
    expect(dialog.textContent).toContain("Quedarán 2 productos sin categoría.");
    expect(dialog.textContent).toContain("Esta acción no se puede deshacer.");
    expect(dialog.querySelector("img")).toBeNull();

    dialog.querySelector('[data-action="confirm"]').click();

    await expect(result).resolves.toBe(true);
    expect(document.querySelector(".modal-overlay")).toBeNull();
    expect(document.activeElement).toBe(opener);
  });

  it("resuelve false al cancelar", async () => {
    const result = confirmDialog({ message: "¿Continuar?" });
    document
      .querySelector('[data-action="cancel"]')
      .dispatchEvent(new MouseEvent("click", { bubbles: true }));

    await expect(result).resolves.toBe(false);
    expect(document.querySelector(".modal-overlay")).toBeNull();
  });

  it("inicia el foco en Cancelar, atrapa Tab y Shift+Tab, y Escape devuelve el foco", async () => {
    const opener = document.createElement("button");
    document.body.append(opener);
    opener.focus();

    const result = confirmDialog({ message: "¿Continuar?" });
    const dialog = document.querySelector('[role="dialog"]');
    const cancel = dialog.querySelector('[data-action="cancel"]');
    const confirm = dialog.querySelector('[data-action="confirm"]');
    const pressTab = (shiftKey = false) =>
      dialog.dispatchEvent(
        new KeyboardEvent("keydown", {
          key: "Tab",
          shiftKey,
          bubbles: true,
          cancelable: true,
        }),
      );

    expect(document.activeElement).toBe(cancel);
    pressTab();
    expect(document.activeElement).toBe(confirm);
    pressTab();
    expect(document.activeElement).toBe(cancel);
    pressTab(true);
    expect(document.activeElement).toBe(confirm);

    dialog.dispatchEvent(
      new KeyboardEvent("keydown", {
        key: "Escape",
        bubbles: true,
        cancelable: true,
      }),
    );

    await expect(result).resolves.toBe(false);
    expect(document.querySelector(".modal-overlay")).toBeNull();
    expect(document.activeElement).toBe(opener);
  });
});
