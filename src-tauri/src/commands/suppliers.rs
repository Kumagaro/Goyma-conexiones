use chrono::{DateTime, Utc};
use rusqlite::{params, Connection, OptionalExtension};

use crate::commands::auth::{require_session, Author};
use crate::commands::products::purge_best_effort;
use crate::db::connection::{current_app_data_dir, lock_conn};
use crate::errors::{log_error, map_db_error, AppError};
use crate::models::client::ContactIndicators;
use crate::models::invoice::{AssociatedInvoice, InvoiceListItem};
use crate::models::supplier::{SupplierDetail, SupplierInput, SupplierListItem, SupplierMatch};
use crate::search::search_contacts;
use crate::text::spanish_sort_key;
use crate::timestamps::format_utc_timestamp;
use crate::validators::validate_supplier_input;

type SupplierDetailRow = (
    String,
    String,
    String,
    String,
    Option<String>,
    Option<String>,
    Option<String>,
    Option<String>,
    String,
);

pub fn list_suppliers_inner(
    connection: &mut Connection,
    now: DateTime<Utc>,
) -> Result<Vec<SupplierListItem>, AppError> {
    require_session(connection, now)?;

    let mut statement = connection
        .prepare("SELECT id, firstName, COALESCE(lastName, ''), company, COALESCE(phone, '') FROM suppliers ORDER BY id ASC")
        .map_err(|error| {
            AppError::new(
                "internal",
                "Ocurrió un error inesperado",
                &format!("failed to prepare list_suppliers query: {error}"),
            )
        })?;

    let rows = statement
        .query_map([], |row| {
            Ok((
                row.get::<_, i64>(0)?,
                row.get::<_, String>(1)?,
                row.get::<_, String>(2)?,
                row.get::<_, Option<String>>(3)?,
                row.get::<_, String>(4)?,
            ))
        })
        .map_err(|error| {
            AppError::new(
                "internal",
                "Ocurrió un error inesperado",
                &format!("failed to query suppliers: {error}"),
            )
        })?;

    let mut items = Vec::new();
    for row in rows {
        let (id, first_name, last_name, company, phone) = row.map_err(|error| {
            AppError::new(
                "internal",
                "Ocurrió un error inesperado",
                &format!("failed to map supplier row: {error}"),
            )
        })?;
        items.push((
            spanish_sort_key(&last_name),
            spanish_sort_key(&first_name),
            id,
            SupplierListItem {
                id,
                full_name: crate::text::full_name(&first_name, Some(&last_name)),
                company,
                phone,
            },
        ));
    }

    items.sort_by(|left, right| {
        left.0
            .cmp(&right.0)
            .then(left.1.cmp(&right.1))
            .then(left.2.cmp(&right.2))
    });

    Ok(items.into_iter().map(|(_, _, _, item)| item).collect())
}

#[tauri::command]
pub fn list_suppliers(
    state: tauri::State<'_, std::sync::Mutex<Connection>>,
) -> Result<Vec<SupplierListItem>, AppError> {
    let mut connection = lock_conn(&state);
    let now = Utc::now();
    let result = list_suppliers_inner(&mut connection, now);
    if let Err(error) = &result {
        if error.should_log() {
            if let Some(app_data_dir) = crate::db::connection::current_app_data_dir() {
                log_error(
                    &connection,
                    "suppliers",
                    "listSuppliers",
                    error,
                    now,
                    app_data_dir,
                );
            }
        }
    }
    result
}

pub fn search_suppliers_inner(
    connection: &mut Connection,
    now: DateTime<Utc>,
    query: &str,
) -> Result<Vec<SupplierMatch>, AppError> {
    require_session(connection, now)?;
    search_contacts(connection, "suppliers", query, 20)
}

pub fn create_supplier_inner(
    connection: &mut Connection,
    now: DateTime<Utc>,
    input: SupplierInput,
) -> Result<SupplierListItem, AppError> {
    require_session(connection, now)?;

    let validated = validate_supplier_input(crate::validators::ContactInput {
        first_name: &input.first_name,
        last_name: &input.last_name,
        national_id: &input.national_id,
        phone: &input.phone,
        company: input.company.as_deref(),
        position: input.position.as_deref(),
        email: input.email.as_deref(),
        address: input.address.as_deref(),
        invoice_info: None,
    })?;

    let timestamp = format_utc_timestamp(now);
    connection
        .execute(
            "INSERT INTO suppliers (firstName, lastName, nationalId, phone, company, position, email, address, createdAt, updatedAt)
             VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8, ?9, ?10)",
            params![
                validated.first_name,
                validated.last_name,
                validated.national_id,
                validated.phone,
                validated.company,
                validated.position,
                validated.email,
                validated.address,
                timestamp,
                timestamp,
            ],
        )
        .map_err(|error| {
            map_db_error(
                error,
                "createSupplier",
                &[("suppliers.nationalId", "Ya existe un proveedor con ese carnet de identidad")],
            )
        })?;

    let id = connection.last_insert_rowid();
    Ok(SupplierListItem {
        id,
        full_name: crate::text::full_name(&validated.first_name, validated.last_name.as_deref()),
        company: validated.company,
        phone: validated.phone.unwrap_or_default(),
    })
}

