use chrono::{DateTime, Local, NaiveDate, Utc};
use rusqlite::{Connection, OptionalExtension};
use tauri::State;

use crate::commands::auth::require_session;
use crate::commands::products::purge_best_effort;
use crate::db::connection::{current_app_data_dir, lock_conn};
use crate::errors::{log_error, map_db_error, AppError};
use crate::export::{
    export_history_pdf as write_history_pdf, export_history_xlsx as write_history_xlsx,
    HistoryHeader,
};
use crate::models::product::format_quantity_milli;
use crate::models::product_movement::{
    ExportHistoryInput, ExportHistoryResult, ProductHistoryFilters, ProductHistoryItem,
    ProductHistoryList, ProductHistoryOption,
};
use crate::text::{sort_spanish, spanish_sort_key};

fn parse_local_date(value: &str) -> Option<NaiveDate> {
    NaiveDate::parse_from_str(value, "%Y-%m-%d").ok()
}

fn matches_date_range(created_at: &str, from_date: Option<&str>, to_date: Option<&str>) -> bool {
    let parsed = match DateTime::parse_from_rfc3339(created_at) {
        Ok(timestamp) => timestamp.with_timezone(&Local).date_naive(),
        Err(_) => return true,
    };

    if let Some(from_date) = from_date.and_then(parse_local_date) {
        if parsed < from_date {
            return false;
        }
    }

    if let Some(to_date) = to_date.and_then(parse_local_date) {
        if parsed > to_date {
            return false;
        }
    }

    true
}

fn matches_type_filter(movement_type: &str, types: &[String]) -> bool {
    types.is_empty() || types.iter().any(|value| value == movement_type)
}

pub fn list_history_products_inner(
    connection: &Connection,
    now: DateTime<Utc>,
) -> Result<Vec<ProductHistoryOption>, AppError> {
    let _actor = require_session(connection, now)?;

    let mut active_statement = connection
        .prepare("SELECT id, name FROM products")
        .map_err(|error| map_db_error(error, "listHistoryProducts", &[]))?;
    let mut active_rows = active_statement
        .query([])
        .map_err(|error| map_db_error(error, "listHistoryProducts", &[]))?;

    let mut active_products = Vec::new();
    while let Some(row) = active_rows
        .next()
        .map_err(|error| map_db_error(error, "listHistoryProducts", &[]))?
    {
        active_products.push(ProductHistoryOption {
            id: row
                .get(0)
                .map_err(|error| map_db_error(error, "listHistoryProducts", &[]))?,
            name: row
                .get(1)
                .map_err(|error| map_db_error(error, "listHistoryProducts", &[]))?,
            is_deleted: false,
        });
    }
    sort_spanish(&mut active_products, |product| {
        (spanish_sort_key(&product.name), product.id)
    });

    let mut deleted_statement = connection
        .prepare(
            "SELECT pm.productIdOriginal AS id, pm.productName AS name
             FROM productMovements pm
             WHERE pm.id = (
                 SELECT pm2.id
                 FROM productMovements pm2
                 WHERE pm2.productIdOriginal = pm.productIdOriginal
                 ORDER BY pm2.createdAt DESC, pm2.id DESC
                 LIMIT 1
             )
             AND pm.productIdOriginal NOT IN (SELECT id FROM products)",
        )
        .map_err(|error| map_db_error(error, "listHistoryProducts", &[]))?;
    let mut deleted_rows = deleted_statement
        .query([])
        .map_err(|error| map_db_error(error, "listHistoryProducts", &[]))?;

    let mut deleted_products = Vec::new();
    while let Some(row) = deleted_rows
        .next()
        .map_err(|error| map_db_error(error, "listHistoryProducts", &[]))?
    {
        deleted_products.push(ProductHistoryOption {
            id: row
                .get(0)
                .map_err(|error| map_db_error(error, "listHistoryProducts", &[]))?,
            name: row
                .get(1)
                .map_err(|error| map_db_error(error, "listHistoryProducts", &[]))?,
            is_deleted: true,
        });
    }
    sort_spanish(&mut deleted_products, |product| {
        (spanish_sort_key(&product.name), product.id)
    });

    active_products.extend(deleted_products);
    Ok(active_products)
}

#[tauri::command]
pub fn list_history_products(
    state: State<'_, std::sync::Mutex<Connection>>,
) -> Result<Vec<ProductHistoryOption>, AppError> {
    let connection = lock_conn(&state);
    let now = Utc::now();
    let result = list_history_products_inner(&connection, now);
    if let Err(error) = &result {
        if error.should_log() {
            if let Some(app_data_dir) = current_app_data_dir() {
                log_error(
                    &connection,
                    "history",
                    "listHistoryProducts",
                    error,
                    now,
                    app_data_dir,
                );
            }
        }
    }
    result
}

