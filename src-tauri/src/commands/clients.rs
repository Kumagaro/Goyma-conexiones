use chrono::{DateTime, Utc};
use rusqlite::{params, Connection, OptionalExtension};

use crate::commands::auth::require_session;
use crate::commands::products::purge_best_effort;
use crate::db::connection::{current_app_data_dir, lock_conn};
use crate::errors::{log_error, map_db_error, AppError};
use crate::models::client::{
    ContactDetail, ContactIndicators, ContactInput, ContactListItem, ContactMatch,
};
use crate::models::invoice::{AssociatedInvoice, InvoiceListItem};
use crate::search::search_contacts;
use crate::text::spanish_sort_key;
use crate::timestamps::format_utc_timestamp;
use crate::validators::validate_client_input;

type ClientDetailRow = (
    String,
    String,
    String,
    String,
    Option<String>,
    Option<String>,
    Option<String>,
    Option<String>,
    Option<String>,
    String,
);

pub fn list_clients_inner(
    connection: &mut Connection,
    now: DateTime<Utc>,
) -> Result<Vec<ContactListItem>, AppError> {
    require_session(connection, now)?;

    let mut statement = connection
        .prepare("SELECT id, firstName, lastName, company, phone FROM clients ORDER BY id ASC")
        .map_err(|error| {
            AppError::new(
                "internal",
                "Ocurrió un error inesperado",
                &format!("failed to prepare list_clients query: {error}"),
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
                &format!("failed to query clients: {error}"),
            )
        })?;

    let mut items = Vec::new();
    for row in rows {
        let (id, first_name, last_name, company, phone) = row.map_err(|error| {
            AppError::new(
                "internal",
                "Ocurrió un error inesperado",
                &format!("failed to map client row: {error}"),
            )
        })?;
        items.push((
            spanish_sort_key(&last_name),
            spanish_sort_key(&first_name),
            id,
            ContactListItem {
                id,
                full_name: format!("{first_name} {last_name}"),
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
pub fn list_clients(
    state: tauri::State<'_, std::sync::Mutex<Connection>>,
) -> Result<Vec<ContactListItem>, AppError> {
    let mut connection = lock_conn(&state);
    let now = Utc::now();
    let result = list_clients_inner(&mut connection, now);
    if let Err(error) = &result {
        if error.should_log() {
            if let Some(app_data_dir) = crate::db::connection::current_app_data_dir() {
                log_error(
                    &connection,
                    "clients",
                    "listClients",
                    error,
                    now,
                    app_data_dir,
                );
            }
        }
    }
    result
}

pub fn search_clients_inner(
    connection: &mut Connection,
    now: DateTime<Utc>,
    query: &str,
) -> Result<Vec<ContactMatch>, AppError> {
    require_session(connection, now)?;
    search_contacts(connection, "clients", query, 20)
}

pub fn create_client_inner(
    connection: &mut Connection,
    now: DateTime<Utc>,
    input: ContactInput,
) -> Result<ContactListItem, AppError> {
    require_session(connection, now)?;

    let validated = validate_client_input(crate::validators::ContactInput {
        first_name: &input.first_name,
        last_name: &input.last_name,
        national_id: &input.national_id,
        phone: &input.phone,
        company: input.company.as_deref(),
        position: input.position.as_deref(),
        email: input.email.as_deref(),
        address: input.address.as_deref(),
        invoice_info: input.invoice_info.as_deref(),
    })?;

    let timestamp = format_utc_timestamp(now);
    connection
        .execute(
            "INSERT INTO clients (firstName, lastName, nationalId, phone, company, position, email, address, invoiceInfo, createdAt, updatedAt)
             VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8, ?9, ?10, ?11)",
            params![
                validated.first_name,
                validated.last_name,
                validated.national_id,
                validated.phone,
                validated.company,
                validated.position,
                validated.email,
                validated.address,
                validated.invoice_info,
                timestamp,
                timestamp,
            ],
        )
        .map_err(|error| {
            map_db_error(
                error,
                "createClient",
                &[("clients.nationalId", "Ya existe un cliente con ese carnet de identidad")],
            )
        })?;

    let id = connection.last_insert_rowid();
    Ok(ContactListItem {
        id,
        full_name: format!("{} {}", validated.first_name, validated.last_name),
        company: validated.company,
        phone: validated.phone,
    })
}

#[tauri::command]
pub fn create_client(
    state: tauri::State<'_, std::sync::Mutex<Connection>>,
    input: ContactInput,
) -> Result<ContactListItem, AppError> {
    let mut connection = lock_conn(&state);
    let now = Utc::now();
    let result = create_client_inner(&mut connection, now, input);
    if let Err(error) = &result {
        if error.should_log() {
            if let Some(app_data_dir) = crate::db::connection::current_app_data_dir() {
                log_error(
                    &connection,
                    "clients",
                    "createClient",
                    error,
                    now,
                    app_data_dir,
                );
            }
        }
    }
    result
}

pub fn update_client_inner(
    connection: &mut Connection,
    now: DateTime<Utc>,
    client_id: i64,
    input: ContactInput,
) -> Result<ContactListItem, AppError> {
    require_session(connection, now)?;

    let exists: Option<i64> = connection
        .query_row("SELECT id FROM clients WHERE id = ?1", [client_id], |row| {
            row.get(0)
        })
        .optional()
        .map_err(|error| map_db_error(error, "updateClient", &[]))?;
    if exists.is_none() {
        return Err(AppError::new(
            "notFound",
            "El cliente ya no existe",
            "client missing during update",
        ));
    }

    let validated = validate_client_input(crate::validators::ContactInput {
        first_name: &input.first_name,
        last_name: &input.last_name,
        national_id: &input.national_id,
        phone: &input.phone,
        company: input.company.as_deref(),
        position: input.position.as_deref(),
        email: input.email.as_deref(),
        address: input.address.as_deref(),
        invoice_info: input.invoice_info.as_deref(),
    })?;

    let duplicate_id: Option<i64> = connection
        .query_row(
            "SELECT id FROM clients WHERE nationalId = ?1 AND id != ?2 LIMIT 1",
            params![validated.national_id, client_id],
            |row| row.get(0),
        )
        .optional()
        .map_err(|error| map_db_error(error, "updateClient", &[]))?;
    if duplicate_id.is_some() {
        return Err(AppError::new(
            "duplicateField",
            "Ya existe un cliente con ese carnet de identidad",
            "duplicate nationalId while updating client",
        ));
    }

    let timestamp = format_utc_timestamp(now);
    connection
        .execute(
            "UPDATE clients
             SET firstName = ?1,
                 lastName = ?2,
                 nationalId = ?3,
                 phone = ?4,
                 company = ?5,
                 position = ?6,
                 email = ?7,
                 address = ?8,
                 invoiceInfo = ?9,
                 updatedAt = ?10
             WHERE id = ?11",
            params![
                validated.first_name,
                validated.last_name,
                validated.national_id,
                validated.phone,
                validated.company,
                validated.position,
                validated.email,
                validated.address,
                validated.invoice_info,
                timestamp,
                client_id,
            ],
        )
        .map_err(|error| {
            map_db_error(
                error,
                "updateClient",
                &[(
                    "clients.nationalId",
                    "Ya existe un cliente con ese carnet de identidad",
                )],
            )
        })?;

    Ok(ContactListItem {
        id: client_id,
        full_name: format!("{} {}", validated.first_name, validated.last_name),
        company: validated.company,
        phone: validated.phone,
    })
}

#[tauri::command]
pub fn update_client(
    state: tauri::State<'_, std::sync::Mutex<Connection>>,
    client_id: i64,
    input: ContactInput,
) -> Result<ContactListItem, AppError> {
    let mut connection = lock_conn(&state);
    let now = Utc::now();
    let result = update_client_inner(&mut connection, now, client_id, input);
    if let Err(error) = &result {
        if error.should_log() {
            if let Some(app_data_dir) = crate::db::connection::current_app_data_dir() {
                log_error(
                    &connection,
                    "clients",
                    "updateClient",
                    error,
                    now,
                    app_data_dir,
                );
            }
        }
    }
    result
}

pub fn delete_client_inner(
    connection: &mut Connection,
    now: DateTime<Utc>,
    client_id: i64,
) -> Result<(), AppError> {
    require_session(connection, now)?;

    let tx = connection
        .transaction()
        .map_err(|error| map_db_error(error, "deleteClient", &[]))?;

    let exists: Option<i64> = tx
        .query_row("SELECT id FROM clients WHERE id = ?1", [client_id], |row| {
            row.get(0)
        })
        .optional()
        .map_err(|error| map_db_error(error, "deleteClient", &[]))?;
    if exists.is_none() {
        return Err(AppError::new(
            "notFound",
            "El cliente ya no existe",
            "client missing during deletion",
        ));
    }

    tx.execute(
        "UPDATE invoices SET clientId = NULL WHERE clientId = ?1",
        [client_id],
    )
    .map_err(|error| map_db_error(error, "deleteClient", &[]))?;

    tx.execute("DELETE FROM clients WHERE id = ?1", [client_id])
        .map_err(|error| map_db_error(error, "deleteClient", &[]))?;

    tx.commit()
        .map_err(|error| map_db_error(error, "deleteClient", &[]))?;

    Ok(())
}

#[tauri::command]
pub fn delete_client(
    state: tauri::State<'_, std::sync::Mutex<Connection>>,
    client_id: i64,
) -> Result<(), AppError> {
    let mut connection = lock_conn(&state);
    let now = Utc::now();
    let result = delete_client_inner(&mut connection, now, client_id);
    if let Err(error) = &result {
        if error.should_log() {
            if let Some(app_data_dir) = crate::db::connection::current_app_data_dir() {
                log_error(
                    &connection,
                    "clients",
                    "deleteClient",
                    error,
                    now,
                    app_data_dir,
                );
            }
        }
    }
    result
}

pub fn get_client_detail_inner(
    connection: &mut Connection,
    now: DateTime<Utc>,
    client_id: i64,
) -> Result<ContactDetail, AppError> {
    require_session(connection, now)?;

    // Los indicadores se activan en la Fase 3: purga best-effort antes de leer.
    let expired_cleanup_warning = match purge_best_effort(connection, now) {
        Ok(warning) => warning,
        Err(error) => {
            if let Some(app_data_dir) = current_app_data_dir() {
                log_error(
                    connection,
                    "clients",
                    "getClientDetail",
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

    let row: Option<ClientDetailRow> = connection
        .query_row(
            "SELECT firstName, lastName, nationalId, phone, company, position, email, address, invoiceInfo, createdAt
             FROM clients WHERE id = ?1",
            [client_id],
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
                    row.get(9)?,
                ))
            },
        )
        .optional()
        .map_err(|error| map_db_error(error, "getClientDetail", &[]))?;

    let Some((
        first_name,
        last_name,
        national_id,
        phone,
        company,
        position,
        email,
        address,
        invoice_info,
        created_at,
    )) = row
    else {
        return Err(AppError::new(
            "notFound",
            "El cliente ya no existe",
            "client missing during detail lookup",
        ));
    };

    Ok(ContactDetail {
        id: client_id,
        first_name: first_name.clone(),
        last_name: last_name.clone(),
        full_name: format!("{} {}", first_name, last_name),
        national_id,
        phone,
        company,
        position,
        email,
        address,
        invoice_info,
        created_at,
        indicators: client_indicators(connection, client_id)?,
        invoice_history: client_invoice_history(connection, client_id)?,
        expired_cleanup_warning,
    })
}

/// Total facturado (ventas y servicios no cancelados), saldo Por Cobrar,
/// cantidad y última fecha. Garantías e insumos no se cobran (CL-03).
fn client_indicators(
    connection: &Connection,
    client_id: i64,
) -> Result<ContactIndicators, AppError> {
    // Las facturas de un contacto eliminado quedan con clientId NULL y no
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
             WHERE i.clientId = ?1 AND i.type IN ('V', 'S')",
            [client_id],
            |row| Ok((row.get(0)?, row.get(1)?, row.get(2)?, row.get(3)?)),
        )
        .map_err(|error| map_db_error(error, "getClientDetail", &[]))?;
    Ok(ContactIndicators {
        total_invoiced_cents: row.0,
        total_purchased_cents: 0,
        pending_balance_cents: row.1,
        invoice_count: row.2,
        last_purchase_date: row.3,
    })
}

/// Historial del cliente (ventas, servicios, garantías e insumos), de la
/// más reciente a la más antigua (CL-03).
fn client_invoice_history(
    connection: &Connection,
    client_id: i64,
) -> Result<Vec<InvoiceListItem>, AppError> {
    let mut statement = connection
        .prepare(
            "SELECT i.id, i.number, i.type, i.contactFullName, i.invoiceDate,
                COALESCE((SELECT SUM(il.subtotalCents) FROM invoiceLines il WHERE il.invoiceId = i.id), 0),
                i.status, i.associatedNumber, i.associatedType
             FROM invoices i
             WHERE i.clientId = ?1 AND i.type IN ('V', 'S', 'GG', 'GI')
             ORDER BY i.invoiceDate DESC, i.id DESC",
        )
        .map_err(|error| map_db_error(error, "getClientDetail", &[]))?;
    let rows = statement
        .query_map([client_id], |row| {
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
            ))
        })
        .map_err(|error| map_db_error(error, "getClientDetail", &[]))?
        .collect::<rusqlite::Result<Vec<_>>>()
        .map_err(|error| map_db_error(error, "getClientDetail", &[]))?;
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
                }
            },
        )
        .collect())
}

