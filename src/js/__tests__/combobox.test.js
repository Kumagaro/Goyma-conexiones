import { afterEach, beforeEach, describe, expect, it, vi } from "vitest";

describe("componente combobox", () => {
  beforeEach(() => {
    vi.resetModules();
    document.body.innerHTML = '<div id="combo"></div>';
  });

  afterEach(() => {
    document.body.innerHTML = "";
  });

  it("muestra resultados, agrega solo sin coincidencia exacta y permite elegir con teclado", async () => {
    const search = vi
      .fn()
      .mockResolvedValue([{ id: 1, name: "Café molido", isExactMatch: false }]);
    const onAddNew = vi.fn().mockResolvedValue({ id: 2, name: "Cafe" });
    const onSelect = vi.fn();
    const { createCombobox } = await import("../components/combobox.js");
    const combobox = createCombobox({
      container: document.getElementById("combo"),
      id: "category",
      placeholder: "Buscar categoría",
      search,
      formatOption: (item) => item.name,
      addOptionLabel: (query) => `Agregar nueva categoría «${query}»`,
      onAddNew,
      onSelect,
      debounceMs: 0,
    });
    const input = document.querySelector("#combo [role=combobox]");

    input.value = "Cafe";
    input.dispatchEvent(new Event("input", { bubbles: true }));
    await vi.waitFor(() => expect(search).toHaveBeenCalledWith("Cafe"));

    const listbox = document.getElementById("category-listbox");
    expect(input.getAttribute("aria-expanded")).toBe("true");
    expect(listbox.getAttribute("role")).toBe("listbox");
    expect(listbox.textContent).toContain("Café molido");
    expect(listbox.textContent).toContain("Agregar nueva categoría «Cafe»");

    input.dispatchEvent(
      new KeyboardEvent("keydown", { key: "ArrowDown", bubbles: true }),
    );
    input.dispatchEvent(
      new KeyboardEvent("keydown", { key: "ArrowUp", bubbles: true }),
    );
    const activeId = input.getAttribute("aria-activedescendant");
    expect(activeId).toBeTruthy();
    expect(document.getElementById(activeId).getAttribute("role")).toBe(
      "option",
    );
    input.dispatchEvent(
      new KeyboardEvent("keydown", { key: "Enter", bubbles: true }),
    );
    await vi.waitFor(() => expect(onAddNew).toHaveBeenCalledWith("Cafe"));
    expect(combobox.getSelected()).toEqual({ id: 2, name: "Cafe" });
    expect(input.value).toBe("Cafe");
    expect(onSelect).toHaveBeenLastCalledWith({ id: 2, name: "Cafe" });
  });

  it("conserva coincidencias exactas dobles, muestra unidades y limpia solo en edición", async () => {
    const search = vi.fn().mockResolvedValue([
      { id: 1, name: "kg", abbreviation: "k", isExactMatch: true },
      { id: 2, name: "Kilogramo", abbreviation: "kg", isExactMatch: true },
    ]);
    const onAddNew = vi.fn();
    const onSelect = vi.fn();
    const { createCombobox } = await import("../components/combobox.js");
    const combobox = createCombobox({
      container: document.getElementById("combo"),
      id: "unit",
      placeholder: "Buscar unidad",
      search,
      formatOption: (item) => `${item.name} (${item.abbreviation})`,
      addOptionLabel: (query) => `Agregar nueva unidad «${query}»`,
      onAddNew,
      onSelect,
      clearLabel: "Sin unidad",
      debounceMs: 0,
    });
    const input = document.querySelector("#combo [role=combobox]");
    const clearButton = document.querySelector("#combo [data-combobox-clear]");

    input.value = "kg";
    input.dispatchEvent(new Event("input", { bubbles: true }));
    await vi.waitFor(() => expect(search).toHaveBeenCalledWith("kg"));

    const listbox = document.getElementById("unit-listbox");
    expect(listbox.textContent).toContain("kg (k)");
    expect(listbox.textContent).toContain("Kilogramo (kg)");
    expect(listbox.textContent).not.toContain("Agregar nueva unidad");
    listbox.querySelector('[role="option"]').click();
    expect(combobox.getSelected().id).toBe(1);
    expect(clearButton.hidden).toBe(true);

    combobox.setEditing(true);
    expect(clearButton.hidden).toBe(false);
    clearButton.click();
    expect(combobox.getSelected()).toBeNull();
    expect(clearButton.hidden).toBe(true);
    expect(onSelect).toHaveBeenLastCalledWith(null);

    input.value = "kil";
    input.dispatchEvent(new Event("input", { bubbles: true }));
    await vi.waitFor(() => expect(search).toHaveBeenCalledTimes(2));
    const tabEvent = new KeyboardEvent("keydown", {
      key: "Tab",
      bubbles: true,
      cancelable: true,
    });
    input.dispatchEvent(tabEvent);
    expect(tabEvent.defaultPrevented).toBe(false);
    expect(listbox.hidden).toBe(true);
    expect(combobox.getSelected()).toBeNull();

    input.dispatchEvent(new Event("input", { bubbles: true }));
    await vi.waitFor(() => expect(search).toHaveBeenCalledTimes(3));
    input.dispatchEvent(
      new KeyboardEvent("keydown", { key: "Escape", bubbles: true }),
    );
    expect(listbox.hidden).toBe(true);
  });
});
