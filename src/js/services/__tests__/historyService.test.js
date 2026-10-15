import { afterEach, describe, expect, it, vi } from "vitest";

describe("servicio de historial", () => {
  afterEach(() => {
    delete window.__TAURI__;
    vi.resetModules();
  });

  it("envía los comandos de historial con el contrato Tauri correcto", async () => {
    const invoke = vi.fn().mockResolvedValue({ ok: true });
    window.__TAURI__ = { core: { invoke } };

    const {
      getProductHistory,
      listHistoryProducts,
      exportHistoryExcel,
      exportHistoryPdf,
    } = await import("../historyService.js");

    const filters = {
      productId: 7,
      fromDate: "2026-01-01",
      toDate: "2026-02-01",
      types: ["entry", "saleOut"],
    };
    const exportInput = { ...filters, destinationPath: "/tmp/historial.xlsx" };

    await getProductHistory(filters);
    await listHistoryProducts();
    await exportHistoryExcel(exportInput);
    await exportHistoryPdf(exportInput);

    expect(invoke).toHaveBeenNthCalledWith(1, "get_product_history", {
      filters,
    });
    expect(invoke).toHaveBeenNthCalledWith(
      2,
      "list_history_products",
      undefined,
    );
    expect(invoke).toHaveBeenNthCalledWith(3, "export_history_excel", {
      input: exportInput,
    });
    expect(invoke).toHaveBeenNthCalledWith(4, "export_history_pdf", {
      input: exportInput,
    });
  });
});
