import { afterEach, describe, expect, it, vi } from "vitest";

describe("servicios de contactos", () => {
  afterEach(() => {
    delete window.__TAURI__;
    vi.resetModules();
  });

  it("envía los comandos de clientes con el contrato Tauri correcto", async () => {
    const invoke = vi.fn().mockResolvedValue({ ok: true });
    window.__TAURI__ = { core: { invoke } };

    const {
      listClients,
      searchClients,
      createClient,
      updateClient,
      deleteClient,
      getClientDetail,
    } = await import("../clientsService.js");

    const input = {
      firstName: "Ana",
      lastName: "Pérez",
      nationalId: "12345678901",
      phone: "+5355551234",
      company: "Café Norte",
      position: "Gerente",
      email: "ana@cafenorte.com",
      address: "Calle 5",
      invoiceInfo: "Factura mensual",
    };

    await listClients();
    await searchClients("ana");
    await createClient(input);
    await updateClient(7, input);
    await deleteClient(7);
    await getClientDetail(7);

    expect(invoke).toHaveBeenNthCalledWith(1, "list_clients", undefined);
    expect(invoke).toHaveBeenNthCalledWith(2, "search_clients", { query: "ana" });
    expect(invoke).toHaveBeenNthCalledWith(3, "create_client", { input });
    expect(invoke).toHaveBeenNthCalledWith(4, "update_client", {
      clientId: 7,
      input,
    });
    expect(invoke).toHaveBeenNthCalledWith(5, "delete_client", { clientId: 7 });
    expect(invoke).toHaveBeenNthCalledWith(6, "get_client_detail", { clientId: 7 });
  });

  it("envía los comandos de proveedores con el contrato Tauri correcto", async () => {
    const invoke = vi.fn().mockResolvedValue({ ok: true });
    window.__TAURI__ = { core: { invoke } };

    const {
      listSuppliers,
      searchSuppliers,
      createSupplier,
      updateSupplier,
      deleteSupplier,
      getSupplierDetail,
    } = await import("../suppliersService.js");

    const input = {
      firstName: "Luis",
      lastName: "García",
      nationalId: "12345678902",
      phone: "+5355555678",
      company: "García & Co",
      position: "Director",
      email: "luis@garciayco.com",
      address: "Avenida 2",
    };

    await listSuppliers();
    await searchSuppliers("garcia");
    await createSupplier(input);
    await updateSupplier(9, input);
    await deleteSupplier(9);
    await getSupplierDetail(9);

    expect(invoke).toHaveBeenNthCalledWith(1, "list_suppliers", undefined);
    expect(invoke).toHaveBeenNthCalledWith(2, "search_suppliers", { query: "garcia" });
    expect(invoke).toHaveBeenNthCalledWith(3, "create_supplier", { input });
    expect(invoke).toHaveBeenNthCalledWith(4, "update_supplier", {
      supplierId: 9,
      input,
    });
    expect(invoke).toHaveBeenNthCalledWith(5, "delete_supplier", { supplierId: 9 });
    expect(invoke).toHaveBeenNthCalledWith(6, "get_supplier_detail", { supplierId: 9 });
  });
});
