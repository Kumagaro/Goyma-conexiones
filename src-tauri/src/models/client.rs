use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct ContactListItem {
    pub id: i64,
    pub full_name: String,
    pub company: Option<String>,
    pub phone: String,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct ContactMatch {
    pub id: i64,
    pub full_name: String,
    pub company: Option<String>,
    pub is_exact_match: bool,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct ContactIndicators {
    pub total_invoiced_cents: i64,
    pub total_purchased_cents: i64,
    pub pending_balance_cents: i64,
    pub invoice_count: i64,
    pub last_purchase_date: Option<String>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct ContactDetail {
    pub id: i64,
    pub first_name: String,
    pub last_name: String,
    pub full_name: String,
    pub national_id: String,
    pub phone: String,
    pub company: Option<String>,
    pub position: Option<String>,
    pub email: Option<String>,
    pub address: Option<String>,
    pub invoice_info: Option<String>,
    pub created_at: String,
    pub indicators: ContactIndicators,
    pub invoice_history: Vec<super::invoice::InvoiceListItem>,
    pub expired_cleanup_warning: Option<String>,
}

#[derive(Debug, Clone, PartialEq, Eq, Deserialize)]
#[serde(deny_unknown_fields, rename_all = "camelCase")]
pub struct ContactInput {
    pub first_name: String,
    pub last_name: String,
    pub national_id: String,
    pub phone: String,
    #[serde(default)]
    pub company: Option<String>,
    #[serde(default)]
    pub position: Option<String>,
    #[serde(default)]
    pub email: Option<String>,
    #[serde(default)]
    pub address: Option<String>,
    #[serde(default)]
    pub invoice_info: Option<String>,
}

#[cfg(test)]
mod tests {
    use serde_json::json;

    use super::{ContactInput, ContactListItem};

    #[test]
    fn contact_dto_and_input_use_camel_case_and_reject_unknown_fields() {
        let item = ContactListItem {
            id: 7,
            full_name: "Ana Pérez".to_owned(),
            company: None,
            phone: "+5355551234".to_owned(),
        };
        assert_eq!(
            serde_json::to_value(item).unwrap(),
            json!({"id":7,"fullName":"Ana Pérez","company":null,"phone":"+5355551234"})
        );
        assert!(serde_json::from_value::<ContactInput>(json!({"firstName":"Ana","lastName":"Pérez","nationalId":"12345678901","phone":"+5355551234","unknown":true})).is_err());
    }
}
