use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct AssociatedInvoice {
    pub number: String,
    pub invoice_type: String,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct InvoiceListItem {
    pub id: i64,
    pub number: String,
    pub invoice_type: String,
    pub contact_name: Option<String>,
    pub date: String,
    pub total_cents: i64,
    pub status: Option<String>,
    pub associated_invoice: Option<AssociatedInvoice>,
    /// Solo en pérdidas: copia del producto y cantidad de su única línea.
    pub product_name: Option<String>,
    pub quantity: Option<String>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct InvoiceActionResult {
    pub invoice_id: i64,
    pub number: String,
    pub expired_cleanup_warning: Option<String>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct InvoiceDetail {
    pub id: i64,
    pub number: String,
    pub invoice_type: String,
    pub contact_name: Option<String>,
    pub date: String,
    pub total_cents: i64,
    pub associated_invoice: Option<AssociatedInvoice>,
    pub status: Option<String>,
    pub notes: Option<String>,
    pub payment_method: Option<String>,
    pub paid_at: Option<String>,
    /// Cliente o proveedor vigente (para editar); `None` si se eliminó.
    pub contact_id: Option<i64>,
    /// Factura asociada vigente (garantía e insumos); `None` si se eliminó.
    pub associated_invoice_id: Option<i64>,
    pub contact_full_name: Option<String>,
    pub contact_company: Option<String>,
    pub contact_national_id: Option<String>,
    pub contact_phone: Option<String>,
    pub contact_email: Option<String>,
    pub attachment_file_name: Option<String>,
    pub attachment_original_name: Option<String>,
    pub attachment_size_bytes: Option<i64>,
    pub attachment_uploaded_at: Option<String>,
    pub created_by_name: String,
    pub created_at: String,
    pub updated_at: String,
    pub lines: Vec<crate::models::invoice_line::InvoiceLineDetail>,
}

#[derive(Debug, Clone, PartialEq, Eq, Deserialize, Default)]
#[serde(deny_unknown_fields, rename_all = "camelCase")]
pub struct InvoiceFilters {
    #[serde(default)]
    pub search: Option<String>,
    #[serde(default)]
    pub invoice_type: Option<String>,
    #[serde(default)]
    pub status: Option<String>,
    #[serde(default)]
    pub from_date: Option<String>,
    #[serde(default)]
    pub to_date: Option<String>,
}

#[derive(Debug, Clone, PartialEq, Eq, Deserialize)]
#[serde(deny_unknown_fields, rename_all = "camelCase")]
pub struct InvoiceLineInput {
    pub product_id: i64,
    pub quantity: String,
}

#[derive(Debug, Clone, PartialEq, Eq, Deserialize)]
#[serde(deny_unknown_fields, rename_all = "camelCase")]
pub struct NewProductInput {
    pub name: String,
    #[serde(default)]
    pub description: Option<String>,
    #[serde(default)]
    pub category_id: Option<i64>,
    #[serde(default)]
    pub unit_id: Option<i64>,
    pub cost: String,
    pub profit_percent: String,
    #[serde(default)]
    pub expires_on: Option<String>,
}

#[derive(Debug, Clone, PartialEq, Eq, Deserialize)]
#[serde(deny_unknown_fields, rename_all = "camelCase")]
pub struct EntryInvoiceLineInput {
    #[serde(default)]
    pub product_id: Option<i64>,
    #[serde(default)]
    pub new_product: Option<NewProductInput>,
    pub quantity: String,
    pub unit_price: String,
}

#[derive(Debug, Clone, PartialEq, Eq, Deserialize)]
#[serde(deny_unknown_fields, rename_all = "camelCase")]
pub struct CreateEntryInvoiceInput {
    pub supplier_id: i64,
    pub invoice_date: String,
    #[serde(default)]
    pub status: Option<String>,
    #[serde(default)]
    pub notes: Option<String>,
    #[serde(default)]
    pub payment_method: Option<String>,
    #[serde(default)]
    pub attachment_file_name: Option<String>,
    #[serde(default)]
    pub attachment_original_name: Option<String>,
    #[serde(default)]
    pub attachment_size_bytes: Option<i64>,
    #[serde(default)]
    pub attachment_uploaded_at: Option<String>,
    pub lines: Vec<EntryInvoiceLineInput>,
}

#[derive(Debug, Clone, PartialEq, Eq, Deserialize)]
#[serde(deny_unknown_fields, rename_all = "camelCase")]
pub struct SaleInvoiceLineInput {
    pub product_id: i64,
    pub quantity: String,
}

#[derive(Debug, Clone, PartialEq, Eq, Deserialize)]
#[serde(deny_unknown_fields, rename_all = "camelCase")]
pub struct CreateSaleInvoiceInput {
    pub client_id: i64,
    pub invoice_date: String,
    #[serde(default)]
    pub status: Option<String>,
    #[serde(default)]
    pub notes: Option<String>,
    #[serde(default)]
    pub payment_method: Option<String>,
    pub lines: Vec<SaleInvoiceLineInput>,
}

#[derive(Debug, Clone, PartialEq, Eq, Deserialize)]
#[serde(deny_unknown_fields, rename_all = "camelCase")]
pub struct ServiceInvoiceLineInput {
    pub product_id: i64,
    pub quantity: String,
}

#[derive(Debug, Clone, PartialEq, Eq, Deserialize)]
#[serde(deny_unknown_fields, rename_all = "camelCase")]
pub struct CreateServiceInvoiceInput {
    pub client_id: i64,
    pub invoice_date: String,
    #[serde(default)]
    pub status: Option<String>,
    #[serde(default)]
    pub notes: Option<String>,
    #[serde(default)]
    pub payment_method: Option<String>,
    pub lines: Vec<ServiceInvoiceLineInput>,
}

#[derive(Debug, Clone, PartialEq, Eq, Deserialize)]
#[serde(deny_unknown_fields, rename_all = "camelCase")]
pub struct GuaranteeInvoiceLineInput {
    pub product_id: i64,
    pub quantity: String,
}

#[derive(Debug, Clone, PartialEq, Eq, Deserialize)]
#[serde(deny_unknown_fields, rename_all = "camelCase")]
pub struct CreateGuaranteeInvoiceInput {
    pub client_id: i64,
    pub associated_invoice_id: i64,
    pub invoice_date: String,
    #[serde(default)]
    pub status: Option<String>,
    #[serde(default)]
    pub notes: Option<String>,
    pub lines: Vec<GuaranteeInvoiceLineInput>,
}

#[derive(Debug, Clone, PartialEq, Eq, Deserialize)]
#[serde(deny_unknown_fields, rename_all = "camelCase")]
pub struct InstallationInvoiceLineInput {
    pub product_id: i64,
    pub quantity: String,
}

#[derive(Debug, Clone, PartialEq, Eq, Deserialize)]
#[serde(deny_unknown_fields, rename_all = "camelCase")]
pub struct CreateInstallationInvoiceInput {
    pub client_id: i64,
    pub associated_invoice_id: i64,
    pub invoice_date: String,
    #[serde(default)]
    pub status: Option<String>,
    #[serde(default)]
    pub notes: Option<String>,
    pub lines: Vec<InstallationInvoiceLineInput>,
}

#[derive(Debug, Clone, PartialEq, Eq, Deserialize)]
#[serde(deny_unknown_fields, rename_all = "camelCase")]
pub struct CreateLossInvoiceInput {
    pub product_id: i64,
    pub quantity: String,
    pub invoice_date: String,
    #[serde(default)]
    pub notes: Option<String>,
}

#[derive(Debug, Clone, PartialEq, Eq, Deserialize)]
#[serde(deny_unknown_fields, rename_all = "camelCase")]
pub struct UpdateOutgoingInvoiceLineInput {
    #[serde(default)]
    pub product_id_original: Option<i64>,
    #[serde(default)]
    pub product_id: Option<i64>,
    pub quantity: String,
}

#[derive(Debug, Clone, PartialEq, Eq, Deserialize)]
#[serde(deny_unknown_fields, rename_all = "camelCase")]
pub struct UpdateSaleInvoiceInput {
    pub invoice_id: i64,
    pub client_id: i64,
    pub invoice_date: String,
    #[serde(default)]
    pub notes: Option<String>,
    pub lines: Vec<UpdateOutgoingInvoiceLineInput>,
}

#[derive(Debug, Clone, PartialEq, Eq, Deserialize)]
#[serde(deny_unknown_fields, rename_all = "camelCase")]
pub struct UpdateServiceInvoiceInput {
    pub invoice_id: i64,
    pub client_id: i64,
    pub invoice_date: String,
    #[serde(default)]
    pub notes: Option<String>,
    pub lines: Vec<UpdateOutgoingInvoiceLineInput>,
}

#[derive(Debug, Clone, PartialEq, Eq, Deserialize)]
#[serde(deny_unknown_fields, rename_all = "camelCase")]
pub struct UpdateGuaranteeInvoiceInput {
    pub invoice_id: i64,
    pub client_id: i64,
    pub associated_invoice_id: i64,
    pub invoice_date: String,
    #[serde(default)]
    pub notes: Option<String>,
    pub lines: Vec<UpdateOutgoingInvoiceLineInput>,
}

#[derive(Debug, Clone, PartialEq, Eq, Deserialize)]
#[serde(deny_unknown_fields, rename_all = "camelCase")]
pub struct UpdateInstallationInvoiceInput {
    pub invoice_id: i64,
    pub client_id: i64,
    pub associated_invoice_id: i64,
    pub invoice_date: String,
    #[serde(default)]
    pub notes: Option<String>,
    pub lines: Vec<UpdateOutgoingInvoiceLineInput>,
}

#[derive(Debug, Clone, PartialEq, Eq, Deserialize)]
#[serde(deny_unknown_fields, rename_all = "camelCase")]
pub struct UpdateEntryInvoiceLineInput {
    #[serde(default)]
    pub product_id_original: Option<i64>,
    #[serde(default)]
    pub product_id: Option<i64>,
    pub quantity: String,
    pub unit_price: String,
}

#[derive(Debug, Clone, PartialEq, Eq, Deserialize)]
#[serde(deny_unknown_fields, rename_all = "camelCase")]
pub struct UpdateEntryInvoiceInput {
    pub invoice_id: i64,
    pub supplier_id: i64,
    pub invoice_date: String,
    #[serde(default)]
    pub notes: Option<String>,
    pub lines: Vec<UpdateEntryInvoiceLineInput>,
}

#[cfg(test)]
mod tests {
    use super::{
        AssociatedInvoice, CreateGuaranteeInvoiceInput, CreateInstallationInvoiceInput,
        CreateLossInvoiceInput, CreateSaleInvoiceInput, CreateServiceInvoiceInput, InvoiceFilters,
        InvoiceListItem, UpdateEntryInvoiceInput, UpdateSaleInvoiceInput,
    };
    use serde_json::{json, Value};

    #[test]
    fn invoice_filters_default_omitted_fields_when_deserialized() {
        let filters: InvoiceFilters = serde_json::from_str("{}").unwrap();
        assert_eq!(filters, InvoiceFilters::default());
    }

    #[test]
    fn invoice_list_dto_serializes_camel_case_and_optional_association() {
        let item = InvoiceListItem {
            id: 2,
            number: "V-00001".to_owned(),
            invoice_type: "V".to_owned(),
            contact_name: Some("Ana Pérez".to_owned()),
            date: "2026-10-03".to_owned(),
            total_cents: 1200,
            status: Some("pending".to_owned()),
            associated_invoice: Some(AssociatedInvoice {
                number: "S-00001".to_owned(),
                invoice_type: "S".to_owned(),
            }),
            product_name: None,
            quantity: None,
        };
        let value: Value = serde_json::to_value(item).unwrap();
        assert_eq!(value["invoiceType"], "V");
        assert_eq!(value["totalCents"], json!(1200));
        assert_eq!(value["associatedInvoice"]["invoiceType"], "S");
    }

    #[test]
    fn update_entry_invoice_input_rejects_legacy_top_level_quantity() {
        let valid = json!({
            "invoiceId": 1,
            "supplierId": 2,
            "invoiceDate": "2026-10-04",
            "lines": [{
                "productIdOriginal": 7,
                "productId": 7,
                "quantity": "2,000",
                "unitPrice": "1,50"
            }]
        });
        let parsed: UpdateEntryInvoiceInput = serde_json::from_value(valid.clone()).unwrap();
        assert_eq!(parsed.invoice_id, 1);

        let mut with_legacy_quantity = valid;
        with_legacy_quantity["quantity"] = json!(2);
        assert!(serde_json::from_value::<UpdateEntryInvoiceInput>(with_legacy_quantity).is_err());
    }

    #[test]
    fn create_sale_invoice_input_accepts_lines_and_rejects_unit_price() {
        let valid = json!({
            "clientId": 1,
            "invoiceDate": "2026-10-04",
            "status": "pending",
            "lines": [{ "productId": 7, "quantity": "2" }]
        });
        let parsed: CreateSaleInvoiceInput = serde_json::from_value(valid).unwrap();
        assert_eq!(parsed.client_id, 1);
        assert_eq!(parsed.lines.len(), 1);

        // El precio lo calcula el backend; el frontend no lo envía.
        let with_price = json!({
            "clientId": 1,
            "invoiceDate": "2026-10-04",
            "lines": [{ "productId": 7, "quantity": "2", "unitPrice": "1,50" }]
        });
        assert!(serde_json::from_value::<CreateSaleInvoiceInput>(with_price).is_err());
    }

    #[test]
    fn create_service_invoice_input_accepts_lines_and_rejects_unit_price() {
        let valid = json!({
            "clientId": 1,
            "invoiceDate": "2026-10-04",
            "status": "pending",
            "lines": [{ "productId": 7, "quantity": "2" }]
        });
        let parsed: CreateServiceInvoiceInput = serde_json::from_value(valid).unwrap();
        assert_eq!(parsed.client_id, 1);
        assert_eq!(parsed.lines.len(), 1);

        let with_price = json!({
            "clientId": 1,
            "invoiceDate": "2026-10-04",
            "lines": [{ "productId": 7, "quantity": "2", "unitPrice": "1,50" }]
        });
        assert!(serde_json::from_value::<CreateServiceInvoiceInput>(with_price).is_err());
    }

    #[test]
    fn create_guarantee_invoice_input_requires_association_and_rejects_payment() {
        let valid = json!({
            "clientId": 1,
            "associatedInvoiceId": 9,
            "invoiceDate": "2026-10-04",
            "status": "pending",
            "lines": [{ "productId": 7, "quantity": "2" }]
        });
        let parsed: CreateGuaranteeInvoiceInput = serde_json::from_value(valid).unwrap();
        assert_eq!(parsed.associated_invoice_id, 9);

        // Las garantías no tienen método de pago: se rechaza el campo.
        let with_payment = json!({
            "clientId": 1,
            "associatedInvoiceId": 9,
            "invoiceDate": "2026-10-04",
            "paymentMethod": "cash",
            "lines": [{ "productId": 7, "quantity": "2" }]
        });
        assert!(serde_json::from_value::<CreateGuaranteeInvoiceInput>(with_payment).is_err());
    }

    #[test]
    fn create_installation_invoice_input_requires_service_association() {
        let valid = json!({
            "clientId": 1,
            "associatedInvoiceId": 9,
            "invoiceDate": "2026-10-04",
            "status": "pending",
            "lines": [{ "productId": 7, "quantity": "2" }]
        });
        let parsed: CreateInstallationInvoiceInput = serde_json::from_value(valid).unwrap();
        assert_eq!(parsed.associated_invoice_id, 9);

        let with_payment = json!({
            "clientId": 1,
            "associatedInvoiceId": 9,
            "invoiceDate": "2026-10-04",
            "paymentMethod": "cash",
            "lines": [{ "productId": 7, "quantity": "2" }]
        });
        assert!(serde_json::from_value::<CreateInstallationInvoiceInput>(with_payment).is_err());
    }

    #[test]
    fn create_loss_invoice_input_takes_single_product_without_lines_array() {
        let valid = json!({
            "productId": 7,
            "quantity": "2",
            "invoiceDate": "2026-10-04",
            "notes": "Rotura"
        });
        let parsed: CreateLossInvoiceInput = serde_json::from_value(valid).unwrap();
        assert_eq!(parsed.product_id, 7);

        // Un solo producto: no es un arreglo de líneas.
        let with_lines = json!({
            "productId": 7,
            "quantity": "2",
            "invoiceDate": "2026-10-04",
            "lines": [{ "productId": 7, "quantity": "2" }]
        });
        assert!(serde_json::from_value::<CreateLossInvoiceInput>(with_lines).is_err());
    }

    #[test]
    fn update_sale_invoice_input_keeps_line_identity_without_prices() {
        let valid = json!({
            "invoiceId": 1,
            "clientId": 2,
            "invoiceDate": "2026-10-04",
            "lines": [
                { "productIdOriginal": 7, "productId": 7, "quantity": "2" },
                { "productId": 9, "quantity": "1" }
            ]
        });
        let parsed: UpdateSaleInvoiceInput = serde_json::from_value(valid).unwrap();
        assert_eq!(parsed.lines.len(), 2);

        // El precio lo decide el backend (conservado o vigente); no se envía.
        let with_price = json!({
            "invoiceId": 1,
            "clientId": 2,
            "invoiceDate": "2026-10-04",
            "lines": [{ "productIdOriginal": 7, "productId": 7, "quantity": "2", "unitPrice": "1,50" }]
        });
        assert!(serde_json::from_value::<UpdateSaleInvoiceInput>(with_price).is_err());
    }
}
