use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct ProductListItem {
    pub id: i64,
    pub name: String,
    pub category_id: Option<i64>,
    pub category_name: Option<String>,
    pub unit_id: Option<i64>,
    pub unit_name: Option<String>,
    pub unit_abbreviation: Option<String>,
    pub quantity: String,
    pub cost_cents: i64,
    pub valor_total_cents: i64,
    pub selling_price_cents: i64,
    pub profit_percent: String,
    pub expires_on: Option<String>,
    pub is_out_of_stock: bool,
    pub is_low_stock: bool,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct ProductMatch {
    pub id: i64,
    pub name: String,
    pub category_name: Option<String>,
    pub unit_abbreviation: Option<String>,
    pub stock: String,
    /// Precio de venta vigente: solo sirve para el total preliminar del
    /// formulario; el backend recalcula el precio al guardar.
    pub selling_price_cents: i64,
    pub is_out_of_stock: bool,
    pub is_discontinued: bool,
}

/// Unidad que se puede elegir en una línea de factura (CV-01): la del
/// producto (factor 1) o una con conversión de un paso hacia ella.
#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct UnitOption {
    pub unit_id: i64,
    pub name: String,
    pub abbreviation: String,
    /// Unidades del producto que equivalen a 1 de esta unidad.
    pub factor: String,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct ProductAssociatedInvoice {
    pub invoice_id: i64,
    pub invoice_number: String,
    pub invoice_type: String,
    pub invoice_date: String,
    pub status: Option<String>,
    pub quantity: String,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct ProductConditionItem {
    pub id: i64,
    pub name: String,
    pub category_name: Option<String>,
    pub unit_name: Option<String>,
    pub unit_abbreviation: Option<String>,
    pub quantity: String,
    pub is_out_of_stock: bool,
    pub is_discontinued: bool,
    pub remaining_days: Option<i64>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct ProductConditionList {
    pub items: Vec<ProductConditionItem>,
    pub expired_cleanup_warning: Option<String>,
}

#[derive(Debug, Clone, Copy, Default, PartialEq, Eq, Deserialize)]
#[serde(rename_all = "camelCase")]
pub enum ProductConditionFilter {
    #[default]
    All,
    OutOfStock,
    Discontinued,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct UpdateProductResult {
    pub product: ProductListItem,
    pub expired_cleanup_warning: Option<String>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct ProductActionResult {
    pub expired_cleanup_warning: Option<String>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct ProductDetail {
    #[serde(flatten)]
    pub product: ProductListItem,
    pub description: Option<String>,
    pub is_discontinued: bool,
    pub discontinued_at: Option<String>,
    pub remaining_days: Option<i64>,
    pub expired_cleanup_warning: Option<String>,
    /// RS-03: suma de sus líneas en reservas activas.
    pub reserved_quantity: String,
    pub reserved_invoices: Vec<ReservedInvoice>,
}

/// Una reserva activa que incluye el producto (enlace desde su detalle).
#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct ReservedInvoice {
    pub id: i64,
    pub number: String,
    pub invoice_type: String,
    pub reserved_until: String,
    pub quantity: String,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct PriceQuote {
    pub selling_price_cents: i64,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct OutOfStockCategory {
    pub name: String,
    pub count: i64,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct DashboardSummary {
    pub out_of_stock_total: i64,
    pub top_categories: Vec<OutOfStockCategory>,
    pub without_category_count: i64,
}

#[derive(Debug, Clone, Copy, Default, PartialEq, Eq, Deserialize)]
#[serde(rename_all = "camelCase")]
pub enum StockFilter {
    #[default]
    All,
    Available,
    OutOfStock,
    Low,
}

#[derive(Debug, Clone, Copy, Default, PartialEq, Eq, Deserialize)]
#[serde(rename_all = "camelCase")]
pub enum ExpiryFilter {
    #[default]
    All,
    WithExpiry,
    WithoutExpiry,
    Expired,
    ExpiringSoon,
}

#[derive(Debug, Clone, Default, PartialEq, Eq, Deserialize)]
#[serde(deny_unknown_fields, rename_all = "camelCase")]
pub struct ProductFilters {
    #[serde(default)]
    pub search: Option<String>,
    #[serde(default)]
    pub category_id: Option<i64>,
    #[serde(default)]
    pub without_category: bool,
    #[serde(default)]
    pub stock: StockFilter,
    #[serde(default)]
    pub expiry: ExpiryFilter,
}

#[derive(Debug, Clone, PartialEq, Eq, Deserialize)]
#[serde(deny_unknown_fields, rename_all = "camelCase")]
pub struct CreateProductInput {
    pub name: String,
    #[serde(default)]
    pub description: Option<String>,
    #[serde(default)]
    pub category_id: Option<i64>,
    #[serde(default)]
    pub unit_id: Option<i64>,
    pub cost: String,
    pub quantity: String,
    pub profit_percent: String,
    #[serde(default)]
    pub expires_on: Option<String>,
}

#[derive(Debug, Clone, PartialEq, Eq, Deserialize)]
#[serde(deny_unknown_fields, rename_all = "camelCase")]
pub struct UpdateProductInput {
    pub product_id: i64,
    pub name: String,
    #[serde(default)]
    pub description: Option<String>,
    #[serde(default)]
    pub category_id: Option<i64>,
    #[serde(default)]
    pub unit_id: Option<i64>,
    pub profit_percent: String,
    #[serde(default)]
    pub expires_on: Option<String>,
}

#[derive(Debug, Clone, PartialEq, Eq, Deserialize)]
#[serde(deny_unknown_fields, rename_all = "camelCase")]
pub struct PreviewSellingPriceInput {
    pub profit_percent: String,
    #[serde(default)]
    pub product_id: Option<i64>,
    #[serde(default)]
    pub cost: Option<String>,
}

fn format_scaled(value: i64, scale: i128, decimal_places: usize, fixed_decimals: bool) -> String {
    let signed_value = i128::from(value);
    let sign = if signed_value < 0 { "-" } else { "" };
    let absolute_value = signed_value.abs();
    let whole_part = absolute_value / scale;
    let fractional_part = absolute_value % scale;

    if fixed_decimals {
        return format!(
            "{sign}{whole_part},{fractional_part:0width$}",
            width = decimal_places
        );
    }

    let fractional_text = format!("{fractional_part:0decimal_places$}");
    let fractional_text = fractional_text.trim_end_matches('0');
    if fractional_text.is_empty() {
        format!("{sign}{whole_part}")
    } else {
        format!("{sign}{whole_part},{fractional_text}")
    }
}

pub fn format_quantity_milli(quantity_milli: i64) -> String {
    format_scaled(quantity_milli, 1_000, 3, false)
}

pub fn format_quantity_micro(quantity_micro: i64) -> String {
    format_scaled(quantity_micro, 1_000_000, 6, false)
}

pub fn format_factor_micro(factor_micro: i64) -> String {
    format_scaled(factor_micro, 1_000_000, 6, false)
}

pub fn format_profit_percent_hundredths(profit_percent_hundredths: i64) -> String {
    format_scaled(profit_percent_hundredths, 100, 2, true)
}

#[cfg(test)]
mod tests {
    use serde_json::{json, Value};

    use super::{
        DashboardSummary, ExpiryFilter, OutOfStockCategory, PriceQuote, ProductActionResult,
        ProductConditionFilter, ProductConditionItem, ProductConditionList, ProductDetail,
        ProductFilters, ProductListItem, StockFilter, UpdateProductResult,
    };

    #[test]
    fn product_dtos_serialize_camel_case_keys_null_warnings_and_decimal_strings() {
        let item = ProductListItem {
            id: 11,
            name: "Café".to_owned(),
            category_id: None,
            category_name: None,
            unit_id: Some(2),
            unit_name: Some("Kilogramo".to_owned()),
            unit_abbreviation: Some("kg".to_owned()),
            quantity: "1,25".to_owned(),
            cost_cents: 1250,
            valor_total_cents: 1563,
            selling_price_cents: 1500,
            profit_percent: "20,00".to_owned(),
            expires_on: None,
            is_out_of_stock: false,
            is_low_stock: true,
        };
        let item_json = serde_json::to_value(&item).unwrap();
        assert_eq!(
            item_json,
            json!({"id": 11, "name": "Café", "categoryId": null, "categoryName": null, "unitId": 2, "unitName": "Kilogramo", "unitAbbreviation": "kg", "quantity": "1,25", "costCents": 1250, "valorTotalCents": 1563, "sellingPriceCents": 1500, "profitPercent": "20,00", "expiresOn": null, "isOutOfStock": false, "isLowStock": true})
        );
        assert!(item_json["quantity"].is_string());
        assert!(item_json["profitPercent"].is_string());

        let product_list = ProductConditionList {
            items: vec![ProductConditionItem {
                id: 11,
                name: "Café".to_owned(),
                category_name: None,
                unit_name: Some("Kilogramo".to_owned()),
                unit_abbreviation: Some("kg".to_owned()),
                quantity: "0".to_owned(),
                is_out_of_stock: true,
                is_discontinued: false,
                remaining_days: None,
            }],
            expired_cleanup_warning: None,
        };
        assert_eq!(
            serde_json::to_value(product_list).unwrap(),
            json!({"items": [{"id": 11, "name": "Café", "categoryName": null, "unitName": "Kilogramo", "unitAbbreviation": "kg", "quantity": "0", "isOutOfStock": true, "isDiscontinued": false, "remainingDays": null}], "expiredCleanupWarning": null})
        );
        let condition_list_with_warning = ProductConditionList {
            items: Vec::new(),
            expired_cleanup_warning: Some("aviso".to_owned()),
        };
        assert_eq!(
            serde_json::to_value(condition_list_with_warning).unwrap()["expiredCleanupWarning"],
            json!("aviso")
        );

        let update = UpdateProductResult {
            product: item.clone(),
            expired_cleanup_warning: Some("aviso".to_owned()),
        };
        let update_json = serde_json::to_value(update).unwrap();
        assert_eq!(update_json["expiredCleanupWarning"], "aviso");
        assert!(update_json["product"]["quantity"].is_string());
        let update_without_warning = UpdateProductResult {
            product: item.clone(),
            expired_cleanup_warning: None,
        };
        assert_eq!(
            serde_json::to_value(update_without_warning).unwrap()["expiredCleanupWarning"],
            Value::Null
        );

        let action = ProductActionResult {
            expired_cleanup_warning: None,
        };
        assert_eq!(
            serde_json::to_value(action).unwrap(),
            json!({"expiredCleanupWarning": null})
        );
        let action_with_warning = ProductActionResult {
            expired_cleanup_warning: Some("aviso".to_owned()),
        };
        assert_eq!(
            serde_json::to_value(action_with_warning).unwrap()["expiredCleanupWarning"],
            json!("aviso")
        );

        let detail = ProductDetail {
            product: item.clone(),
            description: None,
            is_discontinued: true,
            discontinued_at: Some("2026-10-03T12:00:00Z".to_owned()),
            remaining_days: Some(30),
            expired_cleanup_warning: None,
            reserved_quantity: "0".to_owned(),
            reserved_invoices: vec![],
        };
        let detail_json = serde_json::to_value(detail).unwrap();
        assert_eq!(
            detail_json,
            json!({"id": 11, "name": "Café", "categoryId": null, "categoryName": null, "unitId": 2, "unitName": "Kilogramo", "unitAbbreviation": "kg", "quantity": "1,25", "costCents": 1250, "valorTotalCents": 1563, "sellingPriceCents": 1500, "profitPercent": "20,00", "expiresOn": null, "isOutOfStock": false, "isLowStock": true, "description": null, "isDiscontinued": true, "discontinuedAt": "2026-10-03T12:00:00Z", "remainingDays": 30, "expiredCleanupWarning": null, "reservedQuantity": "0", "reservedInvoices": []})
        );
        assert_eq!(detail_json["isDiscontinued"], true);
        assert_eq!(detail_json["discontinuedAt"], "2026-10-03T12:00:00Z");
        assert_eq!(detail_json["remainingDays"], 30);
        assert_eq!(detail_json["expiredCleanupWarning"], Value::Null);
        let detail_with_warning = ProductDetail {
            product: item,
            description: None,
            is_discontinued: false,
            discontinued_at: None,
            remaining_days: None,
            expired_cleanup_warning: Some("aviso".to_owned()),
            reserved_quantity: "0".to_owned(),
            reserved_invoices: vec![],
        };
        assert_eq!(
            serde_json::to_value(detail_with_warning).unwrap()["expiredCleanupWarning"],
            json!("aviso")
        );

        let price_quote = PriceQuote {
            selling_price_cents: 1500,
        };
        assert_eq!(
            serde_json::to_value(price_quote).unwrap(),
            json!({"sellingPriceCents": 1500})
        );

        let dashboard = DashboardSummary {
            out_of_stock_total: 4,
            top_categories: vec![OutOfStockCategory {
                name: "Café".to_owned(),
                count: 4,
            }],
            without_category_count: 1,
        };
        assert_eq!(
            serde_json::to_value(dashboard).unwrap(),
            json!({"outOfStockTotal": 4, "topCategories": [{"name": "Café", "count": 4}], "withoutCategoryCount": 1})
        );
    }

    #[test]
    fn fixed_point_formatters_use_comma_without_grouping_or_unneeded_zeroes() {
        assert_eq!(super::format_quantity_milli(1_250), "1,25");
        assert_eq!(super::format_quantity_milli(1_001), "1,001");
        assert_eq!(super::format_quantity_milli(1_000), "1");
        assert_eq!(super::format_quantity_micro(1_234_500_000), "1234,5");
        assert_eq!(super::format_factor_micro(1), "0,000001");
        assert_eq!(
            super::format_quantity_micro(999_999_999_999_999),
            "999999999,999999"
        );
        assert_eq!(super::format_profit_percent_hundredths(2_000), "20,00");
        assert_eq!(super::format_profit_percent_hundredths(2_005), "20,05");
        assert_eq!(super::format_profit_percent_hundredths(0), "0,00");
    }

    #[test]
    fn product_input_dtos_reject_unknown_fields_and_filters_apply_defaults() {
        let filters: ProductFilters = serde_json::from_value(json!({"search": "café"})).unwrap();
        assert_eq!(filters.search.as_deref(), Some("café"));
        assert_eq!(filters.category_id, None);
        assert!(!filters.without_category);
        assert_eq!(filters.stock, StockFilter::All);
        assert_eq!(filters.expiry, ExpiryFilter::All);
        assert!(serde_json::from_value::<ProductFilters>(json!({"unexpected": true})).is_err());

        assert_eq!(
            serde_json::from_value::<ProductConditionFilter>(json!("all")).unwrap(),
            ProductConditionFilter::All
        );
        assert_eq!(
            serde_json::from_value::<ProductConditionFilter>(json!("outOfStock")).unwrap(),
            ProductConditionFilter::OutOfStock
        );
        assert_eq!(
            serde_json::from_value::<ProductConditionFilter>(json!("discontinued")).unwrap(),
            ProductConditionFilter::Discontinued
        );
        assert!(serde_json::from_value::<ProductConditionFilter>(json!("unknown")).is_err());

        assert!(serde_json::from_value::<super::CreateProductInput>(json!({
            "name": "Café",
            "cost": "1,00",
            "quantity": "1",
            "profitPercent": "20,00",
            "sellingPrice": "1,20"
        }))
        .is_err());
        let create_input: super::CreateProductInput = serde_json::from_value(json!({
            "name": "Café",
            "cost": "1,00",
            "quantity": "1",
            "profitPercent": "20,00"
        }))
        .unwrap();
        assert_eq!(create_input.cost, "1,00");
        assert_eq!(create_input.quantity, "1");
        assert!(serde_json::from_value::<super::CreateProductInput>(json!({
            "name": "Café",
            "cost": "1,00",
            "profitPercent": "20,00"
        }))
        .is_err());
        assert!(serde_json::from_value::<super::UpdateProductInput>(json!({
            "productId": 1,
            "name": "Café",
            "profitPercent": "20,00",
            "quantity": "1"
        }))
        .is_err());
        let update_input: super::UpdateProductInput = serde_json::from_value(json!({
            "productId": 1,
            "name": "Café",
            "profitPercent": "20,00"
        }))
        .unwrap();
        assert_eq!(update_input.product_id, 1);
        assert!(update_input.description.is_none());
        for forbidden_field in ["quantity", "sellingPrice", "cost"] {
            let mut input = json!({
                "productId": 1,
                "name": "Café",
                "profitPercent": "20,00"
            });
            input[forbidden_field] = json!("1");
            assert!(serde_json::from_value::<super::UpdateProductInput>(input).is_err());
        }
        let preview_existing: super::PreviewSellingPriceInput = serde_json::from_value(json!({
            "profitPercent": "20,00",
            "productId": 1
        }))
        .unwrap();
        assert_eq!(preview_existing.product_id, Some(1));
        assert_eq!(preview_existing.cost, None);
        let preview_new: super::PreviewSellingPriceInput = serde_json::from_value(json!({
            "profitPercent": "20,00",
            "cost": "1,00"
        }))
        .unwrap();
        assert_eq!(preview_new.product_id, None);
        assert_eq!(preview_new.cost.as_deref(), Some("1,00"));
        assert!(
            serde_json::from_value::<super::PreviewSellingPriceInput>(json!({
                "profitPercent": "20,00",
                "cost": "1,00",
                "unexpected": true
            }))
            .is_err()
        );
    }
}
