import {
  afterEach,
  beforeAll,
  beforeEach,
  describe,
  expect,
  it,
  vi,
} from "vitest";
import {
  chooseFromCombobox,
  lineInputs,
  loadInvoicePagesHtml,
  setValue,
  submitForm,
  toastText,
  typeInCombobox,
} from "./helpers/invoiceUi.js";

vi.mock("../services/invoicesService.js", () => ({
  listInvoices: vi.fn(),
  getInvoice: vi.fn(),
  createEntryInvoice: vi.fn(),
  updateEntryInvoice: vi.fn(),
  createSaleInvoice: vi.fn(),
  updateSaleInvoice: vi.fn(),
  createServiceInvoice: vi.fn(),
  updateServiceInvoice: vi.fn(),
  createGuaranteeInvoice: vi.fn(),
  updateGuaranteeInvoice: vi.fn(),
  createInstallationInvoice: vi.fn(),
  updateInstallationInvoice: vi.fn(),
  createLossInvoice: vi.fn(),
  saveInvoiceAttachment: vi.fn(),
  downloadInvoiceAttachment: vi.fn(),
  deleteInvoice: vi.fn(),
  changeInvoiceStatus: vi.fn(),
  getInvoicePrintModel: vi.fn(),
  exportInvoicePdf: vi.fn(),
}));

vi.mock("../services/clientsService.js", () => ({
  searchClients: vi.fn(),
  getClientDetail: vi.fn(),
  createClient: vi.fn(),
}));

vi.mock("../services/suppliersService.js", () => ({
  searchSuppliers: vi.fn(),
  createSupplier: vi.fn(),
}));

vi.mock("../services/productsService.js", () => ({
  listUnitOptionsForProduct: vi.fn(),
}));

vi.mock("../services/historyService.js", () => ({
  searchProductsForInvoice: vi.fn(),
  getProductAssociatedInvoices: vi.fn(),
}));

vi.mock("../services/categoriesService.js", () => ({
  listCategories: vi.fn(() =>
    Promise.resolve({ items: [{ id: 2, name: "Redes" }] }),
  ),
}));

vi.mock("../services/unitsService.js", () => ({
  listUnits: vi.fn(() =>
    Promise.resolve({ items: [{ id: 3, name: "Unidad", abbreviation: "u" }] }),
  ),
}));

vi.mock("../components/confirmDialog.js", () => ({
  confirmDialog: vi.fn(() => Promise.resolve(true)),
}));

// Con isolate:false los módulos se comparten entre archivos de test: se
// recarga la vista para que use los mocks de este archivo.
let invoices, clients, suppliers, history, products, confirmDialog, view;
let pagesHtml = "";

beforeAll(async () => {
  vi.resetModules();
  invoices = await import("../services/invoicesService.js");
  clients = await import("../services/clientsService.js");
  suppliers = await import("../services/suppliersService.js");
  history = await import("../services/historyService.js");
  products = await import("../services/productsService.js");
  ({ confirmDialog } = await import("../components/confirmDialog.js"));
  view = await import("../views/facturasView.js");
  pagesHtml = await loadInvoicePagesHtml();
});

const CABLE = {
  id: 7,
  name: "Cable UTP",
  unitAbbreviation: "m",
  stock: "10",
  sellingPriceCents: 250,
  isOutOfStock: false,
  isDiscontinued: false,
};
const AGOTADO = {
  id: 5,
  name: "Conector agotado",
  unitAbbreviation: "u",
  stock: "0",
  sellingPriceCents: 100,
  isOutOfStock: true,
  isDiscontinued: false,
};
const ANA = {
  id: 4,
  fullName: "Ana Pérez",
  company: null,
  isExactMatch: false,
};

// Ids de cada formulario en el HTML de producción.
const TYPES = [
  {
    name: "entrada",
    type: "E",
    view: "factura-entrada",
    field: "factura",
    buttons: "factura-entrada",
    contactInput: "factura-proveedor",
    productInput: "factura-buscar-producto",
    contactKey: "supplierId",
    purpose: "entry",
    create: "createEntryInvoice",
    update: "updateEntryInvoice",
    statusLabel: "Pagada",
  },
  {
    name: "venta",
    type: "V",
    view: "factura-venta",
    field: "factura-venta",
    buttons: "factura-venta",
    contactInput: "factura-venta-cliente",
    productInput: "factura-venta-buscar-producto",
    contactKey: "clientId",
    purpose: "outgoing",
    create: "createSaleInvoice",
    update: "updateSaleInvoice",
    statusLabel: "Cobrada",
  },
  {
    name: "servicio",
    type: "S",
    view: "factura-servicios",
    field: "factura-servicio",
    buttons: "factura-servicio",
    contactInput: "factura-servicio-cliente",
    productInput: "factura-servicio-buscar",
    contactKey: "clientId",
    purpose: "outgoing",
    create: "createServiceInvoice",
    update: "updateServiceInvoice",
    statusLabel: "Cobrada",
  },
  {
    name: "garantía",
    type: "GG",
    view: "factura-garantia",
    field: "factura-garantia",
    buttons: "factura-garantia",
    contactInput: "factura-garantia-cliente",
    productInput: "factura-garantia-buscar",
    contactKey: "clientId",
    purpose: "outgoing",
    create: "createGuaranteeInvoice",
    update: "updateGuaranteeInvoice",
    statusLabel: "Completada",
    associatedSelect: "factura-garantia-original",
    associatedId: 9,
  },
  {
    name: "insumos",
    type: "GI",
    view: "factura-insumos",
    field: "factura-insumo",
    buttons: "factura-insumo",
    contactInput: "factura-insumo-cliente",
    productInput: "factura-insumo-buscar",
    contactKey: "clientId",
    purpose: "outgoing",
    create: "createInstallationInvoice",
    update: "updateInstallationInvoice",
    statusLabel: "Completada",
    associatedSelect: "factura-insumo-servicio",
    associatedId: 8,
  },
];

const linesBody = (t) => `${t.field}-lineas-body`;
// spec-04 TO-01: los mensajes del formulario salen como toast.
const feedback = () => toastText();
const openDetail = (t, id) =>
  ({
    E: view.__testHooks.openEntryDetail,
    V: view.__testHooks.openSaleDetail,
    S: view.__testHooks.openServiceDetail,
    GG: view.__testHooks.openGuaranteeDetail,
    GI: view.__testHooks.openInstallationDetail,
  })[t.type](id);

async function chooseContact(t) {
  await chooseFromCombobox(t.contactInput, "ana", "Ana Pérez");
  if (t.associatedSelect) {
    await vi.waitFor(() =>
      expect(
        document.querySelector(
          `#${t.associatedSelect} option[value="${t.associatedId}"]`,
        ),
      ).not.toBeNull(),
    );
    setValue(t.associatedSelect, String(t.associatedId), "change");
  }
}

