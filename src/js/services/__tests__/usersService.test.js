import { afterEach, describe, expect, it, vi } from "vitest";

describe("servicio de usuarios", () => {
  afterEach(() => {
    delete window.__TAURI__;
    vi.resetModules();
  });

  it("envía el contrato de creación al comando Tauri sin la confirmación", async () => {
    const invoke = vi.fn().mockResolvedValue({
      id: 2,
      username: "contador1",
      role: "contador",
      phone: "+5350000001",
      nationalId: "12345678902",
      isCurrentUser: false,
    });
    window.__TAURI__ = { core: { invoke } };

    const { createUser } = await import("../usersService.js");
    const result = await createUser(
      "Contador1",
      "contador",
      "+5350000001",
      "12345678902",
      "ValidPass1!",
    );

    expect(invoke).toHaveBeenCalledWith("create_user", {
      username: "Contador1",
      role: "contador",
      phone: "+5350000001",
      nationalId: "12345678902",
      password: "ValidPass1!",
    });
    expect(result.username).toBe("contador1");
  });

  it("invoca list_users para obtener los DTO del listado", async () => {
    const users = [
      {
        id: 1,
        username: "admin",
        role: "administrador",
        phone: "+5300000000",
        nationalId: "00000000000",
        isCurrentUser: true,
      },
    ];
    const invoke = vi.fn().mockResolvedValue(users);
    window.__TAURI__ = { core: { invoke } };

    const { listUsers } = await import("../usersService.js");

    await expect(listUsers()).resolves.toEqual(users);
    expect(invoke).toHaveBeenCalledWith("list_users", undefined);
  });

  it("invoca get_user_profile para obtener el perfil autenticado", async () => {
    const profile = {
      username: "admin",
      role: "administrador",
      phone: "+5300000000",
      nationalId: "00000000000",
      hasReservedContactData: true,
    };
    const invoke = vi.fn().mockResolvedValue(profile);
    window.__TAURI__ = { core: { invoke } };

    const { getUserProfile } = await import("../usersService.js");

    await expect(getUserProfile()).resolves.toEqual(profile);
    expect(invoke).toHaveBeenCalledWith("get_user_profile", undefined);
  });

  it("envía solo los datos editables a update_own_profile", async () => {
    const profile = {
      username: "admin",
      role: "administrador",
      phone: "+5350000001",
      nationalId: "12345678901",
      hasReservedContactData: false,
    };
    const invoke = vi.fn().mockResolvedValue(profile);
    window.__TAURI__ = { core: { invoke } };

    const { updateOwnProfile } = await import("../usersService.js");

    await expect(
      updateOwnProfile("Admin", "+5350000001", "12345678901"),
    ).resolves.toEqual(profile);
    expect(invoke).toHaveBeenCalledWith("update_own_profile", {
      username: "Admin",
      phone: "+5350000001",
      nationalId: "12345678901",
    });
  });

  it("envía solo contraseña actual y nueva a change_own_password", async () => {
    const invoke = vi.fn().mockResolvedValue(null);
    window.__TAURI__ = { core: { invoke } };

    const { changeOwnPassword } = await import("../usersService.js");

    await expect(
      changeOwnPassword("CurrentPass1!", "NewPassword2!"),
    ).resolves.toBeNull();
    expect(invoke).toHaveBeenCalledWith("change_own_password", {
      currentPassword: "CurrentPass1!",
      newPassword: "NewPassword2!",
    });
  });

  it("envía los datos de edición a update_user", async () => {
    const user = {
      id: 4,
      username: "counterone",
      role: "administrador",
      phone: "+5350000001",
      nationalId: "12345678901",
      isCurrentUser: false,
    };
    const invoke = vi.fn().mockResolvedValue(user);
    window.__TAURI__ = { core: { invoke } };

    const { updateUser } = await import("../usersService.js");
    await expect(
      updateUser(
        4,
        "CounterOne",
        "administrador",
        "+5350000001",
        "12345678901",
      ),
    ).resolves.toEqual(user);
    expect(invoke).toHaveBeenCalledWith("update_user", {
      userId: 4,
      username: "CounterOne",
      role: "administrador",
      phone: "+5350000001",
      nationalId: "12345678901",
    });
  });

  it("omite currentPassword del reset administrativo de otro usuario", async () => {
    const invoke = vi.fn().mockResolvedValue(null);
    window.__TAURI__ = { core: { invoke } };

    const { resetUserPassword } = await import("../usersService.js");
    await expect(resetUserPassword(4, "NewPassword2!")).resolves.toBeNull();
    expect(invoke).toHaveBeenCalledWith("reset_user_password", {
      userId: 4,
      newPassword: "NewPassword2!",
    });
  });
});
