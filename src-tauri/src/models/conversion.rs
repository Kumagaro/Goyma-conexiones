use serde::Serialize;

#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct UnitReference {
    pub id: i64,
    pub name: String,
    pub abbreviation: String,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct ConversionItem {
    pub id: i64,
    pub from_unit: UnitReference,
    pub to_unit: UnitReference,
    pub factor: String,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct ConversionResult {
    pub result: String,
}

#[cfg(test)]
mod tests {
    use serde_json::json;

    use super::{ConversionItem, ConversionResult, UnitReference};

    #[test]
    fn conversion_dtos_serialize_nested_camel_case_data_and_decimal_text() {
        let conversion = ConversionItem {
            id: 9,
            from_unit: UnitReference {
                id: 1,
                name: "Kilogramo".to_owned(),
                abbreviation: "kg".to_owned(),
            },
            to_unit: UnitReference {
                id: 2,
                name: "Gramo".to_owned(),
                abbreviation: "g".to_owned(),
            },
            factor: "1000,5".to_owned(),
        };
        let value = serde_json::to_value(conversion).unwrap();
        assert_eq!(
            value,
            json!({"id": 9, "fromUnit": {"id": 1, "name": "Kilogramo", "abbreviation": "kg"}, "toUnit": {"id": 2, "name": "Gramo", "abbreviation": "g"}, "factor": "1000,5"})
        );
        assert!(value["factor"].is_string());

        let result = ConversionResult {
            result: "0,000001".to_owned(),
        };
        assert_eq!(
            serde_json::to_value(result).unwrap(),
            json!({"result": "0,000001"})
        );
    }
}