beforeEach(async () => {
  document.body.innerHTML = `<main>${pagesHtml}</main>`;
  vi.clearAllMocks();
  invoices.listInvoices.mockResolvedValue([]);
  clients.searchClients.mockResolvedValue([ANA]);
  suppliers.searchSuppliers.mockResolvedValue([ANA]);
  history.searchProductsForInvoice.mockResolvedValue([CABLE, AGOTADO]);
  // Sin conversiones salvo que el test las pida (CV-01).
  products.listUnitOptionsForProduct.mockResolvedValue([]);
  clients.getClientDetail.mockResolvedValue({
    invoiceHistory: [
      { id: 9, number: "V-00009", invoiceType: "V", status: "pending" },
      { id: 8, number: "S-00008", invoiceType: "S", status: "completed" },
      { id: 3, number: "V-00003", invoiceType: "V", status: "cancelled" },
    ],
  });
  await view.initFacturasView();
});

describe.each(TYPES)("formulario de $name (§7.1)", (t) => {
  it("elige el contacto con el buscador y volver a escribir lo deselecciona", async () => {
    await chooseContact(t);
    expect(document.getElementById(t.contactInput).value).toBe("Ana Pérez");
    const search =
      t.contactKey === "supplierId"
        ? suppliers.searchSuppliers
        : clients.searchClients;
    expect(search).toHaveBeenCalledWith("ana");

    // Escribir otra vez anula la selección: guardar pide el contacto.
    const input = document.getElementById(t.contactInput);
    input.value = "an";
    input.dispatchEvent(new Event("input", { bubbles: true }));
    await chooseFromCombobox(t.productInput, "cable", "Cable UTP");
    setValue(`${t.field}-fecha-emision`, "2026-02-10");
    submitForm(`${t.view}-form`);
    expect(feedback(t)).toMatch(/Completa/);
    expect(invoices[t.create]).not.toHaveBeenCalled();
  });

  it("el buscador de producto agrega la línea con su unidad y no repite productos", async () => {
    const options = await typeInCombobox(t.productInput, "cable");
    expect(history.searchProductsForInvoice).toHaveBeenCalledWith(
      "cable",
      t.purpose,
    );
    expect(options[0].textContent).toContain("Cable UTP · stock 10 m");
    options[0].click();

    const rows = document.querySelectorAll(`#${linesBody(t)} .li-row`);
    expect(rows).toHaveLength(1);
    expect(rows[0].textContent).toContain("Cable UTP");
    expect(rows[0].textContent).toContain("m");
    expect(document.getElementById(t.productInput).value).toBe("");

    await chooseFromCombobox(t.productInput, "cable", "Cable UTP");
    expect(document.querySelectorAll(`#${linesBody(t)} .li-row`)).toHaveLength(
      1,
    );
    expect(feedback(t)).toContain("«Cable UTP» ya está en la factura.");
  });

  it("la cantidad se edita como texto, la línea se puede quitar y el total se recalcula", async () => {
    await chooseFromCombobox(t.productInput, "cable", "Cable UTP");
    const quantity = lineInputs(linesBody(t), "quantity")[0];
    quantity.value = "2";
    quantity.dispatchEvent(new Event("input", { bubbles: true }));
    const total = () => document.getElementById(`${t.field}-total`).textContent;
    if (t.type === "E") {
      const cost = lineInputs(linesBody(t), "unitPrice")[0];
      cost.value = "1,50";
      cost.dispatchEvent(new Event("input", { bubbles: true }));
      expect(total()).toBe("3,00 USD");
    } else if (t.type === "V" || t.type === "S") {
      // Precio vigente × cantidad tecleada (total preliminar del frontend).
      expect(total()).toBe("5,00 USD");
    } else {
      // A costo real: solo se conoce al guardar.
      expect(total()).toBe("Se calcula al guardar");
    }

    document.querySelector(`#${linesBody(t)} .li-remove-btn`).click();
    expect(document.querySelectorAll(`#${linesBody(t)} .li-row`)).toHaveLength(
      0,
    );
    expect(document.getElementById(linesBody(t)).textContent).toContain(
      "Busca un producto",
    );
  });

  it("de extremo a extremo: elige contacto y producto, teclea y guarda con los ids correctos", async () => {
    invoices[t.create].mockResolvedValue({
      invoiceId: 31,
      number: `${t.type}-00031`,
      expiredCleanupWarning: null,
    });
    await chooseContact(t);
    await chooseFromCombobox(t.productInput, "cable", "Cable UTP");
    const quantity = lineInputs(linesBody(t), "quantity")[0];
    quantity.value = "2,5";
    quantity.dispatchEvent(new Event("input", { bubbles: true }));
    if (t.type === "E") {
      const cost = lineInputs(linesBody(t), "unitPrice")[0];
      cost.value = "1,50";
      cost.dispatchEvent(new Event("input", { bubbles: true }));
    }
    setValue(`${t.field}-fecha-emision`, "2026-02-10");

    // §14: doble clic en "Guardar Factura" → una sola llamada.
    submitForm(`${t.view}-form`);
    submitForm(`${t.view}-form`);
    await vi.waitFor(() => expect(invoices[t.create]).toHaveBeenCalled());
    expect(invoices[t.create]).toHaveBeenCalledTimes(1);

    const input = invoices[t.create].mock.calls[0][0];
    expect(input[t.contactKey]).toBe(4);
    expect(input.invoiceDate).toBe("2026-02-10");
    expect(input.lines).toEqual([
      t.type === "E"
        ? { productId: 7, quantity: "2,5", unitPrice: "1,50" }
        : { productId: 7, quantity: "2,5" },
    ]);
    if (t.associatedSelect) {
      expect(input.associatedInvoiceId).toBe(t.associatedId);
    }
    expect(JSON.stringify(input)).not.toContain("null");
    expect(Number.isNaN(input.lines[0].productId)).toBe(false);
  });

  it("un error del backend se muestra en español sin perder lo tecleado", async () => {
    invoices[t.create].mockRejectedValue({
      code: "stockInsuficiente",
      message: "No hay stock suficiente de «Cable UTP»",
    });
    await chooseContact(t);
    await chooseFromCombobox(t.productInput, "cable", "Cable UTP");
    const quantity = lineInputs(linesBody(t), "quantity")[0];
    quantity.value = "99";
    quantity.dispatchEvent(new Event("input", { bubbles: true }));
    if (t.type === "E") {
      const cost = lineInputs(linesBody(t), "unitPrice")[0];
      cost.value = "1";
      cost.dispatchEvent(new Event("input", { bubbles: true }));
    }
    setValue(`${t.field}-fecha-emision`, "2026-02-10");
    submitForm(`${t.view}-form`);
    await vi.waitFor(() =>
      expect(feedback(t)).toContain("No hay stock suficiente de «Cable UTP»"),
    );
    expect(lineInputs(linesBody(t), "quantity")[0].value).toBe("99");
    expect(document.getElementById(t.contactInput).value).toBe("Ana Pérez");
  });
});

