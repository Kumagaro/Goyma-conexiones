use serde::{Deserialize, Serialize};

use crate::models::invoice::InvoiceFilters;
use crate::models::product::ProductFilters;

/// Filtros activos del listado que se exporta (CSV-01): `search` en los que
/// solo tienen buscador; `products` e `invoices` llevan los de su listado.
#[derive(Debug, Clone, Default, Deserialize)]
#[serde(default, deny_unknown_fields, rename_all = "camelCase")]
pub struct CsvFilters {
    pub search: Option<String>,
    pub products: Option<ProductFilters>,
    pub invoices: Option<InvoiceFilters>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct SkippedRow {
    pub row: usize,
    pub reason: String,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct ImportSummary {
    pub imported: usize,
    pub skipped: Vec<SkippedRow>,
}