pub fn get_product_history_inner(
    connection: &Connection,
    now: DateTime<Utc>,
    filters: ProductHistoryFilters,
) -> Result<ProductHistoryList, AppError> {
    let _actor = require_session(connection, now)?;

    if let Some(from_date) = filters.from_date.as_deref() {
        parse_local_date(from_date).ok_or_else(|| {
            AppError::new(
                "validationFailed",
                "La fecha inicial no es válida",
                "product history fromDate is invalid",
            )
        })?;
    }

    if let Some(to_date) = filters.to_date.as_deref() {
        parse_local_date(to_date).ok_or_else(|| {
            AppError::new(
                "validationFailed",
                "La fecha final no es válida",
                "product history toDate is invalid",
            )
        })?;
    }

    if let (Some(from_date), Some(to_date)) =
        (filters.from_date.as_deref(), filters.to_date.as_deref())
    {
        let from_date = parse_local_date(from_date).ok_or_else(|| {
            AppError::new(
                "validationFailed",
                "La fecha inicial no es válida",
                "product history fromDate is invalid",
            )
        })?;
        let to_date = parse_local_date(to_date).ok_or_else(|| {
            AppError::new(
                "validationFailed",
                "La fecha final no es válida",
                "product history toDate is invalid",
            )
        })?;
        if from_date > to_date {
            return Err(AppError::new(
                "validationFailed",
                "La fecha inicial no puede ser posterior a la fecha final",
                "product history date range is invalid",
            ));
        }
    }

    let expired_cleanup_warning = match purge_best_effort(connection, now) {
        Ok(warning) => warning,
        Err(error) => {
            if let Some(app_data_dir) = current_app_data_dir() {
                log_error(
                    connection,
                    "history",
                    "getProductHistory",
                    &error,
                    now,
                    app_data_dir,
                );
            }
            None
        }
    };

    let has_history = connection
        .query_row(
            "SELECT 1 FROM productMovements WHERE productIdOriginal = ?1 LIMIT 1",
            [&filters.product_id],
            |row| row.get::<_, i64>(0),
        )
        .optional()
        .map_err(|error| map_db_error(error, "getProductHistory", &[]))?
        .is_some();

    if !has_history {
        return Err(AppError::new(
            "notFound",
            "El producto no tiene historial",
            "product history requested for an unknown product",
        ));
    }

    let mut statement = connection
        .prepare(
            "SELECT id, createdAt, type, quantityMilli, stockAfterMilli, unitPriceCents, totalPriceCents, invoiceNumber, invoiceType, userName, productIdOriginal FROM productMovements WHERE productIdOriginal = ?1 ORDER BY createdAt DESC, id DESC",
        )
        .map_err(|error| map_db_error(error, "getProductHistory", &[]))?;

    let rows = statement
        .query_map([filters.product_id], |row| {
            Ok(ProductHistoryItem {
                id: row.get(0)?,
                created_at: row.get(1)?,
                movement_type: row.get(2)?,
                quantity: format_quantity_milli(row.get::<_, i64>(3)?),
                stock_after: format_quantity_milli(row.get::<_, i64>(4)?),
                unit_price_cents: row.get(5)?,
                total_price_cents: row.get(6)?,
                invoice_number: row.get(7)?,
                invoice_type: row.get(8)?,
                user_name: row.get(9)?,
                product_id_original: row.get(10)?,
            })
        })
        .map_err(|error| map_db_error(error, "getProductHistory", &[]))?;

    let items = rows
        .collect::<Result<Vec<_>, _>>()
        .map_err(|error| map_db_error(error, "getProductHistory", &[]))?
        .into_iter()
        .filter(|item| {
            matches_type_filter(&item.movement_type, &filters.types)
                && matches_date_range(
                    &item.created_at,
                    filters.from_date.as_deref(),
                    filters.to_date.as_deref(),
                )
        })
        .collect();

    Ok(ProductHistoryList {
        items,
        expired_cleanup_warning,
    })
}

#[tauri::command]
pub fn get_product_history(
    state: State<'_, std::sync::Mutex<Connection>>,
    filters: ProductHistoryFilters,
) -> Result<ProductHistoryList, AppError> {
    let connection = lock_conn(&state);
    let now = Utc::now();
    let result = get_product_history_inner(&connection, now, filters);
    if let Err(error) = &result {
        if error.should_log() {
            if let Some(app_data_dir) = current_app_data_dir() {
                log_error(
                    &connection,
                    "history",
                    "getProductHistory",
                    error,
                    now,
                    app_data_dir,
                );
            }
        }
    }
    result
}