describe("buscador de productos según el formulario (§7.2)", () => {
  it("en salidas el agotado se ve pero no se puede elegir", async () => {
    const options = await typeInCombobox(
      "factura-venta-buscar-producto",
      "con",
    );
    const agotado = options.find((o) => o.textContent.includes("Agotado"));
    expect(agotado.disabled).toBe(true);
    expect(agotado.getAttribute("aria-disabled")).toBe("true");
    agotado.click();
    expect(
      document.querySelectorAll("#factura-venta-lineas-body .li-row"),
    ).toHaveLength(0);
  });

  it("en entrada el agotado se puede elegir para reponerlo", async () => {
    await chooseFromCombobox(
      "factura-buscar-producto",
      "con",
      "Conector agotado",
    );
    expect(
      document.querySelector("#factura-lineas-body .li-row").textContent,
    ).toContain("Conector agotado");
  });

  it("la pérdida busca con purpose loss y ofrece el descontinuado con stock", async () => {
    history.searchProductsForInvoice.mockResolvedValue([
      {
        id: 12,
        name: "Router viejo",
        stock: "3",
        unitAbbreviation: "u",
        isOutOfStock: false,
        isDiscontinued: true,
      },
    ]);
    await chooseFromCombobox("factura-perdida-producto", "router", "Router");
    expect(history.searchProductsForInvoice).toHaveBeenCalledWith(
      "router",
      "loss",
    );
    expect(document.getElementById("factura-perdida-producto").value).toContain(
      "Descontinuado",
    );
    expect(view.__testHooks.getLossCombobox().getSelected().id).toBe(12);
  });
});

describe("garantía e insumos: factura asociada (§7.1)", () => {
  it("elegir el cliente llena el selector solo con las facturas vigentes del tipo", async () => {
    await chooseFromCombobox("factura-garantia-cliente", "ana", "Ana");
    await vi.waitFor(() =>
      expect(
        document.querySelectorAll("#factura-garantia-original option"),
      ).toHaveLength(3),
    );
    const values = [
      ...document.querySelectorAll("#factura-garantia-original option"),
    ].map((o) => o.value);
    expect(values).toEqual(["", "9", "8"]);
    expect(clients.getClientDetail).toHaveBeenCalledWith(4);

    await chooseFromCombobox("factura-insumo-cliente", "ana", "Ana");
    await vi.waitFor(() =>
      expect(
        [...document.querySelectorAll("#factura-insumo-servicio option")].map(
          (o) => o.value,
        ),
      ).toEqual(["", "8"]),
    );
  });
});

const PENDING_SALE = {
  id: 1,
  number: "V-00001",
  invoiceType: "V",
  status: "pending",
  contactId: 4,
  contactFullName: "Ana Pérez",
  date: "2026-02-10",
  notes: "Entrega en tienda",
  totalCents: 2000,
  lines: [
    {
      position: 1,
      productIdOriginal: 7,
      productName: "Cable UTP",
      unitAbbreviation: "m",
      quantity: "2",
      unitPriceCents: 1000,
      subtotalCents: 2000,
      productStatus: "active",
    },
    {
      position: 2,
      productIdOriginal: 8,
      productName: "Canaleta",
      unitAbbreviation: "u",
      quantity: "1",
      unitPriceCents: 300,
      subtotalCents: 300,
      productStatus: null,
    },
    {
      position: 3,
      productIdOriginal: 9,
      productName: "Router",
      unitAbbreviation: "u",
      quantity: "3",
      unitPriceCents: 500,
      subtotalCents: 1500,
      productStatus: "discontinued",
    },
  ],
};

describe("editar facturas (§7.3)", () => {
  it("Editar abre el formulario cargado y guardar llama a update, no a create", async () => {
    const warning = "No se pudo limpiar 1 producto vencido";
    invoices.getInvoice.mockResolvedValue(PENDING_SALE);
    invoices.updateSaleInvoice.mockResolvedValue({
      invoiceId: 1,
      number: "V-00001",
      expiredCleanupWarning: warning,
    });
    await view.__testHooks.openSaleDetail(1);
    document.getElementById("factura-venta-editar").click();

    expect(
      document.getElementById("factura-venta-form-title").textContent,
    ).toBe("Editar factura V-00001");
    expect(document.getElementById("factura-venta-cliente").value).toBe(
      "Ana Pérez",
    );
    expect(document.getElementById("factura-venta-fecha-emision").value).toBe(
      "2026-02-10",
    );
    expect(document.getElementById("factura-venta-descripcion").value).toBe(
      "Entrega en tienda",
    );
    const rows = document.querySelectorAll(
      "#factura-venta-lineas-body .li-row",
    );
    expect(rows).toHaveLength(3);
    // Precio emitido en las existentes; el eliminado no se puede cambiar.
    expect(rows[0].textContent).toContain("10,00 USD");
    expect(rows[1].textContent).toContain("(eliminado)");
    expect(rows[1].querySelector("input").disabled).toBe(true);
    expect(rows[2].textContent).toContain("(descontinuado)");

    // Una línea nueva toma el precio vigente (FS-01).
    history.searchProductsForInvoice.mockResolvedValue([
      { ...CABLE, id: 11, name: "Switch", sellingPriceCents: 550 },
    ]);
    await chooseFromCombobox("factura-venta-buscar-producto", "sw", "Switch");
    expect(
      document.querySelectorAll("#factura-venta-lineas-body .li-row")[3]
        .textContent,
    ).toContain("5,50 USD");

    // Un descontinuado solo se reduce.
    const discontinued = lineInputs("factura-venta-lineas-body", "quantity")[2];
    discontinued.value = "5";
    discontinued.dispatchEvent(new Event("input", { bubbles: true }));
    submitForm("factura-venta-form");
    expect(feedback(TYPES[1])).toContain("solo puedes reducir");
    discontinued.value = "1";
    discontinued.dispatchEvent(new Event("input", { bubbles: true }));
    submitForm("factura-venta-form");

    await vi.waitFor(() =>
      expect(invoices.updateSaleInvoice).toHaveBeenCalledTimes(1),
    );
    expect(invoices.createSaleInvoice).not.toHaveBeenCalled();
    expect(invoices.updateSaleInvoice).toHaveBeenCalledWith({
      invoiceId: 1,
      clientId: 4,
      invoiceDate: "2026-02-10",
      notes: "Entrega en tienda",
      lines: [
        { productIdOriginal: 7, productId: 7, quantity: "2" },
        { productIdOriginal: 8, quantity: "1" },
        { productIdOriginal: 9, productId: 9, quantity: "1" },
        { productId: 11, quantity: "1" },
      ],
    });
    // §5 bis: el aviso de la purga tras editar.
    await vi.waitFor(() => expect(toastText()).toContain(warning));
  });

  it("la entrada edita el costo y la garantía conserva su factura asociada", async () => {
    invoices.getInvoice.mockResolvedValueOnce({
      ...PENDING_SALE,
      number: "E-00004",
      invoiceType: "E",
      lines: [PENDING_SALE.lines[0]],
    });
    invoices.updateEntryInvoice.mockResolvedValue({ invoiceId: 1 });
    await view.__testHooks.openEntryDetail(1);
    document.getElementById("factura-entrada-editar").click();
    expect(lineInputs("factura-lineas-body", "unitPrice")[0].value).toBe(
      "10,00",
    );
    submitForm("factura-entrada-form");
    await vi.waitFor(() =>
      expect(invoices.updateEntryInvoice).toHaveBeenCalledWith(
        expect.objectContaining({
          invoiceId: 1,
          supplierId: 4,
          lines: [
            {
              productIdOriginal: 7,
              productId: 7,
              quantity: "2",
              unitPrice: "10,00",
            },
          ],
        }),
      ),
    );

    invoices.getInvoice.mockResolvedValueOnce({
      ...PENDING_SALE,
      number: "GG-00002",
      invoiceType: "GG",
      associatedInvoiceId: 3,
      lines: [PENDING_SALE.lines[0]],
    });
    invoices.updateGuaranteeInvoice.mockResolvedValue({ invoiceId: 1 });
    await view.__testHooks.openGuaranteeDetail(1);
    document.getElementById("factura-garantia-editar").click();
    // La asociada cancelada sigue disponible porque es la de la factura.
    await vi.waitFor(() =>
      expect(document.getElementById("factura-garantia-original").value).toBe(
        "3",
      ),
    );
    submitForm("factura-garantia-form");
    await vi.waitFor(() =>
      expect(invoices.updateGuaranteeInvoice).toHaveBeenCalledWith(
        expect.objectContaining({ invoiceId: 1, associatedInvoiceId: 3 }),
      ),
    );
  });

  it("una factura no pendiente no muestra Editar", async () => {
    invoices.getInvoice.mockResolvedValue({
      ...PENDING_SALE,
      status: "completed",
    });
    await view.__testHooks.openSaleDetail(1);
    expect(document.getElementById("factura-venta-editar").hidden).toBe(true);
  });
});