#[tauri::command]
pub fn get_client_detail(
    state: tauri::State<'_, std::sync::Mutex<Connection>>,
    client_id: i64,
) -> Result<ContactDetail, AppError> {
    let mut connection = lock_conn(&state);
    let now = Utc::now();
    let result = get_client_detail_inner(&mut connection, now, client_id);
    if let Err(error) = &result {
        if error.should_log() {
            if let Some(app_data_dir) = crate::db::connection::current_app_data_dir() {
                log_error(
                    &connection,
                    "clients",
                    "getClientDetail",
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
pub fn search_clients(
    state: tauri::State<'_, std::sync::Mutex<Connection>>,
    query: String,
) -> Result<Vec<ContactMatch>, AppError> {
    let mut connection = lock_conn(&state);
    let now = Utc::now();
    let result = search_clients_inner(&mut connection, now, &query);
    if let Err(error) = &result {
        if error.should_log() {
            if let Some(app_data_dir) = crate::db::connection::current_app_data_dir() {
                log_error(
                    &connection,
                    "clients",
                    "searchClients",
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
        create_client_inner, delete_client_inner, get_client_detail_inner, list_clients_inner,
        search_clients_inner, update_client_inner,
    };
    use crate::models::client::ContactInput;

    type ClientPersistedRow = (
        String,
        String,
        String,
        Option<String>,
        Option<String>,
        Option<String>,
        Option<String>,
    );
    use crate::test_utils::{
        assert_allows_both_roles, assert_requires_session, create_test_session, create_test_user,
        insert_product_fixture, setup_test_db,
    };
    use chrono::Utc;
    use rusqlite::OptionalExtension;

    const TS: &str = "2026-10-03T00:00:00Z";

    #[test]
    fn list_clients_inner_returns_all_clients_sorted_by_last_name_then_first_name() {
        let now = Utc::now();
        let mut connection = setup_test_db();
        let user_id = create_test_user(
            &connection,
            "listclientuser",
            "contador",
            "+5350000001",
            "12345678901",
            "ValidPass1!",
        );
        create_test_session(&connection, user_id, false, now, now);

        connection
            .execute(
                "INSERT INTO clients (firstName, lastName, nationalId, phone, company, createdAt, updatedAt) VALUES (?, ?, ?, ?, ?, ?, ?)",
                (
                    "Ana",
                    "Pérez",
                    "12345678902",
                    "+5355550001",
                    "Café Norte",
                    TS,
                    TS,
                ),
            )
            .unwrap();
        connection
            .execute(
                "INSERT INTO clients (firstName, lastName, nationalId, phone, company, createdAt, updatedAt) VALUES (?, ?, ?, ?, ?, ?, ?)",
                (
                    "Luis",
                    "García",
                    "12345678903",
                    "+5355550002",
                    "García & Co",
                    TS,
                    TS,
                ),
            )
            .unwrap();
        connection
            .execute(
                "INSERT INTO clients (firstName, lastName, nationalId, phone, company, createdAt, updatedAt) VALUES (?, ?, ?, ?, ?, ?, ?)",
                (
                    "Marta",
                    "Aldama",
                    "12345678904",
                    "+5355550003",
                    "Dulce",
                    TS,
                    TS,
                ),
            )
            .unwrap();

        let clients = list_clients_inner(&mut connection, now).unwrap();
        assert_eq!(
            clients.iter().map(|client| client.id).collect::<Vec<_>>(),
            vec![3, 2, 1]
        );
        assert_eq!(clients[0].full_name, "Marta Aldama");
        assert_eq!(clients[1].full_name, "Luis García");
        assert_eq!(clients[2].full_name, "Ana Pérez");
    }

    #[test]
    fn search_clients_inner_uses_name_phone_company_and_limit() {
        let now = Utc::now();
        let mut connection = setup_test_db();
        let user_id = create_test_user(
            &connection,
            "searchclientuser",
            "contador",
            "+5350000002",
            "12345678905",
            "ValidPass1!",
        );
        create_test_session(&connection, user_id, false, now, now);

        connection
            .execute(
                "INSERT INTO clients (firstName, lastName, nationalId, phone, company, createdAt, updatedAt) VALUES (?, ?, ?, ?, ?, ?, ?)",
                (
                    "Ana",
                    "Pérez",
                    "12345678906",
                    "+5355551234",
                    "Café Norte",
                    TS,
                    TS,
                ),
            )
            .unwrap();
        connection
            .execute(
                "INSERT INTO clients (firstName, lastName, nationalId, phone, company, createdAt, updatedAt) VALUES (?, ?, ?, ?, ?, ?, ?)",
                (
                    "María",
                    "Cano",
                    "12345678907",
                    "+5355559876",
                    None::<&str>,
                    TS,
                    TS,
                ),
            )
            .unwrap();

        let by_name = search_clients_inner(&mut connection, now, "ana").unwrap();
        assert_eq!(by_name[0].full_name, "Ana Pérez");
        assert!(by_name[0].is_exact_match);

        let by_company = search_clients_inner(&mut connection, now, "cafe").unwrap();
        assert_eq!(by_company[0].full_name, "Ana Pérez");

        let by_phone = search_clients_inner(&mut connection, now, "+53 5555 9876").unwrap();
        assert_eq!(by_phone[0].full_name, "María Cano");

        let empty = search_clients_inner(&mut connection, now, "").unwrap();
        assert_eq!(empty.len(), 2);
    }

    #[test]
    fn create_client_inner_validates_and_persists_client_data() {
        let now = Utc::now();
        let mut connection = setup_test_db();
        let user_id = create_test_user(
            &connection,
            "clientcreator",
            "contador",
            "+5350000003",
            "12345678908",
            "ValidPass1!",
        );
        create_test_session(&connection, user_id, false, now, now);

        let input = ContactInput {
            first_name: "Ana".to_owned(),
            last_name: "Pérez".to_owned(),
            national_id: "12345678909".to_owned(),
            phone: "+5355550004".to_owned(),
            company: Some("Café Norte".to_owned()),
            position: Some("Gerente".to_owned()),
            email: Some("ana@cafenorte.com".to_owned()),
            address: Some("Calle 5".to_owned()),
            invoice_info: Some("Factura de prueba".to_owned()),
        };

        let client = create_client_inner(&mut connection, now, input).unwrap();
        assert_eq!(client.full_name, "Ana Pérez");
        assert_eq!(client.company, Some("Café Norte".to_owned()));
        assert_eq!(client.phone, "+5355550004");

        let row: ClientPersistedRow = connection
            .query_row(
                "SELECT firstName, lastName, nationalId, company, email, address, invoiceInfo FROM clients WHERE id = ?1",
                [client.id],
                |row| Ok((row.get(0)?, row.get(1)?, row.get(2)?, row.get(3)?, row.get(4)?, row.get(5)?, row.get(6)?)),
            )
            .unwrap();
        assert_eq!(row.0, "Ana");
        assert_eq!(row.1, "Pérez");
        assert_eq!(row.2, "12345678909");
        assert_eq!(row.3, Some("Café Norte".to_owned()));
        assert_eq!(row.4, Some("ana@cafenorte.com".to_owned()));
        assert_eq!(row.5, Some("Calle 5".to_owned()));
        assert_eq!(row.6, Some("Factura de prueba".to_owned()));

        let minimal = ContactInput {
            first_name: "Luis".to_owned(),
            last_name: "García".to_owned(),
            national_id: "12345678910".to_owned(),
            phone: "+5355550005".to_owned(),
            company: None,
            position: None,
            email: None,
            address: None,
            invoice_info: None,
        };
        let second = create_client_inner(&mut connection, now, minimal).unwrap();
        assert_eq!(second.company, None);
    }

    #[test]
    fn create_client_inner_rejects_duplicate_cis_and_invalid_data() {
        let now = Utc::now();
        let mut connection = setup_test_db();
        let user_id = create_test_user(
            &connection,
            "clientvalidator",
            "contador",
            "+5350000004",
            "12345678911",
            "ValidPass1!",
        );
        create_test_session(&connection, user_id, false, now, now);

        let first = ContactInput {
            first_name: "Ana".to_owned(),
            last_name: "Pérez".to_owned(),
            national_id: "12345678912".to_owned(),
            phone: "+5355550006".to_owned(),
            company: None,
            position: None,
            email: None,
            address: None,
            invoice_info: None,
        };
        create_client_inner(&mut connection, now, first).unwrap();

        let duplicate = ContactInput {
            first_name: "Marta".to_owned(),
            last_name: "López".to_owned(),
            national_id: "12345678912".to_owned(),
            phone: "+5355550007".to_owned(),
            company: None,
            position: None,
            email: None,
            address: None,
            invoice_info: None,
        };
        let duplicate_error = create_client_inner(&mut connection, now, duplicate).unwrap_err();
        assert_eq!(duplicate_error.code, "duplicateField");
        assert_eq!(
            duplicate_error.message,
            "Ya existe un cliente con ese carnet de identidad"
        );

        let with_same_phone = ContactInput {
            first_name: "Luis".to_owned(),
            last_name: "García".to_owned(),
            national_id: "12345678913".to_owned(),
            phone: "+5355550006".to_owned(),
            company: None,
            position: None,
            email: None,
            address: None,
            invoice_info: None,
        };
        assert!(create_client_inner(&mut connection, now, with_same_phone).is_ok());

        let invalid_name = ContactInput {
            first_name: "A".to_owned(),
            last_name: "Pérez".to_owned(),
            national_id: "12345678914".to_owned(),
            phone: "+5355550008".to_owned(),
            company: None,
            position: None,
            email: None,
            address: None,
            invoice_info: None,
        };
        assert_eq!(
            create_client_inner(&mut connection, now, invalid_name)
                .unwrap_err()
                .code,
            "validationFailed"
        );
    }

    #[test]
    fn update_client_inner_updates_existing_data_and_rejects_duplicates() {
        let now = Utc::now();
        let mut connection = setup_test_db();
        let user_id = create_test_user(
            &connection,
            "clientupdater",
            "contador",
            "+5350000005",
            "12345678915",
            "ValidPass1!",
        );
        create_test_session(&connection, user_id, false, now, now);

        let created = create_client_inner(
            &mut connection,
            now,
            ContactInput {
                first_name: "Ana".to_owned(),
                last_name: "Pérez".to_owned(),
                national_id: "12345678916".to_owned(),
                phone: "+5355550010".to_owned(),
                company: Some("Viejo".to_owned()),
                position: Some("Gerente".to_owned()),
                email: Some("ana@viejo.com".to_owned()),
                address: Some("Calle vieja".to_owned()),
                invoice_info: Some("Factura vieja".to_owned()),
            },
        )
        .unwrap();

        let same_id = update_client_inner(
            &mut connection,
            now,
            created.id,
            ContactInput {
                first_name: "Ana".to_owned(),
                last_name: "Pérez".to_owned(),
                national_id: "12345678916".to_owned(),
                phone: "+5355550011".to_owned(),
                company: Some("Nuevo".to_owned()),
                position: Some("Director".to_owned()),
                email: Some("ana@nuevo.com".to_owned()),
                address: Some("Calle nueva".to_owned()),
                invoice_info: Some("Factura nueva".to_owned()),
            },
        )
        .unwrap();
        assert_eq!(same_id.full_name, "Ana Pérez");
        assert_eq!(same_id.company, Some("Nuevo".to_owned()));

        let _other = create_client_inner(
            &mut connection,
            now,
            ContactInput {
                first_name: "Luis".to_owned(),
                last_name: "García".to_owned(),
                national_id: "12345678917".to_owned(),
                phone: "+5355550012".to_owned(),
                company: None,
                position: None,
                email: None,
                address: None,
                invoice_info: None,
            },
        )
        .unwrap();

        let duplicate = update_client_inner(
            &mut connection,
            now,
            created.id,
            ContactInput {
                first_name: "Ana".to_owned(),
                last_name: "Pérez".to_owned(),
                national_id: "12345678917".to_owned(),
                phone: "+5355550013".to_owned(),
                company: None,
                position: None,
                email: None,
                address: None,
                invoice_info: None,
            },
        )
        .unwrap_err();
        assert_eq!(duplicate.code, "duplicateField");
        assert_eq!(
            duplicate.message,
            "Ya existe un cliente con ese carnet de identidad"
        );

        let missing = update_client_inner(
            &mut connection,
            now,
            9999,
            ContactInput {
                first_name: "No".to_owned(),
                last_name: "Existe".to_owned(),
                national_id: "12345678918".to_owned(),
                phone: "+5355550014".to_owned(),
                company: None,
                position: None,
                email: None,
                address: None,
                invoice_info: None,
            },
        );
        assert_eq!(missing.unwrap_err().code, "notFound");
    }

    #[test]
    fn delete_client_inner_removes_row_and_nulls_invoice_reference() {
        let now = Utc::now();
        let mut connection = setup_test_db();
        let user_id = create_test_user(
            &connection,
            "clientdeleter",
            "contador",
            "+5350000006",
            "12345678920",
            "ValidPass1!",
        );
        create_test_session(&connection, user_id, false, now, now);

        let client_id = create_client_inner(
            &mut connection,
            now,
            ContactInput {
                first_name: "Ana".to_owned(),
                last_name: "Pérez".to_owned(),
                national_id: "12345678921".to_owned(),
                phone: "+5355550016".to_owned(),
                company: Some("Café Norte".to_owned()),
                position: Some("Gerente".to_owned()),
                email: Some("ana@cafenorte.com".to_owned()),
                address: Some("Calle 5".to_owned()),
                invoice_info: Some("Factura de prueba".to_owned()),
            },
        )
        .unwrap()
        .id;

        connection
            .execute(
                "INSERT INTO invoices (type, number, invoiceDate, status, clientId, contactFullName, contactFullNameNormalized, contactNationalId, contactPhone, createdByName, createdAt, updatedAt)
                 VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8, ?9, ?10, ?11, ?12)",
                (
                    "V",
                    "V-00001",
                    "2026-01-15",
                    "pending",
                    client_id,
                    "Ana Pérez",
                    "ana perez",
                    "12345678921",
                    "+5355550016",
                    "Admin",
                    "2026-10-03T00:00:00Z",
                    "2026-10-03T00:00:00Z",
                ),
            )
            .unwrap();

        delete_client_inner(&mut connection, now, client_id).unwrap();

        let remaining: Option<i64> = connection
            .query_row("SELECT id FROM clients WHERE id = ?1", [client_id], |row| {
                row.get(0)
            })
            .optional()
            .unwrap();
        assert!(remaining.is_none());

        let invoice_client_id: Option<Option<i64>> = connection
            .query_row(
                "SELECT clientId FROM invoices WHERE number = ?1",
                ["V-00001"],
                |row| row.get::<_, Option<i64>>(0),
            )
            .optional()
            .unwrap();
        assert_eq!(invoice_client_id, Some(None));

        let missing = delete_client_inner(&mut connection, now, 9999).unwrap_err();
        assert_eq!(missing.code, "notFound");
        assert_eq!(missing.message, "El cliente ya no existe");
    }

    #[test]
    fn get_client_detail_inner_returns_profile_with_zero_indicators_and_empty_history() {
        let now = Utc::now();
        let mut connection = setup_test_db();
        let user_id = create_test_user(
            &connection,
            "clientdetailuser",
            "contador",
            "+5350000011",
            "12345678931",
            "ValidPass1!",
        );
        create_test_session(&connection, user_id, false, now, now);

        let created = create_client_inner(
            &mut connection,
            now,
            ContactInput {
                first_name: "Ana".to_owned(),
                last_name: "Pérez".to_owned(),
                national_id: "12345678932".to_owned(),
                phone: "+5355550111".to_owned(),
                company: Some("Café Norte".to_owned()),
                position: Some("Gerente".to_owned()),
                email: Some("ana@cafenorte.com".to_owned()),
                address: Some("Calle 5".to_owned()),
                invoice_info: Some("Factura de prueba".to_owned()),
            },
        )
        .unwrap();

        let detail = get_client_detail_inner(&mut connection, now, created.id).unwrap();
        assert_eq!(detail.id, created.id);
        assert_eq!(detail.full_name, "Ana Pérez");
        assert_eq!(detail.national_id, "12345678932");
        assert_eq!(detail.company, Some("Café Norte".to_owned()));
        assert_eq!(detail.indicators.total_invoiced_cents, 0);
        assert_eq!(detail.indicators.total_purchased_cents, 0);
        assert_eq!(detail.indicators.pending_balance_cents, 0);
        assert_eq!(detail.indicators.invoice_count, 0);
        assert_eq!(detail.indicators.last_purchase_date, None);
        assert!(detail.invoice_history.is_empty());

        let missing = get_client_detail_inner(&mut connection, now, 9999).unwrap_err();
        assert_eq!(missing.code, "notFound");
        assert_eq!(missing.message, "El cliente ya no existe");
    }

    #[test]
    fn get_client_detail_reports_indicators_and_history() {
        let now = Utc.with_ymd_and_hms(2026, 10, 4, 12, 0, 0).unwrap();
        let mut connection = setup_test_db();
        let user_id = create_test_user(
            &connection,
            "clientindicators",
            "contador",
            "+5350000040",
            "12345678940",
            "ValidPass1!",
        );
        create_test_session(&connection, user_id, false, now, now);
        let created = create_client_inner(
            &mut connection,
            now,
            ContactInput {
                first_name: "Luis".to_owned(),
                last_name: "García".to_owned(),
                national_id: "12345678941".to_owned(),
                phone: "+5355550112".to_owned(),
                company: None,
                position: None,
                email: None,
                address: None,
                invoice_info: None,
            },
        )
        .unwrap();
        // Ventas y servicio no cancelados cuentan; la garantía va al
        // historial pero no a los indicadores; la cancelada, a ninguno.
        for (invoice_type, number, date, status, subtotal) in [
            ("V", "V-00001", "2026-10-01", Some("pending"), 200),
            ("S", "S-00001", "2026-10-02", Some("pending"), 50),
            ("V", "V-00002", "2026-10-03", Some("completed"), 300),
            ("GG", "GG-00001", "2026-10-03", Some("pending"), 70),
            ("V", "V-00003", "2026-09-28", Some("cancelled"), 999),
        ] {
            let associated = matches!(invoice_type, "GG" | "GI");
            connection
                .execute(
                    "INSERT INTO invoices (type, number, invoiceDate, status, clientId, contactFullName,
                        contactFullNameNormalized, contactNationalId, contactPhone, associatedNumber,
                        associatedType, createdByUserId, createdByName, createdAt, updatedAt)
                     VALUES (?1, ?2, ?3, ?4, ?5, 'Luis García', 'luis garcia', '12345678941', '+5355550112',
                        CASE WHEN ?6 THEN 'V-00001' END, CASE WHEN ?6 THEN 'V' END, ?7, 'admin', ?8, ?8)",
                    rusqlite::params![
                        invoice_type,
                        number,
                        date,
                        status,
                        created.id,
                        associated,
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

        let detail = get_client_detail_inner(&mut connection, now, created.id).unwrap();
        assert_eq!(detail.indicators.total_invoiced_cents, 550);
        assert_eq!(detail.indicators.total_purchased_cents, 0);
        assert_eq!(detail.indicators.pending_balance_cents, 250);
        assert_eq!(detail.indicators.invoice_count, 3);
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
            vec!["GG-00001", "V-00002", "S-00001", "V-00001", "V-00003"]
        );
        let guarantee = detail
            .invoice_history
            .iter()
            .find(|item| item.number == "GG-00001")
            .unwrap();
        assert_eq!(guarantee.total_cents, 70);
        assert_eq!(
            guarantee.associated_invoice.as_ref().unwrap().number,
            "V-00001"
        );
    }

    #[test]
    fn get_client_detail_purges_before_reading() {
        let now = Utc.with_ymd_and_hms(2026, 10, 4, 12, 0, 0).unwrap();
        let mut connection = setup_test_db();
        let user_id = create_test_user(
            &connection,
            "clientpurgeuser",
            "contador",
            "+5350000042",
            "12345678942",
            "ValidPass1!",
        );
        create_test_session(&connection, user_id, false, now, now);
        let created = create_client_inner(
            &mut connection,
            now,
            ContactInput {
                first_name: "Ana".to_owned(),
                last_name: "Pérez".to_owned(),
                national_id: "12345678943".to_owned(),
                phone: "+5355550113".to_owned(),
                company: None,
                position: None,
                email: None,
                address: None,
                invoice_info: None,
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

        get_client_detail_inner(&mut connection, now, created.id).unwrap();
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
    fn get_client_detail_ignores_deleted_contact_invoices() {
        let now = Utc.with_ymd_and_hms(2026, 10, 4, 12, 0, 0).unwrap();
        let mut connection = setup_test_db();
        let user_id = create_test_user(
            &connection,
            "clientdeleteduser",
            "contador",
            "+5350000044",
            "12345678944",
            "ValidPass1!",
        );
        create_test_session(&connection, user_id, false, now, now);
        let doomed = create_client_inner(
            &mut connection,
            now,
            ContactInput {
                first_name: "Pedro".to_owned(),
                last_name: "Ruiz".to_owned(),
                national_id: "12345678945".to_owned(),
                phone: "+5355550114".to_owned(),
                company: None,
                position: None,
                email: None,
                address: None,
                invoice_info: None,
            },
        )
        .unwrap();
        let survivor = create_client_inner(
            &mut connection,
            now,
            ContactInput {
                first_name: "Marta".to_owned(),
                last_name: "León".to_owned(),
                national_id: "12345678946".to_owned(),
                phone: "+5355550115".to_owned(),
                company: None,
                position: None,
                email: None,
                address: None,
                invoice_info: None,
            },
        )
        .unwrap();
        connection
            .execute(
                "INSERT INTO invoices (type, number, invoiceDate, status, clientId, contactFullName,
                    contactFullNameNormalized, contactNationalId, contactPhone, createdByUserId,
                    createdByName, createdAt, updatedAt)
                 VALUES ('V', 'V-00999', '2026-10-01', 'pending', ?1, 'Pedro Ruiz', 'pedro ruiz',
                    '12345678945', '+5355550114', ?2, 'admin', ?3, ?3)",
                rusqlite::params![
                    doomed.id,
                    user_id,
                    crate::timestamps::format_utc_timestamp(now)
                ],
            )
            .unwrap();
        delete_client_inner(&mut connection, now, doomed.id).unwrap();

        // La factura del eliminado (clientId NULL) no rompe el otro cálculo.
        let detail = get_client_detail_inner(&mut connection, now, survivor.id).unwrap();
        assert_eq!(detail.indicators.total_invoiced_cents, 0);
        assert_eq!(detail.indicators.invoice_count, 0);
        assert!(detail.invoice_history.is_empty());
        let missing = get_client_detail_inner(&mut connection, now, doomed.id).unwrap_err();
        assert_eq!(missing.code, "notFound");
    }

    #[test]
    fn list_and_search_clients_require_session_and_allow_both_roles() {
        assert_requires_session(list_clients_inner);
        assert_allows_both_roles(list_clients_inner);
        assert_requires_session(|connection, now| search_clients_inner(connection, now, "ana"));
        assert_allows_both_roles(|connection, now| search_clients_inner(connection, now, "ana"));
    }

    #[test]
    fn create_client_requires_session_and_allows_both_roles() {
        let sample = ContactInput {
            first_name: "Ana".to_owned(),
            last_name: "Pérez".to_owned(),
            national_id: "12345678915".to_owned(),
            phone: "+5355550009".to_owned(),
            company: None,
            position: None,
            email: None,
            address: None,
            invoice_info: None,
        };
        assert_requires_session(|connection, now| {
            create_client_inner(connection, now, sample.clone())
        });
        assert_allows_both_roles(|connection, now| {
            create_client_inner(connection, now, sample.clone())
        });
    }

    #[test]
    fn update_client_requires_session_and_allows_both_roles() {
        let sample = ContactInput {
            first_name: "Ana".to_owned(),
            last_name: "Pérez".to_owned(),
            national_id: "12345678919".to_owned(),
            phone: "+5355550015".to_owned(),
            company: None,
            position: None,
            email: None,
            address: None,
            invoice_info: None,
        };
        assert_requires_session(|connection, now| {
            update_client_inner(connection, now, 1, sample.clone())
        });
        assert_allows_both_roles(|connection, now| {
            update_client_inner(connection, now, 1, sample.clone())
        });
    }

    #[test]
    fn delete_client_requires_session_and_allows_both_roles() {
        assert_requires_session(|connection, now| delete_client_inner(connection, now, 1));
        assert_allows_both_roles(|connection, now| delete_client_inner(connection, now, 1));
    }
}