#[derive(Clone, Copy)]
enum ExportFormat {
    Xlsx,
    Pdf,
}

/// Exporta todos los movimientos que cumplen los filtros (no solo la página
/// visible), en el orden de la pantalla. `get_product_history_inner` exige la
/// sesión y purga best-effort antes de leer.
fn export_history_inner(
    connection: &Connection,
    now: DateTime<Utc>,
    input: ExportHistoryInput,
    format: ExportFormat,
) -> Result<ExportHistoryResult, AppError> {
    let destination = input.destination_path.trim().to_owned();
    if destination.is_empty() {
        return Err(AppError::new(
            "validationFailed",
            "Elige dónde guardar el archivo",
            "history export without destination",
        ));
    }
    let list = get_product_history_inner(
        connection,
        now,
        ProductHistoryFilters {
            product_id: input.product_id,
            from_date: input.from_date.clone(),
            to_date: input.to_date.clone(),
            types: input.types.clone(),
        },
    )?;
    if list.items.is_empty() {
        return Err(AppError::new(
            "invalidState",
            "No hay movimientos para exportar",
            "history export with no movements",
        ));
    }
    // El nombre vigente del producto o, si se eliminó, la copia del historial.
    let product_name: String = connection
        .query_row(
            "SELECT COALESCE(
                (SELECT name FROM products WHERE id = ?1),
                (SELECT productName FROM productMovements
                 WHERE productIdOriginal = ?1 ORDER BY id DESC LIMIT 1))",
            [input.product_id],
            |row| row.get(0),
        )
        .map_err(|error| map_db_error(error, "exportHistory", &[]))?;
    let header = HistoryHeader {
        product_name,
        from_date: input.from_date,
        to_date: input.to_date,
        types: input.types,
        generated_at: now,
    };
    let path = std::path::Path::new(&destination);
    match format {
        ExportFormat::Xlsx => write_history_xlsx(&list.items, &header, path)?,
        ExportFormat::Pdf => write_history_pdf(&list.items, &header, path)?,
    }
    Ok(ExportHistoryResult {
        exported_count: list.items.len(),
        expired_cleanup_warning: list.expired_cleanup_warning,
    })
}

pub fn export_history_excel_inner(
    connection: &Connection,
    now: DateTime<Utc>,
    input: ExportHistoryInput,
) -> Result<ExportHistoryResult, AppError> {
    export_history_inner(connection, now, input, ExportFormat::Xlsx)
}

pub fn export_history_pdf_inner(
    connection: &Connection,
    now: DateTime<Utc>,
    input: ExportHistoryInput,
) -> Result<ExportHistoryResult, AppError> {
    export_history_inner(connection, now, input, ExportFormat::Pdf)
}

fn log_export_error(
    connection: &Connection,
    operation: &str,
    now: DateTime<Utc>,
    result: &Result<ExportHistoryResult, AppError>,
) {
    if let Err(error) = result {
        if error.should_log() {
            if let Some(app_data_dir) = current_app_data_dir() {
                log_error(connection, "history", operation, error, now, app_data_dir);
            }
        }
    }
}

#[tauri::command]
pub fn export_history_excel(
    state: State<'_, std::sync::Mutex<Connection>>,
    input: ExportHistoryInput,
) -> Result<ExportHistoryResult, AppError> {
    let connection = lock_conn(&state);
    let now = Utc::now();
    let result = export_history_excel_inner(&connection, now, input);
    log_export_error(&connection, "exportHistoryExcel", now, &result);
    result
}

#[tauri::command]
pub fn export_history_pdf(
    state: State<'_, std::sync::Mutex<Connection>>,
    input: ExportHistoryInput,
) -> Result<ExportHistoryResult, AppError> {
    let connection = lock_conn(&state);
    let now = Utc::now();
    let result = export_history_pdf_inner(&connection, now, input);
    log_export_error(&connection, "exportHistoryPdf", now, &result);
    result
}

#[cfg(test)]
mod tests {
    use crate::commands::auth::require_session;
    use crate::test_utils::{create_test_session, create_test_user, setup_test_db};
    use chrono::{TimeZone, Utc};

    use super::*;
    use crate::models::product_movement::ProductHistoryFilters;