describe("cambiar el estado desde el detalle (§7.3)", () => {
  const statusButtons = (t) => [
    ...document.querySelectorAll(`#${t.view}-estado-acciones button`),
  ];

  it.each(TYPES)("$name: ofrece solo las transiciones válidas", async (t) => {
    invoices.getInvoice.mockResolvedValue({
      ...PENDING_SALE,
      invoiceType: t.type,
    });
    await openDetail(t, 1);
    expect(statusButtons(t).map((b) => b.textContent)).toEqual([
      `Marcar como ${t.statusLabel}`,
      "Cancelar factura",
    ]);

    invoices.getInvoice.mockResolvedValue({
      ...PENDING_SALE,
      invoiceType: t.type,
      status: "completed",
    });
    await openDetail(t, 1);
    const withPayment = ["E", "V", "S"].includes(t.type);
    expect(statusButtons(t).map((b) => b.dataset.statusTarget)).toEqual(
      withPayment ? ["pending", "cancelled"] : ["cancelled"],
    );

    invoices.getInvoice.mockResolvedValue({
      ...PENDING_SALE,
      invoiceType: t.type,
      status: "cancelled",
    });
    await openDetail(t, 1);
    expect(statusButtons(t)).toHaveLength(0);
  });

  it("completar pide el método de pago, cancelar confirma y se repinta el detalle", async () => {
    const warning = "No se pudo limpiar 1 producto vencido";
    invoices.getInvoice.mockResolvedValue(PENDING_SALE);
    invoices.changeInvoiceStatus.mockResolvedValue({
      invoiceId: 1,
      expiredCleanupWarning: warning,
    });
    await view.__testHooks.openSaleDetail(1);
    document.getElementById("factura-venta-estado-metodo").value = "zelle";
    statusButtons(TYPES[1])
      .find((b) => b.dataset.statusTarget === "completed")
      .click();
    await vi.waitFor(() =>
      expect(invoices.changeInvoiceStatus).toHaveBeenCalledWith(
        1,
        "completed",
        "zelle",
      ),
    );
    await vi.waitFor(() => expect(toastText()).toContain(warning));
    await vi.waitFor(() =>
      expect(invoices.getInvoice).toHaveBeenCalledTimes(2),
    );
    expect(invoices.listInvoices).toHaveBeenCalledWith(
      expect.objectContaining({ invoiceType: "V" }),
    );

    confirmDialog.mockResolvedValueOnce(false);
    statusButtons(TYPES[1])
      .find((b) => b.dataset.statusTarget === "cancelled")
      .click();
    await vi.waitFor(() => expect(confirmDialog).toHaveBeenCalled());
    expect(invoices.changeInvoiceStatus).toHaveBeenCalledTimes(1);

    statusButtons(TYPES[1])
      .find((b) => b.dataset.statusTarget === "cancelled")
      .click();
    await vi.waitFor(() =>
      expect(invoices.changeInvoiceStatus).toHaveBeenLastCalledWith(
        1,
        "cancelled",
        undefined,
      ),
    );
  });

  it("un error del backend se muestra en el detalle", async () => {
    invoices.getInvoice.mockResolvedValue(PENDING_SALE);
    invoices.changeInvoiceStatus.mockRejectedValue({
      code: "invalidState",
      message: "La transición de estado no es válida para este tipo de factura",
    });
    await view.__testHooks.openSaleDetail(1);
    statusButtons(TYPES[1])[0].click();
    await vi.waitFor(() =>
      expect(toastText()).toContain(
        "La transición de estado no es válida para este tipo de factura",
      ),
    );
  });
});

describe("alta rápida de contacto (§7.4)", () => {
  const dialog = () => document.querySelector('[role="dialog"]');
  const dialogInput = (name) => dialog().querySelector(`[name="${name}"]`);

  it("sin coincidencias ofrece crear el cliente y lo deja seleccionado", async () => {
    clients.searchClients.mockResolvedValue([]);
    clients.createClient.mockResolvedValue({
      id: 12,
      fullName: "Zoe Ruiz",
      company: null,
      phone: "+5355551111",
    });
    await chooseFromCombobox("factura-venta-buscar-producto", "cable", "Cable");
    await chooseFromCombobox(
      "factura-venta-cliente",
      "Zoe Ruiz",
      "Crear nuevo cliente «Zoe Ruiz»",
    );
    await vi.waitFor(() => expect(dialog()).not.toBeNull());
    expect(dialogInput("firstName").value).toBe("Zoe");
    expect(dialogInput("lastName").value).toBe("Ruiz");
    dialogInput("nationalId").value = "12345678901";
    dialogInput("phone").value = "+5355551111";
    dialog().querySelector('button[type="submit"]').click();

    await vi.waitFor(() =>
      expect(clients.createClient).toHaveBeenCalledWith(
        expect.objectContaining({
          firstName: "Zoe",
          lastName: "Ruiz",
          nationalId: "12345678901",
          phone: "+5355551111",
        }),
      ),
    );
    await vi.waitFor(() => expect(dialog()).toBeNull());
    expect(document.getElementById("factura-venta-cliente").value).toBe(
      "Zoe Ruiz",
    );
    // Lo tecleado en la factura se conserva.
    expect(
      document.querySelectorAll("#factura-venta-lineas-body .li-row"),
    ).toHaveLength(1);
  });

  it("un CI duplicado muestra el error y cancelar vuelve sin cambios", async () => {
    suppliers.createSupplier.mockRejectedValue({
      code: "duplicateField",
      message: "El carnet de identidad ya está registrado",
    });
    document.getElementById("factura-proveedor-add").click();
    await vi.waitFor(() => expect(dialog()).not.toBeNull());
    expect(dialogInput("invoiceInfo")).toBeNull();
    for (const [name, value] of [
      ["firstName", "Luis"],
      ["lastName", "Gómez"],
      ["nationalId", "12345678901"],
      ["phone", "+5355552222"],
    ]) {
      dialogInput(name).value = value;
    }
    dialog().querySelector('button[type="submit"]').click();
    await vi.waitFor(() =>
      expect(toastText()).toContain(
        "El carnet de identidad ya está registrado",
      ),
    );

    [...dialog().querySelectorAll("button")]
      .find((b) => b.textContent === "Cancelar")
      .click();
    expect(dialog()).toBeNull();
    expect(document.getElementById("factura-proveedor").value).toBe("");
  });
});

