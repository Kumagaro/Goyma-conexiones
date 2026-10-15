use chrono::{DateTime, Utc};

use crate::commands::auth::Actor;
use crate::timestamps::format_utc_timestamp;

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Contact {
    pub first_name: String,
    pub last_name: String,
    pub company: Option<String>,
    pub national_id: String,
    pub phone: String,
    pub email: Option<String>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ContactSnapshot {
    pub full_name: String,
    pub company: Option<String>,
    pub national_id: String,
    pub phone: String,
    pub email: Option<String>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Product {
    pub id: i64,
    pub name: String,
    pub name_normalized: String,
    pub unit_id: Option<i64>,
    pub unit_name: Option<String>,
    pub unit_abbreviation: Option<String>,
    pub category_id: Option<i64>,
    pub category_name: Option<String>,
    pub cost_cents: i64,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct LineInput {
    pub product_id: i64,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct LineSnapshot {
    pub product_id: Option<i64>,
    pub product_id_original: i64,
    pub product_name: String,
    pub product_name_normalized: String,
    pub unit_id: Option<i64>,
    pub unit_name: Option<String>,
    pub unit_abbreviation: Option<String>,
    pub category_id: Option<i64>,
    pub category_name: Option<String>,
    pub cost_at_sale_cents: Option<i64>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct MovementSnapshot {
    pub created_at: String,
    pub product_id: Option<i64>,
    pub product_id_original: i64,
    pub product_name: String,
    pub movement_type: String,
    pub quantity_milli: i64,
    pub stock_after_milli: i64,
    pub unit_price_cents: i64,
    pub total_price_cents: i64,
    pub invoice_id: Option<i64>,
    pub invoice_number: Option<String>,
    pub invoice_type: Option<String>,
    pub user_id: Option<i64>,
    pub user_name: String,
}

pub fn build_contact_snapshot(contact: &Contact) -> ContactSnapshot {
    ContactSnapshot {
        full_name: format!("{} {}", contact.first_name, contact.last_name),
        company: contact.company.clone(),
        national_id: contact.national_id.clone(),
        phone: contact.phone.clone(),
        email: contact.email.clone(),
    }
}

pub fn build_line_snapshot(product: &Product, line: &LineInput, is_sale: bool) -> LineSnapshot {
    LineSnapshot {
        product_id: Some(product.id),
        product_id_original: line.product_id,
        product_name: product.name.clone(),
        product_name_normalized: product.name_normalized.clone(),
        unit_id: product.unit_id,
        unit_name: product.unit_name.clone(),
        unit_abbreviation: product.unit_abbreviation.clone(),
        category_id: if is_sale { product.category_id } else { None },
        category_name: if is_sale {
            product.category_name.clone()
        } else {
            None
        },
        cost_at_sale_cents: is_sale.then_some(product.cost_cents),
    }
}

#[allow(clippy::too_many_arguments)]
pub fn build_movement_snapshot(
    product: &Product,
    invoice_number: Option<&str>,
    invoice_type: Option<&str>,
    user: &Actor,
    quantity_milli: i64,
    stock_after_milli: i64,
    unit_price_cents: i64,
    total_price_cents: i64,
    movement_type: &str,
    now: DateTime<Utc>,
) -> MovementSnapshot {
    MovementSnapshot {
        created_at: format_utc_timestamp(now),
        product_id: Some(product.id),
        product_id_original: product.id,
        product_name: product.name.clone(),
        movement_type: movement_type.to_owned(),
        quantity_milli,
        stock_after_milli,
        unit_price_cents,
        total_price_cents,
        invoice_id: None,
        invoice_number: invoice_number.map(str::to_owned),
        invoice_type: invoice_type.map(str::to_owned),
        user_id: Some(user.id()),
        user_name: user.username().to_owned(),
    }
}

#[cfg(test)]
mod tests {
    use chrono::{TimeZone, Utc};

    use crate::commands::auth::require_session;
    use crate::test_utils::{create_test_session, create_test_user, setup_test_db};

    use super::{
        build_contact_snapshot, build_line_snapshot, build_movement_snapshot, Contact, LineInput,
        Product,
    };

    #[test]
    fn contact_snapshot_copies_identity_and_optional_fields() {
        let contact = Contact {
            first_name: "Ana".to_owned(),
            last_name: "Pérez".to_owned(),
            company: Some("Goyma".to_owned()),
            national_id: "12345678901".to_owned(),
            phone: "+5355551234".to_owned(),
            email: None,
        };
        let snapshot = build_contact_snapshot(&contact);
        assert_eq!(snapshot.full_name, "Ana Pérez");
        assert_eq!(snapshot.company.as_deref(), Some("Goyma"));
        assert_eq!(snapshot.national_id, "12345678901");
        assert_eq!(snapshot.phone, "+5355551234");
        assert_eq!(snapshot.email, None);
    }

    #[test]
    fn line_snapshot_includes_sale_only_category_and_cost_fields() {
        let product = Product {
            id: 12,
            name: "Cable UTP".to_owned(),
            name_normalized: "cable utp".to_owned(),
            unit_id: Some(3),
            unit_name: Some("Metro".to_owned()),
            unit_abbreviation: Some("m".to_owned()),
            category_id: Some(4),
            category_name: Some("Cables".to_owned()),
            cost_cents: 250,
        };
        let line = LineInput { product_id: 12 };
        let sale = build_line_snapshot(&product, &line, true);
        assert_eq!(sale.product_id_original, 12);
        assert_eq!(sale.product_name_normalized, "cable utp");
        assert_eq!(sale.category_id, Some(4));
        assert_eq!(sale.category_name.as_deref(), Some("Cables"));
        assert_eq!(sale.cost_at_sale_cents, Some(250));

        let purchase = build_line_snapshot(&product, &line, false);
        assert_eq!(purchase.category_id, None);
        assert_eq!(purchase.category_name, None);
        assert_eq!(purchase.cost_at_sale_cents, None);
    }

    #[test]
    fn movement_snapshot_copies_product_invoice_and_authenticated_user() {
        let connection = setup_test_db();
        let user_id = create_test_user(
            &connection,
            "snapshotuser",
            "contador",
            "+5350000011",
            "12345678911",
            "ValidPass1!",
        );
        let now = Utc.with_ymd_and_hms(2026, 10, 6, 12, 0, 0).unwrap();
        create_test_session(&connection, user_id, false, now, now);
        let actor = require_session(&connection, now).unwrap();
        let product = Product {
            id: 12,
            name: "Cable UTP".to_owned(),
            name_normalized: "cable utp".to_owned(),
            unit_id: Some(3),
            unit_name: Some("Metro".to_owned()),
            unit_abbreviation: Some("m".to_owned()),
            category_id: None,
            category_name: None,
            cost_cents: 250,
        };
        let movement = build_movement_snapshot(
            &product,
            Some("V-00012"),
            Some("V"),
            &actor,
            -3_000,
            7_000,
            400,
            1_200,
            "saleOut",
            now,
        );
        assert_eq!(movement.product_id_original, 12);
        assert_eq!(movement.product_name, "Cable UTP");
        assert_eq!(movement.invoice_number.as_deref(), Some("V-00012"));
        assert_eq!(movement.invoice_type.as_deref(), Some("V"));
        assert_eq!(movement.user_id, Some(user_id));
        assert_eq!(movement.user_name, "snapshotuser");
        assert_eq!(movement.quantity_milli, -3_000);
        assert_eq!(movement.stock_after_milli, 7_000);
        assert_eq!(movement.created_at, "2026-10-06T12:00:00Z");
    }
}
