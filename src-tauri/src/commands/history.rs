use chrono::{DateTime, Local, NaiveDate, Utc};
use rusqlite::{Connection, OptionalExtension};
use tauri::State;

use crate::commands::auth::require_session;
use crate::commands::products::purge_best_effort;
use crate::db::connection::{current_app_data_dir, lock_conn};
use crate::errors::{log_error, map_db_error, AppError};
use crate::models::product::format_quantity_milli;
use crate::models::product_movement::{
    ProductHistoryFilters, ProductHistoryItem, ProductHistoryList, ProductHistoryOption,
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

#[cfg(test)]
mod tests {
    use crate::commands::auth::require_session;
    use crate::test_utils::{create_test_session, create_test_user, setup_test_db};
    use chrono::{TimeZone, Utc};

    use super::*;
    use crate::models::product_movement::ProductHistoryFilters;

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
