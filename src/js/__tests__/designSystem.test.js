import { afterEach, describe, expect, it, vi } from "vitest";
import { readFile } from "node:fs/promises";
import path from "node:path";
import { emptyState } from "../utils/dom.js";
import { installFormValidation } from "../utils/formValidation.js";
import { installShortcuts } from "../utils/shortcuts.js";
import { cellValue, installTableSort } from "../utils/tableSort.js";

const read = (...parts) =>
  readFile(path.resolve(process.cwd(), "src", ...parts), "utf8");

afterEach(() => {
  document.body.replaceChildren();
});

describe("reglas del sistema de diseño (ui-spec)", () => {
  it("las hojas no vuelven a valores sueltos ni a texto de menos de 12px", async () => {
    const css = await read("css", "dashboard.css");
    expect(css).not.toMatch(/font-size:\s*(9|10|11)(\.\d+)?px/);
    expect(css).not.toMatch(/transition:\s*all\b/);
    expect(css).not.toMatch(/color:\s*#020f1e80/);
  });

  it("cada página tiene un único título h1", async () => {
    const html = await read("dashboard-unificado.html");
    const parsed = new DOMParser().parseFromString(html, "text/html");
    for (const page of parsed.querySelectorAll(".page")) {
      if (page.id === "page-placeholder") continue;
      expect(page.querySelectorAll("h1"), page.id).toHaveLength(1);
    }
  });

  it("la campana es un botón real con nombre", async () => {
    const html = await read("dashboard-unificado.html");
    const parsed = new DOMParser().parseFromString(html, "text/html");
    const bell = parsed.querySelector(".notificaciones");
    expect(bell.tagName).toBe("BUTTON");
    expect(bell.getAttribute("aria-label")).toBe("Ver notificaciones");
  });
});

describe("arranque de las vistas", () => {
  it("todo enlace del menú lleva a una página conocida", async () => {
    const [html, script] = await Promise.all([
      read("dashboard-unificado.html"),
      read("js", "dashboard.js"),
    ]);
    const parsed = new DOMParser().parseFromString(html, "text/html");
    const known = /const knownPages = \[([\s\S]*?)\];/.exec(script)[1];
    for (const link of parsed.querySelectorAll("#sidebar [data-page]")) {
      expect(known, link.dataset.page).toContain(`"${link.dataset.page}"`);
    }
  });

  it("toda vista arranca aunque el documento ya esté cargado", async () => {
    // Los módulos corren después de analizar el HTML: nunca en "loading".
    for (const view of ["clientesView", "proveedoresView"]) {
      const source = await read("js", "views", `${view}.js`);
      expect(source, view).toMatch(/readyState[\s\S]*\} else \{/);
    }
  });
});

describe("estado vacío", () => {
  function table() {
    document.body.innerHTML =
      "<table><thead><tr><th>A</th><th>B</th></tr></thead><tbody id='b'></tbody></table>";
    return document.getElementById("b");
  }

  it("dice qué falta y ofrece crearlo", () => {
    const onClick = vi.fn();
    const body = table();
    emptyState(body, {
      empty: "Aún no hay clientes.",
      action: { label: "Crear cliente", onClick },
    });
    expect(body.querySelector("td").getAttribute("colspan")).toBe("2");
    expect(body.textContent).toContain("Aún no hay clientes.");
    body.querySelector("button").click();
    expect(onClick).toHaveBeenCalled();
  });

  it("con filtros no ofrece crear", () => {
    const body = table();
    emptyState(body, {
      empty: "Aún no hay clientes.",
      filtered: true,
      action: { label: "Crear cliente", onClick: vi.fn() },
    });
    expect(body.textContent).toContain("No hay resultados");
    expect(body.querySelector("button")).toBeNull();
  });
});

describe("validación al salir del campo", () => {
  function form() {
    document.body.innerHTML = `<form id="f">
      <input id="phone" name="phone" pattern="\\+53[0-9]{8}" required />
    </form>`;
    const root = document.getElementById("f");
    installFormValidation(root);
    return root;
  }

  const leave = (input) =>
    input.dispatchEvent(new FocusEvent("focusout", { bubbles: true }));

  it("avisa junto al campo y el error se va al corregir", () => {
    const input = form().querySelector("input");
    input.value = "55";
    leave(input);
    const error = document.getElementById("phone-error");
    expect(error.textContent).toContain("+53");
    expect(input.getAttribute("aria-invalid")).toBe("true");

    input.value = "+5355555555";
    input.dispatchEvent(new Event("input", { bubbles: true }));
    expect(document.getElementById("phone-error")).toBeNull();
  });

  it("un campo vacío no molesta al pasar por él, pero se avisa al guardar", () => {
    const root = form();
    const input = root.querySelector("input");
    leave(input);
    expect(document.getElementById("phone-error")).toBeNull();

    root.reportValidity();
    expect(document.getElementById("phone-error").textContent).toBe(
      "Completa este campo",
    );
  });
});

describe("ordenar columnas", () => {
  function table() {
    const wrapper = document.createElement("div");
    wrapper.innerHTML = `<table class="list-table">
      <thead><tr><th>Nombre</th><th>Total</th><th id="th-fecha-x">Fecha</th><th>Acciones</th></tr></thead>
      <tbody>
        <tr><td>Ñandú</td><td>1.250,50 USD</td><td>05/11/2026</td><td></td></tr>
        <tr><td>Ana</td><td>18,75 USD</td><td>01/02/2026</td><td></td></tr>
        <tr><td>Óscar</td><td>300,00 USD</td><td>03/01/2026</td><td></td></tr>
      </tbody></table>`;
    // Fuera del documento: otra vista ya pudo instalar el orden en `document`.
    installTableSort(wrapper);
    return wrapper;
  }
  const column = (root, index) =>
    [...root.querySelectorAll("tbody tr")].map(
      (row) => row.cells[index].textContent,
    );

  it("entiende importes, fechas y códigos", () => {
    expect(cellValue("1.250,50 USD")).toBe(1250.5);
    expect(cellValue("#00012")).toBe(12);
    expect(cellValue("05/11/2026")).toBe(20261105);
    expect(cellValue("Cable 2")).toBe("Cable 2");
  });

  it("ordena números y textos, y alterna el sentido", () => {
    const root = table();
    const [name, total] = root.querySelectorAll("th");
    total.click();
    expect(column(root, 1)).toEqual([
      "18,75 USD",
      "300,00 USD",
      "1.250,50 USD",
    ]);
    expect(total.getAttribute("aria-sort")).toBe("ascending");
    total.click();
    expect(column(root, 1)[0]).toBe("1.250,50 USD");
    expect(total.getAttribute("aria-sort")).toBe("descending");

    name.click();
    expect(column(root, 0)).toEqual(["Ana", "Ñandú", "Óscar"]);
    expect(total.hasAttribute("aria-sort")).toBe(false);
  });

  it("deja fuera las acciones y la fecha con orden propio", () => {
    const root = table();
    const [, , date, actions] = root.querySelectorAll("th");
    date.click();
    actions.click();
    expect(column(root, 0)[0]).toBe("Ñandú");
    expect(date.hasAttribute("tabindex")).toBe(false);
    expect(actions.hasAttribute("tabindex")).toBe(false);
  });
});

describe("atajos de teclado", () => {
  it("Ctrl+S guarda el formulario abierto", () => {
    document.body.innerHTML = `<div class="modal-overlay active"><form></form></div>`;
    const form = document.querySelector("form");
    form.requestSubmit = vi.fn();
    installShortcuts();
    const event = new KeyboardEvent("keydown", {
      key: "s",
      ctrlKey: true,
      bubbles: true,
      cancelable: true,
    });
    document.dispatchEvent(event);
    expect(form.requestSubmit).toHaveBeenCalled();
    expect(event.defaultPrevented).toBe(true);
  });
});