#[tauri::command]
pub fn create_supplier(
    state: tauri::State<'_, std::sync::Mutex<Connection>>,
    input: SupplierInput,
) -> Result<SupplierListItem, AppError> {
    let mut connection = lock_conn(&state);
    let now = Utc::now();
    let result = create_supplier_inner(&mut connection, now, input);
    if let Err(error) = &result {
        if error.should_log() {
            if let Some(app_data_dir) = crate::db::connection::current_app_data_dir() {
                log_error(
                    &connection,
                    "suppliers",
                    "createSupplier",
                    error,
                    now,
                    app_data_dir,
                );
            }
        }
    }
    result
}

pub fn update_supplier_inner(
    connection: &mut Connection,
    now: DateTime<Utc>,
    supplier_id: i64,
    input: SupplierInput,
) -> Result<SupplierListItem, AppError> {
    require_session(connection, now)?;

    let exists: Option<i64> = connection
        .query_row(
            "SELECT id FROM suppliers WHERE id = ?1",
            [supplier_id],
            |row| row.get(0),
        )
        .optional()
        .map_err(|error| map_db_error(error, "updateSupplier", &[]))?;
    if exists.is_none() {
        return Err(AppError::new(
            "notFound",
            "El proveedor ya no existe",
            "supplier missing during update",
        ));
    }

    let validated = validate_supplier_input(crate::validators::ContactInput {
        first_name: &input.first_name,
        last_name: &input.last_name,
        national_id: &input.national_id,
        phone: &input.phone,
        company: input.company.as_deref(),
        position: input.position.as_deref(),
        email: input.email.as_deref(),
        address: input.address.as_deref(),
        invoice_info: None,
    })?;

    let duplicate_id: Option<i64> = connection
        .query_row(
            "SELECT id FROM suppliers WHERE nationalId = ?1 AND id != ?2 LIMIT 1",
            params![validated.national_id, supplier_id],
            |row| row.get(0),
        )
        .optional()
        .map_err(|error| map_db_error(error, "updateSupplier", &[]))?;
    if duplicate_id.is_some() {
        return Err(AppError::new(
            "duplicateField",
            "Ya existe un proveedor con ese carnet de identidad",
            "duplicate nationalId while updating supplier",
        ));
    }

    let timestamp = format_utc_timestamp(now);
    connection
        .execute(
            "UPDATE suppliers
             SET firstName = ?1,
                 lastName = ?2,
                 nationalId = ?3,
                 phone = ?4,
                 company = ?5,
                 position = ?6,
                 email = ?7,
                 address = ?8,
                 updatedAt = ?9
             WHERE id = ?10",
            params![
                validated.first_name,
                validated.last_name,
                validated.national_id,
                validated.phone,
                validated.company,
                validated.position,
                validated.email,
                validated.address,
                timestamp,
                supplier_id,
            ],
        )
        .map_err(|error| {
            map_db_error(
                error,
                "updateSupplier",
                &[(
                    "suppliers.nationalId",
                    "Ya existe un proveedor con ese carnet de identidad",
                )],
            )
        })?;

    Ok(SupplierListItem {
        id: supplier_id,
        full_name: crate::text::full_name(&validated.first_name, validated.last_name.as_deref()),
        company: validated.company,
        phone: validated.phone.unwrap_or_default(),
    })
}

#[tauri::command]
pub fn update_supplier(
    state: tauri::State<'_, std::sync::Mutex<Connection>>,
    supplier_id: i64,
    input: SupplierInput,
) -> Result<SupplierListItem, AppError> {
    let mut connection = lock_conn(&state);
    let now = Utc::now();
    let result = update_supplier_inner(&mut connection, now, supplier_id, input);
    if let Err(error) = &result {
        if error.should_log() {
            if let Some(app_data_dir) = crate::db::connection::current_app_data_dir() {
                log_error(
                    &connection,
                    "suppliers",
                    "updateSupplier",
                    error,
                    now,
                    app_data_dir,
                );
            }
        }
    }
    result
}