describe("filtros de los listados (§7.7)", () => {
  it.each([
    ["factura-venta", "V", true],
    ["factura-entrada", "E", true],
    ["factura-garantia", "GG", true],
    ["factura-perdidas", "P", false],
  ])(
    "%s envía estado y rango de fechas a listInvoices",
    async (prefix, type, withStatus) => {
      if (withStatus) setValue(`${prefix}-filtro-estado`, "pending", "change");
      setValue(`${prefix}-desde`, "2026-01-01", "change");
      setValue(`${prefix}-hasta`, "2026-01-31", "change");
      await vi.waitFor(() =>
        expect(invoices.listInvoices).toHaveBeenLastCalledWith({
          invoiceType: type,
          ...(withStatus ? { status: "pending" } : {}),
          fromDate: "2026-01-01",
          toDate: "2026-01-31",
        }),
      );
      expect(document.getElementById(`${prefix}-filtro-estado`) != null).toBe(
        withStatus,
      );
    },
  );

  it("la lista general envía el rango y un rango invertido muestra el mensaje del backend", async () => {
    invoices.listInvoices.mockRejectedValueOnce({
      code: "validationFailed",
      message: "La fecha inicial no puede ser posterior a la final",
    });
    setValue("factura-general-desde", "2026-02-01", "change");
    await vi.waitFor(() =>
      expect(invoices.listInvoices).toHaveBeenLastCalledWith({
        fromDate: "2026-02-01",
      }),
    );
    await vi.waitFor(() =>
      expect(document.getElementById("facturas-general-body").textContent).toBe(
        "La fecha inicial no puede ser posterior a la final",
      ),
    );
  });

  it("los filtros tienen etiquetas visibles", () => {
    for (const id of [
      "factura-venta-filtro-estado",
      "factura-venta-desde",
      "factura-perdidas-hasta",
      "factura-general-desde",
    ]) {
      expect(
        document.querySelector(`label[for="${id}"]`)?.textContent,
      ).toBeTruthy();
    }
  });
});

describe("avisos al eliminar una compra (§7.8, FE-05)", () => {
  it("detalla por producto las unidades que se pierden y las ya consumidas", async () => {
    invoices.listInvoices.mockResolvedValue([
      { id: 6, number: "E-00006", invoiceType: "E", status: "pending" },
    ]);
    invoices.getInvoice.mockResolvedValue({
      id: 6,
      number: "E-00006",
      invoiceType: "E",
      status: "pending",
      lines: [
        {
          productName: "Cable UTP",
          unitAbbreviation: "m",
          quantity: "10",
          remainingQuantity: "8",
          consumedQuantity: "2",
        },
        {
          productName: "Conector",
          unitAbbreviation: "u",
          quantity: "5",
          remainingQuantity: "0",
          consumedQuantity: "5",
        },
      ],
    });
    confirmDialog.mockResolvedValueOnce(false);
    await view.initEntryInvoicesView();
    [...document.querySelectorAll("#facturas-entrada-body button")]
      .find((b) => b.textContent === "Eliminar")
      .click();
    await vi.waitFor(() => expect(confirmDialog).toHaveBeenCalled());
    const { message } = confirmDialog.mock.calls[0][0];
    expect(message).toContain(
      "Cable UTP: se perderán 8 m; ya se consumieron 2 m en salidas.",
    );
    expect(message).not.toContain("Conector");
    expect(invoices.deleteInvoice).not.toHaveBeenCalled();
  });
});

