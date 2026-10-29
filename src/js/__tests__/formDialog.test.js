import { afterEach, beforeEach, describe, expect, it, vi } from "vitest";
import { openFormDialog } from "../components/formDialog.js";
import { toastsOfType, toastText } from "./helpers/invoiceUi.js";

function openDialog(onSubmit) {
  return openFormDialog({
    title: "Nuevo cliente",
    fields: [
      { name: "firstName", label: "Nombres", required: true },
      { name: "company", label: "Empresa" },
    ],
    submitText: "Crear",
    missingMessage: "Completa los campos obligatorios.",
    onSubmit,
  });
}

const submit = () =>
  document
    .querySelector(".modal-card form")
    .dispatchEvent(new Event("submit", { cancelable: true }));

describe("formDialog (spec-04 TO-01)", () => {
  beforeEach(() => {
    document.body.innerHTML = "";
  });

  // Escape cierra el diálogo y quita su listener de document (isolate: false).
  afterEach(() => {
    document.dispatchEvent(new KeyboardEvent("keydown", { key: "Escape" }));
  });

  it("marca el obligatorio vacío junto al campo y resume el error en un toast", async () => {
    const onSubmit = vi.fn();
    void openDialog(onSubmit);
    submit();
    await Promise.resolve();
    const input = document.querySelector("[name='firstName']");
    expect(onSubmit).not.toHaveBeenCalled();
    expect(input.getAttribute("aria-invalid")).toBe("true");
    expect(document.getElementById(`${input.id}-error`).textContent).toBe(
      "Completa este campo",
    );
    expect(toastsOfType("error")).toHaveLength(1);
    expect(toastText()).toContain("Completa los campos obligatorios.");
    expect(document.querySelector(".modal-card .form-feedback")).toBeNull();
  });

  it("un rechazo del backend sale como toast y el diálogo sigue abierto", async () => {
    const onSubmit = vi.fn(() =>
      Promise.reject({
        code: "duplicateField",
        message: "El CI ya está registrado",
      }),
    );
    void openDialog(onSubmit);
    document.querySelector("[name='firstName']").value = "Rosa";
    submit();
    await vi.waitFor(() =>
      expect(toastText()).toContain("El CI ya está registrado"),
    );
    expect(document.querySelector(".modal-card")).not.toBeNull();
  });
});