    /// Producto con tres movimientos (Inicial, Entrada y Salida Venta).
    fn export_fixture(username: &str, phone: &str, national_id: &str) -> (Connection, i64) {
        let connection = setup_test_db();
        let now = Utc.with_ymd_and_hms(2026, 10, 7, 9, 30, 0).unwrap();
        let user_id = create_test_user(
            &connection,
            username,
            "contador",
            phone,
            national_id,
            "ValidPass1!",
        );
        create_test_session(&connection, user_id, false, now, now);
        let timestamp = now.to_rfc3339_opts(chrono::SecondsFormat::Secs, true);
        connection
            .execute(
                "INSERT INTO products (name, nameNormalized, description, categoryId, unitId, costCents, profitPercentHundredths, expiresOn, quantityMilli, status, discontinuedAt, createdAt, updatedAt) VALUES ('Cable Ñandú', 'cable nandu', NULL, NULL, NULL, 1000, 2500, NULL, 0, 'active', NULL, ?1, ?1)",
                [&timestamp],
            )
            .unwrap();
        let product_id = connection.last_insert_rowid();
        for (movement_type, quantity, invoice, invoice_type) in [
            ("initial", 1000, None, None),
            ("entry", 3000, Some("E-00001"), Some("E")),
            ("saleOut", -2000, Some("V-00001"), Some("V")),
        ] {
            connection
                .execute(
                    "INSERT INTO productMovements (createdAt, productId, productIdOriginal, productName, type, quantityMilli, stockAfterMilli, unitPriceCents, totalPriceCents, invoiceId, invoiceNumber, invoiceType, userId, userName) VALUES (?1, ?2, ?2, 'Cable Ñandú', ?3, ?4, 2000, 1250, 2500, NULL, ?5, ?8, ?6, ?7)",
                    rusqlite::params![
                        timestamp,
                        product_id,
                        movement_type,
                        quantity,
                        invoice,
                        user_id,
                        username,
                        invoice_type
                    ],
                )
                .unwrap();
        }
        (connection, product_id)
    }

    fn export_input(
        product_id: i64,
        types: &[&str],
        destination: &std::path::Path,
    ) -> ExportHistoryInput {
        ExportHistoryInput {
            product_id,
            from_date: None,
            to_date: None,
            types: types.iter().map(|value| (*value).to_owned()).collect(),
            destination_path: destination.to_string_lossy().into_owned(),
        }
    }

    fn temp_export_path(name: &str) -> std::path::PathBuf {
        let dir = std::env::temp_dir().join(format!("goyma-export-{}", uuid::Uuid::new_v4()));
        std::fs::create_dir_all(&dir).unwrap();
        dir.join(name)
    }

    #[test]
    fn export_history_excel_inner_writes_only_the_filtered_movements() {
        use std::io::Read;
        let (connection, product_id) = export_fixture("exportxlsx", "+5350000140", "12345678940");
        let now = Utc.with_ymd_and_hms(2026, 10, 7, 9, 30, 0).unwrap();
        let path = temp_export_path("historial.xlsx");

        let result = export_history_excel_inner(
            &connection,
            now,
            export_input(product_id, &["entry", "saleOut"], &path),
        )
        .unwrap();
        assert_eq!(result.exported_count, 2);

        let bytes = std::fs::read(&path).unwrap();
        assert!(!bytes.is_empty());
        let mut archive = zip::ZipArchive::new(std::io::Cursor::new(bytes)).unwrap();
        let mut sheet = String::new();
        archive
            .by_name("xl/worksheets/sheet1.xml")
            .unwrap()
            .read_to_string(&mut sheet)
            .unwrap();
        assert!(sheet.contains("Cable Ñandú"));
        assert!(sheet.contains("Entrada, Salida Venta"));
        assert!(sheet.contains("E-00001") && sheet.contains("V-00001"));
        // El movimiento Inicial no pasa el filtro de tipos.
        assert!(!sheet.contains(">Inicial<"));
        std::fs::remove_dir_all(path.parent().unwrap()).unwrap();
    }

    #[test]
    fn export_history_pdf_inner_writes_a_pdf_with_the_movements() {
        let (connection, product_id) = export_fixture("exportpdf", "+5350000141", "12345678941");
        let now = Utc.with_ymd_and_hms(2026, 10, 7, 9, 30, 0).unwrap();
        let path = temp_export_path("historial.pdf");

        let result =
            export_history_pdf_inner(&connection, now, export_input(product_id, &[], &path))
                .unwrap();
        assert_eq!(result.exported_count, 3);
        let bytes = std::fs::read(&path).unwrap();
        assert!(bytes.starts_with(b"%PDF"));
        assert!(printpdf::lopdf::Document::load_mem(&bytes).is_ok());
        std::fs::remove_dir_all(path.parent().unwrap()).unwrap();
    }

