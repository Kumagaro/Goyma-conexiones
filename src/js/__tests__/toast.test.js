import {
  afterEach,
  beforeAll,
  beforeEach,
  describe,
  expect,
  it,
  vi,
} from "vitest";

let showToast, dismissAllToasts;

beforeAll(async () => {
  vi.resetModules();
  ({ showToast, dismissAllToasts } = await import("../components/toast.js"));
});

const region = () => document.getElementById("toast-region");
const toasts = () => [...document.querySelectorAll("#toast-region .toast")];

describe("toasts (spec-04 TO-01)", () => {
  beforeEach(() => {
    vi.useFakeTimers();
    document.body.innerHTML = "<main></main>";
  });

  afterEach(() => {
    dismissAllToasts();
    vi.useRealTimers();
    document.body.innerHTML = "";
  });

  it("crea la región una sola vez y apila los toasts", () => {
    showToast({ type: "success", message: "Cliente creado" });
    showToast({ type: "info", message: "Otro" });
    expect(document.querySelectorAll("#toast-region")).toHaveLength(1);
    expect(toasts()).toHaveLength(2);
    expect(region().getAttribute("aria-live")).toBeNull();
  });

  it.each([
    ["success", "status", "toast-success"],
    ["info", "status", "toast-info"],
    ["warning", "status", "toast-warning"],
    ["error", "alert", "toast-error"],
    ["critical", "alert", "toast-critical"],
  ])("%s usa role=%s y su clase", (type, role, className) => {
    const toast = showToast({ type, message: "Mensaje" });
    expect(toast.getAttribute("role")).toBe(role);
    expect(toast.classList.contains(className)).toBe(true);
    expect(toast.querySelector("svg")).not.toBeNull();
  });

  it("el título por defecto depende del tipo y el mensaje se pinta como texto", () => {
    const toast = showToast({
      type: "error",
      message: '<img src=x onerror="alert(1)">No se pudo guardar',
    });
    expect(toast.querySelector(".toast-title").textContent).toBe("Error");
    expect(toast.querySelector(".toast-message").textContent).toContain(
      "No se pudo guardar",
    );
    expect(toast.querySelector("img")).toBeNull();
    const titled = showToast({ type: "success", title: "Listo", message: "x" });
    expect(titled.querySelector(".toast-title").textContent).toBe("Listo");
  });

  it("éxito e info se cierran a los ~4 s y advertencia a los ~7 s", () => {
    showToast({ type: "success", message: "a" });
    showToast({ type: "info", message: "b" });
    showToast({ type: "warning", message: "c" });
    vi.advanceTimersByTime(3999);
    expect(toasts()).toHaveLength(3);
    vi.advanceTimersByTime(1);
    vi.advanceTimersByTime(300);
    expect(toasts()).toHaveLength(1);
    vi.advanceTimersByTime(3000);
    vi.advanceTimersByTime(300);
    expect(toasts()).toHaveLength(0);
  });

  it("error y crítico se quedan hasta cerrarlos con el botón", () => {
    const toast = showToast({ type: "error", message: "Falló" });
    showToast({ type: "critical", message: "Agotado" });
    vi.advanceTimersByTime(60_000);
    expect(toasts()).toHaveLength(2);
    const close = toast.querySelector("button.toast-close");
    expect(close.getAttribute("aria-label")).toBe("Cerrar notificación");
    close.click();
    vi.advanceTimersByTime(300);
    expect(toasts()).toHaveLength(1);
  });

  it("pasar el ratón o tener el foco dentro pausa el cierre", () => {
    const toast = showToast({ type: "success", message: "a" });
    vi.advanceTimersByTime(3000);
    toast.dispatchEvent(new Event("mouseenter"));
    vi.advanceTimersByTime(10_000);
    expect(toasts()).toHaveLength(1);
    toast.dispatchEvent(new Event("mouseleave"));
    vi.advanceTimersByTime(999);
    expect(toasts()).toHaveLength(1);
    vi.advanceTimersByTime(1 + 300);
    expect(toasts()).toHaveLength(0);

    const focused = showToast({ type: "info", message: "b" });
    focused.dispatchEvent(new Event("focusin"));
    vi.advanceTimersByTime(10_000);
    expect(toasts()).toHaveLength(1);
  });

  it("como máximo hay 4 visibles: sale el más antiguo que se cierra solo", () => {
    const error = showToast({ type: "error", message: "persistente" });
    const first = showToast({ type: "success", message: "1" });
    for (const n of [2, 3, 4]) showToast({ type: "info", message: String(n) });
    const visible = toasts().filter((t) => !t.classList.contains("is-leaving"));
    expect(visible).toHaveLength(4);
    expect(visible).toContain(error);
    expect(visible).not.toContain(first);
  });

  it("una acción opcional se muestra como botón y cierra el toast", () => {
    const onClick = vi.fn();
    const toast = showToast({
      type: "info",
      message: "Tienes 3 notificaciones sin leer",
      action: { label: "Ver", onClick },
    });
    const action = toast.querySelector("button.toast-action");
    expect(action.textContent).toBe("Ver");
    action.click();
    expect(onClick).toHaveBeenCalledOnce();
    vi.advanceTimersByTime(300);
    expect(toasts()).toHaveLength(0);
  });

  it("con prefers-reduced-motion se marca la región para no animar", () => {
    const original = window.matchMedia;
    window.matchMedia = vi.fn(() => ({ matches: true }));
    document.getElementById("toast-region")?.remove();
    showToast({ type: "info", message: "x" });
    expect(region().classList.contains("reduce-motion")).toBe(true);
    window.matchMedia = original;
  });
});
