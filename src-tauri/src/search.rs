use rusqlite::Connection;

use crate::errors::AppError;
use crate::models::client::ContactMatch;
use crate::models::invoice::{AssociatedInvoice, InvoiceListItem};
use crate::models::product::{format_quantity_milli, ProductMatch};
use crate::text::{clean_name, normalize_text, spanish_sort_key};

pub fn normalize_query(raw: &str) -> String {
    normalize_text(&clean_name(raw))
}

pub fn relevance_rank(field_normalized: &str, query_normalized: &str) -> Option<u8> {
    if field_normalized == query_normalized {
        return Some(0);
    }
    if field_normalized.starts_with(query_normalized)
        || field_normalized
            .split_whitespace()
            .any(|word| word.starts_with(query_normalized))
    {
        return Some(1);
    }
    if field_normalized.contains(query_normalized) {
        return Some(2);
    }
    None
}

pub fn search_contacts(
    connection: &Connection,
    table: &str,
    raw_query: &str,
    limit: usize,
) -> Result<Vec<ContactMatch>, AppError> {
    if !matches!(table, "clients" | "suppliers") {
        return Err(AppError::new(
            "validationFailed",
            "La búsqueda no es válida",
            "unsupported contact search table",
        ));
    }
    if limit == 0 {
        return Ok(Vec::new());
    }
    let query = normalize_query(raw_query);
    let phone_query = raw_query
        .chars()
        .filter(|ch| !ch.is_whitespace() && *ch != '+')
        .collect::<String>();
    let phone_query = normalize_query(&phone_query);
    let sql = format!("SELECT id, firstName, lastName, company, nationalId, phone FROM {table}");
    let mut statement = connection
        .prepare(&sql)
        .map_err(|_| internal_error("failed to prepare contact search"))?;
    let rows = statement
        .query_map([], |row| {
            Ok((
                row.get::<_, i64>(0)?,
                row.get::<_, String>(1)?,
                row.get::<_, String>(2)?,
                row.get::<_, Option<String>>(3)?,
                row.get::<_, String>(4)?,
                row.get::<_, String>(5)?,
            ))
        })
        .map_err(|_| internal_error("failed to query contacts"))?
        .collect::<rusqlite::Result<Vec<_>>>()
        .map_err(|_| internal_error("failed to collect contacts"))?;
    let mut matches = Vec::new();
    for (id, first_name, last_name, company, national_id, phone) in rows {
        let full_name = format!("{first_name} {last_name}");
        let normalized_phone = phone
            .chars()
            .filter(|ch| !ch.is_whitespace() && *ch != '+')
            .collect::<String>();
        let fields = [
            normalize_query(&full_name),
            normalize_query(&first_name),
            normalize_query(&last_name),
            normalize_query(&national_id),
            normalize_query(&normalized_phone),
            company.as_deref().map(normalize_query).unwrap_or_default(),
        ];
        let rank = if query.is_empty() {
            Some(0)
        } else {
            fields
                .iter()
                .enumerate()
                .filter_map(|(index, field)| {
                    relevance_rank(field, if index == 4 { &phone_query } else { &query })
                })
                .min()
        };
        if let Some(rank) = rank {
            matches.push((
                rank,
                spanish_sort_key(&full_name),
                id,
                ContactMatch {
                    id,
                    full_name,
                    company,
                    is_exact_match: !query.is_empty()
                        && fields.iter().enumerate().any(|(index, field)| {
                            field == if index == 4 { &phone_query } else { &query }
                        }),
                },
            ));
        }
    }
    matches.sort_by(|left, right| (&left.0, &left.1, left.2).cmp(&(&right.0, &right.1, right.2)));
    Ok(matches.into_iter().take(limit).map(|item| item.3).collect())
}

pub fn search_products_for_invoice(
    connection: &Connection,
    raw_query: &str,
    limit: usize,
) -> Result<Vec<ProductMatch>, AppError> {
    if limit == 0 {
        return Ok(Vec::new());
    }
    let query = normalize_query(raw_query);
    let mut statement = connection
        .prepare("SELECT p.id, p.name, p.description, c.name, u.abbreviation, p.quantityMilli FROM products p LEFT JOIN categories c ON c.id = p.categoryId LEFT JOIN units u ON u.id = p.unitId WHERE p.status = 'active' AND p.quantityMilli > 0")
        .map_err(|_| internal_error("failed to prepare product search"))?;
    let rows = statement
        .query_map([], |row| {
            Ok((
                row.get::<_, i64>(0)?,
                row.get::<_, String>(1)?,
                row.get::<_, Option<String>>(2)?,
                row.get::<_, Option<String>>(3)?,
                row.get::<_, Option<String>>(4)?,
                row.get::<_, i64>(5)?,
            ))
        })
        .map_err(|_| internal_error("failed to query invoice products"))?
        .collect::<rusqlite::Result<Vec<_>>>()
        .map_err(|_| internal_error("failed to collect invoice products"))?;
    let mut matches = Vec::new();
    for (id, name, description, category_name, unit_abbreviation, stock_milli) in rows {
        let fields = [
            normalize_query(&name),
            category_name
                .as_deref()
                .map(normalize_query)
                .unwrap_or_default(),
            description
                .as_deref()
                .map(normalize_query)
                .unwrap_or_default(),
        ];
        let rank = if query.is_empty() {
            Some(0)
        } else {
            fields
                .iter()
                .filter_map(|field| relevance_rank(field, &query))
                .min()
        };
        if let Some(rank) = rank {
            matches.push((
                rank,
                spanish_sort_key(&name),
                id,
                ProductMatch {
                    id,
                    name,
                    category_name,
                    unit_abbreviation,
                    stock: format_quantity_milli(stock_milli),
                    is_out_of_stock: false,
                    is_discontinued: false,
                },
            ));
        }
    }
    matches.sort_by(|left, right| (&left.0, &left.1, left.2).cmp(&(&right.0, &right.1, right.2)));
    Ok(matches.into_iter().take(limit).map(|item| item.3).collect())
}

