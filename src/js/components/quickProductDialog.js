import { openFormDialog } from "./formDialog.js";
import { listCategories } from "../services/categoriesService.js";
import { listUnits } from "../services/unitsService.js";

// Producto nuevo desde una factura de entrada (FE-01): pide nombre,
// descripción, categoría, unidad y % de ganancia. No crea nada: el producto
// se crea al guardar la factura, con la cantidad y el costo de su línea.
// Devuelve los datos elegidos o null si se cancela.

async function loadItems(load) {
  try {
    const response = await load();
    return Array.isArray(response) ? response : (response?.items ?? []);
  } catch {
    // Sin la lista, el selector solo ofrece "Sin categoría"/"Sin unidad".
    return [];
  }
}

export async function openQuickProductDialog({ initialName = "" }) {
  const [categories, units] = await Promise.all([
    loadItems(listCategories),
    loadItems(listUnits),
  ]);
  const unitAbbreviations = new Map(
    units.map((unit) => [String(unit.id), unit.abbreviation ?? null]),
  );
  return openFormDialog({
    title: "Nuevo producto",
    submitText: "Agregar a la factura",
    missingMessage: "Completa el nombre y el porcentaje de ganancia.",
    fields: [
      {
        name: "name",
        label: "Nombre",
        required: true,
        value: String(initialName).trim(),
      },
      { name: "description", label: "Descripción", type: "textarea" },
      {
        name: "categoryId",
        label: "Categoría",
        type: "select",
        options: [
          ["", "Sin categoría"],
          ...categories.map((item) => [String(item.id), item.name ?? ""]),
        ],
      },
      {
        name: "unitId",
        label: "Unidad",
        type: "select",
        options: [
          ["", "Sin unidad"],
          ...units.map((item) => [
            String(item.id),
            item.abbreviation
              ? `${item.name} (${item.abbreviation})`
              : item.name,
          ]),
        ],
      },
      {
        name: "profitPercent",
        label: "% de ganancia",
        required: true,
        inputMode: "decimal",
      },
    ],
    // El backend valida nombre, descripción y porcentaje al guardar la factura.
    onSubmit: (values) => ({
      name: values.name,
      description: values.description || undefined,
      categoryId: values.categoryId ? Number(values.categoryId) : undefined,
      unitId: values.unitId ? Number(values.unitId) : undefined,
      profitPercent: values.profitPercent,
      unitAbbreviation: unitAbbreviations.get(values.unitId) ?? null,
    }),
  });
}