    #[test]
    fn export_history_without_movements_after_filters_is_invalid_state() {
        let (connection, product_id) = export_fixture("exportempty", "+5350000142", "12345678942");
        let now = Utc.with_ymd_and_hms(2026, 10, 7, 9, 30, 0).unwrap();
        let path = temp_export_path("vacio.xlsx");
        for result in [
            export_history_excel_inner(
                &connection,
                now,
                export_input(product_id, &["loss"], &path),
            ),
            export_history_pdf_inner(&connection, now, export_input(product_id, &["loss"], &path)),
        ] {
            let error = result.unwrap_err();
            assert_eq!(error.code, "invalidState");
            assert_eq!(error.message, "No hay movimientos para exportar");
        }
        // No se crea ningún archivo.
        assert!(!path.exists());
        std::fs::remove_dir_all(path.parent().unwrap()).unwrap();
    }

    #[test]
    fn export_history_write_failure_is_internal_with_a_clear_message() {
        let (connection, product_id) = export_fixture("exportfail", "+5350000143", "12345678943");
        let now = Utc.with_ymd_and_hms(2026, 10, 7, 9, 30, 0).unwrap();
        // El directorio de destino no existe: la escritura falla.
        let path = std::env::temp_dir()
            .join(format!("goyma-no-existe-{}", uuid::Uuid::new_v4()))
            .join("historial.xlsx");
        for result in [
            export_history_excel_inner(&connection, now, export_input(product_id, &[], &path)),
            export_history_pdf_inner(&connection, now, export_input(product_id, &[], &path)),
        ] {
            let error = result.unwrap_err();
            assert_eq!(error.code, "internal");
            assert_eq!(error.message, "No se pudo guardar el archivo");
            assert!(error.should_log());
        }
    }

    #[test]
    fn export_history_requires_a_destination() {
        let (connection, product_id) = export_fixture("exportnodest", "+5350000144", "12345678944");
        let now = Utc.with_ymd_and_hms(2026, 10, 7, 9, 30, 0).unwrap();
        let mut input = export_input(product_id, &[], std::path::Path::new(""));
        input.destination_path = "   ".to_owned();
        let error = export_history_excel_inner(&connection, now, input).unwrap_err();
        assert_eq!(error.code, "validationFailed");
    }

    #[test]
    fn export_history_commands_require_session_and_allow_both_roles() {
        use crate::test_utils::{assert_allows_both_roles, assert_requires_session};
        let input = || ExportHistoryInput {
            product_id: 999,
            from_date: None,
            to_date: None,
            types: vec![],
            destination_path: "historial.xlsx".to_owned(),
        };
        assert_requires_session(|connection, now| {
            export_history_excel_inner(connection, now, input())
        });
        assert_allows_both_roles(|connection, now| {
            export_history_excel_inner(connection, now, input())
        });
        assert_requires_session(|connection, now| {
            export_history_pdf_inner(connection, now, input())
        });
        assert_allows_both_roles(|connection, now| {
            export_history_pdf_inner(connection, now, input())
        });
    }

    #[test]
    fn get_product_history_returns_items_in_reverse_chronological_order() {
        let connection = setup_test_db();
        let now = Utc.with_ymd_and_hms(2026, 10, 7, 9, 30, 0).unwrap();
        let user_id = create_test_user(
            &connection,
            "historyuser",
            "contador",
            "+5350000100",
            "12345678922",
            "ValidPass1!",
        );
        create_test_session(&connection, user_id, false, now, now);
        let _actor = require_session(&connection, now).unwrap();

        connection
            .execute(
                "INSERT INTO products (name, nameNormalized, description, categoryId, unitId, costCents, profitPercentHundredths, expiresOn, quantityMilli, status, discontinuedAt, createdAt, updatedAt) VALUES (?1, ?2, NULL, NULL, NULL, 1000, 2500, NULL, 0, 'active', NULL, ?3, ?3)",
                (
                    "Cable UTP",
                    "cable utp",
                    now.to_rfc3339_opts(chrono::SecondsFormat::Secs, true),
                ),
            )
            .unwrap();
        let product_id = connection.last_insert_rowid();

        connection
            .execute(
                "INSERT INTO productMovements (createdAt, productId, productIdOriginal, productName, type, quantityMilli, stockAfterMilli, unitPriceCents, totalPriceCents, invoiceId, invoiceNumber, invoiceType, userId, userName) VALUES (?1, ?2, ?2, 'Cable UTP', 'initial', 1000, 1000, 1000, 1000, NULL, NULL, NULL, ?3, 'historyuser')",
                (
                    now.to_rfc3339_opts(chrono::SecondsFormat::Secs, true),
                    product_id,
                    user_id,
                ),
            )
            .unwrap();
        connection
            .execute(
                "INSERT INTO productMovements (createdAt, productId, productIdOriginal, productName, type, quantityMilli, stockAfterMilli, unitPriceCents, totalPriceCents, invoiceId, invoiceNumber, invoiceType, userId, userName) VALUES (?1, ?2, ?2, 'Cable UTP', 'entry', 3000, 2000, 2000, 6000, NULL, 'E-00001', 'E', ?3, 'historyuser')",
                (
                    now.to_rfc3339_opts(chrono::SecondsFormat::Secs, true),
                    product_id,
                    user_id,
                ),
            )
            .unwrap();
        connection
            .execute("DELETE FROM products WHERE id = ?1", [product_id])
            .unwrap();

        let result = get_product_history_inner(
            &connection,
            now,
            ProductHistoryFilters {
                product_id,
                from_date: None,
                to_date: None,
                types: vec![],
            },
        )
        .unwrap();

        assert_eq!(result.items.len(), 2);
        assert_eq!(result.items[0].movement_type, "entry");
        assert_eq!(result.items[1].movement_type, "initial");
    }