pub fn delete_supplier_inner(
    connection: &mut Connection,
    now: DateTime<Utc>,
    supplier_id: i64,
) -> Result<(), AppError> {
    let actor = require_session(connection, now)?;

    let tx = connection
        .transaction()
        .map_err(|error| map_db_error(error, "deleteSupplier", &[]))?;

    let exists: Option<i64> = tx
        .query_row(
            "SELECT id FROM suppliers WHERE id = ?1",
            [supplier_id],
            |row| row.get(0),
        )
        .optional()
        .map_err(|error| map_db_error(error, "deleteSupplier", &[]))?;
    if exists.is_none() {
        return Err(AppError::new(
            "notFound",
            "El proveedor ya no existe",
            "supplier missing during deletion",
        ));
    }

    tx.execute(
        "UPDATE invoices SET supplierId = NULL WHERE supplierId = ?1",
        [supplier_id],
    )
    .map_err(|error| map_db_error(error, "deleteSupplier", &[]))?;

    crate::activity::record_deletion(
        &tx,
        &Author::User(&actor),
        now,
        "supplier",
        "SELECT trim(firstName || ' ' || COALESCE(lastName, '')) FROM suppliers WHERE id = ?1",
        supplier_id,
    )
    .map_err(|error| map_db_error(error, "deleteSupplier", &[]))?;

    tx.execute("DELETE FROM suppliers WHERE id = ?1", [supplier_id])
        .map_err(|error| map_db_error(error, "deleteSupplier", &[]))?;

    tx.commit()
        .map_err(|error| map_db_error(error, "deleteSupplier", &[]))?;

    Ok(())
}

#[tauri::command]
pub fn delete_supplier(
    state: tauri::State<'_, std::sync::Mutex<Connection>>,
    supplier_id: i64,
) -> Result<(), AppError> {
    let mut connection = lock_conn(&state);
    let now = Utc::now();
    let result = delete_supplier_inner(&mut connection, now, supplier_id);
    if let Err(error) = &result {
        if error.should_log() {
            if let Some(app_data_dir) = crate::db::connection::current_app_data_dir() {
                log_error(
                    &connection,
                    "suppliers",
                    "deleteSupplier",
                    error,
                    now,
                    app_data_dir,
                );
            }
        }
    }
    result
}

pub fn get_supplier_detail_inner(
    connection: &mut Connection,
    now: DateTime<Utc>,
    supplier_id: i64,
) -> Result<SupplierDetail, AppError> {
    require_session(connection, now)?;

    // Los indicadores se activan en la Fase 3: purga best-effort antes de leer.
    let expired_cleanup_warning = match purge_best_effort(connection, now) {
        Ok(warning) => warning,
        Err(error) => {
            if let Some(app_data_dir) = current_app_data_dir() {
                log_error(
                    connection,
                    "suppliers",
                    "getSupplierDetail",
                    &error,
                    now,
                    app_data_dir,
                );
            }
            Some(
                "No se pudieron limpiar los productos vencidos; se reintentará en la próxima operación"
                    .to_owned(),
            )
        }
    };

    let row: Option<SupplierDetailRow> = connection
        .query_row(
            "SELECT firstName, COALESCE(lastName, ''), COALESCE(nationalId, ''), COALESCE(phone, ''), company, position, email, address, createdAt
             FROM suppliers WHERE id = ?1",
            [supplier_id],
            |row| {
                Ok((
                    row.get(0)?,
                    row.get(1)?,
                    row.get(2)?,
                    row.get(3)?,
                    row.get(4)?,
                    row.get(5)?,
                    row.get(6)?,
                    row.get(7)?,
                    row.get(8)?,
                ))
            },
        )
        .optional()
        .map_err(|error| map_db_error(error, "getSupplierDetail", &[]))?;

    let Some((
        first_name,
        last_name,
        national_id,
        phone,
        company,
        position,
        email,
        address,
        created_at,
    )) = row
    else {
        return Err(AppError::new(
            "notFound",
            "El proveedor ya no existe",
            "supplier missing during detail lookup",
        ));
    };

    Ok(SupplierDetail {
        id: supplier_id,
        first_name: first_name.clone(),
        last_name: last_name.clone(),
        full_name: crate::text::full_name(&first_name, Some(&last_name)),
        national_id,
        phone,
        company,
        position,
        email,
        address,
        created_at,
        indicators: supplier_indicators(connection, supplier_id)?,
        invoice_history: supplier_invoice_history(connection, supplier_id)?,
        expired_cleanup_warning,
    })
}

/// Total comprado (entradas no canceladas), saldo Por Pagar, cantidad y
/// última fecha (PV-03).
fn supplier_indicators(
    connection: &Connection,
    supplier_id: i64,
) -> Result<ContactIndicators, AppError> {
    // Las facturas de un contacto eliminado quedan con supplierId NULL y no
    // entran en ningún cálculo.
    let row: (i64, i64, i64, Option<String>) = connection
        .query_row(
            "SELECT
                COALESCE(SUM(CASE WHEN i.status <> 'cancelled'
                    THEN (SELECT COALESCE(SUM(il.subtotalCents), 0) FROM invoiceLines il WHERE il.invoiceId = i.id)
                    ELSE 0 END), 0),
                COALESCE(SUM(CASE WHEN i.status = 'pending'
                    THEN (SELECT COALESCE(SUM(il.subtotalCents), 0) FROM invoiceLines il WHERE il.invoiceId = i.id)
                    ELSE 0 END), 0),
                COUNT(CASE WHEN i.status <> 'cancelled' THEN 1 END),
                MAX(CASE WHEN i.status <> 'cancelled' THEN i.invoiceDate END)
             FROM invoices i
             WHERE i.supplierId = ?1 AND i.type = 'E'",
            [supplier_id],
            |row| Ok((row.get(0)?, row.get(1)?, row.get(2)?, row.get(3)?)),
        )
        .map_err(|error| map_db_error(error, "getSupplierDetail", &[]))?;
    Ok(ContactIndicators {
        total_invoiced_cents: 0,
        total_purchased_cents: row.0,
        pending_balance_cents: row.1,
        invoice_count: row.2,
        last_purchase_date: row.3,
    })
}