pub fn search_invoices(
    connection: &Connection,
    raw_query: &str,
    limit: usize,
) -> Result<Vec<InvoiceListItem>, AppError> {
    if limit == 0 {
        return Ok(Vec::new());
    }
    let query = normalize_query(raw_query);
    let mut statement = connection
        .prepare("SELECT id, number, type, contactFullName, contactCompany, contactNationalId, contactPhone, invoiceDate, status, associatedNumber, associatedType FROM invoices")
        .map_err(|_| internal_error("failed to prepare invoice search"))?;
    let rows = statement
        .query_map([], |row| {
            Ok((
                row.get::<_, i64>(0)?,
                row.get::<_, String>(1)?,
                row.get::<_, String>(2)?,
                row.get::<_, Option<String>>(3)?,
                row.get::<_, Option<String>>(4)?,
                row.get::<_, Option<String>>(5)?,
                row.get::<_, Option<String>>(6)?,
                row.get::<_, String>(7)?,
                row.get::<_, Option<String>>(8)?,
                row.get::<_, Option<String>>(9)?,
                row.get::<_, Option<String>>(10)?,
            ))
        })
        .map_err(|_| internal_error("failed to query invoices"))?
        .collect::<rusqlite::Result<Vec<_>>>()
        .map_err(|_| internal_error("failed to collect invoices"))?;
    let mut matches = Vec::new();
    for (
        id,
        number,
        invoice_type,
        contact_name,
        company,
        national_id,
        phone,
        date,
        status,
        associated_number,
        associated_type,
    ) in rows
    {
        let normalized_phone = phone
            .as_deref()
            .unwrap_or_default()
            .chars()
            .filter(|ch| !ch.is_whitespace() && *ch != '+')
            .collect::<String>();
        let phone_query = raw_query
            .chars()
            .filter(|ch| !ch.is_whitespace() && *ch != '+')
            .collect::<String>();
        let phone_query = normalize_query(&phone_query);
        let fields = [
            normalize_query(&number),
            contact_name
                .as_deref()
                .map(normalize_query)
                .unwrap_or_default(),
            company.as_deref().map(normalize_query).unwrap_or_default(),
            national_id
                .as_deref()
                .map(normalize_query)
                .unwrap_or_default(),
            normalize_query(&normalized_phone),
        ];
        let rank = if query.is_empty() {
            Some(0)
        } else {
            fields
                .iter()
                .enumerate()
                .filter_map(|(index, field)| {
                    relevance_rank(field, if index == 4 { &phone_query } else { &query })
                })
                .min()
        };
        if let Some(rank) = rank {
            let total_cents = connection
                .query_row(
                    "SELECT COALESCE(SUM(subtotalCents), 0) FROM invoiceLines WHERE invoiceId = ?1",
                    [id],
                    |row| row.get::<_, i64>(0),
                )
                .map_err(|_| internal_error("failed to calculate invoice total for search"))?;
            matches.push((
                rank,
                date.clone(),
                id,
                InvoiceListItem {
                    id,
                    number,
                    invoice_type,
                    contact_name,
                    date,
                    total_cents,
                    status,
                    associated_invoice: associated_number.zip(associated_type).map(
                        |(number, invoice_type)| AssociatedInvoice {
                            number,
                            invoice_type,
                        },
                    ),
                },
            ));
        }
    }
    matches.sort_by(|left, right| {
        (
            &left.0,
            std::cmp::Reverse(&left.1),
            std::cmp::Reverse(left.2),
        )
            .cmp(&(
                &right.0,
                std::cmp::Reverse(&right.1),
                std::cmp::Reverse(right.2),
            ))
    });
    Ok(matches.into_iter().take(limit).map(|item| item.3).collect())
}

fn internal_error(cause: &str) -> AppError {
    AppError::new("internal", "Ocurrió un error inesperado", cause)
}

#[cfg(test)]
mod tests {
    use crate::test_utils::{
        create_test_user, insert_category_fixture, insert_product_fixture, setup_test_db,
    };

    use super::{
        normalize_query, relevance_rank, search_contacts, search_invoices,
        search_products_for_invoice,
    };

    const TS: &str = "2026-10-03T00:00:00Z";