    #[test]
    fn get_product_history_applies_filters_and_rejects_bad_ranges() {
        let connection = setup_test_db();
        let now = Utc.with_ymd_and_hms(2026, 10, 7, 9, 30, 0).unwrap();
        let user_id = create_test_user(
            &connection,
            "historyfilteruser",
            "contador",
            "+5350000110",
            "12345678923",
            "ValidPass1!",
        );
        create_test_session(&connection, user_id, false, now, now);

        connection
            .execute(
                "INSERT INTO products (name, nameNormalized, description, categoryId, unitId, costCents, profitPercentHundredths, expiresOn, quantityMilli, status, discontinuedAt, createdAt, updatedAt) VALUES (?1, ?2, NULL, NULL, NULL, 1000, 2500, NULL, 0, 'active', NULL, ?3, ?3)",
                (
                    "Filtro Cable",
                    "filtro cable",
                    now.to_rfc3339_opts(chrono::SecondsFormat::Secs, true),
                ),
            )
            .unwrap();
        let product_id = connection.last_insert_rowid();

        connection
            .execute(
                "INSERT INTO productMovements (createdAt, productId, productIdOriginal, productName, type, quantityMilli, stockAfterMilli, unitPriceCents, totalPriceCents, invoiceId, invoiceNumber, invoiceType, userId, userName) VALUES (?1, ?2, ?2, 'Filtro Cable', 'entry', 2000, 2000, 2000, 2000, NULL, 'E-00002', 'E', ?3, 'historyfilteruser')",
                ("2026-10-05T08:00:00Z", product_id, user_id),
            )
            .unwrap();
        connection
            .execute(
                "INSERT INTO productMovements (createdAt, productId, productIdOriginal, productName, type, quantityMilli, stockAfterMilli, unitPriceCents, totalPriceCents, invoiceId, invoiceNumber, invoiceType, userId, userName) VALUES (?1, ?2, ?2, 'Filtro Cable', 'saleOut', -1000, 1000, 1500, 1500, NULL, 'V-00001', 'V', ?3, 'historyfilteruser')",
                ("2026-10-06T08:00:00Z", product_id, user_id),
            )
            .unwrap();

        let filtered = get_product_history_inner(
            &connection,
            now,
            ProductHistoryFilters {
                product_id,
                from_date: Some("2026-10-05".to_string()),
                to_date: Some("2026-10-06".to_string()),
                types: vec!["entry".to_string(), "saleOut".to_string()],
            },
        )
        .unwrap();
        assert_eq!(filtered.items.len(), 2);
        assert_eq!(filtered.items[0].movement_type, "saleOut");
        assert_eq!(filtered.items[1].movement_type, "entry");

        let error = get_product_history_inner(
            &connection,
            now,
            ProductHistoryFilters {
                product_id,
                from_date: Some("2026-10-07".to_string()),
                to_date: Some("2026-10-06".to_string()),
                types: vec![],
            },
        )
        .unwrap_err();
        assert_eq!(error.code, "validationFailed");
    }

