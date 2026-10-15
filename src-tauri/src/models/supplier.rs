use serde::{Deserialize, Serialize};

use super::client::{ContactIndicators, ContactListItem, ContactMatch};
use super::invoice::InvoiceListItem;

pub type SupplierListItem = ContactListItem;
pub type SupplierMatch = ContactMatch;

#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct SupplierDetail {
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
    pub created_at: String,
    pub indicators: ContactIndicators,
    pub invoice_history: Vec<InvoiceListItem>,
    pub expired_cleanup_warning: Option<String>,
}

#[derive(Debug, Clone, PartialEq, Eq, Deserialize)]
#[serde(deny_unknown_fields, rename_all = "camelCase")]
pub struct SupplierInput {
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
}

#[cfg(test)]
mod tests {
    use serde_json::json;

    use super::SupplierInput;

    #[test]
    fn supplier_input_has_no_invoice_info_and_rejects_unknown_fields() {
        let parsed = serde_json::from_value::<SupplierInput>(json!({
            "firstName":"Ana", "lastName":"Pérez", "nationalId":"12345678901", "phone":"+5355551234"
        }));
        assert!(parsed.is_ok());
        assert!(serde_json::from_value::<SupplierInput>(json!({
			"firstName":"Ana", "lastName":"Pérez", "nationalId":"12345678901", "phone":"+5355551234", "invoiceInfo":"no"
		})).is_err());
    }
}
