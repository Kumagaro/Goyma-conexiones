import { afterEach, describe, expect, it, vi } from "vitest";
import { toastText } from "./helpers/invoiceUi.js";

const { createUser, deleteUser, listUsers, updateUser, resetUserPassword } =
  vi.hoisted(() => ({
    createUser: vi.fn(),
    deleteUser: vi.fn(),
    listUsers: vi.fn(),
    updateUser: vi.fn(),
    resetUserPassword: vi.fn(),
  }));

vi.mock("../services/usersService.js", () => ({
  createUser,
  deleteUser,
  listUsers,
  updateUser,
  resetUserPassword,
}));

describe("vista de usuarios", () => {
  afterEach(() => {
    document.body.innerHTML = "";
    delete window.__TAURI__;
    createUser.mockReset();
    deleteUser.mockReset();
    listUsers.mockReset().mockResolvedValue([]);
    updateUser.mockReset();
    resetUserPassword.mockReset();
    vi.resetModules();
  });

  it.each([
    ["contador", "none"],
    ["administrador", ""],
  ])("ajusta la visibilidad del menú para %s", async (role, display) => {
    document.body.innerHTML = `
      <ul><li><a data-page="usuarios">Usuarios</a></li></ul>
    `;
    window.__TAURI__ = {
      core: {
        invoke: vi.fn().mockResolvedValue({
          username: "usuario1",
          role,
          rememberSession: false,
        }),
      },
    };

    const usersView = await import("../views/usuarios.js");
    await usersView.initUsersView();

    expect(
      document.querySelector('a[data-page="usuarios"]').closest("li").style
        .display,
    ).toBe(display);
  });

  it("pinta los usuarios y no muestra acciones en la fila propia", async () => {
    document.body.innerHTML = `
      <ul><li><a data-page="usuarios">Usuarios</a></li></ul>
      <input id="usuario-search" />
      <table><tbody id="usuarios-body"></tbody></table>
    `;
    window.__TAURI__ = {
      core: {
        invoke: vi.fn().mockResolvedValue({
          username: "admin",
          role: "administrador",
          rememberSession: false,
        }),
      },
    };
    listUsers.mockResolvedValue([
      {
        id: 1,
        username: "admin",
        role: "administrador",
        phone: "+5300000000",
        nationalId: "00000000000",
        isCurrentUser: true,
      },
      {
        id: 2,
        username: "contador1",
        role: "contador",
        phone: "+5350000001",
        nationalId: "12345678901",
        isCurrentUser: false,
      },
    ]);

    const usersView = await import("../views/usuarios.js");
    await usersView.initUsersView();

    const rows = [...document.querySelectorAll("#usuarios-body tr")];
    expect(listUsers).toHaveBeenCalledOnce();
    expect(rows).toHaveLength(2);
    expect(rows[0].textContent).toContain("admin");
    expect(rows[0].textContent).toContain("Administrador");
    expect(rows[0].textContent).toContain("+5300000000");
    expect(rows[0].textContent).toContain("00000000000");
    expect(rows[0].querySelector("button, a")).toBeNull();
    expect(
      rows[1].querySelector("button[data-user-action='edit']"),
    ).not.toBeNull();
    expect(rows[1].textContent).toContain("Contador");
  });

  it("guarda primero los datos y muestra error parcial si falla el reset", async () => {
    document.body.innerHTML = `
      <ul><li><a data-page="usuarios">Usuarios</a></li></ul>
      <button id="tab-usuario-list"></button>
      <button id="tab-usuario-create"></button>
      <input id="usuario-search" />
      <table><tbody id="usuarios-body"></tbody></table>
      <p id="usuarios-feedback" role="status"></p>
      <form id="usuario-form">
        <h3 id="usuario-form-title"></h3>
        <h3 id="usuario-form-title"></h3>
        <input id="usuario-username" name="username" required />
        <select id="usuario-rol" name="role" required>
          <option value="contador">Contador</option>
          <option value="administrador">Administrador</option>
        </select>
        <input id="usuario-telefono" name="phone" required />
        <input id="usuario-ci" name="nationalId" required />
        <div id="usuario-password-section">
          <input id="usuario-password" name="password" minlength="8" maxlength="72" />
          <input id="usuario-password-confirm" name="confirmPassword" minlength="8" maxlength="72" />
        </div>
        <p id="usuario-feedback" role="status"></p>
        <button type="button" data-user-action="cancel">Cancelar</button>
        <button type="submit">Guardar</button>
      </form>
    `;
    window.__TAURI__ = {
      core: {
        invoke: vi.fn().mockResolvedValue({
          username: "admin",
          role: "administrador",
          rememberSession: false,
        }),
      },
    };
    const listedUsers = [
      {
        id: 1,
        username: "admin",
        role: "administrador",
        phone: "+5300000000",
        nationalId: "00000000000",
        isCurrentUser: true,
      },
      {
        id: 2,
        username: "counterone",
        role: "contador",
        phone: "+5350000001",
        nationalId: "12345678901",
        isCurrentUser: false,
      },
    ];
    listUsers.mockResolvedValue(listedUsers);
    updateUser.mockResolvedValue({
      ...listedUsers[1],
      username: "newcounter",
      role: "administrador",
      phone: "+5350000002",
      nationalId: "12345678902",
    });
    resetUserPassword.mockRejectedValue({
      code: "validationFailed",
      message: "La contraseña debe incluir al menos una mayúscula",
    });

    const usersView = await import("../views/usuarios.js");
    await usersView.initUsersView();
    document
      .querySelector("#usuarios-body button[data-user-action='edit']")
      .click();
    document.getElementById("usuario-username").value = "NewCounter";
    document.getElementById("usuario-rol").value = "administrador";
    document.getElementById("usuario-telefono").value = "+5350000002";
    document.getElementById("usuario-ci").value = "12345678902";
    document.getElementById("usuario-password").value = "weakpassword2!";
    document.getElementById("usuario-password-confirm").value =
      "weakpassword2!";
    document
      .getElementById("usuario-form")
      .dispatchEvent(new Event("submit", { bubbles: true, cancelable: true }));

    await vi.waitFor(() => expect(resetUserPassword).toHaveBeenCalledOnce());
    expect(updateUser).toHaveBeenCalledWith(
      2,
      "NewCounter",
      "administrador",
      "+5350000002",
      "12345678902",
    );
    expect(resetUserPassword).toHaveBeenCalledWith(2, "weakpassword2!");
    expect(toastText()).toContain(
      "Los datos se guardaron, pero la contraseña no se cambió",
    );
    expect(
      document.querySelector("#usuarios-body tr[data-username='newcounter']"),
    ).not.toBeNull();
  });

  it("no intenta resetear si falla updateUser y no ofrece editar la fila propia", async () => {
    document.body.innerHTML = `
      <ul><li><a data-page="usuarios">Usuarios</a></li></ul>
      <button id="tab-usuario-list"></button>
      <button id="tab-usuario-create"></button>
      <input id="usuario-search" />
      <table><tbody id="usuarios-body"></tbody></table>
      <p id="usuarios-feedback" role="status"></p>
      <form id="usuario-form">
        <h3 id="usuario-form-title"></h3>
        <input id="usuario-username" name="username" required />
        <select id="usuario-rol" name="role" required><option value="contador">Contador</option></select>
        <input id="usuario-telefono" name="phone" required />
        <input id="usuario-ci" name="nationalId" required />
        <div id="usuario-password-section">
          <input id="usuario-password" name="password" minlength="8" maxlength="72" />
          <input id="usuario-password-confirm" name="confirmPassword" minlength="8" maxlength="72" />
        </div>
        <p id="usuario-feedback" role="status"></p>
        <button type="button" data-user-action="cancel">Cancelar</button>
        <button type="submit">Guardar</button>
      </form>
    `;
    window.__TAURI__ = {
      core: {
        invoke: vi.fn().mockResolvedValue({
          username: "admin",
          role: "administrador",
          rememberSession: false,
        }),
      },
    };
    listUsers.mockResolvedValue([
      {
        id: 1,
        username: "admin",
        role: "administrador",
        phone: "+5300000000",
        nationalId: "00000000000",
        isCurrentUser: true,
      },
      {
        id: 2,
        username: "counterone",
        role: "contador",
        phone: "+5350000001",
        nationalId: "12345678901",
        isCurrentUser: false,
      },
    ]);
    updateUser.mockRejectedValue({
      code: "duplicateField",
      message: "El teléfono ya está registrado",
    });

    const usersView = await import("../views/usuarios.js");
    await usersView.initUsersView();
    const rows = [...document.querySelectorAll("#usuarios-body tr")];
    expect(rows[0].querySelector("button[data-user-action='edit']")).toBeNull();
    expect(
      rows[1].querySelector("button[data-user-action='edit']"),
    ).not.toBeNull();

    rows[1].querySelector("button[data-user-action='edit']").click();
    document.getElementById("usuario-username").value = "newcounter";
    document.getElementById("usuario-rol").value = "contador";
    document.getElementById("usuario-telefono").value = "+5350000003";
    document.getElementById("usuario-ci").value = "12345678903";
    document.getElementById("usuario-password").value = "NewPassword2!";
    document.getElementById("usuario-password-confirm").value = "NewPassword2!";
    document
      .getElementById("usuario-form")
      .dispatchEvent(new Event("submit", { bubbles: true, cancelable: true }));

    await vi.waitFor(() => expect(updateUser).toHaveBeenCalledOnce());
    expect(resetUserPassword).not.toHaveBeenCalled();
    expect(toastText()).toContain("El teléfono ya está registrado");
  });

  it.each([
    ["username", "tador1", "contador1"],
    ["teléfono", "50000002", "contador2"],
    ["CI", "98765", "contador2"],
    ["no filtra por rol", "administrador", null],
  ])(
    "filtra en memoria por %s sin volver a consultar",
    async (_field, query, expected) => {
      document.body.innerHTML = `
      <ul><li><a data-page="usuarios">Usuarios</a></li></ul>
      <input id="usuario-search" />
      <table><tbody id="usuarios-body"></tbody></table>
    `;
      window.__TAURI__ = {
        core: {
          invoke: vi.fn().mockResolvedValue({
            username: "admin",
            role: "administrador",
            rememberSession: false,
          }),
        },
      };
      listUsers.mockResolvedValue([
        {
          id: 1,
          username: "adminuser",
          role: "administrador",
          phone: "+5350000000",
          nationalId: "12345678900",
          isCurrentUser: true,
        },
        {
          id: 2,
          username: "contador1",
          role: "contador",
          phone: "+5350000001",
          nationalId: "12345678901",
          isCurrentUser: false,
        },
        {
          id: 3,
          username: "contador2",
          role: "contador",
          phone: "+5350000002",
          nationalId: "98765678901",
          isCurrentUser: false,
        },
      ]);

      const usersView = await import("../views/usuarios.js");
      await usersView.initUsersView();
      const search = document.getElementById("usuario-search");
      search.value = query;
      search.dispatchEvent(new Event("input", { bubbles: true }));

      const renderedUsers = [
        ...document.querySelectorAll("#usuarios-body tr"),
      ].map((row) => row.dataset.username);
      expect(renderedUsers).toEqual(expected ? [expected] : []);
      expect(listUsers).toHaveBeenCalledOnce();
    },
  );

  it("crea usuario con confirmación local y muestra la respuesta del backend", async () => {
    document.body.innerHTML = `
      <ul><li><a data-page="usuarios">Usuarios</a></li></ul>
      <button id="tab-usuario-list"></button>
      <button id="tab-usuario-create"></button>
      <form id="usuario-form">
        <input id="usuario-username" name="username" required value="Contador1" />
        <select id="usuario-rol" name="role" required>
          <option value="contador" selected>Contador</option>
        </select>
        <input id="usuario-telefono" name="phone" required value="+5350000001" />
        <input id="usuario-ci" name="nationalId" required value="12345678902" />
        <input id="usuario-password" name="password" required value="ValidPass1!" />
        <input id="usuario-password-confirm" name="confirmPassword" required value="ValidPass1!" />
        <button type="submit">Crear</button>
      </form>
      <p id="usuario-feedback" role="status" hidden></p>
    `;
    window.__TAURI__ = {
      core: {
        invoke: vi.fn().mockResolvedValue({
          username: "adminuser",
          role: "administrador",
          rememberSession: false,
        }),
      },
    };
    createUser.mockResolvedValue({ username: "contador1" });

    const usersView = await import("../views/usuarios.js");
    await usersView.initUsersView();
    const form = document.getElementById("usuario-form");
    form.dispatchEvent(
      new Event("submit", { bubbles: true, cancelable: true }),
    );
    await vi.waitFor(() => expect(createUser).toHaveBeenCalledOnce());

    expect(createUser).toHaveBeenCalledWith(
      "Contador1",
      "contador",
      "+5350000001",
      "12345678902",
      "ValidPass1!",
    );
    expect(toastText()).toContain("contador1");
  });

  it("no envía el usuario si las contraseñas de creación no coinciden", async () => {
    document.body.innerHTML = `
      <ul><li><a data-page="usuarios">Usuarios</a></li></ul>
      <form id="usuario-form">
        <input id="usuario-username" name="username" required value="contador1" />
        <select id="usuario-rol" name="role" required>
          <option value="contador" selected>Contador</option>
        </select>
        <input id="usuario-telefono" name="phone" required value="+5350000001" />
        <input id="usuario-ci" name="nationalId" required value="12345678902" />
        <input id="usuario-password" name="password" required value="ValidPass1!" />
        <input id="usuario-password-confirm" name="confirmPassword" required value="Different1!" />
      </form>
      <p id="usuario-feedback" role="status" hidden></p>
    `;
    window.__TAURI__ = {
      core: {
        invoke: vi.fn().mockResolvedValue({
          username: "adminuser",
          role: "administrador",
          rememberSession: false,
        }),
      },
    };

    const usersView = await import("../views/usuarios.js");
    await usersView.initUsersView();
    document
      .getElementById("usuario-form")
      .dispatchEvent(new Event("submit", { bubbles: true, cancelable: true }));

    expect(createUser).not.toHaveBeenCalled();
    expect(
      document.getElementById("usuario-password-confirm").validationMessage,
    ).toBe("Las contraseñas no coinciden");
  });

  it("muestra en español los errores HTML5 del formulario de alta", async () => {
    document.body.innerHTML = `
      <ul><li><a data-page="usuarios">Usuarios</a></li></ul>
      <form id="usuario-form">
        <input id="usuario-username" name="username" data-validation-field="username" value="ab" minlength="3" maxlength="50" pattern="[A-Za-z0-9]{3,50}" required />
      </form>
    `;
    window.__TAURI__ = {
      core: {
        invoke: vi.fn().mockResolvedValue({
          username: "adminuser",
          role: "administrador",
          rememberSession: false,
        }),
      },
    };

    const usersView = await import("../views/usuarios.js");
    await usersView.initUsersView();
    const username = document.getElementById("usuario-username");
    expect(username.checkValidity()).toBe(false);
    expect(username.validationMessage).toBe(
      "El nombre de usuario debe tener de 3 a 50 caracteres alfanuméricos (letras y números, sin acentos ni espacios)",
    );
  });

  it("elimina usuario con confirmación y actualiza la lista", async () => {
    document.body.innerHTML = `
      <ul><li><a data-page="usuarios">Usuarios</a></li></ul>
      <button id="tab-usuario-list"></button>
      <input id="usuario-search" />
      <table><tbody id="usuarios-body"></tbody></table>
      <p id="usuarios-feedback" role="status"></p>
    `;
    window.__TAURI__ = {
      core: {
        invoke: vi.fn().mockResolvedValue({
          username: "admin",
          role: "administrador",
          rememberSession: false,
        }),
      },
    };
    const listedUsers = [
      {
        id: 1,
        username: "admin",
        role: "administrador",
        phone: "+5300000000",
        nationalId: "00000000000",
        isCurrentUser: true,
      },
      {
        id: 2,
        username: "counterone",
        role: "contador",
        phone: "+5350000001",
        nationalId: "12345678901",
        isCurrentUser: false,
      },
    ];
    listUsers.mockResolvedValue(listedUsers);
    deleteUser.mockResolvedValue(undefined);
    global.confirm = vi.fn(() => true);

    const usersView = await import("../views/usuarios.js");
    await usersView.initUsersView();

    const deleteButton = document.querySelector(
      "#usuarios-body button[data-user-action='delete']",
    );
    expect(deleteButton).not.toBeNull();
    deleteButton.click();

    await vi.waitFor(() => expect(deleteUser).toHaveBeenCalledOnce());
    expect(deleteUser).toHaveBeenCalledWith(2);
    expect(global.confirm).toHaveBeenCalledWith(
      expect.stringContaining(
        '¿Estás seguro de que quieres eliminar al usuario "counterone"?',
      ),
    );
    expect(toastText()).toContain("Usuario counterone eliminado correctamente");
    expect(
      document.querySelector("#usuarios-body tr[data-username='counterone']"),
    ).toBeNull();
  });

  it("no elimina si el usuario cancela la confirmación", async () => {
    document.body.innerHTML = `
      <ul><li><a data-page="usuarios">Usuarios</a></li></ul>
      <button id="tab-usuario-list"></button>
      <input id="usuario-search" />
      <table><tbody id="usuarios-body"></tbody></table>
    `;
    window.__TAURI__ = {
      core: {
        invoke: vi.fn().mockResolvedValue({
          username: "admin",
          role: "administrador",
          rememberSession: false,
        }),
      },
    };
    listUsers.mockResolvedValue([
      {
        id: 1,
        username: "admin",
        role: "administrador",
        phone: "+5300000000",
        nationalId: "00000000000",
        isCurrentUser: true,
      },
      {
        id: 2,
        username: "counterone",
        role: "contador",
        phone: "+5350000001",
        nationalId: "12345678901",
        isCurrentUser: false,
      },
    ]);
    global.confirm = vi.fn(() => false);

    const usersView = await import("../views/usuarios.js");
    await usersView.initUsersView();

    const deleteButton = document.querySelector(
      "#usuarios-body button[data-user-action='delete']",
    );
    deleteButton.click();

    expect(deleteUser).not.toHaveBeenCalled();
    expect(
      document.querySelector("#usuarios-body tr[data-username='counterone']"),
    ).not.toBeNull();
  });

  it("no ofrece botón de eliminar en la fila propia", async () => {
    document.body.innerHTML = `
      <ul><li><a data-page="usuarios">Usuarios</a></li></ul>
      <button id="tab-usuario-list"></button>
      <input id="usuario-search" />
      <table><tbody id="usuarios-body"></tbody></table>
    `;
    window.__TAURI__ = {
      core: {
        invoke: vi.fn().mockResolvedValue({
          username: "admin",
          role: "administrador",
          rememberSession: false,
        }),
      },
    };
    listUsers.mockResolvedValue([
      {
        id: 1,
        username: "admin",
        role: "administrador",
        phone: "+5300000000",
        nationalId: "00000000000",
        isCurrentUser: true,
      },
      {
        id: 2,
        username: "counterone",
        role: "contador",
        phone: "+5350000001",
        nationalId: "12345678901",
        isCurrentUser: false,
      },
    ]);

    const usersView = await import("../views/usuarios.js");
    await usersView.initUsersView();

    const rows = [...document.querySelectorAll("#usuarios-body tr")];
    expect(
      rows[0].querySelector("button[data-user-action='delete']"),
    ).toBeNull();
    expect(
      rows[1].querySelector("button[data-user-action='delete']"),
    ).not.toBeNull();
  });
});