/// Historial de compras del proveedor (entradas), de la más reciente a la
/// más antigua (PV-03).
fn supplier_invoice_history(
    connection: &Connection,
    supplier_id: i64,
) -> Result<Vec<InvoiceListItem>, AppError> {
    let mut statement = connection
        .prepare(
            "SELECT i.id, i.number, i.type, i.contactFullName, i.invoiceDate,
                COALESCE((SELECT SUM(il.subtotalCents) FROM invoiceLines il WHERE il.invoiceId = i.id), 0),
                i.status, i.associatedNumber, i.associatedType,
                (SELECT COUNT(*) FROM invoiceLines il WHERE il.invoiceId = i.id), i.createdByName
             FROM invoices i
             WHERE i.supplierId = ?1 AND i.type = 'E'
             ORDER BY i.invoiceDate DESC, i.id DESC",
        )
        .map_err(|error| map_db_error(error, "getSupplierDetail", &[]))?;
    let rows = statement
        .query_map([supplier_id], |row| {
            Ok((
                row.get::<_, i64>(0)?,
                row.get::<_, String>(1)?,
                row.get::<_, String>(2)?,
                row.get::<_, Option<String>>(3)?,
                row.get::<_, String>(4)?,
                row.get::<_, i64>(5)?,
                row.get::<_, Option<String>>(6)?,
                row.get::<_, Option<String>>(7)?,
                row.get::<_, Option<String>>(8)?,
                row.get::<_, i64>(9)?,
                row.get::<_, String>(10)?,
            ))
        })
        .map_err(|error| map_db_error(error, "getSupplierDetail", &[]))?
        .collect::<rusqlite::Result<Vec<_>>>()
        .map_err(|error| map_db_error(error, "getSupplierDetail", &[]))?;
    drop(statement);

    Ok(rows
        .into_iter()
        .map(
            |(
                id,
                number,
                invoice_type,
                contact_name,
                date,
                total_cents,
                status,
                associated_number,
                associated_type,
                product_count,
                created_by_name,
            )| {
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
                    product_name: None,
                    quantity: None,
                    product_count,
                    created_by_name,
                    reserved_until: None,
                }
            },
        )
        .collect())
}

#[tauri::command]
pub fn get_supplier_detail(
    state: tauri::State<'_, std::sync::Mutex<Connection>>,
    supplier_id: i64,
) -> Result<SupplierDetail, AppError> {
    let mut connection = lock_conn(&state);
    let now = Utc::now();
    let result = get_supplier_detail_inner(&mut connection, now, supplier_id);
    if let Err(error) = &result {
        if error.should_log() {
            if let Some(app_data_dir) = crate::db::connection::current_app_data_dir() {
                log_error(
                    &connection,
                    "suppliers",
                    "getSupplierDetail",
                    error,
                    now,
                    app_data_dir,
                );
            }
        }
    }
    result
}

#[tauri::command]
pub fn search_suppliers(
    state: tauri::State<'_, std::sync::Mutex<Connection>>,
    query: String,
) -> Result<Vec<SupplierMatch>, AppError> {
    let mut connection = lock_conn(&state);
    let now = Utc::now();
    let result = search_suppliers_inner(&mut connection, now, &query);
    if let Err(error) = &result {
        if error.should_log() {
            if let Some(app_data_dir) = crate::db::connection::current_app_data_dir() {
                log_error(
                    &connection,
                    "suppliers",
                    "searchSuppliers",
                    error,
                    now,
                    app_data_dir,
                );
            }
        }
    }
    result
}

#[cfg(test)]
mod tests {
    use super::{
        create_supplier_inner, delete_supplier_inner, get_supplier_detail_inner,
        list_suppliers_inner, search_suppliers_inner, update_supplier_inner,
    };
    use crate::models::supplier::SupplierInput;
    use crate::test_utils::{
        assert_allows_both_roles, assert_requires_session, create_test_session, create_test_user,
        insert_product_fixture, setup_test_db,
    };
    use chrono::{TimeZone, Utc};
    use rusqlite::OptionalExtension;