describe("huecos de la revisión contra spec-03", () => {
  const dialog = () => document.querySelector('[role="dialog"]');

  it("FE-07: el PDF se elige con el diálogo nativo y se adjunta con su ruta", async () => {
    const path = "C:\\Compras\\factura-12.pdf";
    const open = vi.fn(() => Promise.resolve(path));
    window.__TAURI__ = { dialog: { open } };
    invoices.createEntryInvoice.mockResolvedValue({ invoiceId: 31 });
    try {
      document.getElementById("factura-dropzone").click();
      await vi.waitFor(() =>
        expect(
          document.getElementById("factura-archivo-nombre").textContent,
        ).toBe("factura-12.pdf"),
      );
      expect(open).toHaveBeenCalledWith(
        expect.objectContaining({
          filters: [{ name: "Documento PDF", extensions: ["pdf"] }],
        }),
      );
      await chooseFromCombobox("factura-proveedor", "ana", "Ana Pérez");
      await chooseFromCombobox("factura-buscar-producto", "cable", "Cable UTP");
      const cost = lineInputs("factura-lineas-body", "unitPrice")[0];
      cost.value = "1";
      cost.dispatchEvent(new Event("input", { bubbles: true }));
      submitForm("factura-entrada-form");
      await vi.waitFor(() =>
        expect(invoices.saveInvoiceAttachment).toHaveBeenCalledWith(31, path),
      );
    } finally {
      delete window.__TAURI__;
    }
  });

  it("FE-07: el arrastre de Tauri entrega la ruta y rechaza lo que no es PDF", async () => {
    let onDrop = null;
    window.__TAURI__ = {
      event: {
        listen: vi.fn((name, handler) => {
          if (name === "tauri://drag-drop") onDrop = handler;
          return Promise.resolve(() => {});
        }),
      },
    };
    delete window.__facturasDropBound;
    try {
      await view.initEntryInvoicesView();
      document.getElementById("tab-factura-entrada-create").click();
      onDrop({ payload: { paths: ["C:\\Fotos\\recibo.png"] } });
      expect(feedback(TYPES[0])).toContain(
        "Solo se puede adjuntar un archivo PDF.",
      );
      onDrop({ payload: { paths: ["C:\\Compras\\recibo.pdf"] } });
      expect(
        document.getElementById("factura-archivo-nombre").textContent,
      ).toBe("recibo.pdf");
    } finally {
      delete window.__TAURI__;
      delete window.__facturasDropBound;
    }
  });

  it("FE-01: un producto que no existe se crea al guardar la compra, con el costo de su línea", async () => {
    history.searchProductsForInvoice.mockResolvedValue([]);
    invoices.createEntryInvoice.mockResolvedValue({ invoiceId: 32 });
    await chooseFromCombobox("factura-proveedor", "ana", "Ana Pérez");
    await chooseFromCombobox(
      "factura-buscar-producto",
      "Router",
      "Producto no encontrado · Crear producto «Router»",
    );
    await vi.waitFor(() => expect(dialog()).not.toBeNull());
    expect(dialog().querySelector('[name="name"]').value).toBe("Router");
    dialog().querySelector('[name="unitId"]').value = "3";
    dialog().querySelector('[name="categoryId"]').value = "2";
    dialog().querySelector('[name="profitPercent"]').value = "20";
    dialog().querySelector('button[type="submit"]').click();
    await vi.waitFor(() => expect(dialog()).toBeNull());

    const row = document.querySelector("#factura-lineas-body .li-row");
    expect(row.textContent).toContain("Router (nuevo)");
    const [quantity] = lineInputs("factura-lineas-body", "quantity");
    const [cost] = lineInputs("factura-lineas-body", "unitPrice");
    quantity.value = "2";
    quantity.dispatchEvent(new Event("input", { bubbles: true }));
    cost.value = "5,00";
    cost.dispatchEvent(new Event("input", { bubbles: true }));
    submitForm("factura-entrada-form");
    await vi.waitFor(() =>
      expect(invoices.createEntryInvoice).toHaveBeenCalledTimes(1),
    );
    expect(invoices.createEntryInvoice.mock.calls[0][0].lines).toEqual([
      {
        newProduct: {
          name: "Router",
          categoryId: 2,
          unitId: 3,
          profitPercent: "20",
          cost: "5,00",
        },
        quantity: "2",
        unitPrice: "5,00",
      },
    ]);
  });

  it("FE-01: al editar una compra no se ofrece crear productos", async () => {
    invoices.getInvoice.mockResolvedValue({
      ...PENDING_SALE,
      invoiceType: "E",
      lines: [PENDING_SALE.lines[0]],
    });
    history.searchProductsForInvoice.mockResolvedValue([]);
    await view.__testHooks.openEntryDetail(1);
    document.getElementById("factura-entrada-editar").click();
    await chooseFromCombobox(
      "factura-buscar-producto",
      "Router",
      "Crear producto",
    );
    await vi.waitFor(() =>
      expect(feedback(TYPES[0])).toContain("solo se usan productos existentes"),
    );
    expect(dialog()).toBeNull();
  });

  it("FE-01, FS-01, FP-01: la fecha de emisión es hoy por defecto", () => {
    const today = view.todayLocal();
    expect(today).toMatch(/^\d{4}-\d{2}-\d{2}$/);
    for (const t of TYPES) {
      expect(document.getElementById(`${t.field}-fecha-emision`).value).toBe(
        today,
      );
    }
    expect(document.getElementById("factura-perdida-fecha").value).toBe(today);
  });

  it("FE-03, FS-01, FP-01: el detalle muestra la fecha de creación y el responsable", async () => {
    invoices.getInvoice.mockResolvedValue({
      ...PENDING_SALE,
      createdAt: "2026-02-10T15:30:00Z",
      createdByName: "contadora",
    });
    await view.__testHooks.openSaleDetail(1);
    expect(
      document.getElementById("factura-venta-ver-registro").textContent,
    ).toMatch(/^Registrada el \d{2}\/\d{2}\/2026 \d{2}:\d{2} por contadora$/);
    await view.__testHooks.openLossDetail(1);
    expect(
      document.getElementById("factura-perdidas-ver-registro").textContent,
    ).toContain("por contadora");
  });

  it("FE-02: el listado de entradas muestra la cantidad de productos", async () => {
    invoices.listInvoices.mockResolvedValue([
      {
        id: 4,
        number: "E-00004",
        invoiceType: "E",
        contactName: "Ana",
        date: "2026-02-10",
        totalCents: 100,
        status: "pending",
        productCount: 3,
      },
    ]);
    await view.initEntryInvoicesView();
    const headers = [
      ...document.querySelectorAll("#factura-entrada-list thead th"),
    ].map((th) => th.textContent.trim());
    const cells = [
      ...document.querySelectorAll("#facturas-entrada-body tr td"),
    ].map((td) => td.textContent);
    expect(headers).toContain("Productos");
    expect(cells[headers.indexOf("Productos")]).toBe("3");
  });

  it("el encabezado de fecha alterna el orden de los listados", async () => {
    invoices.listInvoices.mockResolvedValue([
      { id: 2, number: "V-00002", invoiceType: "V", date: "2026-02-10" },
      { id: 1, number: "V-00001", invoiceType: "V", date: "2026-01-05" },
    ]);
    await view.initGeneralInvoicesView();
    const numbers = () =>
      [...document.querySelectorAll("#facturas-general-body tr")].map(
        (row) => row.cells[0].textContent,
      );
    expect(numbers()).toEqual(["V-00002", "V-00001"]);
    document.getElementById("th-fecha-facturas-general").click();
    expect(numbers()).toEqual(["V-00001", "V-00002"]);
    document.getElementById("th-fecha-facturas-general").click();
    expect(numbers()).toEqual(["V-00002", "V-00001"]);
  });

  it("CL-06 y PV-06: sin coincidencias dice que el contacto no se encontró", async () => {
    clients.searchClients.mockResolvedValue([]);
    suppliers.searchSuppliers.mockResolvedValue([]);
    const [clientOption] = await typeInCombobox("factura-venta-cliente", "Zoe");
    expect(clientOption.textContent).toBe(
      "Cliente no encontrado · Crear nuevo cliente «Zoe»",
    );
    const [supplierOption] = await typeInCombobox("factura-proveedor", "Luis");
    expect(supplierOption.textContent).toBe(
      "Proveedor no encontrado · Crear nuevo proveedor «Luis»",
    );
  });
});

describe.each(TYPES.filter((t) => t.type === "V" || t.type === "S"))(
  "reservas en $name (spec-04 RS-01, RS-03)",
  (t) => {
    const reserveField = () =>
      document.getElementById(`${t.field}-reserva-field`);

    it("'Reservar hasta' solo se ve Por Cobrar y se envía al crear", async () => {
      invoices[t.create].mockResolvedValue({
        invoiceId: 40,
        number: `${t.type}-00040`,
        expiredCleanupWarning: null,
      });
      expect(reserveField().hidden).toBe(false);
      setValue(`${t.field}-estado`, "completed", "change");
      expect(reserveField().hidden).toBe(true);
      setValue(`${t.field}-estado`, "pending", "change");
      expect(reserveField().hidden).toBe(false);

      await chooseContact(t);
      await chooseFromCombobox(t.productInput, "cable", "Cable UTP");
      setValue(`${t.field}-fecha-emision`, "2026-02-10");
      setValue(`${t.field}-reservar-hasta`, "2026-02-15");
      submitForm(`${t.view}-form`);
      await vi.waitFor(() => expect(invoices[t.create]).toHaveBeenCalled());
      expect(invoices[t.create].mock.calls[0][0]).toMatchObject({
        status: "pending",
        reservedUntil: "2026-02-15",
      });
    });

    it("una factura Cobrada no envía la reserva aunque el campo tenga fecha", async () => {
      invoices[t.create].mockResolvedValue({
        invoiceId: 41,
        number: `${t.type}-00041`,
        expiredCleanupWarning: null,
      });
      await chooseContact(t);
      await chooseFromCombobox(t.productInput, "cable", "Cable UTP");
      setValue(`${t.field}-fecha-emision`, "2026-02-10");
      setValue(`${t.field}-reservar-hasta`, "2026-02-15");
      setValue(`${t.field}-estado`, "completed", "change");
      submitForm(`${t.view}-form`);
      await vi.waitFor(() => expect(invoices[t.create]).toHaveBeenCalled());
      expect(invoices[t.create].mock.calls[0][0]).not.toHaveProperty(
        "reservedUntil",
      );
    });

    it("el filtro 'Reservas activas' pide status=reserved y la fecha límite sale en el listado y el detalle", async () => {
      expect(
        document.querySelector(
          `#${t.view}-filtro-estado option[value="reserved"]`,
        ).textContent,
      ).toBe("Reservas activas");
      invoices.listInvoices.mockResolvedValue([
        {
          id: 12,
          number: `${t.type}-00012`,
          invoiceType: t.type,
          contactName: "Ana Pérez",
          date: "2026-02-10",
          totalCents: 500,
          status: "pending",
          productCount: 1,
          reservedUntil: "2026-02-15",
        },
      ]);
      setValue(`${t.view}-filtro-estado`, "reserved", "change");
      await vi.waitFor(() =>
        expect(invoices.listInvoices).toHaveBeenLastCalledWith(
          expect.objectContaining({ invoiceType: t.type, status: "reserved" }),
        ),
      );
      const body =
        t.type === "V" ? "facturas-venta-body" : "facturas-servicios-body";
      await vi.waitFor(() =>
        expect(document.getElementById(body).textContent).toContain(
          "15/02/2026",
        ),
      );

      invoices.getInvoice.mockResolvedValue({
        ...PENDING_SALE,
        invoiceType: t.type,
        reservedUntil: "2026-02-15",
      });
      await openDetail(t, 12);
      expect(
        document.getElementById(`${t.field}-ver-reserva-card`).hidden,
      ).toBe(false);
      expect(
        document.getElementById(`${t.field}-ver-reserva`).textContent,
      ).toBe("15/02/2026");
      invoices.getInvoice.mockResolvedValue({
        ...PENDING_SALE,
        invoiceType: t.type,
      });
      await openDetail(t, 12);
      expect(
        document.getElementById(`${t.field}-ver-reserva-card`).hidden,
      ).toBe(true);
    });
  },
);