    #[test]
    fn get_product_history_keeps_running_when_purge_fails() {
        let connection = setup_test_db();
        let now = Utc.with_ymd_and_hms(2026, 10, 7, 9, 30, 0).unwrap();
        let user_id = create_test_user(
            &connection,
            "historypurgeuser",
            "contador",
            "+5350000111",
            "12345678924",
            "ValidPass1!",
        );
        create_test_session(&connection, user_id, false, now, now);

        connection
            .execute(
                "INSERT INTO products (name, nameNormalized, description, categoryId, unitId, costCents, profitPercentHundredths, expiresOn, quantityMilli, status, discontinuedAt, createdAt, updatedAt) VALUES (?1, ?2, NULL, NULL, NULL, 1000, 2500, NULL, 10, 'discontinued', '2022-01-01T00:00:00Z', ?3, ?3)",
                (
                    "Cable Purge",
                    "cable purge",
                    now.to_rfc3339_opts(chrono::SecondsFormat::Secs, true),
                ),
            )
            .unwrap();
        let product_id = connection.last_insert_rowid();

        connection
            .execute(
                "INSERT INTO productMovements (createdAt, productId, productIdOriginal, productName, type, quantityMilli, stockAfterMilli, unitPriceCents, totalPriceCents, invoiceId, invoiceNumber, invoiceType, userId, userName) VALUES (?1, ?2, ?2, 'Cable Purge', 'initial', 1000, 1000, 1000, 1000, NULL, NULL, NULL, ?3, 'historypurgeuser')",
                ("2026-10-05T08:00:00Z", product_id, user_id),
            )
            .unwrap();

        connection
            .execute(
                "CREATE TRIGGER fail_history_purge BEFORE DELETE ON products BEGIN SELECT RAISE(ABORT, 'purge failed'); END;",
                (),
            )
            .unwrap();

        let result = get_product_history_inner(
            &connection,
            now,
            ProductHistoryFilters {
                product_id,
                from_date: None,
                to_date: None,
                types: vec![],
            },
        )
        .unwrap();

        assert_eq!(result.items.len(), 1);
        assert!(result.expired_cleanup_warning.is_some());
        let logged_errors: i64 = connection
            .query_row(
                "SELECT COUNT(*) FROM errorLogs WHERE module = 'products' AND operation = 'purge_expired_products'",
                [],
                |row| row.get(0),
            )
            .unwrap();
        assert!(logged_errors > 0);
    }

    #[test]
    fn get_product_history_rejects_unknown_products_without_history() {
        let connection = setup_test_db();
        let now = Utc::now();
        let user_id = create_test_user(
            &connection,
            "unknownproductuser",
            "contador",
            "+5350000120",
            "12345678925",
            "ValidPass1!",
        );
        create_test_session(&connection, user_id, false, now, now);

        let error = get_product_history_inner(
            &connection,
            now,
            ProductHistoryFilters {
                product_id: 999,
                from_date: None,
                to_date: None,
                types: vec![],
            },
        )
        .unwrap_err();

        assert_eq!(error.code, "notFound");
    }

    #[test]
    fn get_product_history_requires_session_and_allows_both_roles() {
        use crate::test_utils::{assert_allows_both_roles, assert_requires_session};

        let filters = || ProductHistoryFilters {
            product_id: 999,
            from_date: None,
            to_date: None,
            types: vec![],
        };

        assert_requires_session(|connection, now| {
            get_product_history_inner(connection, now, filters())
        });
        assert_allows_both_roles(|connection, now| {
            get_product_history_inner(connection, now, filters())
        });
    }

    #[test]
    fn list_history_products_returns_active_and_deleted_products_with_movements() {
        let connection = setup_test_db();
        let now = Utc.with_ymd_and_hms(2026, 10, 7, 9, 30, 0).unwrap();
        let user_id = create_test_user(
            &connection,
            "historylistuser",
            "contador",
            "+5350000130",
            "12345678926",
            "ValidPass1!",
        );
        create_test_session(&connection, user_id, false, now, now);

        connection
            .execute(
                "INSERT INTO products (name, nameNormalized, description, categoryId, unitId, costCents, profitPercentHundredths, expiresOn, quantityMilli, status, discontinuedAt, createdAt, updatedAt) VALUES (?1, ?2, NULL, NULL, NULL, 1000, 2500, NULL, 1000, 'active', NULL, ?3, ?3)",
                (
                    "Zebra",
                    "zebra",
                    now.to_rfc3339_opts(chrono::SecondsFormat::Secs, true),
                ),
            )
            .unwrap();
        let _active_id = connection.last_insert_rowid();
        connection
            .execute(
                "INSERT INTO products (name, nameNormalized, description, categoryId, unitId, costCents, profitPercentHundredths, expiresOn, quantityMilli, status, discontinuedAt, createdAt, updatedAt) VALUES (?1, ?2, NULL, NULL, NULL, 1000, 2500, NULL, 1000, 'active', NULL, ?3, ?3)",
                (
                    "Águila",
                    "aguila",
                    now.to_rfc3339_opts(chrono::SecondsFormat::Secs, true),
                ),
            )
            .unwrap();
        let _other_active_id = connection.last_insert_rowid();

        connection
            .execute(
                "INSERT INTO products (name, nameNormalized, description, categoryId, unitId, costCents, profitPercentHundredths, expiresOn, quantityMilli, status, discontinuedAt, createdAt, updatedAt) VALUES (?1, ?2, NULL, NULL, NULL, 1000, 2500, NULL, 1000, 'active', NULL, ?3, ?3)",
                (
                    "Borrado",
                    "borrado",
                    now.to_rfc3339_opts(chrono::SecondsFormat::Secs, true),
                ),
            )
            .unwrap();
        let deleted_id = connection.last_insert_rowid();
        connection
            .execute(
                "INSERT INTO productMovements (createdAt, productId, productIdOriginal, productName, type, quantityMilli, stockAfterMilli, unitPriceCents, totalPriceCents, invoiceId, invoiceNumber, invoiceType, userId, userName) VALUES (?1, ?2, ?2, 'Borrado', 'initial', 1000, 1000, 1000, 1000, NULL, NULL, NULL, ?3, 'historylistuser')",
                (
                    now.to_rfc3339_opts(chrono::SecondsFormat::Secs, true),
                    deleted_id,
                    user_id,
                ),
            )
            .unwrap();
        connection
            .execute("DELETE FROM products WHERE id = ?1", [deleted_id])
            .unwrap();

        connection
            .execute(
                "INSERT INTO products (name, nameNormalized, description, categoryId, unitId, costCents, profitPercentHundredths, expiresOn, quantityMilli, status, discontinuedAt, createdAt, updatedAt) VALUES (?1, ?2, NULL, NULL, NULL, 1000, 2500, NULL, 1000, 'active', NULL, ?3, ?3)",
                (
                    "Sin historial",
                    "sin historial",
                    now.to_rfc3339_opts(chrono::SecondsFormat::Secs, true),
                ),
            )
            .unwrap();
        let ignored_id = connection.last_insert_rowid();
        connection
            .execute("DELETE FROM products WHERE id = ?1", [ignored_id])
            .unwrap();

        let result = list_history_products_inner(&connection, now).unwrap();

        let active_names: Vec<_> = result
            .iter()
            .filter(|item| !item.is_deleted)
            .map(|item| item.name.clone())
            .collect();
        let deleted_names: Vec<_> = result
            .iter()
            .filter(|item| item.is_deleted)
            .map(|item| item.name.clone())
            .collect();

        assert_eq!(active_names, vec!["Águila", "Zebra"]);
        assert_eq!(deleted_names, vec!["Borrado"]);
        assert_eq!(
            result.iter().filter(|item| item.id == deleted_id).count(),
            1
        );
    }