    #[test]
    fn list_suppliers_inner_returns_all_suppliers_sorted_by_last_name_then_first_name() {
        let now = Utc::now();
        let mut connection = setup_test_db();
        let user_id = create_test_user(
            &connection,
            "listsupplieruser",
            "contador",
            "+5350000101",
            "12345678951",
            "ValidPass1!",
        );
        create_test_session(&connection, user_id, false, now, now);

        connection
            .execute(
                "INSERT INTO suppliers (firstName, lastName, nationalId, phone, company, createdAt, updatedAt) VALUES (?, ?, ?, ?, ?, ?, ?)",
                (
                    "Ana",
                    "Pérez",
                    "12345678952",
                    "+5355550101",
                    "Café Norte",
                    "2026-10-03T00:00:00Z",
                    "2026-10-03T00:00:00Z",
                ),
            )
            .unwrap();
        connection
            .execute(
                "INSERT INTO suppliers (firstName, lastName, nationalId, phone, company, createdAt, updatedAt) VALUES (?, ?, ?, ?, ?, ?, ?)",
                (
                    "Luis",
                    "García",
                    "12345678953",
                    "+5355550102",
                    "García & Co",
                    "2026-10-03T00:00:00Z",
                    "2026-10-03T00:00:00Z",
                ),
            )
            .unwrap();

        let suppliers = list_suppliers_inner(&mut connection, now).unwrap();
        assert_eq!(
            suppliers
                .iter()
                .map(|supplier| supplier.id)
                .collect::<Vec<_>>(),
            vec![2, 1]
        );
        assert_eq!(suppliers[0].full_name, "Luis García");
        assert_eq!(suppliers[1].full_name, "Ana Pérez");
    }

    #[test]
    fn search_suppliers_inner_uses_name_phone_company_and_limit() {
        let now = Utc::now();
        let mut connection = setup_test_db();
        let user_id = create_test_user(
            &connection,
            "searchsupplieruser",
            "contador",
            "+5350000102",
            "12345678954",
            "ValidPass1!",
        );
        create_test_session(&connection, user_id, false, now, now);

        connection
            .execute(
                "INSERT INTO suppliers (firstName, lastName, nationalId, phone, company, createdAt, updatedAt) VALUES (?, ?, ?, ?, ?, ?, ?)",
                (
                    "Ana",
                    "Pérez",
                    "12345678955",
                    "+5355550103",
                    "Café Norte",
                    "2026-10-03T00:00:00Z",
                    "2026-10-03T00:00:00Z",
                ),
            )
            .unwrap();
        connection
            .execute(
                "INSERT INTO suppliers (firstName, lastName, nationalId, phone, company, createdAt, updatedAt) VALUES (?, ?, ?, ?, ?, ?, ?)",
                (
                    "María",
                    "Cano",
                    "12345678956",
                    "+5355550104",
                    None::<&str>,
                    "2026-10-03T00:00:00Z",
                    "2026-10-03T00:00:00Z",
                ),
            )
            .unwrap();

        let by_name = search_suppliers_inner(&mut connection, now, "ana").unwrap();
        assert_eq!(by_name[0].full_name, "Ana Pérez");
        let by_company = search_suppliers_inner(&mut connection, now, "cafe").unwrap();
        assert_eq!(by_company[0].full_name, "Ana Pérez");
        let by_phone = search_suppliers_inner(&mut connection, now, "+53 5555 0104").unwrap();
        assert_eq!(by_phone[0].full_name, "María Cano");
    }

    #[test]
    fn create_supplier_inner_validates_and_persists_supplier_data() {
        let now = Utc::now();
        let mut connection = setup_test_db();
        let user_id = create_test_user(
            &connection,
            "suppliercreator",
            "contador",
            "+5350000103",
            "12345678957",
            "ValidPass1!",
        );
        create_test_session(&connection, user_id, false, now, now);

        let input = SupplierInput {
            first_name: "Ana".to_owned(),
            last_name: "Pérez".to_owned(),
            national_id: "12345678958".to_owned(),
            phone: "+5355550105".to_owned(),
            company: Some("Café Norte".to_owned()),
            position: Some("Gerente".to_owned()),
            email: Some("ana@cafenorte.com".to_owned()),
            address: Some("Calle 5".to_owned()),
        };

        let supplier = create_supplier_inner(&mut connection, now, input).unwrap();
        assert_eq!(supplier.full_name, "Ana Pérez");
        assert_eq!(supplier.company, Some("Café Norte".to_owned()));

        let row: (String, String, String, Option<String>) = connection
            .query_row(
                "SELECT firstName, lastName, nationalId, company FROM suppliers WHERE id = ?1",
                [supplier.id],
                |row| Ok((row.get(0)?, row.get(1)?, row.get(2)?, row.get(3)?)),
            )
            .unwrap();
        assert_eq!(
            row,
            (
                "Ana".to_owned(),
                "Pérez".to_owned(),
                "12345678958".to_owned(),
                Some("Café Norte".to_owned())
            )
        );
    }

