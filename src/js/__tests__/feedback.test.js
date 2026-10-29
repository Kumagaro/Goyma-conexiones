import {
  afterEach,
  beforeAll,
  beforeEach,
  describe,
  expect,
  it,
  vi,
} from "vitest";

let feedback, toast;

beforeAll(async () => {
  vi.resetModules();
  toast = await import("../components/toast.js");
  feedback = await import("../utils/feedback.js");
});

const lastToast = () => [...document.querySelectorAll(".toast")].at(-1);

describe("utils/feedback.js (spec-04 TO-01)", () => {
  beforeEach(() => {
    document.body.innerHTML = `
      <form id="f">
        <div class="field">
          <label for="ci">CI</label>
          <input id="ci" aria-describedby="ci-hint" />
          <p id="ci-hint">11 dígitos</p>
        </div>
      </form>`;
  });

  afterEach(() => {
    toast.dismissAllToasts();
    document.body.innerHTML = "";
  });

  it("notifySuccess, notifyInfo y notifyWarning muestran el toast del tipo", () => {
    feedback.notifySuccess("Cliente creado");
    expect(lastToast().classList.contains("toast-success")).toBe(true);
    expect(lastToast().textContent).toContain("Cliente creado");
    feedback.notifyInfo("Sin cambios");
    expect(lastToast().classList.contains("toast-info")).toBe(true);
    feedback.notifyWarning("No se pudieron limpiar 2 productos vencidos");
    expect(lastToast().classList.contains("toast-warning")).toBe(true);
  });

  it("notifyError usa el mensaje del backend y, si no hay, el genérico", () => {
    feedback.notifyError(
      { code: "stockInsuficiente", message: "No hay stock suficiente" },
      "No se pudo guardar la factura.",
    );
    expect(lastToast().classList.contains("toast-error")).toBe(true);
    expect(lastToast().textContent).toContain("No hay stock suficiente");
    feedback.notifyError(undefined, "No se pudo guardar la factura.");
    expect(lastToast().textContent).toContain("No se pudo guardar la factura.");
  });

  it("notifyWarning con null o vacío no muestra nada (contrato de expiredCleanupWarning)", () => {
    feedback.notifyWarning(null);
    feedback.notifyWarning("");
    expect(document.querySelectorAll(".toast")).toHaveLength(0);
  });

  it("setFieldError pone el texto bajo el campo, aria-invalid y aria-describedby; escribir lo limpia", () => {
    const input = document.getElementById("ci");
    feedback.setFieldError(input, "El CI ya está registrado");
    const error = document.getElementById("ci-error");
    expect(error.textContent).toBe("El CI ya está registrado");
    expect(error.className).toBe("field-error");
    expect(input.getAttribute("aria-invalid")).toBe("true");
    expect(input.getAttribute("aria-describedby")).toBe("ci-hint ci-error");

    // Un segundo error reutiliza el mismo elemento.
    feedback.setFieldError(input, "Formato inválido");
    expect(document.querySelectorAll("#ci-error")).toHaveLength(1);

    input.value = "1";
    input.dispatchEvent(new Event("input", { bubbles: true }));
    expect(document.getElementById("ci-error")).toBeNull();
    expect(input.hasAttribute("aria-invalid")).toBe(false);
    expect(input.getAttribute("aria-describedby")).toBe("ci-hint");
  });

  it("clearFieldErrors limpia todos los errores de un formulario", () => {
    feedback.setFieldError(document.getElementById("ci"), "x");
    feedback.clearFieldErrors(document.getElementById("f"));
    expect(document.querySelector(".field-error")).toBeNull();
  });

  it("el texto de error se pinta como texto, no como HTML", () => {
    feedback.setFieldError(
      document.getElementById("ci"),
      '<img src=x onerror="alert(1)">',
    );
    expect(document.querySelector(".field-error img")).toBeNull();
  });
});

describe("reportError y requireFields", () => {
  beforeEach(() => {
    document.body.innerHTML = `
      <input id="nombre" />
      <input id="ci" value="123" />`;
  });

  afterEach(() => {
    toast.dismissAllToasts();
    document.body.innerHTML = "";
  });

  it("un rechazo de validación que nombra un campo va al toast y junto al campo", () => {
    feedback.reportError(
      {
        code: "duplicateField",
        message: "Ya existe un cliente con ese carnet de identidad",
      },
      "No se pudo guardar el cliente.",
      [
        [/nombre/i, "nombre"],
        [/carnet/i, "ci"],
      ],
    );
    expect(lastToast().textContent).toContain("carnet de identidad");
    expect(document.getElementById("ci-error").textContent).toBe(
      "Ya existe un cliente con ese carnet de identidad",
    );
    expect(document.getElementById("nombre-error")).toBeNull();
  });

  it("un error interno solo va al toast", () => {
    feedback.reportError(
      { code: "internal", message: "Ocurrió un error inesperado" },
      "No se pudo guardar.",
      [[/error/i, "nombre"]],
    );
    expect(lastToast().textContent).toContain("Ocurrió un error inesperado");
    expect(document.querySelector(".field-error")).toBeNull();
  });

  it("requireFields marca los vacíos, enfoca el primero y dice si falta alguno", () => {
    expect(feedback.requireFields(["nombre", "ci"])).toBe(false);
    expect(document.getElementById("nombre-error").textContent).toBe(
      "Completa este campo",
    );
    expect(document.getElementById("ci-error")).toBeNull();
    expect(document.activeElement.id).toBe("nombre");
    document.getElementById("nombre").value = "Ana";
    expect(feedback.requireFields(["nombre", "ci"])).toBe(true);
  });
});