    #[test]
    fn relevance_ranks_exact_prefix_substring_and_normalized_spanish_text() {
        assert_eq!(relevance_rank("cafe", "cafe"), Some(0));
        assert_eq!(relevance_rank("cafe molido", "cafe"), Some(1));
        assert_eq!(relevance_rank("cafe molido", "molido"), Some(1));
        assert_eq!(relevance_rank("cafe molido", "olido"), Some(2));
        assert_eq!(relevance_rank("cafe", "te"), None);
        assert_eq!(normalize_query("  CAFÉ   "), "cafe");
        assert_eq!(
            relevance_rank(&normalize_query("Café"), &normalize_query("cafe")),
            Some(0)
        );
        assert_eq!(
            relevance_rank(&normalize_query("Año"), &normalize_query("ano")),
            None
        );
    }

    #[test]
    fn contact_search_uses_name_phone_company_and_relevance() {
        let connection = setup_test_db();
        connection.execute(
            "INSERT INTO clients (firstName, lastName, nationalId, phone, company, createdAt, updatedAt)
             VALUES ('Ana', 'Pérez', '12345678901', '+5355551234', 'Café Norte', ?1, ?1)",
            [TS],
        ).unwrap();
        connection
            .execute(
                "INSERT INTO clients (firstName, lastName, nationalId, phone, createdAt, updatedAt)
             VALUES ('María', 'Cano', '12345678902', '+5355559876', ?1, ?1)",
                [TS],
            )
            .unwrap();

        let exact = search_contacts(&connection, "clients", "ana", 20).unwrap();
        assert_eq!(exact[0].full_name, "Ana Pérez");
        assert!(exact[0].is_exact_match);
        assert_eq!(
            search_contacts(&connection, "clients", "cafe", 20).unwrap()[0].id,
            1
        );
        assert_eq!(
            search_contacts(&connection, "clients", "+53 5555 9876", 20).unwrap()[0].full_name,
            "María Cano"
        );
        assert_eq!(
            search_contacts(&connection, "clients", "", 1)
                .unwrap()
                .len(),
            1
        );
        assert!(search_contacts(&connection, "invalid_table", "ana", 20).is_err());
    }

    #[test]
    fn invoice_product_search_excludes_inactive_and_out_of_stock_and_caps_results() {
        let connection = setup_test_db();
        let category_id = insert_category_fixture(&connection, "Cables", None);
        let available_id = insert_product_fixture(
            &connection,
            "Cable UTP",
            Some(category_id),
            None,
            100,
            0,
            5_000,
            "active",
            None,
            None,
        );
        let out_id = insert_product_fixture(
            &connection,
            "Cable agotado",
            Some(category_id),
            None,
            100,
            0,
            0,
            "active",
            None,
            None,
        );
        let discontinued_id = insert_product_fixture(
            &connection,
            "Cable viejo",
            Some(category_id),
            None,
            100,
            0,
            5_000,
            "discontinued",
            Some("2026-10-03T00:00:00Z"),
            None,
        );
        connection
            .execute(
                "UPDATE products SET description = 'Cable de red' WHERE id = ?1",
                [available_id],
            )
            .unwrap();

        let results = search_products_for_invoice(&connection, "cable", 20).unwrap();
        assert_eq!(results.len(), 1);
        assert_eq!(results[0].id, available_id);
        assert!(!results[0].is_out_of_stock);
        assert!(!results[0].is_discontinued);
        assert!(search_products_for_invoice(&connection, "red", 1).unwrap()[0].id == available_id);
        assert!(search_products_for_invoice(&connection, "", 0)
            .unwrap()
            .is_empty());
        assert_ne!(available_id, out_id);
        assert_ne!(available_id, discontinued_id);
    }

    #[test]
    fn invoice_search_matches_number_contact_and_associated_invoice() {
        let connection = setup_test_db();
        connection.execute(
            "INSERT INTO clients (firstName, lastName, nationalId, phone, company, createdAt, updatedAt)
             VALUES ('Ana', 'Pérez', '12345678901', '+5355551234', 'Goyma', ?1, ?1)",
            [TS],
        ).unwrap();
        let client_id = connection.last_insert_rowid();
        let user_id = create_test_user(
            &connection,
            "searchuser",
            "contador",
            "+5350000011",
            "12345678902",
            "ValidPass1!",
        );
        connection.execute(
            "INSERT INTO invoices (type, number, invoiceDate, status, clientId, contactFullName, contactFullNameNormalized,
                contactCompany, contactNationalId, contactPhone, createdByUserId, createdByName, createdAt, updatedAt)
             VALUES ('V', 'V-00001', '2026-10-03', 'pending', ?1, 'Ana Pérez', 'ana perez', 'Goyma', '12345678901', '+5355551234', ?2, 'admin', ?3, ?3)",
            (client_id, user_id, TS),
        ).unwrap();
        let by_contact = search_invoices(&connection, "ana", 10).unwrap();
        assert_eq!(by_contact[0].number, "V-00001");
        assert_eq!(
            search_invoices(&connection, "v-00001", 10).unwrap().len(),
            1
        );
        assert!(search_invoices(&connection, "not found", 10)
            .unwrap()
            .is_empty());
    }
}
