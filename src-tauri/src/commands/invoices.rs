use chrono::{DateTime, NaiveDate, Utc};
use rusqlite::{params, Connection, OptionalExtension};
use std::fs;
use std::path::Path;

use crate::attachments::{attachment_path, delete_attachment, store_attachment};
use crate::commands::auth::{require_session, Actor, Author};
use crate::commands::products::purge_best_effort;
use crate::db::connection::current_app_data_dir;
use crate::errors::{append_error_log, log_error, map_db_error, AppError};
use crate::inventory::{
    consume_fifo, line_total_cost_cents, line_unit_cost_cents, next_arrival_order,
    recalculate_product_cost, recalculate_product_quantity, restore_consumption,
    reverse_lots_of_invoice, validate_stock_limit,
};
use crate::invoice_counter::next_invoice_number;
use crate::models::invoice::{
    AssociatedInvoice, CreateEntryInvoiceInput, CreateGuaranteeInvoiceInput,
    CreateInstallationInvoiceInput, CreateLossInvoiceInput, CreateSaleInvoiceInput,
    CreateServiceInvoiceInput, InvoiceActionResult, InvoiceDetail, InvoiceFilters, InvoiceListItem,
    UpdateEntryInvoiceInput, UpdateGuaranteeInvoiceInput, UpdateInstallationInvoiceInput,
    UpdateOutgoingInvoiceLineInput, UpdateSaleInvoiceInput, UpdateServiceInvoiceInput,
};
use crate::models::invoice_line::InvoiceLineDetail;
use crate::models::product::format_quantity_milli;
use crate::pricing::selling_price_cents;
use crate::state::{
    validate_edit_allowed, validate_status_payload, validate_transition, InvoiceStatus, InvoiceType,
};
use crate::text::{clean_name, normalize_text};
use crate::timestamps::{format_utc_timestamp, parse_invoice_date, today_local};
use crate::validators::{
    parse_cost_cents, parse_percent_hundredths, parse_quantity_milli,
    validate_attachment_original_name, validate_expiry_date, validate_invoice_notes,
    validate_name_with_messages, validate_optional_description,
};

fn validation_failed(message: &str, cause: &str) -> AppError {
    AppError::new("validationFailed", message, cause)
}

/// RS-01: "Reservar hasta" es opcional; si llega, la factura debe estar Por
/// Cobrar y la fecha ser posterior a la emisión y a 365 días como máximo.
fn validate_reserved_until(
    raw: Option<&str>,
    invoice_date: NaiveDate,
    status: &str,
) -> Result<Option<String>, AppError> {
    let Some(raw) = raw.map(str::trim).filter(|value| !value.is_empty()) else {
        return Ok(None);
    };
    if status != "pending" {
        return Err(validation_failed(
            "Solo se puede reservar una factura Por Cobrar",
            "reservation on a non-pending invoice",
        ));
    }
    let date = NaiveDate::parse_from_str(raw, "%Y-%m-%d")
        .ok()
        .filter(|date| date.format("%Y-%m-%d").to_string() == raw)
        .ok_or_else(|| {
            validation_failed(
                "La fecha de reserva no es válida",
                "invalid reservation date",
            )
        })?;
    if date <= invoice_date {
        return Err(validation_failed(
            "La fecha de reserva debe ser posterior a la fecha de emisión",
            "reservation date is not after the issue date",
        ));
    }
    if date > invoice_date + chrono::Duration::days(365) {
        return Err(validation_failed(
            "La reserva no puede superar los 365 días desde la emisión",
            "reservation date is more than 365 days after the issue date",
        ));
    }
    Ok(Some(raw.to_owned()))
}

#[derive(Debug, Clone)]
struct EntryInvoiceLineRow {
    id: i64,
    position: i64,
    product_id: Option<i64>,
    product_id_original: i64,
    product_name: String,
    quantity_milli: i64,
    unit_price_cents: i64,
    lot_id: Option<i64>,
    lot_initial_milli: Option<i64>,
    lot_remaining_milli: Option<i64>,
    lot_unit_cost_cents: Option<i64>,
}

struct EntryUpdateMovement<'a> {
    invoice_id: i64,
    invoice_number: &'a str,
    product_id: i64,
    product_id_original: i64,
    product_name: &'a str,
    movement_type: &'a str,
    quantity_milli: i64,
    unit_price_cents: i64,
}

type EntryUpdateProductRow = (String, Option<i64>, Option<String>, Option<String>, String);
// El estado es NULL en las pérdidas (no tienen estados).
type InvoiceStatusRow = (
    String,
    Option<String>,
    Option<String>,
    Option<String>,
    String,
);
// Fila de producto para las facturas de venta y servicio: identidad, estado,
// costo y ganancia vigentes, categoría y unidad (copias para la línea).
type OutgoingProductRow = (
    String,
    String,
    i64,
    i64,
    Option<i64>,
    Option<String>,
    Option<i64>,
    Option<String>,
    Option<String>,
);

struct ReversalMovementInput<'a> {
    invoice_number: &'a str,
    invoice_type: &'a str,
    product_id: Option<i64>,
    product_id_original: i64,
    product_name: &'a str,
    quantity_milli: i64,
    unit_price_cents: i64,
}

fn entry_invoice_invalid_state(message: &str, cause: &str) -> AppError {
    AppError::new("invalidState", message, cause)
}

fn parse_entry_update_quantity(raw: &str) -> Result<i64, AppError> {
    let value = raw.trim();
    if matches!(
        value,
        "0" | "0,0" | "0,00" | "0,000" | "0.0" | "0.00" | "0.000"
    ) {
        Ok(0)
    } else {
        parse_quantity_milli(value)
    }
}

fn entry_line_consumed_milli(row: &EntryInvoiceLineRow) -> i64 {
    row.lot_initial_milli
        .zip(row.lot_remaining_milli)
        .map(|(initial, remaining)| initial - remaining)
        .unwrap_or(0)
}

fn consuming_invoice_number(
    connection: &Connection,
    lot_id: i64,
) -> Result<Option<String>, AppError> {
    connection
        .query_row(
            "SELECT i.number FROM consumptions c
             JOIN invoiceLines il ON il.id = c.invoiceLineId
             JOIN invoices i ON i.id = il.invoiceId
             WHERE c.lotId = ?1 AND (i.status IS NULL OR i.status <> 'cancelled')
             ORDER BY c.id LIMIT 1",
            [lot_id],
            |row| row.get(0),
        )
        .optional()
        .map_err(|error| map_db_error(error, "updateEntryInvoice", &[]))
}

fn insert_entry_update_movement(
    connection: &Connection,
    actor: &Actor,
    movement: EntryUpdateMovement<'_>,
    now: DateTime<Utc>,
) -> Result<(), AppError> {
    let stock_after: i64 = connection
        .query_row(
            "SELECT quantityMilli FROM products WHERE id = ?1",
            [movement.product_id],
            |row| row.get(0),
        )
        .map_err(|error| map_db_error(error, "updateEntryInvoice", &[]))?;
    let total_price_cents = crate::inventory::round_half_up(
        i128::from(movement.quantity_milli.abs()) * i128::from(movement.unit_price_cents),
        1_000,
    ) as i64;
    connection
        .execute(
            "INSERT INTO productMovements (createdAt, productId, productIdOriginal, productName, type,
                quantityMilli, stockAfterMilli, unitPriceCents, totalPriceCents, invoiceId, invoiceNumber,
                invoiceType, userId, userName)
             VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8, ?9, ?10, ?11, 'E', ?12, ?13)",
            params![
                format_utc_timestamp(now),
                movement.product_id,
                movement.product_id_original,
                movement.product_name,
                movement.movement_type,
                movement.quantity_milli,
                stock_after,
                movement.unit_price_cents,
                total_price_cents,
                movement.invoice_id,
                movement.invoice_number,
                actor.id(),
                actor.username(),
            ],
        )
        .map_err(|error| map_db_error(error, "updateEntryInvoice", &[]))?;
    Ok(())
}

fn remove_entry_invoice_line(
    connection: &Connection,
    actor: &Actor,
    invoice_id: i64,
    invoice_number: &str,
    row: &EntryInvoiceLineRow,
    now: DateTime<Utc>,
) -> Result<(), AppError> {
    let Some(product_id) = row.product_id else {
        connection
            .execute("DELETE FROM invoiceLines WHERE id = ?1", [row.id])
            .map_err(|error| map_db_error(error, "updateEntryInvoice", &[]))?;
        return Ok(());
    };

    let remaining_milli = row.lot_remaining_milli.unwrap_or(0);
    let unit_cost_cents = row.lot_unit_cost_cents.unwrap_or(row.unit_price_cents);
    connection
        .execute("DELETE FROM invoiceLines WHERE id = ?1", [row.id])
        .map_err(|error| map_db_error(error, "updateEntryInvoice", &[]))?;
    recalculate_product_quantity(connection, product_id)?;
    recalculate_product_cost(connection, product_id)?;
    insert_entry_update_movement(
        connection,
        actor,
        EntryUpdateMovement {
            invoice_id,
            invoice_number,
            product_id,
            product_id_original: row.product_id_original,
            product_name: &row.product_name,
            movement_type: "reversal",
            quantity_milli: -remaining_milli,
            unit_price_cents: unit_cost_cents,
        },
        now,
    )
}

fn normalize_optional_filter(value: Option<&str>) -> Option<String> {
    value
        .map(str::trim)
        .filter(|value| !value.is_empty())
        .map(|value| {
            if matches!(value.to_ascii_lowercase().as_str(), "all" | "todos") {
                String::new()
            } else {
                value.to_string()
            }
        })
        .filter(|value| !value.is_empty())
}

pub fn list_invoices_inner(
    connection: &mut Connection,
    now: DateTime<Utc>,
    filters: InvoiceFilters,
) -> Result<Vec<InvoiceListItem>, AppError> {
    require_session(connection, now)?;

    if let Err(error) = purge_best_effort(connection, now) {
        if let Some(app_data_dir) = current_app_data_dir() {
            log_error(
                connection,
                "invoices",
                "listInvoices",
                &error,
                now,
                app_data_dir,
            );
        }
    }

    let search_query = filters
        .search
        .as_deref()
        .map(str::trim)
        .filter(|value| !value.is_empty())
        .map(|value| normalize_text(&clean_name(value)))
        .unwrap_or_default();

    let invoice_type = normalize_optional_filter(filters.invoice_type.as_deref())
        .map(|value| value.to_ascii_uppercase());
    let status_filter = normalize_optional_filter(filters.status.as_deref())
        .map(|value| value.to_ascii_lowercase())
        .map(|value| match value.as_str() {
            "pendientes" => "pending".to_owned(),
            "completadas" => "completed".to_owned(),
            "canceladas" => "cancelled".to_owned(),
            "reservas" => "reserved".to_owned(),
            _ => value,
        });
    let phone_query = filters
        .search
        .as_deref()
        .unwrap_or_default()
        .chars()
        .filter(|character| !character.is_whitespace() && *character != '+')
        .collect::<String>();
    let phone_query = normalize_text(&phone_query);

    let from_date = filters
        .from_date
        .as_deref()
        .filter(|value| !value.trim().is_empty())
        .map(parse_invoice_date)
        .transpose()?;
    let to_date = filters
        .to_date
        .as_deref()
        .filter(|value| !value.trim().is_empty())
        .map(parse_invoice_date)
        .transpose()?;

    if let (Some(from_date), Some(to_date)) = (from_date, to_date) {
        if from_date > to_date {
            return Err(validation_failed(
                "La fecha inicial no puede ser posterior a la final",
                "invoice date range is inverted",
            ));
        }
    }

    let mut statement = connection
        .prepare(
                "SELECT i.id, i.number, i.type, i.contactFullName, i.contactCompany, i.contactNationalId,
                    i.contactPhone, i.invoiceDate, i.status, i.associatedNumber, i.associatedType,
                    l.productName, l.quantityMilli, i.reservedUntil
             FROM invoices i
             -- La pérdida tiene una sola línea: de ella salen producto y cantidad.
             LEFT JOIN invoiceLines l ON i.type = 'P' AND l.invoiceId = i.id AND l.position = 1
             ORDER BY i.invoiceDate DESC, i.id DESC",
        )
        .map_err(|error| map_db_error(error, "listInvoices", &[]))?;

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
                row.get::<_, Option<String>>(11)?,
                row.get::<_, Option<i64>>(12)?,
                row.get::<_, Option<String>>(13)?,
            ))
        })
        .map_err(|error| map_db_error(error, "listInvoices", &[]))?;

    let mut items = Vec::new();
    for row in rows {
        let (
            id,
            number,
            invoice_type_value,
            contact_name,
            contact_company,
            contact_national_id,
            contact_phone,
            invoice_date,
            status_value,
            associated_number,
            associated_type,
            product_name,
            quantity_milli,
            reserved_until,
        ) = row.map_err(|error| map_db_error(error, "listInvoices", &[]))?;

        let invoice_day = match NaiveDate::parse_from_str(&invoice_date, "%Y-%m-%d") {
            Ok(date) => date,
            Err(_) => continue,
        };

        if let Some(from_date) = from_date {
            if invoice_day < from_date {
                continue;
            }
        }
        if let Some(to_date) = to_date {
            if invoice_day > to_date {
                continue;
            }
        }

        if let Some(ref expected_type) = invoice_type {
            if invoice_type_value != *expected_type {
                continue;
            }
        }

        if let Some(ref expected_status) = status_filter {
            if invoice_type_value == "P" {
                continue;
            }
            // RS-03: "Reservas activas" = Por Cobrar con fecha límite.
            let matches = if expected_status == "reserved" {
                status_value.as_deref() == Some("pending") && reserved_until.is_some()
            } else {
                status_value.as_deref() == Some(expected_status.as_str())
            };
            if !matches {
                continue;
            }
        }

        if !search_query.is_empty() {
            let fields = [
                normalize_text(&number),
                contact_name
                    .as_deref()
                    .map(normalize_text)
                    .unwrap_or_default(),
                contact_company
                    .as_deref()
                    .map(normalize_text)
                    .unwrap_or_default(),
                contact_national_id
                    .as_deref()
                    .map(normalize_text)
                    .unwrap_or_default(),
                product_name
                    .as_deref()
                    .map(normalize_text)
                    .unwrap_or_default(),
            ];
            let normalized_phone = contact_phone
                .as_deref()
                .unwrap_or_default()
                .chars()
                .filter(|character| !character.is_whitespace() && *character != '+')
                .collect::<String>();
            let phone_matches =
                !phone_query.is_empty() && normalize_text(&normalized_phone).contains(&phone_query);
            if !fields.iter().any(|field| field.contains(&search_query)) && !phone_matches {
                continue;
            }
        }

        let (total_cents, product_count, created_by_name) =
            crate::search::invoice_list_stats(connection, id)
                .map_err(|error| map_db_error(error, "listInvoices", &[]))?;

        items.push(InvoiceListItem {
            id,
            number,
            invoice_type: invoice_type_value,
            contact_name,
            date: invoice_date,
            total_cents,
            status: status_value,
            associated_invoice: associated_number.zip(associated_type).map(
                |(number, invoice_type)| AssociatedInvoice {
                    number,
                    invoice_type,
                },
            ),
            product_name,
            quantity: quantity_milli.map(format_quantity_milli),
            product_count,
            created_by_name,
            reserved_until,
        });
    }

    items.sort_by(|left, right| right.date.cmp(&left.date).then(right.id.cmp(&left.id)));

    Ok(items)
}

pub fn get_invoice_inner(
    connection: &mut Connection,
    now: DateTime<Utc>,
    invoice_id: i64,
) -> Result<InvoiceDetail, AppError> {
    require_session(connection, now)?;

    if let Err(error) = purge_best_effort(connection, now) {
        if let Some(app_data_dir) = current_app_data_dir() {
            log_error(
                connection,
                "invoices",
                "getInvoice",
                &error,
                now,
                app_data_dir,
            );
        }
    }

    type InvoiceDetailRow = (
        String,
        String,
        String,
        Option<String>,
        Option<String>,
        Option<String>,
        Option<String>,
        Option<String>,
        Option<String>,
        Option<String>,
        Option<String>,
        Option<String>,
        Option<String>,
        Option<String>,
        Option<String>,
        Option<String>,
        Option<i64>,
        Option<String>,
        String,
        String,
        String,
    );
    let row: Option<InvoiceDetailRow> = connection
        .query_row(
            "SELECT type, number, invoiceDate, status, notes, paymentMethod, paidAt, contactFullName,
                    contactCompany, contactNationalId, contactPhone, contactEmail, associatedNumber,
                    associatedType, attachmentFileName, attachmentOriginalName, attachmentSizeBytes,
                    attachmentUploadedAt, createdByName, createdAt, updatedAt
             FROM invoices WHERE id = ?1",
            [invoice_id],
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
                    row.get(10)?,
                    row.get(11)?,
                    row.get(12)?,
                    row.get(13)?,
                    row.get(14)?,
                    row.get(15)?,
                    row.get(16)?,
                    row.get(17)?,
                    row.get(18)?,
                    row.get(19)?,
                    row.get(20)?,
                ))
            },
        )
        .optional()
        .map_err(|error| map_db_error(error, "getInvoice", &[]))?;

    let Some((
        invoice_type,
        number,
        date,
        status,
        notes,
        payment_method,
        paid_at,
        contact_full_name,
        contact_company,
        contact_national_id,
        contact_phone,
        contact_email,
        associated_number,
        associated_type,
        attachment_file_name,
        attachment_original_name,
        attachment_size_bytes,
        attachment_uploaded_at,
        created_by_name,
        created_at,
        updated_at,
    )) = row
    else {
        return Err(AppError::new(
            "notFound",
            "La factura ya no existe",
            "invoice missing during detail lookup",
        ));
    };

    let reserved_until: Option<String> = connection
        .query_row(
            "SELECT reservedUntil FROM invoices WHERE id = ?1",
            [invoice_id],
            |row| row.get(0),
        )
        .map_err(|error| map_db_error(error, "getInvoice", &[]))?;
    let total_cents: i64 = connection
        .query_row(
            "SELECT COALESCE(SUM(subtotalCents), 0) FROM invoiceLines WHERE invoiceId = ?1",
            [invoice_id],
            |row| row.get(0),
        )
        .map_err(|error| map_db_error(error, "getInvoice", &[]))?;

    // Ids vigentes para editar la factura: `None` si el contacto o la factura
    // asociada se eliminaron (la copia de texto se conserva igual).
    let (contact_id, associated_invoice_id): (Option<i64>, Option<i64>) = connection
        .query_row(
            "SELECT COALESCE(clientId, supplierId), associatedInvoiceId FROM invoices WHERE id = ?1",
            [invoice_id],
            |row| Ok((row.get(0)?, row.get(1)?)),
        )
        .map_err(|error| map_db_error(error, "getInvoice", &[]))?;

    let lines = {
        let mut statement = connection
            .prepare(
                "SELECT il.position, il.productIdOriginal, il.productName, il.unitName, il.unitAbbreviation,
                        il.quantityMilli, il.unitPriceCents, il.subtotalCents, il.categoryName,
                        il.costAtSaleCents, p.status, l.remainingMilli,
                        CASE WHEN l.id IS NULL THEN NULL ELSE (
                            SELECT COALESCE(SUM(c.quantityMilli), 0)
                            FROM consumptions c
                            JOIN invoiceLines cl ON cl.id = c.invoiceLineId
                            JOIN invoices ci ON ci.id = cl.invoiceId
                            WHERE c.lotId = l.id AND (ci.status IS NULL OR ci.status <> 'cancelled')
                        ) END
                 FROM invoiceLines il
                 LEFT JOIN products p ON p.id = il.productId
                 LEFT JOIN lots l ON l.invoiceLineId = il.id
                 WHERE il.invoiceId = ?1 ORDER BY il.position",
            )
            .map_err(|error| map_db_error(error, "getInvoice", &[]))?;
        let rows = statement
            .query_map([invoice_id], |row| {
                Ok(InvoiceLineDetail {
                    position: row.get(0)?,
                    product_id_original: row.get(1)?,
                    product_name: row.get(2)?,
                    unit_name: row.get(3)?,
                    unit_abbreviation: row.get(4)?,
                    quantity: format_quantity_milli(row.get(5)?),
                    unit_price_cents: row.get(6)?,
                    subtotal_cents: row.get(7)?,
                    category_name: row.get(8)?,
                    cost_at_sale_cents: row.get(9)?,
                    product_status: row.get(10)?,
                    remaining_quantity: row.get::<_, Option<i64>>(11)?.map(format_quantity_milli),
                    consumed_quantity: row.get::<_, Option<i64>>(12)?.map(format_quantity_milli),
                })
            })
            .map_err(|error| map_db_error(error, "getInvoice", &[]))?;
        rows.collect::<Result<Vec<_>, _>>()
            .map_err(|error| map_db_error(error, "getInvoice", &[]))?
    };

    Ok(InvoiceDetail {
        id: invoice_id,
        number,
        invoice_type,
        contact_name: contact_full_name.clone(),
        date,
        total_cents,
        associated_invoice: associated_number
            .zip(associated_type)
            .map(|(number, invoice_type)| AssociatedInvoice {
                number,
                invoice_type,
            }),
        status,
        notes,
        payment_method,
        paid_at,
        contact_id,
        associated_invoice_id,
        contact_full_name,
        contact_company,
        contact_national_id,
        contact_phone,
        contact_email,
        attachment_file_name,
        attachment_original_name,
        attachment_size_bytes,
        attachment_uploaded_at,
        created_by_name,
        created_at,
        updated_at,
        reserved_until,
        lines,
    })
}

#[tauri::command]
pub fn get_invoice(
    state: tauri::State<'_, std::sync::Mutex<Connection>>,
    invoice_id: i64,
) -> Result<InvoiceDetail, AppError> {
    let mut connection = crate::db::connection::lock_conn(&state);
    let now = Utc::now();
    let result = get_invoice_inner(&mut connection, now, invoice_id);
    if let Err(error) = &result {
        if error.should_log() {
            if let Some(app_data_dir) = current_app_data_dir() {
                log_error(
                    &connection,
                    "invoices",
                    "getInvoice",
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
pub fn list_invoices(
    state: tauri::State<'_, std::sync::Mutex<Connection>>,
    filters: InvoiceFilters,
) -> Result<Vec<InvoiceListItem>, AppError> {
    let mut connection = crate::db::connection::lock_conn(&state);
    let now = Utc::now();
    let result = list_invoices_inner(&mut connection, now, filters);
    if let Err(error) = &result {
        if error.should_log() {
            if let Some(app_data_dir) = current_app_data_dir() {
                log_error(
                    &connection,
                    "invoices",
                    "listInvoices",
                    error,
                    now,
                    app_data_dir,
                );
            }
        }
    }
    result
}

fn insert_reversal_movement(
    transaction: &Connection,
    author: &Author<'_>,
    movement: ReversalMovementInput<'_>,
    now: DateTime<Utc>,
) -> Result<(), AppError> {
    let Some(product_id) = movement.product_id else {
        return Ok(());
    };
    let stock_after: i64 = transaction
        .query_row(
            "SELECT quantityMilli FROM products WHERE id = ?1",
            [product_id],
            |row| row.get(0),
        )
        .map_err(|error| map_db_error(error, "changeInvoiceStatus", &[]))?;
    let total_price = (i128::from(movement.quantity_milli) * i128::from(movement.unit_price_cents)
        / i128::from(1_000)) as i64;
    transaction
        .execute(
            "INSERT INTO productMovements (createdAt, productId, productIdOriginal, productName, type,
                quantityMilli, stockAfterMilli, unitPriceCents, totalPriceCents, invoiceId,
                invoiceNumber, invoiceType, userId, userName)
             VALUES (?1, ?2, ?3, ?4, 'reversal', ?5, ?6, ?7, ?8, ?9, ?10, ?11, ?12, ?13)",
            params![
                &format_utc_timestamp(now),
                product_id,
                movement.product_id_original,
                movement.product_name,
                movement.quantity_milli,
                stock_after,
                movement.unit_price_cents,
                total_price,
                None::<i64>,
                movement.invoice_number,
                movement.invoice_type,
                author.id(),
                author.name(),
            ],
        )
        .map_err(|error| map_db_error(error, "changeInvoiceStatus", &[]))?;
    Ok(())
}

pub(crate) fn reverse_outgoing_invoice_consumptions(
    transaction: &Connection,
    author: &Author<'_>,
    invoice_id: i64,
    invoice_number: &str,
    invoice_type: &str,
    now: DateTime<Utc>,
) -> Result<(), AppError> {
    let mut statement = transaction
        .prepare(
            "SELECT id, productId, productIdOriginal, productName FROM invoiceLines WHERE invoiceId = ?1 ORDER BY position",
        )
        .map_err(|error| map_db_error(error, "changeInvoiceStatus", &[]))?;
    let rows = statement
        .query_map([invoice_id], |row| {
            Ok((
                row.get::<_, i64>(0)?,
                row.get::<_, Option<i64>>(1)?,
                row.get::<_, i64>(2)?,
                row.get::<_, String>(3)?,
            ))
        })
        .map_err(|error| map_db_error(error, "changeInvoiceStatus", &[]))?
        .collect::<rusqlite::Result<Vec<_>>>()
        .map_err(|error| map_db_error(error, "changeInvoiceStatus", &[]))?;
    drop(statement);

    for (line_id, _product_id, product_id_original, product_name) in rows {
        let mut consumption_statement = transaction
            .prepare(
                "SELECT id, quantityMilli, unitCostCents FROM consumptions WHERE invoiceLineId = ?1 ORDER BY id",
            )
            .map_err(|error| map_db_error(error, "changeInvoiceStatus", &[]))?;
        let consumptions = consumption_statement
            .query_map([line_id], |row| {
                Ok((
                    row.get::<_, i64>(0)?,
                    row.get::<_, i64>(1)?,
                    row.get::<_, i64>(2)?,
                ))
            })
            .map_err(|error| map_db_error(error, "changeInvoiceStatus", &[]))?
            .collect::<rusqlite::Result<Vec<_>>>()
            .map_err(|error| map_db_error(error, "changeInvoiceStatus", &[]))?;
        drop(consumption_statement);

        for (consumption_id, quantity_milli, unit_cost_cents) in consumptions {
            let restoration = restore_consumption(transaction, consumption_id, now)?;
            if let Some(product_id) = restoration.product_id {
                insert_reversal_movement(
                    transaction,
                    author,
                    ReversalMovementInput {
                        invoice_number,
                        invoice_type,
                        product_id: Some(product_id),
                        product_id_original,
                        product_name: &product_name,
                        quantity_milli,
                        unit_price_cents: unit_cost_cents,
                    },
                    now,
                )?;
            }
        }
    }

    Ok(())
}

pub fn create_entry_invoice_inner(
    connection: &mut Connection,
    now: DateTime<Utc>,
    input: CreateEntryInvoiceInput,
) -> Result<InvoiceActionResult, AppError> {
    let _actor = require_session(connection, now)?;
    let transaction = connection
        .transaction()
        .map_err(|error| map_db_error(error, "createEntryInvoice", &[]))?;
    // Purga best-effort de P-08 (spec-03 §7.6); su aviso viaja en el resultado.
    let expired_cleanup_warning = purge_best_effort(&transaction, now)?;
    let connection = &transaction;

    if input.lines.is_empty() || input.lines.len() > 100 {
        return Err(validation_failed(
            "La factura debe tener entre 1 y 100 líneas",
            "entry invoice line count out of range",
        ));
    }

    let invoice_date = parse_invoice_date(&input.invoice_date)?;
    let today = today_local(now);
    if invoice_date > today {
        return Err(validation_failed(
            "La fecha no puede ser futura",
            "entry invoice date is in the future",
        ));
    }

    let supplier_exists: bool = connection
        .query_row(
            "SELECT EXISTS(SELECT 1 FROM suppliers WHERE id = ?1)",
            [input.supplier_id],
            |row| row.get(0),
        )
        .map_err(|error| map_db_error(error, "createEntryInvoice", &[]))?;
    if !supplier_exists {
        return Err(AppError::new(
            "notFound",
            "El proveedor ya no existe",
            "supplier missing during entry invoice creation",
        ));
    }

    let normalized_status = input
        .status
        .as_deref()
        .map(str::trim)
        .filter(|value| !value.is_empty())
        .unwrap_or("pending");
    if !matches!(normalized_status, "pending" | "completed") {
        return Err(validation_failed(
            "El estado de la factura no es válido",
            "entry invoice status rejected",
        ));
    }

    let attachment_file_name = input
        .attachment_file_name
        .as_deref()
        .map(str::trim)
        .filter(|value| !value.is_empty());
    let attachment_original_name = input
        .attachment_original_name
        .as_deref()
        .map(str::trim)
        .filter(|value| !value.is_empty())
        .map(validate_attachment_original_name)
        .transpose()?;
    let attachment_size_bytes = input.attachment_size_bytes;
    let attachment_uploaded_at = input
        .attachment_uploaded_at
        .as_deref()
        .map(str::trim)
        .filter(|value| !value.is_empty());
    let has_attachment = attachment_file_name.is_some()
        || attachment_original_name.is_some()
        || attachment_size_bytes.is_some()
        || attachment_uploaded_at.is_some();
    if has_attachment {
        if attachment_file_name.is_none()
            || attachment_original_name.is_none()
            || attachment_size_bytes.is_none()
            || attachment_uploaded_at.is_none()
        {
            return Err(validation_failed(
                "Los datos del adjunto no son válidos",
                "entry invoice attachment incomplete",
            ));
        }
        if !matches!(attachment_size_bytes, Some(1..=5_242_880))
            || attachment_uploaded_at
                .and_then(crate::timestamps::parse_utc_timestamp)
                .is_none()
        {
            return Err(AppError::new(
                "attachmentInvalid",
                "Los datos del adjunto no son válidos",
                "entry invoice attachment metadata rejected",
            ));
        }
    }
    let notes = input
        .notes
        .as_deref()
        .map(validate_invoice_notes)
        .transpose()?
        .flatten();

    let mut seen_products = std::collections::HashSet::new();
    let mut seen_product_names = std::collections::HashSet::new();
    let mut line_rows = Vec::new();
    let mut total_cents = 0_i64;

    for (position, line) in input.lines.iter().enumerate() {
        if line.product_id.is_some() == line.new_product.is_some() {
            return Err(validation_failed(
                "Cada línea debe indicar un producto existente o los datos de uno nuevo",
                "entry invoice line product selection is invalid",
            ));
        }

        let quantity_milli = parse_quantity_milli(&line.quantity).map_err(|error| {
            AppError::new(
                "validationFailed",
                "La cantidad de la línea no es válida",
                &format!("invalid quantity for entry line: {}", error.message),
            )
        })?;
        let unit_price_cents = parse_cost_cents(&line.unit_price).map_err(|error| {
            AppError::new(
                "validationFailed",
                "El precio de la línea no es válido",
                &format!("invalid unit price for entry line: {}", error.message),
            )
        })?;
        let subtotal_cents = crate::inventory::round_half_up(
            i128::from(quantity_milli) * i128::from(unit_price_cents),
            1_000,
        ) as i64;
        total_cents = total_cents.checked_add(subtotal_cents).ok_or_else(|| {
            AppError::new(
                "validationFailed",
                "El total de la factura supera el rango permitido",
                "entry invoice total exceeded",
            )
        })?;
        if total_cents > 99_999_999_999 {
            return Err(validation_failed(
                "El total de la factura no puede superar 999.999.999,99",
                "entry invoice total exceeds legal bound",
            ));
        }

        let (product_id, product_name, product_unit_name, product_unit_abbreviation) = if let Some(
            product_id,
        ) =
            line.product_id
        {
            if !seen_products.insert(product_id) {
                return Err(validation_failed(
                    "El producto ya está en la factura",
                    "duplicate product in entry invoice",
                ));
            }
            let product = connection
                    .query_row(
                        "SELECT p.name, u.name, u.abbreviation, p.status FROM products p LEFT JOIN units u ON u.id = p.unitId WHERE p.id = ?1",
                        [product_id],
                        |row| Ok((row.get::<_, String>(0)?, row.get::<_, Option<String>>(1)?, row.get::<_, Option<String>>(2)?, row.get::<_, String>(3)?)),
                    )
                    .optional()
                    .map_err(|error| map_db_error(error, "createEntryInvoice", &[]))?
                    .ok_or_else(|| {
                        AppError::new(
                            "notFound",
                            "El producto ya no existe",
                            "product missing during entry invoice creation",
                        )
                    })?;
            if product.3 == "discontinued" {
                return Err(validation_failed(
                    &format!(
                        "El producto «{}» está descontinuado. Actívalo para usarlo en facturas",
                        product.0
                    ),
                    "discontinued product used in entry invoice",
                ));
            }
            if !seen_product_names.insert(normalize_text(&product.0)) {
                return Err(validation_failed(
                    "El producto ya está en la factura",
                    "duplicate product in entry invoice",
                ));
            }
            validate_stock_limit(connection, product_id, quantity_milli)?;
            (product_id, product.0, product.1, product.2)
        } else {
            let new_product = line.new_product.as_ref().ok_or_else(|| {
                validation_failed(
                    "Cada línea debe indicar un producto existente o los datos de uno nuevo",
                    "entry invoice line product selection is invalid",
                )
            })?;
            let (name, normalized_name) = validate_name_with_messages(
                &new_product.name,
                2,
                100,
                "El nombre del producto debe tener al menos 2 caracteres",
                "El nombre del producto no puede tener más de 100 caracteres",
                "El nombre del producto contiene caracteres no permitidos",
            )?;
            if !seen_product_names.insert(normalized_name.clone()) {
                return Err(validation_failed(
                    "El producto ya está en la factura",
                    "duplicate product in entry invoice",
                ));
            }
            let description = validate_optional_description(
                new_product.description.as_deref(),
                500,
                "La descripción no puede tener más de 500 caracteres",
                "La descripción contiene caracteres no permitidos",
            )?;
            let product_cost_cents = parse_cost_cents(&new_product.cost).map_err(|error| {
                AppError::new(
                    "validationFailed",
                    "El costo inicial del producto no es válido",
                    &format!("invalid new product cost: {}", error.message),
                )
            })?;
            let profit_percent_hundredths = parse_percent_hundredths(&new_product.profit_percent)
                .map_err(|error| {
                AppError::new(
                    "validationFailed",
                    "El porcentaje de ganancia no es válido",
                    &format!("invalid new product profit: {}", error.message),
                )
            })?;
            let expires_on = new_product
                .expires_on
                .as_deref()
                .filter(|date| !date.trim().is_empty())
                .map(|date| validate_expiry_date(date, today, None))
                .transpose()?;
            let category_name = if let Some(category_id) = new_product.category_id {
                Some(
                    connection
                        .query_row(
                            "SELECT name FROM categories WHERE id = ?1",
                            [category_id],
                            |row| row.get::<_, String>(0),
                        )
                        .optional()
                        .map_err(|error| map_db_error(error, "createEntryInvoice", &[]))?
                        .ok_or_else(|| {
                            AppError::new(
                                "notFound",
                                "La categoría ya no existe",
                                "category missing during invoice product creation",
                            )
                        })?,
                )
            } else {
                None
            };
            let unit = if let Some(unit_id) = new_product.unit_id {
                Some(
                    connection
                        .query_row(
                            "SELECT name, abbreviation FROM units WHERE id = ?1",
                            [unit_id],
                            |row| Ok((row.get::<_, String>(0)?, row.get::<_, String>(1)?)),
                        )
                        .optional()
                        .map_err(|error| map_db_error(error, "createEntryInvoice", &[]))?
                        .ok_or_else(|| {
                            AppError::new(
                                "notFound",
                                "La unidad ya no existe",
                                "unit missing during invoice product creation",
                            )
                        })?,
                )
            } else {
                None
            };
            let duplicate: bool = connection
                    .query_row(
                        "SELECT EXISTS(SELECT 1 FROM products WHERE nameNormalized = ?1 AND status = 'active')",
                        [&normalized_name],
                        |row| row.get(0),
                    )
                    .map_err(|error| map_db_error(error, "createEntryInvoice", &[]))?;
            if duplicate {
                return Err(AppError::new(
                    "duplicateField",
                    "Ya existe un producto con ese nombre",
                    "active product name duplicated during entry invoice creation",
                ));
            }
            let timestamp = format_utc_timestamp(now);
            connection
                    .execute(
                        "INSERT INTO products (name, nameNormalized, description, categoryId, unitId, costCents,
                            profitPercentHundredths, expiresOn, quantityMilli, status, discontinuedAt, createdAt, updatedAt)
                         VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8, 0, 'active', NULL, ?9, ?9)",
                        params![name, normalized_name, description, new_product.category_id, new_product.unit_id,
                            product_cost_cents, profit_percent_hundredths, expires_on, timestamp],
                    )
                    .map_err(|error| map_db_error(error, "createEntryInvoice", &[("products.nameNormalized", "Ya existe un producto con ese nombre")]))?;
            let product_id = connection.last_insert_rowid();
            seen_products.insert(product_id);
            let _ = category_name;
            (
                product_id,
                name,
                unit.as_ref().map(|value| value.0.clone()),
                unit.as_ref().map(|value| value.1.clone()),
            )
        };

        line_rows.push((
            position as i64 + 1,
            product_id,
            product_id,
            product_name.clone(),
            product_unit_name,
            product_unit_abbreviation,
            quantity_milli,
            unit_price_cents,
            subtotal_cents,
        ));
    }

    let number = next_invoice_number(connection, "E")?;
    let timestamp = format_utc_timestamp(now);
    let payment_method_input = input
        .payment_method
        .as_deref()
        .map(str::trim)
        .filter(|value| !value.is_empty());
    if payment_method_input
        .is_some_and(|method| !matches!(method, "cash" | "transfer" | "card" | "zelle"))
    {
        return Err(validation_failed(
            "El método de pago no es válido",
            "entry invoice payment method rejected",
        ));
    }
    if normalized_status == "pending" && payment_method_input.is_some() {
        return Err(validation_failed(
            "El método de pago solo se indica en una factura pagada",
            "payment method provided for pending entry invoice",
        ));
    }
    let payment_method = if normalized_status == "completed" {
        Some(payment_method_input.unwrap_or("cash"))
    } else {
        None
    };

    let paid_at = if normalized_status == "completed" {
        Some(timestamp.clone())
    } else {
        None
    };

    connection.execute(
        "INSERT INTO invoices (type, number, invoiceDate, status, supplierId, contactFullName, contactFullNameNormalized,
            contactNationalId, contactPhone, notes, paymentMethod, paidAt, attachmentFileName,
            attachmentOriginalName, attachmentSizeBytes, attachmentUploadedAt, createdByUserId, createdByName, createdAt, updatedAt)
         VALUES (?1, ?2, ?3, ?4, ?5, (SELECT firstName || ' ' || lastName FROM suppliers WHERE id = ?5),
            (SELECT lower(firstName || ' ' || lastName) FROM suppliers WHERE id = ?5),
            (SELECT nationalId FROM suppliers WHERE id = ?5), (SELECT phone FROM suppliers WHERE id = ?5), ?6, ?7, ?8,
            ?9, ?10, ?11, ?12, ?13, ?14, ?15, ?16)",
        params![
            "E",
            &number,
            &input.invoice_date,
            normalized_status,
            input.supplier_id,
            notes.as_deref(),
            payment_method,
            paid_at.as_deref(),
            attachment_file_name,
            attachment_original_name,
            attachment_size_bytes,
            attachment_uploaded_at,
            _actor.id(),
            _actor.username(),
            &timestamp,
            &timestamp,
        ],
    ).map_err(|error| map_db_error(error, "createEntryInvoice", &[]))?;

    let invoice_id = connection.last_insert_rowid();
    for (
        position,
        product_id,
        product_id_original,
        product_name,
        unit_name,
        unit_abbreviation,
        quantity_milli,
        unit_price_cents,
        subtotal_cents,
    ) in line_rows
    {
        let arrival_order = next_arrival_order(connection, product_id)?;
        connection.execute(
            "INSERT INTO invoiceLines (invoiceId, position, productId, productIdOriginal, productName, productNameNormalized,
                unitId, unitName, unitAbbreviation, quantityMilli, unitPriceCents, subtotalCents, createdAt, updatedAt)
             VALUES (?1, ?2, ?3, ?4, ?5, ?6, (SELECT unitId FROM products WHERE id = ?3), ?7, ?8, ?9, ?10, ?11, ?12, ?12)",
            params![
                invoice_id,
                position,
                product_id,
                product_id_original,
                product_name,
                normalize_text(&product_name),
                unit_name,
                unit_abbreviation,
                quantity_milli,
                unit_price_cents,
                subtotal_cents,
                &timestamp,
            ],
        ).map_err(|error| map_db_error(error, "createEntryInvoice", &[]))?;

        let line_id = connection.last_insert_rowid();
        connection.execute(
            "INSERT INTO lots (productId, origin, invoiceLineId, arrivalOrder, initialMilli, remainingMilli, unitCostCents, createdAt, updatedAt)
             VALUES (?1, 'entry', ?2, ?3, ?4, ?4, ?5, ?6, ?6)",
            params![
                product_id,
                line_id,
                arrival_order,
                quantity_milli,
                unit_price_cents,
                &timestamp,
            ],
        ).map_err(|error| map_db_error(error, "createEntryInvoice", &[]))?;

        recalculate_product_quantity(connection, product_id)?;
        recalculate_product_cost(connection, product_id)?;

        let stock_after: i64 = connection
            .query_row(
                "SELECT quantityMilli FROM products WHERE id = ?1",
                [product_id],
                |row| row.get(0),
            )
            .map_err(|error| map_db_error(error, "createEntryInvoice", &[]))?;
        let total_price =
            (i128::from(quantity_milli) * i128::from(unit_price_cents) / 1_000) as i64;
        connection.execute(
            "INSERT INTO productMovements (createdAt, productId, productIdOriginal, productName, type, quantityMilli,
                stockAfterMilli, unitPriceCents, totalPriceCents, invoiceId, invoiceNumber, invoiceType, userId, userName)
             VALUES (?1, ?2, ?3, ?4, 'entry', ?5, ?6, ?7, ?8, ?9, ?10, ?11, ?12, ?13)",
            params![
                &timestamp,
                product_id,
                product_id,
                product_name,
                quantity_milli,
                stock_after,
                unit_price_cents,
                total_price,
                invoice_id,
                &number,
                "E",
                _actor.id(),
                _actor.username(),
            ],
        ).map_err(|error| map_db_error(error, "createEntryInvoice", &[]))?;
    }

    // NT-02/NT-03: avisos de situación con el estado final (best-effort).
    crate::automation::evaluate_watches_best_effort(&transaction, now);
    transaction
        .commit()
        .map_err(|error| map_db_error(error, "createEntryInvoice", &[]))?;

    Ok(InvoiceActionResult {
        invoice_id,
        number,
        expired_cleanup_warning,
    })
}

/// Núcleo compartido de las facturas de venta y de servicio (FS-01): ambas
/// descuentan por FIFO al precio de venta vigente y guardan la misma copia.
/// Solo cambian el tipo de factura, el tipo de movimiento y las etiquetas.
#[allow(clippy::too_many_arguments)]
fn create_client_outgoing_invoice_inner(
    connection: &mut Connection,
    now: DateTime<Utc>,
    op: &str,
    kind: &str,
    invoice_type: &str,
    movement_type: &str,
    client_id: i64,
    invoice_date: &str,
    status: Option<&str>,
    notes: Option<&str>,
    payment_method: Option<&str>,
    lines: &[(i64, String)],
    reserved_until: Option<&str>,
) -> Result<InvoiceActionResult, AppError> {
    let _actor = require_session(connection, now)?;
    let transaction = connection
        .transaction()
        .map_err(|error| map_db_error(error, op, &[]))?;
    // Purga best-effort de P-08 (spec-03 §7.6); su aviso viaja en el resultado.
    let expired_cleanup_warning = purge_best_effort(&transaction, now)?;
    let connection = &transaction;

    if lines.is_empty() || lines.len() > 100 {
        return Err(validation_failed(
            "Una factura admite entre 1 y 100 productos",
            &format!("{kind} line count out of range"),
        ));
    }

    let invoice_date = parse_invoice_date(invoice_date)?;
    let today = today_local(now);
    if invoice_date > today {
        return Err(validation_failed(
            "La fecha no puede ser futura",
            &format!("{kind} date is in the future"),
        ));
    }

    let client_exists: bool = connection
        .query_row(
            "SELECT EXISTS(SELECT 1 FROM clients WHERE id = ?1)",
            [client_id],
            |row| row.get(0),
        )
        .map_err(|error| map_db_error(error, op, &[]))?;
    if !client_exists {
        return Err(AppError::new(
            "notFound",
            "El cliente ya no existe",
            &format!("client missing during {kind} creation"),
        ));
    }

    let normalized_status = status
        .map(str::trim)
        .filter(|value| !value.is_empty())
        .unwrap_or("pending");
    if !matches!(normalized_status, "pending" | "completed") {
        return Err(validation_failed(
            "El estado de la factura no es válido",
            &format!("{kind} status rejected"),
        ));
    }
    let reserved_until = validate_reserved_until(reserved_until, invoice_date, normalized_status)?;

    let notes = notes.map(validate_invoice_notes).transpose()?.flatten();

    let mut seen_products = std::collections::HashSet::new();
    let mut line_rows = Vec::new();
    let mut total_cents = 0_i64;

    for (position, (product_id, quantity)) in lines.iter().enumerate() {
        if !seen_products.insert(*product_id) {
            return Err(validation_failed(
                "El producto ya está en la factura",
                &format!("duplicate product in {kind}"),
            ));
        }

        let quantity_milli = parse_quantity_milli(quantity).map_err(|error| {
            AppError::new(
                "validationFailed",
                "La cantidad de la línea no es válida",
                &format!("invalid quantity for {kind} line: {}", error.message),
            )
        })?;

        let product: OutgoingProductRow = connection
            .query_row(
                "SELECT p.name, p.status, p.costCents, p.profitPercentHundredths,
                    p.categoryId, c.name, p.unitId, u.name, u.abbreviation
                 FROM products p
                 LEFT JOIN categories c ON c.id = p.categoryId
                 LEFT JOIN units u ON u.id = p.unitId
                 WHERE p.id = ?1",
                [*product_id],
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
            .map_err(|error| map_db_error(error, op, &[]))?
            .ok_or_else(|| {
                AppError::new(
                    "notFound",
                    "El producto ya no existe",
                    &format!("product missing during {kind} creation"),
                )
            })?;
        let (
            product_name,
            product_status,
            cost_cents,
            profit_percent_hundredths,
            category_id,
            category_name,
            unit_id,
            unit_name,
            unit_abbreviation,
        ) = product;
        if product_status == "discontinued" {
            return Err(validation_failed(
                &format!(
                    "El producto «{product_name}» está descontinuado. Actívalo para usarlo en facturas"
                ),
                &format!("discontinued product used in {kind}"),
            ));
        }
        // Sin existencias no se puede agregar (FS-01); la insuficiencia
        // parcial la informa `consume_fifo` con el disponible.
        let available_milli: i64 = connection
            .query_row(
                "SELECT COALESCE(SUM(remainingMilli), 0) FROM lots WHERE productId = ?1",
                [*product_id],
                |row| row.get(0),
            )
            .map_err(|error| map_db_error(error, op, &[]))?;
        if available_milli == 0 {
            return Err(validation_failed(
                &format!("El producto «{product_name}» está agotado"),
                &format!("out of stock product used in {kind}"),
            ));
        }

        // Precio de venta vigente al crear (spec-02 P-11).
        let unit_price_cents = selling_price_cents(cost_cents, profit_percent_hundredths);
        let subtotal_cents = crate::inventory::round_half_up(
            i128::from(quantity_milli) * i128::from(unit_price_cents),
            1_000,
        ) as i64;
        total_cents = total_cents.checked_add(subtotal_cents).ok_or_else(|| {
            AppError::new(
                "validationFailed",
                "El total de la factura supera el rango permitido",
                &format!("{kind} total exceeded"),
            )
        })?;
        if total_cents > 99_999_999_999 {
            return Err(validation_failed(
                "El total de la factura no puede superar $999.999.999,99",
                &format!("{kind} total exceeds legal bound"),
            ));
        }

        let product_name_normalized = normalize_text(&product_name);
        line_rows.push((
            position as i64 + 1,
            *product_id,
            product_name,
            product_name_normalized,
            unit_id,
            unit_name,
            unit_abbreviation,
            quantity_milli,
            unit_price_cents,
            subtotal_cents,
            category_id,
            category_name,
            cost_cents,
        ));
    }

    let number = next_invoice_number(connection, invoice_type)?;
    let timestamp = format_utc_timestamp(now);
    let payment_method_input = payment_method
        .map(str::trim)
        .filter(|value| !value.is_empty());
    if payment_method_input
        .is_some_and(|method| !matches!(method, "cash" | "transfer" | "card" | "zelle"))
    {
        return Err(validation_failed(
            "El método de pago no es válido",
            &format!("{kind} payment method rejected"),
        ));
    }
    if normalized_status == "pending" && payment_method_input.is_some() {
        return Err(validation_failed(
            "El método de pago solo se indica en una factura cobrada",
            "payment method provided for pending sale invoice",
        ));
    }
    let payment_method = if normalized_status == "completed" {
        Some(payment_method_input.unwrap_or("cash"))
    } else {
        None
    };

    let paid_at = if normalized_status == "completed" {
        Some(timestamp.clone())
    } else {
        None
    };

    connection.execute(
        "INSERT INTO invoices (type, number, invoiceDate, status, clientId, contactFullName, contactFullNameNormalized,
            contactNationalId, contactPhone, notes, paymentMethod, paidAt, createdByUserId, createdByName, createdAt, updatedAt,
            reservedUntil)
         VALUES (?1, ?2, ?3, ?4, ?5, (SELECT firstName || ' ' || lastName FROM clients WHERE id = ?5),
            (SELECT lower(firstName || ' ' || lastName) FROM clients WHERE id = ?5),
            (SELECT nationalId FROM clients WHERE id = ?5), (SELECT phone FROM clients WHERE id = ?5), ?6, ?7, ?8,
            ?9, ?10, ?11, ?11, ?12)",
        params![
            invoice_type,
            &number,
            invoice_date,
            normalized_status,
            client_id,
            notes.as_deref(),
            payment_method,
            paid_at.as_deref(),
            _actor.id(),
            _actor.username(),
            &timestamp,
            reserved_until.as_deref(),
        ],
    ).map_err(|error| map_db_error(error, op, &[]))?;

    let invoice_id = connection.last_insert_rowid();
    for (
        position,
        product_id,
        product_name,
        product_name_normalized,
        unit_id,
        unit_name,
        unit_abbreviation,
        quantity_milli,
        unit_price_cents,
        subtotal_cents,
        category_id,
        category_name,
        cost_at_sale_cents,
    ) in line_rows
    {
        connection.execute(
            "INSERT INTO invoiceLines (invoiceId, position, productId, productIdOriginal, productName, productNameNormalized,
                unitId, unitName, unitAbbreviation, quantityMilli, unitPriceCents, subtotalCents,
                categoryId, categoryName, costAtSaleCents, createdAt, updatedAt)
             VALUES (?1, ?2, ?3, ?3, ?4, ?5, ?6, ?7, ?8, ?9, ?10, ?11, ?12, ?13, ?14, ?15, ?15)",
            params![
                invoice_id,
                position,
                product_id,
                product_name,
                product_name_normalized,
                unit_id,
                unit_name,
                unit_abbreviation,
                quantity_milli,
                unit_price_cents,
                subtotal_cents,
                category_id,
                category_name,
                cost_at_sale_cents,
                &timestamp,
            ],
        ).map_err(|error| map_db_error(error, op, &[]))?;

        let line_id = connection.last_insert_rowid();
        // Descuenta por FIFO (LT-02); sin stock suficiente revierte todo.
        consume_fifo(connection, product_id, quantity_milli, line_id, now)?;

        recalculate_product_quantity(connection, product_id)?;
        recalculate_product_cost(connection, product_id)?;

        let stock_after: i64 = connection
            .query_row(
                "SELECT quantityMilli FROM products WHERE id = ?1",
                [product_id],
                |row| row.get(0),
            )
            .map_err(|error| map_db_error(error, op, &[]))?;
        connection.execute(
            "INSERT INTO productMovements (createdAt, productId, productIdOriginal, productName, type, quantityMilli,
                stockAfterMilli, unitPriceCents, totalPriceCents, invoiceId, invoiceNumber, invoiceType, userId, userName)
             VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8, ?9, ?10, ?11, ?12, ?13, ?14)",
            params![
                &timestamp,
                product_id,
                product_id,
                product_name,
                movement_type,
                -quantity_milli,
                stock_after,
                unit_price_cents,
                subtotal_cents,
                invoice_id,
                &number,
                invoice_type,
                _actor.id(),
                _actor.username(),
            ],
        ).map_err(|error| map_db_error(error, op, &[]))?;
    }

    // NT-02/NT-03: avisos de situación con el estado final (best-effort).
    crate::automation::evaluate_watches_best_effort(&transaction, now);
    transaction
        .commit()
        .map_err(|error| map_db_error(error, op, &[]))?;

    Ok(InvoiceActionResult {
        invoice_id,
        number,
        expired_cleanup_warning,
    })
}

pub fn create_sale_invoice_inner(
    connection: &mut Connection,
    now: DateTime<Utc>,
    input: CreateSaleInvoiceInput,
) -> Result<InvoiceActionResult, AppError> {
    let lines: Vec<(i64, String)> = input
        .lines
        .iter()
        .map(|line| (line.product_id, line.quantity.clone()))
        .collect();
    let result = create_client_outgoing_invoice_inner(
        connection,
        now,
        "createSaleInvoice",
        "sale invoice",
        "V",
        "saleOut",
        input.client_id,
        &input.invoice_date,
        input.status.as_deref(),
        input.notes.as_deref(),
        input.payment_method.as_deref(),
        &lines,
        input.reserved_until.as_deref(),
    );
    crate::automation::after_rejected_outgoing(connection, now, result)
}

pub fn create_service_invoice_inner(
    connection: &mut Connection,
    now: DateTime<Utc>,
    input: CreateServiceInvoiceInput,
) -> Result<InvoiceActionResult, AppError> {
    let lines: Vec<(i64, String)> = input
        .lines
        .iter()
        .map(|line| (line.product_id, line.quantity.clone()))
        .collect();
    let result = create_client_outgoing_invoice_inner(
        connection,
        now,
        "createServiceInvoice",
        "service invoice",
        "S",
        "serviceOut",
        input.client_id,
        &input.invoice_date,
        input.status.as_deref(),
        input.notes.as_deref(),
        input.payment_method.as_deref(),
        &lines,
        input.reserved_until.as_deref(),
    );
    crate::automation::after_rejected_outgoing(connection, now, result)
}

/// Núcleo compartido de las facturas de gasto de garantía y de insumos de
/// instalación (FS-04, FS-05): factura asociada obligatoria, líneas a costo
/// real FIFO y sin pago. Solo cambian el tipo, el movimiento, los tipos
/// asociados admitidos y el mensaje de asociada inválida.
#[allow(clippy::too_many_arguments)]
fn create_associated_outgoing_invoice_inner(
    connection: &mut Connection,
    now: DateTime<Utc>,
    op: &str,
    invoice_type: &str,
    movement_type: &str,
    allowed_associated: &[&str],
    associated_error: &str,
    client_id: i64,
    associated_invoice_id: i64,
    invoice_date: &str,
    status: Option<&str>,
    notes: Option<&str>,
    lines: &[(i64, String)],
) -> Result<InvoiceActionResult, AppError> {
    let _actor = require_session(connection, now)?;
    let transaction = connection
        .transaction()
        .map_err(|error| map_db_error(error, op, &[]))?;
    // Purga best-effort de P-08 (spec-03 §7.6); su aviso viaja en el resultado.
    let expired_cleanup_warning = purge_best_effort(&transaction, now)?;
    let connection = &transaction;

    if lines.is_empty() || lines.len() > 100 {
        return Err(validation_failed(
            "Una factura admite entre 1 y 100 productos",
            &format!("{op} line count out of range"),
        ));
    }

    let invoice_date = parse_invoice_date(invoice_date)?;
    let today = today_local(now);
    if invoice_date > today {
        return Err(validation_failed(
            "La fecha no puede ser futura",
            &format!("{op} date is in the future"),
        ));
    }

    let client_exists: bool = connection
        .query_row(
            "SELECT EXISTS(SELECT 1 FROM clients WHERE id = ?1)",
            [client_id],
            |row| row.get(0),
        )
        .map_err(|error| map_db_error(error, op, &[]))?;
    if !client_exists {
        return Err(AppError::new(
            "notFound",
            "El cliente ya no existe",
            &format!("client missing during {op} creation"),
        ));
    }

    // La factura original: del tipo admitido, del mismo cliente, no cancelada.
    let associated: Option<(String, String, Option<i64>, Option<String>)> = connection
        .query_row(
            "SELECT type, number, clientId, status FROM invoices WHERE id = ?1",
            [associated_invoice_id],
            |row| Ok((row.get(0)?, row.get(1)?, row.get(2)?, row.get(3)?)),
        )
        .optional()
        .map_err(|error| map_db_error(error, op, &[]))?;
    let Some((associated_type, associated_number, associated_client_id, associated_status)) =
        associated
    else {
        return Err(AppError::new(
            "notFound",
            "La factura ya no existe",
            &format!("associated invoice missing during {op} creation"),
        ));
    };
    if !allowed_associated.contains(&associated_type.as_str())
        || associated_client_id != Some(client_id)
        || associated_status.as_deref() == Some("cancelled")
    {
        return Err(validation_failed(
            associated_error,
            &format!("{op} associated invoice rejected"),
        ));
    }

    let normalized_status = status
        .map(str::trim)
        .filter(|value| !value.is_empty())
        .unwrap_or("pending");
    if !matches!(normalized_status, "pending" | "completed") {
        return Err(validation_failed(
            "El estado de la factura no es válido",
            &format!("{op} status rejected"),
        ));
    }

    let notes = notes.map(validate_invoice_notes).transpose()?.flatten();

    let mut seen_products = std::collections::HashSet::new();
    let mut line_rows = Vec::new();
    let mut total_cents = 0_i64;

    for (position, (product_id, quantity)) in lines.iter().enumerate() {
        if !seen_products.insert(*product_id) {
            return Err(validation_failed(
                "El producto ya está en la factura",
                &format!("duplicate product in {op}"),
            ));
        }

        let quantity_milli = parse_quantity_milli(quantity).map_err(|error| {
            AppError::new(
                "validationFailed",
                "La cantidad de la línea no es válida",
                &format!("invalid quantity for {op} line: {}", error.message),
            )
        })?;

        let product: (String, String, Option<i64>, Option<String>, Option<String>) = connection
            .query_row(
                "SELECT p.name, p.status, p.unitId, u.name, u.abbreviation
                 FROM products p
                 LEFT JOIN units u ON u.id = p.unitId
                 WHERE p.id = ?1",
                [*product_id],
                |row| {
                    Ok((
                        row.get(0)?,
                        row.get(1)?,
                        row.get(2)?,
                        row.get(3)?,
                        row.get(4)?,
                    ))
                },
            )
            .optional()
            .map_err(|error| map_db_error(error, op, &[]))?
            .ok_or_else(|| {
                AppError::new(
                    "notFound",
                    "El producto ya no existe",
                    &format!("product missing during {op} creation"),
                )
            })?;
        let (product_name, product_status, unit_id, unit_name, unit_abbreviation) = product;
        if product_status == "discontinued" {
            return Err(validation_failed(
                &format!(
                    "El producto «{product_name}» está descontinuado. Actívalo para usarlo en facturas"
                ),
                &format!("discontinued product used in {op}"),
            ));
        }
        let available_milli: i64 = connection
            .query_row(
                "SELECT COALESCE(SUM(remainingMilli), 0) FROM lots WHERE productId = ?1",
                [*product_id],
                |row| row.get(0),
            )
            .map_err(|error| map_db_error(error, op, &[]))?;
        if available_milli == 0 {
            return Err(validation_failed(
                &format!("El producto «{product_name}» está agotado"),
                &format!("out of stock product used in {op}"),
            ));
        }

        let product_name_normalized = normalize_text(&product_name);
        line_rows.push((
            position as i64 + 1,
            *product_id,
            product_name,
            product_name_normalized,
            unit_id,
            unit_name,
            unit_abbreviation,
            quantity_milli,
        ));
    }

    let number = next_invoice_number(connection, invoice_type)?;
    let timestamp = format_utc_timestamp(now);

    connection.execute(
        "INSERT INTO invoices (type, number, invoiceDate, status, clientId, contactFullName, contactFullNameNormalized,
            contactNationalId, contactPhone, notes, associatedInvoiceId, associatedNumber, associatedType,
            createdByUserId, createdByName, createdAt, updatedAt)
         VALUES (?1, ?2, ?3, ?4, ?5, (SELECT firstName || ' ' || lastName FROM clients WHERE id = ?5),
            (SELECT lower(firstName || ' ' || lastName) FROM clients WHERE id = ?5),
            (SELECT nationalId FROM clients WHERE id = ?5), (SELECT phone FROM clients WHERE id = ?5), ?6,
            ?7, ?8, ?9, ?10, ?11, ?12, ?12)",
        params![
            invoice_type,
            &number,
            invoice_date,
            normalized_status,
            client_id,
            notes.as_deref(),
            associated_invoice_id,
            associated_number,
            associated_type,
            _actor.id(),
            _actor.username(),
            &timestamp,
        ],
    ).map_err(|error| map_db_error(error, op, &[]))?;

    let invoice_id = connection.last_insert_rowid();
    for (
        position,
        product_id,
        product_name,
        product_name_normalized,
        unit_id,
        unit_name,
        unit_abbreviation,
        quantity_milli,
    ) in line_rows
    {
        connection.execute(
            "INSERT INTO invoiceLines (invoiceId, position, productId, productIdOriginal, productName, productNameNormalized,
                unitId, unitName, unitAbbreviation, quantityMilli, unitPriceCents, subtotalCents, createdAt, updatedAt)
             VALUES (?1, ?2, ?3, ?3, ?4, ?5, ?6, ?7, ?8, ?9, 0, 0, ?10, ?10)",
            params![
                invoice_id,
                position,
                product_id,
                product_name,
                product_name_normalized,
                unit_id,
                unit_name,
                unit_abbreviation,
                quantity_milli,
                &timestamp,
            ],
        ).map_err(|error| map_db_error(error, op, &[]))?;

        let line_id = connection.last_insert_rowid();
        // Costo real de los lotes consumidos (LT-02, FS-04/FS-05).
        let consumptions = consume_fifo(connection, product_id, quantity_milli, line_id, now)?;
        let line_total_cents = line_total_cost_cents(&consumptions);
        let line_unit_cost_cents = line_unit_cost_cents(line_total_cents, quantity_milli);
        total_cents = total_cents.checked_add(line_total_cents).ok_or_else(|| {
            AppError::new(
                "validationFailed",
                "El total de la factura supera el rango permitido",
                &format!("{op} total exceeded"),
            )
        })?;
        if total_cents > 99_999_999_999 {
            return Err(validation_failed(
                "El total de la factura no puede superar $999.999.999,99",
                &format!("{op} total exceeds legal bound"),
            ));
        }
        connection.execute(
            "UPDATE invoiceLines SET unitPriceCents = ?1, subtotalCents = ?2, updatedAt = ?3 WHERE id = ?4",
            params![line_unit_cost_cents, line_total_cents, &timestamp, line_id],
        ).map_err(|error| map_db_error(error, op, &[]))?;

        recalculate_product_quantity(connection, product_id)?;
        recalculate_product_cost(connection, product_id)?;

        let stock_after: i64 = connection
            .query_row(
                "SELECT quantityMilli FROM products WHERE id = ?1",
                [product_id],
                |row| row.get(0),
            )
            .map_err(|error| map_db_error(error, op, &[]))?;
        connection.execute(
            "INSERT INTO productMovements (createdAt, productId, productIdOriginal, productName, type, quantityMilli,
                stockAfterMilli, unitPriceCents, totalPriceCents, invoiceId, invoiceNumber, invoiceType, userId, userName)
             VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8, ?9, ?10, ?11, ?12, ?13, ?14)",
            params![
                &timestamp,
                product_id,
                product_id,
                product_name,
                movement_type,
                -quantity_milli,
                stock_after,
                line_unit_cost_cents,
                line_total_cents,
                invoice_id,
                &number,
                invoice_type,
                _actor.id(),
                _actor.username(),
            ],
        ).map_err(|error| map_db_error(error, op, &[]))?;
    }

    // NT-02/NT-03: avisos de situación con el estado final (best-effort).
    crate::automation::evaluate_watches_best_effort(&transaction, now);
    transaction
        .commit()
        .map_err(|error| map_db_error(error, op, &[]))?;

    Ok(InvoiceActionResult {
        invoice_id,
        number,
        expired_cleanup_warning,
    })
}

pub fn create_guarantee_invoice_inner(
    connection: &mut Connection,
    now: DateTime<Utc>,
    input: CreateGuaranteeInvoiceInput,
) -> Result<InvoiceActionResult, AppError> {
    let lines: Vec<(i64, String)> = input
        .lines
        .iter()
        .map(|line| (line.product_id, line.quantity.clone()))
        .collect();
    let result = create_associated_outgoing_invoice_inner(
        connection,
        now,
        "createGuaranteeInvoice",
        "GG",
        "warrantyOut",
        &["V", "S"],
        "La factura original debe ser una venta o servicio de este cliente que no esté cancelada",
        input.client_id,
        input.associated_invoice_id,
        &input.invoice_date,
        input.status.as_deref(),
        input.notes.as_deref(),
        &lines,
    );
    crate::automation::after_rejected_outgoing(connection, now, result)
}

pub fn create_installation_invoice_inner(
    connection: &mut Connection,
    now: DateTime<Utc>,
    input: CreateInstallationInvoiceInput,
) -> Result<InvoiceActionResult, AppError> {
    let lines: Vec<(i64, String)> = input
        .lines
        .iter()
        .map(|line| (line.product_id, line.quantity.clone()))
        .collect();
    let result = create_associated_outgoing_invoice_inner(
        connection,
        now,
        "createInstallationInvoice",
        "GI",
        "supplyOut",
        &["S"],
        "La factura de servicio debe ser de este cliente y no estar cancelada",
        input.client_id,
        input.associated_invoice_id,
        &input.invoice_date,
        input.status.as_deref(),
        input.notes.as_deref(),
        &lines,
    );
    crate::automation::after_rejected_outgoing(connection, now, result)
}

/// Línea de una factura de salida leída para editar.
#[derive(Debug, Clone)]
struct OutgoingInvoiceLineRow {
    id: i64,
    position: i64,
    product_id: Option<i64>,
    product_id_original: i64,
    product_name: String,
    quantity_milli: i64,
    unit_price_cents: i64,
}

struct OutgoingUpdateMovement<'a> {
    invoice_id: i64,
    invoice_number: &'a str,
    invoice_type: &'a str,
    product_id: i64,
    product_id_original: i64,
    product_name: &'a str,
    movement_type: &'a str,
    quantity_milli: i64,
    unit_price_cents: i64,
}

fn insert_outgoing_update_movement(
    connection: &Connection,
    actor: &Actor,
    movement: OutgoingUpdateMovement<'_>,
    now: DateTime<Utc>,
    op: &str,
) -> Result<(), AppError> {
    let stock_after: i64 = connection
        .query_row(
            "SELECT quantityMilli FROM products WHERE id = ?1",
            [movement.product_id],
            |row| row.get(0),
        )
        .map_err(|error| map_db_error(error, op, &[]))?;
    let total_price_cents = crate::inventory::round_half_up(
        i128::from(movement.quantity_milli.abs()) * i128::from(movement.unit_price_cents),
        1_000,
    ) as i64;
    connection
        .execute(
            "INSERT INTO productMovements (createdAt, productId, productIdOriginal, productName, type,
                quantityMilli, stockAfterMilli, unitPriceCents, totalPriceCents, invoiceId, invoiceNumber,
                invoiceType, userId, userName)
             VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8, ?9, ?10, ?11, ?12, ?13, ?14)",
            params![
                format_utc_timestamp(now),
                movement.product_id,
                movement.product_id_original,
                movement.product_name,
                movement.movement_type,
                movement.quantity_milli,
                stock_after,
                movement.unit_price_cents,
                total_price_cents,
                movement.invoice_id,
                movement.invoice_number,
                movement.invoice_type,
                actor.id(),
                actor.username(),
            ],
        )
        .map_err(|error| map_db_error(error, op, &[]))?;
    Ok(())
}

struct OutgoingLineRestoration<'a> {
    invoice_id: i64,
    invoice_number: &'a str,
    invoice_type: &'a str,
    line_id: i64,
    product_id_original: i64,
    product_name: &'a str,
    record_reversal: bool,
    op: &'a str,
}

/// Devuelve a sus lotes todos los consumos de una línea (LT-04). Con
/// `record_reversal` registra además una Reversión por consumo, como al
/// cancelar; sin él la devolución es silenciosa (edición con reajuste).
/// Devuelve el total devuelto en milésimas.
fn restore_outgoing_line_consumptions(
    connection: &Connection,
    actor: &Actor,
    restoration: OutgoingLineRestoration<'_>,
    now: DateTime<Utc>,
) -> Result<i64, AppError> {
    let op = restoration.op;
    let mut statement = connection
        .prepare(
            "SELECT id, quantityMilli, unitCostCents FROM consumptions WHERE invoiceLineId = ?1 ORDER BY id",
        )
        .map_err(|error| map_db_error(error, op, &[]))?;
    let consumptions = statement
        .query_map([restoration.line_id], |row| {
            Ok((
                row.get::<_, i64>(0)?,
                row.get::<_, i64>(1)?,
                row.get::<_, i64>(2)?,
            ))
        })
        .map_err(|error| map_db_error(error, op, &[]))?
        .collect::<rusqlite::Result<Vec<_>>>()
        .map_err(|error| map_db_error(error, op, &[]))?;
    drop(statement);

    let mut restored_milli = 0_i64;
    for (consumption_id, quantity_milli, unit_cost_cents) in consumptions {
        let restoration_result = restore_consumption(connection, consumption_id, now)?;
        let Some(product_id) = restoration_result.product_id else {
            continue;
        };
        // El consumo ya volvió a su lote: si se conservara, la siguiente
        // edición lo devolvería otra vez e inflaría el stock.
        connection
            .execute("DELETE FROM consumptions WHERE id = ?1", [consumption_id])
            .map_err(|error| map_db_error(error, op, &[]))?;
        restored_milli += quantity_milli;
        if restoration.record_reversal {
            let total_price_cents = crate::inventory::round_half_up(
                i128::from(quantity_milli) * i128::from(unit_cost_cents),
                1_000,
            ) as i64;
            connection
                .execute(
                    "INSERT INTO productMovements (createdAt, productId, productIdOriginal, productName, type,
                        quantityMilli, stockAfterMilli, unitPriceCents, totalPriceCents, invoiceId,
                        invoiceNumber, invoiceType, userId, userName)
                     VALUES (?1, ?2, ?3, ?4, 'reversal', ?5,
                        (SELECT quantityMilli FROM products WHERE id = ?2), ?6, ?7, ?8, ?9, ?10, ?11, ?12)",
                    params![
                        format_utc_timestamp(now),
                        product_id,
                        restoration.product_id_original,
                        restoration.product_name,
                        quantity_milli,
                        unit_cost_cents,
                        total_price_cents,
                        restoration.invoice_id,
                        restoration.invoice_number,
                        restoration.invoice_type,
                        actor.id(),
                        actor.username(),
                    ],
                )
                .map_err(|error| map_db_error(error, op, &[]))?;
        }
    }
    Ok(restored_milli)
}

/// Fila de producto para agregar líneas de garantía/insumos al editar:
/// nombre, unidad (id, nombre, abreviatura) y estado.
type CostedProductRow = (String, Option<i64>, Option<String>, Option<String>, String);

/// Fila de producto para agregar líneas de venta/servicio al editar.
type PricedProductRow = (
    String,
    i64,
    i64,
    Option<i64>,
    Option<String>,
    Option<i64>,
    Option<String>,
    Option<String>,
    String,
);

/// Núcleo de edición de ventas y servicios (FS-01): las líneas existentes
/// conservan el precio con que se emitieron; las nuevas toman el vigente.
/// Devuelve los consumos anteriores (LT-04), vuelve a descontar y registra
/// Salida por línea nueva, Reversión por línea quitada y Ajuste solo si la
/// cantidad cambió (H-A4).
#[allow(clippy::too_many_arguments)]
fn update_priced_outgoing_invoice_inner(
    connection: &mut Connection,
    now: DateTime<Utc>,
    op: &str,
    invoice_type: InvoiceType,
    sale_movement: &str,
    invoice_id: i64,
    client_id: i64,
    invoice_date: &str,
    notes: Option<&str>,
    lines: &[UpdateOutgoingInvoiceLineInput],
    reserved_until: Option<&str>,
) -> Result<InvoiceActionResult, AppError> {
    let type_code = invoice_type.as_str();
    let actor = require_session(connection, now)?;
    let transaction = connection
        .transaction()
        .map_err(|error| map_db_error(error, op, &[]))?;
    // Purga best-effort de P-08 (spec-03 §7.6); su aviso viaja en el resultado.
    let expired_cleanup_warning = purge_best_effort(&transaction, now)?;
    let connection = &transaction;

    let header: Option<(String, String)> = connection
        .query_row(
            "SELECT status, number FROM invoices WHERE id = ?1 AND type = ?2",
            params![invoice_id, type_code],
            |row| Ok((row.get(0)?, row.get(1)?)),
        )
        .optional()
        .map_err(|error| map_db_error(error, op, &[]))?;
    let Some((status, invoice_number)) = header else {
        return Err(AppError::new(
            "notFound",
            "La factura ya no existe",
            &format!("{op} invoice missing during update"),
        ));
    };
    validate_edit_allowed(invoice_type, InvoiceStatus::from_str(&status)?)?;

    if lines.is_empty() || lines.len() > 100 {
        return Err(validation_failed(
            "Una factura admite entre 1 y 100 productos",
            &format!("{op} line count out of range"),
        ));
    }
    let invoice_date = parse_invoice_date(invoice_date)?;
    if invoice_date > today_local(now) {
        return Err(validation_failed(
            "La fecha no puede ser futura",
            &format!("{op} date is in the future"),
        ));
    }
    let reserved_until = validate_reserved_until(reserved_until, invoice_date, &status)?;
    let client: Option<(String, String, String, String)> = connection
        .query_row(
            "SELECT firstName, lastName, nationalId, phone FROM clients WHERE id = ?1",
            [client_id],
            |row| Ok((row.get(0)?, row.get(1)?, row.get(2)?, row.get(3)?)),
        )
        .optional()
        .map_err(|error| map_db_error(error, op, &[]))?;
    let Some((client_first_name, client_last_name, client_national_id, client_phone)) = client
    else {
        return Err(AppError::new(
            "notFound",
            "El cliente ya no existe",
            &format!("client missing during {op} update"),
        ));
    };
    let notes = notes.map(validate_invoice_notes).transpose()?.flatten();

    let current_lines: Vec<OutgoingInvoiceLineRow> = {
        let mut statement = connection
            .prepare(
                "SELECT id, position, productId, productIdOriginal, productName,
                    quantityMilli, unitPriceCents
                 FROM invoiceLines WHERE invoiceId = ?1 ORDER BY position",
            )
            .map_err(|error| map_db_error(error, op, &[]))?;
        let lines = statement
            .query_map([invoice_id], |row| {
                Ok(OutgoingInvoiceLineRow {
                    id: row.get(0)?,
                    position: row.get(1)?,
                    product_id: row.get(2)?,
                    product_id_original: row.get(3)?,
                    product_name: row.get(4)?,
                    quantity_milli: row.get(5)?,
                    unit_price_cents: row.get(6)?,
                })
            })
            .map_err(|error| map_db_error(error, op, &[]))?
            .collect::<rusqlite::Result<Vec<_>>>()
            .map_err(|error| map_db_error(error, op, &[]))?;
        lines
    };
    let existing_by_original: std::collections::HashMap<i64, OutgoingInvoiceLineRow> =
        current_lines
            .iter()
            .cloned()
            .map(|row| (row.product_id_original, row))
            .collect();

    // Referencias y cantidades; los precios los decide el backend.
    let mut seen_original_ids = std::collections::HashSet::new();
    let mut seen_product_ids = std::collections::HashSet::new();
    let mut parsed_lines = Vec::with_capacity(lines.len());
    for line in lines {
        if let Some(product_id) = line.product_id {
            if !seen_product_ids.insert(product_id) {
                return Err(validation_failed(
                    "El producto ya está en la factura",
                    &format!("duplicate product in {op}"),
                ));
            }
        }
        if let Some(original_id) = line.product_id_original {
            if !seen_original_ids.insert(original_id)
                || !existing_by_original.contains_key(&original_id)
            {
                return Err(validation_failed(
                    "Las líneas de la factura no son válidas",
                    &format!("{op} line reference is invalid"),
                ));
            }
        } else if line
            .product_id
            .is_some_and(|product_id| existing_by_original.contains_key(&product_id))
        {
            return Err(validation_failed(
                "Las líneas existentes deben conservar su identificador original",
                &format!("existing {op} line omitted productIdOriginal"),
            ));
        }
        let quantity_milli = parse_entry_update_quantity(&line.quantity).map_err(|error| {
            AppError::new(
                "validationFailed",
                "La cantidad de la línea no es válida",
                &format!("invalid updated {op} quantity: {}", error.message),
            )
        })?;
        parsed_lines.push((line, quantity_milli));
    }

    let mut occupied_positions: std::collections::BTreeSet<i64> =
        current_lines.iter().map(|row| row.position).collect();
    // Quitar: lo ausente en la entrada, lo reducido a 0 y lo que cambia de
    // producto se devuelve (con Reversión) y se borra; se guarda la posición.
    let mut position_overrides: std::collections::HashMap<i64, i64> =
        std::collections::HashMap::new();
    for row in &current_lines {
        let referencing = parsed_lines
            .iter()
            .find(|(line, _)| line.product_id_original == Some(row.product_id_original));
        let remove = match referencing {
            None => true,
            Some((line, quantity_milli)) => {
                *quantity_milli == 0 || line.product_id.is_some_and(|id| Some(id) != row.product_id)
            }
        };
        if !remove {
            continue;
        }
        if row.product_id.is_some() {
            restore_outgoing_line_consumptions(
                connection,
                &actor,
                OutgoingLineRestoration {
                    invoice_id,
                    invoice_number: &invoice_number,
                    invoice_type: type_code,
                    line_id: row.id,
                    product_id_original: row.product_id_original,
                    product_name: &row.product_name,
                    record_reversal: true,
                    op,
                },
                now,
            )?;
        }
        connection
            .execute("DELETE FROM invoiceLines WHERE id = ?1", [row.id])
            .map_err(|error| map_db_error(error, op, &[]))?;
        occupied_positions.remove(&row.position);
        if let Some((line, _)) = referencing {
            if let Some(new_product_id) = line.product_id {
                if Some(new_product_id) != row.product_id {
                    position_overrides.insert(new_product_id, row.position);
                }
            }
        }
    }

    // Devolver en silencio lo consumido por las líneas que se conservan,
    // para volver a descontar con el stock completo disponible.
    for row in &current_lines {
        if !occupied_positions.contains(&row.position) {
            continue;
        }
        if row.product_id.is_none() {
            continue;
        }
        let still_referenced = parsed_lines
            .iter()
            .any(|(line, _)| line.product_id_original == Some(row.product_id_original));
        if !still_referenced {
            continue;
        }
        restore_outgoing_line_consumptions(
            connection,
            &actor,
            OutgoingLineRestoration {
                invoice_id,
                invoice_number: &invoice_number,
                invoice_type: type_code,
                line_id: row.id,
                product_id_original: row.product_id_original,
                product_name: &row.product_name,
                record_reversal: false,
                op,
            },
            now,
        )?;
    }

    let mut invoice_total_cents = 0_i64;
    let timestamp = format_utc_timestamp(now);
    for (line, quantity_milli) in parsed_lines {
        let original = line
            .product_id_original
            .and_then(|original_id| existing_by_original.get(&original_id));
        // Línea conservada con el mismo producto.
        if let Some(row) = original {
            if row.product_id.is_none() {
                if line.product_id.is_some() {
                    return Err(AppError::new(
                        "invalidState",
                        "Un producto eliminado no se puede reemplazar al editar la factura",
                        &format!("deleted {op} product was replaced"),
                    ));
                }
                if quantity_milli != row.quantity_milli {
                    return Err(AppError::new(
                        "invalidState",
                        &format!(
                            "El producto eliminado «{}» solo se puede quitar",
                            row.product_name
                        ),
                        &format!("deleted {op} product line was modified"),
                    ));
                }
                invoice_total_cents = invoice_total_cents
                    .checked_add(crate::inventory::round_half_up(
                        i128::from(row.quantity_milli) * i128::from(row.unit_price_cents),
                        1_000,
                    ) as i64)
                    .ok_or_else(|| {
                        validation_failed(
                            "El total de la factura supera el rango permitido",
                            &format!("{op} total overflow"),
                        )
                    })?;
                if invoice_total_cents > 99_999_999_999 {
                    return Err(validation_failed(
                        "El total de la factura no puede superar $999.999.999,99",
                        &format!("{op} total exceeds legal bound"),
                    ));
                }
                continue;
            }
            let row_product_id = row.product_id.unwrap_or_default();
            if line.product_id.is_some_and(|id| id != row_product_id) {
                // El cambio de producto se resolvió arriba (quitar + agregar).
                continue;
            }
            let (product_status, product_name): (String, String) = connection
                .query_row(
                    "SELECT status, name FROM products WHERE id = ?1",
                    [row_product_id],
                    |db_row| Ok((db_row.get(0)?, db_row.get(1)?)),
                )
                .optional()
                .map_err(|error| map_db_error(error, op, &[]))?
                .ok_or_else(|| {
                    AppError::new(
                        "notFound",
                        "El producto ya no existe",
                        &format!("{op} product disappeared during update"),
                    )
                })?;
            if product_status == "discontinued" && quantity_milli != row.quantity_milli {
                return Err(AppError::new(
                    "invalidState",
                    &format!(
                        "El producto «{product_name}» está descontinuado; solo se puede reducir la cantidad"
                    ),
                    &format!("discontinued {op} product was changed beyond reduction"),
                ));
            }
            // H-A4: sin cambio de cantidad no hay Ajuste; el precio emitido
            // se conserva y solo se refresca el costo del momento.
            consume_fifo(connection, row_product_id, quantity_milli, row.id, now)?;
            recalculate_product_quantity(connection, row_product_id)?;
            recalculate_product_cost(connection, row_product_id)?;
            let current_cost_cents: i64 = connection
                .query_row(
                    "SELECT costCents FROM products WHERE id = ?1",
                    [row_product_id],
                    |db_row| db_row.get(0),
                )
                .map_err(|error| map_db_error(error, op, &[]))?;
            let subtotal_cents = crate::inventory::round_half_up(
                i128::from(quantity_milli) * i128::from(row.unit_price_cents),
                1_000,
            ) as i64;
            connection
                .execute(
                    "UPDATE invoiceLines SET quantityMilli = ?1, subtotalCents = ?2,
                        costAtSaleCents = ?3, updatedAt = ?4 WHERE id = ?5",
                    params![
                        quantity_milli,
                        subtotal_cents,
                        current_cost_cents,
                        timestamp,
                        row.id,
                    ],
                )
                .map_err(|error| map_db_error(error, op, &[]))?;
            invoice_total_cents =
                invoice_total_cents
                    .checked_add(subtotal_cents)
                    .ok_or_else(|| {
                        validation_failed(
                            "El total de la factura supera el rango permitido",
                            &format!("{op} total overflow"),
                        )
                    })?;
            if invoice_total_cents > 99_999_999_999 {
                return Err(validation_failed(
                    "El total de la factura no puede superar $999.999.999,99",
                    &format!("{op} total exceeds legal bound"),
                ));
            }
            let delta_milli = quantity_milli - row.quantity_milli;
            if delta_milli != 0 {
                insert_outgoing_update_movement(
                    connection,
                    &actor,
                    OutgoingUpdateMovement {
                        invoice_id,
                        invoice_number: &invoice_number,
                        invoice_type: type_code,
                        product_id: row_product_id,
                        product_id_original: row.product_id_original,
                        product_name: &row.product_name,
                        movement_type: "adjustment",
                        // Con signo del stock (HP-01): subir la cantidad vendida saca stock.
                        quantity_milli: -delta_milli,
                        unit_price_cents: row.unit_price_cents,
                    },
                    now,
                    op,
                )?;
            }
            continue;
        }
        // Línea nueva (o con producto cambiado): precio de venta vigente.
        let product_id = line.product_id.ok_or_else(|| {
            validation_failed(
                "Cada línea nueva debe indicar un producto",
                &format!("new {op} line has no product"),
            )
        })?;
        if quantity_milli == 0 {
            return Err(validation_failed(
                "La cantidad de una línea nueva debe ser mayor que cero",
                &format!("new {op} line quantity is zero"),
            ));
        }
        let product: Option<PricedProductRow> = connection
            .query_row(
                "SELECT p.name, p.costCents, p.profitPercentHundredths, p.categoryId, c.name,
                    p.unitId, u.name, u.abbreviation, p.status
                 FROM products p
                 LEFT JOIN categories c ON c.id = p.categoryId
                 LEFT JOIN units u ON u.id = p.unitId
                 WHERE p.id = ?1",
                [product_id],
                |db_row| {
                    Ok((
                        db_row.get(0)?,
                        db_row.get(1)?,
                        db_row.get(2)?,
                        db_row.get(3)?,
                        db_row.get(4)?,
                        db_row.get(5)?,
                        db_row.get(6)?,
                        db_row.get(7)?,
                        db_row.get(8)?,
                    ))
                },
            )
            .optional()
            .map_err(|error| map_db_error(error, op, &[]))?;
        let Some((
            product_name,
            cost_cents,
            profit_percent_hundredths,
            category_id,
            category_name,
            unit_id,
            unit_name,
            unit_abbreviation,
            product_status,
        )) = product
        else {
            return Err(AppError::new(
                "notFound",
                "El producto ya no existe",
                &format!("new {op} line product missing"),
            ));
        };
        if product_status == "discontinued" {
            return Err(validation_failed(
                &format!(
                    "El producto «{product_name}» está descontinuado. Actívalo para usarlo en facturas"
                ),
                &format!("discontinued product added to {op}"),
            ));
        }
        let available_milli: i64 = connection
            .query_row(
                "SELECT COALESCE(SUM(remainingMilli), 0) FROM lots WHERE productId = ?1",
                [product_id],
                |row| row.get(0),
            )
            .map_err(|error| map_db_error(error, op, &[]))?;
        if available_milli == 0 {
            return Err(validation_failed(
                &format!("El producto «{product_name}» está agotado"),
                &format!("out of stock product added to {op}"),
            ));
        }
        let unit_price_cents = selling_price_cents(cost_cents, profit_percent_hundredths);
        let subtotal_cents = crate::inventory::round_half_up(
            i128::from(quantity_milli) * i128::from(unit_price_cents),
            1_000,
        ) as i64;
        invoice_total_cents = invoice_total_cents
            .checked_add(subtotal_cents)
            .ok_or_else(|| {
                validation_failed(
                    "El total de la factura supera el rango permitido",
                    &format!("{op} total overflow"),
                )
            })?;
        if invoice_total_cents > 99_999_999_999 {
            return Err(validation_failed(
                "El total de la factura no puede superar $999.999.999,99",
                &format!("{op} total exceeds legal bound"),
            ));
        }
        let position = if let Some(position) = position_overrides.get(&product_id) {
            *position
        } else {
            (1..=100)
                .find(|position| !occupied_positions.contains(position))
                .ok_or_else(|| {
                    validation_failed(
                        "Una factura admite entre 1 y 100 productos",
                        &format!("no free {op} line position"),
                    )
                })?
        };
        let product_name_normalized = normalize_text(&product_name);
        connection
            .execute(
                "INSERT INTO invoiceLines (invoiceId, position, productId, productIdOriginal, productName,
                    productNameNormalized, unitId, unitName, unitAbbreviation, quantityMilli, unitPriceCents,
                    subtotalCents, categoryId, categoryName, costAtSaleCents, createdAt, updatedAt)
                 VALUES (?1, ?2, ?3, ?3, ?4, ?5, ?6, ?7, ?8, ?9, ?10, ?11, ?12, ?13, ?14, ?15, ?15)",
                params![
                    invoice_id,
                    position,
                    product_id,
                    product_name,
                    product_name_normalized,
                    unit_id,
                    unit_name,
                    unit_abbreviation,
                    quantity_milli,
                    unit_price_cents,
                    subtotal_cents,
                    category_id,
                    category_name,
                    cost_cents,
                    timestamp,
                ],
            )
            .map_err(|error| map_db_error(error, op, &[]))?;
        let line_id = connection.last_insert_rowid();
        consume_fifo(connection, product_id, quantity_milli, line_id, now)?;
        recalculate_product_quantity(connection, product_id)?;
        recalculate_product_cost(connection, product_id)?;
        insert_outgoing_update_movement(
            connection,
            &actor,
            OutgoingUpdateMovement {
                invoice_id,
                invoice_number: &invoice_number,
                invoice_type: type_code,
                product_id,
                product_id_original: product_id,
                product_name: &product_name,
                movement_type: sale_movement,
                quantity_milli: -quantity_milli,
                unit_price_cents,
            },
            now,
            op,
        )?;
        occupied_positions.insert(position);
    }

    let full_name = format!("{client_first_name} {client_last_name}");
    connection
        .execute(
            "UPDATE invoices SET clientId = ?1, invoiceDate = ?2, notes = ?3,
                contactFullName = ?4, contactFullNameNormalized = ?5, contactNationalId = ?6,
                contactPhone = ?7, updatedAt = ?8,
                reservedUntil = CASE WHEN status = 'pending' THEN ?10 ELSE reservedUntil END
                WHERE id = ?9",
            params![
                client_id,
                invoice_date,
                notes,
                full_name,
                normalize_text(&format!("{client_first_name} {client_last_name}")),
                client_national_id,
                client_phone,
                timestamp,
                invoice_id,
                reserved_until,
            ],
        )
        .map_err(|error| map_db_error(error, op, &[]))?;

    // NT-02/NT-03: avisos de situación con el estado final (best-effort).
    crate::automation::evaluate_watches_best_effort(&transaction, now);
    transaction
        .commit()
        .map_err(|error| map_db_error(error, op, &[]))?;
    Ok(InvoiceActionResult {
        invoice_id,
        number: invoice_number,
        expired_cleanup_warning,
    })
}
/// Núcleo de edición de garantías e insumos (FS-04, FS-05): el costo se
/// recalcula con los lotes que resulten consumidos; el cambio de costo por
/// re-FIFO sin cambio de cantidad no genera Ajuste (H-A4).
#[allow(clippy::too_many_arguments)]
fn update_costed_outgoing_invoice_inner(
    connection: &mut Connection,
    now: DateTime<Utc>,
    op: &str,
    invoice_type: InvoiceType,
    sale_movement: &str,
    allowed_associated: &[&str],
    associated_error: &str,
    invoice_id: i64,
    client_id: i64,
    associated_invoice_id: i64,
    invoice_date: &str,
    notes: Option<&str>,
    lines: &[UpdateOutgoingInvoiceLineInput],
) -> Result<InvoiceActionResult, AppError> {
    let type_code = invoice_type.as_str();
    let actor = require_session(connection, now)?;
    let transaction = connection
        .transaction()
        .map_err(|error| map_db_error(error, op, &[]))?;
    // Purga best-effort de P-08 (spec-03 §7.6); su aviso viaja en el resultado.
    let expired_cleanup_warning = purge_best_effort(&transaction, now)?;
    let connection = &transaction;

    let header: Option<(String, String)> = connection
        .query_row(
            "SELECT status, number FROM invoices WHERE id = ?1 AND type = ?2",
            params![invoice_id, type_code],
            |row| Ok((row.get(0)?, row.get(1)?)),
        )
        .optional()
        .map_err(|error| map_db_error(error, op, &[]))?;
    let Some((status, invoice_number)) = header else {
        return Err(AppError::new(
            "notFound",
            "La factura ya no existe",
            &format!("{op} invoice missing during update"),
        ));
    };
    validate_edit_allowed(invoice_type, InvoiceStatus::from_str(&status)?)?;

    if lines.is_empty() || lines.len() > 100 {
        return Err(validation_failed(
            "Una factura admite entre 1 y 100 productos",
            &format!("{op} line count out of range"),
        ));
    }
    let invoice_date = parse_invoice_date(invoice_date)?;
    if invoice_date > today_local(now) {
        return Err(validation_failed(
            "La fecha no puede ser futura",
            &format!("{op} date is in the future"),
        ));
    }
    let client: Option<(String, String, String, String)> = connection
        .query_row(
            "SELECT firstName, lastName, nationalId, phone FROM clients WHERE id = ?1",
            [client_id],
            |row| Ok((row.get(0)?, row.get(1)?, row.get(2)?, row.get(3)?)),
        )
        .optional()
        .map_err(|error| map_db_error(error, op, &[]))?;
    let Some((client_first_name, client_last_name, client_national_id, client_phone)) = client
    else {
        return Err(AppError::new(
            "notFound",
            "El cliente ya no existe",
            &format!("client missing during {op} update"),
        ));
    };
    let associated: Option<(String, String, Option<i64>, Option<String>)> = connection
        .query_row(
            "SELECT type, number, clientId, status FROM invoices WHERE id = ?1",
            [associated_invoice_id],
            |row| Ok((row.get(0)?, row.get(1)?, row.get(2)?, row.get(3)?)),
        )
        .optional()
        .map_err(|error| map_db_error(error, op, &[]))?;
    let Some((associated_type, associated_number, associated_client_id, associated_status)) =
        associated
    else {
        return Err(AppError::new(
            "notFound",
            "La factura ya no existe",
            &format!("associated invoice missing during {op} update"),
        ));
    };
    if !allowed_associated.contains(&associated_type.as_str())
        || associated_client_id != Some(client_id)
        || associated_status.as_deref() == Some("cancelled")
    {
        return Err(validation_failed(
            associated_error,
            &format!("{op} associated invoice rejected"),
        ));
    }
    let notes = notes.map(validate_invoice_notes).transpose()?.flatten();

    let current_lines: Vec<OutgoingInvoiceLineRow> = {
        let mut statement = connection
            .prepare(
                "SELECT id, position, productId, productIdOriginal, productName,
                    quantityMilli, unitPriceCents
                 FROM invoiceLines WHERE invoiceId = ?1 ORDER BY position",
            )
            .map_err(|error| map_db_error(error, op, &[]))?;
        let lines = statement
            .query_map([invoice_id], |row| {
                Ok(OutgoingInvoiceLineRow {
                    id: row.get(0)?,
                    position: row.get(1)?,
                    product_id: row.get(2)?,
                    product_id_original: row.get(3)?,
                    product_name: row.get(4)?,
                    quantity_milli: row.get(5)?,
                    unit_price_cents: row.get(6)?,
                })
            })
            .map_err(|error| map_db_error(error, op, &[]))?
            .collect::<rusqlite::Result<Vec<_>>>()
            .map_err(|error| map_db_error(error, op, &[]))?;
        lines
    };
    let existing_by_original: std::collections::HashMap<i64, OutgoingInvoiceLineRow> =
        current_lines
            .iter()
            .cloned()
            .map(|row| (row.product_id_original, row))
            .collect();

    let mut seen_original_ids = std::collections::HashSet::new();
    let mut seen_product_ids = std::collections::HashSet::new();
    let mut parsed_lines = Vec::with_capacity(lines.len());
    for line in lines {
        if let Some(product_id) = line.product_id {
            if !seen_product_ids.insert(product_id) {
                return Err(validation_failed(
                    "El producto ya está en la factura",
                    &format!("duplicate product in {op}"),
                ));
            }
        }
        if let Some(original_id) = line.product_id_original {
            if !seen_original_ids.insert(original_id)
                || !existing_by_original.contains_key(&original_id)
            {
                return Err(validation_failed(
                    "Las líneas de la factura no son válidas",
                    &format!("{op} line reference is invalid"),
                ));
            }
        } else if line
            .product_id
            .is_some_and(|product_id| existing_by_original.contains_key(&product_id))
        {
            return Err(validation_failed(
                "Las líneas existentes deben conservar su identificador original",
                &format!("existing {op} line omitted productIdOriginal"),
            ));
        }
        let quantity_milli = parse_entry_update_quantity(&line.quantity).map_err(|error| {
            AppError::new(
                "validationFailed",
                "La cantidad de la línea no es válida",
                &format!("invalid updated {op} quantity: {}", error.message),
            )
        })?;
        parsed_lines.push((line, quantity_milli));
    }

    let mut occupied_positions: std::collections::BTreeSet<i64> =
        current_lines.iter().map(|row| row.position).collect();
    let mut position_overrides: std::collections::HashMap<i64, i64> =
        std::collections::HashMap::new();
    for row in &current_lines {
        let referencing = parsed_lines
            .iter()
            .find(|(line, _)| line.product_id_original == Some(row.product_id_original));
        let remove = match referencing {
            None => true,
            Some((line, quantity_milli)) => {
                *quantity_milli == 0 || line.product_id.is_some_and(|id| Some(id) != row.product_id)
            }
        };
        if !remove {
            continue;
        }
        if row.product_id.is_some() {
            restore_outgoing_line_consumptions(
                connection,
                &actor,
                OutgoingLineRestoration {
                    invoice_id,
                    invoice_number: &invoice_number,
                    invoice_type: type_code,
                    line_id: row.id,
                    product_id_original: row.product_id_original,
                    product_name: &row.product_name,
                    record_reversal: true,
                    op,
                },
                now,
            )?;
        }
        connection
            .execute("DELETE FROM invoiceLines WHERE id = ?1", [row.id])
            .map_err(|error| map_db_error(error, op, &[]))?;
        occupied_positions.remove(&row.position);
        if let Some((line, _)) = referencing {
            if let Some(new_product_id) = line.product_id {
                if Some(new_product_id) != row.product_id {
                    position_overrides.insert(new_product_id, row.position);
                }
            }
        }
    }

    for row in &current_lines {
        if !occupied_positions.contains(&row.position) {
            continue;
        }
        if row.product_id.is_none() {
            continue;
        }
        let still_referenced = parsed_lines
            .iter()
            .any(|(line, _)| line.product_id_original == Some(row.product_id_original));
        if !still_referenced {
            continue;
        }
        restore_outgoing_line_consumptions(
            connection,
            &actor,
            OutgoingLineRestoration {
                invoice_id,
                invoice_number: &invoice_number,
                invoice_type: type_code,
                line_id: row.id,
                product_id_original: row.product_id_original,
                product_name: &row.product_name,
                record_reversal: false,
                op,
            },
            now,
        )?;
    }

    let mut invoice_total_cents = 0_i64;
    let timestamp = format_utc_timestamp(now);
    for (line, quantity_milli) in parsed_lines {
        let original = line
            .product_id_original
            .and_then(|original_id| existing_by_original.get(&original_id));
        if let Some(row) = original {
            if row.product_id.is_none() {
                if line.product_id.is_some() {
                    return Err(AppError::new(
                        "invalidState",
                        "Un producto eliminado no se puede reemplazar al editar la factura",
                        &format!("deleted {op} product was replaced"),
                    ));
                }
                if quantity_milli != row.quantity_milli {
                    return Err(AppError::new(
                        "invalidState",
                        &format!(
                            "El producto eliminado «{}» solo se puede quitar",
                            row.product_name
                        ),
                        &format!("deleted {op} product line was modified"),
                    ));
                }
                invoice_total_cents = invoice_total_cents
                    .checked_add(crate::inventory::round_half_up(
                        i128::from(row.quantity_milli) * i128::from(row.unit_price_cents),
                        1_000,
                    ) as i64)
                    .ok_or_else(|| {
                        validation_failed(
                            "El total de la factura supera el rango permitido",
                            &format!("{op} total overflow"),
                        )
                    })?;
                if invoice_total_cents > 99_999_999_999 {
                    return Err(validation_failed(
                        "El total de la factura no puede superar $999.999.999,99",
                        &format!("{op} total exceeds legal bound"),
                    ));
                }
                continue;
            }
            let row_product_id = row.product_id.unwrap_or_default();
            if line.product_id.is_some_and(|id| id != row_product_id) {
                continue;
            }
            let (product_status, product_name): (String, String) = connection
                .query_row(
                    "SELECT status, name FROM products WHERE id = ?1",
                    [row_product_id],
                    |db_row| Ok((db_row.get(0)?, db_row.get(1)?)),
                )
                .optional()
                .map_err(|error| map_db_error(error, op, &[]))?
                .ok_or_else(|| {
                    AppError::new(
                        "notFound",
                        "El producto ya no existe",
                        &format!("{op} product disappeared during update"),
                    )
                })?;
            if product_status == "discontinued" && quantity_milli != row.quantity_milli {
                return Err(AppError::new(
                    "invalidState",
                    &format!(
                        "El producto «{product_name}» está descontinuado; solo se puede reducir la cantidad"
                    ),
                    &format!("discontinued {op} product was changed beyond reduction"),
                ));
            }
            let fresh_consumptions =
                consume_fifo(connection, row_product_id, quantity_milli, row.id, now)?;
            recalculate_product_quantity(connection, row_product_id)?;
            recalculate_product_cost(connection, row_product_id)?;
            let line_total_cents = line_total_cost_cents(&fresh_consumptions);
            let line_unit_cost_cents = line_unit_cost_cents(line_total_cents, quantity_milli);
            connection
                .execute(
                    "UPDATE invoiceLines SET quantityMilli = ?1, unitPriceCents = ?2, subtotalCents = ?3,
                        updatedAt = ?4 WHERE id = ?5",
                    params![
                        quantity_milli,
                        line_unit_cost_cents,
                        line_total_cents,
                        timestamp,
                        row.id,
                    ],
                )
                .map_err(|error| map_db_error(error, op, &[]))?;
            invoice_total_cents = invoice_total_cents
                .checked_add(line_total_cents)
                .ok_or_else(|| {
                    validation_failed(
                        "El total de la factura supera el rango permitido",
                        &format!("{op} total overflow"),
                    )
                })?;
            if invoice_total_cents > 99_999_999_999 {
                return Err(validation_failed(
                    "El total de la factura no puede superar $999.999.999,99",
                    &format!("{op} total exceeds legal bound"),
                ));
            }
            let delta_milli = quantity_milli - row.quantity_milli;
            if delta_milli != 0 {
                insert_outgoing_update_movement(
                    connection,
                    &actor,
                    OutgoingUpdateMovement {
                        invoice_id,
                        invoice_number: &invoice_number,
                        invoice_type: type_code,
                        product_id: row_product_id,
                        product_id_original: row.product_id_original,
                        product_name: &row.product_name,
                        movement_type: "adjustment",
                        // Con signo del stock (HP-01): subir la cantidad vendida saca stock.
                        quantity_milli: -delta_milli,
                        unit_price_cents: line_unit_cost_cents,
                    },
                    now,
                    op,
                )?;
            }
            continue;
        }
        let product_id = line.product_id.ok_or_else(|| {
            validation_failed(
                "Cada línea nueva debe indicar un producto",
                &format!("new {op} line has no product"),
            )
        })?;
        if quantity_milli == 0 {
            return Err(validation_failed(
                "La cantidad de una línea nueva debe ser mayor que cero",
                &format!("new {op} line quantity is zero"),
            ));
        }
        let product: Option<CostedProductRow> = connection
            .query_row(
                "SELECT p.name, p.unitId, u.name, u.abbreviation, p.status
                     FROM products p
                     LEFT JOIN units u ON u.id = p.unitId
                     WHERE p.id = ?1",
                [product_id],
                |db_row| {
                    Ok((
                        db_row.get(0)?,
                        db_row.get(1)?,
                        db_row.get(2)?,
                        db_row.get(3)?,
                        db_row.get(4)?,
                    ))
                },
            )
            .optional()
            .map_err(|error| map_db_error(error, op, &[]))?;
        let Some((product_name, unit_id, unit_name, unit_abbreviation, product_status)) = product
        else {
            return Err(AppError::new(
                "notFound",
                "El producto ya no existe",
                &format!("new {op} line product missing"),
            ));
        };
        if product_status == "discontinued" {
            return Err(validation_failed(
                &format!(
                    "El producto «{product_name}» está descontinuado. Actívalo para usarlo en facturas"
                ),
                &format!("discontinued product added to {op}"),
            ));
        }
        let available_milli: i64 = connection
            .query_row(
                "SELECT COALESCE(SUM(remainingMilli), 0) FROM lots WHERE productId = ?1",
                [product_id],
                |row| row.get(0),
            )
            .map_err(|error| map_db_error(error, op, &[]))?;
        if available_milli == 0 {
            return Err(validation_failed(
                &format!("El producto «{product_name}» está agotado"),
                &format!("out of stock product added to {op}"),
            ));
        }
        let position = if let Some(position) = position_overrides.get(&product_id) {
            *position
        } else {
            (1..=100)
                .find(|position| !occupied_positions.contains(position))
                .ok_or_else(|| {
                    validation_failed(
                        "Una factura admite entre 1 y 100 productos",
                        &format!("no free {op} line position"),
                    )
                })?
        };
        let product_name_normalized = normalize_text(&product_name);
        connection
            .execute(
                "INSERT INTO invoiceLines (invoiceId, position, productId, productIdOriginal, productName,
                    productNameNormalized, unitId, unitName, unitAbbreviation, quantityMilli, unitPriceCents,
                    subtotalCents, createdAt, updatedAt)
                 VALUES (?1, ?2, ?3, ?3, ?4, ?5, ?6, ?7, ?8, ?9, 0, 0, ?10, ?10)",
                params![
                    invoice_id,
                    position,
                    product_id,
                    product_name,
                    product_name_normalized,
                    unit_id,
                    unit_name,
                    unit_abbreviation,
                    quantity_milli,
                    timestamp,
                ],
            )
            .map_err(|error| map_db_error(error, op, &[]))?;
        let line_id = connection.last_insert_rowid();
        let new_consumptions = consume_fifo(connection, product_id, quantity_milli, line_id, now)?;
        let new_total_cents = line_total_cost_cents(&new_consumptions);
        let new_unit_cost_cents = line_unit_cost_cents(new_total_cents, quantity_milli);
        invoice_total_cents = invoice_total_cents
            .checked_add(new_total_cents)
            .ok_or_else(|| {
                validation_failed(
                    "El total de la factura supera el rango permitido",
                    &format!("{op} total overflow"),
                )
            })?;
        if invoice_total_cents > 99_999_999_999 {
            return Err(validation_failed(
                "El total de la factura no puede superar $999.999.999,99",
                &format!("{op} total exceeds legal bound"),
            ));
        }
        connection
            .execute(
                "UPDATE invoiceLines SET unitPriceCents = ?1, subtotalCents = ?2, updatedAt = ?3 WHERE id = ?4",
                params![new_unit_cost_cents, new_total_cents, &timestamp, line_id],
            )
            .map_err(|error| map_db_error(error, op, &[]))?;
        recalculate_product_quantity(connection, product_id)?;
        recalculate_product_cost(connection, product_id)?;
        insert_outgoing_update_movement(
            connection,
            &actor,
            OutgoingUpdateMovement {
                invoice_id,
                invoice_number: &invoice_number,
                invoice_type: type_code,
                product_id,
                product_id_original: product_id,
                product_name: &product_name,
                movement_type: sale_movement,
                quantity_milli: -quantity_milli,
                unit_price_cents: new_unit_cost_cents,
            },
            now,
            op,
        )?;
        occupied_positions.insert(position);
    }

    let full_name = format!("{client_first_name} {client_last_name}");
    connection
        .execute(
            "UPDATE invoices SET clientId = ?1, invoiceDate = ?2, notes = ?3,
                contactFullName = ?4, contactFullNameNormalized = ?5, contactNationalId = ?6,
                contactPhone = ?7, associatedInvoiceId = ?8, associatedNumber = ?9, associatedType = ?10,
                updatedAt = ?11 WHERE id = ?12",
            params![
                client_id,
                invoice_date,
                notes,
                full_name,
                normalize_text(&format!("{client_first_name} {client_last_name}")),
                client_national_id,
                client_phone,
                associated_invoice_id,
                associated_number,
                associated_type,
                timestamp,
                invoice_id,
            ],
        )
        .map_err(|error| map_db_error(error, op, &[]))?;

    // NT-02/NT-03: avisos de situación con el estado final (best-effort).
    crate::automation::evaluate_watches_best_effort(&transaction, now);
    transaction
        .commit()
        .map_err(|error| map_db_error(error, op, &[]))?;
    Ok(InvoiceActionResult {
        invoice_id,
        number: invoice_number,
        expired_cleanup_warning,
    })
}
pub fn update_sale_invoice_inner(
    connection: &mut Connection,
    now: DateTime<Utc>,
    input: UpdateSaleInvoiceInput,
) -> Result<InvoiceActionResult, AppError> {
    let lines = input.lines;
    let result = update_priced_outgoing_invoice_inner(
        connection,
        now,
        "updateSaleInvoice",
        InvoiceType::Sale,
        "saleOut",
        input.invoice_id,
        input.client_id,
        &input.invoice_date,
        input.notes.as_deref(),
        &lines
            .iter()
            .map(|line| UpdateOutgoingInvoiceLineInput {
                product_id_original: line.product_id_original,
                product_id: line.product_id,
                quantity: line.quantity.clone(),
            })
            .collect::<Vec<_>>(),
        input.reserved_until.as_deref(),
    );
    crate::automation::after_rejected_outgoing(connection, now, result)
}

pub fn update_service_invoice_inner(
    connection: &mut Connection,
    now: DateTime<Utc>,
    input: UpdateServiceInvoiceInput,
) -> Result<InvoiceActionResult, AppError> {
    let lines = input.lines;
    let result = update_priced_outgoing_invoice_inner(
        connection,
        now,
        "updateServiceInvoice",
        InvoiceType::Service,
        "serviceOut",
        input.invoice_id,
        input.client_id,
        &input.invoice_date,
        input.notes.as_deref(),
        &lines
            .iter()
            .map(|line| UpdateOutgoingInvoiceLineInput {
                product_id_original: line.product_id_original,
                product_id: line.product_id,
                quantity: line.quantity.clone(),
            })
            .collect::<Vec<_>>(),
        input.reserved_until.as_deref(),
    );
    crate::automation::after_rejected_outgoing(connection, now, result)
}

pub fn update_guarantee_invoice_inner(
    connection: &mut Connection,
    now: DateTime<Utc>,
    input: UpdateGuaranteeInvoiceInput,
) -> Result<InvoiceActionResult, AppError> {
    let lines = input.lines;
    let result = update_costed_outgoing_invoice_inner(
        connection,
        now,
        "updateGuaranteeInvoice",
        InvoiceType::Guarantee,
        "warrantyOut",
        &["V", "S"],
        "La factura original debe ser una venta o servicio de este cliente que no esté cancelada",
        input.invoice_id,
        input.client_id,
        input.associated_invoice_id,
        &input.invoice_date,
        input.notes.as_deref(),
        &lines
            .iter()
            .map(|line| UpdateOutgoingInvoiceLineInput {
                product_id_original: line.product_id_original,
                product_id: line.product_id,
                quantity: line.quantity.clone(),
            })
            .collect::<Vec<_>>(),
    );
    crate::automation::after_rejected_outgoing(connection, now, result)
}

pub fn update_installation_invoice_inner(
    connection: &mut Connection,
    now: DateTime<Utc>,
    input: UpdateInstallationInvoiceInput,
) -> Result<InvoiceActionResult, AppError> {
    let lines = input.lines;
    let result = update_costed_outgoing_invoice_inner(
        connection,
        now,
        "updateInstallationInvoice",
        InvoiceType::Installation,
        "supplyOut",
        &["S"],
        "La factura de servicio debe ser de este cliente y no estar cancelada",
        input.invoice_id,
        input.client_id,
        input.associated_invoice_id,
        &input.invoice_date,
        input.notes.as_deref(),
        &lines
            .iter()
            .map(|line| UpdateOutgoingInvoiceLineInput {
                product_id_original: line.product_id_original,
                product_id: line.product_id,
                quantity: line.quantity.clone(),
            })
            .collect::<Vec<_>>(),
    );
    crate::automation::after_rejected_outgoing(connection, now, result)
}
pub fn create_loss_invoice_inner(
    connection: &mut Connection,
    now: DateTime<Utc>,
    input: CreateLossInvoiceInput,
) -> Result<InvoiceActionResult, AppError> {
    let result = create_loss_invoice_tx(connection, now, input);
    crate::automation::after_rejected_outgoing(connection, now, result)
}

fn create_loss_invoice_tx(
    connection: &mut Connection,
    now: DateTime<Utc>,
    input: CreateLossInvoiceInput,
) -> Result<InvoiceActionResult, AppError> {
    let _actor = require_session(connection, now)?;
    let transaction = connection
        .transaction()
        .map_err(|error| map_db_error(error, "createLossInvoice", &[]))?;
    // Purga best-effort de P-08 (spec-03 §7.6); su aviso viaja en el resultado.
    let expired_cleanup_warning = purge_best_effort(&transaction, now)?;
    let connection = &transaction;

    let quantity_milli = parse_quantity_milli(&input.quantity).map_err(|error| {
        AppError::new(
            "validationFailed",
            "La cantidad de la línea no es válida",
            &format!("invalid quantity for loss: {}", error.message),
        )
    })?;

    let invoice_date = parse_invoice_date(&input.invoice_date)?;
    let today = today_local(now);
    if invoice_date > today {
        return Err(validation_failed(
            "La fecha no puede ser futura",
            "loss invoice date is in the future",
        ));
    }

    let product: (String, String, Option<i64>, Option<String>, Option<String>) = connection
        .query_row(
            "SELECT p.name, p.status, p.unitId, u.name, u.abbreviation
             FROM products p
             LEFT JOIN units u ON u.id = p.unitId
             WHERE p.id = ?1",
            [input.product_id],
            |row| {
                Ok((
                    row.get(0)?,
                    row.get(1)?,
                    row.get(2)?,
                    row.get(3)?,
                    row.get(4)?,
                ))
            },
        )
        .optional()
        .map_err(|error| map_db_error(error, "createLossInvoice", &[]))?
        .ok_or_else(|| {
            AppError::new(
                "notFound",
                "El producto ya no existe",
                "product missing during loss invoice creation",
            )
        })?;
    let (product_name, _product_status, unit_id, unit_name, unit_abbreviation) = product;
    // Excepción de S-22 (FP-01): un descontinuado CON stock sí se puede
    // perder, así que el estado no bloquea; sin stock ya se rechazó arriba.
    let available_milli: i64 = connection
        .query_row(
            "SELECT COALESCE(SUM(remainingMilli), 0) FROM lots WHERE productId = ?1",
            [input.product_id],
            |row| row.get(0),
        )
        .map_err(|error| map_db_error(error, "createLossInvoice", &[]))?;
    if available_milli == 0 {
        return Err(validation_failed(
            &format!("El producto «{product_name}» está agotado"),
            "out of stock product used in loss invoice",
        ));
    }
    // Excepción de S-22 (FP-01): un descontinuado CON stock sí se puede
    // perder; sin stock ya se rechazó arriba.
    let notes = input
        .notes
        .as_deref()
        .map(validate_invoice_notes)
        .transpose()?
        .flatten();

    let number = next_invoice_number(connection, "P")?;
    let timestamp = format_utc_timestamp(now);

    // Sin estados, sin cliente y sin pago: todo queda en NULL (FP-01).
    connection
        .execute(
            "INSERT INTO invoices (type, number, invoiceDate, status, notes,
            createdByUserId, createdByName, createdAt, updatedAt)
         VALUES (?1, ?2, ?3, NULL, ?4, ?5, ?6, ?7, ?7)",
            params![
                "P",
                &number,
                &input.invoice_date,
                notes.as_deref(),
                _actor.id(),
                _actor.username(),
                &timestamp,
            ],
        )
        .map_err(|error| map_db_error(error, "createLossInvoice", &[]))?;

    let invoice_id = connection.last_insert_rowid();
    let product_name_normalized = normalize_text(&product_name);
    connection.execute(
        "INSERT INTO invoiceLines (invoiceId, position, productId, productIdOriginal, productName, productNameNormalized,
            unitId, unitName, unitAbbreviation, quantityMilli, unitPriceCents, subtotalCents, createdAt, updatedAt)
         VALUES (?1, 1, ?2, ?2, ?3, ?4, ?5, ?6, ?7, ?8, 0, 0, ?9, ?9)",
        params![
            invoice_id,
            input.product_id,
            product_name,
            product_name_normalized,
            unit_id,
            unit_name,
            unit_abbreviation,
            quantity_milli,
            &timestamp,
        ],
    ).map_err(|error| map_db_error(error, "createLossInvoice", &[]))?;

    let line_id = connection.last_insert_rowid();
    // El costo lo calcula el sistema con el consumo FIFO (LT-02, FP-01).
    let consumptions = consume_fifo(connection, input.product_id, quantity_milli, line_id, now)?;
    let line_total_cents = line_total_cost_cents(&consumptions);
    let line_unit_cost_cents = line_unit_cost_cents(line_total_cents, quantity_milli);
    if line_total_cents > 99_999_999_999 {
        return Err(validation_failed(
            "El total de la factura no puede superar $999.999.999,99",
            "loss invoice total exceeds legal bound",
        ));
    }
    connection.execute(
        "UPDATE invoiceLines SET unitPriceCents = ?1, subtotalCents = ?2, updatedAt = ?3 WHERE id = ?4",
        params![line_unit_cost_cents, line_total_cents, &timestamp, line_id],
    ).map_err(|error| map_db_error(error, "createLossInvoice", &[]))?;

    recalculate_product_quantity(connection, input.product_id)?;
    recalculate_product_cost(connection, input.product_id)?;

    let stock_after: i64 = connection
        .query_row(
            "SELECT quantityMilli FROM products WHERE id = ?1",
            [input.product_id],
            |row| row.get(0),
        )
        .map_err(|error| map_db_error(error, "createLossInvoice", &[]))?;
    connection.execute(
        "INSERT INTO productMovements (createdAt, productId, productIdOriginal, productName, type, quantityMilli,
            stockAfterMilli, unitPriceCents, totalPriceCents, invoiceId, invoiceNumber, invoiceType, userId, userName)
         VALUES (?1, ?2, ?3, ?4, 'loss', ?5, ?6, ?7, ?8, ?9, ?10, ?11, ?12, ?13)",
        params![
            &timestamp,
            input.product_id,
            input.product_id,
            product_name,
            -quantity_milli,
            stock_after,
            line_unit_cost_cents,
            line_total_cents,
            invoice_id,
            &number,
            "P",
            _actor.id(),
            _actor.username(),
        ],
    ).map_err(|error| map_db_error(error, "createLossInvoice", &[]))?;

    // NT-02/NT-03: avisos de situación con el estado final (best-effort).
    crate::automation::evaluate_watches_best_effort(&transaction, now);
    transaction
        .commit()
        .map_err(|error| map_db_error(error, "createLossInvoice", &[]))?;

    Ok(InvoiceActionResult {
        invoice_id,
        number,
        expired_cleanup_warning,
    })
}

pub fn update_entry_invoice_inner(
    connection: &mut Connection,
    now: DateTime<Utc>,
    input: UpdateEntryInvoiceInput,
) -> Result<InvoiceActionResult, AppError> {
    let actor = require_session(connection, now)?;
    let transaction = connection
        .transaction()
        .map_err(|error| map_db_error(error, "updateEntryInvoice", &[]))?;
    // Purga best-effort de P-08 (spec-03 §7.6); su aviso viaja en el resultado.
    let expired_cleanup_warning = purge_best_effort(&transaction, now)?;
    let connection = &transaction;

    let header: Option<(String, String)> = connection
        .query_row(
            "SELECT status, number FROM invoices WHERE id = ?1 AND type = 'E'",
            [input.invoice_id],
            |row| Ok((row.get(0)?, row.get(1)?)),
        )
        .optional()
        .map_err(|error| map_db_error(error, "updateEntryInvoice", &[]))?;
    let Some((status, invoice_number)) = header else {
        return Err(AppError::new(
            "notFound",
            "La factura ya no existe",
            "entry invoice missing during update",
        ));
    };
    validate_edit_allowed(InvoiceType::Entry, InvoiceStatus::from_str(&status)?)?;

    if input.lines.is_empty() || input.lines.len() > 100 {
        return Err(validation_failed(
            "La factura debe tener entre 1 y 100 líneas",
            "updated entry invoice line count out of range",
        ));
    }
    let invoice_date = parse_invoice_date(&input.invoice_date)?;
    if invoice_date > today_local(now) {
        return Err(validation_failed(
            "La fecha no puede ser futura",
            "updated entry invoice date is in the future",
        ));
    }
    let supplier: Option<(String, String, String, String)> = connection
        .query_row(
            "SELECT firstName, lastName, nationalId, phone FROM suppliers WHERE id = ?1",
            [input.supplier_id],
            |row| Ok((row.get(0)?, row.get(1)?, row.get(2)?, row.get(3)?)),
        )
        .optional()
        .map_err(|error| map_db_error(error, "updateEntryInvoice", &[]))?;
    let Some((supplier_first_name, supplier_last_name, supplier_national_id, supplier_phone)) =
        supplier
    else {
        return Err(AppError::new(
            "notFound",
            "El proveedor ya no existe",
            "supplier missing during entry invoice update",
        ));
    };
    let notes = input
        .notes
        .as_deref()
        .map(validate_invoice_notes)
        .transpose()?
        .flatten();

    let current_lines: Vec<EntryInvoiceLineRow> = {
        let mut statement = connection
            .prepare(
                "SELECT il.id, il.position, il.productId, il.productIdOriginal, il.productName,
                    il.quantityMilli, il.unitPriceCents, l.id, l.initialMilli, l.remainingMilli, l.unitCostCents
                 FROM invoiceLines il LEFT JOIN lots l ON l.invoiceLineId = il.id
                 WHERE il.invoiceId = ?1 ORDER BY il.position",
            )
            .map_err(|error| map_db_error(error, "updateEntryInvoice", &[]))?;
        let rows = statement
            .query_map([input.invoice_id], |row| {
                Ok(EntryInvoiceLineRow {
                    id: row.get(0)?,
                    position: row.get(1)?,
                    product_id: row.get(2)?,
                    product_id_original: row.get(3)?,
                    product_name: row.get(4)?,
                    quantity_milli: row.get(5)?,
                    unit_price_cents: row.get(6)?,
                    lot_id: row.get(7)?,
                    lot_initial_milli: row.get(8)?,
                    lot_remaining_milli: row.get(9)?,
                    lot_unit_cost_cents: row.get(10)?,
                })
            })
            .map_err(|error| map_db_error(error, "updateEntryInvoice", &[]))?
            .collect::<rusqlite::Result<Vec<_>>>()
            .map_err(|error| map_db_error(error, "updateEntryInvoice", &[]))?;
        rows
    };
    let existing_by_original: std::collections::HashMap<i64, EntryInvoiceLineRow> = current_lines
        .iter()
        .cloned()
        .map(|row| (row.product_id_original, row))
        .collect();

    let mut seen_original_ids = std::collections::HashSet::new();
    let mut seen_product_ids = std::collections::HashSet::new();
    let mut lines = Vec::with_capacity(input.lines.len());
    let mut invoice_total_cents = 0_i64;
    for line in &input.lines {
        if let Some(product_id) = line.product_id {
            if !seen_product_ids.insert(product_id) {
                return Err(validation_failed(
                    "El producto ya está en la factura",
                    "duplicate product in updated entry invoice",
                ));
            }
        }
        if let Some(original_id) = line.product_id_original {
            if !seen_original_ids.insert(original_id)
                || !existing_by_original.contains_key(&original_id)
            {
                return Err(validation_failed(
                    "Las líneas de la factura no son válidas",
                    "updated entry invoice line reference is invalid",
                ));
            }
        } else if line
            .product_id
            .is_some_and(|product_id| existing_by_original.contains_key(&product_id))
        {
            return Err(validation_failed(
                "Las líneas existentes deben conservar su identificador original",
                "existing entry invoice line omitted productIdOriginal",
            ));
        }
        let quantity_milli = parse_entry_update_quantity(&line.quantity).map_err(|error| {
            AppError::new(
                "validationFailed",
                "La cantidad de la línea no es válida",
                &format!("invalid updated entry quantity: {}", error.message),
            )
        })?;
        let unit_price_cents = parse_cost_cents(&line.unit_price).map_err(|error| {
            AppError::new(
                "validationFailed",
                "El precio de la línea no es válido",
                &format!("invalid updated entry price: {}", error.message),
            )
        })?;
        let subtotal_cents = crate::inventory::round_half_up(
            i128::from(quantity_milli) * i128::from(unit_price_cents),
            1_000,
        ) as i64;
        invoice_total_cents = invoice_total_cents
            .checked_add(subtotal_cents)
            .ok_or_else(|| {
                validation_failed(
                    "El total de la factura supera el rango permitido",
                    "updated entry invoice total overflow",
                )
            })?;
        if invoice_total_cents > 99_999_999_999 {
            return Err(validation_failed(
                "El total de la factura no puede superar 999.999.999,99",
                "updated entry invoice total exceeds legal bound",
            ));
        }
        lines.push((line, quantity_milli, unit_price_cents, subtotal_cents));
    }

    let mut occupied_positions: std::collections::BTreeSet<i64> =
        current_lines.iter().map(|row| row.position).collect();
    for row in &current_lines {
        if !seen_original_ids.contains(&row.product_id_original) {
            let consumed_milli = entry_line_consumed_milli(row);
            if consumed_milli > 0 {
                return Err(entry_invoice_invalid_state(
                    &format!(
                        "No se puede quitar «{}»: ya se usaron unidades de esta compra",
                        row.product_name
                    ),
                    "cannot remove consumed entry invoice line",
                ));
            }
            remove_entry_invoice_line(
                connection,
                &actor,
                input.invoice_id,
                &invoice_number,
                row,
                now,
            )?;
            occupied_positions.remove(&row.position);
        }
    }

    for (line, quantity_milli, unit_price_cents, subtotal_cents) in lines {
        let original = line
            .product_id_original
            .and_then(|original_id| existing_by_original.get(&original_id));
        let mut position_override = None;
        let mut existing = original;

        if let Some(row) = original {
            let consumed_milli = entry_line_consumed_milli(row);
            if row.product_id.is_none() {
                if line.product_id.is_some() {
                    return Err(entry_invoice_invalid_state(
                        "Un producto eliminado no se puede reemplazar al editar la factura",
                        "deleted entry product was replaced",
                    ));
                }
                if quantity_milli == 0 {
                    connection
                        .execute("DELETE FROM invoiceLines WHERE id = ?1", [row.id])
                        .map_err(|error| map_db_error(error, "updateEntryInvoice", &[]))?;
                    occupied_positions.remove(&row.position);
                    continue;
                }
                if quantity_milli != row.quantity_milli || unit_price_cents != row.unit_price_cents
                {
                    return Err(entry_invoice_invalid_state(
                        &format!(
                            "El producto eliminado «{}» solo se puede quitar o reducir a 0",
                            row.product_name
                        ),
                        "deleted entry product line was modified",
                    ));
                }
                continue;
            }

            let existing_product_id = row.product_id.unwrap_or_default();
            let existing_status: String = connection
                .query_row(
                    "SELECT status FROM products WHERE id = ?1",
                    [existing_product_id],
                    |db_row| db_row.get(0),
                )
                .optional()
                .map_err(|error| map_db_error(error, "updateEntryInvoice", &[]))?
                .ok_or_else(|| {
                    AppError::new(
                        "notFound",
                        "El producto ya no existe",
                        "entry invoice product disappeared during update",
                    )
                })?;
            let existing_quantity_delta = quantity_milli - row.quantity_milli;
            let existing_cost_changed = unit_price_cents != row.unit_price_cents;
            if existing_status == "discontinued"
                && (line.product_id != row.product_id
                    || existing_cost_changed
                    || existing_quantity_delta > 0)
            {
                return Err(entry_invoice_invalid_state(
                    &format!(
                        "El producto «{}» está descontinuado; solo se puede reducir la cantidad",
                        row.product_name
                    ),
                    "discontinued entry product was changed beyond reduction",
                ));
            }

            if line.product_id != row.product_id {
                if consumed_milli > 0 {
                    return Err(entry_invoice_invalid_state(
                        &format!(
                            "No se puede cambiar el producto «{}»: ya se usaron unidades de esta compra",
                            row.product_name
                        ),
                        "cannot change product of consumed entry invoice line",
                    ));
                }
                remove_entry_invoice_line(
                    connection,
                    &actor,
                    input.invoice_id,
                    &invoice_number,
                    row,
                    now,
                )?;
                occupied_positions.remove(&row.position);
                position_override = Some(row.position);
                existing = None;
            } else if quantity_milli == 0 && consumed_milli == 0 {
                remove_entry_invoice_line(
                    connection,
                    &actor,
                    input.invoice_id,
                    &invoice_number,
                    row,
                    now,
                )?;
                occupied_positions.remove(&row.position);
                continue;
            } else if quantity_milli < consumed_milli {
                return Err(entry_invoice_invalid_state(
                    &format!(
                        "No se puede reducir «{}» por debajo de las {} unidades ya usadas",
                        row.product_name,
                        format_quantity_milli(consumed_milli)
                    ),
                    "updated entry quantity below consumed quantity",
                ));
            }
        }

        if let Some(row) = existing {
            let consumed_milli = entry_line_consumed_milli(row);
            let product_id = row.product_id.unwrap_or_default();
            let (product_status, product_name): (String, String) = connection
                .query_row(
                    "SELECT status, name FROM products WHERE id = ?1",
                    [product_id],
                    |db_row| Ok((db_row.get(0)?, db_row.get(1)?)),
                )
                .optional()
                .map_err(|error| map_db_error(error, "updateEntryInvoice", &[]))?
                .ok_or_else(|| {
                    AppError::new(
                        "notFound",
                        "El producto ya no existe",
                        "entry invoice product disappeared during update",
                    )
                })?;
            let delta_milli = quantity_milli - row.quantity_milli;
            let cost_changed = unit_price_cents != row.unit_price_cents;
            if product_status == "discontinued" && (delta_milli > 0 || cost_changed) {
                return Err(entry_invoice_invalid_state(
                    &format!(
                        "El producto «{product_name}» está descontinuado; solo se puede reducir la cantidad"
                    ),
                    "discontinued entry product was increased or repriced",
                ));
            }
            if consumed_milli > 0 && cost_changed {
                let sale_number = row
                    .lot_id
                    .map(|lot_id| consuming_invoice_number(connection, lot_id))
                    .transpose()?
                    .flatten()
                    .unwrap_or_else(|| "desconocida".to_owned());
                return Err(entry_invoice_invalid_state(
                    &format!(
                        "No se puede cambiar el costo de «{}»: ya se usaron unidades de esta compra en la factura {sale_number}",
                        row.product_name
                    ),
                    "cannot change cost of consumed entry invoice line",
                ));
            }
            if delta_milli > 0 {
                validate_stock_limit(connection, product_id, delta_milli)?;
            }
            let lot_id = row.lot_id.ok_or_else(|| {
                AppError::new(
                    "internal",
                    "Ocurrió un error inesperado",
                    "entry invoice line is missing its inventory lot",
                )
            })?;
            let lot_initial = row.lot_initial_milli.unwrap_or(row.quantity_milli);
            let lot_remaining = row
                .lot_remaining_milli
                .unwrap_or(row.quantity_milli - consumed_milli);
            let timestamp = format_utc_timestamp(now);
            connection
                .execute(
                    "UPDATE lots SET initialMilli = ?1, remainingMilli = ?2, unitCostCents = ?3, updatedAt = ?4 WHERE id = ?5",
                    params![
                        lot_initial + delta_milli,
                        lot_remaining + delta_milli,
                        unit_price_cents,
                        timestamp,
                        lot_id,
                    ],
                )
                .map_err(|error| map_db_error(error, "updateEntryInvoice", &[]))?;
            connection
                .execute(
                    "UPDATE invoiceLines SET quantityMilli = ?1, unitPriceCents = ?2, subtotalCents = ?3,
                        updatedAt = ?4 WHERE id = ?5",
                    params![
                        quantity_milli,
                        unit_price_cents,
                        subtotal_cents,
                        timestamp,
                        row.id,
                    ],
                )
                .map_err(|error| map_db_error(error, "updateEntryInvoice", &[]))?;
            recalculate_product_quantity(connection, product_id)?;
            recalculate_product_cost(connection, product_id)?;
            if delta_milli != 0 {
                insert_entry_update_movement(
                    connection,
                    &actor,
                    EntryUpdateMovement {
                        invoice_id: input.invoice_id,
                        invoice_number: &invoice_number,
                        product_id,
                        product_id_original: row.product_id_original,
                        product_name: &row.product_name,
                        movement_type: "adjustment",
                        quantity_milli: delta_milli,
                        unit_price_cents,
                    },
                    now,
                )?;
            }
        } else {
            let product_id = line.product_id.ok_or_else(|| {
                validation_failed(
                    "Cada línea nueva debe indicar un producto",
                    "new entry invoice line has no product",
                )
            })?;
            if quantity_milli == 0 {
                return Err(validation_failed(
                    "La cantidad de una línea nueva debe ser mayor que cero",
                    "new entry invoice line quantity is zero",
                ));
            }
            let product: Option<EntryUpdateProductRow> = connection
                .query_row(
                    "SELECT p.name, p.unitId, u.name, u.abbreviation, p.status
                         FROM products p LEFT JOIN units u ON u.id = p.unitId WHERE p.id = ?1",
                    [product_id],
                    |db_row| {
                        Ok((
                            db_row.get(0)?,
                            db_row.get(1)?,
                            db_row.get(2)?,
                            db_row.get(3)?,
                            db_row.get(4)?,
                        ))
                    },
                )
                .optional()
                .map_err(|error| map_db_error(error, "updateEntryInvoice", &[]))?;
            let Some((product_name, unit_id, unit_name, unit_abbreviation, status)) = product
            else {
                return Err(AppError::new(
                    "notFound",
                    "El producto ya no existe",
                    "new entry invoice line product missing",
                ));
            };
            if status == "discontinued" {
                return Err(validation_failed(
                    &format!(
                        "El producto «{product_name}» está descontinuado. Actívalo para usarlo en facturas"
                    ),
                    "discontinued product added to entry invoice",
                ));
            }
            validate_stock_limit(connection, product_id, quantity_milli)?;
            let position = if let Some(position) = position_override {
                position
            } else {
                (1..=100)
                    .find(|position| !occupied_positions.contains(position))
                    .ok_or_else(|| {
                        validation_failed(
                            "La factura no puede tener más de 100 líneas",
                            "no free entry invoice line position",
                        )
                    })?
            };
            let timestamp = format_utc_timestamp(now);
            connection
                .execute(
                    "INSERT INTO invoiceLines (invoiceId, position, productId, productIdOriginal,
                        productName, productNameNormalized, unitId, unitName, unitAbbreviation,
                        quantityMilli, unitPriceCents, subtotalCents, createdAt, updatedAt)
                     VALUES (?1, ?2, ?3, ?3, ?4, ?5, ?6, ?7, ?8, ?9, ?10, ?11, ?12, ?12)",
                    params![
                        input.invoice_id,
                        position,
                        product_id,
                        product_name,
                        normalize_text(&product_name),
                        unit_id,
                        unit_name,
                        unit_abbreviation,
                        quantity_milli,
                        unit_price_cents,
                        subtotal_cents,
                        timestamp,
                    ],
                )
                .map_err(|error| map_db_error(error, "updateEntryInvoice", &[]))?;
            let line_id = connection.last_insert_rowid();
            connection
                .execute(
                    "INSERT INTO lots (productId, origin, invoiceLineId, arrivalOrder, initialMilli,
                        remainingMilli, unitCostCents, createdAt, updatedAt)
                     VALUES (?1, 'entry', ?2, ?3, ?4, ?4, ?5, ?6, ?6)",
                    params![
                        product_id,
                        line_id,
                        next_arrival_order(connection, product_id)?,
                        quantity_milli,
                        unit_price_cents,
                        timestamp,
                    ],
                )
                .map_err(|error| map_db_error(error, "updateEntryInvoice", &[]))?;
            recalculate_product_quantity(connection, product_id)?;
            recalculate_product_cost(connection, product_id)?;
            insert_entry_update_movement(
                connection,
                &actor,
                EntryUpdateMovement {
                    invoice_id: input.invoice_id,
                    invoice_number: &invoice_number,
                    product_id,
                    product_id_original: product_id,
                    product_name: &product_name,
                    movement_type: "entry",
                    quantity_milli,
                    unit_price_cents,
                },
                now,
            )?;
            occupied_positions.insert(position);
        }
    }

    let timestamp = format_utc_timestamp(now);
    let full_name = format!("{supplier_first_name} {supplier_last_name}");
    connection
        .execute(
            "UPDATE invoices SET supplierId = ?1, invoiceDate = ?2, notes = ?3,
                contactFullName = ?4, contactFullNameNormalized = ?5, contactNationalId = ?6,
                contactPhone = ?7, updatedAt = ?8 WHERE id = ?9 AND type = 'E'",
            params![
                input.supplier_id,
                input.invoice_date,
                notes,
                full_name,
                normalize_text(&format!("{supplier_first_name} {supplier_last_name}")),
                supplier_national_id,
                supplier_phone,
                timestamp,
                input.invoice_id,
            ],
        )
        .map_err(|error| map_db_error(error, "updateEntryInvoice", &[]))?;

    // NT-02/NT-03: avisos de situación con el estado final (best-effort).
    crate::automation::evaluate_watches_best_effort(&transaction, now);
    transaction
        .commit()
        .map_err(|error| map_db_error(error, "updateEntryInvoice", &[]))?;
    Ok(InvoiceActionResult {
        invoice_id: input.invoice_id,
        number: invoice_number,
        expired_cleanup_warning,
    })
}

/// RS-02: cancela las reservas cuya fecha límite ya pasó (en hora local),
/// exactamente como una cancelación manual pero firmada por "Sistema":
/// los consumos vuelven a sus lotes con su Reversión y se genera N-06.
/// Excepción documentada a `spec-03` §3.2. Devuelve cuántas canceló.
pub fn expire_reservations(connection: &Connection, now: DateTime<Utc>) -> Result<usize, AppError> {
    let today = today_local(now).format("%Y-%m-%d").to_string();
    let expired: Vec<(i64, String, String, String)> = connection
        .prepare(
            "SELECT id, number, type, COALESCE(contactFullName, '') FROM invoices
             WHERE status = 'pending' AND reservedUntil IS NOT NULL AND reservedUntil < ?1
             ORDER BY reservedUntil, id",
        )
        .and_then(|mut statement| {
            statement
                .query_map([&today], |row| {
                    Ok((row.get(0)?, row.get(1)?, row.get(2)?, row.get(3)?))
                })?
                .collect()
        })
        .map_err(|error| map_db_error(error, "expireReservations", &[]))?;

    let timestamp = format_utc_timestamp(now);
    for (invoice_id, number, invoice_type, contact) in &expired {
        connection
            .execute(
                "UPDATE invoices SET status = 'cancelled', updatedAt = ?1 WHERE id = ?2",
                params![timestamp, invoice_id],
            )
            .map_err(|error| map_db_error(error, "expireReservations", &[]))?;
        reverse_outgoing_invoice_consumptions(
            connection,
            &Author::System,
            *invoice_id,
            number,
            invoice_type,
            now,
        )?;
        crate::db::notifications::notify(
            connection,
            "N-06",
            crate::db::notifications::INFO,
            &format!(
                "Factura de reserva cancelada: Factura # {number} - Cliente: {contact} - Motivo: Tiempo límite excedido"
            ),
            Some(crate::db::notifications::Entity::Invoice(*invoice_id)),
            crate::db::notifications::Audience::All,
            now,
        )
        .map_err(|error| map_db_error(error, "expireReservations", &[]))?;
    }
    Ok(expired.len())
}

pub fn change_invoice_status_inner(
    connection: &mut Connection,
    now: DateTime<Utc>,
    invoice_id: i64,
    status: &str,
    payment_method: Option<&str>,
) -> Result<InvoiceActionResult, AppError> {
    let actor = require_session(connection, now)?;
    let transaction = connection
        .transaction()
        .map_err(|error| map_db_error(error, "changeInvoiceStatus", &[]))?;
    let expired_cleanup_warning = purge_best_effort(&transaction, now)?;

    let row: Option<InvoiceStatusRow> = transaction
        .query_row(
            "SELECT type, status, paymentMethod, paidAt, number FROM invoices WHERE id = ?1",
            [invoice_id],
            |row| {
                Ok((
                    row.get(0)?,
                    row.get(1)?,
                    row.get(2)?,
                    row.get(3)?,
                    row.get(4)?,
                ))
            },
        )
        .optional()
        .map_err(|error| map_db_error(error, "changeInvoiceStatus", &[]))?;
    let Some((
        invoice_type_value,
        current_status_value,
        current_payment_method,
        current_paid_at,
        number,
    )) = row
    else {
        return Err(AppError::new(
            "notFound",
            "La factura ya no existe",
            "invoice missing during status update",
        ));
    };

    let invoice_type = InvoiceType::from_str(&invoice_type_value)?;
    let Some(current_status_value) = current_status_value else {
        return Err(AppError::new(
            "invalidState",
            "Las facturas de pérdida no tienen estado",
            "status change attempted on loss invoice",
        ));
    };
    let current_status = InvoiceStatus::from_str(&current_status_value)?;
    let next_status = InvoiceStatus::from_str(status)?;
    validate_transition(invoice_type, current_status, next_status)?;

    let normalized_payment = payment_method
        .map(str::trim)
        .filter(|value| !value.is_empty())
        .map(str::to_owned);
    let supports_payment = matches!(
        invoice_type,
        InvoiceType::Entry | InvoiceType::Sale | InvoiceType::Service
    );

    let next_paid_at = if next_status == InvoiceStatus::Completed {
        if supports_payment {
            Some(format_utc_timestamp(now))
        } else {
            None
        }
    } else if next_status == InvoiceStatus::Pending {
        None
    } else {
        current_paid_at.clone()
    };
    let next_payment_method = match next_status {
        InvoiceStatus::Pending => None,
        InvoiceStatus::Completed if supports_payment => {
            normalized_payment.or_else(|| current_payment_method.clone())
        }
        _ => current_payment_method.clone(),
    };

    validate_status_payload(
        invoice_type,
        next_status,
        next_payment_method.as_deref(),
        next_paid_at.as_deref(),
    )?;

    transaction.execute(
        "UPDATE invoices SET status = ?1, paymentMethod = ?2, paidAt = ?3, updatedAt = ?4 WHERE id = ?5",
        params![
            next_status.as_str(),
            next_payment_method.as_deref(),
            next_paid_at.as_deref(),
            &format_utc_timestamp(now),
            invoice_id,
        ],
    ).map_err(|error| map_db_error(error, "changeInvoiceStatus", &[]))?;

    if next_status == InvoiceStatus::Cancelled && current_status != InvoiceStatus::Cancelled {
        match invoice_type {
            InvoiceType::Entry => {
                reverse_lots_of_invoice(&transaction, invoice_id, now)?;
            }
            InvoiceType::Sale
            | InvoiceType::Service
            | InvoiceType::Guarantee
            | InvoiceType::Installation => {
                reverse_outgoing_invoice_consumptions(
                    &transaction,
                    &Author::User(&actor),
                    invoice_id,
                    &number,
                    &invoice_type_value,
                    now,
                )?;
            }
            InvoiceType::Loss => {
                reverse_outgoing_invoice_consumptions(
                    &transaction,
                    &Author::User(&actor),
                    invoice_id,
                    &number,
                    &invoice_type_value,
                    now,
                )?;
            }
        }
    }

    // NT-02/NT-03: avisos de situación con el estado final (best-effort).
    crate::automation::evaluate_watches_best_effort(&transaction, now);
    transaction
        .commit()
        .map_err(|error| map_db_error(error, "changeInvoiceStatus", &[]))?;

    Ok(InvoiceActionResult {
        invoice_id,
        number,
        expired_cleanup_warning,
    })
}

#[tauri::command]
pub fn save_invoice_attachment_inner(
    connection: &mut Connection,
    now: DateTime<Utc>,
    invoice_id: i64,
    source_path: Option<&Path>,
    app_data_dir: &Path,
) -> Result<(), AppError> {
    require_session(connection, now)?;
    let header: Option<(String, Option<String>)> = connection
        .query_row(
            "SELECT status, attachmentFileName FROM invoices WHERE id = ?1 AND type = 'E'",
            [invoice_id],
            |row| Ok((row.get(0)?, row.get(1)?)),
        )
        .optional()
        .map_err(|error| map_db_error(error, "saveInvoiceAttachment", &[]))?;
    let Some((status, old_file_name)) = header else {
        return Err(AppError::new(
            "notFound",
            "La factura ya no existe",
            "entry invoice missing while saving attachment",
        ));
    };
    if status != "pending" {
        return Err(AppError::new(
            "invalidState",
            "Solo se pueden editar las facturas que están Por Pagar",
            "attachment can only be changed on a pending entry invoice",
        ));
    }

    let stored = if let Some(source_path) = source_path {
        if !source_path.exists() {
            return Err(AppError::new(
                "notFound",
                "El archivo adjunto no se encuentra en el equipo",
                "attachment source file is missing",
            ));
        }
        let mut attachment = store_attachment(app_data_dir, source_path)?;
        attachment.uploaded_at = format_utc_timestamp(now);
        Some(attachment)
    } else {
        None
    };

    let transaction = connection
        .transaction()
        .map_err(|error| map_db_error(error, "saveInvoiceAttachment", &[]))?;
    let changed = transaction
        .execute(
            "UPDATE invoices SET attachmentFileName = ?1, attachmentOriginalName = ?2,
                attachmentSizeBytes = ?3, attachmentUploadedAt = ?4, updatedAt = ?5
             WHERE id = ?6 AND type = 'E' AND status = 'pending'",
            params![
                stored
                    .as_ref()
                    .map(|attachment| attachment.file_name.as_str()),
                stored
                    .as_ref()
                    .map(|attachment| attachment.original_name.as_str()),
                stored
                    .as_ref()
                    .map(|attachment| attachment.size_bytes as i64),
                stored
                    .as_ref()
                    .map(|attachment| attachment.uploaded_at.as_str()),
                format_utc_timestamp(now),
                invoice_id,
            ],
        )
        .map_err(|error| map_db_error(error, "saveInvoiceAttachment", &[]))?;
    if changed != 1 {
        return Err(AppError::new(
            "invalidState",
            "Solo se pueden editar las facturas que están Por Pagar",
            "entry invoice was no longer pending while saving attachment",
        ));
    }
    transaction
        .commit()
        .map_err(|error| map_db_error(error, "saveInvoiceAttachment", &[]))?;

    let new_file_name = stored
        .as_ref()
        .map(|attachment| attachment.file_name.as_str());
    if let Some(old_file_name) = old_file_name.as_deref() {
        if Some(old_file_name) != new_file_name {
            delete_attachment(app_data_dir, old_file_name)?;
        }
    }

    Ok(())
}

pub fn download_invoice_attachment_inner(
    connection: &mut Connection,
    now: DateTime<Utc>,
    invoice_id: i64,
    destination_path: &Path,
    app_data_dir: &Path,
) -> Result<(), AppError> {
    require_session(connection, now)?;
    let invoice: Option<(String, Option<String>, Option<i64>)> = connection
        .query_row(
            "SELECT type, attachmentFileName, attachmentSizeBytes FROM invoices WHERE id = ?1",
            [invoice_id],
            |row| Ok((row.get(0)?, row.get(1)?, row.get(2)?)),
        )
        .optional()
        .map_err(|error| map_db_error(error, "downloadInvoiceAttachment", &[]))?;
    let Some((invoice_type, file_name, recorded_size)) = invoice else {
        return Err(AppError::new(
            "notFound",
            "La factura ya no existe",
            "invoice missing while downloading attachment",
        ));
    };
    let Some(file_name) = file_name.filter(|_| invoice_type == "E") else {
        return Err(AppError::new(
            "notFound",
            "La factura no tiene un archivo adjunto",
            "invoice has no stored attachment",
        ));
    };
    let source_path = attachment_path(app_data_dir, &file_name)?;
    let metadata = fs::metadata(&source_path).map_err(|error| {
        if error.kind() == std::io::ErrorKind::NotFound {
            AppError::new(
                "notFound",
                "El archivo adjunto no se encuentra en el equipo. Puede haber sido movido o eliminado",
                "stored attachment file is missing",
            )
        } else {
            AppError::new(
                "internal",
                "No se pudo guardar el archivo en el equipo",
                "failed to read stored attachment metadata",
            )
        }
    })?;
    if recorded_size.is_some_and(|size| size >= 0 && size as u64 != metadata.len()) {
        append_error_log(
            app_data_dir,
            &format!(
                "attachment size mismatch: invoiceId={invoice_id} expected={} actual={}",
                recorded_size.unwrap_or_default(),
                metadata.len()
            ),
        );
    }
    fs::copy(&source_path, destination_path).map_err(|_| {
        AppError::new(
            "internal",
            "No se pudo guardar el archivo en el equipo",
            "failed to copy stored attachment to destination",
        )
    })?;
    Ok(())
}

#[tauri::command]
pub fn save_invoice_attachment(
    state: tauri::State<'_, std::sync::Mutex<Connection>>,
    invoice_id: i64,
    source_path: Option<String>,
) -> Result<(), AppError> {
    let mut connection = crate::db::connection::lock_conn(&state);
    let now = Utc::now();
    let result = match current_app_data_dir() {
        Some(app_data_dir) => save_invoice_attachment_inner(
            &mut connection,
            now,
            invoice_id,
            source_path.as_deref().map(Path::new),
            app_data_dir,
        ),
        None => Err(AppError::new(
            "internal",
            "Ocurrió un error inesperado",
            "application data directory is unavailable",
        )),
    };
    if let Err(error) = &result {
        if error.should_log() {
            if let Some(app_data_dir) = current_app_data_dir() {
                log_error(
                    &connection,
                    "invoices",
                    "saveInvoiceAttachment",
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
pub fn download_invoice_attachment(
    state: tauri::State<'_, std::sync::Mutex<Connection>>,
    invoice_id: i64,
    destination_path: String,
) -> Result<(), AppError> {
    let mut connection = crate::db::connection::lock_conn(&state);
    let now = Utc::now();
    let result = match current_app_data_dir() {
        Some(app_data_dir) => download_invoice_attachment_inner(
            &mut connection,
            now,
            invoice_id,
            Path::new(&destination_path),
            app_data_dir,
        ),
        None => Err(AppError::new(
            "internal",
            "Ocurrió un error inesperado",
            "application data directory is unavailable",
        )),
    };
    if let Err(error) = &result {
        if error.should_log() {
            if let Some(app_data_dir) = current_app_data_dir() {
                log_error(
                    &connection,
                    "invoices",
                    "downloadInvoiceAttachment",
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
pub fn create_entry_invoice(
    state: tauri::State<'_, std::sync::Mutex<Connection>>,
    input: CreateEntryInvoiceInput,
) -> Result<InvoiceActionResult, AppError> {
    let mut connection = crate::db::connection::lock_conn(&state);
    let now = Utc::now();
    let result = create_entry_invoice_inner(&mut connection, now, input);
    if let Err(error) = &result {
        if error.should_log() {
            if let Some(app_data_dir) = current_app_data_dir() {
                log_error(
                    &connection,
                    "invoices",
                    "createEntryInvoice",
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
pub fn create_sale_invoice(
    state: tauri::State<'_, std::sync::Mutex<Connection>>,
    input: CreateSaleInvoiceInput,
) -> Result<InvoiceActionResult, AppError> {
    let mut connection = crate::db::connection::lock_conn(&state);
    let now = Utc::now();
    let result = create_sale_invoice_inner(&mut connection, now, input);
    if let Err(error) = &result {
        if error.should_log() {
            if let Some(app_data_dir) = current_app_data_dir() {
                log_error(
                    &connection,
                    "invoices",
                    "createSaleInvoice",
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
pub fn create_service_invoice(
    state: tauri::State<'_, std::sync::Mutex<Connection>>,
    input: CreateServiceInvoiceInput,
) -> Result<InvoiceActionResult, AppError> {
    let mut connection = crate::db::connection::lock_conn(&state);
    let now = Utc::now();
    let result = create_service_invoice_inner(&mut connection, now, input);
    if let Err(error) = &result {
        if error.should_log() {
            if let Some(app_data_dir) = current_app_data_dir() {
                log_error(
                    &connection,
                    "invoices",
                    "createServiceInvoice",
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
pub fn create_guarantee_invoice(
    state: tauri::State<'_, std::sync::Mutex<Connection>>,
    input: CreateGuaranteeInvoiceInput,
) -> Result<InvoiceActionResult, AppError> {
    let mut connection = crate::db::connection::lock_conn(&state);
    let now = Utc::now();
    let result = create_guarantee_invoice_inner(&mut connection, now, input);
    if let Err(error) = &result {
        if error.should_log() {
            if let Some(app_data_dir) = current_app_data_dir() {
                log_error(
                    &connection,
                    "invoices",
                    "createGuaranteeInvoice",
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
pub fn create_installation_invoice(
    state: tauri::State<'_, std::sync::Mutex<Connection>>,
    input: CreateInstallationInvoiceInput,
) -> Result<InvoiceActionResult, AppError> {
    let mut connection = crate::db::connection::lock_conn(&state);
    let now = Utc::now();
    let result = create_installation_invoice_inner(&mut connection, now, input);
    if let Err(error) = &result {
        if error.should_log() {
            if let Some(app_data_dir) = current_app_data_dir() {
                log_error(
                    &connection,
                    "invoices",
                    "createInstallationInvoice",
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
pub fn create_loss_invoice(
    state: tauri::State<'_, std::sync::Mutex<Connection>>,
    input: CreateLossInvoiceInput,
) -> Result<InvoiceActionResult, AppError> {
    let mut connection = crate::db::connection::lock_conn(&state);
    let now = Utc::now();
    let result = create_loss_invoice_inner(&mut connection, now, input);
    if let Err(error) = &result {
        if error.should_log() {
            if let Some(app_data_dir) = current_app_data_dir() {
                log_error(
                    &connection,
                    "invoices",
                    "createLossInvoice",
                    error,
                    now,
                    app_data_dir,
                );
            }
        }
    }
    result
}

// Faltaba el comando: el núcleo existía, pero el frontend no lo podía invocar.
#[tauri::command]
pub fn update_entry_invoice(
    state: tauri::State<'_, std::sync::Mutex<Connection>>,
    input: UpdateEntryInvoiceInput,
) -> Result<InvoiceActionResult, AppError> {
    let mut connection = crate::db::connection::lock_conn(&state);
    let now = Utc::now();
    let result = update_entry_invoice_inner(&mut connection, now, input);
    if let Err(error) = &result {
        if error.should_log() {
            if let Some(app_data_dir) = current_app_data_dir() {
                log_error(
                    &connection,
                    "invoices",
                    "updateEntryInvoice",
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
pub fn update_sale_invoice(
    state: tauri::State<'_, std::sync::Mutex<Connection>>,
    input: UpdateSaleInvoiceInput,
) -> Result<InvoiceActionResult, AppError> {
    let mut connection = crate::db::connection::lock_conn(&state);
    let now = Utc::now();
    let result = update_sale_invoice_inner(&mut connection, now, input);
    if let Err(error) = &result {
        if error.should_log() {
            if let Some(app_data_dir) = current_app_data_dir() {
                log_error(
                    &connection,
                    "invoices",
                    "updateSaleInvoice",
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
pub fn update_service_invoice(
    state: tauri::State<'_, std::sync::Mutex<Connection>>,
    input: UpdateServiceInvoiceInput,
) -> Result<InvoiceActionResult, AppError> {
    let mut connection = crate::db::connection::lock_conn(&state);
    let now = Utc::now();
    let result = update_service_invoice_inner(&mut connection, now, input);
    if let Err(error) = &result {
        if error.should_log() {
            if let Some(app_data_dir) = current_app_data_dir() {
                log_error(
                    &connection,
                    "invoices",
                    "updateServiceInvoice",
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
pub fn update_guarantee_invoice(
    state: tauri::State<'_, std::sync::Mutex<Connection>>,
    input: UpdateGuaranteeInvoiceInput,
) -> Result<InvoiceActionResult, AppError> {
    let mut connection = crate::db::connection::lock_conn(&state);
    let now = Utc::now();
    let result = update_guarantee_invoice_inner(&mut connection, now, input);
    if let Err(error) = &result {
        if error.should_log() {
            if let Some(app_data_dir) = current_app_data_dir() {
                log_error(
                    &connection,
                    "invoices",
                    "updateGuaranteeInvoice",
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
pub fn update_installation_invoice(
    state: tauri::State<'_, std::sync::Mutex<Connection>>,
    input: UpdateInstallationInvoiceInput,
) -> Result<InvoiceActionResult, AppError> {
    let mut connection = crate::db::connection::lock_conn(&state);
    let now = Utc::now();
    let result = update_installation_invoice_inner(&mut connection, now, input);
    if let Err(error) = &result {
        if error.should_log() {
            if let Some(app_data_dir) = current_app_data_dir() {
                log_error(
                    &connection,
                    "invoices",
                    "updateInstallationInvoice",
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
pub fn change_invoice_status(
    state: tauri::State<'_, std::sync::Mutex<Connection>>,
    invoice_id: i64,
    status: String,
    payment_method: Option<String>,
) -> Result<InvoiceActionResult, AppError> {
    let mut connection = crate::db::connection::lock_conn(&state);
    let now = Utc::now();
    let result = change_invoice_status_inner(
        &mut connection,
        now,
        invoice_id,
        &status,
        payment_method.as_deref(),
    );
    if let Err(error) = &result {
        if error.should_log() {
            if let Some(app_data_dir) = current_app_data_dir() {
                log_error(
                    &connection,
                    "invoices",
                    "changeInvoiceStatus",
                    error,
                    now,
                    app_data_dir,
                );
            }
        }
    }
    result
}

pub fn delete_invoice_inner(
    connection: &mut Connection,
    now: DateTime<Utc>,
    invoice_id: i64,
) -> Result<InvoiceActionResult, AppError> {
    delete_invoice_inner_with_app_data_dir(connection, now, invoice_id, current_app_data_dir())
}

fn delete_invoice_inner_with_app_data_dir(
    connection: &mut Connection,
    now: DateTime<Utc>,
    invoice_id: i64,
    app_data_dir: Option<&Path>,
) -> Result<InvoiceActionResult, AppError> {
    let actor = require_session(connection, now)?;
    let transaction = connection
        .transaction()
        .map_err(|error| map_db_error(error, "deleteInvoice", &[]))?;
    let expired_cleanup_warning = purge_best_effort(&transaction, now)?;

    let row: Option<(String, Option<String>, String, Option<String>)> = transaction
        .query_row(
            "SELECT type, status, number, attachmentFileName FROM invoices WHERE id = ?1",
            [invoice_id],
            |row| Ok((row.get(0)?, row.get(1)?, row.get(2)?, row.get(3)?)),
        )
        .optional()
        .map_err(|error| map_db_error(error, "deleteInvoice", &[]))?;
    let Some((invoice_type_value, current_status_value, number, attachment_file_name)) = row else {
        return Err(AppError::new(
            "notFound",
            "La factura ya no existe",
            "invoice missing during deletion",
        ));
    };

    let invoice_type = InvoiceType::from_str(&invoice_type_value)?;
    let current_status = current_status_value
        .as_deref()
        .map(InvoiceStatus::from_str)
        .transpose()?;

    let has_invoice_remainder: bool = transaction
        .query_row(
            "SELECT EXISTS(SELECT 1 FROM lots l JOIN invoiceLines il ON il.id = l.invoiceLineId WHERE il.invoiceId = ?1 AND l.remainingMilli > 0)",
            [invoice_id],
            |row| row.get(0),
        )
        .map_err(|error| map_db_error(error, "deleteInvoice", &[]))?;
    let has_missing_products: bool = transaction
        .query_row(
            "SELECT EXISTS(SELECT 1 FROM invoiceLines WHERE invoiceId = ?1 AND productId IS NULL)",
            [invoice_id],
            |row| row.get(0),
        )
        .map_err(|error| map_db_error(error, "deleteInvoice", &[]))?;

    match invoice_type {
        InvoiceType::Entry => {
            if current_status != Some(InvoiceStatus::Cancelled) || has_invoice_remainder {
                reverse_lots_of_invoice(&transaction, invoice_id, now)?;
            }
        }
        InvoiceType::Sale
        | InvoiceType::Service
        | InvoiceType::Guarantee
        | InvoiceType::Installation => {
            if current_status != Some(InvoiceStatus::Cancelled) {
                reverse_outgoing_invoice_consumptions(
                    &transaction,
                    &Author::User(&actor),
                    invoice_id,
                    &number,
                    &invoice_type_value,
                    now,
                )?;
            }
        }
        InvoiceType::Loss => {
            reverse_outgoing_invoice_consumptions(
                &transaction,
                &Author::User(&actor),
                invoice_id,
                &number,
                &invoice_type_value,
                now,
            )?;
        }
    }

    let deleted_rows = transaction
        .execute("DELETE FROM invoices WHERE id = ?1", [invoice_id])
        .map_err(|error| map_db_error(error, "deleteInvoice", &[]))?;
    if deleted_rows != 1 {
        return Err(AppError::new(
            "notFound",
            "La factura ya no existe",
            "invoice missing during deletion commit",
        ));
    }

    // NT-02/NT-03: avisos de situación con el estado final (best-effort).
    crate::automation::evaluate_watches_best_effort(&transaction, now);
    transaction
        .commit()
        .map_err(|error| map_db_error(error, "deleteInvoice", &[]))?;

    if let (Some(file_name), Some(app_data_dir)) = (attachment_file_name.as_deref(), app_data_dir) {
        let _ = delete_attachment(app_data_dir, file_name);
    }

    Ok(InvoiceActionResult {
        invoice_id,
        number,
        expired_cleanup_warning: match (
            expired_cleanup_warning,
            has_missing_products.then_some(
                "Algunos productos de la factura ya no existen; no se modificó su inventario.",
            ),
        ) {
            (Some(purge), Some(missing)) => Some(format!("{purge} {missing}")),
            (purge, missing) => purge.or_else(|| missing.map(str::to_owned)),
        },
    })
}

#[tauri::command]
pub fn delete_invoice(
    state: tauri::State<'_, std::sync::Mutex<Connection>>,
    invoice_id: i64,
) -> Result<InvoiceActionResult, AppError> {
    let mut connection = crate::db::connection::lock_conn(&state);
    let now = Utc::now();
    let result = delete_invoice_inner(&mut connection, now, invoice_id);
    if let Err(error) = &result {
        if error.should_log() {
            if let Some(app_data_dir) = current_app_data_dir() {
                log_error(
                    &connection,
                    "invoices",
                    "deleteInvoice",
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
    use chrono::{TimeZone, Utc};
    use std::fs;
    use std::path::PathBuf;
    use std::sync::atomic::{AtomicU64, Ordering};

    use crate::models::invoice::InvoiceFilters;
    use crate::test_utils::{
        assert_allows_both_roles, assert_requires_session, create_test_session, create_test_user,
        insert_product_fixture, setup_test_db,
    };
    use crate::timestamps::format_utc_timestamp;

    use super::{
        change_invoice_status_inner, delete_invoice_inner, delete_invoice_inner_with_app_data_dir,
        get_invoice_inner, list_invoices_inner,
    };

    static NEXT_TEMP_DIR: AtomicU64 = AtomicU64::new(0);

    fn insert_invoice(
        connection: &rusqlite::Connection,
        user_id: i64,
        invoice_type: &str,
        number: &str,
        status: Option<&str>,
        now: chrono::DateTime<Utc>,
    ) -> i64 {
        let timestamp = format_utc_timestamp(now);
        if invoice_type == "P" {
            connection
                .execute(
                    "INSERT INTO invoices (type, number, invoiceDate, status, createdByUserId, createdByName, createdAt, updatedAt)
                     VALUES ('P', ?1, '2026-10-01', NULL, ?2, 'testuser', ?3, ?3)",
                    (number, user_id, timestamp),
                )
                .unwrap();
        } else {
            connection
                .execute(
                    "INSERT INTO invoices (type, number, invoiceDate, status, contactFullName, contactFullNameNormalized,
                        contactNationalId, contactPhone, createdByUserId, createdByName, createdAt, updatedAt)
                     VALUES (?1, ?2, '2026-10-01', ?3, 'Ana Pérez', 'ana perez', '12345678920', '+5355552222', ?4, 'testuser', ?5, ?5)",
                    (invoice_type, number, status, user_id, timestamp),
                )
                .unwrap();
        }
        connection.last_insert_rowid()
    }

    fn insert_invoice_line(
        connection: &rusqlite::Connection,
        invoice_id: i64,
        product_id: Option<i64>,
        product_id_original: i64,
        now: chrono::DateTime<Utc>,
    ) -> i64 {
        let timestamp = format_utc_timestamp(now);
        connection
            .execute(
                "INSERT INTO invoiceLines (invoiceId, position, productId, productIdOriginal, productName,
                    productNameNormalized, quantityMilli, unitPriceCents, subtotalCents, createdAt, updatedAt)
                 VALUES (?1, 1, ?2, ?3, 'Producto de prueba', 'producto de prueba', 10_000, 100, 1_000, ?4, ?4)",
                (invoice_id, product_id, product_id_original, timestamp),
            )
            .unwrap();
        connection.last_insert_rowid()
    }

    fn temp_dir() -> PathBuf {
        let sequence = NEXT_TEMP_DIR.fetch_add(1, Ordering::Relaxed);
        std::env::temp_dir().join(format!(
            "goyma-delete-invoice-{}-{sequence}",
            std::process::id()
        ))
    }

    fn setup_entry_fixture(
        username: &str,
        now: chrono::DateTime<Utc>,
    ) -> (rusqlite::Connection, i64, i64) {
        let connection = setup_test_db();
        let user_id = create_test_user(
            &connection,
            username,
            "contador",
            "+5350000133",
            "12345678925",
            "ValidPass1!",
        );
        create_test_session(&connection, user_id, false, now, now);
        connection
            .execute(
                "INSERT INTO suppliers (firstName, lastName, nationalId, phone, createdAt, updatedAt)
                 VALUES ('Ana', 'Pérez', '12345678926', '+5355559092', ?1, ?1)",
                [format_utc_timestamp(now)],
            )
            .unwrap();
        let supplier_id = connection.last_insert_rowid();
        (connection, user_id, supplier_id)
    }

    fn entry_line(
        product_id: Option<i64>,
        new_product: Option<crate::models::invoice::NewProductInput>,
        quantity: &str,
        unit_price: &str,
    ) -> crate::models::invoice::EntryInvoiceLineInput {
        crate::models::invoice::EntryInvoiceLineInput {
            product_id,
            new_product,
            quantity: quantity.to_owned(),
            unit_price: unit_price.to_owned(),
        }
    }

    fn new_product(name: &str) -> crate::models::invoice::NewProductInput {
        crate::models::invoice::NewProductInput {
            name: name.to_owned(),
            description: None,
            category_id: None,
            unit_id: None,
            cost: "1,00".to_owned(),
            profit_percent: "20,00".to_owned(),
            expires_on: None,
        }
    }

    fn entry_input(
        supplier_id: i64,
        lines: Vec<crate::models::invoice::EntryInvoiceLineInput>,
        status: Option<&str>,
    ) -> crate::models::invoice::CreateEntryInvoiceInput {
        crate::models::invoice::CreateEntryInvoiceInput {
            supplier_id,
            invoice_date: "2026-10-04".to_owned(),
            status: status.map(str::to_owned),
            notes: None,
            payment_method: None,
            attachment_file_name: None,
            attachment_original_name: None,
            attachment_size_bytes: None,
            attachment_uploaded_at: None,
            lines,
        }
    }

    fn update_entry_line(
        product_id_original: Option<i64>,
        product_id: i64,
        quantity: &str,
        unit_price: &str,
    ) -> crate::models::invoice::UpdateEntryInvoiceLineInput {
        crate::models::invoice::UpdateEntryInvoiceLineInput {
            product_id_original,
            product_id: Some(product_id),
            quantity: quantity.to_owned(),
            unit_price: unit_price.to_owned(),
        }
    }

    fn update_deleted_entry_line(
        product_id_original: i64,
        quantity: &str,
        unit_price: &str,
    ) -> crate::models::invoice::UpdateEntryInvoiceLineInput {
        crate::models::invoice::UpdateEntryInvoiceLineInput {
            product_id_original: Some(product_id_original),
            product_id: None,
            quantity: quantity.to_owned(),
            unit_price: unit_price.to_owned(),
        }
    }

    fn update_entry_input(
        invoice_id: i64,
        supplier_id: i64,
        invoice_date: &str,
        lines: Vec<crate::models::invoice::UpdateEntryInvoiceLineInput>,
    ) -> crate::models::invoice::UpdateEntryInvoiceInput {
        crate::models::invoice::UpdateEntryInvoiceInput {
            invoice_id,
            supplier_id,
            invoice_date: invoice_date.to_owned(),
            notes: Some("Observaciones editadas".to_owned()),
            lines,
        }
    }

    fn create_entry_line_fixture(
        connection: &mut rusqlite::Connection,
        now: chrono::DateTime<Utc>,
        supplier_id: i64,
        product_id: i64,
        quantity: &str,
        unit_price: &str,
    ) -> (i64, i64, i64) {
        let result = super::create_entry_invoice_inner(
            connection,
            now,
            entry_input(
                supplier_id,
                vec![entry_line(Some(product_id), None, quantity, unit_price)],
                None,
            ),
        )
        .unwrap();
        let (line_id, lot_id): (i64, i64) = connection
            .query_row(
                "SELECT il.id, l.id FROM invoiceLines il JOIN lots l ON l.invoiceLineId = il.id WHERE il.invoiceId = ?1",
                [result.invoice_id],
                |row| Ok((row.get(0)?, row.get(1)?)),
            )
            .unwrap();
        (result.invoice_id, line_id, lot_id)
    }

    fn create_attachment_entry_invoice(
        connection: &mut rusqlite::Connection,
        now: chrono::DateTime<Utc>,
        supplier_id: i64,
    ) -> i64 {
        let sequence = NEXT_TEMP_DIR.fetch_add(1, Ordering::Relaxed);
        let product_id = insert_product_fixture(
            connection,
            &format!("Producto adjunto {sequence}"),
            None,
            None,
            100,
            0,
            0,
            "active",
            None,
            None,
        );
        create_entry_line_fixture(connection, now, supplier_id, product_id, "1,000", "1,00").0
    }

    #[test]
    fn save_invoice_attachment_stores_pdf_metadata_and_updates_audit_timestamp() {
        let created_at = Utc.with_ymd_and_hms(2026, 10, 4, 12, 0, 0).unwrap();
        let uploaded_at = created_at + chrono::Duration::minutes(3);
        let app_data_dir = temp_dir();
        fs::create_dir_all(&app_data_dir).unwrap();
        let source_path = app_data_dir.join("Factura Octubre.pdf");
        fs::write(&source_path, b"%PDF-1.7 factura valida").unwrap();
        let (mut connection, _, supplier_id) = setup_entry_fixture("saveattachment", created_at);
        let product_id = insert_product_fixture(
            &connection,
            "Producto adjunto",
            None,
            None,
            100,
            0,
            0,
            "active",
            None,
            None,
        );
        let (invoice_id, _, _) = create_entry_line_fixture(
            &mut connection,
            created_at,
            supplier_id,
            product_id,
            "1,000",
            "1,00",
        );

        super::save_invoice_attachment_inner(
            &mut connection,
            uploaded_at,
            invoice_id,
            Some(&source_path),
            &app_data_dir,
        )
        .unwrap();

        let metadata: (
            Option<String>,
            Option<String>,
            Option<i64>,
            Option<String>,
            String,
            String,
        ) = connection
            .query_row(
                "SELECT attachmentFileName, attachmentOriginalName, attachmentSizeBytes,
                        attachmentUploadedAt, createdAt, updatedAt FROM invoices WHERE id = ?1",
                [invoice_id],
                |row| {
                    Ok((
                        row.get(0)?,
                        row.get(1)?,
                        row.get(2)?,
                        row.get(3)?,
                        row.get(4)?,
                        row.get(5)?,
                    ))
                },
            )
            .unwrap();
        let stored_path = app_data_dir
            .join("attachments")
            .join(metadata.0.as_deref().unwrap());
        assert!(stored_path.exists());
        assert_eq!(fs::read(stored_path).unwrap(), b"%PDF-1.7 factura valida");
        assert_eq!(metadata.1.as_deref(), Some("Factura Octubre.pdf"));
        assert_eq!(metadata.2, Some(b"%PDF-1.7 factura valida".len() as i64));
        assert_eq!(
            metadata.3.as_deref(),
            Some(crate::timestamps::format_utc_timestamp(uploaded_at).as_str())
        );
        assert_eq!(
            metadata.4,
            crate::timestamps::format_utc_timestamp(created_at)
        );
        assert_eq!(
            metadata.5,
            crate::timestamps::format_utc_timestamp(uploaded_at)
        );
        fs::remove_dir_all(app_data_dir).unwrap();
    }

    #[test]
    fn save_invoice_attachment_rejects_invalid_pdf_size_and_non_pending_invoices() {
        let now = Utc.with_ymd_and_hms(2026, 10, 4, 12, 0, 0).unwrap();
        let app_data_dir = temp_dir();
        fs::create_dir_all(&app_data_dir).unwrap();
        let (mut connection, _, supplier_id) = setup_entry_fixture("attachmentinvalid", now);
        let invoice_id = create_attachment_entry_invoice(&mut connection, now, supplier_id);
        let text_path = app_data_dir.join("not-pdf.pdf");
        fs::write(&text_path, b"not a pdf").unwrap();
        let invalid = super::save_invoice_attachment_inner(
            &mut connection,
            now,
            invoice_id,
            Some(&text_path),
            &app_data_dir,
        )
        .unwrap_err();
        assert_eq!(invalid.code, "validationFailed");
        assert_eq!(invalid.message, "El archivo debe ser un PDF de hasta 5 MB");

        let large_path = app_data_dir.join("large.pdf");
        fs::write(&large_path, b"%PDF-").unwrap();
        fs::OpenOptions::new()
            .write(true)
            .open(&large_path)
            .unwrap()
            .set_len(5 * 1024 * 1024 + 1)
            .unwrap();
        let too_large = super::save_invoice_attachment_inner(
            &mut connection,
            now,
            invoice_id,
            Some(&large_path),
            &app_data_dir,
        )
        .unwrap_err();
        assert_eq!(too_large.code, "validationFailed");

        let valid_path = app_data_dir.join("valid.pdf");
        fs::write(&valid_path, b"%PDF-1.7 valid").unwrap();
        for status in ["completed", "cancelled"] {
            super::change_invoice_status_inner(&mut connection, now, invoice_id, status, None)
                .unwrap();
            let error = super::save_invoice_attachment_inner(
                &mut connection,
                now,
                invoice_id,
                Some(&valid_path),
                &app_data_dir,
            )
            .unwrap_err();
            assert_eq!(error.code, "invalidState");
            assert_eq!(
                error.message,
                "Solo se pueden editar las facturas que están Por Pagar"
            );
        }
        let attachment_count: i64 = connection
            .query_row(
                "SELECT COUNT(*) FROM invoices WHERE id = ?1 AND attachmentFileName IS NOT NULL",
                [invoice_id],
                |row| row.get(0),
            )
            .unwrap();
        assert_eq!(attachment_count, 0);
        fs::remove_dir_all(app_data_dir).unwrap();
    }

    #[test]
    fn save_invoice_attachment_replaces_or_removes_files_and_sweeps_failed_updates() {
        let now = Utc.with_ymd_and_hms(2026, 10, 4, 12, 0, 0).unwrap();
        let app_data_dir = temp_dir();
        fs::create_dir_all(&app_data_dir).unwrap();
        let old_source = app_data_dir.join("old.pdf");
        let replacement_source = app_data_dir.join("replacement.pdf");
        let failed_source = app_data_dir.join("failed.pdf");
        fs::write(&old_source, b"%PDF-1.7 old").unwrap();
        fs::write(&replacement_source, b"%PDF-1.7 replacement").unwrap();
        fs::write(&failed_source, b"%PDF-1.7 failed").unwrap();
        let (mut connection, _, supplier_id) = setup_entry_fixture("attachmentreplace", now);
        let invoice_id = create_attachment_entry_invoice(&mut connection, now, supplier_id);

        super::save_invoice_attachment_inner(
            &mut connection,
            now,
            invoice_id,
            Some(&old_source),
            &app_data_dir,
        )
        .unwrap();
        let old_file_name: String = connection
            .query_row(
                "SELECT attachmentFileName FROM invoices WHERE id = ?1",
                [invoice_id],
                |row| row.get(0),
            )
            .unwrap();
        let old_file = app_data_dir.join("attachments").join(&old_file_name);
        assert!(old_file.exists());

        super::save_invoice_attachment_inner(
            &mut connection,
            now + chrono::Duration::minutes(1),
            invoice_id,
            Some(&replacement_source),
            &app_data_dir,
        )
        .unwrap();
        assert!(!old_file.exists());
        let replacement_file_name: String = connection
            .query_row(
                "SELECT attachmentFileName FROM invoices WHERE id = ?1",
                [invoice_id],
                |row| row.get(0),
            )
            .unwrap();
        let replacement_file = app_data_dir
            .join("attachments")
            .join(&replacement_file_name);
        assert!(replacement_file.exists());

        super::save_invoice_attachment_inner(
            &mut connection,
            now + chrono::Duration::minutes(2),
            invoice_id,
            None,
            &app_data_dir,
        )
        .unwrap();
        assert!(!replacement_file.exists());
        let cleared: (Option<String>, Option<String>, Option<i64>, Option<String>) = connection
            .query_row(
                "SELECT attachmentFileName, attachmentOriginalName, attachmentSizeBytes, attachmentUploadedAt
                 FROM invoices WHERE id = ?1",
                [invoice_id],
                |row| Ok((row.get(0)?, row.get(1)?, row.get(2)?, row.get(3)?)),
            )
            .unwrap();
        assert_eq!(cleared, (None, None, None, None));

        super::save_invoice_attachment_inner(
            &mut connection,
            now + chrono::Duration::minutes(3),
            invoice_id,
            Some(&old_source),
            &app_data_dir,
        )
        .unwrap();
        let stable_file_name: String = connection
            .query_row(
                "SELECT attachmentFileName FROM invoices WHERE id = ?1",
                [invoice_id],
                |row| row.get(0),
            )
            .unwrap();
        connection
            .execute_batch(
                "CREATE TRIGGER fail_attachment_update BEFORE UPDATE OF attachmentFileName ON invoices
                 BEGIN SELECT RAISE(ABORT, 'forced attachment update failure'); END;",
            )
            .unwrap();
        assert!(super::save_invoice_attachment_inner(
            &mut connection,
            now + chrono::Duration::minutes(4),
            invoice_id,
            Some(&failed_source),
            &app_data_dir,
        )
        .is_err());
        let after_failure: String = connection
            .query_row(
                "SELECT attachmentFileName FROM invoices WHERE id = ?1",
                [invoice_id],
                |row| row.get(0),
            )
            .unwrap();
        assert_eq!(after_failure, stable_file_name);
        assert!(app_data_dir
            .join("attachments")
            .join(&stable_file_name)
            .exists());
        assert_eq!(
            crate::attachments::sweep_orphans(&app_data_dir, &connection).unwrap(),
            1
        );
        let files: Vec<String> = fs::read_dir(app_data_dir.join("attachments"))
            .unwrap()
            .map(|entry| entry.unwrap().file_name().to_string_lossy().into_owned())
            .collect();
        assert_eq!(files, vec![stable_file_name]);
        fs::remove_dir_all(app_data_dir).unwrap();
    }

    #[test]
    fn download_invoice_attachment_copies_corrupt_pdf_and_logs_size_mismatch() {
        let now = Utc.with_ymd_and_hms(2026, 10, 4, 12, 0, 0).unwrap();
        let app_data_dir = temp_dir();
        let destination_dir = temp_dir();
        fs::create_dir_all(app_data_dir.join("attachments")).unwrap();
        fs::create_dir_all(&destination_dir).unwrap();
        let (mut connection, _, supplier_id) = setup_entry_fixture("attachmentdownload", now);
        let invoice_id = create_attachment_entry_invoice(&mut connection, now, supplier_id);
        let source_path = app_data_dir.join("attachments").join("corrupt.pdf");
        let destination_path = destination_dir.join("copied.pdf");
        fs::write(&source_path, b"not a pdf").unwrap();
        connection
            .execute(
                "UPDATE invoices SET attachmentFileName = 'corrupt.pdf', attachmentOriginalName = 'factura.pdf',
                    attachmentSizeBytes = 100, attachmentUploadedAt = ?1 WHERE id = ?2",
                (format_utc_timestamp(now), invoice_id),
            )
            .unwrap();
        super::change_invoice_status_inner(&mut connection, now, invoice_id, "completed", None)
            .unwrap();

        super::download_invoice_attachment_inner(
            &mut connection,
            now,
            invoice_id,
            &destination_path,
            &app_data_dir,
        )
        .unwrap();

        assert_eq!(fs::read(destination_path).unwrap(), b"not a pdf");
        let error_log = fs::read_to_string(app_data_dir.join("errors.log")).unwrap();
        assert!(error_log.contains("attachment size mismatch"));
        fs::remove_dir_all(app_data_dir).unwrap();
        fs::remove_dir_all(destination_dir).unwrap();
    }

    #[test]
    fn download_invoice_attachment_reports_missing_file_and_requires_session() {
        let now = Utc.with_ymd_and_hms(2026, 10, 4, 12, 0, 0).unwrap();
        let app_data_dir = temp_dir();
        fs::create_dir_all(app_data_dir.join("attachments")).unwrap();
        let (mut connection, _, supplier_id) = setup_entry_fixture("attachmentmissing", now);
        let invoice_id = create_attachment_entry_invoice(&mut connection, now, supplier_id);
        connection
            .execute(
                "UPDATE invoices SET attachmentFileName = 'missing.pdf', attachmentOriginalName = 'factura.pdf',
                    attachmentSizeBytes = 10, attachmentUploadedAt = ?1 WHERE id = ?2",
                (format_utc_timestamp(now), invoice_id),
            )
            .unwrap();
        let destination = app_data_dir.join("download.pdf");
        let error = super::download_invoice_attachment_inner(
            &mut connection,
            now,
            invoice_id,
            &destination,
            &app_data_dir,
        )
        .unwrap_err();
        assert_eq!(error.code, "notFound");
        assert_eq!(
            error.message,
            "El archivo adjunto no se encuentra en el equipo. Puede haber sido movido o eliminado"
        );

        assert_requires_session(|connection, now| {
            super::download_invoice_attachment_inner(
                connection,
                now,
                1,
                &destination,
                &app_data_dir,
            )
        });
        fs::remove_dir_all(app_data_dir).unwrap();
    }

    #[test]
    fn save_and_download_invoice_attachment_require_session_and_allow_both_roles() {
        let app_data_dir = temp_dir();
        fs::create_dir_all(&app_data_dir).unwrap();
        let source_path = app_data_dir.join("matriz.pdf");
        let destination_path = app_data_dir.join("destino.pdf");
        fs::write(&source_path, b"%PDF-1.7 matriz").unwrap();
        assert_requires_session(|connection, now| {
            super::save_invoice_attachment_inner(
                connection,
                now,
                1,
                Some(&source_path),
                &app_data_dir,
            )
        });
        assert_allows_both_roles(|connection, now| {
            connection
                .execute(
                    "INSERT INTO suppliers (firstName, lastName, nationalId, phone, createdAt, updatedAt)
                     VALUES ('Ana', 'Pérez', '12345678930', '+5355559096', ?1, ?1)",
                    [format_utc_timestamp(now)],
                )
                .unwrap();
            let supplier_id = connection.last_insert_rowid();
            let product_id = insert_product_fixture(
                connection,
                "Producto autorización adjunto",
                None,
                None,
                100,
                0,
                0,
                "active",
                None,
                None,
            );
            let invoice = super::create_entry_invoice_inner(
                connection,
                now,
                entry_input(
                    supplier_id,
                    vec![entry_line(Some(product_id), None, "1,000", "1,00")],
                    None,
                ),
            )?;
            super::save_invoice_attachment_inner(
                connection,
                now,
                invoice.invoice_id,
                Some(&source_path),
                &app_data_dir,
            )?;
            super::download_invoice_attachment_inner(
                connection,
                now,
                invoice.invoice_id,
                &destination_path,
                &app_data_dir,
            )
        });
        assert_eq!(fs::read(&destination_path).unwrap(), b"%PDF-1.7 matriz");
        fs::remove_dir_all(app_data_dir).unwrap();
    }

    fn add_entry_lot_consumption(
        connection: &rusqlite::Connection,
        now: chrono::DateTime<Utc>,
        user_id: i64,
        product_id: i64,
        lot_id: i64,
        quantity_milli: i64,
        invoice_number: &str,
    ) {
        let sale_id = insert_invoice(
            connection,
            user_id,
            "V",
            invoice_number,
            Some("pending"),
            now,
        );
        let sale_line_id =
            insert_invoice_line(connection, sale_id, Some(product_id), product_id, now);
        let (remaining_milli, unit_cost_cents): (i64, i64) = connection
            .query_row(
                "SELECT remainingMilli, unitCostCents FROM lots WHERE id = ?1",
                [lot_id],
                |row| Ok((row.get(0)?, row.get(1)?)),
            )
            .unwrap();
        connection
            .execute(
                "INSERT INTO consumptions (invoiceLineId, lotId, quantityMilli, unitCostCents, createdAt)
                 VALUES (?1, ?2, ?3, ?4, ?5)",
                (sale_line_id, lot_id, quantity_milli, unit_cost_cents, format_utc_timestamp(now)),
            )
            .unwrap();
        connection
            .execute(
                "UPDATE lots SET remainingMilli = ?1 WHERE id = ?2",
                (remaining_milli - quantity_milli, lot_id),
            )
            .unwrap();
        connection
            .execute(
                "UPDATE products SET quantityMilli = (SELECT COALESCE(SUM(remainingMilli), 0) FROM lots WHERE productId = ?1)
                 WHERE id = ?1",
                [product_id],
            )
            .unwrap();
    }

    #[test]
    fn create_entry_invoice_inner_creates_valid_entry_and_tracks_lot_and_movement() {
        let now = Utc.with_ymd_and_hms(2026, 10, 4, 12, 0, 0).unwrap();
        let mut connection = setup_test_db();
        let user_id = create_test_user(
            &connection,
            "entrycreator",
            "contador",
            "+5350000131",
            "12345678921",
            "ValidPass1!",
        );
        create_test_session(&connection, user_id, false, now, now);

        connection
            .execute(
                "INSERT INTO suppliers (firstName, lastName, nationalId, phone, createdAt, updatedAt)
                 VALUES ('Ana', 'Pérez', '12345678922', '+5355559090', ?1, ?1)",
                [format_utc_timestamp(now)],
            )
            .unwrap();
        let supplier_id = connection.last_insert_rowid();

        let product_id = insert_product_fixture(
            &connection,
            "Producto de compra",
            None,
            None,
            100,
            0,
            0,
            "active",
            None,
            None,
        );

        let input = crate::models::invoice::CreateEntryInvoiceInput {
            supplier_id,
            invoice_date: "2026-10-04".to_owned(),
            status: Some("pending".to_owned()),
            notes: None,
            payment_method: None,
            attachment_file_name: None,
            attachment_original_name: None,
            attachment_size_bytes: None,
            attachment_uploaded_at: None,
            lines: vec![crate::models::invoice::EntryInvoiceLineInput {
                product_id: Some(product_id),
                new_product: None,
                quantity: "3,000".to_owned(),
                unit_price: "1,50".to_owned(),
            }],
        };

        let result = super::create_entry_invoice_inner(&mut connection, now, input).unwrap();
        assert_eq!(result.invoice_id, 1);
        assert_eq!(result.number, "E-00001");

        let stock: i64 = connection
            .query_row(
                "SELECT quantityMilli FROM products WHERE id = ?1",
                [product_id],
                |row| row.get(0),
            )
            .unwrap();
        assert_eq!(stock, 3_000);

        let lot_row: (i64, i64) = connection
            .query_row(
                "SELECT quantityMilli, unitPriceCents FROM productMovements WHERE invoiceNumber = 'E-00001' AND type = 'entry'",
                [],
                |row| Ok((row.get(0)?, row.get(1)?)),
            )
            .unwrap();
        assert_eq!(lot_row, (3_000, 150));
    }

    #[test]
    fn create_entry_invoice_inner_creates_new_product_in_the_entry_lot_and_snapshots_attachment() {
        let now = Utc.with_ymd_and_hms(2026, 10, 4, 12, 0, 0).unwrap();
        let mut connection = setup_test_db();
        let user_id = create_test_user(
            &connection,
            "newentryproduct",
            "contador",
            "+5350000132",
            "12345678923",
            "ValidPass1!",
        );
        create_test_session(&connection, user_id, false, now, now);
        connection
            .execute(
                "INSERT INTO suppliers (firstName, lastName, nationalId, phone, createdAt, updatedAt)
                 VALUES ('Ana', 'Pérez', '12345678924', '+5355559091', ?1, ?1)",
                [format_utc_timestamp(now)],
            )
            .unwrap();
        let supplier_id = connection.last_insert_rowid();
        let input = crate::models::invoice::CreateEntryInvoiceInput {
            supplier_id,
            invoice_date: "2026-10-04".to_owned(),
            status: Some("completed".to_owned()),
            notes: None,
            payment_method: Some("cash".to_owned()),
            attachment_file_name: Some("uuid.pdf".to_owned()),
            attachment_original_name: Some("compra.pdf".to_owned()),
            attachment_size_bytes: Some(123),
            attachment_uploaded_at: Some(format_utc_timestamp(now)),
            lines: vec![crate::models::invoice::EntryInvoiceLineInput {
                product_id: None,
                new_product: Some(crate::models::invoice::NewProductInput {
                    name: "Producto nuevo".to_owned(),
                    description: Some("Creado desde compra".to_owned()),
                    category_id: None,
                    unit_id: None,
                    cost: "1,50".to_owned(),
                    profit_percent: "20,00".to_owned(),
                    expires_on: None,
                }),
                quantity: "3,000".to_owned(),
                unit_price: "1,50".to_owned(),
            }],
        };

        let result = super::create_entry_invoice_inner(&mut connection, now, input).unwrap();

        let product_row: (i64, i64, i64) = connection
            .query_row(
                "SELECT id, quantityMilli, costCents FROM products WHERE name = 'Producto nuevo'",
                [],
                |row| Ok((row.get(0)?, row.get(1)?, row.get(2)?)),
            )
            .unwrap();
        assert_eq!(product_row.1, 3_000);
        assert_eq!(product_row.2, 150);
        let lot_count: i64 = connection
            .query_row(
                "SELECT COUNT(*) FROM lots WHERE productId = ?1",
                [product_row.0],
                |row| row.get(0),
            )
            .unwrap();
        assert_eq!(lot_count, 1);
        let invoice_snapshot: (String, Option<String>, Option<String>, Option<i64>, Option<String>) = connection
            .query_row(
                "SELECT status, attachmentFileName, attachmentOriginalName, attachmentSizeBytes, attachmentUploadedAt
                 FROM invoices WHERE id = ?1",
                [result.invoice_id],
                |row| Ok((row.get(0)?, row.get(1)?, row.get(2)?, row.get(3)?, row.get(4)?)),
            )
            .unwrap();
        assert_eq!(invoice_snapshot.0, "completed");
        assert_eq!(invoice_snapshot.1.as_deref(), Some("uuid.pdf"));
        assert_eq!(invoice_snapshot.2.as_deref(), Some("compra.pdf"));
        assert_eq!(invoice_snapshot.3, Some(123));
        assert_eq!(
            invoice_snapshot.4.as_deref(),
            Some(format_utc_timestamp(now).as_str())
        );
        let paid_at: Option<String> = connection
            .query_row(
                "SELECT paidAt FROM invoices WHERE id = ?1",
                [result.invoice_id],
                |row| row.get(0),
            )
            .unwrap();
        assert_eq!(paid_at.as_deref(), Some(format_utc_timestamp(now).as_str()));
    }

    #[test]
    fn create_entry_invoice_creates_a_lot_and_movement_per_line_in_arrival_order() {
        let now = Utc.with_ymd_and_hms(2026, 10, 4, 12, 0, 0).unwrap();
        let (mut connection, _, supplier_id) = setup_entry_fixture("entrymultiline", now);
        let first_product_id = insert_product_fixture(
            &connection,
            "Primer producto",
            None,
            None,
            100,
            0,
            0,
            "active",
            None,
            None,
        );
        let second_product_id = insert_product_fixture(
            &connection,
            "Segundo producto",
            None,
            None,
            100,
            0,
            0,
            "active",
            None,
            None,
        );
        let input = entry_input(
            supplier_id,
            vec![
                entry_line(Some(first_product_id), None, "2,000", "1,25"),
                entry_line(Some(second_product_id), None, "3,000", "2,50"),
            ],
            None,
        );

        let result = super::create_entry_invoice_inner(&mut connection, now, input).unwrap();

        assert_eq!(result.number, "E-00001");
        let lot_orders: Vec<(i64, i64)> = {
            let mut statement = connection
                .prepare("SELECT productId, arrivalOrder FROM lots ORDER BY id")
                .unwrap();
            statement
                .query_map([], |row| Ok((row.get(0)?, row.get(1)?)))
                .unwrap()
                .map(Result::unwrap)
                .collect()
        };
        assert_eq!(
            lot_orders,
            vec![(first_product_id, 1), (second_product_id, 2)]
        );
        let movement_count: i64 = connection
            .query_row(
                "SELECT COUNT(*) FROM productMovements WHERE invoiceId = ?1 AND type = 'entry'",
                [result.invoice_id],
                |row| row.get(0),
            )
            .unwrap();
        assert_eq!(movement_count, 2);
        let supplier_snapshot: (String, String) = connection
            .query_row(
                "SELECT contactFullName, contactNationalId FROM invoices WHERE id = ?1",
                [result.invoice_id],
                |row| Ok((row.get(0)?, row.get(1)?)),
            )
            .unwrap();
        assert_eq!(
            supplier_snapshot,
            ("Ana Pérez".to_owned(), "12345678926".to_owned())
        );
        let line_snapshots: Vec<(i64, String, String)> = {
            let mut statement = connection
                .prepare("SELECT productIdOriginal, productName, productNameNormalized FROM invoiceLines WHERE invoiceId = ?1 ORDER BY position")
                .unwrap();
            statement
                .query_map([result.invoice_id], |row| {
                    Ok((row.get(0)?, row.get(1)?, row.get(2)?))
                })
                .unwrap()
                .map(Result::unwrap)
                .collect()
        };
        assert_eq!(
            line_snapshots,
            vec![
                (
                    first_product_id,
                    "Primer producto".to_owned(),
                    "primer producto".to_owned()
                ),
                (
                    second_product_id,
                    "Segundo producto".to_owned(),
                    "segundo producto".to_owned()
                ),
            ]
        );
    }

    #[test]
    fn create_entry_invoice_rejects_duplicate_new_product_without_saving_invoice() {
        let now = Utc.with_ymd_and_hms(2026, 10, 4, 12, 0, 0).unwrap();
        let (mut connection, _, supplier_id) = setup_entry_fixture("entryduplicate", now);
        insert_product_fixture(
            &connection,
            "Producto repetido",
            None,
            None,
            100,
            0,
            0,
            "active",
            None,
            None,
        );
        let input = entry_input(
            supplier_id,
            vec![entry_line(
                None,
                Some(new_product("  PRODUCTO REPETIDO  ")),
                "1,000",
                "1,00",
            )],
            None,
        );

        let error = super::create_entry_invoice_inner(&mut connection, now, input).unwrap_err();

        assert_eq!(error.code, "duplicateField");
        assert_eq!(error.message, "Ya existe un producto con ese nombre");
        let invoice_count: i64 = connection
            .query_row("SELECT COUNT(*) FROM invoices", [], |row| row.get(0))
            .unwrap();
        assert_eq!(invoice_count, 0);
    }

    #[test]
    fn create_entry_invoice_rolls_back_new_product_invoice_lines_and_counter_when_lot_insert_fails()
    {
        let now = Utc.with_ymd_and_hms(2026, 10, 4, 12, 0, 0).unwrap();
        let (mut connection, _, supplier_id) = setup_entry_fixture("entryrollback", now);
        connection
            .execute_batch(
                "CREATE TRIGGER fail_entry_lot BEFORE INSERT ON lots
                 BEGIN SELECT RAISE(ABORT, 'forced lot insertion failure'); END;",
            )
            .unwrap();
        let input = entry_input(
            supplier_id,
            vec![entry_line(
                None,
                Some(new_product("Producto transaccional")),
                "1,000",
                "1,00",
            )],
            None,
        );

        assert!(super::create_entry_invoice_inner(&mut connection, now, input).is_err());

        for table in [
            "products",
            "invoices",
            "invoiceLines",
            "lots",
            "productMovements",
        ] {
            let count: i64 = connection
                .query_row(&format!("SELECT COUNT(*) FROM {table}"), [], |row| {
                    row.get(0)
                })
                .unwrap();
            assert_eq!(count, 0, "la tabla {table} debe quedar intacta");
        }
        let next_number: i64 = connection
            .query_row(
                "SELECT nextNumber FROM invoiceCounters WHERE type = 'E'",
                [],
                |row| row.get(0),
            )
            .unwrap();
        assert_eq!(next_number, 1);
    }

    #[test]
    fn create_entry_invoice_enforces_stock_limit_atomically() {
        let now = Utc.with_ymd_and_hms(2026, 10, 4, 12, 0, 0).unwrap();
        for (username, quantity, expected_error) in [
            ("entrystockreject", "2,000", Some("stockLimitExceeded")),
            ("entrystockaccept", "0,999", None),
        ] {
            let (mut connection, _, supplier_id) = setup_entry_fixture(username, now);
            let product_id = insert_product_fixture(
                &connection,
                "Producto al límite",
                None,
                None,
                100,
                0,
                999_999_000,
                "active",
                None,
                None,
            );
            connection
                .execute(
                    "INSERT INTO lots (productId, origin, arrivalOrder, initialMilli, remainingMilli, unitCostCents, createdAt, updatedAt)
                     VALUES (?1, 'initial', 1, 999999000, 999999000, 100, ?2, ?2)",
                    (product_id, format_utc_timestamp(now)),
                )
                .unwrap();
            let input = entry_input(
                supplier_id,
                vec![entry_line(Some(product_id), None, quantity, "1,00")],
                None,
            );
            let result = super::create_entry_invoice_inner(&mut connection, now, input);

            if let Some(expected_error) = expected_error {
                let error = result.unwrap_err();
                assert_eq!(error.code, expected_error);
                for table in ["invoices", "invoiceLines", "productMovements"] {
                    let count: i64 = connection
                        .query_row(&format!("SELECT COUNT(*) FROM {table}"), [], |row| {
                            row.get(0)
                        })
                        .unwrap();
                    assert_eq!(count, 0, "la tabla {table} no debe recibir filas");
                }
                let lot_count: i64 = connection
                    .query_row(
                        "SELECT COUNT(*) FROM lots WHERE productId = ?1",
                        [product_id],
                        |row| row.get(0),
                    )
                    .unwrap();
                assert_eq!(lot_count, 1);
            } else {
                result.unwrap();
                let stock: i64 = connection
                    .query_row(
                        "SELECT quantityMilli FROM products WHERE id = ?1",
                        [product_id],
                        |row| row.get(0),
                    )
                    .unwrap();
                assert_eq!(stock, 999_999_999);
            }
        }
    }

    #[test]
    fn create_entry_invoice_validates_limits_references_products_dates_and_total() {
        let now = Utc.with_ymd_and_hms(2026, 10, 4, 12, 0, 0).unwrap();
        type InputMutation = Box<dyn Fn(&mut crate::models::invoice::CreateEntryInvoiceInput, i64)>;
        let cases: Vec<(&str, InputMutation)> = vec![
            ("empty lines", Box::new(|input, _| input.lines.clear())),
            (
                "too many lines",
                Box::new(|input, product_id| {
                    input.lines = (0..101)
                        .map(|_| entry_line(Some(product_id), None, "1,000", "1,00"))
                        .collect();
                }),
            ),
            (
                "repeated product",
                Box::new(|input, product_id| {
                    input.lines = vec![
                        entry_line(Some(product_id), None, "1,000", "1,00"),
                        entry_line(Some(product_id), None, "1,000", "1,00"),
                    ];
                }),
            ),
            (
                "invalid initial status",
                Box::new(|input, _| input.status = Some("cancelled".to_owned())),
            ),
            (
                "future date",
                Box::new(|input, _| input.invoice_date = "2026-10-05".to_owned()),
            ),
            (
                "date before supported range",
                Box::new(|input, _| input.invoice_date = "1999-12-31".to_owned()),
            ),
            (
                "impossible date",
                Box::new(|input, _| input.invoice_date = "2026-02-30".to_owned()),
            ),
            (
                "invoice total",
                Box::new(|input, product_id| {
                    input.lines = vec![entry_line(
                        Some(product_id),
                        None,
                        "999999,999",
                        "999999,99",
                    )];
                }),
            ),
        ];
        for (index, (_, mutate)) in cases.into_iter().enumerate() {
            let (mut connection, _, supplier_id) =
                setup_entry_fixture(&format!("entryvalidation{index}"), now);
            let product_id = insert_product_fixture(
                &connection,
                "Producto de validación",
                None,
                None,
                100,
                0,
                0,
                "active",
                None,
                None,
            );
            let mut input = entry_input(
                supplier_id,
                vec![entry_line(Some(product_id), None, "1,000", "1,00")],
                None,
            );
            mutate(&mut input, product_id);
            let error = super::create_entry_invoice_inner(&mut connection, now, input).unwrap_err();
            assert_eq!(error.code, "validationFailed");
            let invoice_count: i64 = connection
                .query_row("SELECT COUNT(*) FROM invoices", [], |row| row.get(0))
                .unwrap();
            assert_eq!(invoice_count, 0);
        }

        let (mut connection, _, supplier_id) = setup_entry_fixture("entrymissingref", now);
        let product_id = insert_product_fixture(
            &connection,
            "Producto activo",
            None,
            None,
            100,
            0,
            0,
            "active",
            None,
            None,
        );
        for (supplier, line, expected_message) in [
            (
                i64::MAX,
                entry_line(Some(product_id), None, "1,000", "1,00"),
                "El proveedor ya no existe",
            ),
            (
                supplier_id,
                entry_line(Some(i64::MAX), None, "1,000", "1,00"),
                "El producto ya no existe",
            ),
        ] {
            let error = super::create_entry_invoice_inner(
                &mut connection,
                now,
                entry_input(supplier, vec![line], None),
            )
            .unwrap_err();
            assert_eq!(error.code, "notFound");
            assert_eq!(error.message, expected_message);
        }
        let discontinued_id = insert_product_fixture(
            &connection,
            "Producto discontinuado",
            None,
            None,
            100,
            0,
            0,
            "discontinued",
            Some(&format_utc_timestamp(now)),
            None,
        );
        let error = super::create_entry_invoice_inner(
            &mut connection,
            now,
            entry_input(
                supplier_id,
                vec![entry_line(Some(discontinued_id), None, "1,000", "1,00")],
                None,
            ),
        )
        .unwrap_err();
        assert_eq!(error.code, "validationFailed");
        assert_eq!(error.message, "El producto «Producto discontinuado» está descontinuado. Actívalo para usarlo en facturas");
    }

    #[test]
    fn create_entry_invoice_accepts_the_maximum_of_100_lines() {
        let now = Utc.with_ymd_and_hms(2026, 10, 4, 12, 0, 0).unwrap();
        let (mut connection, _, supplier_id) = setup_entry_fixture("entryhundredlines", now);
        let lines = (0..100)
            .map(|index| {
                entry_line(
                    None,
                    Some(new_product(&format!("Producto {index:03}"))),
                    "1,000",
                    "1,00",
                )
            })
            .collect();

        let result = super::create_entry_invoice_inner(
            &mut connection,
            now,
            entry_input(supplier_id, lines, Some("pending")),
        )
        .unwrap();

        let line_count: i64 = connection
            .query_row(
                "SELECT COUNT(*) FROM invoiceLines WHERE invoiceId = ?1",
                [result.invoice_id],
                |row| row.get(0),
            )
            .unwrap();
        assert_eq!(line_count, 100);
    }

    #[test]
    fn create_entry_invoice_enforces_the_invoice_total_limit_in_cents() {
        let now = Utc.with_ymd_and_hms(2026, 10, 4, 12, 0, 0).unwrap();
        for (username, unit_price, should_succeed) in [
            ("entrytotallimitok", "1000,00", true),
            ("entrytotallimitfail", "1000,01", false),
        ] {
            let (mut connection, _, supplier_id) = setup_entry_fixture(username, now);
            let product_id = insert_product_fixture(
                &connection,
                "Producto total",
                None,
                None,
                100,
                0,
                0,
                "active",
                None,
                None,
            );
            let input = entry_input(
                supplier_id,
                vec![entry_line(Some(product_id), None, "999999,999", unit_price)],
                None,
            );
            let result = super::create_entry_invoice_inner(&mut connection, now, input);

            if should_succeed {
                let result = result.unwrap();
                let total_cents: i64 = connection
                    .query_row(
                        "SELECT subtotalCents FROM invoiceLines WHERE invoiceId = ?1",
                        [result.invoice_id],
                        |row| row.get(0),
                    )
                    .unwrap();
                assert_eq!(total_cents, 99_999_999_900);
            } else {
                let error = result.unwrap_err();
                assert_eq!(error.code, "validationFailed");
                let invoice_count: i64 = connection
                    .query_row("SELECT COUNT(*) FROM invoices", [], |row| row.get(0))
                    .unwrap();
                assert_eq!(invoice_count, 0);
            }
        }
    }

    fn setup_sale_fixture(
        username: &str,
        client_national_id: &str,
        client_phone: &str,
        supplier_national_id: &str,
        supplier_phone: &str,
        now: chrono::DateTime<Utc>,
    ) -> (rusqlite::Connection, i64, i64, i64) {
        let connection = setup_test_db();
        let user_id = create_test_user(
            &connection,
            username,
            "contador",
            "+5350000133",
            "12345678925",
            "ValidPass1!",
        );
        create_test_session(&connection, user_id, false, now, now);
        connection
            .execute(
                "INSERT INTO clients (firstName, lastName, nationalId, phone, createdAt, updatedAt)
                 VALUES ('Luis', 'García', ?1, ?2, ?3, ?3)",
                (client_national_id, client_phone, format_utc_timestamp(now)),
            )
            .unwrap();
        let client_id = connection.last_insert_rowid();
        connection
            .execute(
                "INSERT INTO suppliers (firstName, lastName, nationalId, phone, createdAt, updatedAt)
                 VALUES ('Ana', 'Pérez', ?1, ?2, ?3, ?3)",
                (
                    supplier_national_id,
                    supplier_phone,
                    format_utc_timestamp(now),
                ),
            )
            .unwrap();
        let supplier_id = connection.last_insert_rowid();
        (connection, user_id, client_id, supplier_id)
    }

    fn sale_line(product_id: i64, quantity: &str) -> crate::models::invoice::SaleInvoiceLineInput {
        crate::models::invoice::SaleInvoiceLineInput {
            product_id,
            quantity: quantity.to_owned(),
        }
    }

    fn sale_input(
        client_id: i64,
        lines: Vec<crate::models::invoice::SaleInvoiceLineInput>,
        status: Option<&str>,
    ) -> crate::models::invoice::CreateSaleInvoiceInput {
        crate::models::invoice::CreateSaleInvoiceInput {
            reserved_until: None,
            client_id,
            invoice_date: "2026-10-04".to_owned(),
            status: status.map(str::to_owned),
            notes: None,
            payment_method: None,
            lines,
        }
    }

    /// Crea un producto activo y lo abastece con una entrada real, para que
    /// las cachés (`quantityMilli`, `costCents`) queden sincronizadas.
    #[allow(clippy::too_many_arguments)]
    fn insert_stocked_product(
        connection: &mut rusqlite::Connection,
        now: chrono::DateTime<Utc>,
        supplier_id: i64,
        name: &str,
        cost_cents: i64,
        profit_hundredths: i64,
        stock_quantity: &str,
        unit_cost: &str,
    ) -> i64 {
        let product_id = insert_product_fixture(
            connection,
            name,
            None,
            None,
            cost_cents,
            profit_hundredths,
            0,
            "active",
            None,
            None,
        );
        super::create_entry_invoice_inner(
            connection,
            now,
            entry_input(
                supplier_id,
                vec![entry_line(
                    Some(product_id),
                    None,
                    stock_quantity,
                    unit_cost,
                )],
                None,
            ),
        )
        .unwrap();
        product_id
    }

    #[test]
    fn create_sale_invoice_creates_valid_sale_with_fifo_and_movement() {
        let now = Utc.with_ymd_and_hms(2026, 10, 4, 12, 0, 0).unwrap();
        let (mut connection, _, client_id, supplier_id) = setup_sale_fixture(
            "salevalid",
            "12345678930",
            "+5355559094",
            "12345678931",
            "+5355559095",
            now,
        );
        let category_id = crate::test_utils::insert_category_fixture(&connection, "Bebidas", None);
        let product_id = insert_product_fixture(
            &connection,
            "Café molido",
            Some(category_id),
            None,
            100,
            2_000,
            0,
            "active",
            None,
            None,
        );
        super::create_entry_invoice_inner(
            &mut connection,
            now,
            entry_input(
                supplier_id,
                vec![entry_line(Some(product_id), None, "10", "1,00")],
                None,
            ),
        )
        .unwrap();

        let result = super::create_sale_invoice_inner(
            &mut connection,
            now,
            sale_input(client_id, vec![sale_line(product_id, "2")], None),
        )
        .unwrap();
        assert_eq!(result.number, "V-00001");

        // El precio de venta vigente: 100 * 12000 / 10000 = 120.
        let line: (i64, i64, i64, Option<i64>, Option<String>, Option<i64>) = connection
            .query_row(
                "SELECT quantityMilli, unitPriceCents, subtotalCents, categoryId, categoryName, costAtSaleCents
                 FROM invoiceLines WHERE invoiceId = ?1",
                [result.invoice_id],
                |row| Ok((row.get(0)?, row.get(1)?, row.get(2)?, row.get(3)?, row.get(4)?, row.get(5)?)),
            )
            .unwrap();
        assert_eq!(
            line,
            (
                2_000,
                120,
                240,
                Some(category_id),
                Some("Bebidas".to_owned()),
                Some(100)
            )
        );

        // FIFO: el lote de la entrada queda con 8 unidades.
        let remaining: i64 = connection
            .query_row(
                "SELECT remainingMilli FROM lots WHERE productId = ?1",
                [product_id],
                |row| row.get(0),
            )
            .unwrap();
        assert_eq!(remaining, 8_000);
        let quantity: i64 = connection
            .query_row(
                "SELECT quantityMilli FROM products WHERE id = ?1",
                [product_id],
                |row| row.get(0),
            )
            .unwrap();
        assert_eq!(quantity, 8_000);

        // Movimiento de salida con cantidad negativa y precio de venta.
        let movement: (String, i64, i64, i64, i64, Option<String>) = connection
            .query_row(
                "SELECT type, quantityMilli, stockAfterMilli, unitPriceCents, totalPriceCents, invoiceNumber
                 FROM productMovements WHERE invoiceId = ?1",
                [result.invoice_id],
                |row| Ok((row.get(0)?, row.get(1)?, row.get(2)?, row.get(3)?, row.get(4)?, row.get(5)?)),
            )
            .unwrap();
        assert_eq!(
            movement,
            (
                "saleOut".to_owned(),
                -2_000,
                8_000,
                120,
                240,
                Some("V-00001".to_owned())
            )
        );

        let next_number: i64 = connection
            .query_row(
                "SELECT nextNumber FROM invoiceCounters WHERE type = 'V'",
                [],
                |row| row.get(0),
            )
            .unwrap();
        assert_eq!(next_number, 2);
    }

    #[test]
    fn create_sale_invoice_consumes_lots_in_arrival_order() {
        let now = Utc.with_ymd_and_hms(2026, 10, 4, 12, 0, 0).unwrap();
        let (mut connection, _, client_id, supplier_id) = setup_sale_fixture(
            "salefifo",
            "12345678932",
            "+5355559096",
            "12345678933",
            "+5355559097",
            now,
        );
        let product_id = insert_product_fixture(
            &connection,
            "Azúcar",
            None,
            None,
            100,
            0,
            0,
            "active",
            None,
            None,
        );
        super::create_entry_invoice_inner(
            &mut connection,
            now,
            entry_input(
                supplier_id,
                vec![entry_line(Some(product_id), None, "10", "1,00")],
                None,
            ),
        )
        .unwrap();
        super::create_entry_invoice_inner(
            &mut connection,
            now,
            entry_input(
                supplier_id,
                vec![entry_line(Some(product_id), None, "5", "3,00")],
                None,
            ),
        )
        .unwrap();

        // 10 a $1,00 + 5 a $3,00; vender 12 consume 10 del primero y 2 del segundo.
        let result = super::create_sale_invoice_inner(
            &mut connection,
            now,
            sale_input(client_id, vec![sale_line(product_id, "12")], None),
        )
        .unwrap();
        let consumptions: Vec<(i64, i64)> = connection
            .prepare(
                "SELECT c.quantityMilli, c.unitCostCents FROM consumptions c
                 JOIN invoiceLines il ON il.id = c.invoiceLineId
                 WHERE il.invoiceId = ?1 ORDER BY c.id",
            )
            .unwrap()
            .query_map([result.invoice_id], |row| Ok((row.get(0)?, row.get(1)?)))
            .unwrap()
            .collect::<rusqlite::Result<Vec<_>>>()
            .unwrap();
        assert_eq!(consumptions, vec![(10_000, 100), (2_000, 300)]);
        let remaining: Vec<i64> = connection
            .prepare("SELECT remainingMilli FROM lots WHERE productId = ?1 ORDER BY arrivalOrder")
            .unwrap()
            .query_map([product_id], |row| row.get(0))
            .unwrap()
            .collect::<rusqlite::Result<Vec<_>>>()
            .unwrap();
        assert_eq!(remaining, vec![0, 3_000]);
    }

    #[test]
    fn create_sale_invoice_category_snapshot_survives_category_deletion() {
        let now = Utc.with_ymd_and_hms(2026, 10, 4, 12, 0, 0).unwrap();
        let (mut connection, _, client_id, supplier_id) = setup_sale_fixture(
            "salecategory",
            "12345678934",
            "+5355559098",
            "12345678935",
            "+5355559099",
            now,
        );
        let category_id = crate::test_utils::insert_category_fixture(&connection, "Granos", None);
        let product_id = insert_product_fixture(
            &connection,
            "Arroz",
            Some(category_id),
            None,
            100,
            0,
            0,
            "active",
            None,
            None,
        );
        super::create_entry_invoice_inner(
            &mut connection,
            now,
            entry_input(
                supplier_id,
                vec![entry_line(Some(product_id), None, "10", "1,00")],
                None,
            ),
        )
        .unwrap();
        let result = super::create_sale_invoice_inner(
            &mut connection,
            now,
            sale_input(client_id, vec![sale_line(product_id, "1")], None),
        )
        .unwrap();

        // La copia no tiene clave foránea: sobrevive a la categoría.
        connection
            .execute("DELETE FROM categories WHERE id = ?1", [category_id])
            .unwrap();
        let snapshot: (Option<i64>, Option<String>) = connection
            .query_row(
                "SELECT categoryId, categoryName FROM invoiceLines WHERE invoiceId = ?1",
                [result.invoice_id],
                |row| Ok((row.get(0)?, row.get(1)?)),
            )
            .unwrap();
        assert_eq!(snapshot, (Some(category_id), Some("Granos".to_owned())));
    }

    #[test]
    fn create_sale_invoice_rejects_insufficient_stock() {
        let now = Utc.with_ymd_and_hms(2026, 10, 4, 12, 0, 0).unwrap();
        let (mut connection, _, client_id, supplier_id) = setup_sale_fixture(
            "salestock",
            "12345678936",
            "+5355559100",
            "12345678937",
            "+5355559101",
            now,
        );
        let product_id = insert_stocked_product(
            &mut connection,
            now,
            supplier_id,
            "Leche",
            100,
            0,
            "3",
            "1,00",
        );

        let error = super::create_sale_invoice_inner(
            &mut connection,
            now,
            sale_input(client_id, vec![sale_line(product_id, "5")], None),
        )
        .unwrap_err();
        assert_eq!(error.code, "stockInsuficiente");
        assert_eq!(
            error.message,
            "No hay stock suficiente de «Leche» (disponible: 3)"
        );
        // Nada parcial: ni factura ni consumos ni movimientos de venta.
        let invoices: i64 = connection
            .query_row(
                "SELECT COUNT(*) FROM invoices WHERE type = 'V'",
                [],
                |row| row.get(0),
            )
            .unwrap();
        assert_eq!(invoices, 0);
    }

    fn notification_messages(connection: &rusqlite::Connection) -> Vec<String> {
        connection
            .prepare("SELECT message FROM notifications ORDER BY id")
            .unwrap()
            .query_map([], |row| row.get(0))
            .unwrap()
            .collect::<rusqlite::Result<_>>()
            .unwrap()
    }

    #[test]
    fn n02_and_n03_follow_the_stock_left_by_a_sale_and_a_rejected_sale() {
        let now = Utc.with_ymd_and_hms(2026, 10, 4, 12, 0, 0).unwrap();
        let (mut connection, _, client_id, supplier_id) = setup_sale_fixture(
            "salewatch",
            "12345678951",
            "+5355559120",
            "12345678952",
            "+5355559121",
            now,
        );
        let product_id = insert_stocked_product(
            &mut connection,
            now,
            supplier_id,
            "Leche",
            100,
            0,
            "10",
            "1,00",
        );
        assert!(notification_messages(&connection).is_empty());

        // La venta deja 4: stock bajo (N-03).
        super::create_sale_invoice_inner(
            &mut connection,
            now,
            sale_input(client_id, vec![sale_line(product_id, "6")], None),
        )
        .unwrap();
        assert_eq!(
            notification_messages(&connection),
            vec!["Producto con stock bajo: Leche - Cantidad actual: 4"]
        );

        // Una venta rechazada no deja nada nuevo: la situación no cambió.
        let error = super::create_sale_invoice_inner(
            &mut connection,
            now,
            sale_input(client_id, vec![sale_line(product_id, "9")], None),
        )
        .unwrap_err();
        assert_eq!(error.code, "stockInsuficiente");
        assert_eq!(notification_messages(&connection).len(), 1);

        // La que lo agota genera N-02 en su propia transacción.
        super::create_sale_invoice_inner(
            &mut connection,
            now,
            sale_input(client_id, vec![sale_line(product_id, "4")], None),
        )
        .unwrap();
        assert_eq!(
            notification_messages(&connection)
                .last()
                .map(String::as_str),
            Some("Producto agotado: Leche")
        );
    }

    #[test]
    fn n02_is_saved_even_when_the_sale_of_an_out_of_stock_product_is_rejected() {
        let now = Utc.with_ymd_and_hms(2026, 10, 4, 12, 0, 0).unwrap();
        let (mut connection, _, client_id, _) = setup_sale_fixture(
            "saleempty",
            "12345678953",
            "+5355559122",
            "12345678954",
            "+5355559123",
            now,
        );
        // Agotado sin aviso previo (por ejemplo, la app no ha arrancado aún).
        let empty_id = insert_product_fixture(
            &connection,
            "Mantequilla",
            None,
            None,
            100,
            0,
            0,
            "active",
            None,
            None,
        );

        let error = super::create_sale_invoice_inner(
            &mut connection,
            now,
            sale_input(client_id, vec![sale_line(empty_id, "1")], None),
        )
        .unwrap_err();
        assert_eq!(error.code, "validationFailed");
        assert_eq!(
            notification_messages(&connection),
            vec!["Producto agotado: Mantequilla"]
        );
        let invoices: i64 = connection
            .query_row(
                "SELECT COUNT(*) FROM invoices WHERE type = 'V'",
                [],
                |row| row.get(0),
            )
            .unwrap();
        assert_eq!(invoices, 0);

        // Otro intento no lo repite (NT-03).
        super::create_sale_invoice_inner(
            &mut connection,
            now,
            sale_input(client_id, vec![sale_line(empty_id, "1")], None),
        )
        .unwrap_err();
        assert_eq!(notification_messages(&connection).len(), 1);
    }

    #[test]
    fn create_sale_invoice_rejects_discontinued_and_out_of_stock_products() {
        let now = Utc.with_ymd_and_hms(2026, 10, 4, 12, 0, 0).unwrap();
        let (mut connection, _, client_id, _supplier_id) = setup_sale_fixture(
            "salestatus",
            "12345678938",
            "+5355559102",
            "12345678939",
            "+5355559103",
            now,
        );
        let discontinued_id = insert_product_fixture(
            &connection,
            "Queso viejo",
            None,
            None,
            100,
            0,
            0,
            "discontinued",
            Some("2026-10-04T12:00:00Z"),
            None,
        );
        let error = super::create_sale_invoice_inner(
            &mut connection,
            now,
            sale_input(client_id, vec![sale_line(discontinued_id, "1")], None),
        )
        .unwrap_err();
        assert_eq!(error.code, "validationFailed");
        assert!(error.message.contains("descontinuado"));

        let empty_id = insert_product_fixture(
            &connection,
            "Mantequilla",
            None,
            None,
            100,
            0,
            0,
            "active",
            None,
            None,
        );
        let error = super::create_sale_invoice_inner(
            &mut connection,
            now,
            sale_input(client_id, vec![sale_line(empty_id, "1")], None),
        )
        .unwrap_err();
        assert_eq!(error.code, "validationFailed");
        assert!(error.message.contains("agotado"));
    }

    #[test]
    fn create_sale_invoice_validates_limits_references_dates_and_total() {
        let now = Utc.with_ymd_and_hms(2026, 10, 4, 12, 0, 0).unwrap();
        let (mut connection, _, client_id, supplier_id) = setup_sale_fixture(
            "salelimits",
            "12345678940",
            "+5355559104",
            "12345678941",
            "+5355559105",
            now,
        );
        let product_id = insert_stocked_product(
            &mut connection,
            now,
            supplier_id,
            "Café límite",
            100,
            0,
            "10",
            "1,00",
        );

        // Sin líneas.
        let error = super::create_sale_invoice_inner(
            &mut connection,
            now,
            sale_input(client_id, Vec::new(), None),
        )
        .unwrap_err();
        assert_eq!(error.code, "validationFailed");

        // Más de 100 líneas.
        let many: Vec<crate::models::invoice::SaleInvoiceLineInput> =
            (1..=101).map(|id| sale_line(id, "1")).collect();
        let error = super::create_sale_invoice_inner(
            &mut connection,
            now,
            sale_input(client_id, many, None),
        )
        .unwrap_err();
        assert_eq!(error.code, "validationFailed");
        assert_eq!(error.message, "Una factura admite entre 1 y 100 productos");

        // Producto repetido.
        let error = super::create_sale_invoice_inner(
            &mut connection,
            now,
            sale_input(
                client_id,
                vec![sale_line(product_id, "1"), sale_line(product_id, "1")],
                None,
            ),
        )
        .unwrap_err();
        assert_eq!(error.code, "validationFailed");
        assert_eq!(error.message, "El producto ya está en la factura");

        // Cliente inexistente.
        let error = super::create_sale_invoice_inner(
            &mut connection,
            now,
            sale_input(9_999, vec![sale_line(product_id, "1")], None),
        )
        .unwrap_err();
        assert_eq!(error.code, "notFound");
        assert_eq!(error.message, "El cliente ya no existe");

        // Fecha futura (lejana para que lo sea en cualquier huso horario).
        let mut future = sale_input(client_id, vec![sale_line(product_id, "1")], None);
        future.invoice_date = "2099-01-01".to_owned();
        let error = super::create_sale_invoice_inner(&mut connection, now, future).unwrap_err();
        assert_eq!(error.code, "validationFailed");
        assert_eq!(error.message, "La fecha no puede ser futura");
    }

    #[test]
    fn create_sale_invoice_rejects_total_above_legal_bound() {
        let now = Utc.with_ymd_and_hms(2026, 10, 4, 12, 0, 0).unwrap();
        let (mut connection, _, client_id, supplier_id) = setup_sale_fixture(
            "saletotal",
            "12345678942",
            "+5355559106",
            "12345678943",
            "+5355559107",
            now,
        );
        // Costo máximo con ganancia máxima: 99.999.999 * 11 = 1.099.999.989 por unidad.
        let product_id = insert_product_fixture(
            &connection,
            "Oro",
            None,
            None,
            99_999_999,
            100_000,
            0,
            "active",
            None,
            None,
        );
        super::create_entry_invoice_inner(
            &mut connection,
            now,
            entry_input(
                supplier_id,
                vec![entry_line(Some(product_id), None, "1000", "999999,99")],
                None,
            ),
        )
        .unwrap();

        let error = super::create_sale_invoice_inner(
            &mut connection,
            now,
            sale_input(client_id, vec![sale_line(product_id, "100")], None),
        )
        .unwrap_err();
        assert_eq!(error.code, "validationFailed");
        assert_eq!(
            error.message,
            "El total de la factura no puede superar $999.999.999,99"
        );
        let invoices: i64 = connection
            .query_row(
                "SELECT COUNT(*) FROM invoices WHERE type = 'V'",
                [],
                |row| row.get(0),
            )
            .unwrap();
        assert_eq!(invoices, 0);
    }

    #[test]
    fn create_sale_invoice_rolls_back_when_consumption_insert_fails() {
        let now = Utc.with_ymd_and_hms(2026, 10, 4, 12, 0, 0).unwrap();
        let (mut connection, _, client_id, supplier_id) = setup_sale_fixture(
            "saleatomic",
            "12345678944",
            "+5355559108",
            "12345678945",
            "+5355559109",
            now,
        );
        let product_id = insert_stocked_product(
            &mut connection,
            now,
            supplier_id,
            "Azúcar atómica",
            100,
            0,
            "10",
            "1,00",
        );
        connection
            .execute_batch(
                "CREATE TRIGGER fail_sale_consumption BEFORE INSERT ON consumptions
                 BEGIN SELECT RAISE(ABORT, 'forced consumption insertion failure'); END;",
            )
            .unwrap();

        assert!(super::create_sale_invoice_inner(
            &mut connection,
            now,
            sale_input(client_id, vec![sale_line(product_id, "2")], None),
        )
        .is_err());

        // Solo queda lo de la entrada de abastecimiento: sin factura de
        // venta, sin líneas ni consumos ni movimientos de venta.
        for (table, expected) in [
            ("invoices", 1),
            ("invoiceLines", 1),
            ("consumptions", 0),
            ("productMovements", 1),
        ] {
            let count: i64 = connection
                .query_row(&format!("SELECT COUNT(*) FROM {table}"), [], |row| {
                    row.get(0)
                })
                .unwrap();
            assert_eq!(
                count, expected,
                "la tabla {table} no debe tener restos de la venta"
            );
        }
        // El lote de la entrada queda intacto y el contador no avanza.
        let remaining: i64 = connection
            .query_row(
                "SELECT remainingMilli FROM lots WHERE productId = ?1",
                [product_id],
                |row| row.get(0),
            )
            .unwrap();
        assert_eq!(remaining, 10_000);
        let next_number: i64 = connection
            .query_row(
                "SELECT nextNumber FROM invoiceCounters WHERE type = 'V'",
                [],
                |row| row.get(0),
            )
            .unwrap();
        assert_eq!(next_number, 1);
    }

    #[test]
    fn change_invoice_status_requires_session_and_allows_both_roles() {
        // Fila de la matriz de autorización que faltaba (§6).
        assert_requires_session(|connection, now| {
            super::change_invoice_status_inner(connection, now, 1, "completed", None)
        });
        assert_allows_both_roles(|connection, now| {
            super::change_invoice_status_inner(connection, now, 1, "completed", None)
        });
    }

    #[test]
    fn create_sale_invoice_requires_session_and_allows_both_roles() {
        assert_requires_session(|connection, now| {
            super::create_sale_invoice_inner(connection, now, sale_input(1, Vec::new(), None))
        });
        assert_allows_both_roles(|connection, now| {
            connection
                .execute(
                    "INSERT INTO clients (firstName, lastName, nationalId, phone, createdAt, updatedAt)
                     VALUES ('Ana', 'Pérez', '12345678946', '+5355559110', ?1, ?1)",
                    [format_utc_timestamp(now)],
                )
                .unwrap();
            let client_id = connection.last_insert_rowid();
            super::create_sale_invoice_inner(
                connection,
                now,
                sale_input(client_id, vec![sale_line(999, "1")], None),
            )
        });
    }

    fn service_line(
        product_id: i64,
        quantity: &str,
    ) -> crate::models::invoice::ServiceInvoiceLineInput {
        crate::models::invoice::ServiceInvoiceLineInput {
            product_id,
            quantity: quantity.to_owned(),
        }
    }

    fn service_input(
        client_id: i64,
        lines: Vec<crate::models::invoice::ServiceInvoiceLineInput>,
        status: Option<&str>,
    ) -> crate::models::invoice::CreateServiceInvoiceInput {
        crate::models::invoice::CreateServiceInvoiceInput {
            reserved_until: None,
            client_id,
            invoice_date: "2026-10-04".to_owned(),
            status: status.map(str::to_owned),
            notes: None,
            payment_method: None,
            lines,
        }
    }

    #[test]
    fn create_service_invoice_creates_valid_service_with_fifo_and_movement() {
        let now = Utc.with_ymd_and_hms(2026, 10, 4, 12, 0, 0).unwrap();
        let (mut connection, _, client_id, supplier_id) = setup_sale_fixture(
            "servicevalid",
            "12345678950",
            "+5355559112",
            "12345678951",
            "+5355559113",
            now,
        );
        let category_id = crate::test_utils::insert_category_fixture(&connection, "Insumos", None);
        let product_id = insert_product_fixture(
            &connection,
            "Cable HDMI",
            Some(category_id),
            None,
            200,
            5_000,
            0,
            "active",
            None,
            None,
        );
        super::create_entry_invoice_inner(
            &mut connection,
            now,
            entry_input(
                supplier_id,
                vec![entry_line(Some(product_id), None, "10", "2,00")],
                None,
            ),
        )
        .unwrap();

        let result = super::create_service_invoice_inner(
            &mut connection,
            now,
            service_input(client_id, vec![service_line(product_id, "3")], None),
        )
        .unwrap();
        assert_eq!(result.number, "S-00001");

        // Tipo S vinculado al cliente, con la copia de sus datos.
        let header: (String, i64, Option<String>) = connection
            .query_row(
                "SELECT type, clientId, contactFullName FROM invoices WHERE id = ?1",
                [result.invoice_id],
                |row| Ok((row.get(0)?, row.get(1)?, row.get(2)?)),
            )
            .unwrap();
        assert_eq!(
            header,
            ("S".to_owned(), client_id, Some("Luis García".to_owned()))
        );

        // Precio de venta vigente: 200 * 15000 / 10000 = 300.
        let line: (i64, i64, i64, Option<i64>, Option<String>, Option<i64>) = connection
            .query_row(
                "SELECT quantityMilli, unitPriceCents, subtotalCents, categoryId, categoryName, costAtSaleCents
                 FROM invoiceLines WHERE invoiceId = ?1",
                [result.invoice_id],
                |row| {
                    Ok((
                        row.get(0)?,
                        row.get(1)?,
                        row.get(2)?,
                        row.get(3)?,
                        row.get(4)?,
                        row.get(5)?,
                    ))
                },
            )
            .unwrap();
        assert_eq!(
            line,
            (
                3_000,
                300,
                900,
                Some(category_id),
                Some("Insumos".to_owned()),
                Some(200)
            )
        );

        let movement: (String, i64, i64, i64, i64, Option<String>) = connection
            .query_row(
                "SELECT type, quantityMilli, stockAfterMilli, unitPriceCents, totalPriceCents, invoiceNumber
                 FROM productMovements WHERE invoiceId = ?1",
                [result.invoice_id],
                |row| {
                    Ok((
                        row.get(0)?,
                        row.get(1)?,
                        row.get(2)?,
                        row.get(3)?,
                        row.get(4)?,
                        row.get(5)?,
                    ))
                },
            )
            .unwrap();
        assert_eq!(
            movement,
            (
                "serviceOut".to_owned(),
                -3_000,
                7_000,
                300,
                900,
                Some("S-00001".to_owned())
            )
        );

        let next_number: i64 = connection
            .query_row(
                "SELECT nextNumber FROM invoiceCounters WHERE type = 'S'",
                [],
                |row| row.get(0),
            )
            .unwrap();
        assert_eq!(next_number, 2);
    }

    #[test]
    fn create_service_invoice_requires_session_and_allows_both_roles() {
        assert_requires_session(|connection, now| {
            super::create_service_invoice_inner(connection, now, service_input(1, Vec::new(), None))
        });
        assert_allows_both_roles(|connection, now| {
            connection
                .execute(
                    "INSERT INTO clients (firstName, lastName, nationalId, phone, createdAt, updatedAt)
                     VALUES ('Ana', 'Pérez', '12345678952', '+5355559114', ?1, ?1)",
                    [format_utc_timestamp(now)],
                )
                .unwrap();
            let client_id = connection.last_insert_rowid();
            super::create_service_invoice_inner(
                connection,
                now,
                service_input(client_id, vec![service_line(999, "1")], None),
            )
        });
    }

    fn guarantee_line(
        product_id: i64,
        quantity: &str,
    ) -> crate::models::invoice::GuaranteeInvoiceLineInput {
        crate::models::invoice::GuaranteeInvoiceLineInput {
            product_id,
            quantity: quantity.to_owned(),
        }
    }

    fn guarantee_input(
        client_id: i64,
        associated_invoice_id: i64,
        lines: Vec<crate::models::invoice::GuaranteeInvoiceLineInput>,
        status: Option<&str>,
    ) -> crate::models::invoice::CreateGuaranteeInvoiceInput {
        crate::models::invoice::CreateGuaranteeInvoiceInput {
            client_id,
            associated_invoice_id,
            invoice_date: "2026-10-04".to_owned(),
            status: status.map(str::to_owned),
            notes: None,
            lines,
        }
    }

    #[test]
    fn create_guarantee_invoice_creates_valid_guarantee_at_real_cost() {
        let now = Utc.with_ymd_and_hms(2026, 10, 4, 12, 0, 0).unwrap();
        let (mut connection, _, client_id, supplier_id) = setup_sale_fixture(
            "guaranteevalid",
            "12345678960",
            "+5355559116",
            "12345678961",
            "+5355559117",
            now,
        );
        let product_id = insert_stocked_product(
            &mut connection,
            now,
            supplier_id,
            "Batidora",
            100,
            0,
            "10",
            "1,00",
        );
        let sale = super::create_sale_invoice_inner(
            &mut connection,
            now,
            sale_input(client_id, vec![sale_line(product_id, "1")], None),
        )
        .unwrap();

        let result = super::create_guarantee_invoice_inner(
            &mut connection,
            now,
            guarantee_input(
                client_id,
                sale.invoice_id,
                vec![guarantee_line(product_id, "2")],
                None,
            ),
        )
        .unwrap();
        assert_eq!(result.number, "GG-00001");

        // Cabecera con la factura asociada y sin pago.
        let header: (
            String,
            String,
            i64,
            String,
            String,
            Option<String>,
            Option<String>,
        ) = connection
            .query_row(
                "SELECT type, status, associatedInvoiceId, associatedNumber, associatedType,
                        paymentMethod, paidAt FROM invoices WHERE id = ?1",
                [result.invoice_id],
                |row| {
                    Ok((
                        row.get(0)?,
                        row.get(1)?,
                        row.get(2)?,
                        row.get(3)?,
                        row.get(4)?,
                        row.get(5)?,
                        row.get(6)?,
                    ))
                },
            )
            .unwrap();
        assert_eq!(
            header,
            (
                "GG".to_owned(),
                "pending".to_owned(),
                sale.invoice_id,
                "V-00001".to_owned(),
                "V".to_owned(),
                None,
                None,
            )
        );

        // Costo real de los lotes: 2 unidades a $1,00.
        let line: (i64, i64, i64, Option<i64>, Option<i64>) = connection
            .query_row(
                "SELECT quantityMilli, unitPriceCents, subtotalCents, categoryId, costAtSaleCents
                 FROM invoiceLines WHERE invoiceId = ?1",
                [result.invoice_id],
                |row| {
                    Ok((
                        row.get(0)?,
                        row.get(1)?,
                        row.get(2)?,
                        row.get(3)?,
                        row.get(4)?,
                    ))
                },
            )
            .unwrap();
        assert_eq!(line, (2_000, 100, 200, None, None));

        let movement: (String, i64, i64, i64, i64, Option<String>) = connection
            .query_row(
                "SELECT type, quantityMilli, stockAfterMilli, unitPriceCents, totalPriceCents, invoiceNumber
                 FROM productMovements WHERE invoiceId = ?1",
                [result.invoice_id],
                |row| {
                    Ok((
                        row.get(0)?,
                        row.get(1)?,
                        row.get(2)?,
                        row.get(3)?,
                        row.get(4)?,
                        row.get(5)?,
                    ))
                },
            )
            .unwrap();
        assert_eq!(
            movement,
            (
                "warrantyOut".to_owned(),
                -2_000,
                7_000,
                100,
                200,
                Some("GG-00001".to_owned())
            )
        );

        let next_number: i64 = connection
            .query_row(
                "SELECT nextNumber FROM invoiceCounters WHERE type = 'GG'",
                [],
                |row| row.get(0),
            )
            .unwrap();
        assert_eq!(next_number, 2);
    }

    #[test]
    fn create_guarantee_invoice_validates_associated_invoice() {
        let now = Utc.with_ymd_and_hms(2026, 10, 4, 12, 0, 0).unwrap();
        let (mut connection, _, client_id, supplier_id) = setup_sale_fixture(
            "guaranteeassoc",
            "12345678962",
            "+5355559118",
            "12345678963",
            "+5355559119",
            now,
        );
        let product_id = insert_stocked_product(
            &mut connection,
            now,
            supplier_id,
            "Licuadora",
            100,
            0,
            "10",
            "1,00",
        );
        let sale = super::create_sale_invoice_inner(
            &mut connection,
            now,
            sale_input(client_id, vec![sale_line(product_id, "1")], None),
        )
        .unwrap();
        let entry_id = super::create_entry_invoice_inner(
            &mut connection,
            now,
            entry_input(
                supplier_id,
                vec![entry_line(Some(product_id), None, "1", "1,00")],
                None,
            ),
        )
        .unwrap()
        .invoice_id;
        connection
            .execute(
                "INSERT INTO clients (firstName, lastName, nationalId, phone, createdAt, updatedAt)
                 VALUES ('Marta', 'Ruiz', '12345678964', '+5355559120', ?1, ?1)",
                [format_utc_timestamp(now)],
            )
            .unwrap();
        let other_client_id = connection.last_insert_rowid();
        let other_sale = super::create_sale_invoice_inner(
            &mut connection,
            now,
            sale_input(other_client_id, vec![sale_line(product_id, "1")], None),
        )
        .unwrap();
        super::change_invoice_status_inner(
            &mut connection,
            now,
            sale.invoice_id,
            "cancelled",
            None,
        )
        .unwrap();

        // De otro cliente.
        let error = super::create_guarantee_invoice_inner(
            &mut connection,
            now,
            guarantee_input(
                client_id,
                other_sale.invoice_id,
                vec![guarantee_line(product_id, "1")],
                None,
            ),
        )
        .unwrap_err();
        assert_eq!(error.code, "validationFailed");
        assert_eq!(
            error.message,
            "La factura original debe ser una venta o servicio de este cliente que no esté cancelada"
        );

        // Cancelada.
        let error = super::create_guarantee_invoice_inner(
            &mut connection,
            now,
            guarantee_input(
                client_id,
                sale.invoice_id,
                vec![guarantee_line(product_id, "1")],
                None,
            ),
        )
        .unwrap_err();
        assert_eq!(error.code, "validationFailed");
        assert_eq!(
            error.message,
            "La factura original debe ser una venta o servicio de este cliente que no esté cancelada"
        );

        // Inexistente.
        let error = super::create_guarantee_invoice_inner(
            &mut connection,
            now,
            guarantee_input(
                client_id,
                9_999,
                vec![guarantee_line(product_id, "1")],
                None,
            ),
        )
        .unwrap_err();
        assert_eq!(error.code, "notFound");
        assert_eq!(error.message, "La factura ya no existe");

        // De entrada.
        let error = super::create_guarantee_invoice_inner(
            &mut connection,
            now,
            guarantee_input(
                client_id,
                entry_id,
                vec![guarantee_line(product_id, "1")],
                None,
            ),
        )
        .unwrap_err();
        assert_eq!(error.code, "validationFailed");
        assert_eq!(
            error.message,
            "La factura original debe ser una venta o servicio de este cliente que no esté cancelada"
        );
    }

    #[test]
    fn create_guarantee_invoice_rolls_back_when_consumption_insert_fails() {
        let now = Utc.with_ymd_and_hms(2026, 10, 4, 12, 0, 0).unwrap();
        let (mut connection, _, client_id, supplier_id) = setup_sale_fixture(
            "guaranteeatomic",
            "12345678965",
            "+5355559121",
            "12345678966",
            "+5355559122",
            now,
        );
        let product_id = insert_stocked_product(
            &mut connection,
            now,
            supplier_id,
            "Olla",
            100,
            0,
            "10",
            "1,00",
        );
        let sale = super::create_sale_invoice_inner(
            &mut connection,
            now,
            sale_input(client_id, vec![sale_line(product_id, "1")], None),
        )
        .unwrap();
        connection
            .execute_batch(
                "CREATE TRIGGER fail_guarantee_consumption BEFORE INSERT ON consumptions
                 BEGIN SELECT RAISE(ABORT, 'forced consumption insertion failure'); END;",
            )
            .unwrap();

        assert!(super::create_guarantee_invoice_inner(
            &mut connection,
            now,
            guarantee_input(
                client_id,
                sale.invoice_id,
                vec![guarantee_line(product_id, "2")],
                None,
            ),
        )
        .is_err());

        // La entrada y la venta quedan intactas; nada de la garantía persiste.
        for (table, expected) in [
            ("invoices", 2),
            ("invoiceLines", 2),
            ("consumptions", 1),
            ("productMovements", 2),
        ] {
            let count: i64 = connection
                .query_row(&format!("SELECT COUNT(*) FROM {table}"), [], |row| {
                    row.get(0)
                })
                .unwrap();
            assert_eq!(
                count, expected,
                "la tabla {table} no debe tener restos de la garantía"
            );
        }
        let next_number: i64 = connection
            .query_row(
                "SELECT nextNumber FROM invoiceCounters WHERE type = 'GG'",
                [],
                |row| row.get(0),
            )
            .unwrap();
        assert_eq!(next_number, 1);
    }

    #[test]
    fn create_guarantee_invoice_requires_session_and_allows_both_roles() {
        assert_requires_session(|connection, now| {
            super::create_guarantee_invoice_inner(
                connection,
                now,
                guarantee_input(1, 1, Vec::new(), None),
            )
        });
        assert_allows_both_roles(|connection, now| {
            connection
                .execute(
                    "INSERT INTO clients (firstName, lastName, nationalId, phone, createdAt, updatedAt)
                     VALUES ('Ana', 'Pérez', '12345678967', '+5355559123', ?1, ?1)",
                    [format_utc_timestamp(now)],
                )
                .unwrap();
            let client_id = connection.last_insert_rowid();
            super::create_guarantee_invoice_inner(
                connection,
                now,
                guarantee_input(client_id, 999, vec![guarantee_line(999, "1")], None),
            )
        });
    }

    fn installation_line(
        product_id: i64,
        quantity: &str,
    ) -> crate::models::invoice::InstallationInvoiceLineInput {
        crate::models::invoice::InstallationInvoiceLineInput {
            product_id,
            quantity: quantity.to_owned(),
        }
    }

    fn installation_input(
        client_id: i64,
        associated_invoice_id: i64,
        lines: Vec<crate::models::invoice::InstallationInvoiceLineInput>,
        status: Option<&str>,
    ) -> crate::models::invoice::CreateInstallationInvoiceInput {
        crate::models::invoice::CreateInstallationInvoiceInput {
            client_id,
            associated_invoice_id,
            invoice_date: "2026-10-04".to_owned(),
            status: status.map(str::to_owned),
            notes: None,
            lines,
        }
    }

    #[test]
    fn create_installation_invoice_creates_valid_installation_at_real_cost() {
        let now = Utc.with_ymd_and_hms(2026, 10, 4, 12, 0, 0).unwrap();
        let (mut connection, _, client_id, supplier_id) = setup_sale_fixture(
            "installationvalid",
            "12345678970",
            "+5355559124",
            "12345678971",
            "+5355559125",
            now,
        );
        let product_id = insert_stocked_product(
            &mut connection,
            now,
            supplier_id,
            "Tubo PVC",
            200,
            0,
            "10",
            "2,00",
        );
        let service = super::create_service_invoice_inner(
            &mut connection,
            now,
            service_input(client_id, vec![service_line(product_id, "1")], None),
        )
        .unwrap();

        let result = super::create_installation_invoice_inner(
            &mut connection,
            now,
            installation_input(
                client_id,
                service.invoice_id,
                vec![installation_line(product_id, "3")],
                None,
            ),
        )
        .unwrap();
        assert_eq!(result.number, "GI-00001");

        let header: (String, String, i64, String, String, Option<String>) = connection
            .query_row(
                "SELECT type, status, associatedInvoiceId, associatedNumber, associatedType,
                    paymentMethod FROM invoices WHERE id = ?1",
                [result.invoice_id],
                |row| {
                    Ok((
                        row.get(0)?,
                        row.get(1)?,
                        row.get(2)?,
                        row.get(3)?,
                        row.get(4)?,
                        row.get(5)?,
                    ))
                },
            )
            .unwrap();
        assert_eq!(
            header,
            (
                "GI".to_owned(),
                "pending".to_owned(),
                service.invoice_id,
                "S-00001".to_owned(),
                "S".to_owned(),
                None,
            )
        );

        let line: (i64, i64, i64) = connection
            .query_row(
                "SELECT quantityMilli, unitPriceCents, subtotalCents
                 FROM invoiceLines WHERE invoiceId = ?1",
                [result.invoice_id],
                |row| Ok((row.get(0)?, row.get(1)?, row.get(2)?)),
            )
            .unwrap();
        assert_eq!(line, (3_000, 200, 600));

        let movement: (String, i64, i64, i64, i64, Option<String>) = connection
            .query_row(
                "SELECT type, quantityMilli, stockAfterMilli, unitPriceCents, totalPriceCents, invoiceNumber
                 FROM productMovements WHERE invoiceId = ?1",
                [result.invoice_id],
                |row| {
                    Ok((
                        row.get(0)?,
                        row.get(1)?,
                        row.get(2)?,
                        row.get(3)?,
                        row.get(4)?,
                        row.get(5)?,
                    ))
                },
            )
            .unwrap();
        assert_eq!(
            movement,
            (
                "supplyOut".to_owned(),
                -3_000,
                6_000,
                200,
                600,
                Some("GI-00001".to_owned())
            )
        );

        let next_number: i64 = connection
            .query_row(
                "SELECT nextNumber FROM invoiceCounters WHERE type = 'GI'",
                [],
                |row| row.get(0),
            )
            .unwrap();
        assert_eq!(next_number, 2);
    }

    #[test]
    fn create_installation_invoice_requires_service_association() {
        let now = Utc.with_ymd_and_hms(2026, 10, 4, 12, 0, 0).unwrap();
        let (mut connection, _, client_id, supplier_id) = setup_sale_fixture(
            "installationassoc",
            "12345678972",
            "+5355559126",
            "12345678973",
            "+5355559127",
            now,
        );
        let product_id = insert_stocked_product(
            &mut connection,
            now,
            supplier_id,
            "Codo PVC",
            200,
            0,
            "10",
            "2,00",
        );
        let sale = super::create_sale_invoice_inner(
            &mut connection,
            now,
            sale_input(client_id, vec![sale_line(product_id, "1")], None),
        )
        .unwrap();
        let service = super::create_service_invoice_inner(
            &mut connection,
            now,
            service_input(client_id, vec![service_line(product_id, "1")], None),
        )
        .unwrap();
        connection
            .execute(
                "INSERT INTO clients (firstName, lastName, nationalId, phone, createdAt, updatedAt)
                 VALUES ('Marta', 'Ruiz', '12345678974', '+5355559128', ?1, ?1)",
                [format_utc_timestamp(now)],
            )
            .unwrap();
        let other_client_id = connection.last_insert_rowid();
        let other_service = super::create_service_invoice_inner(
            &mut connection,
            now,
            service_input(other_client_id, vec![service_line(product_id, "1")], None),
        )
        .unwrap();
        super::change_invoice_status_inner(
            &mut connection,
            now,
            service.invoice_id,
            "cancelled",
            None,
        )
        .unwrap();

        // Una venta no sirve como asociada (solo un servicio).
        let error = super::create_installation_invoice_inner(
            &mut connection,
            now,
            installation_input(
                client_id,
                sale.invoice_id,
                vec![installation_line(product_id, "1")],
                None,
            ),
        )
        .unwrap_err();
        assert_eq!(error.code, "validationFailed");
        assert_eq!(
            error.message,
            "La factura de servicio debe ser de este cliente y no estar cancelada"
        );

        // De otro cliente.
        let error = super::create_installation_invoice_inner(
            &mut connection,
            now,
            installation_input(
                client_id,
                other_service.invoice_id,
                vec![installation_line(product_id, "1")],
                None,
            ),
        )
        .unwrap_err();
        assert_eq!(error.code, "validationFailed");
        assert_eq!(
            error.message,
            "La factura de servicio debe ser de este cliente y no estar cancelada"
        );

        // Cancelada.
        let error = super::create_installation_invoice_inner(
            &mut connection,
            now,
            installation_input(
                client_id,
                service.invoice_id,
                vec![installation_line(product_id, "1")],
                None,
            ),
        )
        .unwrap_err();
        assert_eq!(error.code, "validationFailed");
        assert_eq!(
            error.message,
            "La factura de servicio debe ser de este cliente y no estar cancelada"
        );

        // Inexistente.
        let error = super::create_installation_invoice_inner(
            &mut connection,
            now,
            installation_input(
                client_id,
                9_999,
                vec![installation_line(product_id, "1")],
                None,
            ),
        )
        .unwrap_err();
        assert_eq!(error.code, "notFound");
        assert_eq!(error.message, "La factura ya no existe");
    }

    #[test]
    fn create_installation_invoice_requires_session_and_allows_both_roles() {
        assert_requires_session(|connection, now| {
            super::create_installation_invoice_inner(
                connection,
                now,
                installation_input(1, 1, Vec::new(), None),
            )
        });
        assert_allows_both_roles(|connection, now| {
            connection
                .execute(
                    "INSERT INTO clients (firstName, lastName, nationalId, phone, createdAt, updatedAt)
                     VALUES ('Ana', 'Pérez', '12345678975', '+5355559129', ?1, ?1)",
                    [format_utc_timestamp(now)],
                )
                .unwrap();
            let client_id = connection.last_insert_rowid();
            super::create_installation_invoice_inner(
                connection,
                now,
                installation_input(client_id, 999, vec![installation_line(999, "1")], None),
            )
        });
    }

    fn loss_input(
        product_id: i64,
        quantity: &str,
    ) -> crate::models::invoice::CreateLossInvoiceInput {
        crate::models::invoice::CreateLossInvoiceInput {
            product_id,
            quantity: quantity.to_owned(),
            invoice_date: "2026-10-04".to_owned(),
            notes: None,
        }
    }

    #[test]
    fn create_loss_invoice_creates_valid_loss_at_fifo_cost() {
        let now = Utc.with_ymd_and_hms(2026, 10, 4, 12, 0, 0).unwrap();
        let (mut connection, _, _, supplier_id) = setup_sale_fixture(
            "lossvalid",
            "12345678980",
            "+5355559130",
            "12345678981",
            "+5355559131",
            now,
        );
        let product_id = insert_stocked_product(
            &mut connection,
            now,
            supplier_id,
            "Huevos",
            100,
            0,
            "10",
            "1,00",
        );

        let result =
            super::create_loss_invoice_inner(&mut connection, now, loss_input(product_id, "3"))
                .unwrap();
        assert_eq!(result.number, "P-00001");

        // Sin estados, sin contacto y sin pago: todo en NULL.
        let header: (
            String,
            Option<String>,
            Option<String>,
            Option<String>,
            Option<String>,
        ) = connection
            .query_row(
                "SELECT type, status, contactFullName, paymentMethod, paidAt
                     FROM invoices WHERE id = ?1",
                [result.invoice_id],
                |row| {
                    Ok((
                        row.get(0)?,
                        row.get(1)?,
                        row.get(2)?,
                        row.get(3)?,
                        row.get(4)?,
                    ))
                },
            )
            .unwrap();
        assert_eq!(header, ("P".to_owned(), None, None, None, None,));

        // Costo real FIFO: 3 unidades a $1,00.
        let line: (i64, i64, i64) = connection
            .query_row(
                "SELECT quantityMilli, unitPriceCents, subtotalCents
                 FROM invoiceLines WHERE invoiceId = ?1",
                [result.invoice_id],
                |row| Ok((row.get(0)?, row.get(1)?, row.get(2)?)),
            )
            .unwrap();
        assert_eq!(line, (3_000, 100, 300));

        let movement: (String, i64, i64, i64, i64, Option<String>) = connection
            .query_row(
                "SELECT type, quantityMilli, stockAfterMilli, unitPriceCents, totalPriceCents, invoiceNumber
                 FROM productMovements WHERE invoiceId = ?1",
                [result.invoice_id],
                |row| {
                    Ok((
                        row.get(0)?,
                        row.get(1)?,
                        row.get(2)?,
                        row.get(3)?,
                        row.get(4)?,
                        row.get(5)?,
                    ))
                },
            )
            .unwrap();
        assert_eq!(
            movement,
            (
                "loss".to_owned(),
                -3_000,
                7_000,
                100,
                300,
                Some("P-00001".to_owned())
            )
        );

        let next_number: i64 = connection
            .query_row(
                "SELECT nextNumber FROM invoiceCounters WHERE type = 'P'",
                [],
                |row| row.get(0),
            )
            .unwrap();
        assert_eq!(next_number, 2);
    }

    #[test]
    fn create_loss_invoice_allows_discontinued_with_stock_but_rejects_without_it() {
        let now = Utc.with_ymd_and_hms(2026, 10, 4, 12, 0, 0).unwrap();
        let (mut connection, _, _, supplier_id) = setup_sale_fixture(
            "lossdiscontinued",
            "12345678982",
            "+5355559132",
            "12345678983",
            "+5355559133",
            now,
        );
        // Descontinuado con stock: se abastece en activo y luego se descontinúa.
        let stocked_id = insert_stocked_product(
            &mut connection,
            now,
            supplier_id,
            "Queso viejo",
            100,
            0,
            "10",
            "1,00",
        );
        connection
            .execute(
                "UPDATE products SET status = 'discontinued',
                    discontinuedAt = '2026-10-04T12:00:00Z' WHERE id = ?1",
                [stocked_id],
            )
            .unwrap();
        let result =
            super::create_loss_invoice_inner(&mut connection, now, loss_input(stocked_id, "2"))
                .unwrap();
        assert_eq!(result.number, "P-00001");

        // Descontinuado sin stock: se rechaza.
        let empty_id = insert_product_fixture(
            &connection,
            "Queso vacío",
            None,
            None,
            100,
            0,
            0,
            "discontinued",
            Some("2026-10-04T12:00:00Z"),
            None,
        );
        let error =
            super::create_loss_invoice_inner(&mut connection, now, loss_input(empty_id, "1"))
                .unwrap_err();
        assert_eq!(error.code, "validationFailed");
    }

    #[test]
    fn create_loss_invoice_rejects_quantity_above_stock() {
        let now = Utc.with_ymd_and_hms(2026, 10, 4, 12, 0, 0).unwrap();
        let (mut connection, _, _, supplier_id) = setup_sale_fixture(
            "lossstock",
            "12345678984",
            "+5355559134",
            "12345678985",
            "+5355559135",
            now,
        );
        let product_id = insert_stocked_product(
            &mut connection,
            now,
            supplier_id,
            "Leche en polvo",
            100,
            0,
            "3",
            "1,00",
        );

        let error =
            super::create_loss_invoice_inner(&mut connection, now, loss_input(product_id, "5"))
                .unwrap_err();
        assert_eq!(error.code, "stockInsuficiente");
        assert_eq!(
            error.message,
            "No hay stock suficiente de «Leche en polvo» (disponible: 3)"
        );
    }

    #[test]
    fn create_loss_invoice_requires_session_and_allows_both_roles() {
        assert_requires_session(|connection, now| {
            super::create_loss_invoice_inner(connection, now, loss_input(1, "1"))
        });
        assert_allows_both_roles(|connection, now| {
            super::create_loss_invoice_inner(connection, now, loss_input(999, "1"))
        });
    }

    fn update_outgoing_line(
        original: Option<i64>,
        product: Option<i64>,
        quantity: &str,
    ) -> crate::models::invoice::UpdateOutgoingInvoiceLineInput {
        crate::models::invoice::UpdateOutgoingInvoiceLineInput {
            product_id_original: original,
            product_id: product,
            quantity: quantity.to_owned(),
        }
    }

    fn update_sale_input(
        invoice_id: i64,
        client_id: i64,
        lines: Vec<crate::models::invoice::UpdateOutgoingInvoiceLineInput>,
    ) -> crate::models::invoice::UpdateSaleInvoiceInput {
        crate::models::invoice::UpdateSaleInvoiceInput {
            reserved_until: None,
            invoice_id,
            client_id,
            invoice_date: "2026-10-04".to_owned(),
            notes: None,
            lines,
        }
    }

    fn update_service_input(
        invoice_id: i64,
        client_id: i64,
        lines: Vec<crate::models::invoice::UpdateOutgoingInvoiceLineInput>,
    ) -> crate::models::invoice::UpdateServiceInvoiceInput {
        crate::models::invoice::UpdateServiceInvoiceInput {
            reserved_until: None,
            invoice_id,
            client_id,
            invoice_date: "2026-10-04".to_owned(),
            notes: None,
            lines,
        }
    }

    fn update_guarantee_input(
        invoice_id: i64,
        client_id: i64,
        associated_invoice_id: i64,
        lines: Vec<crate::models::invoice::UpdateOutgoingInvoiceLineInput>,
    ) -> crate::models::invoice::UpdateGuaranteeInvoiceInput {
        crate::models::invoice::UpdateGuaranteeInvoiceInput {
            invoice_id,
            client_id,
            associated_invoice_id,
            invoice_date: "2026-10-04".to_owned(),
            notes: None,
            lines,
        }
    }

    fn update_installation_input(
        invoice_id: i64,
        client_id: i64,
        associated_invoice_id: i64,
        lines: Vec<crate::models::invoice::UpdateOutgoingInvoiceLineInput>,
    ) -> crate::models::invoice::UpdateInstallationInvoiceInput {
        crate::models::invoice::UpdateInstallationInvoiceInput {
            invoice_id,
            client_id,
            associated_invoice_id,
            invoice_date: "2026-10-04".to_owned(),
            notes: None,
            lines,
        }
    }

    /// Venta con dos líneas abastecidas (10 u a $1,00 cada una) para editar.
    fn insert_sale_for_update(
        connection: &mut rusqlite::Connection,
        now: chrono::DateTime<Utc>,
        supplier_id: i64,
        client_id: i64,
    ) -> (i64, i64, i64, i64) {
        let product_a = insert_product_fixture(
            connection,
            "Producto A",
            None,
            None,
            100,
            0,
            0,
            "active",
            None,
            None,
        );
        let product_b = insert_product_fixture(
            connection,
            "Producto B",
            None,
            None,
            100,
            0,
            0,
            "active",
            None,
            None,
        );
        for product_id in [product_a, product_b] {
            super::create_entry_invoice_inner(
                connection,
                now,
                entry_input(
                    supplier_id,
                    vec![entry_line(Some(product_id), None, "10", "1,00")],
                    None,
                ),
            )
            .unwrap();
        }
        let sale = super::create_sale_invoice_inner(
            connection,
            now,
            sale_input(
                client_id,
                vec![sale_line(product_a, "2"), sale_line(product_b, "1")],
                None,
            ),
        )
        .unwrap();
        (sale.invoice_id, product_a, product_b, client_id)
    }

    #[test]
    fn update_sale_invoice_edits_pending_with_adjustment_reversal_and_new_sale() {
        let now = Utc.with_ymd_and_hms(2026, 10, 4, 12, 0, 0).unwrap();
        let (mut connection, _, client_id, supplier_id) = setup_sale_fixture(
            "updatesale",
            "12345678980",
            "+5355559136",
            "12345678981",
            "+5355559137",
            now,
        );
        let (sale_id, product_a, product_b, _) =
            insert_sale_for_update(&mut connection, now, supplier_id, client_id);
        let product_c = insert_stocked_product(
            &mut connection,
            now,
            supplier_id,
            "Producto C",
            100,
            0,
            "10",
            "1,00",
        );

        // A: 2→3 (Ajuste −1), B: se quita (Reversión +1), C: nueva (Salida −1).
        // Las líneas quitadas se procesan antes que las conservadas.
        super::update_sale_invoice_inner(
            &mut connection,
            now,
            update_sale_input(
                sale_id,
                client_id,
                vec![
                    update_outgoing_line(Some(product_a), Some(product_a), "3"),
                    update_outgoing_line(None, Some(product_c), "1"),
                ],
            ),
        )
        .unwrap();

        let movements: Vec<(String, i64)> = connection
            .prepare(
                "SELECT type, quantityMilli FROM productMovements
                     WHERE invoiceId = ?1 ORDER BY id",
            )
            .unwrap()
            .query_map([sale_id], |row| Ok((row.get(0)?, row.get(1)?)))
            .unwrap()
            .collect::<rusqlite::Result<Vec<_>>>()
            .unwrap();
        assert_eq!(
            movements,
            vec![
                ("saleOut".to_owned(), -2_000),
                ("saleOut".to_owned(), -1_000),
                ("reversal".to_owned(), 1_000),
                ("adjustment".to_owned(), -1_000),
                ("saleOut".to_owned(), -1_000),
            ]
        );

        // La línea existente conserva el precio emitido.
        let line_a: (i64, i64, i64) = connection
            .query_row(
                "SELECT quantityMilli, unitPriceCents, subtotalCents FROM invoiceLines
                     WHERE invoiceId = ?1 AND productIdOriginal = ?2",
                (sale_id, product_a),
                |row| Ok((row.get(0)?, row.get(1)?, row.get(2)?)),
            )
            .unwrap();
        assert_eq!(line_a, (3_000, 100, 300));
        let remaining_b: i64 = connection
            .query_row(
                "SELECT COUNT(*) FROM invoiceLines WHERE invoiceId = ?1 AND productIdOriginal = ?2",
                (sale_id, product_b),
                |row| row.get(0),
            )
            .unwrap();
        assert_eq!(remaining_b, 0);

        // Stock: A 7, B 10 (devuelto), C 9.
        for (product_id, expected) in [(product_a, 7_000), (product_b, 10_000), (product_c, 9_000)]
        {
            let stock: i64 = connection
                .query_row(
                    "SELECT quantityMilli FROM products WHERE id = ?1",
                    [product_id],
                    |row| row.get(0),
                )
                .unwrap();
            assert_eq!(stock, expected);
        }
    }

    #[test]
    fn update_sale_invoice_rejects_completed_invoice() {
        let now = Utc.with_ymd_and_hms(2026, 10, 4, 12, 0, 0).unwrap();
        let (mut connection, _, client_id, supplier_id) = setup_sale_fixture(
            "updatesalecompleted",
            "12345678982",
            "+5355559138",
            "12345678983",
            "+5355559139",
            now,
        );
        let (sale_id, product_a, _, _) =
            insert_sale_for_update(&mut connection, now, supplier_id, client_id);
        super::change_invoice_status_inner(&mut connection, now, sale_id, "completed", None)
            .unwrap();

        let error = super::update_sale_invoice_inner(
            &mut connection,
            now,
            update_sale_input(
                sale_id,
                client_id,
                vec![update_outgoing_line(Some(product_a), Some(product_a), "2")],
            ),
        )
        .unwrap_err();
        assert_eq!(error.code, "invalidState");
        assert_eq!(
            error.message,
            "Solo se pueden editar las facturas que están Por Cobrar"
        );
    }

    #[test]
    fn update_sale_invoice_rejects_insufficient_stock_after_edit() {
        let now = Utc.with_ymd_and_hms(2026, 10, 4, 12, 0, 0).unwrap();
        let (mut connection, _, client_id, supplier_id) = setup_sale_fixture(
            "updatesalestock",
            "12345678984",
            "+5355559140",
            "12345678985",
            "+5355559141",
            now,
        );
        let product_id = insert_stocked_product(
            &mut connection,
            now,
            supplier_id,
            "Azúcar morena",
            100,
            0,
            "10",
            "1,00",
        );
        let sale = super::create_sale_invoice_inner(
            &mut connection,
            now,
            sale_input(client_id, vec![sale_line(product_id, "8")], None),
        )
        .unwrap();

        let error = super::update_sale_invoice_inner(
            &mut connection,
            now,
            update_sale_input(
                sale.invoice_id,
                client_id,
                vec![update_outgoing_line(
                    Some(product_id),
                    Some(product_id),
                    "12",
                )],
            ),
        )
        .unwrap_err();
        assert_eq!(error.code, "stockInsuficiente");
        // Nada se guarda: fecha, línea y stock intactos.
        let date: String = connection
            .query_row(
                "SELECT invoiceDate FROM invoices WHERE id = ?1",
                [sale.invoice_id],
                |row| row.get(0),
            )
            .unwrap();
        assert_eq!(date, "2026-10-04");
        let stock: i64 = connection
            .query_row(
                "SELECT quantityMilli FROM products WHERE id = ?1",
                [product_id],
                |row| row.get(0),
            )
            .unwrap();
        assert_eq!(stock, 2_000);
    }

    /// Invariante de caché (plan-03 §3.2): `quantityMilli` = suma de lo restante
    /// en los lotes y `costCents` = promedio ponderado de los lotes con
    /// existencia. Con stock 0 el producto conserva su último costo, así que
    /// ese caso no se puede comprobar contra los lotes.
    fn assert_cache_invariant(connection: &rusqlite::Connection, step: &str) {
        let rows: Vec<(i64, i64, i64, i64, i64)> = connection
            .prepare(
                "SELECT p.id, p.quantityMilli, p.costCents,
                    COALESCE(SUM(l.remainingMilli), 0),
                    COALESCE(SUM(l.remainingMilli * l.unitCostCents), 0)
                 FROM products p
                 LEFT JOIN lots l ON l.productId = p.id AND l.remainingMilli > 0
                 GROUP BY p.id",
            )
            .unwrap()
            .query_map([], |row| {
                Ok((
                    row.get(0)?,
                    row.get(1)?,
                    row.get(2)?,
                    row.get(3)?,
                    row.get(4)?,
                ))
            })
            .unwrap()
            .collect::<rusqlite::Result<Vec<_>>>()
            .unwrap();
        for (product_id, quantity, cost, remaining, weighted) in rows {
            assert_eq!(
                quantity, remaining,
                "{step}: quantityMilli del producto {product_id}"
            );
            if remaining > 0 {
                let expected =
                    crate::inventory::round_half_up(i128::from(weighted), i128::from(remaining))
                        as i64;
                assert_eq!(
                    cost, expected,
                    "{step}: costCents del producto {product_id}"
                );
            }
        }
    }

    #[test]
    fn change_invoice_status_rejects_loss_with_clear_message() {
        // FP-01: la pérdida no tiene estados; antes acababa en `internal`
        // porque el estado NULL no se podía leer como texto.
        let now = Utc.with_ymd_and_hms(2026, 10, 4, 12, 0, 0).unwrap();
        let (mut connection, _, _, supplier_id) = setup_sale_fixture(
            "lossstatus",
            "12345679000",
            "+5355559156",
            "12345679001",
            "+5355559157",
            now,
        );
        let product = insert_stocked_product(
            &mut connection,
            now,
            supplier_id,
            "Pérdida sin estado",
            100,
            2_000,
            "5",
            "1,00",
        );
        let loss = super::create_loss_invoice_inner(&mut connection, now, loss_input(product, "1"))
            .unwrap();
        for status in ["pending", "completed", "cancelled"] {
            let error = super::change_invoice_status_inner(
                &mut connection,
                now,
                loss.invoice_id,
                status,
                None,
            )
            .unwrap_err();
            assert_eq!(error.code, "invalidState");
            assert_eq!(error.message, "Las facturas de pérdida no tienen estado");
        }
        let stock: i64 = connection
            .query_row(
                "SELECT quantityMilli FROM products WHERE id = ?1",
                [product],
                |row| row.get(0),
            )
            .unwrap();
        assert_eq!(stock, 4_000);
    }

    #[test]
    fn invoice_writes_keep_working_and_warn_when_the_purge_fails() {
        // §6 / spec-03 §7.6: con la purga forzada a fallar, cada comando de
        // escritura de facturas termina bien, devuelve el aviso y registra el
        // error. El producto vencido sigue ahí (la purga se deshizo).
        let now = Utc.with_ymd_and_hms(2026, 10, 4, 12, 0, 0).unwrap();
        let (mut connection, _, client_id, supplier_id) = setup_sale_fixture(
            "purgewrites",
            "12345679002",
            "+5355559158",
            "12345679003",
            "+5355559159",
            now,
        );
        let product = insert_stocked_product(
            &mut connection,
            now,
            supplier_id,
            "Purga activo",
            100,
            2_000,
            "50",
            "1,00",
        );
        let expired = insert_product_fixture(
            &connection,
            "Purga vencido",
            None,
            None,
            100,
            2_000,
            0,
            "discontinued",
            Some("2020-01-01T00:00:00Z"),
            None,
        );
        crate::test_utils::create_fail_product_delete_trigger(&connection).unwrap();
        let logged = |connection: &rusqlite::Connection| -> i64 {
            connection
                .query_row("SELECT COUNT(*) FROM errorLogs", [], |row| row.get(0))
                .unwrap()
        };
        fn check(
            connection: &rusqlite::Connection,
            step: &str,
            result: Result<crate::models::invoice::InvoiceActionResult, crate::errors::AppError>,
            before: i64,
        ) -> crate::models::invoice::InvoiceActionResult {
            let result = result.unwrap_or_else(|error| panic!("{step}: {error:?}"));
            assert!(
                result
                    .expired_cleanup_warning
                    .as_deref()
                    .is_some_and(|warning| warning.contains("vencidos")),
                "{step}: {:?}",
                result.expired_cleanup_warning
            );
            let after: i64 = connection
                .query_row("SELECT COUNT(*) FROM errorLogs", [], |row| row.get(0))
                .unwrap();
            assert!(after > before, "{step}: no registró el error");
            result
        }
        // Ejecuta el comando y después comprueba resultado, aviso y registro.
        macro_rules! run {
            ($step:expr, $call:expr) => {{
                let before = logged(&connection);
                let result = $call;
                check(&connection, $step, result, before)
            }};
        }

        let entry = run!(
            "createEntryInvoice",
            super::create_entry_invoice_inner(
                &mut connection,
                now,
                entry_input(
                    supplier_id,
                    vec![entry_line(Some(product), None, "5", "1,00")],
                    None,
                ),
            )
        );
        let sale = run!(
            "createSaleInvoice",
            super::create_sale_invoice_inner(
                &mut connection,
                now,
                sale_input(client_id, vec![sale_line(product, "2")], None),
            )
        );
        let service = run!(
            "createServiceInvoice",
            super::create_service_invoice_inner(
                &mut connection,
                now,
                service_input(client_id, vec![service_line(product, "2")], None),
            )
        );
        let guarantee = run!(
            "createGuaranteeInvoice",
            super::create_guarantee_invoice_inner(
                &mut connection,
                now,
                guarantee_input(
                    client_id,
                    sale.invoice_id,
                    vec![guarantee_line(product, "1")],
                    None,
                ),
            )
        );
        let installation = run!(
            "createInstallationInvoice",
            super::create_installation_invoice_inner(
                &mut connection,
                now,
                installation_input(
                    client_id,
                    service.invoice_id,
                    vec![installation_line(product, "1")],
                    None,
                ),
            )
        );
        let loss = run!(
            "createLossInvoice",
            super::create_loss_invoice_inner(&mut connection, now, loss_input(product, "1"))
        );
        run!(
            "updateEntryInvoice",
            super::update_entry_invoice_inner(
                &mut connection,
                now,
                update_entry_input(
                    entry.invoice_id,
                    supplier_id,
                    "2026-10-04",
                    vec![update_entry_line(Some(product), product, "6", "1,00")],
                ),
            )
        );
        run!(
            "updateSaleInvoice",
            super::update_sale_invoice_inner(
                &mut connection,
                now,
                update_sale_input(
                    sale.invoice_id,
                    client_id,
                    vec![update_outgoing_line(Some(product), Some(product), "3")],
                ),
            )
        );
        run!(
            "updateServiceInvoice",
            super::update_service_invoice_inner(
                &mut connection,
                now,
                update_service_input(
                    service.invoice_id,
                    client_id,
                    vec![update_outgoing_line(Some(product), Some(product), "3")],
                ),
            )
        );
        run!(
            "updateGuaranteeInvoice",
            super::update_guarantee_invoice_inner(
                &mut connection,
                now,
                update_guarantee_input(
                    guarantee.invoice_id,
                    client_id,
                    sale.invoice_id,
                    vec![update_outgoing_line(Some(product), Some(product), "2")],
                ),
            )
        );
        run!(
            "updateInstallationInvoice",
            super::update_installation_invoice_inner(
                &mut connection,
                now,
                update_installation_input(
                    installation.invoice_id,
                    client_id,
                    service.invoice_id,
                    vec![update_outgoing_line(Some(product), Some(product), "2")],
                ),
            )
        );
        run!(
            "changeInvoiceStatus",
            super::change_invoice_status_inner(
                &mut connection,
                now,
                sale.invoice_id,
                "completed",
                None,
            )
        );
        run!(
            "deleteInvoice",
            super::delete_invoice_inner(&mut connection, now, loss.invoice_id)
        );

        // La purga se deshizo en cada intento: el vencido no se borró.
        let still_there: i64 = connection
            .query_row(
                "SELECT COUNT(*) FROM products WHERE id = ?1",
                [expired],
                |row| row.get(0),
            )
            .unwrap();
        assert_eq!(still_there, 1);
        assert_cache_invariant(&connection, "tras la batería con purga fallida");
    }

    #[test]
    fn invoice_writes_return_no_cleanup_warning_when_the_purge_works() {
        let now = Utc.with_ymd_and_hms(2026, 10, 4, 12, 0, 0).unwrap();
        let (mut connection, _, client_id, supplier_id) = setup_sale_fixture(
            "purgeok",
            "12345679004",
            "+5355559160",
            "12345679005",
            "+5355559161",
            now,
        );
        let product = insert_stocked_product(
            &mut connection,
            now,
            supplier_id,
            "Purga normal",
            100,
            2_000,
            "5",
            "1,00",
        );
        let expired = insert_product_fixture(
            &connection,
            "Vencido purgable",
            None,
            None,
            100,
            2_000,
            0,
            "discontinued",
            Some("2020-01-01T00:00:00Z"),
            None,
        );
        let sale = super::create_sale_invoice_inner(
            &mut connection,
            now,
            sale_input(client_id, vec![sale_line(product, "1")], None),
        )
        .unwrap();
        assert_eq!(sale.expired_cleanup_warning, None);
        // La purga se ejecutó: el vencido ya no existe.
        let remaining: i64 = connection
            .query_row(
                "SELECT COUNT(*) FROM products WHERE id = ?1",
                [expired],
                |row| row.get(0),
            )
            .unwrap();
        assert_eq!(remaining, 0);
    }

    #[test]
    fn cache_invariant_holds_after_every_phase3_command() {
        // §4.8: el invariante se cumple tras cada comando de salidas.
        let now = Utc.with_ymd_and_hms(2026, 10, 4, 12, 0, 0).unwrap();
        let (mut connection, _, client_id, supplier_id) = setup_sale_fixture(
            "invariantuser",
            "12345678994",
            "+5355559150",
            "12345678995",
            "+5355559151",
            now,
        );
        let product_a = insert_stocked_product(
            &mut connection,
            now,
            supplier_id,
            "Invariante A",
            100,
            2_000,
            "10",
            "1,00",
        );
        let product_b = insert_stocked_product(
            &mut connection,
            now,
            supplier_id,
            "Invariante B",
            300,
            2_000,
            "8",
            "3,00",
        );
        // Segundo lote de A con otro costo para que el promedio importe.
        super::create_entry_invoice_inner(
            &mut connection,
            now,
            entry_input(
                supplier_id,
                vec![entry_line(Some(product_a), None, "6", "2,50")],
                None,
            ),
        )
        .unwrap();
        assert_cache_invariant(&connection, "entradas iniciales");

        let sale = super::create_sale_invoice_inner(
            &mut connection,
            now,
            sale_input(
                client_id,
                vec![sale_line(product_a, "12"), sale_line(product_b, "1")],
                None,
            ),
        )
        .unwrap();
        assert_cache_invariant(&connection, "createSaleInvoice");

        let service = super::create_service_invoice_inner(
            &mut connection,
            now,
            service_input(client_id, vec![service_line(product_b, "2")], None),
        )
        .unwrap();
        assert_cache_invariant(&connection, "createServiceInvoice");

        let guarantee = super::create_guarantee_invoice_inner(
            &mut connection,
            now,
            guarantee_input(
                client_id,
                sale.invoice_id,
                vec![guarantee_line(product_a, "1")],
                None,
            ),
        )
        .unwrap();
        assert_cache_invariant(&connection, "createGuaranteeInvoice");

        let installation = super::create_installation_invoice_inner(
            &mut connection,
            now,
            installation_input(
                client_id,
                service.invoice_id,
                vec![installation_line(product_b, "1")],
                None,
            ),
        )
        .unwrap();
        assert_cache_invariant(&connection, "createInstallationInvoice");

        let loss =
            super::create_loss_invoice_inner(&mut connection, now, loss_input(product_a, "0.5"))
                .unwrap();
        assert_cache_invariant(&connection, "createLossInvoice");

        super::update_sale_invoice_inner(
            &mut connection,
            now,
            update_sale_input(
                sale.invoice_id,
                client_id,
                vec![update_outgoing_line(Some(product_a), Some(product_a), "9")],
            ),
        )
        .unwrap();
        assert_cache_invariant(&connection, "updateSaleInvoice");

        super::update_service_invoice_inner(
            &mut connection,
            now,
            update_service_input(
                service.invoice_id,
                client_id,
                vec![
                    update_outgoing_line(Some(product_b), Some(product_b), "3"),
                    update_outgoing_line(None, Some(product_a), "1"),
                ],
            ),
        )
        .unwrap();
        assert_cache_invariant(&connection, "updateServiceInvoice");

        super::update_guarantee_invoice_inner(
            &mut connection,
            now,
            update_guarantee_input(
                guarantee.invoice_id,
                client_id,
                sale.invoice_id,
                vec![update_outgoing_line(Some(product_a), Some(product_a), "2")],
            ),
        )
        .unwrap();
        assert_cache_invariant(&connection, "updateGuaranteeInvoice");

        super::update_installation_invoice_inner(
            &mut connection,
            now,
            update_installation_input(
                installation.invoice_id,
                client_id,
                service.invoice_id,
                vec![update_outgoing_line(Some(product_b), Some(product_b), "2")],
            ),
        )
        .unwrap();
        assert_cache_invariant(&connection, "updateInstallationInvoice");

        super::change_invoice_status_inner(
            &mut connection,
            now,
            guarantee.invoice_id,
            "cancelled",
            None,
        )
        .unwrap();
        assert_cache_invariant(&connection, "changeInvoiceStatus cancelled");

        for (invoice_id, step) in [
            (guarantee.invoice_id, "deleteInvoice cancelada"),
            (installation.invoice_id, "deleteInvoice pendiente"),
            (loss.invoice_id, "deleteInvoice pérdida"),
            (service.invoice_id, "deleteInvoice servicio"),
            (sale.invoice_id, "deleteInvoice venta"),
        ] {
            super::delete_invoice_inner(&mut connection, now, invoice_id).unwrap();
            assert_cache_invariant(&connection, step);
        }

        // Sin salidas, el stock vuelve al de las entradas: A 16, B 8.
        let stocks: Vec<i64> = [product_a, product_b]
            .iter()
            .map(|id| {
                connection
                    .query_row(
                        "SELECT quantityMilli FROM products WHERE id = ?1",
                        [id],
                        |row| row.get(0),
                    )
                    .unwrap()
            })
            .collect();
        assert_eq!(stocks, vec![16_000, 8_000]);
    }

    #[test]
    fn cache_invariant_holds_under_pseudorandom_phase3_operations() {
        // Varias semillas fijas: cada una es una secuencia reproducible distinta.
        for seed in [0x9E37_79B9_7F4A_7C15, 1, 42, 2026, 0xDEAD_BEEF] {
            run_pseudorandom_phase3_battery(seed);
        }
    }

    fn run_pseudorandom_phase3_battery(seed: u64) {
        // Batería pseudoaleatoria (plan-03 §8): xorshift con semilla fija para
        // que un fallo se pueda reproducir. Los rechazos esperados se admiten;
        // un error `internal` o una caché desincronizada, no.
        struct XorShift(u64);
        impl XorShift {
            fn next(&mut self) -> u64 {
                self.0 ^= self.0 << 13;
                self.0 ^= self.0 >> 7;
                self.0 ^= self.0 << 17;
                self.0
            }
            fn below(&mut self, n: usize) -> usize {
                (self.next() % n as u64) as usize
            }
        }

        let now = Utc.with_ymd_and_hms(2026, 10, 4, 12, 0, 0).unwrap();
        let (mut connection, _, client_id, supplier_id) = setup_sale_fixture(
            "randominvariant",
            "12345678996",
            "+5355559152",
            "12345678997",
            "+5355559153",
            now,
        );
        let products: Vec<i64> = [("Azar A", "1,00"), ("Azar B", "2,00"), ("Azar C", "3,50")]
            .iter()
            .map(|(name, cost)| {
                insert_stocked_product(
                    &mut connection,
                    now,
                    supplier_id,
                    name,
                    100,
                    2_000,
                    "20",
                    cost,
                )
            })
            .collect();
        let quantities = ["1", "2", "3", "0.5"];
        let allowed = [
            "stockInsuficiente",
            "invalidState",
            "validationFailed",
            "stockLimitExceeded",
            "notFound",
        ];
        // (id, tipo) de las facturas de salida vivas.
        let mut invoices: Vec<(i64, &str)> = Vec::new();
        let mut rng = XorShift(seed);
        let mut rejected = 0;

        for step in 0..300 {
            let product = products[rng.below(products.len())];
            let quantity = quantities[rng.below(quantities.len())];
            let op = rng.below(9);
            let result: Result<Option<(i64, &str)>, crate::errors::AppError> = match op {
                0 => super::create_sale_invoice_inner(
                    &mut connection,
                    now,
                    sale_input(client_id, vec![sale_line(product, quantity)], None),
                )
                .map(|r| Some((r.invoice_id, "V"))),
                1 => super::create_service_invoice_inner(
                    &mut connection,
                    now,
                    service_input(client_id, vec![service_line(product, quantity)], None),
                )
                .map(|r| Some((r.invoice_id, "S"))),
                2 => super::create_loss_invoice_inner(
                    &mut connection,
                    now,
                    loss_input(product, quantity),
                )
                .map(|r| Some((r.invoice_id, "P"))),
                3 | 4 if !invoices.is_empty() => {
                    let (invoice_id, _) = invoices[rng.below(invoices.len())];
                    if op == 3 {
                        super::change_invoice_status_inner(
                            &mut connection,
                            now,
                            invoice_id,
                            "cancelled",
                            None,
                        )
                        .map(|_| None)
                    } else {
                        let deleted = super::delete_invoice_inner(&mut connection, now, invoice_id)
                            .map(|_| None);
                        if deleted.is_ok() {
                            invoices.retain(|(id, _)| *id != invoice_id);
                        }
                        deleted
                    }
                }
                5 => match invoices
                    .iter()
                    .filter(|(_, kind)| *kind == "V")
                    .nth(rng.below(4))
                {
                    Some(&(invoice_id, _)) => super::update_sale_invoice_inner(
                        &mut connection,
                        now,
                        update_sale_input(
                            invoice_id,
                            client_id,
                            vec![update_outgoing_line(None, Some(product), quantity)],
                        ),
                    )
                    .map(|_| None),
                    None => Ok(None),
                },
                6 => match invoices.iter().find(|(_, kind)| matches!(*kind, "V" | "S")) {
                    Some(&(associated, _)) => super::create_guarantee_invoice_inner(
                        &mut connection,
                        now,
                        guarantee_input(
                            client_id,
                            associated,
                            vec![guarantee_line(product, quantity)],
                            None,
                        ),
                    )
                    .map(|r| Some((r.invoice_id, "GG"))),
                    None => Ok(None),
                },
                7 => match invoices.iter().find(|(_, kind)| *kind == "S") {
                    Some(&(associated, _)) => super::create_installation_invoice_inner(
                        &mut connection,
                        now,
                        installation_input(
                            client_id,
                            associated,
                            vec![installation_line(product, quantity)],
                            None,
                        ),
                    )
                    .map(|r| Some((r.invoice_id, "GI"))),
                    None => Ok(None),
                },
                _ => super::create_entry_invoice_inner(
                    &mut connection,
                    now,
                    entry_input(
                        supplier_id,
                        vec![entry_line(Some(product), None, "4", "1,75")],
                        None,
                    ),
                )
                .map(|_| None),
            };
            match result {
                Ok(Some(created)) => invoices.push(created),
                Ok(None) => {}
                Err(error) => {
                    assert!(
                        allowed.contains(&error.code.as_str()),
                        "semilla {seed}, paso {step}, op {op}: error inesperado {error:?}"
                    );
                    rejected += 1;
                }
            }
            assert_cache_invariant(
                &connection,
                &format!("semilla {seed}, paso {step}, op {op}"),
            );
        }
        // La batería ejercita tanto éxitos como rechazos.
        assert!(
            rejected > 0 && rejected < 300,
            "semilla {seed}: rechazos {rejected}"
        );
    }

    #[test]
    fn sp03_income_and_stock_value_follow_real_sales_and_category_snapshots() {
        // SP-03 con ventas reales: "Ingresos" suma los subtotales de ventas y
        // servicios no cancelados por la categoría guardada en la línea; un
        // cambio posterior de categoría no mueve las ventas anteriores.
        // "Valor Total" es lo restante de cada lote por su costo (FIFO).
        let now = Utc.with_ymd_and_hms(2026, 10, 4, 12, 0, 0).unwrap();
        let today = now.date_naive();
        let (mut connection, _, client_id, supplier_id) = setup_sale_fixture(
            "sp03user",
            "12345678992",
            "+5355559148",
            "12345678993",
            "+5355559149",
            now,
        );
        let drinks = crate::test_utils::insert_category_fixture(&connection, "Bebidas", None);
        let dairy = crate::test_utils::insert_category_fixture(&connection, "Lácteos", None);
        let product_id = insert_product_fixture(
            &connection,
            "Café SP03",
            Some(drinks),
            None,
            100,
            2_000,
            0,
            "active",
            None,
            None,
        );
        // Dos lotes: 10 a $1,00 y luego 5 a $2,00.
        for (quantity, cost) in [("10", "1,00"), ("5", "2,00")] {
            super::create_entry_invoice_inner(
                &mut connection,
                now,
                entry_input(
                    supplier_id,
                    vec![entry_line(Some(product_id), None, quantity, cost)],
                    None,
                ),
            )
            .unwrap();
        }

        let sale = super::create_sale_invoice_inner(
            &mut connection,
            now,
            sale_input(client_id, vec![sale_line(product_id, "2")], None),
        )
        .unwrap();
        let service = super::create_service_invoice_inner(
            &mut connection,
            now,
            service_input(client_id, vec![service_line(product_id, "1")], None),
        )
        .unwrap();
        let cancelled = super::create_sale_invoice_inner(
            &mut connection,
            now,
            sale_input(client_id, vec![sale_line(product_id, "4")], None),
        )
        .unwrap();
        super::change_invoice_status_inner(
            &mut connection,
            now,
            cancelled.invoice_id,
            "cancelled",
            None,
        )
        .unwrap();

        let subtotal = |connection: &rusqlite::Connection, invoice_id: i64| -> i64 {
            connection
                .query_row(
                    "SELECT subtotalCents FROM invoiceLines WHERE invoiceId = ?1",
                    [invoice_id],
                    |row| row.get(0),
                )
                .unwrap()
        };
        let expected_drinks =
            subtotal(&connection, sale.invoice_id) + subtotal(&connection, service.invoice_id);
        assert!(expected_drinks > 0);

        let income = |connection: &mut rusqlite::Connection, category_id: i64| -> (i64, i64) {
            let listed = crate::commands::categories::list_categories_inner(connection, now, None)
                .unwrap()
                .items
                .into_iter()
                .find(|item| item.id == category_id)
                .unwrap()
                .ingresos_cents;
            let detail =
                crate::commands::categories::get_category_inner(connection, now, category_id)
                    .unwrap()
                    .ingresos_cents;
            (listed, detail)
        };
        let stock_value = |connection: &mut rusqlite::Connection| -> (i64, i64) {
            let listed = crate::commands::products::list_products_inner(
                connection,
                now,
                today,
                crate::models::product::ProductFilters::default(),
            )
            .unwrap()
            .into_iter()
            .find(|item| item.id == product_id)
            .unwrap()
            .valor_total_cents;
            let detail = crate::commands::products::get_product_inner(connection, now, product_id)
                .unwrap()
                .product
                .valor_total_cents;
            (listed, detail)
        };

        // La venta cancelada no cuenta; listado y detalle coinciden.
        assert_eq!(
            income(&mut connection, drinks),
            (expected_drinks, expected_drinks)
        );
        assert_eq!(income(&mut connection, dairy), (0, 0));
        // Se consumieron 3 del primer lote (la cancelada devolvió sus 4):
        // 7 × $1,00 + 5 × $2,00 = $17,00.
        assert_eq!(stock_value(&mut connection), (1_700, 1_700));

        // El producto cambia de categoría: lo vendido sigue en "Bebidas".
        connection
            .execute(
                "UPDATE products SET categoryId = ?1 WHERE id = ?2",
                (dairy, product_id),
            )
            .unwrap();
        assert_eq!(
            income(&mut connection, drinks),
            (expected_drinks, expected_drinks)
        );
        assert_eq!(income(&mut connection, dairy), (0, 0));

        // Una venta nueva ya cuenta para la categoría actual.
        let later = super::create_sale_invoice_inner(
            &mut connection,
            now,
            sale_input(client_id, vec![sale_line(product_id, "1")], None),
        )
        .unwrap();
        let later_subtotal = subtotal(&connection, later.invoice_id);
        assert_eq!(
            income(&mut connection, drinks),
            (expected_drinks, expected_drinks)
        );
        assert_eq!(
            income(&mut connection, dairy),
            (later_subtotal, later_subtotal)
        );
        assert_eq!(stock_value(&mut connection), (1_600, 1_600));
    }

    #[test]
    fn update_sale_invoice_twice_does_not_restore_consumptions_twice() {
        // Cada edición devuelve los consumos de la anterior una sola vez:
        // dos ediciones sin cambios dejan el stock y los consumos intactos.
        let now = Utc.with_ymd_and_hms(2026, 10, 4, 12, 0, 0).unwrap();
        let (mut connection, _, client_id, supplier_id) = setup_sale_fixture(
            "updatesaletwice",
            "12345678990",
            "+5355559146",
            "12345678991",
            "+5355559147",
            now,
        );
        let product_id = insert_stocked_product(
            &mut connection,
            now,
            supplier_id,
            "Café doble",
            100,
            0,
            "10",
            "1,00",
        );
        let sale = super::create_sale_invoice_inner(
            &mut connection,
            now,
            sale_input(client_id, vec![sale_line(product_id, "2")], None),
        )
        .unwrap();
        for _ in 0..2 {
            super::update_sale_invoice_inner(
                &mut connection,
                now,
                update_sale_input(
                    sale.invoice_id,
                    client_id,
                    vec![update_outgoing_line(
                        Some(product_id),
                        Some(product_id),
                        "2",
                    )],
                ),
            )
            .unwrap();
        }
        let (stock, consumed): (i64, i64) = connection
            .query_row(
                "SELECT p.quantityMilli,
                    (SELECT COALESCE(SUM(c.quantityMilli), 0) FROM consumptions c
                     JOIN invoiceLines il ON il.id = c.invoiceLineId WHERE il.invoiceId = ?2)
                 FROM products p WHERE p.id = ?1",
                (product_id, sale.invoice_id),
                |row| Ok((row.get(0)?, row.get(1)?)),
            )
            .unwrap();
        assert_eq!((stock, consumed), (8_000, 2_000));
    }

    #[test]
    fn update_sale_invoice_keeps_discontinued_lines_only_unchanged() {
        let now = Utc.with_ymd_and_hms(2026, 10, 4, 12, 0, 0).unwrap();
        let (mut connection, _, client_id, supplier_id) = setup_sale_fixture(
            "updatesalediscontinued",
            "12345678986",
            "+5355559142",
            "12345678987",
            "+5355559143",
            now,
        );
        let product_id = insert_stocked_product(
            &mut connection,
            now,
            supplier_id,
            "Café viejo",
            100,
            0,
            "10",
            "1,00",
        );
        let sale = super::create_sale_invoice_inner(
            &mut connection,
            now,
            sale_input(client_id, vec![sale_line(product_id, "2")], None),
        )
        .unwrap();
        connection
            .execute(
                "UPDATE products SET status = 'discontinued',
                        discontinuedAt = '2026-10-04T12:00:00Z' WHERE id = ?1",
                [product_id],
            )
            .unwrap();

        // Conservar sin cambios: pasa.
        super::update_sale_invoice_inner(
            &mut connection,
            now,
            update_sale_input(
                sale.invoice_id,
                client_id,
                vec![update_outgoing_line(
                    Some(product_id),
                    Some(product_id),
                    "2",
                )],
            ),
        )
        .unwrap();

        // Aumentar: se rechaza.
        let error = super::update_sale_invoice_inner(
            &mut connection,
            now,
            update_sale_input(
                sale.invoice_id,
                client_id,
                vec![update_outgoing_line(
                    Some(product_id),
                    Some(product_id),
                    "3",
                )],
            ),
        )
        .unwrap_err();
        assert_eq!(error.code, "invalidState");
    }

    #[test]
    fn update_sale_invoice_refifo_without_quantity_change_records_no_adjustment() {
        let now = Utc.with_ymd_and_hms(2026, 10, 4, 12, 0, 0).unwrap();
        let (mut connection, _, client_id, supplier_id) = setup_sale_fixture(
            "updatehA4",
            "12345678988",
            "+5355559144",
            "12345678989",
            "+5355559145",
            now,
        );
        let product_id = insert_stocked_product(
            &mut connection,
            now,
            supplier_id,
            "Arroz H-A4",
            500,
            0,
            "10",
            "5,00",
        );
        let sale = super::create_sale_invoice_inner(
            &mut connection,
            now,
            sale_input(client_id, vec![sale_line(product_id, "3")], None),
        )
        .unwrap();
        // Se elimina la compra que originó el lote consumido.
        let entry_id: i64 = connection
            .query_row(
                "SELECT invoiceId FROM invoiceLines WHERE productId = ?1 AND invoiceId != ?2 LIMIT 1",
                (product_id, sale.invoice_id),
                |row| row.get(0),
            )
            .unwrap();
        super::delete_invoice_inner(&mut connection, now, entry_id).unwrap();

        // Misma cantidad con los lotes en otro estado: sin Ajuste.
        super::update_sale_invoice_inner(
            &mut connection,
            now,
            update_sale_input(
                sale.invoice_id,
                client_id,
                vec![update_outgoing_line(
                    Some(product_id),
                    Some(product_id),
                    "3",
                )],
            ),
        )
        .unwrap();

        let movements: Vec<(String, i64)> = connection
            .prepare(
                "SELECT type, quantityMilli FROM productMovements
                     WHERE invoiceId = ?1 ORDER BY id",
            )
            .unwrap()
            .query_map([sale.invoice_id], |row| Ok((row.get(0)?, row.get(1)?)))
            .unwrap()
            .collect::<rusqlite::Result<Vec<_>>>()
            .unwrap();
        assert_eq!(movements, vec![("saleOut".to_owned(), -3_000)]);
        let unit_price: i64 = connection
            .query_row(
                "SELECT unitPriceCents FROM invoiceLines WHERE invoiceId = ?1",
                [sale.invoice_id],
                |row| row.get(0),
            )
            .unwrap();
        assert_eq!(unit_price, 500);
    }

    #[test]
    fn update_sale_invoice_rolls_back_when_movement_insert_fails() {
        let now = Utc.with_ymd_and_hms(2026, 10, 4, 12, 0, 0).unwrap();
        let (mut connection, _, client_id, supplier_id) = setup_sale_fixture(
            "updatesaleatomic",
            "12345678990",
            "+5355559146",
            "12345678991",
            "+5355559147",
            now,
        );
        let (sale_id, product_a, _, _) =
            insert_sale_for_update(&mut connection, now, supplier_id, client_id);
        connection
            .execute_batch(
                "CREATE TRIGGER fail_update_movement BEFORE INSERT ON productMovements
                     BEGIN SELECT RAISE(ABORT, 'forced movement insertion failure'); END;",
            )
            .unwrap();

        assert!(super::update_sale_invoice_inner(
            &mut connection,
            now,
            update_sale_input(
                sale_id,
                client_id,
                vec![update_outgoing_line(Some(product_a), Some(product_a), "3",)],
            ),
        )
        .is_err());

        let date: String = connection
            .query_row(
                "SELECT invoiceDate FROM invoices WHERE id = ?1",
                [sale_id],
                |row| row.get(0),
            )
            .unwrap();
        assert_eq!(date, "2026-10-04");
        let line_qty: i64 = connection
            .query_row(
                "SELECT quantityMilli FROM invoiceLines WHERE invoiceId = ?1",
                [sale_id],
                |row| row.get(0),
            )
            .unwrap();
        assert_eq!(line_qty, 2_000);
        let stock: i64 = connection
            .query_row(
                "SELECT quantityMilli FROM products WHERE id = ?1",
                [product_a],
                |row| row.get(0),
            )
            .unwrap();
        assert_eq!(stock, 8_000);
    }

    #[test]
    fn update_outgoing_commands_refresh_updated_at_and_require_session() {
        let now = Utc.with_ymd_and_hms(2026, 10, 4, 12, 0, 0).unwrap();
        let later = Utc.with_ymd_and_hms(2026, 10, 4, 13, 0, 0).unwrap();
        let (mut connection, _, client_id, supplier_id) = setup_sale_fixture(
            "updaten1",
            "12345678992",
            "+5355559148",
            "12345678993",
            "+5355559149",
            now,
        );
        let product_id = insert_stocked_product(
            &mut connection,
            now,
            supplier_id,
            "Producto N-1",
            100,
            0,
            "20",
            "1,00",
        );
        let sale = super::create_sale_invoice_inner(
            &mut connection,
            now,
            sale_input(client_id, vec![sale_line(product_id, "2")], None),
        )
        .unwrap();
        let service = super::create_service_invoice_inner(
            &mut connection,
            now,
            service_input(client_id, vec![service_line(product_id, "2")], None),
        )
        .unwrap();
        let guarantee = super::create_guarantee_invoice_inner(
            &mut connection,
            now,
            guarantee_input(
                client_id,
                sale.invoice_id,
                vec![guarantee_line(product_id, "2")],
                None,
            ),
        )
        .unwrap();
        let installation = super::create_installation_invoice_inner(
            &mut connection,
            now,
            installation_input(
                client_id,
                service.invoice_id,
                vec![installation_line(product_id, "2")],
                None,
            ),
        )
        .unwrap();

        super::update_sale_invoice_inner(
            &mut connection,
            later,
            update_sale_input(
                sale.invoice_id,
                client_id,
                vec![update_outgoing_line(
                    Some(product_id),
                    Some(product_id),
                    "2",
                )],
            ),
        )
        .unwrap();
        super::update_service_invoice_inner(
            &mut connection,
            later,
            update_service_input(
                service.invoice_id,
                client_id,
                vec![update_outgoing_line(
                    Some(product_id),
                    Some(product_id),
                    "2",
                )],
            ),
        )
        .unwrap();
        super::update_guarantee_invoice_inner(
            &mut connection,
            later,
            update_guarantee_input(
                guarantee.invoice_id,
                client_id,
                sale.invoice_id,
                vec![update_outgoing_line(
                    Some(product_id),
                    Some(product_id),
                    "2",
                )],
            ),
        )
        .unwrap();
        super::update_installation_invoice_inner(
            &mut connection,
            later,
            update_installation_input(
                installation.invoice_id,
                client_id,
                service.invoice_id,
                vec![update_outgoing_line(
                    Some(product_id),
                    Some(product_id),
                    "2",
                )],
            ),
        )
        .unwrap();

        for invoice_id in [
            sale.invoice_id,
            service.invoice_id,
            guarantee.invoice_id,
            installation.invoice_id,
        ] {
            let (created_at, updated_at): (String, String) = connection
                .query_row(
                    "SELECT createdAt, updatedAt FROM invoices WHERE id = ?1",
                    [invoice_id],
                    |row| Ok((row.get(0)?, row.get(1)?)),
                )
                .unwrap();
            assert_eq!(created_at, "2026-10-04T12:00:00Z");
            assert_eq!(updated_at, "2026-10-04T13:00:00Z");
        }
    }

    #[test]
    fn update_outgoing_commands_require_session_and_allow_both_roles() {
        assert_requires_session(|connection, now| {
            super::update_sale_invoice_inner(connection, now, update_sale_input(1, 1, Vec::new()))
        });
        assert_requires_session(|connection, now| {
            super::update_service_invoice_inner(
                connection,
                now,
                update_service_input(1, 1, Vec::new()),
            )
        });
        assert_requires_session(|connection, now| {
            super::update_guarantee_invoice_inner(
                connection,
                now,
                update_guarantee_input(1, 1, 1, Vec::new()),
            )
        });
        assert_requires_session(|connection, now| {
            super::update_installation_invoice_inner(
                connection,
                now,
                update_installation_input(1, 1, 1, Vec::new()),
            )
        });
        assert_allows_both_roles(|connection, now| {
            connection
                .execute(
                    "INSERT INTO clients (firstName, lastName, nationalId, phone, createdAt, updatedAt)
                         VALUES ('Ana', 'Pérez', '12345678994', '+5355559150', ?1, ?1)",
                    [format_utc_timestamp(now)],
                )
                .unwrap();
            let client_id = connection.last_insert_rowid();
            super::update_sale_invoice_inner(
                connection,
                now,
                update_sale_input(
                    999,
                    client_id,
                    vec![update_outgoing_line(None, Some(999), "1")],
                ),
            )
        });
        assert_allows_both_roles(|connection, now| {
            connection
                .execute(
                    "INSERT INTO clients (firstName, lastName, nationalId, phone, createdAt, updatedAt)
                         VALUES ('Ana', 'Pérez', '12345678995', '+5355559151', ?1, ?1)",
                    [format_utc_timestamp(now)],
                )
                .unwrap();
            let client_id = connection.last_insert_rowid();
            super::update_service_invoice_inner(
                connection,
                now,
                update_service_input(
                    999,
                    client_id,
                    vec![update_outgoing_line(None, Some(999), "1")],
                ),
            )
        });
        assert_allows_both_roles(|connection, now| {
            connection
                .execute(
                    "INSERT INTO clients (firstName, lastName, nationalId, phone, createdAt, updatedAt)
                         VALUES ('Ana', 'Pérez', '12345678996', '+5355559152', ?1, ?1)",
                    [format_utc_timestamp(now)],
                )
                .unwrap();
            let client_id = connection.last_insert_rowid();
            super::update_guarantee_invoice_inner(
                connection,
                now,
                update_guarantee_input(
                    999,
                    client_id,
                    999,
                    vec![update_outgoing_line(None, Some(999), "1")],
                ),
            )
        });
        assert_allows_both_roles(|connection, now| {
            connection
                .execute(
                    "INSERT INTO clients (firstName, lastName, nationalId, phone, createdAt, updatedAt)
                         VALUES ('Ana', 'Pérez', '12345678997', '+5355559153', ?1, ?1)",
                    [format_utc_timestamp(now)],
                )
                .unwrap();
            let client_id = connection.last_insert_rowid();
            super::update_installation_invoice_inner(
                connection,
                now,
                update_installation_input(
                    999,
                    client_id,
                    999,
                    vec![update_outgoing_line(None, Some(999), "1")],
                ),
            )
        });
    }

    #[test]
    fn update_service_invoice_edits_pending_with_sale_and_adjustment() {
        let now = Utc.with_ymd_and_hms(2026, 10, 4, 12, 0, 0).unwrap();
        let (mut connection, _, client_id, supplier_id) = setup_sale_fixture(
            "updateservice",
            "12345678998",
            "+5355559154",
            "12345678999",
            "+5355559155",
            now,
        );
        let product_id = insert_stocked_product(
            &mut connection,
            now,
            supplier_id,
            "Destornillador",
            200,
            5_000,
            "10",
            "2,00",
        );
        let service = super::create_service_invoice_inner(
            &mut connection,
            now,
            service_input(client_id, vec![service_line(product_id, "2")], None),
        )
        .unwrap();

        // 2→1 (Ajuste +1: vuelve una unidad al stock); el precio emitido (300) se conserva.
        super::update_service_invoice_inner(
            &mut connection,
            now,
            update_service_input(
                service.invoice_id,
                client_id,
                vec![update_outgoing_line(
                    Some(product_id),
                    Some(product_id),
                    "1",
                )],
            ),
        )
        .unwrap();

        let movements: Vec<(String, i64, i64)> = connection
            .prepare(
                "SELECT type, quantityMilli, unitPriceCents FROM productMovements
                     WHERE invoiceId = ?1 ORDER BY id",
            )
            .unwrap()
            .query_map([service.invoice_id], |row| {
                Ok((row.get(0)?, row.get(1)?, row.get(2)?))
            })
            .unwrap()
            .collect::<rusqlite::Result<Vec<_>>>()
            .unwrap();
        assert_eq!(
            movements,
            vec![
                ("serviceOut".to_owned(), -2_000, 300),
                ("adjustment".to_owned(), 1_000, 300),
            ]
        );
        let line: (i64, i64) = connection
            .query_row(
                "SELECT quantityMilli, unitPriceCents FROM invoiceLines WHERE invoiceId = ?1",
                [service.invoice_id],
                |row| Ok((row.get(0)?, row.get(1)?)),
            )
            .unwrap();
        assert_eq!(line, (1_000, 300));
    }

    #[test]
    fn update_guarantee_invoice_edits_pending_recalculating_cost() {
        let now = Utc.with_ymd_and_hms(2026, 10, 4, 12, 0, 0).unwrap();
        let (mut connection, _, client_id, supplier_id) = setup_sale_fixture(
            "updateguarantee",
            "12345679000",
            "+5355559156",
            "12345679001",
            "+5355559157",
            now,
        );
        let product_id = insert_stocked_product(
            &mut connection,
            now,
            supplier_id,
            "Taladro",
            100,
            0,
            "10",
            "1,00",
        );
        let sale = super::create_sale_invoice_inner(
            &mut connection,
            now,
            sale_input(client_id, vec![sale_line(product_id, "1")], None),
        )
        .unwrap();
        let guarantee = super::create_guarantee_invoice_inner(
            &mut connection,
            now,
            guarantee_input(
                client_id,
                sale.invoice_id,
                vec![guarantee_line(product_id, "2")],
                None,
            ),
        )
        .unwrap();

        // 2→1 con el mismo lote: el costo se recalcula (100) con Ajuste +1.
        super::update_guarantee_invoice_inner(
            &mut connection,
            now,
            update_guarantee_input(
                guarantee.invoice_id,
                client_id,
                sale.invoice_id,
                vec![update_outgoing_line(
                    Some(product_id),
                    Some(product_id),
                    "1",
                )],
            ),
        )
        .unwrap();

        let movements: Vec<(String, i64, i64)> = connection
            .prepare(
                "SELECT type, quantityMilli, unitPriceCents FROM productMovements
                     WHERE invoiceId = ?1 ORDER BY id",
            )
            .unwrap()
            .query_map([guarantee.invoice_id], |row| {
                Ok((row.get(0)?, row.get(1)?, row.get(2)?))
            })
            .unwrap()
            .collect::<rusqlite::Result<Vec<_>>>()
            .unwrap();
        assert_eq!(
            movements,
            vec![
                ("warrantyOut".to_owned(), -2_000, 100),
                ("adjustment".to_owned(), 1_000, 100),
            ]
        );
    }

    #[test]
    fn update_installation_invoice_edits_pending_with_supply_outcome() {
        let now = Utc.with_ymd_and_hms(2026, 10, 4, 12, 0, 0).unwrap();
        let (mut connection, _, client_id, supplier_id) = setup_sale_fixture(
            "updateinstallation",
            "12345679002",
            "+5355559158",
            "12345679003",
            "+5355559159",
            now,
        );
        let product_id = insert_stocked_product(
            &mut connection,
            now,
            supplier_id,
            "Llave inglesa",
            100,
            0,
            "10",
            "1,00",
        );
        let service = super::create_service_invoice_inner(
            &mut connection,
            now,
            service_input(client_id, vec![service_line(product_id, "1")], None),
        )
        .unwrap();
        let installation = super::create_installation_invoice_inner(
            &mut connection,
            now,
            installation_input(
                client_id,
                service.invoice_id,
                vec![installation_line(product_id, "2")],
                None,
            ),
        )
        .unwrap();

        super::update_installation_invoice_inner(
            &mut connection,
            now,
            update_installation_input(
                installation.invoice_id,
                client_id,
                service.invoice_id,
                vec![update_outgoing_line(
                    Some(product_id),
                    Some(product_id),
                    "1",
                )],
            ),
        )
        .unwrap();

        let movements: Vec<(String, i64)> = connection
            .prepare(
                "SELECT type, quantityMilli FROM productMovements
                     WHERE invoiceId = ?1 ORDER BY id",
            )
            .unwrap()
            .query_map([installation.invoice_id], |row| {
                Ok((row.get(0)?, row.get(1)?))
            })
            .unwrap()
            .collect::<rusqlite::Result<Vec<_>>>()
            .unwrap();
        assert_eq!(
            movements,
            vec![
                ("supplyOut".to_owned(), -2_000),
                ("adjustment".to_owned(), 1_000),
            ]
        );
    }
    #[test]
    fn update_entry_invoice_changes_pending_header_and_refreshes_updated_at() {
        let created_at = Utc.with_ymd_and_hms(2026, 10, 4, 12, 0, 0).unwrap();
        let edited_at = created_at + chrono::Duration::minutes(5);
        let (mut connection, _, supplier_id) = setup_entry_fixture("entryupdateheader", created_at);
        let product_id = insert_product_fixture(
            &connection,
            "Producto editable",
            None,
            None,
            100,
            0,
            0,
            "active",
            None,
            None,
        );
        let created = super::create_entry_invoice_inner(
            &mut connection,
            created_at,
            entry_input(
                supplier_id,
                vec![entry_line(Some(product_id), None, "2,000", "1,00")],
                None,
            ),
        )
        .unwrap();
        connection
            .execute(
                "INSERT INTO suppliers (firstName, lastName, nationalId, phone, createdAt, updatedAt)
                 VALUES ('Luis', 'Gómez', '12345678929', '+5355559095', ?1, ?1)",
                [format_utc_timestamp(created_at)],
            )
            .unwrap();
        let updated_supplier_id = connection.last_insert_rowid();
        let input = update_entry_input(
            created.invoice_id,
            updated_supplier_id,
            "2026-10-03",
            vec![update_entry_line(
                Some(product_id),
                product_id,
                "2,000",
                "1,00",
            )],
        );

        super::update_entry_invoice_inner(&mut connection, edited_at, input).unwrap();

        let updated: (String, Option<String>, String, String, String, String) = connection
            .query_row(
                "SELECT invoiceDate, notes, createdAt, updatedAt, contactFullName, contactNationalId
                 FROM invoices WHERE id = ?1",
                [created.invoice_id],
                |row| {
                    Ok((
                        row.get(0)?,
                        row.get(1)?,
                        row.get(2)?,
                        row.get(3)?,
                        row.get(4)?,
                        row.get(5)?,
                    ))
                },
            )
            .unwrap();
        assert_eq!(updated.0, "2026-10-03");
        assert_eq!(updated.1.as_deref(), Some("Observaciones editadas"));
        assert_eq!(
            updated.2,
            crate::timestamps::format_utc_timestamp(created_at)
        );
        assert_eq!(
            updated.3,
            crate::timestamps::format_utc_timestamp(edited_at)
        );
        assert_eq!(updated.4, "Luis Gómez");
        assert_eq!(updated.5, "12345678929");
    }

    #[test]
    fn update_entry_invoice_adds_and_removes_lines_with_inventory_movements() {
        let now = Utc.with_ymd_and_hms(2026, 10, 4, 12, 0, 0).unwrap();
        let (mut connection, _, supplier_id) = setup_entry_fixture("entryupdatelines", now);
        let original_product_id = insert_product_fixture(
            &connection,
            "Producto original",
            None,
            None,
            100,
            0,
            0,
            "active",
            None,
            None,
        );
        let added_product_id = insert_product_fixture(
            &connection,
            "Producto agregado",
            None,
            None,
            100,
            0,
            0,
            "active",
            None,
            None,
        );
        let (invoice_id, original_line_id, _) = create_entry_line_fixture(
            &mut connection,
            now,
            supplier_id,
            original_product_id,
            "10,000",
            "1,00",
        );
        let original_id: i64 = connection
            .query_row(
                "SELECT productIdOriginal FROM invoiceLines WHERE id = ?1",
                [original_line_id],
                |row| row.get(0),
            )
            .unwrap();

        super::update_entry_invoice_inner(
            &mut connection,
            now + chrono::Duration::minutes(1),
            update_entry_input(
                invoice_id,
                supplier_id,
                "2026-10-04",
                vec![
                    update_entry_line(Some(original_id), original_product_id, "10,000", "1,00"),
                    update_entry_line(None, added_product_id, "2,000", "2,50"),
                ],
            ),
        )
        .unwrap();

        let added_stock: i64 = connection
            .query_row(
                "SELECT quantityMilli FROM products WHERE id = ?1",
                [added_product_id],
                |row| row.get(0),
            )
            .unwrap();
        let added_cost: i64 = connection
            .query_row(
                "SELECT costCents FROM products WHERE id = ?1",
                [added_product_id],
                |row| row.get(0),
            )
            .unwrap();
        let added_movement: (String, i64, i64) = connection
            .query_row(
                "SELECT type, quantityMilli, stockAfterMilli FROM productMovements
                 WHERE invoiceId = ?1 AND productId = ?2 ORDER BY id DESC LIMIT 1",
                (invoice_id, added_product_id),
                |row| Ok((row.get(0)?, row.get(1)?, row.get(2)?)),
            )
            .unwrap();
        assert_eq!((added_stock, added_cost), (2_000, 250));
        assert_eq!(added_movement, ("entry".to_owned(), 2_000, 2_000));
        let added_original_id: i64 = connection
            .query_row(
                "SELECT productIdOriginal FROM invoiceLines WHERE invoiceId = ?1 AND productId = ?2",
                (invoice_id, added_product_id),
                |row| row.get(0),
            )
            .unwrap();

        super::update_entry_invoice_inner(
            &mut connection,
            now + chrono::Duration::minutes(2),
            update_entry_input(
                invoice_id,
                supplier_id,
                "2026-10-04",
                vec![
                    update_entry_line(Some(original_id), original_product_id, "8,000", "1,00"),
                    update_entry_line(Some(added_original_id), added_product_id, "2,000", "2,50"),
                ],
            ),
        )
        .unwrap();
        let negative_adjustment: (String, i64, i64) = connection
            .query_row(
                "SELECT type, quantityMilli, stockAfterMilli FROM productMovements
                 WHERE invoiceId = ?1 AND productId = ?2 ORDER BY id DESC LIMIT 1",
                (invoice_id, original_product_id),
                |row| Ok((row.get(0)?, row.get(1)?, row.get(2)?)),
            )
            .unwrap();
        assert_eq!(
            negative_adjustment,
            ("adjustment".to_owned(), -2_000, 8_000)
        );

        super::update_entry_invoice_inner(
            &mut connection,
            now + chrono::Duration::minutes(3),
            update_entry_input(
                invoice_id,
                supplier_id,
                "2026-10-04",
                vec![update_entry_line(
                    Some(original_id),
                    original_product_id,
                    "10,000",
                    "1,00",
                )],
            ),
        )
        .unwrap();

        let removed_product: (i64, i64) = connection
            .query_row(
                "SELECT quantityMilli, costCents FROM products WHERE id = ?1",
                [added_product_id],
                |row| Ok((row.get(0)?, row.get(1)?)),
            )
            .unwrap();
        let reversal: (String, i64, i64) = connection
            .query_row(
                "SELECT type, quantityMilli, stockAfterMilli FROM productMovements
                 WHERE invoiceId = ?1 AND productId = ?2 ORDER BY id DESC LIMIT 1",
                (invoice_id, added_product_id),
                |row| Ok((row.get(0)?, row.get(1)?, row.get(2)?)),
            )
            .unwrap();
        let remaining_lots: i64 = connection
            .query_row(
                "SELECT COUNT(*) FROM lots WHERE productId = ?1",
                [added_product_id],
                |row| row.get(0),
            )
            .unwrap();
        assert_eq!(removed_product, (0, 250));
        assert_eq!(reversal, ("reversal".to_owned(), -2_000, 0));
        assert_eq!(remaining_lots, 0);
    }

    #[test]
    fn update_entry_invoice_blocks_removal_reduction_cost_and_product_changes_after_consumption() {
        let now = Utc.with_ymd_and_hms(2026, 10, 4, 12, 0, 0).unwrap();
        let (mut connection, user_id, supplier_id) = setup_entry_fixture("entryusedrules", now);
        let used_product_id = insert_product_fixture(
            &connection,
            "Producto consumido",
            None,
            None,
            100,
            0,
            0,
            "active",
            None,
            None,
        );
        let other_product_id = insert_product_fixture(
            &connection,
            "Otro producto",
            None,
            None,
            100,
            0,
            0,
            "active",
            None,
            None,
        );
        let created = super::create_entry_invoice_inner(
            &mut connection,
            now,
            entry_input(
                supplier_id,
                vec![
                    entry_line(Some(used_product_id), None, "10,000", "1,00"),
                    entry_line(Some(other_product_id), None, "5,000", "1,00"),
                ],
                None,
            ),
        )
        .unwrap();
        let used_original_id: i64 = connection
            .query_row(
                "SELECT productIdOriginal FROM invoiceLines WHERE invoiceId = ?1 AND productId = ?2",
                (created.invoice_id, used_product_id),
                |row| row.get(0),
            )
            .unwrap();
        let other_original_id: i64 = connection
            .query_row(
                "SELECT productIdOriginal FROM invoiceLines WHERE invoiceId = ?1 AND productId = ?2",
                (created.invoice_id, other_product_id),
                |row| row.get(0),
            )
            .unwrap();
        let lot_id: i64 = connection
            .query_row(
                "SELECT id FROM lots WHERE productId = ?1",
                [used_product_id],
                |row| row.get(0),
            )
            .unwrap();
        add_entry_lot_consumption(
            &connection,
            now,
            user_id,
            used_product_id,
            lot_id,
            4_000,
            "V-00012",
        );

        let remove_used = super::update_entry_invoice_inner(
            &mut connection,
            now,
            update_entry_input(
                created.invoice_id,
                supplier_id,
                "2026-10-04",
                vec![update_entry_line(
                    Some(other_original_id),
                    other_product_id,
                    "5,000",
                    "1,00",
                )],
            ),
        )
        .unwrap_err();
        assert_eq!(remove_used.code, "invalidState");
        assert_eq!(
            remove_used.message,
            "No se puede quitar «Producto consumido»: ya se usaron unidades de esta compra"
        );

        let reduce_below_used = super::update_entry_invoice_inner(
            &mut connection,
            now,
            update_entry_input(
                created.invoice_id,
                supplier_id,
                "2026-10-04",
                vec![
                    update_entry_line(Some(used_original_id), used_product_id, "3,000", "1,00"),
                    update_entry_line(Some(other_original_id), other_product_id, "5,000", "1,00"),
                ],
            ),
        )
        .unwrap_err();
        assert_eq!(reduce_below_used.code, "invalidState");
        assert!(reduce_below_used.message.contains("4 unidades ya usadas"));

        let change_cost = super::update_entry_invoice_inner(
            &mut connection,
            now,
            update_entry_input(
                created.invoice_id,
                supplier_id,
                "2026-10-04",
                vec![
                    update_entry_line(Some(used_original_id), used_product_id, "10,000", "2,00"),
                    update_entry_line(Some(other_original_id), other_product_id, "5,000", "1,00"),
                ],
            ),
        )
        .unwrap_err();
        assert_eq!(change_cost.code, "invalidState");
        assert!(change_cost.message.contains("factura V-00012"));

        let change_product = super::update_entry_invoice_inner(
            &mut connection,
            now,
            update_entry_input(
                created.invoice_id,
                supplier_id,
                "2026-10-04",
                vec![update_entry_line(
                    Some(used_original_id),
                    other_product_id,
                    "10,000",
                    "1,00",
                )],
            ),
        )
        .unwrap_err();
        assert_eq!(change_product.code, "invalidState");
        let invoice_lines: i64 = connection
            .query_row(
                "SELECT COUNT(*) FROM invoiceLines WHERE invoiceId = ?1",
                [created.invoice_id],
                |row| row.get(0),
            )
            .unwrap();
        assert_eq!(invoice_lines, 2);
    }

    #[test]
    fn update_entry_invoice_allows_quantity_increase_with_consumption_and_keeps_fifo_order() {
        let now = Utc.with_ymd_and_hms(2026, 10, 4, 12, 0, 0).unwrap();
        let (mut connection, user_id, supplier_id) = setup_entry_fixture("entryincreaseused", now);
        let product_id = insert_product_fixture(
            &connection,
            "Producto aumento",
            None,
            None,
            100,
            0,
            0,
            "active",
            None,
            None,
        );
        let (invoice_id, line_id, lot_id) = create_entry_line_fixture(
            &mut connection,
            now,
            supplier_id,
            product_id,
            "10,000",
            "1,00",
        );
        let original_id: i64 = connection
            .query_row(
                "SELECT productIdOriginal FROM invoiceLines WHERE id = ?1",
                [line_id],
                |row| row.get(0),
            )
            .unwrap();
        let original_order: i64 = connection
            .query_row(
                "SELECT arrivalOrder FROM lots WHERE id = ?1",
                [lot_id],
                |row| row.get(0),
            )
            .unwrap();
        add_entry_lot_consumption(
            &connection,
            now,
            user_id,
            product_id,
            lot_id,
            4_000,
            "V-00013",
        );

        super::update_entry_invoice_inner(
            &mut connection,
            now + chrono::Duration::minutes(1),
            update_entry_input(
                invoice_id,
                supplier_id,
                "2026-10-04",
                vec![update_entry_line(
                    Some(original_id),
                    product_id,
                    "12,000",
                    "1,00",
                )],
            ),
        )
        .unwrap();

        let lot_after: (i64, i64, i64) = connection
            .query_row(
                "SELECT initialMilli, remainingMilli, arrivalOrder FROM lots WHERE id = ?1",
                [lot_id],
                |row| Ok((row.get(0)?, row.get(1)?, row.get(2)?)),
            )
            .unwrap();
        let product_stock: i64 = connection
            .query_row(
                "SELECT quantityMilli FROM products WHERE id = ?1",
                [product_id],
                |row| row.get(0),
            )
            .unwrap();
        let adjustment: (String, i64) = connection
            .query_row(
                "SELECT type, quantityMilli FROM productMovements WHERE invoiceId = ?1 ORDER BY id DESC LIMIT 1",
                [invoice_id],
                |row| Ok((row.get(0)?, row.get(1)?)),
            )
            .unwrap();
        assert_eq!(lot_after, (12_000, 8_000, original_order));
        assert_eq!(product_stock, 8_000);
        assert_eq!(adjustment, ("adjustment".to_owned(), 2_000));
    }

    #[test]
    fn update_entry_invoice_cost_change_without_quantity_change_has_no_adjustment() {
        let now = Utc.with_ymd_and_hms(2026, 10, 4, 12, 0, 0).unwrap();
        let (mut connection, _, supplier_id) = setup_entry_fixture("entrycostonly", now);
        let product_id = insert_product_fixture(
            &connection,
            "Producto costo",
            None,
            None,
            100,
            0,
            0,
            "active",
            None,
            None,
        );
        let (invoice_id, line_id, lot_id) = create_entry_line_fixture(
            &mut connection,
            now,
            supplier_id,
            product_id,
            "5,000",
            "1,00",
        );
        let original_id: i64 = connection
            .query_row(
                "SELECT productIdOriginal FROM invoiceLines WHERE id = ?1",
                [line_id],
                |row| row.get(0),
            )
            .unwrap();

        super::update_entry_invoice_inner(
            &mut connection,
            now + chrono::Duration::minutes(1),
            update_entry_input(
                invoice_id,
                supplier_id,
                "2026-10-04",
                vec![update_entry_line(
                    Some(original_id),
                    product_id,
                    "5,000",
                    "2,50",
                )],
            ),
        )
        .unwrap();

        let snapshot: (i64, i64, i64, i64) = connection
            .query_row(
                "SELECT il.quantityMilli, il.unitPriceCents, l.unitCostCents, p.costCents
                 FROM invoiceLines il JOIN lots l ON l.invoiceLineId = il.id
                 JOIN products p ON p.id = il.productId WHERE il.id = ?1",
                [line_id],
                |row| Ok((row.get(0)?, row.get(1)?, row.get(2)?, row.get(3)?)),
            )
            .unwrap();
        let adjustment_count: i64 = connection
            .query_row(
                "SELECT COUNT(*) FROM productMovements WHERE invoiceId = ?1 AND type = 'adjustment'",
                [invoice_id],
                |row| row.get(0),
            )
            .unwrap();
        assert_eq!(snapshot, (5_000, 250, 250, 250));
        assert_eq!(adjustment_count, 0);
        assert_eq!(
            connection
                .query_row("SELECT id FROM lots WHERE id = ?1", [lot_id], |row| row
                    .get::<_, i64>(0))
                .unwrap(),
            lot_id
        );
    }

    #[test]
    fn update_entry_invoice_rejects_completed_and_cancelled_invoices() {
        let now = Utc.with_ymd_and_hms(2026, 10, 4, 12, 0, 0).unwrap();
        for (username, target_status, expected_message) in [
            (
                "entrycompletededit",
                "completed",
                "Solo se pueden editar las facturas que están Por Pagar",
            ),
            (
                "entrycancellededit",
                "cancelled",
                "Una factura cancelada no se puede reactivar ni editar",
            ),
        ] {
            let (mut connection, _, supplier_id) = setup_entry_fixture(username, now);
            let product_id = insert_product_fixture(
                &connection,
                "Producto no editable",
                None,
                None,
                100,
                0,
                0,
                "active",
                None,
                None,
            );
            let (invoice_id, line_id, _) = create_entry_line_fixture(
                &mut connection,
                now,
                supplier_id,
                product_id,
                "1,000",
                "1,00",
            );
            let original_id: i64 = connection
                .query_row(
                    "SELECT productIdOriginal FROM invoiceLines WHERE id = ?1",
                    [line_id],
                    |row| row.get(0),
                )
                .unwrap();
            super::change_invoice_status_inner(
                &mut connection,
                now,
                invoice_id,
                target_status,
                None,
            )
            .unwrap();

            let error = super::update_entry_invoice_inner(
                &mut connection,
                now + chrono::Duration::minutes(1),
                update_entry_input(
                    invoice_id,
                    supplier_id,
                    "2026-10-03",
                    vec![update_entry_line(
                        Some(original_id),
                        product_id,
                        "1,000",
                        "1,00",
                    )],
                ),
            )
            .unwrap_err();
            assert_eq!(error.code, "invalidState");
            assert_eq!(error.message, expected_message);
        }
    }

    #[test]
    fn update_entry_invoice_discontinued_products_can_only_be_reduced() {
        let now = Utc.with_ymd_and_hms(2026, 10, 4, 12, 0, 0).unwrap();
        let (mut connection, _, supplier_id) = setup_entry_fixture("entrydiscontinued", now);
        let product_id = insert_product_fixture(
            &connection,
            "Producto descontinuado",
            None,
            None,
            100,
            0,
            0,
            "active",
            None,
            None,
        );
        let (invoice_id, line_id, _) = create_entry_line_fixture(
            &mut connection,
            now,
            supplier_id,
            product_id,
            "10,000",
            "1,00",
        );
        let original_id: i64 = connection
            .query_row(
                "SELECT productIdOriginal FROM invoiceLines WHERE id = ?1",
                [line_id],
                |row| row.get(0),
            )
            .unwrap();
        connection
            .execute(
                "UPDATE products SET status = 'discontinued', discontinuedAt = ?1 WHERE id = ?2",
                (format_utc_timestamp(now), product_id),
            )
            .unwrap();

        super::update_entry_invoice_inner(
            &mut connection,
            now + chrono::Duration::minutes(1),
            update_entry_input(
                invoice_id,
                supplier_id,
                "2026-10-04",
                vec![update_entry_line(
                    Some(original_id),
                    product_id,
                    "8,000",
                    "1,00",
                )],
            ),
        )
        .unwrap();
        let error = super::update_entry_invoice_inner(
            &mut connection,
            now + chrono::Duration::minutes(2),
            update_entry_input(
                invoice_id,
                supplier_id,
                "2026-10-04",
                vec![update_entry_line(
                    Some(original_id),
                    product_id,
                    "9,000",
                    "1,00",
                )],
            ),
        )
        .unwrap_err();
        assert_eq!(error.code, "invalidState");
        let replacement_product_id = insert_product_fixture(
            &connection,
            "Producto de reemplazo",
            None,
            None,
            100,
            0,
            0,
            "active",
            None,
            None,
        );
        let replacement_error = super::update_entry_invoice_inner(
            &mut connection,
            now + chrono::Duration::minutes(3),
            update_entry_input(
                invoice_id,
                supplier_id,
                "2026-10-04",
                vec![update_entry_line(
                    Some(original_id),
                    replacement_product_id,
                    "8,000",
                    "1,00",
                )],
            ),
        )
        .unwrap_err();
        assert_eq!(replacement_error.code, "invalidState");
        let lot_quantity: (i64, i64) = connection
            .query_row(
                "SELECT initialMilli, remainingMilli FROM lots WHERE invoiceLineId = ?1",
                [line_id],
                |row| Ok((row.get(0)?, row.get(1)?)),
            )
            .unwrap();
        assert_eq!(lot_quantity, (8_000, 8_000));
    }

    #[test]
    fn update_entry_invoice_deleted_products_can_be_removed_or_reduced_to_zero() {
        let now = Utc.with_ymd_and_hms(2026, 10, 4, 12, 0, 0).unwrap();
        let (mut connection, _, supplier_id) = setup_entry_fixture("entrydeletedlines", now);
        let kept_product_id = insert_product_fixture(
            &connection,
            "Producto conservado",
            None,
            None,
            100,
            0,
            0,
            "active",
            None,
            None,
        );
        let removed_product_id = insert_product_fixture(
            &connection,
            "Producto borrado uno",
            None,
            None,
            100,
            0,
            0,
            "active",
            None,
            None,
        );
        let zeroed_product_id = insert_product_fixture(
            &connection,
            "Producto borrado dos",
            None,
            None,
            100,
            0,
            0,
            "active",
            None,
            None,
        );
        let invoice = super::create_entry_invoice_inner(
            &mut connection,
            now,
            entry_input(
                supplier_id,
                vec![
                    entry_line(Some(kept_product_id), None, "1,000", "1,00"),
                    entry_line(Some(removed_product_id), None, "2,000", "1,00"),
                    entry_line(Some(zeroed_product_id), None, "3,000", "1,00"),
                ],
                None,
            ),
        )
        .unwrap();
        let zeroed_original_id: i64 = connection
            .query_row(
                "SELECT productIdOriginal FROM invoiceLines WHERE invoiceId = ?1 AND productId = ?2",
                (invoice.invoice_id, zeroed_product_id),
                |row| row.get(0),
            )
            .unwrap();
        connection
            .execute(
                "DELETE FROM products WHERE id IN (?1, ?2)",
                (removed_product_id, zeroed_product_id),
            )
            .unwrap();

        super::update_entry_invoice_inner(
            &mut connection,
            now + chrono::Duration::minutes(1),
            update_entry_input(
                invoice.invoice_id,
                supplier_id,
                "2026-10-04",
                vec![
                    update_entry_line(Some(kept_product_id), kept_product_id, "1,000", "1,00"),
                    update_deleted_entry_line(zeroed_original_id, "0", "1,00"),
                ],
            ),
        )
        .unwrap();

        let line_count: i64 = connection
            .query_row(
                "SELECT COUNT(*) FROM invoiceLines WHERE invoiceId = ?1",
                [invoice.invoice_id],
                |row| row.get(0),
            )
            .unwrap();
        assert_eq!(line_count, 1);
        let remaining_product_id: i64 = connection
            .query_row(
                "SELECT productId FROM invoiceLines WHERE invoiceId = ?1",
                [invoice.invoice_id],
                |row| row.get(0),
            )
            .unwrap();
        assert_eq!(remaining_product_id, kept_product_id);
    }

    #[test]
    fn update_entry_invoice_stock_limit_rejects_new_lines_and_increases_without_changes() {
        let now = Utc.with_ymd_and_hms(2026, 10, 4, 12, 0, 0).unwrap();
        for (username, increase_existing) in
            [("entrystockadd", false), ("entrystockincrease", true)]
        {
            let (mut connection, user_id, supplier_id) = setup_entry_fixture(username, now);
            let target_product_id = insert_product_fixture(
                &connection,
                "Producto objetivo",
                None,
                None,
                100,
                0,
                0,
                "active",
                None,
                None,
            );
            let (invoice_id, target_line_id, target_lot_id) = create_entry_line_fixture(
                &mut connection,
                now,
                supplier_id,
                target_product_id,
                "1,000",
                "1,00",
            );
            let target_original_id: i64 = connection
                .query_row(
                    "SELECT productIdOriginal FROM invoiceLines WHERE id = ?1",
                    [target_line_id],
                    |row| row.get(0),
                )
                .unwrap();
            if increase_existing {
                add_entry_lot_consumption(
                    &connection,
                    now,
                    user_id,
                    target_product_id,
                    target_lot_id,
                    1_000,
                    "V-00014",
                );
            }
            let high_stock_product_id = insert_product_fixture(
                &connection,
                "Producto stock alto",
                None,
                None,
                100,
                0,
                999_999_500,
                "active",
                None,
                None,
            );
            connection
                .execute(
                    "INSERT INTO lots (productId, origin, arrivalOrder, initialMilli, remainingMilli, unitCostCents, createdAt, updatedAt)
                     VALUES (?1, 'initial', 1, 999999500, 999999500, 100, ?2, ?2)",
                    (high_stock_product_id, format_utc_timestamp(now)),
                )
                .unwrap();

            let lines = if increase_existing {
                connection
                    .execute(
                        "UPDATE lots SET remainingMilli = 0 WHERE id = ?1",
                        [target_lot_id],
                    )
                    .unwrap();
                connection
                    .execute(
                        "INSERT INTO lots (productId, origin, arrivalOrder, initialMilli, remainingMilli, unitCostCents, createdAt, updatedAt)
                         VALUES (?1, 'initial', 2, 999999500, 999999500, 100, ?2, ?2)",
                        (target_product_id, format_utc_timestamp(now)),
                    )
                    .unwrap();
                connection
                    .execute(
                        "UPDATE products SET quantityMilli = 999999500 WHERE id = ?1",
                        [target_product_id],
                    )
                    .unwrap();
                vec![update_entry_line(
                    Some(target_original_id),
                    target_product_id,
                    "1,500",
                    "1,00",
                )]
            } else {
                vec![
                    update_entry_line(Some(target_original_id), target_product_id, "1,000", "1,00"),
                    update_entry_line(None, high_stock_product_id, "0,500", "1,00"),
                ]
            };
            let before_header: (String, String) = connection
                .query_row(
                    "SELECT invoiceDate, updatedAt FROM invoices WHERE id = ?1",
                    [invoice_id],
                    |row| Ok((row.get(0)?, row.get(1)?)),
                )
                .unwrap();

            let error = super::update_entry_invoice_inner(
                &mut connection,
                now + chrono::Duration::minutes(1),
                update_entry_input(invoice_id, supplier_id, "2026-10-03", lines),
            )
            .unwrap_err();

            assert_eq!(error.code, "stockLimitExceeded");
            let after_header: (String, String) = connection
                .query_row(
                    "SELECT invoiceDate, updatedAt FROM invoices WHERE id = ?1",
                    [invoice_id],
                    |row| Ok((row.get(0)?, row.get(1)?)),
                )
                .unwrap();
            assert_eq!(after_header, before_header);
            let entry_line_count: i64 = connection
                .query_row(
                    "SELECT COUNT(*) FROM invoiceLines WHERE invoiceId = ?1",
                    [invoice_id],
                    |row| row.get(0),
                )
                .unwrap();
            assert_eq!(entry_line_count, 1);
        }
    }

    #[test]
    fn update_entry_invoice_rolls_back_when_reversal_movement_insert_fails() {
        let now = Utc.with_ymd_and_hms(2026, 10, 4, 12, 0, 0).unwrap();
        let (mut connection, _, supplier_id) = setup_entry_fixture("entryupdaterollback", now);
        let kept_product_id = insert_product_fixture(
            &connection,
            "Producto que queda",
            None,
            None,
            100,
            0,
            0,
            "active",
            None,
            None,
        );
        let removed_product_id = insert_product_fixture(
            &connection,
            "Producto que se quita",
            None,
            None,
            100,
            0,
            0,
            "active",
            None,
            None,
        );
        let invoice = super::create_entry_invoice_inner(
            &mut connection,
            now,
            entry_input(
                supplier_id,
                vec![
                    entry_line(Some(kept_product_id), None, "1,000", "1,00"),
                    entry_line(Some(removed_product_id), None, "2,000", "1,00"),
                ],
                None,
            ),
        )
        .unwrap();
        let kept_original_id: i64 = connection
            .query_row(
                "SELECT productIdOriginal FROM invoiceLines WHERE invoiceId = ?1 AND productId = ?2",
                (invoice.invoice_id, kept_product_id),
                |row| row.get(0),
            )
            .unwrap();
        let movements_before: i64 = connection
            .query_row("SELECT COUNT(*) FROM productMovements", [], |row| {
                row.get(0)
            })
            .unwrap();
        connection
            .execute_batch(
                "CREATE TRIGGER fail_update_reversal BEFORE INSERT ON productMovements
                 WHEN NEW.type = 'reversal'
                 BEGIN SELECT RAISE(ABORT, 'forced update reversal failure'); END;",
            )
            .unwrap();

        assert!(super::update_entry_invoice_inner(
            &mut connection,
            now + chrono::Duration::minutes(1),
            update_entry_input(
                invoice.invoice_id,
                supplier_id,
                "2026-10-03",
                vec![update_entry_line(
                    Some(kept_original_id),
                    kept_product_id,
                    "1,000",
                    "1,00",
                )],
            ),
        )
        .is_err());

        let invoice_date: String = connection
            .query_row(
                "SELECT invoiceDate FROM invoices WHERE id = ?1",
                [invoice.invoice_id],
                |row| row.get(0),
            )
            .unwrap();
        let line_count: i64 = connection
            .query_row(
                "SELECT COUNT(*) FROM invoiceLines WHERE invoiceId = ?1",
                [invoice.invoice_id],
                |row| row.get(0),
            )
            .unwrap();
        let removed_product_stock: i64 = connection
            .query_row(
                "SELECT quantityMilli FROM products WHERE id = ?1",
                [removed_product_id],
                |row| row.get(0),
            )
            .unwrap();
        let movements_after: i64 = connection
            .query_row("SELECT COUNT(*) FROM productMovements", [], |row| {
                row.get(0)
            })
            .unwrap();
        assert_eq!(invoice_date, "2026-10-04");
        assert_eq!(line_count, 2);
        assert_eq!(removed_product_stock, 2_000);
        assert_eq!(movements_after, movements_before);
    }

    #[test]
    fn update_entry_invoice_requires_session_and_allows_both_roles() {
        assert_requires_session(|connection, now| {
            super::update_entry_invoice_inner(
                connection,
                now,
                update_entry_input(1, 1, "2026-10-04", Vec::new()),
            )
        });
        assert_allows_both_roles(|connection, now| {
            connection
                .execute(
                    "INSERT INTO suppliers (firstName, lastName, nationalId, phone, createdAt, updatedAt)
                     VALUES ('Ana', 'Pérez', '12345678928', '+5355559094', ?1, ?1)",
                    [format_utc_timestamp(now)],
                )
                .unwrap();
            let supplier_id = connection.last_insert_rowid();
            let product_id = insert_product_fixture(
                connection,
                "Producto matriz edición",
                None,
                None,
                100,
                0,
                0,
                "active",
                None,
                None,
            );
            let invoice = super::create_entry_invoice_inner(
                connection,
                now,
                entry_input(
                    supplier_id,
                    vec![entry_line(Some(product_id), None, "1,000", "1,00")],
                    None,
                ),
            )?;
            let product_original_id: i64 = connection
                .query_row(
                    "SELECT productIdOriginal FROM invoiceLines WHERE invoiceId = ?1",
                    [invoice.invoice_id],
                    |row| row.get(0),
                )
                .unwrap();
            super::update_entry_invoice_inner(
                connection,
                now,
                update_entry_input(
                    invoice.invoice_id,
                    supplier_id,
                    "2026-10-04",
                    vec![update_entry_line(
                        Some(product_original_id),
                        product_id,
                        "1,000",
                        "1,00",
                    )],
                ),
            )
        });
    }

    #[test]
    fn create_entry_invoice_requires_session_and_allows_both_roles() {
        assert_requires_session(|connection, now| {
            super::create_entry_invoice_inner(connection, now, entry_input(1, Vec::new(), None))
        });
        assert_allows_both_roles(|connection, now| {
            connection
                .execute(
                    "INSERT INTO suppliers (firstName, lastName, nationalId, phone, createdAt, updatedAt)
                     VALUES ('Ana', 'Pérez', '12345678927', '+5355559093', ?1, ?1)",
                    [format_utc_timestamp(now)],
                )
                .unwrap();
            let supplier_id = connection.last_insert_rowid();
            super::create_entry_invoice_inner(
                connection,
                now,
                entry_input(
                    supplier_id,
                    vec![entry_line(
                        None,
                        Some(new_product("Producto autorización")),
                        "1,000",
                        "1,00",
                    )],
                    None,
                ),
            )
        });
    }

    #[test]
    fn delete_invoice_inner_removes_remaining_entry_stock_and_tracks_reversal() {
        let now = Utc.with_ymd_and_hms(2026, 10, 4, 12, 0, 0).unwrap();
        let mut connection = setup_test_db();
        let user_id = create_test_user(
            &connection,
            "deleteuser",
            "contador",
            "+5350000123",
            "12345678912",
            "ValidPass1!",
        );
        create_test_session(&connection, user_id, false, now, now);

        let product_id = insert_product_fixture(
            &connection,
            "Producto de compra",
            None,
            None,
            100,
            0,
            10_000,
            "active",
            None,
            None,
        );
        connection
            .execute(
                "INSERT INTO suppliers (firstName, lastName, nationalId, phone, createdAt, updatedAt)
                 VALUES ('Ana', 'Pérez', '12345678920', '+5355552222', ?1, ?1)",
                [format_utc_timestamp(now)],
            )
            .unwrap();
        let supplier_id = connection.last_insert_rowid();
        connection
            .execute(
                "INSERT INTO invoices (type, number, invoiceDate, status, supplierId, contactFullName, contactFullNameNormalized,
                    contactNationalId, contactPhone, createdByUserId, createdByName, createdAt, updatedAt)
                 VALUES ('E', 'E-00001', '2026-10-01', 'pending', ?1, 'Ana Pérez', 'ana perez', '12345678920', '+5355552222', ?2, 'admin', ?3, ?3)",
                (supplier_id, user_id, format_utc_timestamp(now)),
            )
            .unwrap();
        let invoice_id = connection.last_insert_rowid();
        connection
            .execute(
                "INSERT INTO invoiceLines (invoiceId, position, productId, productIdOriginal, productName, productNameNormalized, quantityMilli, unitPriceCents, subtotalCents, createdAt, updatedAt)
                 VALUES (?1, 1, ?2, ?2, 'Producto de compra', 'producto de compra', 10_000, 100, 1_000, ?3, ?3)",
                (invoice_id, product_id, format_utc_timestamp(now)),
            )
            .unwrap();
        let invoice_line_id = connection.last_insert_rowid();
        connection
            .execute(
                "INSERT INTO lots (productId, origin, invoiceLineId, arrivalOrder, initialMilli, remainingMilli, unitCostCents, createdAt, updatedAt)
                 VALUES (?1, 'entry', ?2, 1, 10_000, 10_000, 100, ?3, ?3)",
                (product_id, invoice_line_id, format_utc_timestamp(now)),
            )
            .unwrap();

        let result = delete_invoice_inner(&mut connection, now, invoice_id).unwrap();
        assert_eq!(result.invoice_id, invoice_id);
        assert_eq!(result.number, "E-00001");
        let invoice_exists: bool = connection
            .query_row(
                "SELECT EXISTS(SELECT 1 FROM invoices WHERE id = ?1)",
                [invoice_id],
                |row| row.get(0),
            )
            .unwrap();
        assert!(!invoice_exists);

        let stock: i64 = connection
            .query_row(
                "SELECT quantityMilli FROM products WHERE id = ?1",
                [product_id],
                |row| row.get(0),
            )
            .unwrap();
        assert_eq!(stock, 0);

        let reversal_count: i64 = connection
            .query_row(
                "SELECT COUNT(*) FROM productMovements WHERE invoiceNumber = 'E-00001' AND type = 'reversal'",
                [],
                |row| row.get(0),
            )
            .unwrap();
        assert_eq!(reversal_count, 1);
    }

    #[test]
    fn delete_invoice_inner_restores_sale_and_loss_consumptions() {
        let now = Utc.with_ymd_and_hms(2026, 10, 4, 12, 0, 0).unwrap();
        for (invoice_type, number, username, phone, national_id) in [
            ("V", "V-00001", "deletesale", "+5350000124", "12345678913"),
            ("P", "P-00001", "deleteloss", "+5350000125", "12345678914"),
        ] {
            let mut connection = setup_test_db();
            let user_id = create_test_user(
                &connection,
                username,
                "contador",
                phone,
                national_id,
                "ValidPass1!",
            );
            create_test_session(&connection, user_id, false, now, now);
            let product_id = insert_product_fixture(
                &connection,
                "Producto consumido",
                None,
                None,
                100,
                0,
                6_000,
                "active",
                None,
                None,
            );
            let invoice_id = insert_invoice(
                &connection,
                user_id,
                invoice_type,
                number,
                Some("pending"),
                now,
            );
            let line_id =
                insert_invoice_line(&connection, invoice_id, Some(product_id), product_id, now);
            connection
                .execute(
                    "INSERT INTO lots (productId, origin, arrivalOrder, initialMilli, remainingMilli, unitCostCents, createdAt, updatedAt)
                     VALUES (?1, 'initial', 1, 10_000, 6_000, 100, ?2, ?2)",
                    (product_id, format_utc_timestamp(now)),
                )
                .unwrap();
            let lot_id = connection.last_insert_rowid();
            connection
                .execute(
                    "INSERT INTO consumptions (invoiceLineId, lotId, quantityMilli, unitCostCents, createdAt)
                     VALUES (?1, ?2, 4_000, 100, ?3)",
                    (line_id, lot_id, format_utc_timestamp(now)),
                )
                .unwrap();

            delete_invoice_inner(&mut connection, now, invoice_id).unwrap();

            let stock: i64 = connection
                .query_row(
                    "SELECT quantityMilli FROM products WHERE id = ?1",
                    [product_id],
                    |row| row.get(0),
                )
                .unwrap();
            let restored_lot: i64 = connection
                .query_row(
                    "SELECT remainingMilli FROM lots WHERE id = ?1",
                    [lot_id],
                    |row| row.get(0),
                )
                .unwrap();
            let reversal = connection
                .query_row(
                    "SELECT quantityMilli, invoiceNumber, type FROM productMovements WHERE invoiceNumber = ?1",
                    [number],
                    |row| Ok((row.get::<_, i64>(0)?, row.get::<_, String>(1)?, row.get::<_, String>(2)?)),
                )
                .unwrap();
            assert_eq!(stock, 10_000);
            assert_eq!(restored_lot, 10_000);
            assert_eq!(reversal, (4_000, number.to_owned(), "reversal".to_owned()));
        }
    }

    #[test]
    fn delete_cancelled_entry_without_restored_stock_does_not_change_inventory() {
        let now = Utc.with_ymd_and_hms(2026, 10, 4, 12, 0, 0).unwrap();
        let mut connection = setup_test_db();
        let user_id = create_test_user(
            &connection,
            "deletecancelled",
            "contador",
            "+5350000126",
            "12345678915",
            "ValidPass1!",
        );
        create_test_session(&connection, user_id, false, now, now);
        let product_id = insert_product_fixture(
            &connection,
            "Producto cancelado",
            None,
            None,
            100,
            0,
            0,
            "active",
            None,
            None,
        );
        let invoice_id =
            insert_invoice(&connection, user_id, "E", "E-00001", Some("cancelled"), now);
        let line_id =
            insert_invoice_line(&connection, invoice_id, Some(product_id), product_id, now);
        connection
            .execute(
                "INSERT INTO lots (productId, origin, invoiceLineId, arrivalOrder, initialMilli, remainingMilli, unitCostCents, createdAt, updatedAt)
                 VALUES (?1, 'entry', ?2, 1, 10_000, 0, 100, ?3, ?3)",
                (product_id, line_id, format_utc_timestamp(now)),
            )
            .unwrap();

        delete_invoice_inner(&mut connection, now, invoice_id).unwrap();

        let stock: i64 = connection
            .query_row(
                "SELECT quantityMilli FROM products WHERE id = ?1",
                [product_id],
                |row| row.get(0),
            )
            .unwrap();
        let movement_count: i64 = connection
            .query_row(
                "SELECT COUNT(*) FROM productMovements WHERE invoiceNumber = 'E-00001'",
                [],
                |row| row.get(0),
            )
            .unwrap();
        assert_eq!(stock, 0);
        assert_eq!(movement_count, 0);
    }

    #[test]
    fn deleting_cancelled_entry_with_restored_stock_preserves_inventory_cache() {
        let now = Utc.with_ymd_and_hms(2026, 10, 4, 12, 0, 0).unwrap();
        let mut connection = setup_test_db();
        let user_id = create_test_user(
            &connection,
            "deleteha1",
            "contador",
            "+5350000127",
            "12345678916",
            "ValidPass1!",
        );
        create_test_session(&connection, user_id, false, now, now);
        let product_id = insert_product_fixture(
            &connection,
            "Producto H-A1",
            None,
            None,
            100,
            0,
            6_000,
            "active",
            None,
            None,
        );
        let entry_id = insert_invoice(&connection, user_id, "E", "E-00001", Some("pending"), now);
        let entry_line_id =
            insert_invoice_line(&connection, entry_id, Some(product_id), product_id, now);
        connection
            .execute(
                "INSERT INTO lots (productId, origin, invoiceLineId, arrivalOrder, initialMilli, remainingMilli, unitCostCents, createdAt, updatedAt)
                 VALUES (?1, 'entry', ?2, 1, 10_000, 6_000, 100, ?3, ?3)",
                (product_id, entry_line_id, format_utc_timestamp(now)),
            )
            .unwrap();
        let lot_id = connection.last_insert_rowid();
        let sale_id = insert_invoice(&connection, user_id, "V", "V-00001", Some("pending"), now);
        let sale_line_id =
            insert_invoice_line(&connection, sale_id, Some(product_id), product_id, now);
        connection
            .execute(
                "INSERT INTO consumptions (invoiceLineId, lotId, quantityMilli, unitCostCents, createdAt)
                 VALUES (?1, ?2, 4_000, 100, ?3)",
                (sale_line_id, lot_id, format_utc_timestamp(now)),
            )
            .unwrap();

        change_invoice_status_inner(&mut connection, now, entry_id, "cancelled", None).unwrap();
        change_invoice_status_inner(&mut connection, now, sale_id, "cancelled", None).unwrap();
        let restored: i64 = connection
            .query_row(
                "SELECT remainingMilli FROM lots WHERE id = ?1",
                [lot_id],
                |row| row.get(0),
            )
            .unwrap();
        assert_eq!(restored, 4_000);

        delete_invoice_inner(&mut connection, now, entry_id).unwrap();

        let cache: i64 = connection
            .query_row(
                "SELECT quantityMilli FROM products WHERE id = ?1",
                [product_id],
                |row| row.get(0),
            )
            .unwrap();
        let lot_sum: i64 = connection
            .query_row(
                "SELECT COALESCE(SUM(remainingMilli), 0) FROM lots WHERE productId = ?1",
                [product_id],
                |row| row.get(0),
            )
            .unwrap();
        let deletion_reversal: (i64, i64) = connection
            .query_row(
                "SELECT quantityMilli, unitPriceCents FROM productMovements WHERE invoiceNumber = 'E-00001' AND stockAfterMilli = 0 ORDER BY id DESC LIMIT 1",
                [],
                |row| Ok((row.get(0)?, row.get(1)?)),
            )
            .unwrap();
        assert_eq!((cache, lot_sum), (0, 0));
        assert_eq!(deletion_reversal, (-4_000, 100));
    }

    #[test]
    fn deleting_invoice_skips_missing_products_warns_and_processes_other_lines() {
        let now = Utc.with_ymd_and_hms(2026, 10, 4, 12, 0, 0).unwrap();
        let mut connection = setup_test_db();
        let user_id = create_test_user(
            &connection,
            "deletemissingproduct",
            "contador",
            "+5350000130",
            "12345678919",
            "ValidPass1!",
        );
        create_test_session(&connection, user_id, false, now, now);
        let removed_product_id = insert_product_fixture(
            &connection,
            "Producto eliminado",
            None,
            None,
            100,
            0,
            10_000,
            "active",
            None,
            None,
        );
        let retained_product_id = insert_product_fixture(
            &connection,
            "Producto conservado",
            None,
            None,
            100,
            0,
            5_000,
            "active",
            None,
            None,
        );
        let invoice_id =
            insert_invoice(&connection, user_id, "E", "E-00001", Some("cancelled"), now);
        let removed_line_id = insert_invoice_line(
            &connection,
            invoice_id,
            Some(removed_product_id),
            removed_product_id,
            now,
        );
        connection
            .execute(
                "INSERT INTO invoiceLines (invoiceId, position, productId, productIdOriginal, productName,
                    productNameNormalized, quantityMilli, unitPriceCents, subtotalCents, createdAt, updatedAt)
                 VALUES (?1, 2, ?2, ?2, 'Producto conservado', 'producto conservado', 5_000, 100, 500, ?3, ?3)",
                (invoice_id, retained_product_id, format_utc_timestamp(now)),
            )
            .unwrap();
        let retained_line_id = connection.last_insert_rowid();
        for (product_id, line_id, arrival, quantity) in [
            (removed_product_id, removed_line_id, 1, 10_000),
            (retained_product_id, retained_line_id, 2, 5_000),
        ] {
            connection
                .execute(
                    "INSERT INTO lots (productId, origin, invoiceLineId, arrivalOrder, initialMilli, remainingMilli, unitCostCents, createdAt, updatedAt)
                     VALUES (?1, 'entry', ?2, ?3, ?4, ?4, 100, ?5, ?5)",
                    (product_id, line_id, arrival, quantity, format_utc_timestamp(now)),
                )
                .unwrap();
        }
        connection
            .execute("DELETE FROM products WHERE id = ?1", [removed_product_id])
            .unwrap();

        let result = delete_invoice_inner(&mut connection, now, invoice_id).unwrap();

        assert_eq!(
            result.expired_cleanup_warning.as_deref(),
            Some("Algunos productos de la factura ya no existen; no se modificó su inventario.")
        );
        let retained_stock: i64 = connection
            .query_row(
                "SELECT quantityMilli FROM products WHERE id = ?1",
                [retained_product_id],
                |row| row.get(0),
            )
            .unwrap();
        assert_eq!(retained_stock, 0);
        let reversal_count: i64 = connection
            .query_row(
                "SELECT COUNT(*) FROM productMovements WHERE invoiceNumber = 'E-00001' AND productIdOriginal = ?1 AND type = 'reversal'",
                [retained_product_id],
                |row| row.get(0),
            )
            .unwrap();
        assert_eq!(reversal_count, 1);
    }

    #[test]
    fn delete_invoice_removes_attachment_and_continues_when_file_removal_fails() {
        let now = Utc.with_ymd_and_hms(2026, 10, 4, 12, 0, 0).unwrap();
        for (username, phone, national_id, make_directory) in [
            ("deletefile", "+5350000128", "12345678917", false),
            ("deletefilefail", "+5350000129", "12345678918", true),
        ] {
            let app_data_dir = temp_dir();
            let attachment_dir = app_data_dir.join("attachments");
            fs::create_dir_all(&attachment_dir).unwrap();
            let attachment_path = attachment_dir.join("invoice.pdf");
            if make_directory {
                fs::create_dir(&attachment_path).unwrap();
            } else {
                fs::write(&attachment_path, b"%PDF-test").unwrap();
            }

            let mut connection = setup_test_db();
            let user_id = create_test_user(
                &connection,
                username,
                "contador",
                phone,
                national_id,
                "ValidPass1!",
            );
            create_test_session(&connection, user_id, false, now, now);
            let invoice_id =
                insert_invoice(&connection, user_id, "E", "E-00001", Some("pending"), now);
            connection
                .execute(
                    "UPDATE invoices SET attachmentFileName = 'invoice.pdf', attachmentOriginalName = 'factura.pdf',
                        attachmentSizeBytes = 9, attachmentUploadedAt = ?1 WHERE id = ?2",
                    (format_utc_timestamp(now), invoice_id),
                )
                .unwrap();

            delete_invoice_inner_with_app_data_dir(
                &mut connection,
                now,
                invoice_id,
                Some(app_data_dir.as_path()),
            )
            .unwrap();

            if make_directory {
                assert!(attachment_path.is_dir());
                let log = fs::read_to_string(app_data_dir.join("errors.log")).unwrap();
                assert!(log.contains("attachment delete failed"));
            } else {
                assert!(!attachment_path.exists());
            }
            fs::remove_dir_all(app_data_dir).unwrap();
        }
    }

    #[test]
    fn delete_invoice_requires_session_and_allows_both_roles() {
        assert_requires_session(|connection, now| delete_invoice_inner(connection, now, 1));
        assert_allows_both_roles(|connection, now| {
            let user_id: i64 = connection
                .query_row("SELECT userId FROM sessions WHERE id = 1", [], |row| {
                    row.get(0)
                })
                .unwrap();
            let invoice_id =
                insert_invoice(connection, user_id, "E", "E-00001", Some("pending"), now);
            delete_invoice_inner(connection, now, invoice_id)
        });
    }

    #[test]
    fn change_invoice_status_updates_audit_fields_and_reverses_cancelled_sale() {
        let now = Utc.with_ymd_and_hms(2026, 10, 4, 12, 0, 0).unwrap();
        let mut connection = setup_test_db();
        let user_id = create_test_user(
            &connection,
            "statususer",
            "contador",
            "+5350000099",
            "12345678999",
            "ValidPass1!",
        );
        create_test_session(&connection, user_id, false, now, now);
        let product_id = insert_product_fixture(
            &connection,
            "Producto de venta",
            None,
            None,
            100,
            0,
            10_000,
            "active",
            None,
            None,
        );
        connection
            .execute(
                "INSERT INTO clients (firstName, lastName, nationalId, phone, createdAt, updatedAt)
                 VALUES ('Ana', 'Pérez', '12345678909', '+5355550100', ?1, ?1)",
                [format_utc_timestamp(now)],
            )
            .unwrap();
        let client_id = connection.last_insert_rowid();
        connection
            .execute(
                "INSERT INTO invoices (type, number, invoiceDate, status, clientId, contactFullName, contactFullNameNormalized,
                    contactCompany, contactNationalId, contactPhone, createdByUserId, createdByName, createdAt, updatedAt)
                 VALUES ('V', 'V-00001', '2026-10-04', 'pending', ?1, 'Ana Pérez', 'ana perez', NULL, '12345678909', '+5355550100', ?2, 'statususer', ?3, ?3)",
                (client_id, user_id, format_utc_timestamp(now)),
            )
            .unwrap();
        let invoice_id = connection.last_insert_rowid();
        connection.execute(
            "INSERT INTO invoiceLines (invoiceId, position, productId, productIdOriginal, productName, productNameNormalized, quantityMilli, unitPriceCents, subtotalCents, createdAt, updatedAt)
             VALUES (?1, 1, ?2, ?2, 'Producto de venta', 'producto de venta', 5_000, 200, 1_000, ?3, ?3)",
            (invoice_id, product_id, format_utc_timestamp(now)),
        ).unwrap();
        let line_id = connection.last_insert_rowid();
        connection
            .execute(
                "INSERT INTO lots (productId, origin, arrivalOrder, initialMilli, remainingMilli, unitCostCents, createdAt, updatedAt)
                 VALUES (?1, 'initial', 1, 5_000, 0, 100, ?2, ?2)",
                (product_id, format_utc_timestamp(now)),
            )
            .unwrap();
        let lot_id = connection.last_insert_rowid();
        connection.execute(
            "INSERT INTO consumptions (invoiceLineId, lotId, quantityMilli, unitCostCents, createdAt)
             VALUES (?1, ?2, 5_000, 100, ?3)",
            (line_id, lot_id, format_utc_timestamp(now)),
        ).unwrap();
        let before: (String, Option<String>, Option<String>) = connection
            .query_row(
                "SELECT status, paymentMethod, paidAt FROM invoices WHERE id = ?1",
                [invoice_id],
                |row| Ok((row.get(0)?, row.get(1)?, row.get(2)?)),
            )
            .unwrap();
        assert_eq!(before.0, "pending");

        let result = change_invoice_status_inner(
            &mut connection,
            now,
            invoice_id,
            "completed",
            Some("cash"),
        )
        .unwrap();
        assert_eq!(result.invoice_id, invoice_id);
        let completed: (String, Option<String>, Option<String>) = connection
            .query_row(
                "SELECT status, paymentMethod, paidAt FROM invoices WHERE id = ?1",
                [invoice_id],
                |row| Ok((row.get(0)?, row.get(1)?, row.get(2)?)),
            )
            .unwrap();
        assert_eq!(completed.0, "completed");
        assert_eq!(completed.1.as_deref(), Some("cash"));
        assert!(completed.2.as_deref().is_some());

        let cancelled =
            change_invoice_status_inner(&mut connection, now, invoice_id, "cancelled", None)
                .unwrap();
        assert_eq!(cancelled.invoice_id, invoice_id);
        let final_state: (String, Option<String>, Option<String>) = connection
            .query_row(
                "SELECT status, paymentMethod, paidAt FROM invoices WHERE id = ?1",
                [invoice_id],
                |row| Ok((row.get(0)?, row.get(1)?, row.get(2)?)),
            )
            .unwrap();
        assert_eq!(final_state.0, "cancelled");
        assert_eq!(final_state.1.as_deref(), Some("cash"));
        assert!(final_state.2.as_deref().is_some());
    }

    #[test]
    fn change_invoice_status_restores_lots_records_reversals_and_clears_payment() {
        let now = Utc.with_ymd_and_hms(2026, 10, 4, 12, 0, 0).unwrap();
        let (mut connection, _, client_id, supplier_id) = setup_sale_fixture(
            "statusflow",
            "12345678931",
            "+5355550131",
            "12345678932",
            "+5355550132",
            now,
        );
        let first = insert_stocked_product(
            &mut connection,
            now,
            supplier_id,
            "Cable UTP",
            100,
            0,
            "10",
            "1,00",
        );
        let second = insert_stocked_product(
            &mut connection,
            now,
            supplier_id,
            "Conector RJ45",
            100,
            0,
            "5",
            "1,00",
        );
        let sale = super::create_sale_invoice_inner(
            &mut connection,
            now,
            sale_input(
                client_id,
                vec![sale_line(first, "4"), sale_line(second, "2")],
                None,
            ),
        )
        .unwrap();
        let stock = |connection: &rusqlite::Connection, product_id: i64| -> (i64, i64) {
            connection
                .query_row(
                    "SELECT (SELECT quantityMilli FROM products WHERE id = ?1),
                            (SELECT COALESCE(SUM(remainingMilli), 0) FROM lots WHERE productId = ?1)",
                    [product_id],
                    |row| Ok((row.get(0)?, row.get(1)?)),
                )
                .unwrap()
        };
        assert_eq!(stock(&connection, first), (6_000, 6_000));
        assert_eq!(stock(&connection, second), (3_000, 3_000));
        let payment = |connection: &rusqlite::Connection| -> (Option<String>, Option<String>) {
            connection
                .query_row(
                    "SELECT paymentMethod, paidAt FROM invoices WHERE id = ?1",
                    [sale.invoice_id],
                    |row| Ok((row.get(0)?, row.get(1)?)),
                )
                .unwrap()
        };

        // Por Cobrar → Cobrada → Por Cobrar: se borran la fecha y el método de pago.
        change_invoice_status_inner(
            &mut connection,
            now,
            sale.invoice_id,
            "completed",
            Some("transfer"),
        )
        .unwrap();
        let paid = payment(&connection);
        assert_eq!(paid.0.as_deref(), Some("transfer"));
        assert!(paid.1.is_some());
        change_invoice_status_inner(&mut connection, now, sale.invoice_id, "pending", None)
            .unwrap();
        assert_eq!(payment(&connection), (None, None));

        // Cancelar devuelve cada consumo a su lote y registra una Reversión por línea.
        change_invoice_status_inner(&mut connection, now, sale.invoice_id, "cancelled", None)
            .unwrap();
        assert_eq!(stock(&connection, first), (10_000, 10_000));
        assert_eq!(stock(&connection, second), (5_000, 5_000));
        let reversals: Vec<(i64, i64)> = connection
            .prepare(
                "SELECT productIdOriginal, quantityMilli FROM productMovements
                 WHERE type = 'reversal' AND invoiceNumber = ?1 ORDER BY productIdOriginal",
            )
            .unwrap()
            .query_map([&sale.number], |row| Ok((row.get(0)?, row.get(1)?)))
            .unwrap()
            .collect::<rusqlite::Result<_>>()
            .unwrap();
        assert_eq!(reversals, vec![(first, 4_000), (second, 2_000)]);
        assert_cache_invariant(&connection, "tras cancelar la venta");

        // Cancelar una compra deja sus lotes a 0 sin borrarlos.
        let entry_id: i64 = connection
            .query_row(
                "SELECT invoiceId FROM invoiceLines WHERE productIdOriginal = ?1",
                [first],
                |row| row.get(0),
            )
            .unwrap();
        let lots_before: i64 = connection
            .query_row("SELECT COUNT(*) FROM lots", [], |row| row.get(0))
            .unwrap();
        change_invoice_status_inner(&mut connection, now, entry_id, "cancelled", None).unwrap();
        let entry_lots: Vec<i64> = connection
            .prepare(
                "SELECT l.remainingMilli FROM lots l
                 JOIN invoiceLines il ON il.id = l.invoiceLineId
                 WHERE il.invoiceId = ?1",
            )
            .unwrap()
            .query_map([entry_id], |row| row.get(0))
            .unwrap()
            .collect::<rusqlite::Result<_>>()
            .unwrap();
        assert_eq!(entry_lots, vec![0]);
        let lots_after: i64 = connection
            .query_row("SELECT COUNT(*) FROM lots", [], |row| row.get(0))
            .unwrap();
        assert_eq!(lots_after, lots_before);
        assert_eq!(stock(&connection, first), (0, 0));
        assert_eq!(stock(&connection, second), (5_000, 5_000));
        assert_cache_invariant(&connection, "tras cancelar la compra");
    }

    #[test]
    fn list_invoices_inner_filters_by_type_status_search_and_date_range() {
        let now = Utc.with_ymd_and_hms(2026, 10, 4, 12, 0, 0).unwrap();
        let mut connection = setup_test_db();
        let user_id = create_test_user(
            &connection,
            "billinguser",
            "contador",
            "+5350000020",
            "12345678920",
            "ValidPass1!",
        );
        create_test_session(&connection, user_id, false, now, now);

        connection
            .execute(
                "INSERT INTO invoices (type, number, invoiceDate, status, contactFullName, contactFullNameNormalized,
                    contactCompany, contactNationalId, contactPhone, createdByUserId, createdByName, createdAt, updatedAt)
                 VALUES ('V', 'V-00001', '2026-10-01', 'pending', 'Ana Pérez', 'ana perez', 'Goyma', '12345678901', '+5355551234', ?1, 'admin', ?2, ?2)",
                (user_id, format_utc_timestamp(now)),
            )
            .unwrap();
        connection
            .execute(
                "INSERT INTO invoices (type, number, invoiceDate, status, paidAt, contactFullName, contactFullNameNormalized,
                    contactCompany, contactNationalId, contactPhone, createdByUserId, createdByName, createdAt, updatedAt)
                 VALUES ('S', 'S-00001', '2026-10-02', 'completed', ?2, 'Luis García', 'luis garcia', 'Logos', '12345678902', '+5355552233', ?1, 'admin', ?2, ?2)",
                (user_id, format_utc_timestamp(now)),
            )
            .unwrap();
        connection
            .execute(
                "INSERT INTO invoices (type, number, invoiceDate, status, contactFullName, contactFullNameNormalized,
                    contactCompany, contactNationalId, contactPhone, associatedNumber, associatedType,
                    createdByUserId, createdByName, createdAt, updatedAt)
                 VALUES ('GG', 'GG-00001', '2026-10-03', 'pending', 'María Vega', 'maria vega', 'Goyma', '12345678903', '+5355553344', 'V-00001', 'V', ?1, 'admin', ?2, ?2)",
                (user_id, format_utc_timestamp(now)),
            )
            .unwrap();
        connection
            .execute(
                "INSERT INTO invoices (type, number, invoiceDate, status, contactFullName, contactFullNameNormalized,
                    contactCompany, contactNationalId, contactPhone, associatedNumber, associatedType,
                    createdByUserId, createdByName, createdAt, updatedAt)
                 VALUES ('GI', 'GI-00001', '2026-10-03', 'pending', 'Luis García', 'luis garcia', 'Logos', '12345678906', '+5355556677', 'S-00001', 'S', ?1, 'admin', ?2, ?2)",
                (user_id, format_utc_timestamp(now)),
            )
            .unwrap();
        connection
            .execute(
                "INSERT INTO invoices (type, number, invoiceDate, status, contactFullName, contactFullNameNormalized,
                    contactCompany, contactNationalId, contactPhone, createdByUserId, createdByName, createdAt, updatedAt)
                 VALUES ('P', 'P-00001', '2026-10-04', NULL, NULL, NULL, NULL, '12345678904', '+5355554455', ?1, 'admin', ?2, ?2)",
                (user_id, format_utc_timestamp(now)),
            )
            .unwrap();
        connection
            .execute(
                "INSERT INTO invoices (type, number, invoiceDate, status, contactFullName, contactFullNameNormalized,
                    contactCompany, contactNationalId, contactPhone, createdByUserId, createdByName, createdAt, updatedAt)
                 VALUES ('S', 'S-00002', '2026-09-30', 'cancelled', 'Ana Pérez', 'ana perez', 'Goyma', '12345678905', '+5355551234', ?1, 'admin', ?2, ?2)",
                (user_id, format_utc_timestamp(now)),
            )
            .unwrap();
        connection
            .execute(
                "INSERT INTO invoiceLines (invoiceId, position, productIdOriginal, productName, productNameNormalized, quantityMilli, unitPriceCents, subtotalCents, createdAt, updatedAt)
                 VALUES (1, 1, 1001, 'Producto A', 'producto a', 1000, 200, 200, ?1, ?1)",
                [format_utc_timestamp(now)],
            )
            .unwrap();
        connection
            .execute(
                "INSERT INTO invoiceLines (invoiceId, position, productIdOriginal, productName, productNameNormalized, quantityMilli, unitPriceCents, subtotalCents, createdAt, updatedAt)
                 VALUES (2, 1, 1002, 'Producto B', 'producto b', 3000, 500, 1500, ?1, ?1)",
                [format_utc_timestamp(now)],
            )
            .unwrap();

        let all = list_invoices_inner(&mut connection, now, InvoiceFilters::default()).unwrap();
        assert_eq!(all.len(), 6);
        for item in &all {
            assert_eq!(
                item.associated_invoice.is_some(),
                matches!(item.invoice_type.as_str(), "GG" | "GI")
            );
        }

        let by_type = list_invoices_inner(
            &mut connection,
            now,
            InvoiceFilters {
                invoice_type: Some("V".to_string()),
                ..Default::default()
            },
        )
        .unwrap();
        assert_eq!(by_type.len(), 1);
        assert_eq!(by_type[0].number, "V-00001");

        let by_status = list_invoices_inner(
            &mut connection,
            now,
            InvoiceFilters {
                status: Some("pendientes".to_string()),
                ..Default::default()
            },
        )
        .unwrap();
        assert!(by_status
            .iter()
            .all(|item| item.status.as_deref() == Some("pending")));
        assert!(!by_status.iter().any(|item| item.number == "P-00001"));

        let by_completed = list_invoices_inner(
            &mut connection,
            now,
            InvoiceFilters {
                status: Some("completadas".to_string()),
                ..Default::default()
            },
        )
        .unwrap();
        assert_eq!(by_completed.len(), 1);
        assert_eq!(by_completed[0].number, "S-00001");

        let by_cancelled = list_invoices_inner(
            &mut connection,
            now,
            InvoiceFilters {
                status: Some("canceladas".to_string()),
                ..Default::default()
            },
        )
        .unwrap();
        assert_eq!(by_cancelled.len(), 1);
        assert_eq!(by_cancelled[0].number, "S-00002");

        let by_search = list_invoices_inner(
            &mut connection,
            now,
            InvoiceFilters {
                search: Some("ana".to_string()),
                ..Default::default()
            },
        )
        .unwrap();
        assert_eq!(by_search[0].number, "V-00001");

        for (query, expected_number) in [
            ("Logos", "S-00001"),
            ("12345678903", "GG-00001"),
            ("+53 5555 3344", "GG-00001"),
        ] {
            let matches = list_invoices_inner(
                &mut connection,
                now,
                InvoiceFilters {
                    search: Some(query.to_owned()),
                    ..Default::default()
                },
            )
            .unwrap();
            assert!(matches.iter().any(|item| item.number == expected_number));
        }

        let by_number = list_invoices_inner(
            &mut connection,
            now,
            InvoiceFilters {
                search: Some("gg-00001".to_string()),
                ..Default::default()
            },
        )
        .unwrap();
        assert_eq!(by_number.len(), 1);
        assert_eq!(by_number[0].number, "GG-00001");

        let by_date = list_invoices_inner(
            &mut connection,
            now,
            InvoiceFilters {
                from_date: Some("2026-10-02".to_string()),
                to_date: Some("2026-10-03".to_string()),
                ..Default::default()
            },
        )
        .unwrap();
        assert_eq!(
            by_date
                .iter()
                .map(|item| item.number.as_str())
                .collect::<Vec<_>>(),
            vec!["GI-00001", "GG-00001", "S-00001"]
        );

        let inverted = list_invoices_inner(
            &mut connection,
            now,
            InvoiceFilters {
                from_date: Some("2026-10-03".to_string()),
                to_date: Some("2026-10-02".to_string()),
                ..Default::default()
            },
        );
        let inverted_error = inverted.unwrap_err();
        assert_eq!(inverted_error.code, "validationFailed");
        assert_eq!(
            inverted_error.message,
            "La fecha inicial no puede ser posterior a la final"
        );

        let explicit_loss = list_invoices_inner(
            &mut connection,
            now,
            InvoiceFilters {
                invoice_type: Some("P".to_string()),
                ..Default::default()
            },
        )
        .unwrap();
        assert_eq!(explicit_loss[0].number, "P-00001");
        assert!(explicit_loss[0].status.is_none());

        let associated = list_invoices_inner(
            &mut connection,
            now,
            InvoiceFilters {
                invoice_type: Some("GG".to_string()),
                ..Default::default()
            },
        )
        .unwrap();
        assert_eq!(
            associated[0].associated_invoice.as_ref().unwrap().number,
            "V-00001"
        );
        let installation = list_invoices_inner(
            &mut connection,
            now,
            InvoiceFilters {
                invoice_type: Some("GI".to_string()),
                ..Default::default()
            },
        )
        .unwrap();
        assert_eq!(
            installation[0].associated_invoice.as_ref().unwrap().number,
            "S-00001"
        );

        let expired_product_id = insert_product_fixture(
            &connection,
            "Producto vencido",
            None,
            None,
            100,
            0,
            1_000,
            "discontinued",
            Some(&format_utc_timestamp(now - chrono::Duration::days(61))),
            None,
        );
        list_invoices_inner(&mut connection, now, InvoiceFilters::default()).unwrap();
        let expired_product_exists: bool = connection
            .query_row(
                "SELECT EXISTS(SELECT 1 FROM products WHERE id = ?1)",
                [expired_product_id],
                |row| row.get(0),
            )
            .unwrap();
        assert!(!expired_product_exists);
    }

    #[test]
    fn list_invoices_returns_product_and_quantity_only_for_losses() {
        // FP-01/RF-087: la lista de pérdidas muestra producto y cantidad de su
        // única línea y busca por producto; los demás tipos no los llevan.
        let now = Utc.with_ymd_and_hms(2026, 10, 4, 12, 0, 0).unwrap();
        let mut connection = setup_test_db();
        let user_id = create_test_user(
            &connection,
            "lossuser",
            "contador",
            "+5350000031",
            "12345678931",
            "ValidPass1!",
        );
        create_test_session(&connection, user_id, false, now, now);
        let timestamp = format_utc_timestamp(now);
        connection
            .execute(
                "INSERT INTO invoices (type, number, invoiceDate, status, contactFullName,
                    contactFullNameNormalized, contactNationalId, contactPhone, createdByUserId,
                    createdByName, createdAt, updatedAt)
                 VALUES ('E', 'E-00001', '2026-10-01', 'pending', 'Proveedor Uno', 'proveedor uno',
                    '12345678901', '+5355551234', ?1, 'admin', ?2, ?2)",
                (user_id, timestamp.clone()),
            )
            .unwrap();
        connection
            .execute(
                "INSERT INTO invoices (type, number, invoiceDate, status, createdByUserId,
                    createdByName, createdAt, updatedAt)
                 VALUES ('P', 'P-00001', '2026-10-02', NULL, ?1, 'admin', ?2, ?2)",
                (user_id, timestamp.clone()),
            )
            .unwrap();
        for (invoice_id, product_id, name, normalized, quantity_milli) in [
            (1, 1001, "Producto A", "producto a", 2000),
            (2, 1002, "Cable Coaxial", "cable coaxial", 1500),
        ] {
            connection
                .execute(
                    "INSERT INTO invoiceLines (invoiceId, position, productIdOriginal, productName,
                        productNameNormalized, quantityMilli, unitPriceCents, subtotalCents,
                        createdAt, updatedAt)
                     VALUES (?1, 1, ?2, ?3, ?4, ?5, 200, 300, ?6, ?6)",
                    (
                        invoice_id,
                        product_id,
                        name,
                        normalized,
                        quantity_milli,
                        timestamp.clone(),
                    ),
                )
                .unwrap();
        }

        let all = list_invoices_inner(&mut connection, now, InvoiceFilters::default()).unwrap();
        let loss = all.iter().find(|item| item.number == "P-00001").unwrap();
        assert_eq!(loss.product_name.as_deref(), Some("Cable Coaxial"));
        assert_eq!(
            loss.quantity.as_deref(),
            Some(crate::models::product::format_quantity_milli(1500).as_str())
        );
        assert_eq!(loss.total_cents, 300);
        let entry = all.iter().find(|item| item.number == "E-00001").unwrap();
        assert_eq!(entry.product_name, None);
        assert_eq!(entry.quantity, None);

        let by_product = list_invoices_inner(
            &mut connection,
            now,
            InvoiceFilters {
                invoice_type: Some("P".to_owned()),
                search: Some("coaxial".to_owned()),
                ..InvoiceFilters::default()
            },
        )
        .unwrap();
        assert_eq!(
            by_product
                .iter()
                .map(|item| item.number.as_str())
                .collect::<Vec<_>>(),
            vec!["P-00001"]
        );
    }

    #[test]
    fn list_invoices_returns_all_types_in_date_id_order_for_general_list() {
        // FG-01: sin filtro de tipo devuelve los seis tipos con los campos
        // que pinta la lista general y que pagina el paginador global
        // (número, tipo, contacto, fecha, total, estado, asociada).
        let now = Utc.with_ymd_and_hms(2026, 10, 4, 12, 0, 0).unwrap();
        let mut connection = setup_test_db();
        let user_id = create_test_user(
            &connection,
            "generaluser",
            "contador",
            "+5350000030",
            "12345678930",
            "ValidPass1!",
        );
        create_test_session(&connection, user_id, false, now, now);
        let timestamp = format_utc_timestamp(now);
        for (invoice_type, number, date, status) in [
            ("E", "E-00001", "2026-10-01", Some("pending")),
            ("S", "S-00001", "2026-10-02", Some("completed")),
            ("V", "V-00001", "2026-10-03", Some("pending")),
            ("GG", "GG-00001", "2026-10-03", Some("pending")),
            ("GI", "GI-00001", "2026-09-29", Some("pending")),
            ("P", "P-00001", "2026-10-04", None),
            ("V", "V-00002", "2026-09-28", Some("cancelled")),
        ] {
            if invoice_type == "P" {
                connection
                    .execute(
                        "INSERT INTO invoices (type, number, invoiceDate, status, createdByUserId,
                            createdByName, createdAt, updatedAt)
                         VALUES ('P', ?1, ?2, NULL, ?3, 'admin', ?4, ?4)",
                        (number, date, user_id, timestamp.clone()),
                    )
                    .unwrap();
            } else {
                connection
                    .execute(
                        "INSERT INTO invoices (type, number, invoiceDate, status, contactFullName,
                            contactFullNameNormalized, contactNationalId, contactPhone, associatedNumber,
                            associatedType, paidAt, createdByUserId, createdByName, createdAt, updatedAt)
                         VALUES (?1, ?2, ?3, ?4, 'Proveedor Uno', 'proveedor uno', '12345678901',
                            '+5355551234',
                            CASE ?1 WHEN 'GG' THEN 'V-00001' WHEN 'GI' THEN 'S-00001' END,
                            CASE ?1 WHEN 'GG' THEN 'V' WHEN 'GI' THEN 'S' END,
                            CASE WHEN ?4 = 'completed' THEN ?6 END,
                            ?5, 'admin', ?6, ?6)",
                        (invoice_type, number, date, status, user_id, timestamp.clone()),
                    )
                    .unwrap();
            }
        }
        connection
            .execute(
                "INSERT INTO invoiceLines (invoiceId, position, productIdOriginal, productName,
                    productNameNormalized, quantityMilli, unitPriceCents, subtotalCents, createdAt, updatedAt)
                 VALUES (1, 1, 1001, 'Producto A', 'producto a', 1000, 500, 500, ?1, ?1)",
                [timestamp],
            )
            .unwrap();

        let all = list_invoices_inner(&mut connection, now, InvoiceFilters::default()).unwrap();
        assert_eq!(
            all.iter()
                .map(|item| item.number.as_str())
                .collect::<Vec<_>>(),
            vec!["P-00001", "GG-00001", "V-00001", "S-00001", "E-00001", "GI-00001", "V-00002"]
        );

        // La pérdida viaja sin contacto ni estado; el resto, completos.
        let loss = all.iter().find(|item| item.number == "P-00001").unwrap();
        assert_eq!(loss.contact_name, None);
        assert_eq!(loss.status, None);
        assert_eq!(loss.total_cents, 0);
        let entry = all.iter().find(|item| item.number == "E-00001").unwrap();
        assert_eq!(entry.contact_name.as_deref(), Some("Proveedor Uno"));
        assert_eq!(entry.total_cents, 500);
        assert_eq!(entry.status.as_deref(), Some("pending"));
        for item in &all {
            assert_eq!(
                item.associated_invoice.is_some(),
                matches!(item.invoice_type.as_str(), "GG" | "GI")
            );
        }
        let guarantee = all.iter().find(|item| item.number == "GG-00001").unwrap();
        assert_eq!(
            guarantee.associated_invoice.as_ref().unwrap().number,
            "V-00001"
        );

        // Agrupación por estado con todos los tipos; las pérdidas se excluyen.
        let pending = list_invoices_inner(
            &mut connection,
            now,
            InvoiceFilters {
                status: Some("pendientes".to_string()),
                ..Default::default()
            },
        )
        .unwrap();
        assert_eq!(
            pending
                .iter()
                .map(|item| item.number.as_str())
                .collect::<Vec<_>>(),
            vec!["GG-00001", "V-00001", "E-00001", "GI-00001"]
        );
        let completed = list_invoices_inner(
            &mut connection,
            now,
            InvoiceFilters {
                status: Some("completadas".to_string()),
                ..Default::default()
            },
        )
        .unwrap();
        assert_eq!(
            completed
                .iter()
                .map(|item| item.number.as_str())
                .collect::<Vec<_>>(),
            vec!["S-00001"]
        );
        let cancelled = list_invoices_inner(
            &mut connection,
            now,
            InvoiceFilters {
                status: Some("canceladas".to_string()),
                ..Default::default()
            },
        )
        .unwrap();
        assert_eq!(
            cancelled
                .iter()
                .map(|item| item.number.as_str())
                .collect::<Vec<_>>(),
            vec!["V-00002"]
        );

        // El tipo de entrada también filtra explícito (ausente en §3.1.1).
        let entries = list_invoices_inner(
            &mut connection,
            now,
            InvoiceFilters {
                invoice_type: Some("E".to_string()),
                ..Default::default()
            },
        )
        .unwrap();
        assert_eq!(
            entries
                .iter()
                .map(|item| item.number.as_str())
                .collect::<Vec<_>>(),
            vec!["E-00001"]
        );
    }

    #[test]
    fn get_invoice_inner_returns_detail_with_type_specific_fields() {
        let now = Utc.with_ymd_and_hms(2026, 10, 4, 12, 0, 0).unwrap();
        let mut connection = setup_test_db();
        let user_id = create_test_user(
            &connection,
            "billinguser",
            "contador",
            "+5350000020",
            "12345678920",
            "ValidPass1!",
        );
        create_test_session(&connection, user_id, false, now, now);

        connection
            .execute(
                "INSERT INTO invoices (type, number, invoiceDate, status, paymentMethod, paidAt, contactFullName, contactFullNameNormalized,
                    contactCompany, contactNationalId, contactPhone, contactEmail, notes, attachmentFileName, attachmentOriginalName,
                    attachmentSizeBytes, attachmentUploadedAt, createdByUserId, createdByName, createdAt, updatedAt)
                 VALUES ('E', 'E-00001', '2026-10-01', 'completed', 'transfer', ?2, 'Acme S.A.', 'acme s a', 'Acme', '12345678901', '+5355551234', 'facturas@acme.cu', 'Compra de prueba', 'factura.pdf', 'compra-original.pdf', 2048, ?2, ?1, 'admin', ?2, ?2)",
                (user_id, format_utc_timestamp(now)),
            )
            .unwrap();
        connection
            .execute(
                "INSERT INTO invoiceLines (invoiceId, position, productIdOriginal, productName, productNameNormalized, quantityMilli, unitPriceCents, subtotalCents, createdAt, updatedAt)
                 VALUES (1, 1, 1001, 'Producto A', 'producto a', 2000, 250, 500, ?1, ?1)",
                [format_utc_timestamp(now)],
            )
            .unwrap();

        let expired_product_id = insert_product_fixture(
            &connection,
            "Producto vencido",
            None,
            None,
            100,
            0,
            1_000,
            "discontinued",
            Some(&format_utc_timestamp(now - chrono::Duration::days(61))),
            None,
        );
        let entry = get_invoice_inner(&mut connection, now, 1).unwrap();
        assert_eq!(entry.invoice_type, "E");
        assert_eq!(entry.contact_full_name, Some("Acme S.A.".to_string()));
        assert_eq!(entry.attachment_file_name.as_deref(), Some("factura.pdf"));
        assert_eq!(
            entry.attachment_original_name.as_deref(),
            Some("compra-original.pdf")
        );
        assert_eq!(entry.attachment_size_bytes, Some(2048));
        assert_eq!(
            entry.attachment_uploaded_at.as_deref(),
            Some(format_utc_timestamp(now).as_str())
        );
        assert_eq!(entry.lines[0].product_name, "Producto A");
        assert_eq!(entry.lines[0].quantity, "2");
        assert_eq!(entry.total_cents, 500);
        let expired_product_exists: bool = connection
            .query_row(
                "SELECT EXISTS(SELECT 1 FROM products WHERE id = ?1)",
                [expired_product_id],
                |row| row.get(0),
            )
            .unwrap();
        assert!(!expired_product_exists);

        connection
            .execute(
                "INSERT INTO invoices (type, number, invoiceDate, status, contactFullName, contactFullNameNormalized,
                    contactCompany, contactNationalId, contactPhone, contactEmail, notes, createdByUserId, createdByName, createdAt, updatedAt)
                 VALUES ('V', 'V-00001', '2026-10-02', 'pending', 'Ana Pérez', 'ana perez', 'Goyma', '12345678902', '+5355552233', 'ana@goyma.cu', 'Venta de prueba', ?1, 'admin', ?2, ?2)",
                (user_id, format_utc_timestamp(now)),
            )
            .unwrap();
        connection
            .execute(
                "INSERT INTO invoiceLines (invoiceId, position, productIdOriginal, productName, productNameNormalized, quantityMilli, unitPriceCents, subtotalCents, categoryId, categoryName, costAtSaleCents, createdAt, updatedAt)
                 VALUES (2, 1, 1002, 'Producto B', 'producto b', 1500, 400, 600, 7, 'Servicios', 300, ?1, ?1)",
                [format_utc_timestamp(now)],
            )
            .unwrap();

        let sale = get_invoice_inner(&mut connection, now, 2).unwrap();
        assert_eq!(sale.lines[0].category_name, Some("Servicios".to_string()));
        assert_eq!(sale.lines[0].cost_at_sale_cents, Some(300));

        connection
            .execute(
                "INSERT INTO invoices (type, number, invoiceDate, status, contactFullName, contactFullNameNormalized,
                    contactCompany, contactNationalId, contactPhone, contactEmail, createdByUserId, createdByName, createdAt, updatedAt)
                 VALUES ('S', 'S-00001', '2026-10-02', 'pending', 'Ana Pérez', 'ana perez', 'Goyma', '12345678902', '+5355552233', 'ana@goyma.cu', ?1, 'admin', ?2, ?2)",
                (user_id, format_utc_timestamp(now)),
            )
            .unwrap();
        connection
            .execute(
                "INSERT INTO invoiceLines (invoiceId, position, productIdOriginal, productName, productNameNormalized, quantityMilli, unitPriceCents, subtotalCents, categoryId, categoryName, costAtSaleCents, createdAt, updatedAt)
                 VALUES (3, 1, 1003, 'Servicio B', 'servicio b', 1000, 600, 600, 8, 'Instalación', 350, ?1, ?1)",
                [format_utc_timestamp(now)],
            )
            .unwrap();
        let service = get_invoice_inner(&mut connection, now, 3).unwrap();
        assert_eq!(
            service.lines[0].category_name,
            Some("Instalación".to_string())
        );
        assert_eq!(service.lines[0].cost_at_sale_cents, Some(350));

        connection
            .execute(
                "INSERT INTO invoices (type, number, invoiceDate, status, contactFullName, contactFullNameNormalized,
                    contactCompany, contactNationalId, contactPhone, contactEmail, associatedNumber, associatedType,
                    createdByUserId, createdByName, createdAt, updatedAt)
                 VALUES ('GG', 'GG-00001', '2026-10-03', 'pending', 'Luis García', 'luis garcia', 'Logos', '12345678903', '+5355553344', 'luis@logos.cu', 'V-00001', 'V', ?1, 'admin', ?2, ?2)",
                (user_id, format_utc_timestamp(now)),
            )
            .unwrap();
        let guarantee = get_invoice_inner(&mut connection, now, 4).unwrap();
        let associated_invoice = guarantee.associated_invoice.unwrap();
        assert_eq!(associated_invoice.number, "V-00001");
        assert_eq!(associated_invoice.invoice_type, "V");

        connection
            .execute(
                "INSERT INTO invoices (type, number, invoiceDate, status, contactFullName, contactFullNameNormalized,
                    contactCompany, contactNationalId, contactPhone, contactEmail, associatedNumber, associatedType,
                    createdByUserId, createdByName, createdAt, updatedAt)
                 VALUES ('GI', 'GI-00001', '2026-10-03', 'pending', 'Ana Pérez', 'ana perez', 'Goyma', '12345678902', '+5355552233', 'ana@goyma.cu', 'S-00001', 'S', ?1, 'admin', ?2, ?2)",
                (user_id, format_utc_timestamp(now)),
            )
            .unwrap();
        let installation = get_invoice_inner(&mut connection, now, 5).unwrap();
        let associated_service = installation.associated_invoice.unwrap();
        assert_eq!(associated_service.number, "S-00001");
        assert_eq!(associated_service.invoice_type, "S");

        connection
            .execute(
                "INSERT INTO invoices (type, number, invoiceDate, status, contactFullName, contactFullNameNormalized,
                    contactCompany, contactNationalId, contactPhone, contactEmail, createdByUserId, createdByName, createdAt, updatedAt)
                 VALUES ('P', 'P-00001', '2026-10-04', NULL, NULL, NULL, NULL, '12345678904', '+5355554455', NULL, ?1, 'admin', ?2, ?2)",
                (user_id, format_utc_timestamp(now)),
            )
            .unwrap();
        let loss = get_invoice_inner(&mut connection, now, 6).unwrap();
        assert!(loss.status.is_none());
        assert_eq!(loss.contact_full_name, None);
        assert_eq!(loss.payment_method, None);

        let missing = get_invoice_inner(&mut connection, now, 9999).unwrap_err();
        assert_eq!(missing.code, "notFound");
        assert_eq!(missing.message, "La factura ya no existe");
    }

    #[test]
    fn get_invoice_inner_exposes_current_ids_and_product_status_for_editing() {
        let now = Utc.with_ymd_and_hms(2026, 10, 4, 12, 0, 0).unwrap();
        let (mut connection, _, client_id, supplier_id) = setup_sale_fixture(
            "editdetail",
            "12345678941",
            "+5355550141",
            "12345678942",
            "+5355550142",
            now,
        );
        let active = insert_stocked_product(
            &mut connection,
            now,
            supplier_id,
            "Cable UTP",
            100,
            0,
            "10",
            "1,00",
        );
        let discontinued = insert_stocked_product(
            &mut connection,
            now,
            supplier_id,
            "Conector RJ45",
            100,
            0,
            "10",
            "1,00",
        );
        let deleted = insert_stocked_product(
            &mut connection,
            now,
            supplier_id,
            "Canaleta",
            100,
            0,
            "10",
            "1,00",
        );
        let sale = super::create_sale_invoice_inner(
            &mut connection,
            now,
            sale_input(
                client_id,
                vec![
                    sale_line(active, "1"),
                    sale_line(discontinued, "1"),
                    sale_line(deleted, "1"),
                ],
                None,
            ),
        )
        .unwrap();
        let guarantee = super::create_guarantee_invoice_inner(
            &mut connection,
            now,
            guarantee_input(
                client_id,
                sale.invoice_id,
                vec![guarantee_line(active, "1")],
                None,
            ),
        )
        .unwrap();
        connection
            .execute(
                "UPDATE products SET status = 'discontinued', discontinuedAt = ?2 WHERE id = ?1",
                (discontinued, format_utc_timestamp(now)),
            )
            .unwrap();
        connection
            .execute("DELETE FROM products WHERE id = ?1", [deleted])
            .unwrap();

        let detail = get_invoice_inner(&mut connection, now, sale.invoice_id).unwrap();
        assert_eq!(detail.contact_id, Some(client_id));
        assert_eq!(detail.associated_invoice_id, None);
        let statuses = detail
            .lines
            .iter()
            .map(|line| (line.product_id_original, line.product_status.as_deref()))
            .collect::<Vec<_>>();
        assert_eq!(
            statuses,
            vec![
                (active, Some("active")),
                (discontinued, Some("discontinued")),
                (deleted, None),
            ]
        );

        let guarantee_detail =
            get_invoice_inner(&mut connection, now, guarantee.invoice_id).unwrap();
        assert_eq!(
            guarantee_detail.associated_invoice_id,
            Some(sale.invoice_id)
        );

        // Sin el cliente, el id desaparece y la copia de texto se conserva.
        connection
            .execute("DELETE FROM clients WHERE id = ?1", [client_id])
            .unwrap();
        let orphan = get_invoice_inner(&mut connection, now, sale.invoice_id).unwrap();
        assert_eq!(orphan.contact_id, None);
        assert!(orphan.contact_full_name.is_some());

        let entry_id: i64 = connection
            .query_row(
                "SELECT invoiceId FROM invoiceLines WHERE productIdOriginal = ?1 ORDER BY id LIMIT 1",
                [active],
                |row| row.get(0),
            )
            .unwrap();
        let entry = get_invoice_inner(&mut connection, now, entry_id).unwrap();
        assert_eq!(entry.contact_id, Some(supplier_id));
        // FE-05: del lote de 10 quedan 8; la venta y la garantía consumieron 2.
        assert_eq!(entry.lines[0].remaining_quantity.as_deref(), Some("8"));
        assert_eq!(entry.lines[0].consumed_quantity.as_deref(), Some("2"));
        // Una venta cancelada devuelve su consumo: ya no cuenta como consumido.
        change_invoice_status_inner(
            &mut connection,
            now,
            guarantee.invoice_id,
            "cancelled",
            None,
        )
        .unwrap();
        let entry = get_invoice_inner(&mut connection, now, entry_id).unwrap();
        assert_eq!(entry.lines[0].remaining_quantity.as_deref(), Some("9"));
        assert_eq!(entry.lines[0].consumed_quantity.as_deref(), Some("1"));
        // Las salidas no tienen lote propio: no traen estos campos.
        assert_eq!(orphan.lines[0].remaining_quantity, None);
        assert_eq!(orphan.lines[0].consumed_quantity, None);
    }

    #[test]
    fn invoice_lists_include_product_count_and_responsible() {
        let now = Utc.with_ymd_and_hms(2026, 10, 4, 12, 0, 0).unwrap();
        let (mut connection, _, client_id, supplier_id) = setup_sale_fixture(
            "listcounter",
            "12345678951",
            "+5355550151",
            "12345678952",
            "+5355550152",
            now,
        );
        let first = insert_stocked_product(
            &mut connection,
            now,
            supplier_id,
            "Cable UTP",
            100,
            0,
            "10",
            "1,00",
        );
        let second = insert_stocked_product(
            &mut connection,
            now,
            supplier_id,
            "Conector RJ45",
            100,
            0,
            "10",
            "1,00",
        );
        super::create_sale_invoice_inner(
            &mut connection,
            now,
            sale_input(
                client_id,
                vec![sale_line(first, "1"), sale_line(second, "2")],
                None,
            ),
        )
        .unwrap();

        // FE-02 y FS-01: cada fila trae su número de productos y su responsable.
        let sales = list_invoices_inner(
            &mut connection,
            now,
            crate::models::invoice::InvoiceFilters {
                invoice_type: Some("V".to_owned()),
                ..Default::default()
            },
        )
        .unwrap();
        assert_eq!(sales[0].product_count, 2);
        assert_eq!(sales[0].created_by_name, "listcounter");
        let found = list_invoices_inner(
            &mut connection,
            now,
            crate::models::invoice::InvoiceFilters {
                search: Some("V-00001".to_owned()),
                ..Default::default()
            },
        )
        .unwrap();
        assert_eq!(found[0].product_count, 2);

        // PV-07: el historial del proveedor trae lo mismo por compra.
        let supplier = crate::commands::suppliers::get_supplier_detail_inner(
            &mut connection,
            now,
            supplier_id,
        )
        .unwrap();
        assert_eq!(supplier.invoice_history.len(), 2);
        assert!(supplier
            .invoice_history
            .iter()
            .all(|item| item.product_count == 1 && item.created_by_name == "listcounter"));
        let client =
            crate::commands::clients::get_client_detail_inner(&mut connection, now, client_id)
                .unwrap();
        assert_eq!(client.invoice_history[0].product_count, 2);
    }

    #[test]
    fn get_invoice_inner_requires_session_and_allows_both_roles() {
        assert_requires_session(|connection, now| get_invoice_inner(connection, now, 1));
        assert_allows_both_roles(|connection, now| get_invoice_inner(connection, now, 1));
    }

    #[test]
    fn list_invoices_inner_requires_session_and_allows_both_roles() {
        assert_requires_session(|connection, now| {
            list_invoices_inner(connection, now, InvoiceFilters::default())
        });

        assert_allows_both_roles(|connection, now| {
            list_invoices_inner(connection, now, InvoiceFilters::default())
        });
    }

    // ---------- spec-04 RS-01 a RS-03: reservas ----------

    fn reservation_fixture(
        username: &str,
        seed: u32,
    ) -> (rusqlite::Connection, chrono::DateTime<Utc>, i64, i64) {
        let now = Utc.with_ymd_and_hms(2026, 10, 4, 16, 0, 0).unwrap();
        let (mut connection, _, client_id, supplier_id) = setup_sale_fixture(
            username,
            &format!("123456789{seed:02}"),
            &format!("+53555591{seed:02}"),
            &format!("123456788{seed:02}"),
            &format!("+53555592{seed:02}"),
            now,
        );
        let product_id = insert_stocked_product(
            &mut connection,
            now,
            supplier_id,
            "Leche",
            100,
            0,
            "20",
            "1,00",
        );
        (connection, now, client_id, product_id)
    }

    fn reserved_sale(
        connection: &mut rusqlite::Connection,
        now: chrono::DateTime<Utc>,
        client_id: i64,
        product_id: i64,
        quantity: &str,
        reserved_until: &str,
    ) -> i64 {
        let mut input = sale_input(client_id, vec![sale_line(product_id, quantity)], None);
        input.reserved_until = Some(reserved_until.to_owned());
        super::create_sale_invoice_inner(connection, now, input)
            .unwrap()
            .invoice_id
    }

    fn reserved_until_of(connection: &rusqlite::Connection, invoice_id: i64) -> Option<String> {
        connection
            .query_row(
                "SELECT reservedUntil FROM invoices WHERE id = ?1",
                [invoice_id],
                |row| row.get(0),
            )
            .unwrap()
    }

    fn product_quantity(connection: &rusqlite::Connection, product_id: i64) -> i64 {
        connection
            .query_row(
                "SELECT quantityMilli FROM products WHERE id = ?1",
                [product_id],
                |row| row.get(0),
            )
            .unwrap()
    }

    #[test]
    fn rs01_sales_and_services_are_reserved_on_create_and_edit_and_validated() {
        let (mut connection, now, client_id, product_id) = reservation_fixture("reservaok", 61);

        let sale = reserved_sale(
            &mut connection,
            now,
            client_id,
            product_id,
            "2",
            "2026-10-10",
        );
        assert_eq!(
            reserved_until_of(&connection, sale).as_deref(),
            Some("2026-10-10")
        );
        // La reserva consume stock como cualquier venta Por Cobrar.
        assert_eq!(product_quantity(&connection, product_id), 18_000);

        let mut service = service_input(client_id, vec![service_line(product_id, "1")], None);
        service.reserved_until = Some("2027-10-04".to_owned());
        let service_id = super::create_service_invoice_inner(&mut connection, now, service)
            .unwrap()
            .invoice_id;
        assert_eq!(
            reserved_until_of(&connection, service_id).as_deref(),
            Some("2027-10-04")
        );

        // Editar cambia la fecha y quitarla la convierte en una venta normal.
        let mut update = update_sale_input(
            sale,
            client_id,
            vec![update_outgoing_line(
                Some(product_id),
                Some(product_id),
                "2",
            )],
        );
        update.reserved_until = Some("2026-10-20".to_owned());
        super::update_sale_invoice_inner(&mut connection, now, update.clone()).unwrap();
        assert_eq!(
            reserved_until_of(&connection, sale).as_deref(),
            Some("2026-10-20")
        );
        update.reserved_until = None;
        super::update_sale_invoice_inner(&mut connection, now, update).unwrap();
        assert_eq!(reserved_until_of(&connection, sale), None);

        let reject =
            |connection: &mut rusqlite::Connection, reserved: &str, status: Option<&str>| {
                let mut input = sale_input(client_id, vec![sale_line(product_id, "1")], status);
                input.reserved_until = Some(reserved.to_owned());
                super::create_sale_invoice_inner(connection, now, input).unwrap_err()
            };
        for (reserved, status, message) in [
            (
                "2026-10-04",
                None,
                "La fecha de reserva debe ser posterior a la fecha de emisión",
            ),
            (
                "2026-10-03",
                None,
                "La fecha de reserva debe ser posterior a la fecha de emisión",
            ),
            (
                "2027-10-05",
                None,
                "La reserva no puede superar los 365 días desde la emisión",
            ),
            ("2026-02-30", None, "La fecha de reserva no es válida"),
            ("10/10/2026", None, "La fecha de reserva no es válida"),
            (
                "2026-10-10",
                Some("completed"),
                "Solo se puede reservar una factura Por Cobrar",
            ),
        ] {
            let error = reject(&mut connection, reserved, status);
            assert_eq!(error.code, "validationFailed", "{reserved}");
            assert_eq!(error.message, message, "{reserved}");
        }
        // Garantía, insumos, entrada y pérdida no tienen el campo; el esquema
        // además lo rechaza (migración 3, probado en db::migrations).
        let invoices: i64 = connection
            .query_row(
                "SELECT COUNT(*) FROM invoices WHERE type = 'V'",
                [],
                |row| row.get(0),
            )
            .unwrap();
        assert_eq!(invoices, 1);
    }

    #[test]
    fn rs03_active_reservations_in_the_list_the_detail_and_the_product() {
        let (mut connection, now, client_id, product_id) = reservation_fixture("reservalist", 63);
        let reserved = reserved_sale(
            &mut connection,
            now,
            client_id,
            product_id,
            "3",
            "2026-10-10",
        );
        let second = reserved_sale(
            &mut connection,
            now,
            client_id,
            product_id,
            "1,5",
            "2026-10-08",
        );
        let paid = reserved_sale(
            &mut connection,
            now,
            client_id,
            product_id,
            "1",
            "2026-10-09",
        );
        super::create_sale_invoice_inner(
            &mut connection,
            now,
            sale_input(client_id, vec![sale_line(product_id, "1")], None),
        )
        .unwrap();
        // Cobrar desactiva la reserva pero conserva la fecha.
        change_invoice_status_inner(&mut connection, now, paid, "completed", Some("cash")).unwrap();

        let reserved_ids: Vec<i64> = list_invoices_inner(
            &mut connection,
            now,
            InvoiceFilters {
                status: Some("reserved".to_owned()),
                ..InvoiceFilters::default()
            },
        )
        .unwrap()
        .iter()
        .map(|item| item.id)
        .collect();
        assert_eq!(reserved_ids, vec![second, reserved]);

        let detail = get_invoice_inner(&mut connection, now, paid).unwrap();
        assert_eq!(detail.reserved_until.as_deref(), Some("2026-10-09"));
        let item = list_invoices_inner(&mut connection, now, InvoiceFilters::default())
            .unwrap()
            .into_iter()
            .find(|item| item.id == reserved)
            .unwrap();
        assert_eq!(item.reserved_until.as_deref(), Some("2026-10-10"));

        let product =
            crate::commands::products::get_product_inner(&mut connection, now, product_id).unwrap();
        assert_eq!(product.reserved_quantity, "4,5");
        assert_eq!(
            product
                .reserved_invoices
                .iter()
                .map(|invoice| (invoice.id, invoice.quantity.as_str()))
                .collect::<Vec<_>>(),
            vec![(second, "1,5"), (reserved, "3")]
        );
    }

    #[test]
    fn rs02_expired_reservations_are_cancelled_by_the_system_before_the_next_operation() {
        let (mut connection, now, client_id, product_id) = reservation_fixture("reservaexp", 65);
        let expiring = reserved_sale(
            &mut connection,
            now,
            client_id,
            product_id,
            "5",
            "2026-10-05",
        );
        let alive = reserved_sale(
            &mut connection,
            now,
            client_id,
            product_id,
            "2",
            "2026-10-06",
        );
        assert_eq!(product_quantity(&connection, product_id), 13_000);

        // El 6 de octubre (hora local) la del día 5 ya venció; la del 6 no.
        let later = Utc.with_ymd_and_hms(2026, 10, 6, 16, 0, 0).unwrap();
        // Gancho previo a cada operación (donde se llama la purga).
        crate::commands::products::purge_best_effort(&connection, later).unwrap();

        let status = |connection: &rusqlite::Connection, id: i64| -> String {
            connection
                .query_row("SELECT status FROM invoices WHERE id = ?1", [id], |row| {
                    row.get(0)
                })
                .unwrap()
        };
        assert_eq!(status(&connection, expiring), "cancelled");
        assert_eq!(status(&connection, alive), "pending");
        assert_eq!(
            reserved_until_of(&connection, expiring).as_deref(),
            Some("2026-10-05")
        );
        // Los consumos vuelven a sus lotes y la caché sigue a los lotes.
        assert_eq!(product_quantity(&connection, product_id), 18_000);
        let lots: i64 = connection
            .query_row(
                "SELECT COALESCE(SUM(remainingMilli), 0) FROM lots WHERE productId = ?1",
                [product_id],
                |row| row.get(0),
            )
            .unwrap();
        assert_eq!(lots, 18_000);

        let reversal: (Option<i64>, String, i64) = connection
            .query_row(
                "SELECT userId, userName, quantityMilli FROM productMovements WHERE type = 'reversal'",
                [],
                |row| Ok((row.get(0)?, row.get(1)?, row.get(2)?)),
            )
            .unwrap();
        assert_eq!(reversal, (None, "Sistema".to_owned(), 5_000));

        let n06: (String, String, Option<i64>) = connection
            .query_row(
                "SELECT priority, message, entityId FROM notifications WHERE type = 'N-06'",
                [],
                |row| Ok((row.get(0)?, row.get(1)?, row.get(2)?)),
            )
            .unwrap();
        let number: String = connection
            .query_row(
                "SELECT number FROM invoices WHERE id = ?1",
                [expiring],
                |row| row.get(0),
            )
            .unwrap();
        assert_eq!(
            n06,
            (
                "INFORMATIVO".to_owned(),
                format!("Factura de reserva cancelada: Factura # {number} - Cliente: Luis García - Motivo: Tiempo límite excedido"),
                Some(expiring),
            )
        );

        // Volver a pasar no la cancela otra vez ni repite el aviso.
        assert_eq!(super::expire_reservations(&connection, later).unwrap(), 0);
        let n06_count: i64 = connection
            .query_row(
                "SELECT COUNT(*) FROM notifications WHERE type = 'N-06'",
                [],
                |row| row.get(0),
            )
            .unwrap();
        assert_eq!(n06_count, 1);
    }

    #[test]
    fn rs02_reservations_also_expire_at_startup() {
        let (mut connection, now, client_id, product_id) = reservation_fixture("reservaini", 67);
        let expiring = reserved_sale(
            &mut connection,
            now,
            client_id,
            product_id,
            "1",
            "2026-10-05",
        );
        let later = Utc.with_ymd_and_hms(2026, 10, 9, 16, 0, 0).unwrap();

        crate::automation::run_startup_processes(&mut connection, later);
        let status: String = connection
            .query_row(
                "SELECT status FROM invoices WHERE id = ?1",
                [expiring],
                |row| row.get(0),
            )
            .unwrap();
        assert_eq!(status, "cancelled");
    }
}
