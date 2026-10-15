use serde::Serialize;

#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct CategoryListItem {
    pub id: i64,
    pub name: String,
    pub description: Option<String>,
    pub product_count: i64,
    pub ingresos_cents: i64,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct CategoryList {
    pub items: Vec<CategoryListItem>,
    pub expired_cleanup_warning: Option<String>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct CategoryMatch {
    pub id: i64,
    pub name: String,
    pub is_exact_match: bool,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct ProductBrief {
    pub id: i64,
    pub name: String,
    pub is_out_of_stock: bool,
    pub is_discontinued: bool,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct CategoryDetail {
    pub id: i64,
    pub name: String,
    pub description: Option<String>,
    pub product_count: i64,
    pub ingresos_cents: i64,
    pub products: Vec<ProductBrief>,
    pub expired_cleanup_warning: Option<String>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct DeleteCategoryResult {
    pub affected_products: usize,
    pub expired_cleanup_warning: Option<String>,
}

#[cfg(test)]
mod tests {
    use serde_json::json;

    use super::{
        CategoryDetail, CategoryList, CategoryListItem, CategoryMatch, DeleteCategoryResult,
        ProductBrief,
    };

    #[test]
    fn category_dtos_serialize_expected_camel_case_keys_and_warning_values() {
        let item = CategoryListItem {
            id: 3,
            name: "Café".to_owned(),
            description: None,
            product_count: 2,
            ingresos_cents: 4500,
        };
        assert_eq!(
            serde_json::to_value(&item).unwrap(),
            json!({"id": 3, "name": "Café", "description": null, "productCount": 2, "ingresosCents": 4500})
        );

        let list = CategoryList {
            items: vec![item.clone()],
            expired_cleanup_warning: None,
        };
        assert_eq!(
            serde_json::to_value(&list).unwrap(),
            json!({"items": [{"id": 3, "name": "Café", "description": null, "productCount": 2, "ingresosCents": 4500}], "expiredCleanupWarning": null})
        );
        let list_with_warning = CategoryList {
            items: Vec::new(),
            expired_cleanup_warning: Some("aviso".to_owned()),
        };
        assert_eq!(
            serde_json::to_value(list_with_warning).unwrap()["expiredCleanupWarning"],
            json!("aviso")
        );

        let match_item = CategoryMatch {
            id: 3,
            name: "Café".to_owned(),
            is_exact_match: true,
        };
        assert_eq!(
            serde_json::to_value(match_item).unwrap(),
            json!({"id": 3, "name": "Café", "isExactMatch": true})
        );

        let detail = CategoryDetail {
            id: 3,
            name: "Café".to_owned(),
            description: Some("Molido".to_owned()),
            product_count: 1,
            ingresos_cents: 4500,
            products: vec![ProductBrief {
                id: 8,
                name: "Café molido".to_owned(),
                is_out_of_stock: false,
                is_discontinued: true,
            }],
            expired_cleanup_warning: Some("aviso".to_owned()),
        };
        assert_eq!(
            serde_json::to_value(detail).unwrap(),
            json!({"id": 3, "name": "Café", "description": "Molido", "productCount": 1, "ingresosCents": 4500, "products": [{"id": 8, "name": "Café molido", "isOutOfStock": false, "isDiscontinued": true}], "expiredCleanupWarning": "aviso"})
        );
        let detail_without_warning = CategoryDetail {
            id: 3,
            name: "Café".to_owned(),
            description: None,
            product_count: 0,
            ingresos_cents: 0,
            products: Vec::new(),
            expired_cleanup_warning: None,
        };
        assert_eq!(
            serde_json::to_value(detail_without_warning).unwrap()["expiredCleanupWarning"],
            json!(null)
        );

        let deleted = DeleteCategoryResult {
            affected_products: 4,
            expired_cleanup_warning: None,
        };
        assert_eq!(
            serde_json::to_value(deleted).unwrap(),
            json!({"affectedProducts": 4, "expiredCleanupWarning": null})
        );
        let deleted_with_warning = DeleteCategoryResult {
            affected_products: 4,
            expired_cleanup_warning: Some("aviso".to_owned()),
        };
        assert_eq!(
            serde_json::to_value(deleted_with_warning).unwrap()["expiredCleanupWarning"],
            json!("aviso")
        );
    }
}