    #[test]
    fn update_supplier_inner_updates_existing_data_and_rejects_duplicates() {
        let now = Utc::now();
        let mut connection = setup_test_db();
        let user_id = create_test_user(
            &connection,
            "supplierupdater",
            "contador",
            "+5350000104",
            "12345678959",
            "ValidPass1!",
        );
        create_test_session(&connection, user_id, false, now, now);

        let created = create_supplier_inner(
            &mut connection,
            now,
            SupplierInput {
                first_name: "Ana".to_owned(),
                last_name: "Pérez".to_owned(),
                national_id: "12345678960".to_owned(),
                phone: "+5355550106".to_owned(),
                company: Some("Viejo".to_owned()),
                position: Some("Gerente".to_owned()),
                email: Some("ana@viejo.com".to_owned()),
                address: Some("Calle vieja".to_owned()),
            },
        )
        .unwrap();

        let updated = update_supplier_inner(
            &mut connection,
            now,
            created.id,
            SupplierInput {
                first_name: "Ana".to_owned(),
                last_name: "Pérez".to_owned(),
                national_id: "12345678960".to_owned(),
                phone: "+5355550107".to_owned(),
                company: Some("Nuevo".to_owned()),
                position: Some("Director".to_owned()),
                email: Some("ana@nuevo.com".to_owned()),
                address: Some("Calle nueva".to_owned()),
            },
        )
        .unwrap();
        assert_eq!(updated.company, Some("Nuevo".to_owned()));

        let _second = create_supplier_inner(
            &mut connection,
            now,
            SupplierInput {
                first_name: "Luis".to_owned(),
                last_name: "García".to_owned(),
                national_id: "12345678961".to_owned(),
                phone: "+5355550108".to_owned(),
                company: None,
                position: None,
                email: None,
                address: None,
            },
        )
        .unwrap();

        let duplicate = update_supplier_inner(
            &mut connection,
            now,
            created.id,
            SupplierInput {
                first_name: "Ana".to_owned(),
                last_name: "Pérez".to_owned(),
                national_id: "12345678961".to_owned(),
                phone: "+5355550109".to_owned(),
                company: None,
                position: None,
                email: None,
                address: None,
            },
        )
        .unwrap_err();
        assert_eq!(duplicate.code, "duplicateField");
        assert_eq!(
            duplicate.message,
            "Ya existe un proveedor con ese carnet de identidad"
        );
    }

    #[test]
    fn delete_supplier_inner_removes_row_and_nulls_invoice_reference() {
        let now = Utc::now();
        let mut connection = setup_test_db();
        let user_id = create_test_user(
            &connection,
            "supplierdeleter",
            "contador",
            "+5350000105",
            "12345678962",
            "ValidPass1!",
        );
        create_test_session(&connection, user_id, false, now, now);

        let supplier_id = create_supplier_inner(
            &mut connection,
            now,
            SupplierInput {
                first_name: "Ana".to_owned(),
                last_name: "Pérez".to_owned(),
                national_id: "12345678963".to_owned(),
                phone: "+5355550110".to_owned(),
                company: Some("Café Norte".to_owned()),
                position: Some("Gerente".to_owned()),
                email: Some("ana@cafenorte.com".to_owned()),
                address: Some("Calle 5".to_owned()),
            },
        )
        .unwrap()
        .id;

        connection
            .execute(
                "INSERT INTO invoices (type, number, invoiceDate, status, supplierId, contactFullName, contactFullNameNormalized, contactNationalId, contactPhone, createdByName, createdAt, updatedAt)
                 VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8, ?9, ?10, ?11, ?12)",
                (
                    "E",
                    "E-00001",
                    "2026-01-15",
                    "pending",
                    supplier_id,
                    "Ana Pérez",
                    "ana perez",
                    "12345678963",
                    "+5355550110",
                    "Admin",
                    "2026-10-03T00:00:00Z",
                    "2026-10-03T00:00:00Z",
                ),
            )
            .unwrap();

        delete_supplier_inner(&mut connection, now, supplier_id).unwrap();

        let remaining: Option<i64> = connection
            .query_row(
                "SELECT id FROM suppliers WHERE id = ?1",
                [supplier_id],
                |row| row.get(0),
            )
            .optional()
            .unwrap();
        assert!(remaining.is_none());