describe("reserva al editar (spec-04 RS-01)", () => {
  it("carga la fecha, envía la nueva y quitarla la omite (venta normal)", async () => {
    invoices.getInvoice.mockResolvedValue({
      ...PENDING_SALE,
      reservedUntil: "2026-02-15",
    });
    invoices.updateSaleInvoice.mockResolvedValue({
      invoiceId: 1,
      number: "V-00001",
      expiredCleanupWarning: null,
    });
    await view.__testHooks.openSaleDetail(1);
    document.getElementById("factura-venta-editar").click();
    expect(document.getElementById("factura-venta-reserva-field").hidden).toBe(
      false,
    );
    expect(document.getElementById("factura-venta-reservar-hasta").value).toBe(
      "2026-02-15",
    );

    setValue("factura-venta-reservar-hasta", "2026-02-20");
    submitForm("factura-venta-form");
    await vi.waitFor(() =>
      expect(invoices.updateSaleInvoice).toHaveBeenCalledTimes(1),
    );
    expect(invoices.updateSaleInvoice.mock.calls[0][0].reservedUntil).toBe(
      "2026-02-20",
    );
    // Tras guardar, la vista recarga el detalle: se espera a que termine.
    await vi.waitFor(() =>
      expect(invoices.getInvoice).toHaveBeenCalledTimes(2),
    );
    await vi.waitFor(() =>
      expect(
        document
          .getElementById("factura-venta-ver")
          .classList.contains("active"),
      ).toBe(true),
    );

    document.getElementById("factura-venta-editar").click();
    setValue("factura-venta-reservar-hasta", "");
    submitForm("factura-venta-form");
    await vi.waitFor(() =>
      expect(invoices.updateSaleInvoice).toHaveBeenCalledTimes(2),
    );
    expect(invoices.updateSaleInvoice.mock.calls[1][0]).not.toHaveProperty(
      "reservedUntil",
    );
  });

  it("garantía e insumos no tienen el campo", () => {
    expect(
      document.getElementById("factura-garantia-reserva-field"),
    ).toBeNull();
    expect(document.getElementById("factura-insumo-reserva-field")).toBeNull();
  });
});

describe("imprimir y exportar a PDF (spec-04 IM-01, IM-02)", () => {
  const DETAILS = [
    ["E", "factura-entrada", (id) => view.__testHooks.openEntryDetail(id)],
    ["V", "factura-venta", (id) => view.__testHooks.openSaleDetail(id)],
    ["S", "factura-servicio", (id) => view.__testHooks.openServiceDetail(id)],
    [
      "GG",
      "factura-garantia",
      (id) => view.__testHooks.openGuaranteeDetail(id),
    ],
    [
      "GI",
      "factura-insumo",
      (id) => view.__testHooks.openInstallationDetail(id),
    ],
    ["P", "factura-perdida", (id) => view.openLossDetail(id)],
  ];
  const MODEL = {
    title: "Factura de Venta",
    number: "V-00001",
    date: "10/02/2026",
    company: {
      name: "Goyma Conexiones",
      address: "Calle 23",
      phone: "",
      taxId: "",
      logoDataUrl: null,
    },
    contact: {
      label: "Cliente",
      name: "Ana Pérez",
      company: null,
      nationalId: "12345678901",
      phone: "+5355552222",
      email: null,
    },
    priceLabel: "Precio unitario",
    lines: [
      {
        name: "Cable UTP",
        quantity: "2",
        unit: "m",
        unitPriceCents: 1000,
        subtotalCents: 2000,
      },
    ],
    totalCents: 2000,
    notesLabel: "Descripción",
    notes: "Entrega en tienda",
    associated: null,
    reservedUntil: "15/02/2026",
  };

  afterEach(() => {
    delete window.__TAURI__;
    document.getElementById("print-area")?.remove();
  });

  it.each(DETAILS)(
    "%s: 'Imprimir' abre la vista previa sin estado y desde ella el diálogo del sistema",
    async (type, prefix, open) => {
      invoices.getInvoice.mockResolvedValue({
        ...PENDING_SALE,
        invoiceType: type,
      });
      invoices.getInvoicePrintModel.mockResolvedValue(MODEL);
      const print = vi.spyOn(window, "print").mockImplementation(() => {});
      await open(1);
      document.getElementById(`${prefix}-imprimir`).click();
      await vi.waitFor(() =>
        expect(document.getElementById("print-area")?.hidden).toBe(false),
      );
      expect(invoices.getInvoicePrintModel).toHaveBeenCalledWith(1);
      const sheet = document.querySelector("#print-area .print-sheet");
      expect(sheet.textContent).toContain("Factura de Venta");
      expect(sheet.textContent).toContain("Cliente: Ana Pérez");
      expect(sheet.textContent).toContain("Reservada hasta: 15/02/2026");
      expect(sheet.textContent).toContain("Total: 20,00 USD");
      expect(sheet.textContent).not.toMatch(/Por Cobrar|Cobrada|Pendiente/);

      [...document.querySelectorAll("#print-area .print-toolbar button")]
        .find((button) => button.textContent === "Imprimir")
        .click();
      expect(print).toHaveBeenCalledTimes(1);
      document.dispatchEvent(new KeyboardEvent("keydown", { key: "Escape" }));
      expect(document.getElementById("print-area").hidden).toBe(true);
      print.mockRestore();
    },
  );

  it.each(DETAILS)(
    "%s: 'Exportar a PDF' pide la ruta y llama al backend; cancelar no hace nada",
    async (type, prefix, open) => {
      invoices.getInvoice.mockResolvedValue({
        ...PENDING_SALE,
        invoiceType: type,
      });
      invoices.exportInvoicePdf.mockResolvedValue(null);
      const save = vi
        .fn()
        .mockResolvedValueOnce(null)
        .mockResolvedValueOnce("C:/facturas/V-00001.pdf");
      window.__TAURI__ = { dialog: { save } };
      await open(1);

      document.getElementById(`${prefix}-exportar`).click();
      await vi.waitFor(() => expect(save).toHaveBeenCalledTimes(1));
      expect(save).toHaveBeenCalledWith(
        expect.objectContaining({
          defaultPath: "V-00001.pdf",
          filters: [expect.objectContaining({ extensions: ["pdf"] })],
        }),
      );
      await Promise.resolve();
      expect(invoices.exportInvoicePdf).not.toHaveBeenCalled();

      document.getElementById(`${prefix}-exportar`).click();
      await vi.waitFor(() =>
        expect(invoices.exportInvoicePdf).toHaveBeenCalledWith(
          1,
          "C:/facturas/V-00001.pdf",
        ),
      );
      await vi.waitFor(() =>
        expect(toastText()).toContain("Factura exportada a PDF."),
      );
    },
  );

  it("un fallo al escribir el PDF sale como toast de error", async () => {
    invoices.getInvoice.mockResolvedValue(PENDING_SALE);
    invoices.exportInvoicePdf.mockRejectedValue({
      code: "exportFailed",
      message: "No se pudo guardar el archivo",
    });
    window.__TAURI__ = {
      dialog: { save: vi.fn(() => Promise.resolve("C:/x/V-1.pdf")) },
    };
    await view.__testHooks.openSaleDetail(1);
    document.getElementById("factura-venta-exportar").click();
    await vi.waitFor(() =>
      expect(toastText()).toContain("No se pudo guardar el archivo"),
    );
  });
});