    #[test]
    fn list_history_products_does_not_purge_and_keeps_expired_discontinued_products_visible() {
        let connection = setup_test_db();
        let now = Utc.with_ymd_and_hms(2026, 10, 7, 9, 30, 0).unwrap();
        let user_id = create_test_user(
            &connection,
            "historypurgecheck",
            "contador",
            "+5350000131",
            "12345678927",
            "ValidPass1!",
        );
        create_test_session(&connection, user_id, false, now, now);

        let expired_id = connection
            .execute(
                "INSERT INTO products (name, nameNormalized, description, categoryId, unitId, costCents, profitPercentHundredths, expiresOn, quantityMilli, status, discontinuedAt, createdAt, updatedAt) VALUES (?1, ?2, NULL, NULL, NULL, 1000, 2500, NULL, 1000, 'discontinued', '2020-01-01T00:00:00Z', ?3, ?3)",
                (
                    "Producto vencido",
                    "producto vencido",
                    now.to_rfc3339_opts(chrono::SecondsFormat::Secs, true),
                ),
            )
            .unwrap() as i64;
        connection
            .execute(
                "INSERT INTO productMovements (createdAt, productId, productIdOriginal, productName, type, quantityMilli, stockAfterMilli, unitPriceCents, totalPriceCents, invoiceId, invoiceNumber, invoiceType, userId, userName) VALUES (?1, ?2, ?2, 'Producto vencido', 'initial', 1000, 1000, 1000, 1000, NULL, NULL, NULL, ?3, 'historypurgecheck')",
                (
                    now.to_rfc3339_opts(chrono::SecondsFormat::Secs, true),
                    expired_id,
                    user_id,
                ),
            )
            .unwrap();

        let before = list_history_products_inner(&connection, now).unwrap();
        assert!(before
            .iter()
            .any(|item| item.id == expired_id && !item.is_deleted));

        connection
            .execute(
                "DELETE FROM products WHERE status = 'discontinued' AND discontinuedAt <= ?1",
                [crate::commands::products::expired_products_cutoff(now)],
            )
            .unwrap();

        let after = list_history_products_inner(&connection, now).unwrap();
        assert!(after
            .iter()
            .any(|item| item.id == expired_id && item.is_deleted));
    }

    #[test]
    fn list_history_products_requires_session_and_allows_both_roles() {
        use crate::test_utils::{assert_allows_both_roles, assert_requires_session};

        assert_requires_session(|connection, now| list_history_products_inner(connection, now));
        assert_allows_both_roles(|connection, now| list_history_products_inner(connection, now));
    }
}