        let invoice_supplier_id: Option<Option<i64>> = connection
            .query_row(
                "SELECT supplierId FROM invoices WHERE number = ?1",
                ["E-00001"],
                |row| row.get::<_, Option<i64>>(0),
            )
            .optional()
            .unwrap();
        assert_eq!(invoice_supplier_id, Some(None));
    }

    #[test]
    fn get_supplier_detail_reports_indicators_and_history() {
        let now = Utc.with_ymd_and_hms(2026, 10, 4, 12, 0, 0).unwrap();
        let mut connection = setup_test_db();
        let user_id = create_test_user(
            &connection,
            "supplierindicators",
            "contador",
            "+5350000050",
            "12345678950",
            "ValidPass1!",
        );
        create_test_session(&connection, user_id, false, now, now);
        let created = create_supplier_inner(
            &mut connection,
            now,
            SupplierInput {
                first_name: "Ana".to_owned(),
                last_name: "Pérez".to_owned(),
                national_id: "12345678951".to_owned(),
                phone: "+5355550116".to_owned(),
                company: None,
                position: None,
                email: None,
                address: None,
            },
        )
        .unwrap();
        for (number, date, status, subtotal) in [
            ("E-00001", "2026-10-01", Some("pending"), 400),
            ("E-00002", "2026-10-03", Some("completed"), 600),
            ("E-00003", "2026-09-28", Some("cancelled"), 999),
        ] {
            connection
                .execute(
                    "INSERT INTO invoices (type, number, invoiceDate, status, supplierId, contactFullName,
                        contactFullNameNormalized, contactNationalId, contactPhone, paidAt,
                        createdByUserId, createdByName, createdAt, updatedAt)
                     VALUES ('E', ?1, ?2, ?3, ?4, 'Ana Pérez', 'ana perez', '12345678951', '+5355550116',
                        CASE WHEN ?3 = 'completed' THEN ?6 END, ?5, 'admin', ?6, ?6)",
                    rusqlite::params![
                        number,
                        date,
                        status,
                        created.id,
                        user_id,
                        crate::timestamps::format_utc_timestamp(now)
                    ],
                )
                .unwrap();
            let invoice_id = connection.last_insert_rowid();
            connection
                .execute(
                    "INSERT INTO invoiceLines (invoiceId, position, productIdOriginal, productName,
                        productNameNormalized, quantityMilli, unitPriceCents, subtotalCents, createdAt, updatedAt)
                     VALUES (?1, 1, 5001, 'Producto', 'producto', 1000, ?2, ?2, ?3, ?3)",
                    rusqlite::params![
                        invoice_id,
                        subtotal,
                        crate::timestamps::format_utc_timestamp(now)
                    ],
                )
                .unwrap();
        }

        let detail = get_supplier_detail_inner(&mut connection, now, created.id).unwrap();
        assert_eq!(detail.indicators.total_purchased_cents, 1000);
        assert_eq!(detail.indicators.total_invoiced_cents, 0);
        assert_eq!(detail.indicators.pending_balance_cents, 400);
        assert_eq!(detail.indicators.invoice_count, 2);
        assert_eq!(
            detail.indicators.last_purchase_date.as_deref(),
            Some("2026-10-03")
        );
        assert_eq!(
            detail
                .invoice_history
                .iter()
                .map(|item| item.number.as_str())
                .collect::<Vec<_>>(),
            vec!["E-00002", "E-00001", "E-00003"]
        );
    }

    #[test]
    fn get_supplier_detail_purges_before_reading() {
        let now = Utc.with_ymd_and_hms(2026, 10, 4, 12, 0, 0).unwrap();
        let mut connection = setup_test_db();
        let user_id = create_test_user(
            &connection,
            "supplierpurgeuser",
            "contador",
            "+5350000052",
            "12345678952",
            "ValidPass1!",
        );
        create_test_session(&connection, user_id, false, now, now);
        let created = create_supplier_inner(
            &mut connection,
            now,
            SupplierInput {
                first_name: "Luis".to_owned(),
                last_name: "García".to_owned(),
                national_id: "12345678953".to_owned(),
                phone: "+5355550117".to_owned(),
                company: None,
                position: None,
                email: None,
                address: None,
            },
        )
        .unwrap();
        let expired_id = insert_product_fixture(
            &connection,
            "Producto vencido",
            None,
            None,
            100,
            0,
            1_000,
            "discontinued",
            Some(&crate::timestamps::format_utc_timestamp(
                now - chrono::Duration::days(61),
            )),
            None,
        );

        get_supplier_detail_inner(&mut connection, now, created.id).unwrap();
        let still_exists: bool = connection
            .query_row(
                "SELECT EXISTS(SELECT 1 FROM products WHERE id = ?1)",
                [expired_id],
                |row| row.get(0),
            )
            .unwrap();
        assert!(!still_exists);
    }

    #[test]
    fn get_supplier_detail_ignores_deleted_contact_invoices() {
        let now = Utc.with_ymd_and_hms(2026, 10, 4, 12, 0, 0).unwrap();
        let mut connection = setup_test_db();
        let user_id = create_test_user(
            &connection,
            "supplierdeleteduser",
            "contador",
            "+5350000054",
            "12345678954",
            "ValidPass1!",
        );
        create_test_session(&connection, user_id, false, now, now);
        let doomed = create_supplier_inner(
            &mut connection,
            now,
            SupplierInput {
                first_name: "Pedro".to_owned(),
                last_name: "Ruiz".to_owned(),
                national_id: "12345678955".to_owned(),
                phone: "+5355550118".to_owned(),
                company: None,
                position: None,
                email: None,
                address: None,
            },
        )
        .unwrap();
        let survivor = create_supplier_inner(
            &mut connection,
            now,
            SupplierInput {
                first_name: "Marta".to_owned(),
                last_name: "León".to_owned(),
                national_id: "12345678956".to_owned(),
                phone: "+5355550119".to_owned(),
                company: None,
                position: None,
                email: None,
                address: None,
            },
        )
        .unwrap();
        connection
            .execute(
                "INSERT INTO invoices (type, number, invoiceDate, status, supplierId, contactFullName,
                    contactFullNameNormalized, contactNationalId, contactPhone, createdByUserId,
                    createdByName, createdAt, updatedAt)
                 VALUES ('E', 'E-00999', '2026-10-01', 'pending', ?1, 'Pedro Ruiz', 'pedro ruiz',
                    '12345678955', '+5355550118', ?2, 'admin', ?3, ?3)",
                rusqlite::params![
                    doomed.id,
                    user_id,
                    crate::timestamps::format_utc_timestamp(now)
                ],
            )
            .unwrap();
        delete_supplier_inner(&mut connection, now, doomed.id).unwrap();

        let detail = get_supplier_detail_inner(&mut connection, now, survivor.id).unwrap();
        assert_eq!(detail.indicators.total_purchased_cents, 0);
        assert_eq!(detail.indicators.invoice_count, 0);
        assert!(detail.invoice_history.is_empty());
        let missing = get_supplier_detail_inner(&mut connection, now, doomed.id).unwrap_err();
        assert_eq!(missing.code, "notFound");
    }

    #[test]
    fn get_supplier_detail_inner_returns_profile_with_zero_indicators_and_empty_history() {
        let now = Utc::now();
        let mut connection = setup_test_db();
        let user_id = create_test_user(
            &connection,
            "supplierdetailuser",
            "contador",
            "+5350000106",
            "12345678964",
            "ValidPass1!",
        );
        create_test_session(&connection, user_id, false, now, now);

        let created = create_supplier_inner(
            &mut connection,
            now,
            SupplierInput {
                first_name: "Ana".to_owned(),
                last_name: "Pérez".to_owned(),
                national_id: "12345678965".to_owned(),
                phone: "+5355550111".to_owned(),
                company: Some("Café Norte".to_owned()),
                position: Some("Gerente".to_owned()),
                email: Some("ana@cafenorte.com".to_owned()),
                address: Some("Calle 5".to_owned()),
            },
        )
        .unwrap();

        let detail = get_supplier_detail_inner(&mut connection, now, created.id).unwrap();
        assert_eq!(detail.id, created.id);
        assert_eq!(detail.full_name, "Ana Pérez");
        assert_eq!(detail.indicators.total_invoiced_cents, 0);
        assert_eq!(detail.indicators.invoice_count, 0);
        assert!(detail.invoice_history.is_empty());
        assert_eq!(detail.expired_cleanup_warning, None);

        let missing = get_supplier_detail_inner(&mut connection, now, 9999).unwrap_err();
        assert_eq!(missing.code, "notFound");
    }

    #[test]
    fn suppliers_require_session_and_allow_both_roles() {
        let sample = SupplierInput {
            first_name: "Ana".to_owned(),
            last_name: "Pérez".to_owned(),
            national_id: "12345678966".to_owned(),
            phone: "+5355550112".to_owned(),
            company: None,
            position: None,
            email: None,
            address: None,
        };
        assert_requires_session(list_suppliers_inner);
        assert_allows_both_roles(list_suppliers_inner);
        assert_requires_session(|connection, now| search_suppliers_inner(connection, now, "ana"));
        assert_allows_both_roles(|connection, now| search_suppliers_inner(connection, now, "ana"));
        assert_requires_session(|connection, now| {
            create_supplier_inner(connection, now, sample.clone())
        });
        assert_allows_both_roles(|connection, now| {
            create_supplier_inner(connection, now, sample.clone())
        });
        assert_requires_session(|connection, now| {
            update_supplier_inner(connection, now, 1, sample.clone())
        });
        assert_allows_both_roles(|connection, now| {
            update_supplier_inner(connection, now, 1, sample.clone())
        });
        assert_requires_session(|connection, now| delete_supplier_inner(connection, now, 1));
        assert_allows_both_roles(|connection, now| delete_supplier_inner(connection, now, 1));
        assert_requires_session(|connection, now| get_supplier_detail_inner(connection, now, 1));
        assert_allows_both_roles(|connection, now| get_supplier_detail_inner(connection, now, 1));
    }
}