const BOX_OPTIONS = [
  { unitId: 3, name: "Metro", abbreviation: "m", factor: "1" },
  { unitId: 9, name: "Caja", abbreviation: "cj", factor: "12" },
];

describe.each(TYPES)("unidad por línea en $name (spec-04 CV-01)", (t) => {
  it("ofrece las unidades del producto, recalcula el total preliminar y envía unitId", async () => {
    products.listUnitOptionsForProduct.mockResolvedValue(BOX_OPTIONS);
    invoices[t.create].mockResolvedValue({
      invoiceId: 50,
      number: `${t.type}-00050`,
      expiredCleanupWarning: null,
    });
    await chooseContact(t);
    await chooseFromCombobox(t.productInput, "cable", "Cable UTP");
    const select = () =>
      document.querySelector(`#${linesBody(t)} select[data-line-field="unit"]`);
    await vi.waitFor(() => expect(select()).not.toBeNull());
    expect(products.listUnitOptionsForProduct).toHaveBeenCalledWith(7);
    expect([...select().options].map((option) => option.textContent)).toEqual([
      "m",
      "cj",
    ]);

    select().value = "9";
    select().dispatchEvent(new Event("change", { bubbles: true }));
    const quantity = lineInputs(linesBody(t), "quantity")[0];
    quantity.value = "2";
    quantity.dispatchEvent(new Event("input", { bubbles: true }));
    if (t.type === "E") {
      const cost = lineInputs(linesBody(t), "unitPrice")[0];
      cost.value = "30,00";
      cost.dispatchEvent(new Event("input", { bubbles: true }));
      expect(document.getElementById(`${t.field}-total`).textContent).toBe(
        "60,00 USD",
      );
    } else if (t.type === "V" || t.type === "S") {
      // Precio vigente 2,50 la unidad × 12 = 30,00 la caja.
      expect(
        document.querySelector(`#${linesBody(t)} .li-costo`).textContent,
      ).toBe("30,00 USD");
      expect(document.getElementById(`${t.field}-total`).textContent).toBe(
        "60,00 USD",
      );
    }

    setValue(`${t.field}-fecha-emision`, "2026-02-10");
    submitForm(`${t.view}-form`);
    await vi.waitFor(() => expect(invoices[t.create]).toHaveBeenCalled());
    expect(invoices[t.create].mock.calls[0][0].lines[0]).toMatchObject({
      productId: 7,
      quantity: "2",
      unitId: 9,
    });
  });
});

describe("unidad por línea: detalle, edición y pérdida (spec-04 CV-01)", () => {
  const ENTERED_SALE = {
    ...PENDING_SALE,
    lines: [
      {
        position: 1,
        productIdOriginal: 7,
        productName: "Cable UTP",
        unitAbbreviation: "m",
        quantity: "24",
        unitPriceCents: 250,
        subtotalCents: 6000,
        productStatus: "active",
        entered: {
          unitId: 9,
          name: "Caja",
          abbreviation: "cj",
          quantity: "2",
          unitPriceCents: 3000,
        },
      },
    ],
    totalCents: 6000,
  };

  it("el detalle muestra lo escrito y editar lo carga y lo reenvía", async () => {
    invoices.getInvoice.mockResolvedValue(ENTERED_SALE);
    invoices.updateSaleInvoice.mockResolvedValue({
      invoiceId: 1,
      number: "V-00001",
      expiredCleanupWarning: null,
    });
    products.listUnitOptionsForProduct.mockResolvedValue(BOX_OPTIONS);
    await view.__testHooks.openSaleDetail(1);
    const cells = [
      ...document.querySelectorAll("#factura-venta-ver-productos .li-row div"),
    ].map((cell) => cell.textContent);
    expect(cells).toEqual([
      "1",
      "Cable UTP",
      "2",
      "cj",
      "30,00 USD",
      "60,00 USD",
    ]);

    document.getElementById("factura-venta-editar").click();
    expect(lineInputs("factura-venta-lineas-body", "quantity")[0].value).toBe(
      "2",
    );
    await vi.waitFor(() =>
      expect(
        document.querySelector(
          '#factura-venta-lineas-body select[data-line-field="unit"]',
        )?.value,
      ).toBe("9"),
    );
    submitForm("factura-venta-form");
    await vi.waitFor(() =>
      expect(invoices.updateSaleInvoice).toHaveBeenCalled(),
    );
    expect(invoices.updateSaleInvoice.mock.calls[0][0].lines[0]).toMatchObject({
      productIdOriginal: 7,
      productId: 7,
      quantity: "2",
      unitId: 9,
    });
  });

  it("la pérdida ofrece la unidad junto a la cantidad y la envía", async () => {
    products.listUnitOptionsForProduct.mockResolvedValue(BOX_OPTIONS);
    invoices.createLossInvoice.mockResolvedValue({
      invoiceId: 60,
      number: "P-00060",
      expiredCleanupWarning: null,
    });
    invoices.getInvoice.mockResolvedValue({
      ...PENDING_SALE,
      invoiceType: "P",
    });
    await chooseFromCombobox("factura-perdida-producto", "cable", "Cable UTP");
    await vi.waitFor(() =>
      expect(
        document.getElementById("factura-perdida-unidad-field").hidden,
      ).toBe(false),
    );
    setValue("factura-perdida-unidad", "9", "change");
    setValue("factura-perdida-cantidad", "0,25");
    setValue("factura-perdida-fecha", "2026-02-10");
    submitForm("factura-perdidas-form");
    await vi.waitFor(() =>
      expect(invoices.createLossInvoice).toHaveBeenCalled(),
    );
    expect(invoices.createLossInvoice.mock.calls[0][0]).toMatchObject({
      productId: 7,
      quantity: "0,25",
      unitId: 9,
    });
  });
});
