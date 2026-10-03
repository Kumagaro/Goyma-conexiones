use serde::Serialize;

#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct UnitListItem {
    pub id: i64,
    pub name: String,
    pub abbreviation: String,
    pub product_count: i64,
    pub conversion_count: i64,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct UnitList {
    pub items: Vec<UnitListItem>,
    pub expired_cleanup_warning: Option<String>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct UnitMatch {
    pub id: i64,
    pub name: String,
    pub abbreviation: String,
    pub is_exact_match: bool,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct DeleteUnitResult {
    pub affected_products: usize,
    pub deleted_conversions: usize,
    pub expired_cleanup_warning: Option<String>,
}

#[cfg(test)]
mod tests {
    use serde_json::json;

    use super::{DeleteUnitResult, UnitList, UnitListItem, UnitMatch};

    #[test]
    fn unit_dtos_serialize_expected_camel_case_keys_and_nullable_warning() {
        let item = UnitListItem {
            id: 5,
            name: "Kilogramo".to_owned(),
            abbreviation: "kg".to_owned(),
            product_count: 7,
            conversion_count: 2,
        };
        assert_eq!(
            serde_json::to_value(&item).unwrap(),
            json!({"id": 5, "name": "Kilogramo", "abbreviation": "kg", "productCount": 7, "conversionCount": 2})
        );

        let list = UnitList {
            items: vec![item],
            expired_cleanup_warning: None,
        };
        assert_eq!(
            serde_json::to_value(list).unwrap(),
            json!({"items": [{"id": 5, "name": "Kilogramo", "abbreviation": "kg", "productCount": 7, "conversionCount": 2}], "expiredCleanupWarning": null})
        );
        let list_with_warning = UnitList {
            items: Vec::new(),
            expired_cleanup_warning: Some("aviso".to_owned()),
        };
        assert_eq!(
            serde_json::to_value(list_with_warning).unwrap()["expiredCleanupWarning"],
            json!("aviso")
        );

        let unit_match = UnitMatch {
            id: 5,
            name: "Kilogramo".to_owned(),
            abbreviation: "kg".to_owned(),
            is_exact_match: true,
        };
        assert_eq!(
            serde_json::to_value(unit_match).unwrap(),
            json!({"id": 5, "name": "Kilogramo", "abbreviation": "kg", "isExactMatch": true})
        );

        let deleted = DeleteUnitResult {
            affected_products: 4,
            deleted_conversions: 2,
            expired_cleanup_warning: Some("aviso".to_owned()),
        };
        assert_eq!(
            serde_json::to_value(deleted).unwrap(),
            json!({"affectedProducts": 4, "deletedConversions": 2, "expiredCleanupWarning": "aviso"})
        );
        let deleted_without_warning = DeleteUnitResult {
            affected_products: 0,
            deleted_conversions: 0,
            expired_cleanup_warning: None,
        };
        assert_eq!(
            serde_json::to_value(deleted_without_warning).unwrap()["expiredCleanupWarning"],
            json!(null)
        );
    }
}
