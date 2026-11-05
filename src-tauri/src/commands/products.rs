use std::path::{Path, PathBuf};

use chrono::{DateTime, Duration, Local, NaiveDate, Utc};
use rusqlite::{params, Connection, OptionalExtension};

use crate::db::connection::current_app_data_dir;
use crate::db::connection::lock_conn;
use crate::errors::{log_error, map_db_error, AppError};
use crate::models::product::{
    format_factor_micro, format_profit_percent_hundredths, format_quantity_milli,
    CreateProductInput, ExpiryFilter, PreviewSellingPriceInput, PriceQuote, ProductActionResult,
    ProductAssociatedInvoice, ProductConditionFilter, ProductConditionItem, ProductConditionList,
    ProductDetail, ProductFilters, ProductListItem, ProductMatch, ReservedInvoice, StockFilter,
    UnitOption, UpdateProductInput, UpdateProductResult,
};
use crate::pricing::selling_price_cents;
use crate::text::{clean_name, normalize_text, sort_spanish, spanish_sort_key};
use crate::timestamps::{format_utc_timestamp, parse_utc_timestamp};
use crate::validators::{
    parse_cost_cents, parse_percent_hundredths, parse_quantity_milli, validate_expiry_date,
    validate_name_with_messages, validate_optional_description,
};

const PURGE_SAVEPOINT: &str = "expired_products_purge";
const PURGE_FAILED_WITH_COUNT: &str =
    "No se pudieron limpiar {count} productos vencidos; se reintentará en la próxima operación";
const PURGE_FAILED_WITHOUT_COUNT: &str =
    "No se pudieron limpiar los productos vencidos; se reintentará en la próxima operación";

const PRODUCT_NAME_REQUIRED_MESSAGE: &str = "El nombre es obligatorio";
const PRODUCT_NAME_TOO_SHORT_MESSAGE: &str = "El nombre debe tener al menos 2 caracteres";
const PRODUCT_NAME_TOO_LONG_MESSAGE: &str = "El nombre no puede tener más de 100 caracteres";
const PRODUCT_NAME_FORBIDDEN_MESSAGE: &str = "El nombre contiene caracteres no permitidos";
const PRODUCT_DESCRIPTION_TOO_LONG_MESSAGE: &str =
    "La descripción no puede tener más de 500 caracteres";
const PRODUCT_DESCRIPTION_FORBIDDEN_MESSAGE: &str =
    "La descripción contiene caracteres no permitidos";
const PRODUCT_COST_RANGE_MESSAGE: &str =
    "El precio de entrada debe ser mayor que 0 y no superar 999.999,99";
const PRODUCT_QUANTITY_RANGE_MESSAGE: &str =
    "La cantidad inicial debe ser mayor que 0 y no superar 999.999,999";
const PRODUCT_PERCENT_RANGE_MESSAGE: &str =
    "El porcentaje de ganancia debe estar entre 0 y 1.000,00";
const PRODUCT_DUPLICATE_MESSAGE: &str = "Ya existe un producto con ese nombre";
const PRODUCT_NOT_FOUND_MESSAGE: &str = "El producto ya no existe";
const INVALID_PRODUCT_INPUT_MESSAGE: &str = "Los datos enviados no son válidos";
const PRODUCT_ACTIVATE_DUPLICATE_MESSAGE: &str =
    "No se puede activar: ya existe otro producto activo con ese nombre";

fn product_stock_value_cents(connection: &Connection, product_id: i64) -> Result<i64, AppError> {
    let value_cents: i64 = connection
        .query_row(
            "SELECT COALESCE(SUM(remainingMilli * unitCostCents), 0)
             FROM lots
             WHERE productId = ?1 AND remainingMilli > 0",
            [product_id],
            |row| row.get(0),
        )
        .map_err(|error| map_db_error(error, "productStockValue", &[]))?;
    Ok(crate::inventory::round_half_up(i128::from(value_cents), 1_000) as i64)
}
const PRODUCT_ALREADY_DISCONTINUED_MESSAGE: &str = "El producto ya está descontinuado";
const PRODUCT_ALREADY_ACTIVE_MESSAGE: &str = "El producto ya está activo";
const DISCONTINUED_PRODUCT_EDIT_MESSAGE: &str =
    "Un producto descontinuado no se puede editar. Actívalo primero";

type ProductDetailRow = (
    i64,
    String,
    Option<i64>,
    Option<String>,
    Option<i64>,
    Option<String>,
    Option<String>,
    i64,
    i64,
    i64,
    Option<String>,
    Option<String>,
    String,
    Option<String>,
);
type ProductUpdateRow = (i64, i64, Option<String>, String);

pub fn create_product_inner(
    connection: &mut Connection,
    now: DateTime<Utc>,
    today: NaiveDate,
    input: CreateProductInput,
) -> Result<ProductListItem, AppError> {
    let actor = crate::commands::auth::require_session(connection, now)?;

    let cleaned_name = clean_name(&input.name);
    if cleaned_name.is_empty() {
        return Err(validation_failed(
            PRODUCT_NAME_REQUIRED_MESSAGE,
            "product name is required",
        ));
    }
    let (name, normalized_name) = validate_name_with_messages(
        &input.name,
        2,
        100,
        PRODUCT_NAME_TOO_SHORT_MESSAGE,
        PRODUCT_NAME_TOO_LONG_MESSAGE,
        PRODUCT_NAME_FORBIDDEN_MESSAGE,
    )?;
    let description = validate_optional_description(
        input.description.as_deref(),
        500,
        PRODUCT_DESCRIPTION_TOO_LONG_MESSAGE,
        PRODUCT_DESCRIPTION_FORBIDDEN_MESSAGE,
    )?;

    let cost_cents = parse_cost_cents(&input.cost).map_err(|error| {
        product_decimal_error(error, "El precio de entrada", 2, PRODUCT_COST_RANGE_MESSAGE)
    })?;
    let quantity_milli = parse_quantity_milli(&input.quantity).map_err(|error| {
        product_decimal_error(
            error,
            "La cantidad inicial",
            3,
            PRODUCT_QUANTITY_RANGE_MESSAGE,
        )
    })?;
    let profit_percent_hundredths =
        parse_percent_hundredths(&input.profit_percent).map_err(|error| {
            product_decimal_error(
                error,
                "El porcentaje de ganancia",
                2,
                PRODUCT_PERCENT_RANGE_MESSAGE,
            )
        })?;
    let expires_on = input
        .expires_on
        .as_deref()
        .filter(|date| !date.trim().is_empty())
        .map(|date| validate_expiry_date(date, today, None))
        .transpose()?;

    let category_name = match input.category_id {
        Some(category_id) => Some(
            connection
                .query_row(
                    "SELECT name FROM categories WHERE id = ?1",
                    [category_id],
                    |row| row.get::<_, String>(0),
                )
                .optional()
                .map_err(|error| map_db_error(error, "createProduct", &[]))?
                .ok_or_else(|| {
                    AppError::new(
                        "notFound",
                        "La categoría ya no existe",
                        "category missing during product creation",
                    )
                })?,
        ),
        None => None,
    };
    let (unit_name, unit_abbreviation) = match input.unit_id {
        Some(unit_id) => connection
            .query_row(
                "SELECT name, abbreviation FROM units WHERE id = ?1",
                [unit_id],
                |row| Ok((row.get::<_, String>(0)?, row.get::<_, String>(1)?)),
            )
            .optional()
            .map_err(|error| map_db_error(error, "createProduct", &[]))?
            .map(|(name, abbreviation)| (Some(name), Some(abbreviation)))
            .ok_or_else(|| {
                AppError::new(
                    "notFound",
                    "La unidad ya no existe",
                    "unit missing during product creation",
                )
            })?,
        None => (None, None),
    };

    let timestamp = format_utc_timestamp(now);
    let transaction = connection
        .transaction()
        .map_err(|error| map_db_error(error, "createProduct", &[]))?;

    transaction
        .execute(
            "INSERT INTO products (
            name, nameNormalized, description, categoryId, unitId, costCents,
            profitPercentHundredths, expiresOn, quantityMilli, status, discontinuedAt,
            createdAt, updatedAt
         ) VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8, ?9, 'active', NULL, ?10, ?10)",
            params![
                name,
                normalized_name,
                description,
                input.category_id,
                input.unit_id,
                cost_cents,
                profit_percent_hundredths,
                expires_on,
                quantity_milli,
                timestamp
            ],
        )
        .map_err(|error| {
            map_db_error(
                error,
                "createProduct",
                &[("products.nameNormalized", PRODUCT_DUPLICATE_MESSAGE)],
            )
        })?;
    let product_id = transaction.last_insert_rowid();
    let arrival_order = crate::inventory::next_arrival_order(&transaction, product_id)?;
    let total_price_cents =
        crate::inventory::round_half_up(i128::from(quantity_milli) * i128::from(cost_cents), 1_000)
            as i64;

    transaction
        .execute(
            "INSERT INTO lots (
            productId, origin, invoiceLineId, arrivalOrder, initialMilli, remainingMilli,
            unitCostCents, createdAt, updatedAt
         ) VALUES (?1, 'initial', NULL, ?2, ?3, ?3, ?4, ?5, ?5)",
            params![
                product_id,
                arrival_order,
                quantity_milli,
                cost_cents,
                timestamp
            ],
        )
        .map_err(|error| map_db_error(error, "createProduct", &[]))?;

    transaction
        .execute(
            "INSERT INTO productMovements (
            createdAt, productId, productIdOriginal, productName, type, quantityMilli,
            stockAfterMilli, unitPriceCents, totalPriceCents, invoiceId, invoiceNumber,
            invoiceType, userId, userName
         ) VALUES (?1, ?2, ?2, ?3, 'initial', ?4, ?5, ?6, ?7, NULL, NULL, NULL, ?8, ?9)",
            params![
                timestamp,
                product_id,
                name,
                quantity_milli,
                quantity_milli,
                cost_cents,
                total_price_cents,
                Some(actor.id()),
                actor.username(),
            ],
        )
        .map_err(|error| map_db_error(error, "createProduct", &[]))?;

    let result = ProductListItem {
        id: product_id,
        name,
        category_id: input.category_id,
        category_name,
        unit_id: input.unit_id,
        unit_name,
        unit_abbreviation,
        quantity: format_quantity_milli(quantity_milli),
        cost_cents,
        valor_total_cents: product_stock_value_cents(&transaction, product_id)?,
        selling_price_cents: selling_price_cents(cost_cents, profit_percent_hundredths),
        profit_percent: format_profit_percent_hundredths(profit_percent_hundredths),
        expires_on,
        is_out_of_stock: false,
        is_low_stock: quantity_milli < crate::automation::low_stock_milli(&transaction),
    };

    // NT-02/NT-03: avisos de situación con el estado final (best-effort).
    crate::automation::evaluate_watches_best_effort(&transaction, now);
    transaction
        .commit()
        .map_err(|error| map_db_error(error, "createProduct", &[]))?;
    Ok(result)
}

#[tauri::command]
pub fn create_product(
    state: tauri::State<'_, std::sync::Mutex<Connection>>,
    input: CreateProductInput,
) -> Result<ProductListItem, AppError> {
    let mut connection = lock_conn(&state);
    let now = Utc::now();
    let today = Local::now().date_naive();
    let result = create_product_inner(&mut connection, now, today, input);
    log_product_command_error(&connection, "createProduct", &result, now);
    result
}

pub fn list_products_inner(
    connection: &mut Connection,
    now: DateTime<Utc>,
    today: NaiveDate,
    filters: ProductFilters,
) -> Result<Vec<ProductListItem>, AppError> {
    crate::commands::auth::require_session(connection, now)?;

    if filters.category_id.is_some() && filters.without_category {
        return Err(validation_failed(
            INVALID_PRODUCT_INPUT_MESSAGE,
            "listProducts received conflicting category filters",
        ));
    }

    let search = filters
        .search
        .as_deref()
        .map(clean_name)
        .filter(|value| !value.is_empty())
        .map(|value| normalize_text(&value))
        .unwrap_or_default();
    let stock_filter = match filters.stock {
        StockFilter::All => "all",
        StockFilter::Available => "available",
        StockFilter::OutOfStock => "outOfStock",
        StockFilter::Low => "low",
    };
    let expiry_filter = match filters.expiry {
        ExpiryFilter::All => "all",
        ExpiryFilter::WithExpiry => "withExpiry",
        ExpiryFilter::WithoutExpiry => "withoutExpiry",
        ExpiryFilter::Expired => "expired",
        ExpiryFilter::ExpiringSoon => "expiringSoon",
    };
    let today_text = today.format("%Y-%m-%d").to_string();
    let expiring_through = (today + Duration::days(29)).format("%Y-%m-%d").to_string();

    let low_stock_milli = crate::automation::low_stock_milli(connection);
    let mut statement = connection
        .prepare(
            "SELECT p.id, p.name, p.categoryId, c.name, p.unitId, u.name, u.abbreviation,
                    p.quantityMilli, p.costCents, p.profitPercentHundredths, p.expiresOn
             FROM products p
             LEFT JOIN categories c ON c.id = p.categoryId
             LEFT JOIN units u ON u.id = p.unitId
             WHERE p.status = 'active'
               AND (?1 = '' OR instr(p.nameNormalized, ?1) > 0)
               AND (?2 IS NULL OR p.categoryId = ?2)
               AND (?3 = 0 OR p.categoryId IS NULL)
               AND (
                    ?4 = 'all'
                    OR (?4 = 'available' AND p.quantityMilli > 0)
                    OR (?4 = 'outOfStock' AND p.quantityMilli = 0)
                    OR (?4 = 'low' AND p.quantityMilli > 0 AND p.quantityMilli <
                        COALESCE((SELECT CAST(value AS INTEGER) * 1000 FROM settings WHERE key = 'lowStockThreshold'), 5000))
               )
               AND (
                    ?5 = 'all'
                    OR (?5 = 'withExpiry' AND p.expiresOn IS NOT NULL)
                    OR (?5 = 'withoutExpiry' AND p.expiresOn IS NULL)
                    OR (?5 = 'expired' AND p.expiresOn < ?6)
                    OR (?5 = 'expiringSoon' AND p.expiresOn >= ?6 AND p.expiresOn <= ?7)
               )",
        )
        .map_err(|error| map_db_error(error, "listProducts", &[]))?;
    let mut rows = statement
        .query(params![
            search,
            filters.category_id,
            filters.without_category,
            stock_filter,
            expiry_filter,
            today_text,
            expiring_through
        ])
        .map_err(|error| map_db_error(error, "listProducts", &[]))?;

    let mut products = Vec::new();
    while let Some(row) = rows
        .next()
        .map_err(|error| map_db_error(error, "listProducts", &[]))?
    {
        let quantity_milli = row
            .get::<_, i64>(7)
            .map_err(|error| map_db_error(error, "listProducts", &[]))?;
        let cost_cents = row
            .get::<_, i64>(8)
            .map_err(|error| map_db_error(error, "listProducts", &[]))?;
        let profit_percent_hundredths = row
            .get::<_, i64>(9)
            .map_err(|error| map_db_error(error, "listProducts", &[]))?;

        let product_id = row
            .get(0)
            .map_err(|error| map_db_error(error, "listProducts", &[]))?;
        let valor_total_cents = product_stock_value_cents(connection, product_id)?;

        products.push(ProductListItem {
            id: product_id,
            name: row
                .get(1)
                .map_err(|error| map_db_error(error, "listProducts", &[]))?,
            category_id: row
                .get(2)
                .map_err(|error| map_db_error(error, "listProducts", &[]))?,
            category_name: row
                .get(3)
                .map_err(|error| map_db_error(error, "listProducts", &[]))?,
            unit_id: row
                .get(4)
                .map_err(|error| map_db_error(error, "listProducts", &[]))?,
            unit_name: row
                .get(5)
                .map_err(|error| map_db_error(error, "listProducts", &[]))?,
            unit_abbreviation: row
                .get(6)
                .map_err(|error| map_db_error(error, "listProducts", &[]))?,
            quantity: format_quantity_milli(quantity_milli),
            cost_cents,
            valor_total_cents,
            selling_price_cents: selling_price_cents(cost_cents, profit_percent_hundredths),
            profit_percent: format_profit_percent_hundredths(profit_percent_hundredths),
            expires_on: row
                .get(10)
                .map_err(|error| map_db_error(error, "listProducts", &[]))?,
            is_out_of_stock: quantity_milli == 0,
            is_low_stock: quantity_milli > 0 && quantity_milli < low_stock_milli,
        });
    }

    sort_spanish(&mut products, |product| {
        (spanish_sort_key(&product.name), product.id)
    });
    Ok(products)
}

#[tauri::command]
pub fn list_products(
    state: tauri::State<'_, std::sync::Mutex<Connection>>,
    filters: ProductFilters,
) -> Result<Vec<ProductListItem>, AppError> {
    let mut connection = lock_conn(&state);
    let now = Utc::now();
    let today = Local::now().date_naive();
    let result = list_products_inner(&mut connection, now, today, filters);
    log_product_command_error(&connection, "listProducts", &result, now);
    result
}

pub fn search_products_for_invoice_inner(
    connection: &mut Connection,
    now: DateTime<Utc>,
    query: String,
    purpose: Option<String>,
    limit: Option<usize>,
) -> Result<Vec<ProductMatch>, AppError> {
    crate::commands::auth::require_session(connection, now)?;
    let purpose = crate::search::ProductSearchPurpose::parse(purpose.as_deref())?;

    let transaction = connection
        .transaction()
        .map_err(|error| map_db_error(error, "searchProductsForInvoice", &[]))?;
    let _expired_cleanup_warning = purge_best_effort(&transaction, now)?;
    let limit = limit.unwrap_or(20).min(20);
    let results = crate::search::search_products_for_invoice(&transaction, &query, purpose, limit)?;

    transaction
        .commit()
        .map_err(|error| map_db_error(error, "searchProductsForInvoice", &[]))?;
    Ok(results)
}

#[tauri::command]
pub fn search_products_for_invoice(
    state: tauri::State<'_, std::sync::Mutex<Connection>>,
    query: String,
    purpose: Option<String>,
    limit: Option<usize>,
) -> Result<Vec<ProductMatch>, AppError> {
    let mut connection = lock_conn(&state);
    let now = Utc::now();
    let result = search_products_for_invoice_inner(&mut connection, now, query, purpose, limit);
    log_product_command_error(&connection, "searchProductsForInvoice", &result, now);
    result
}

pub fn get_product_associated_invoices_inner(
    connection: &Connection,
    now: DateTime<Utc>,
    product_id: i64,
    limit: Option<usize>,
) -> Result<Vec<ProductAssociatedInvoice>, AppError> {
    let _actor = crate::commands::auth::require_session(connection, now)?;
    // Purga best-effort (spec-03 §7.6). La respuesta es una lista sin campo de
    // aviso, así que un fallo solo se registra (purge_best_effort ya lo hace).
    if let Err(error) = purge_best_effort(connection, now) {
        if let Some(app_data_dir) = current_app_data_dir() {
            log_error(
                connection,
                "products",
                "getProductAssociatedInvoices",
                &error,
                now,
                app_data_dir,
            );
        }
    }

    let limit = limit.unwrap_or(50).min(50) as i64;
    let mut statement = connection
        .prepare(
            "SELECT i.id, i.number, i.type, i.invoiceDate, i.status, il.quantityMilli
             FROM invoiceLines il
             INNER JOIN invoices i ON i.id = il.invoiceId
             WHERE il.productIdOriginal = ?1
             ORDER BY i.invoiceDate DESC, i.id DESC
             LIMIT ?2",
        )
        .map_err(|error| map_db_error(error, "getProductAssociatedInvoices", &[]))?;

    let rows = statement
        .query_map([product_id, limit], |row| {
            Ok((
                row.get::<_, i64>(0)?,
                row.get::<_, String>(1)?,
                row.get::<_, String>(2)?,
                row.get::<_, String>(3)?,
                row.get::<_, Option<String>>(4)?,
                row.get::<_, i64>(5)?,
            ))
        })
        .map_err(|error| map_db_error(error, "getProductAssociatedInvoices", &[]))?
        .collect::<rusqlite::Result<Vec<_>>>()
        .map_err(|error| map_db_error(error, "getProductAssociatedInvoices", &[]))?;

    Ok(rows
        .into_iter()
        .map(
            |(invoice_id, invoice_number, invoice_type, invoice_date, status, quantity_milli)| {
                ProductAssociatedInvoice {
                    invoice_id,
                    invoice_number,
                    invoice_type,
                    invoice_date,
                    status,
                    quantity: format_quantity_milli(quantity_milli),
                }
            },
        )
        .collect())
}

#[tauri::command]
pub fn get_product_associated_invoices(
    state: tauri::State<'_, std::sync::Mutex<Connection>>,
    product_id: i64,
    limit: Option<usize>,
) -> Result<Vec<ProductAssociatedInvoice>, AppError> {
    let connection = lock_conn(&state);
    let now = Utc::now();
    let result = get_product_associated_invoices_inner(&connection, now, product_id, limit);
    log_product_command_error(&connection, "getProductAssociatedInvoices", &result, now);
    result
}

/// CV-01: unidades que ofrece el selector de una línea para este producto,
/// la suya primero; un producto sin unidad no ofrece conversiones.
pub fn list_unit_options_for_product_inner(
    connection: &Connection,
    now: DateTime<Utc>,
    product_id: i64,
) -> Result<Vec<UnitOption>, AppError> {
    crate::commands::auth::require_session(connection, now)?;
    let product_unit: Option<(i64, String, String)> = connection
        .query_row(
            "SELECT u.id, u.name, u.abbreviation FROM products p
             LEFT JOIN units u ON u.id = p.unitId WHERE p.id = ?1",
            [product_id],
            |row| {
                Ok(row.get::<_, Option<i64>>(0)?.map(|id| {
                    (
                        id,
                        row.get(1).unwrap_or_default(),
                        row.get(2).unwrap_or_default(),
                    )
                }))
            },
        )
        .optional()
        .map_err(|error| map_db_error(error, "listUnitOptionsForProduct", &[]))?
        .ok_or_else(|| {
            AppError::new(
                "notFound",
                PRODUCT_NOT_FOUND_MESSAGE,
                "product missing while listing unit options",
            )
        })?;
    let Some((unit_id, name, abbreviation)) = product_unit else {
        return Ok(vec![]);
    };
    let mut options = vec![UnitOption {
        unit_id,
        name,
        abbreviation,
        factor: "1".to_owned(),
    }];
    let neighbours: Vec<i64> = connection
        .prepare(
            "SELECT CASE WHEN fromUnitId = ?1 THEN toUnitId ELSE fromUnitId END
             FROM unitConversions WHERE fromUnitId = ?1 OR toUnitId = ?1",
        )
        .and_then(|mut statement| statement.query_map([unit_id], |row| row.get(0))?.collect())
        .map_err(|error| map_db_error(error, "listUnitOptionsForProduct", &[]))?;
    for neighbour in neighbours {
        // El mismo cálculo que se aplicará al guardar la línea.
        if let Some(line_unit) =
            crate::inventory::resolve_line_unit(connection, product_id, Some(neighbour), 1_000)?
        {
            options.push(UnitOption {
                unit_id: neighbour,
                name: line_unit.name.clone(),
                abbreviation: line_unit.abbreviation.clone(),
                factor: format_factor_micro(line_unit.factor_micro()),
            });
        }
    }
    sort_spanish(&mut options[1..], |option| {
        (spanish_sort_key(&option.name), option.unit_id)
    });
    Ok(options)
}

#[tauri::command]
pub fn list_unit_options_for_product(
    state: tauri::State<'_, std::sync::Mutex<Connection>>,
    product_id: i64,
) -> Result<Vec<UnitOption>, AppError> {
    let connection = lock_conn(&state);
    let now = Utc::now();
    let result = list_unit_options_for_product_inner(&connection, now, product_id);
    log_product_command_error(&connection, "listUnitOptionsForProduct", &result, now);
    result
}

pub fn list_out_of_stock_and_discontinued_inner(
    connection: &mut Connection,
    now: DateTime<Utc>,
    search: Option<&str>,
    condition: Option<ProductConditionFilter>,
) -> Result<ProductConditionList, AppError> {
    crate::commands::auth::require_session(connection, now)?;

    let transaction = connection
        .transaction()
        .map_err(|error| map_db_error(error, "listOutOfStockAndDiscontinued", &[]))?;
    let expired_cleanup_warning = purge_best_effort(&transaction, now)?;
    let cutoff = expired_products_cutoff(now);
    let normalized_search = search
        .map(clean_name)
        .filter(|value| !value.is_empty())
        .map(|value| normalize_text(&value))
        .unwrap_or_default();
    let condition = match condition.unwrap_or_default() {
        ProductConditionFilter::All => "all",
        ProductConditionFilter::OutOfStock => "outOfStock",
        ProductConditionFilter::Discontinued => "discontinued",
    };

    let mut statement = transaction
        .prepare(
            "SELECT p.id, p.name, p.categoryId, c.name, p.unitId, u.name, u.abbreviation,
                    p.quantityMilli, p.status, p.discontinuedAt
             FROM products p
             LEFT JOIN categories c ON c.id = p.categoryId
             LEFT JOIN units u ON u.id = p.unitId
             WHERE (p.status = 'discontinued' OR p.quantityMilli = 0)
               AND (
                    ?1 = 'all'
                    OR (?1 = 'outOfStock' AND p.quantityMilli = 0)
                    OR (?1 = 'discontinued' AND p.status = 'discontinued')
               )
               AND (?2 = '' OR instr(p.nameNormalized, ?2) > 0)
               AND (p.status <> 'discontinued' OR p.discontinuedAt > ?3)",
        )
        .map_err(|error| map_db_error(error, "listOutOfStockAndDiscontinued", &[]))?;
    let mut rows = statement
        .query(params![condition, normalized_search, cutoff])
        .map_err(|error| map_db_error(error, "listOutOfStockAndDiscontinued", &[]))?;

    let mut items = Vec::new();
    while let Some(row) = rows
        .next()
        .map_err(|error| map_db_error(error, "listOutOfStockAndDiscontinued", &[]))?
    {
        let id = row
            .get(0)
            .map_err(|error| map_db_error(error, "listOutOfStockAndDiscontinued", &[]))?;
        let name = row
            .get(1)
            .map_err(|error| map_db_error(error, "listOutOfStockAndDiscontinued", &[]))?;
        let quantity_milli = row
            .get::<_, i64>(7)
            .map_err(|error| map_db_error(error, "listOutOfStockAndDiscontinued", &[]))?;
        let status = row
            .get::<_, String>(8)
            .map_err(|error| map_db_error(error, "listOutOfStockAndDiscontinued", &[]))?;
        let discontinued_at = row
            .get::<_, Option<String>>(9)
            .map_err(|error| map_db_error(error, "listOutOfStockAndDiscontinued", &[]))?;
        let is_discontinued = status == "discontinued";
        let remaining_days = if is_discontinued {
            Some(remaining_product_days(
                now,
                discontinued_at.as_deref().ok_or_else(|| {
                    internal_product_error("discontinued product is missing its timestamp")
                })?,
            )?)
        } else {
            None
        };

        items.push(ProductConditionItem {
            id,
            name,
            category_name: row
                .get(3)
                .map_err(|error| map_db_error(error, "listOutOfStockAndDiscontinued", &[]))?,
            unit_name: row
                .get(5)
                .map_err(|error| map_db_error(error, "listOutOfStockAndDiscontinued", &[]))?,
            unit_abbreviation: row
                .get(6)
                .map_err(|error| map_db_error(error, "listOutOfStockAndDiscontinued", &[]))?,
            quantity: format_quantity_milli(quantity_milli),
            is_out_of_stock: quantity_milli == 0,
            is_discontinued,
            remaining_days,
        });
    }

    drop(rows);
    drop(statement);
    sort_spanish(&mut items, |item| (spanish_sort_key(&item.name), item.id));
    transaction
        .commit()
        .map_err(|error| map_db_error(error, "listOutOfStockAndDiscontinued", &[]))?;

    Ok(ProductConditionList {
        items,
        expired_cleanup_warning,
    })
}

#[tauri::command]
pub fn list_out_of_stock_and_discontinued(
    state: tauri::State<'_, std::sync::Mutex<Connection>>,
    search: Option<String>,
    condition: Option<ProductConditionFilter>,
) -> Result<ProductConditionList, AppError> {
    let mut connection = lock_conn(&state);
    let now = Utc::now();
    let result = list_out_of_stock_and_discontinued_inner(
        &mut connection,
        now,
        search.as_deref(),
        condition,
    );
    log_product_command_error(&connection, "listOutOfStockAndDiscontinued", &result, now);
    result
}

pub fn get_product_inner(
    connection: &mut Connection,
    now: DateTime<Utc>,
    product_id: i64,
) -> Result<ProductDetail, AppError> {
    crate::commands::auth::require_session(connection, now)?;

    let transaction = connection
        .transaction()
        .map_err(|error| map_db_error(error, "getProduct", &[]))?;
    let expired_cleanup_warning = purge_best_effort(&transaction, now)?;
    let cutoff = expired_products_cutoff(now);
    let product: Option<ProductDetailRow> = transaction
        .query_row(
            "SELECT p.id, p.name, p.categoryId, c.name, p.unitId, u.name, u.abbreviation,
                    p.quantityMilli, p.costCents, p.profitPercentHundredths, p.expiresOn,
                    p.description, p.status, p.discontinuedAt
             FROM products p
             LEFT JOIN categories c ON c.id = p.categoryId
             LEFT JOIN units u ON u.id = p.unitId
             WHERE p.id = ?1
               AND (p.status <> 'discontinued' OR p.discontinuedAt > ?2)",
            params![product_id, cutoff],
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
                ))
            },
        )
        .optional()
        .map_err(|error| map_db_error(error, "getProduct", &[]))?;

    let Some((
        id,
        name,
        category_id,
        category_name,
        unit_id,
        unit_name,
        unit_abbreviation,
        quantity_milli,
        cost_cents,
        profit_percent_hundredths,
        expires_on,
        description,
        status,
        discontinued_at,
    )) = product
    else {
        transaction
            .commit()
            .map_err(|error| map_db_error(error, "getProduct", &[]))?;
        return Err(AppError::new(
            "notFound",
            PRODUCT_NOT_FOUND_MESSAGE,
            "product missing during detail lookup",
        ));
    };

    let (is_discontinued, remaining_days) = match status.as_str() {
        "active" => (false, None),
        "discontinued" => {
            let discontinued_at = discontinued_at.as_deref().ok_or_else(|| {
                internal_product_error("discontinued product is missing its timestamp")
            })?;
            (true, Some(remaining_product_days(now, discontinued_at)?))
        }
        _ => return Err(internal_product_error("product has an invalid status")),
    };

    let reserved_invoices = reserved_invoices_of_product(&transaction, id)?;
    let reserved_milli: i64 = reserved_invoices.iter().map(|(_, milli)| milli).sum();
    let detail = ProductDetail {
        product: ProductListItem {
            id,
            name,
            category_id,
            category_name,
            unit_id,
            unit_name,
            unit_abbreviation,
            quantity: format_quantity_milli(quantity_milli),
            cost_cents,
            valor_total_cents: product_stock_value_cents(&transaction, id)?,
            selling_price_cents: selling_price_cents(cost_cents, profit_percent_hundredths),
            profit_percent: format_profit_percent_hundredths(profit_percent_hundredths),
            expires_on,
            is_out_of_stock: quantity_milli == 0,
            is_low_stock: quantity_milli > 0
                && quantity_milli < crate::automation::low_stock_milli(&transaction),
        },
        description,
        is_discontinued,
        discontinued_at,
        remaining_days,
        expired_cleanup_warning,
        reserved_quantity: format_quantity_milli(reserved_milli),
        reserved_invoices: reserved_invoices
            .into_iter()
            .map(|(invoice, _)| invoice)
            .collect(),
    };

    transaction
        .commit()
        .map_err(|error| map_db_error(error, "getProduct", &[]))?;
    Ok(detail)
}

/// RS-03: reservas activas con este producto y la cantidad de cada una.
fn reserved_invoices_of_product(
    connection: &Connection,
    product_id: i64,
) -> Result<Vec<(ReservedInvoice, i64)>, AppError> {
    connection
        .prepare(
            "SELECT i.id, i.number, i.type, i.reservedUntil, SUM(l.quantityMilli)
             FROM invoices i JOIN invoiceLines l ON l.invoiceId = i.id
             WHERE l.productId = ?1 AND i.status = 'pending' AND i.reservedUntil IS NOT NULL
             GROUP BY i.id ORDER BY i.reservedUntil, i.id",
        )
        .and_then(|mut statement| {
            statement
                .query_map([product_id], |row| {
                    let milli: i64 = row.get(4)?;
                    Ok((
                        ReservedInvoice {
                            id: row.get(0)?,
                            number: row.get(1)?,
                            invoice_type: row.get(2)?,
                            reserved_until: row.get(3)?,
                            quantity: format_quantity_milli(milli),
                        },
                        milli,
                    ))
                })?
                .collect()
        })
        .map_err(|error| map_db_error(error, "getProduct", &[]))
}

#[tauri::command]
pub fn get_product(
    state: tauri::State<'_, std::sync::Mutex<Connection>>,
    product_id: i64,
) -> Result<ProductDetail, AppError> {
    let mut connection = lock_conn(&state);
    let now = Utc::now();
    let result = get_product_inner(&mut connection, now, product_id);
    log_product_command_error(&connection, "getProduct", &result, now);
    result
}

pub fn update_product_inner(
    connection: &mut Connection,
    now: DateTime<Utc>,
    input: UpdateProductInput,
) -> Result<UpdateProductResult, AppError> {
    crate::commands::auth::require_session(connection, now)?;

    let expired_cleanup_warning = purge_best_effort(connection, now)?;
    let transaction = connection
        .transaction()
        .map_err(|error| map_db_error(error, "updateProduct", &[]))?;

    let current: Option<ProductUpdateRow> = transaction
        .query_row(
            "SELECT costCents, quantityMilli, expiresOn, status
             FROM products WHERE id = ?1",
            [input.product_id],
            |row| Ok((row.get(0)?, row.get(1)?, row.get(2)?, row.get(3)?)),
        )
        .optional()
        .map_err(|error| map_db_error(error, "updateProduct", &[]))?;

    let Some((cost_cents, quantity_milli, current_expires_on, status)) = current else {
        return Err(AppError::new(
            "notFound",
            PRODUCT_NOT_FOUND_MESSAGE,
            "product missing during update",
        ));
    };

    if status == "discontinued" {
        return Err(AppError::new(
            "invalidState",
            DISCONTINUED_PRODUCT_EDIT_MESSAGE,
            "update requested for discontinued product",
        ));
    }

    let (cleaned_name, normalized_name) = validate_name_with_messages(
        &input.name,
        2,
        100,
        PRODUCT_NAME_TOO_SHORT_MESSAGE,
        PRODUCT_NAME_TOO_LONG_MESSAGE,
        PRODUCT_NAME_FORBIDDEN_MESSAGE,
    )?;
    let description = validate_optional_description(
        input.description.as_deref(),
        500,
        PRODUCT_DESCRIPTION_TOO_LONG_MESSAGE,
        PRODUCT_DESCRIPTION_FORBIDDEN_MESSAGE,
    )?;
    let category_name = match input.category_id {
        Some(category_id) => Some(
            transaction
                .query_row(
                    "SELECT name FROM categories WHERE id = ?1",
                    [category_id],
                    |row| row.get::<_, String>(0),
                )
                .optional()
                .map_err(|error| map_db_error(error, "updateProduct", &[]))?
                .ok_or_else(|| {
                    AppError::new(
                        "notFound",
                        "La categoría ya no existe",
                        "category missing during product update",
                    )
                })?,
        ),
        None => None,
    };
    let (unit_name, unit_abbreviation) = match input.unit_id {
        Some(unit_id) => transaction
            .query_row(
                "SELECT name, abbreviation FROM units WHERE id = ?1",
                [unit_id],
                |row| Ok((row.get::<_, String>(0)?, row.get::<_, String>(1)?)),
            )
            .optional()
            .map_err(|error| map_db_error(error, "updateProduct", &[]))?
            .map(|(name, abbreviation)| (Some(name), Some(abbreviation)))
            .ok_or_else(|| {
                AppError::new(
                    "notFound",
                    "La unidad ya no existe",
                    "unit missing during product update",
                )
            })?,
        None => (None, None),
    };

    let today = Local::now().date_naive();
    let expires_on = match input
        .expires_on
        .as_deref()
        .filter(|date| !date.trim().is_empty())
    {
        Some(raw) => Some(validate_expiry_date(
            raw,
            today,
            current_expires_on.as_deref(),
        )?),
        None => None,
    };
    let profit_percent_hundredths =
        parse_percent_hundredths(&input.profit_percent).map_err(|error| {
            product_decimal_error(
                error,
                "El porcentaje de ganancia",
                2,
                PRODUCT_PERCENT_RANGE_MESSAGE,
            )
        })?;

    let timestamp = format_utc_timestamp(now);
    transaction
        .execute(
            "UPDATE products
             SET name = ?1, nameNormalized = ?2, description = ?3, categoryId = ?4,
                 unitId = ?5, profitPercentHundredths = ?6, expiresOn = ?7,
                 updatedAt = ?8
             WHERE id = ?9",
            params![
                cleaned_name,
                normalized_name,
                description,
                input.category_id,
                input.unit_id,
                profit_percent_hundredths,
                expires_on,
                timestamp,
                input.product_id
            ],
        )
        .map_err(|error| {
            map_db_error(
                error,
                "updateProduct",
                &[("products.nameNormalized", PRODUCT_DUPLICATE_MESSAGE)],
            )
        })?;

    let result = ProductListItem {
        id: input.product_id,
        name: cleaned_name,
        category_id: input.category_id,
        category_name,
        unit_id: input.unit_id,
        unit_name,
        unit_abbreviation,
        quantity: format_quantity_milli(quantity_milli),
        cost_cents,
        valor_total_cents: product_stock_value_cents(&transaction, input.product_id)?,
        selling_price_cents: selling_price_cents(cost_cents, profit_percent_hundredths),
        profit_percent: format_profit_percent_hundredths(profit_percent_hundredths),
        expires_on,
        is_out_of_stock: quantity_milli == 0,
        is_low_stock: quantity_milli > 0
            && quantity_milli < crate::automation::low_stock_milli(&transaction),
    };

    // NT-02/NT-03: avisos de situación con el estado final (best-effort).
    crate::automation::evaluate_watches_best_effort(&transaction, now);
    transaction
        .commit()
        .map_err(|error| map_db_error(error, "updateProduct", &[]))?;
    Ok(UpdateProductResult {
        product: result,
        expired_cleanup_warning,
    })
}

#[tauri::command]
pub fn update_product(
    state: tauri::State<'_, std::sync::Mutex<Connection>>,
    input: UpdateProductInput,
) -> Result<UpdateProductResult, AppError> {
    let mut connection = lock_conn(&state);
    let now = Utc::now();
    let result = update_product_inner(&mut connection, now, input);
    log_product_command_error(&connection, "updateProduct", &result, now);
    result
}

pub fn discontinue_product_inner(
    connection: &mut Connection,
    now: DateTime<Utc>,
    product_id: i64,
) -> Result<ProductActionResult, AppError> {
    transition_product_status(connection, now, product_id, false, "discontinueProduct")
}

#[tauri::command]
pub fn discontinue_product(
    state: tauri::State<'_, std::sync::Mutex<Connection>>,
    product_id: i64,
) -> Result<ProductActionResult, AppError> {
    let mut connection = lock_conn(&state);
    let now = Utc::now();
    let result = discontinue_product_inner(&mut connection, now, product_id);
    log_product_command_error(&connection, "discontinueProduct", &result, now);
    result
}

pub fn activate_product_inner(
    connection: &mut Connection,
    now: DateTime<Utc>,
    product_id: i64,
) -> Result<ProductActionResult, AppError> {
    transition_product_status(connection, now, product_id, true, "activateProduct")
}

#[tauri::command]
pub fn activate_product(
    state: tauri::State<'_, std::sync::Mutex<Connection>>,
    product_id: i64,
) -> Result<ProductActionResult, AppError> {
    let mut connection = lock_conn(&state);
    let now = Utc::now();
    let result = activate_product_inner(&mut connection, now, product_id);
    log_product_command_error(&connection, "activateProduct", &result, now);
    result
}

pub fn delete_product_inner(
    connection: &mut Connection,
    now: DateTime<Utc>,
    product_id: i64,
) -> Result<ProductActionResult, AppError> {
    let actor = crate::commands::auth::require_session(connection, now)?;

    let transaction = connection
        .transaction()
        .map_err(|error| map_db_error(error, "deleteProduct", &[]))?;
    let expired_cleanup_warning = purge_best_effort(&transaction, now)?;
    let cutoff = expired_products_cutoff(now);
    let product_exists: Option<i64> = transaction
        .query_row(
            "SELECT id FROM products
             WHERE id = ?1 AND (status <> 'discontinued' OR discontinuedAt > ?2)",
            params![product_id, cutoff],
            |row| row.get(0),
        )
        .optional()
        .map_err(|error| map_db_error(error, "deleteProduct", &[]))?;

    let Some(_) = product_exists else {
        transaction
            .commit()
            .map_err(|error| map_db_error(error, "deleteProduct", &[]))?;
        return Err(AppError::new(
            "notFound",
            PRODUCT_NOT_FOUND_MESSAGE,
            "product missing during manual deletion",
        ));
    };

    crate::activity::record_deletion(
        &transaction,
        &crate::commands::auth::Author::User(&actor),
        now,
        "product",
        "SELECT name FROM products WHERE id = ?1",
        product_id,
    )
    .map_err(|error| map_db_error(error, "deleteProduct", &[]))?;
    let deleted_rows = transaction
        .execute("DELETE FROM products WHERE id = ?1", [product_id])
        .map_err(|error| map_db_error(error, "deleteProduct", &[]))?;
    if deleted_rows != 1 {
        transaction
            .commit()
            .map_err(|error| map_db_error(error, "deleteProduct", &[]))?;
        return Err(AppError::new(
            "notFound",
            PRODUCT_NOT_FOUND_MESSAGE,
            "product missing during manual deletion",
        ));
    }

    // NT-02/NT-03: avisos de situación con el estado final (best-effort).
    crate::automation::evaluate_watches_best_effort(&transaction, now);
    transaction
        .commit()
        .map_err(|error| map_db_error(error, "deleteProduct", &[]))?;
    Ok(ProductActionResult {
        expired_cleanup_warning,
    })
}

#[tauri::command]
pub fn delete_product(
    state: tauri::State<'_, std::sync::Mutex<Connection>>,
    product_id: i64,
) -> Result<ProductActionResult, AppError> {
    let mut connection = lock_conn(&state);
    let now = Utc::now();
    let result = delete_product_inner(&mut connection, now, product_id);
    log_product_command_error(&connection, "deleteProduct", &result, now);
    result
}

fn transition_product_status(
    connection: &mut Connection,
    now: DateTime<Utc>,
    product_id: i64,
    activate: bool,
    operation: &str,
) -> Result<ProductActionResult, AppError> {
    crate::commands::auth::require_session(connection, now)?;

    let transaction = connection
        .transaction()
        .map_err(|error| map_db_error(error, operation, &[]))?;
    let expired_cleanup_warning = purge_best_effort(&transaction, now)?;
    let cutoff = expired_products_cutoff(now);
    let status: Option<String> = transaction
        .query_row(
            "SELECT status FROM products
             WHERE id = ?1 AND (status <> 'discontinued' OR discontinuedAt > ?2)",
            params![product_id, cutoff],
            |row| row.get(0),
        )
        .optional()
        .map_err(|error| map_db_error(error, operation, &[]))?;

    let Some(status) = status else {
        transaction
            .commit()
            .map_err(|error| map_db_error(error, operation, &[]))?;
        return Err(AppError::new(
            "notFound",
            PRODUCT_NOT_FOUND_MESSAGE,
            "product missing during status transition",
        ));
    };

    let expected_status = if activate { "discontinued" } else { "active" };
    if status != expected_status {
        let (message, cause) = if activate {
            (
                PRODUCT_ALREADY_ACTIVE_MESSAGE,
                "active product activation requested",
            )
        } else {
            (
                PRODUCT_ALREADY_DISCONTINUED_MESSAGE,
                "discontinuation requested for discontinued product",
            )
        };
        transaction
            .commit()
            .map_err(|error| map_db_error(error, operation, &[]))?;
        return Err(AppError::new("invalidState", message, cause));
    }

    let timestamp = format_utc_timestamp(now);
    let update_result = if activate {
        transaction.execute(
            "UPDATE products SET status = 'active', discontinuedAt = NULL, updatedAt = ?1
             WHERE id = ?2",
            params![timestamp, product_id],
        )
    } else {
        transaction.execute(
            "UPDATE products SET status = 'discontinued', discontinuedAt = ?1, updatedAt = ?1
             WHERE id = ?2",
            params![timestamp, product_id],
        )
    };

    if let Err(error) = update_result {
        let mapped_error = map_db_error(
            error,
            operation,
            &[(
                "products.nameNormalized",
                PRODUCT_ACTIVATE_DUPLICATE_MESSAGE,
            )],
        );
        transaction
            .commit()
            .map_err(|commit_error| map_db_error(commit_error, operation, &[]))?;
        return Err(mapped_error);
    }

    // NT-02/NT-03: avisos de situación con el estado final (best-effort).
    crate::automation::evaluate_watches_best_effort(&transaction, now);
    transaction
        .commit()
        .map_err(|error| map_db_error(error, operation, &[]))?;
    Ok(ProductActionResult {
        expired_cleanup_warning,
    })
}

pub(crate) fn remaining_product_days(
    now: DateTime<Utc>,
    discontinued_at: &str,
) -> Result<i64, AppError> {
    let discontinued_at = parse_utc_timestamp(discontinued_at).ok_or_else(|| {
        internal_product_error("product has an invalid discontinuation timestamp")
    })?;
    let remaining = Duration::days(60) - (now - discontinued_at);
    if remaining <= Duration::zero() {
        return Ok(0);
    }

    let whole_days = remaining.num_days();
    let days_remaining = if remaining > Duration::days(whole_days) {
        whole_days + 1
    } else {
        whole_days
    };
    Ok(days_remaining.clamp(0, 60))
}

fn internal_product_error(cause: &str) -> AppError {
    AppError::new("internal", "Ocurrió un error inesperado", cause)
}

pub fn preview_selling_price_inner(
    connection: &mut Connection,
    now: DateTime<Utc>,
    input: PreviewSellingPriceInput,
) -> Result<PriceQuote, AppError> {
    crate::commands::auth::require_session(connection, now)?;

    if input.product_id.is_some() == input.cost.is_some() {
        return Err(validation_failed(
            INVALID_PRODUCT_INPUT_MESSAGE,
            "previewSellingPrice requires exactly one cost source",
        ));
    }

    let cost_cents = if let Some(product_id) = input.product_id {
        let cutoff = expired_products_cutoff(now);
        let product: Option<(i64, String)> = connection
            .query_row(
                "SELECT costCents, status FROM products
                 WHERE id = ?1 AND (status <> 'discontinued' OR discontinuedAt > ?2)",
                params![product_id, cutoff],
                |row| Ok((row.get(0)?, row.get(1)?)),
            )
            .optional()
            .map_err(|error| map_db_error(error, "previewSellingPrice", &[]))?;
        let Some((cost_cents, status)) = product else {
            return Err(AppError::new(
                "notFound",
                PRODUCT_NOT_FOUND_MESSAGE,
                "product missing during selling price preview",
            ));
        };
        if status == "discontinued" {
            return Err(AppError::new(
                "invalidState",
                DISCONTINUED_PRODUCT_EDIT_MESSAGE,
                "selling price preview requested for discontinued product",
            ));
        }
        cost_cents
    } else {
        parse_cost_cents(input.cost.as_deref().unwrap_or_default()).map_err(|error| {
            product_decimal_error(error, "El precio de entrada", 2, PRODUCT_COST_RANGE_MESSAGE)
        })?
    };

    let profit_percent_hundredths =
        parse_percent_hundredths(&input.profit_percent).map_err(|error| {
            product_decimal_error(
                error,
                "El porcentaje de ganancia",
                2,
                PRODUCT_PERCENT_RANGE_MESSAGE,
            )
        })?;

    Ok(PriceQuote {
        selling_price_cents: selling_price_cents(cost_cents, profit_percent_hundredths),
    })
}

#[tauri::command]
pub fn preview_selling_price(
    state: tauri::State<'_, std::sync::Mutex<Connection>>,
    input: PreviewSellingPriceInput,
) -> Result<PriceQuote, AppError> {
    let mut connection = lock_conn(&state);
    let now = Utc::now();
    let result = preview_selling_price_inner(&mut connection, now, input);
    log_product_command_error(&connection, "previewSellingPrice", &result, now);
    result
}

fn product_decimal_error(
    error: AppError,
    field: &str,
    max_decimals: usize,
    range_message: &str,
) -> AppError {
    let message = match error.message.as_str() {
        "El valor ingresado no es válido" => format!(
            "{field} no tiene un formato válido. Escribe solo dígitos, con una coma o un punto para los decimales"
        ),
        "El valor tiene un formato ambiguo" => format!(
            "{field} no usa separador de miles. Escribe los decimales con coma (por ejemplo, 1,234)"
        ),
        "El valor tiene demasiados decimales" => {
            format!("{field} admite como máximo {max_decimals} decimales")
        }
        "El valor está fuera de rango" => range_message.to_owned(),
        _ => error.message,
    };
    validation_failed(&message, "product numeric input validation failed")
}

fn validation_failed(message: &str, cause: &str) -> AppError {
    AppError::new("validationFailed", message, cause)
}

fn log_product_command_error<T>(
    connection: &Connection,
    operation: &str,
    result: &Result<T, AppError>,
    now: DateTime<Utc>,
) {
    if let Err(error) = result {
        if error.should_log() {
            if let Some(app_data_dir) = current_app_data_dir() {
                log_error(connection, "products", operation, error, now, app_data_dir);
            }
        }
    }
}

pub fn expired_products_cutoff(now: DateTime<Utc>) -> String {
    format_utc_timestamp(now - Duration::days(60))
}

pub fn purge_expired_products(
    connection: &Connection,
    now: DateTime<Utc>,
) -> Result<usize, AppError> {
    let cutoff = expired_products_cutoff(now);
    let purged: Vec<(i64, String)> = connection
        .prepare(
            "SELECT id, name FROM products
             WHERE status = 'discontinued' AND discontinuedAt <= ?1",
        )
        .and_then(|mut statement| {
            statement
                .query_map([&cutoff], |row| Ok((row.get(0)?, row.get(1)?)))?
                .collect()
        })
        .map_err(|error| map_db_error(error, "purge_expired_products", &[]))?;
    let deleted = connection
        .execute(
            "DELETE FROM products
             WHERE status = 'discontinued' AND discontinuedAt <= ?1",
            [&cutoff],
        )
        .map_err(|error| map_db_error(error, "purge_expired_products", &[]))?;
    // N-12 (NT-02): en el mismo SAVEPOINT de la purga.
    crate::db::notifications::insert_purged_products_notification(connection, &purged, now)
        .map_err(|error| map_db_error(error, "purge_expired_products", &[]))?;
    for (_, name) in &purged {
        crate::activity::record_activity(
            connection,
            &crate::commands::auth::Author::System,
            now,
            crate::activity::Entry {
                action: "purge",
                entity_type: "product",
                label: name,
                details: None,
            },
        )
        .map_err(|error| map_db_error(error, "purge_expired_products", &[]))?;
    }
    Ok(deleted)
}

pub fn purge_best_effort(
    connection: &Connection,
    now: DateTime<Utc>,
) -> Result<Option<String>, AppError> {
    let fallback_directory: PathBuf = std::env::temp_dir();
    let app_data_dir = current_app_data_dir().unwrap_or(&fallback_directory);
    let warning = purge_best_effort_with_app_data_dir(connection, now, app_data_dir)?;
    // spec-04 S-04, D-09 (`before_operation`): las reservas vencidas se
    // cancelan antes de cada operación, en el mismo punto que la purga.
    crate::automation::expire_reservations_best_effort(connection, now);
    Ok(warning)
}

fn purge_best_effort_with_app_data_dir(
    connection: &Connection,
    now: DateTime<Utc>,
    app_data_dir: &Path,
) -> Result<Option<String>, AppError> {
    connection
        .execute_batch(&format!("SAVEPOINT {PURGE_SAVEPOINT}"))
        .map_err(|error| {
            let error = map_db_error(error, "purge_expired_products_savepoint", &[]);
            log_purge_error(connection, &error, now, app_data_dir);
            error
        })?;

    match purge_expired_products(connection, now) {
        Ok(_) => {
            connection
                .execute_batch(&format!("RELEASE SAVEPOINT {PURGE_SAVEPOINT}"))
                .map_err(|error| {
                    let error = map_db_error(error, "purge_expired_products_release", &[]);
                    log_purge_error(connection, &error, now, app_data_dir);
                    error
                })?;
            Ok(None)
        }
        Err(error) => {
            connection
                .execute_batch(&format!(
                    "ROLLBACK TO SAVEPOINT {PURGE_SAVEPOINT}; RELEASE SAVEPOINT {PURGE_SAVEPOINT}"
                ))
                .map_err(|rollback_error| {
                    let rollback_error =
                        map_db_error(rollback_error, "purge_expired_products_rollback", &[]);
                    log_purge_error(connection, &error, now, app_data_dir);
                    log_purge_error(connection, &rollback_error, now, app_data_dir);
                    rollback_error
                })?;
            log_purge_error(connection, &error, now, app_data_dir);

            let cutoff = expired_products_cutoff(now);
            let expired_count = connection.query_row(
                "SELECT COUNT(*) FROM products
                 WHERE status = 'discontinued' AND discontinuedAt <= ?1",
                [&cutoff],
                |row| row.get::<_, i64>(0),
            );

            match expired_count {
                Ok(count) if count > 0 => Ok(Some(
                    PURGE_FAILED_WITH_COUNT.replace("{count}", &count.to_string()),
                )),
                Ok(_) => Ok(None),
                Err(count_error) => {
                    let count_error = map_db_error(
                        count_error,
                        "count_expired_products_after_purge_failure",
                        &[],
                    );
                    log_purge_error(connection, &count_error, now, app_data_dir);
                    Ok(Some(PURGE_FAILED_WITHOUT_COUNT.to_owned()))
                }
            }
        }
    }
}

fn log_purge_error(
    connection: &Connection,
    error: &AppError,
    now: DateTime<Utc>,
    app_data_dir: &Path,
) {
    log_error(
        connection,
        "products",
        "purge_expired_products",
        error,
        now,
        app_data_dir,
    );
}

#[cfg(test)]
mod tests {
    use std::fs;
    use std::path::PathBuf;
    use std::sync::atomic::{AtomicU64, Ordering};

    use chrono::{Duration, Local, NaiveDate, TimeZone, Utc};
    use rusqlite::Connection;

    use crate::models::product::{
        CreateProductInput, ExpiryFilter, PreviewSellingPriceInput, ProductConditionFilter,
        ProductFilters, StockFilter, UpdateProductInput,
    };
    use crate::test_utils::{
        assert_allows_both_roles, assert_requires_session, create_fail_product_delete_trigger,
        create_test_session, create_test_user, insert_category_fixture, insert_product_fixture,
        insert_unit_fixture, setup_test_db,
    };
    use crate::timestamps::format_utc_timestamp;

    use super::{
        activate_product_inner, create_product_inner, delete_product_inner,
        discontinue_product_inner, expired_products_cutoff, get_product_associated_invoices_inner,
        get_product_inner, list_out_of_stock_and_discontinued_inner, list_products_inner,
        preview_selling_price_inner, purge_best_effort, purge_best_effort_with_app_data_dir,
        purge_expired_products, search_products_for_invoice_inner, update_product_inner,
    };

    const TS: &str = "2026-10-03T00:00:00Z";

    static NEXT_TEST_PATH: AtomicU64 = AtomicU64::new(0);
    type StoredUpdatedProduct = (
        String,
        Option<String>,
        i64,
        i64,
        i64,
        Option<i64>,
        Option<i64>,
        Option<String>,
    );

    fn unique_log_directory() -> PathBuf {
        let sequence = NEXT_TEST_PATH.fetch_add(1, Ordering::Relaxed);
        std::env::temp_dir().join(format!(
            "goyma-purge-test-{}-{sequence}",
            std::process::id()
        ))
    }

    fn insert_discontinued_product(
        connection: &Connection,
        name: &str,
        discontinued_at: String,
    ) -> i64 {
        insert_product_fixture(
            connection,
            name,
            None,
            None,
            100,
            0,
            1,
            "discontinued",
            Some(&discontinued_at),
            None,
        )
    }

    fn product_input(
        name: &str,
        cost: &str,
        quantity: &str,
        profit_percent: &str,
    ) -> CreateProductInput {
        CreateProductInput {
            name: name.to_owned(),
            description: None,
            category_id: None,
            unit_id: None,
            cost: cost.to_owned(),
            quantity: quantity.to_owned(),
            profit_percent: profit_percent.to_owned(),
            expires_on: None,
        }
    }

    fn preview_input(
        profit_percent: &str,
        product_id: Option<i64>,
        cost: Option<&str>,
    ) -> PreviewSellingPriceInput {
        PreviewSellingPriceInput {
            profit_percent: profit_percent.to_owned(),
            product_id,
            cost: cost.map(str::to_owned),
        }
    }

    fn authorize_test_user(connection: &Connection, now: chrono::DateTime<Utc>) {
        let user_id = create_test_user(
            connection,
            "productuser",
            "administrador",
            "+5350000100",
            "12345678910",
            "ValidPass1!",
        );
        create_test_session(connection, user_id, false, now, now);
    }

    #[test]
    fn list_products_filters_active_items_by_search_category_stock_and_expiry() {
        let mut connection = setup_test_db();
        let now = Utc.with_ymd_and_hms(2026, 10, 4, 12, 0, 0).unwrap();
        let today = NaiveDate::from_ymd_opt(2026, 10, 4).unwrap();
        authorize_test_user(&connection, now);
        let category_id = insert_category_fixture(&connection, "Alimentos", None);
        let other_category_id = insert_category_fixture(&connection, "Bebidas", None);
        let expired_id = insert_product_fixture(
            &connection,
            "Café",
            Some(category_id),
            None,
            1_000,
            0,
            1_000,
            "active",
            None,
            Some("2026-10-03"),
        );
        let today_id = insert_product_fixture(
            &connection,
            "Nuez",
            Some(category_id),
            None,
            1_000,
            0,
            5_000,
            "active",
            None,
            Some("2026-10-04"),
        );
        let soon_id = insert_product_fixture(
            &connection,
            "Ñame",
            Some(other_category_id),
            None,
            1_000,
            0,
            4_999,
            "active",
            None,
            Some("2026-11-02"),
        );
        let outside_soon_id = insert_product_fixture(
            &connection,
            "Oso",
            None,
            None,
            1_000,
            0,
            0,
            "active",
            None,
            Some("2026-11-03"),
        );
        let no_expiry_id = insert_product_fixture(
            &connection,
            "Sin caducidad",
            Some(other_category_id),
            None,
            1_000,
            0,
            4_000,
            "active",
            None,
            None,
        );
        insert_product_fixture(
            &connection,
            "Retirado",
            None,
            None,
            1_000,
            0,
            0,
            "discontinued",
            Some("2026-10-03T00:00:00Z"),
            None,
        );

        let all =
            list_products_inner(&mut connection, now, today, ProductFilters::default()).unwrap();
        assert_eq!(
            all.iter().map(|item| item.id).collect::<Vec<_>>(),
            vec![expired_id, today_id, soon_id, outside_soon_id, no_expiry_id]
        );
        assert_eq!(
            all.iter().find(|item| item.id == expired_id).unwrap().name,
            "Café"
        );
        assert!(
            all.iter()
                .find(|item| item.id == outside_soon_id)
                .unwrap()
                .is_out_of_stock
        );
        assert!(
            all.iter()
                .find(|item| item.id == soon_id)
                .unwrap()
                .is_low_stock
        );
        assert!(
            !all.iter()
                .find(|item| item.id == today_id)
                .unwrap()
                .is_low_stock
        );

        let search = ProductFilters {
            search: Some("CAFE".to_owned()),
            ..ProductFilters::default()
        };
        assert_eq!(
            list_products_inner(&mut connection, now, today, search)
                .unwrap()
                .iter()
                .map(|item| item.id)
                .collect::<Vec<_>>(),
            vec![expired_id]
        );
        let category_name_search = ProductFilters {
            search: Some("Alimentos".to_owned()),
            ..ProductFilters::default()
        };
        assert!(
            list_products_inner(&mut connection, now, today, category_name_search)
                .unwrap()
                .is_empty()
        );

        for (stock, expected) in [
            (
                StockFilter::Available,
                vec![expired_id, today_id, soon_id, no_expiry_id],
            ),
            (StockFilter::OutOfStock, vec![outside_soon_id]),
            (StockFilter::Low, vec![expired_id, soon_id, no_expiry_id]),
        ] {
            let filters = ProductFilters {
                stock,
                ..ProductFilters::default()
            };
            assert_eq!(
                list_products_inner(&mut connection, now, today, filters)
                    .unwrap()
                    .iter()
                    .map(|item| item.id)
                    .collect::<Vec<_>>(),
                expected
            );
        }

        for (expiry, expected) in [
            (
                ExpiryFilter::WithExpiry,
                vec![expired_id, today_id, soon_id, outside_soon_id],
            ),
            (ExpiryFilter::WithoutExpiry, vec![no_expiry_id]),
            (ExpiryFilter::Expired, vec![expired_id]),
            (ExpiryFilter::ExpiringSoon, vec![today_id, soon_id]),
        ] {
            let filters = ProductFilters {
                expiry,
                ..ProductFilters::default()
            };
            assert_eq!(
                list_products_inner(&mut connection, now, today, filters)
                    .unwrap()
                    .iter()
                    .map(|item| item.id)
                    .collect::<Vec<_>>(),
                expected
            );
        }

        let combined = ProductFilters {
            search: Some("CAFE".to_owned()),
            category_id: Some(category_id),
            stock: StockFilter::Low,
            expiry: ExpiryFilter::Expired,
            ..ProductFilters::default()
        };
        assert_eq!(
            list_products_inner(&mut connection, now, today, combined)
                .unwrap()
                .iter()
                .map(|item| item.id)
                .collect::<Vec<_>>(),
            vec![expired_id]
        );

        let category_filter = ProductFilters {
            category_id: Some(category_id),
            ..ProductFilters::default()
        };
        assert_eq!(
            list_products_inner(&mut connection, now, today, category_filter)
                .unwrap()
                .iter()
                .map(|item| item.id)
                .collect::<Vec<_>>(),
            vec![expired_id, today_id]
        );
        let without_category = ProductFilters {
            without_category: true,
            ..ProductFilters::default()
        };
        assert_eq!(
            list_products_inner(&mut connection, now, today, without_category)
                .unwrap()
                .iter()
                .map(|item| item.id)
                .collect::<Vec<_>>(),
            vec![outside_soon_id]
        );
    }

    #[test]
    fn list_products_rejects_conflicting_category_filters_and_covers_creation_and_authorization() {
        assert_requires_session(|connection, now| {
            list_products_inner(
                connection,
                now,
                NaiveDate::from_ymd_opt(2026, 10, 4).unwrap(),
                ProductFilters::default(),
            )
        });
        assert_allows_both_roles(|connection, now| {
            list_products_inner(
                connection,
                now,
                NaiveDate::from_ymd_opt(2026, 10, 4).unwrap(),
                ProductFilters::default(),
            )
            .map(|_| ())
        });

        let mut connection = setup_test_db();
        let now = Utc.with_ymd_and_hms(2026, 10, 4, 12, 0, 0).unwrap();
        let today = NaiveDate::from_ymd_opt(2026, 10, 4).unwrap();
        authorize_test_user(&connection, now);
        let invalid_filters = ProductFilters {
            category_id: Some(1),
            without_category: true,
            ..ProductFilters::default()
        };
        let error = list_products_inner(&mut connection, now, today, invalid_filters).unwrap_err();
        assert_eq!(error.code, "validationFailed");
        assert_eq!(error.message, "Los datos enviados no son válidos");

        let created = create_product_inner(
            &mut connection,
            now,
            today,
            product_input("Creado desde alta", "1", "1", "0"),
        )
        .unwrap();
        let available = list_products_inner(
            &mut connection,
            now,
            today,
            ProductFilters {
                stock: StockFilter::Available,
                ..ProductFilters::default()
            },
        )
        .unwrap();
        assert_eq!(
            available.iter().map(|item| item.id).collect::<Vec<_>>(),
            vec![created.id]
        );
        assert!(list_products_inner(
            &mut connection,
            now,
            today,
            ProductFilters {
                stock: StockFilter::OutOfStock,
                ..ProductFilters::default()
            },
        )
        .unwrap()
        .is_empty());
        assert_eq!(
            list_products_inner(
                &mut connection,
                now,
                today,
                ProductFilters {
                    stock: StockFilter::Low,
                    ..ProductFilters::default()
                },
            )
            .unwrap()
            .iter()
            .map(|item| item.id)
            .collect::<Vec<_>>(),
            vec![created.id]
        );
    }

    #[test]
    fn create_product_creates_initial_lot_and_history_entry() {
        let mut connection = setup_test_db();
        let now = Utc.with_ymd_and_hms(2026, 10, 4, 12, 0, 0).unwrap();
        let today = NaiveDate::from_ymd_opt(2026, 10, 4).unwrap();
        authorize_test_user(&connection, now);

        let created = create_product_inner(
            &mut connection,
            now,
            today,
            product_input("Producto inicial", "1", "1", "1"),
        )
        .unwrap();

        let lot = connection
            .query_row(
                "SELECT origin, initialMilli, remainingMilli, unitCostCents FROM lots WHERE productId = ?1",
                [created.id],
                |row| Ok((row.get::<_, String>(0)?, row.get::<_, i64>(1)?, row.get::<_, i64>(2)?, row.get::<_, i64>(3)?)),
            )
            .unwrap();
        assert_eq!(lot, ("initial".to_owned(), 1_000, 1_000, 100));

        let movement = connection
            .query_row(
                "SELECT type, quantityMilli, stockAfterMilli FROM productMovements WHERE productIdOriginal = ?1",
                [created.id],
                |row| Ok((row.get::<_, String>(0)?, row.get::<_, i64>(1)?, row.get::<_, i64>(2)?)),
            )
            .unwrap();
        assert_eq!(movement, ("initial".to_owned(), 1_000, 1_000));
        assert_eq!(created.valor_total_cents, 100);
    }

    #[test]
    fn condition_list_returns_unique_stock_and_discontinued_products_with_filters_and_days() {
        let mut connection = setup_test_db();
        let now = Utc.with_ymd_and_hms(2026, 10, 4, 12, 0, 0).unwrap();
        authorize_test_user(&connection, now);
        let category_id = insert_category_fixture(&connection, "Alimentos", None);
        let unit_id = insert_unit_fixture(&connection, "Kilogramo", "kg");
        let active_out_of_stock = insert_product_fixture(
            &connection,
            "Agotado activo",
            Some(category_id),
            Some(unit_id),
            1_000,
            0,
            0,
            "active",
            None,
            None,
        );
        let both_conditions = insert_product_fixture(
            &connection,
            "Ambos agotado",
            Some(category_id),
            Some(unit_id),
            1_000,
            0,
            0,
            "discontinued",
            Some(&format_utc_timestamp(
                now - Duration::days(59) - Duration::hours(23),
            )),
            None,
        );
        let discontinued_in_stock = insert_product_fixture(
            &connection,
            "Retirado",
            None,
            None,
            1_000,
            0,
            2_000,
            "discontinued",
            Some(&format_utc_timestamp(
                now - Duration::days(60) + Duration::seconds(1),
            )),
            None,
        );
        let future_discontinuation = insert_product_fixture(
            &connection,
            "Z futuro",
            None,
            None,
            1_000,
            0,
            3_000,
            "discontinued",
            Some(&format_utc_timestamp(now + Duration::days(1))),
            None,
        );
        insert_product_fixture(
            &connection,
            "Disponible activo",
            None,
            None,
            1_000,
            0,
            1_000,
            "active",
            None,
            None,
        );
        let expired = insert_discontinued_product(
            &connection,
            "Vencido",
            format_utc_timestamp(now - Duration::days(60)),
        );

        let all =
            list_out_of_stock_and_discontinued_inner(&mut connection, now, None, None).unwrap();
        assert_eq!(all.expired_cleanup_warning, None);
        assert_eq!(
            all.items.iter().map(|item| item.id).collect::<Vec<_>>(),
            vec![
                active_out_of_stock,
                both_conditions,
                discontinued_in_stock,
                future_discontinuation
            ]
        );
        let active_item = &all.items[0];
        assert!(active_item.is_out_of_stock);
        assert!(!active_item.is_discontinued);
        assert_eq!(active_item.remaining_days, None);
        let both_item = &all.items[1];
        assert!(both_item.is_out_of_stock);
        assert!(both_item.is_discontinued);
        assert_eq!(both_item.remaining_days, Some(1));
        assert_eq!(both_item.category_name.as_deref(), Some("Alimentos"));
        assert_eq!(both_item.unit_name.as_deref(), Some("Kilogramo"));
        assert_eq!(both_item.unit_abbreviation.as_deref(), Some("kg"));
        assert_eq!(all.items[2].remaining_days, Some(1));
        assert_eq!(all.items[3].remaining_days, Some(60));
        assert!(all.items.iter().all(|item| item.id != expired));
        assert!(!product_exists(&connection, expired));

        for (condition, expected) in [
            (
                Some(ProductConditionFilter::OutOfStock),
                vec![active_out_of_stock, both_conditions],
            ),
            (
                Some(ProductConditionFilter::Discontinued),
                vec![
                    both_conditions,
                    discontinued_in_stock,
                    future_discontinuation,
                ],
            ),
        ] {
            let result =
                list_out_of_stock_and_discontinued_inner(&mut connection, now, None, condition)
                    .unwrap();
            assert_eq!(
                result.items.iter().map(|item| item.id).collect::<Vec<_>>(),
                expected
            );
        }

        let search = list_out_of_stock_and_discontinued_inner(
            &mut connection,
            now,
            Some("AMBOS"),
            Some(ProductConditionFilter::All),
        )
        .unwrap();
        assert_eq!(
            search.items.iter().map(|item| item.id).collect::<Vec<_>>(),
            vec![both_conditions]
        );

        let created = create_product_inner(
            &mut connection,
            now,
            NaiveDate::from_ymd_opt(2026, 10, 4).unwrap(),
            product_input("Alta con cantidad inicial", "1", "2", "0"),
        )
        .unwrap();
        let out_of_stock = list_out_of_stock_and_discontinued_inner(
            &mut connection,
            now,
            None,
            Some(ProductConditionFilter::OutOfStock),
        )
        .unwrap();
        assert!(out_of_stock.items.iter().all(|item| item.id != created.id));
    }

    #[test]
    fn condition_list_excludes_expired_products_and_warns_when_purge_fails() {
        let mut connection = setup_test_db();
        let now = Utc.with_ymd_and_hms(2026, 10, 4, 12, 0, 0).unwrap();
        authorize_test_user(&connection, now);
        let visible_active = insert_product_fixture(
            &connection,
            "Agotado visible",
            None,
            None,
            1_000,
            0,
            0,
            "active",
            None,
            None,
        );
        let visible_discontinued = insert_discontinued_product(
            &connection,
            "Retirado visible",
            format_utc_timestamp(now - Duration::days(59)),
        );
        let expired = insert_discontinued_product(
            &connection,
            "Nombre confidencial vencido",
            format_utc_timestamp(now - Duration::days(60)),
        );
        create_fail_product_delete_trigger(&connection).unwrap();

        let result =
            list_out_of_stock_and_discontinued_inner(&mut connection, now, None, None).unwrap();

        assert_eq!(
            result.expired_cleanup_warning.as_deref(),
            Some("No se pudieron limpiar 1 productos vencidos; se reintentará en la próxima operación")
        );
        assert_eq!(
            result.items.iter().map(|item| item.id).collect::<Vec<_>>(),
            vec![visible_active, visible_discontinued]
        );
        assert!(product_exists(&connection, expired));
        let log_count: i64 = connection
            .query_row("SELECT COUNT(*) FROM errorLogs", [], |row| row.get(0))
            .unwrap();
        let log_cause: String = connection
            .query_row("SELECT cause FROM errorLogs LIMIT 1", [], |row| row.get(0))
            .unwrap();
        assert_eq!(log_count, 1);
        assert!(!log_cause.contains("Nombre confidencial vencido"));
    }

    #[test]
    fn condition_list_requires_session_and_allows_both_roles() {
        assert_requires_session(|connection, now| {
            list_out_of_stock_and_discontinued_inner(connection, now, None, None)
        });
        assert_allows_both_roles(|connection, now| {
            list_out_of_stock_and_discontinued_inner(connection, now, None, None).map(|_| ())
        });
    }

    #[test]
    fn get_product_returns_active_and_discontinued_details_with_nullable_references() {
        let mut connection = setup_test_db();
        let now = Utc.with_ymd_and_hms(2026, 10, 4, 12, 0, 0).unwrap();
        authorize_test_user(&connection, now);
        let category_id = insert_category_fixture(&connection, "Alimentos", None);
        let unit_id = insert_unit_fixture(&connection, "Kilogramo", "kg");
        let active_id = insert_product_fixture(
            &connection,
            "Café molido",
            Some(category_id),
            Some(unit_id),
            1_250,
            2_000,
            10_250,
            "active",
            None,
            Some("2026-10-20"),
        );
        connection
            .execute(
                "UPDATE products SET description = 'Tostado oscuro' WHERE id = ?1",
                [active_id],
            )
            .unwrap();

        let active = get_product_inner(&mut connection, now, active_id).unwrap();
        assert_eq!(active.product.id, active_id);
        assert_eq!(active.product.name, "Café molido");
        assert_eq!(active.product.category_id, Some(category_id));
        assert_eq!(active.product.category_name.as_deref(), Some("Alimentos"));
        assert_eq!(active.product.unit_id, Some(unit_id));
        assert_eq!(active.product.unit_name.as_deref(), Some("Kilogramo"));
        assert_eq!(active.product.unit_abbreviation.as_deref(), Some("kg"));
        assert_eq!(active.product.quantity, "10,25");
        assert_eq!(active.product.cost_cents, 1_250);
        assert_eq!(active.product.selling_price_cents, 1_500);
        assert_eq!(active.product.profit_percent, "20,00");
        assert_eq!(active.product.expires_on.as_deref(), Some("2026-10-20"));
        assert!(!active.product.is_out_of_stock);
        assert!(!active.product.is_low_stock);
        assert_eq!(active.description.as_deref(), Some("Tostado oscuro"));
        assert!(!active.is_discontinued);
        assert_eq!(active.discontinued_at, None);
        assert_eq!(active.remaining_days, None);
        assert_eq!(active.expired_cleanup_warning, None);

        let discontinued_at = format_utc_timestamp(now - Duration::days(59) - Duration::seconds(1));
        let discontinued_id =
            insert_discontinued_product(&connection, "Producto retirado", discontinued_at.clone());
        let discontinued = get_product_inner(&mut connection, now, discontinued_id).unwrap();
        assert_eq!(discontinued.product.category_id, None);
        assert_eq!(discontinued.product.category_name, None);
        assert_eq!(discontinued.product.unit_id, None);
        assert_eq!(discontinued.product.unit_name, None);
        assert_eq!(discontinued.product.unit_abbreviation, None);
        assert_eq!(discontinued.description, None);
        assert!(discontinued.is_discontinued);
        assert_eq!(
            discontinued.discontinued_at.as_deref(),
            Some(discontinued_at.as_str())
        );
        assert_eq!(discontinued.remaining_days, Some(1));
        assert!(!discontinued.product.is_out_of_stock);
        assert_eq!(discontinued.expired_cleanup_warning, None);
    }

    #[test]
    fn get_product_clamps_remaining_days_when_the_system_clock_moves_back() {
        let mut connection = setup_test_db();
        let now =
            Utc.with_ymd_and_hms(2026, 10, 4, 12, 0, 0).unwrap() + Duration::milliseconds(500);
        authorize_test_user(&connection, now);
        let future_discontinuation = format_utc_timestamp(now + Duration::days(1));
        let product_id =
            insert_discontinued_product(&connection, "Reloj atrasado", future_discontinuation);

        let detail = get_product_inner(&mut connection, now, product_id).unwrap();

        assert_eq!(detail.remaining_days, Some(60));

        let almost_expired_at =
            format_utc_timestamp(now - Duration::days(60) + Duration::seconds(1));
        let almost_expired =
            insert_discontinued_product(&connection, "A punto de vencer", almost_expired_at);
        let detail = get_product_inner(&mut connection, now, almost_expired).unwrap();

        assert_eq!(detail.remaining_days, Some(1));
    }

    #[test]
    fn get_product_purges_expired_product_before_returning_not_found() {
        let mut connection = setup_test_db();
        let now = Utc.with_ymd_and_hms(2026, 10, 4, 12, 0, 0).unwrap();
        authorize_test_user(&connection, now);
        let product_id = insert_discontinued_product(
            &connection,
            "Producto vencido",
            format_utc_timestamp(now - Duration::days(60)),
        );

        let error = get_product_inner(&mut connection, now, product_id).unwrap_err();

        assert_eq!(error.code, "notFound");
        assert_eq!(error.message, "El producto ya no existe");
        let exists: bool = connection
            .query_row(
                "SELECT EXISTS(SELECT 1 FROM products WHERE id = ?1)",
                [product_id],
                |row| row.get(0),
            )
            .unwrap();
        assert!(!exists);
    }

    #[test]
    fn get_product_rejects_missing_product_and_invalid_discontinuation_timestamp() {
        let mut connection = setup_test_db();
        let now = Utc.with_ymd_and_hms(2026, 10, 4, 12, 0, 0).unwrap();
        authorize_test_user(&connection, now);
        let missing = get_product_inner(&mut connection, now, 999).unwrap_err();
        assert_eq!(missing.code, "notFound");
        assert_eq!(missing.message, "El producto ya no existe");

        let product_id = insert_product_fixture(
            &connection,
            "Timestamp corrupto",
            None,
            None,
            100,
            0,
            1,
            "discontinued",
            Some("2026-10-03T00:00:00Z"),
            None,
        );
        connection
            .pragma_update(None, "ignore_check_constraints", "ON")
            .unwrap();
        connection
            .execute(
                "UPDATE products SET discontinuedAt = 'fecha inválida' WHERE id = ?1",
                [product_id],
            )
            .unwrap();

        let error = get_product_inner(&mut connection, now, product_id).unwrap_err();
        assert_eq!(error.code, "internal");
        assert_eq!(error.message, "Ocurrió un error inesperado");
    }

    #[test]
    fn get_product_requires_session_and_allows_both_roles() {
        assert_requires_session(|connection, now| get_product_inner(connection, now, 1));
        assert_allows_both_roles(|connection, now| {
            let product_id = insert_product_fixture(
                connection,
                "Producto de autorización",
                None,
                None,
                100,
                0,
                1,
                "active",
                None,
                None,
            );
            get_product_inner(connection, now, product_id)
        });
    }

    #[test]
    fn create_product_persists_initial_data_and_returns_derived_price() {
        let mut connection = setup_test_db();
        let now = Utc.with_ymd_and_hms(2026, 10, 4, 12, 0, 0).unwrap();
        let today = NaiveDate::from_ymd_opt(2026, 10, 4).unwrap();
        authorize_test_user(&connection, now);
        let category_id = insert_category_fixture(&connection, "Alimentos", None);
        let unit_id = insert_unit_fixture(&connection, "Kilogramo", "kg");
        let mut input = product_input("  Café   molido  ", "12,50", "10,250", "20");
        input.description = Some("  Tostado   oscuro  ".to_owned());
        input.category_id = Some(category_id);
        input.unit_id = Some(unit_id);
        input.expires_on = Some("2026-10-20".to_owned());

        let created = create_product_inner(&mut connection, now, today, input).unwrap();

        assert_eq!(created.name, "Café molido");
        assert_eq!(created.category_id, Some(category_id));
        assert_eq!(created.category_name.as_deref(), Some("Alimentos"));
        assert_eq!(created.unit_id, Some(unit_id));
        assert_eq!(created.unit_name.as_deref(), Some("Kilogramo"));
        assert_eq!(created.unit_abbreviation.as_deref(), Some("kg"));
        assert_eq!(created.quantity, "10,25");
        assert_eq!(created.cost_cents, 1_250);
        assert_eq!(created.selling_price_cents, 1_500);
        assert_eq!(created.profit_percent, "20,00");
        assert_eq!(created.expires_on.as_deref(), Some("2026-10-20"));
        assert!(!created.is_out_of_stock);
        assert!(!created.is_low_stock);

        let stored: (String, Option<String>, i64, i64, i64, i64, String, String) = connection
            .query_row(
                "SELECT name, description, costCents, profitPercentHundredths,
                        quantityMilli, status = 'active', createdAt, updatedAt
                 FROM products WHERE id = ?1",
                [created.id],
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
                    ))
                },
            )
            .unwrap();
        assert_eq!(stored.0, "Café molido");
        assert_eq!(stored.1.as_deref(), Some("Tostado oscuro"));
        assert_eq!(stored.2, 1_250);
        assert_eq!(stored.3, 2_000);
        assert_eq!(stored.4, 10_250);
        assert_eq!(stored.5, 1);
        assert_eq!(stored.6, format_utc_timestamp(now));
        assert_eq!(stored.7, format_utc_timestamp(now));

        connection
            .execute("DELETE FROM products WHERE id = ?1", [created.id])
            .unwrap();
        let recreated = create_product_inner(
            &mut connection,
            now,
            today,
            product_input("Otro producto", "1", "1", "0"),
        )
        .unwrap();
        assert!(recreated.id > created.id);
    }

    #[test]
    fn create_product_validates_text_and_numbers_in_form_order() {
        let mut connection = setup_test_db();
        let now = Utc.with_ymd_and_hms(2026, 10, 4, 12, 0, 0).unwrap();
        let today = NaiveDate::from_ymd_opt(2026, 10, 4).unwrap();
        authorize_test_user(&connection, now);

        let mut invalid_name = product_input("X", "bad", "bad", "bad");
        assert_eq!(
            create_product_inner(&mut connection, now, today, invalid_name.clone())
                .unwrap_err()
                .message,
            "El nombre debe tener al menos 2 caracteres"
        );
        invalid_name.name = "  ".to_owned();
        assert_eq!(
            create_product_inner(&mut connection, now, today, invalid_name.clone())
                .unwrap_err()
                .message,
            "El nombre es obligatorio"
        );
        invalid_name.name = "A".repeat(101);
        assert_eq!(
            create_product_inner(&mut connection, now, today, invalid_name)
                .unwrap_err()
                .message,
            "El nombre no puede tener más de 100 caracteres"
        );

        let cost_before_quantity = product_input("Producto válido", "bad", "bad", "bad");
        assert!(
            create_product_inner(&mut connection, now, today, cost_before_quantity)
                .unwrap_err()
                .message
                .starts_with("El precio de entrada no tiene un formato válido")
        );
        let quantity_before_percent = product_input("Producto válido", "1", "bad", "bad");
        assert!(
            create_product_inner(&mut connection, now, today, quantity_before_percent)
                .unwrap_err()
                .message
                .starts_with("La cantidad inicial no tiene un formato válido")
        );
        let mut percent_before_expiry = product_input("Producto válido", "1", "1", "bad");
        percent_before_expiry.expires_on = Some("2026-10-03".to_owned());
        assert!(
            create_product_inner(&mut connection, now, today, percent_before_expiry)
                .unwrap_err()
                .message
                .starts_with("El porcentaje de ganancia no tiene un formato válido")
        );

        let mut invalid_description = product_input("Producto válido", "1", "1", "0");
        invalid_description.description = Some("x".repeat(501));
        assert_eq!(
            create_product_inner(&mut connection, now, today, invalid_description)
                .unwrap_err()
                .message,
            "La descripción no puede tener más de 500 caracteres"
        );

        let mut empty_description = product_input("Producto válido", "1", "1", "0");
        empty_description.description = Some("   ".to_owned());
        let created = create_product_inner(&mut connection, now, today, empty_description).unwrap();
        let stored_description: Option<String> = connection
            .query_row(
                "SELECT description FROM products WHERE id = ?1",
                [created.id],
                |row| row.get(0),
            )
            .unwrap();
        assert_eq!(stored_description, None);

        for (raw, expected) in [
            (
                "0",
                "El precio de entrada debe ser mayor que 0 y no superar 999.999,99",
            ),
            (
                "-1",
                "El precio de entrada no tiene un formato válido. Escribe solo dígitos, con una coma o un punto para los decimales",
            ),
            (
                "1e3",
                "El precio de entrada no tiene un formato válido. Escribe solo dígitos, con una coma o un punto para los decimales",
            ),
            (
                "1.234",
                "El precio de entrada no usa separador de miles. Escribe los decimales con coma (por ejemplo, 1,234)",
            ),
            (
                "1,234",
                "El precio de entrada admite como máximo 2 decimales",
            ),
            (
                "1000000",
                "El precio de entrada debe ser mayor que 0 y no superar 999.999,99",
            ),
            (
                "100000000,000",
                "El precio de entrada admite como máximo 2 decimales",
            ),
        ] {
            let input = product_input("Producto válido", raw, "1", "0");
            assert_eq!(
                create_product_inner(&mut connection, now, today, input)
                    .unwrap_err()
                    .message,
                expected
            );
        }

        for (raw, expected) in [
            (
                "",
                "La cantidad inicial no tiene un formato válido. Escribe solo dígitos, con una coma o un punto para los decimales",
            ),
            (
                "0",
                "La cantidad inicial debe ser mayor que 0 y no superar 999.999,999",
            ),
            (
                "-1",
                "La cantidad inicial no tiene un formato válido. Escribe solo dígitos, con una coma o un punto para los decimales",
            ),
            (
                "abc",
                "La cantidad inicial no tiene un formato válido. Escribe solo dígitos, con una coma o un punto para los decimales",
            ),
            (
                "1,0001",
                "La cantidad inicial admite como máximo 3 decimales",
            ),
            (
                "1000000",
                "La cantidad inicial debe ser mayor que 0 y no superar 999.999,999",
            ),
        ] {
            let input = product_input("Producto válido", "1", raw, "0");
            assert_eq!(
                create_product_inner(&mut connection, now, today, input)
                    .unwrap_err()
                    .message,
                expected
            );
        }

        for (raw, expected) in [
            (
                "-1",
                "El porcentaje de ganancia no tiene un formato válido. Escribe solo dígitos, con una coma o un punto para los decimales",
            ),
            (
                "1,001",
                "El porcentaje de ganancia admite como máximo 2 decimales",
            ),
            (
                "1000,01",
                "El porcentaje de ganancia debe estar entre 0 y 1.000,00",
            ),
        ] {
            let input = product_input("Producto válido", "1", "1", raw);
            assert_eq!(
                create_product_inner(&mut connection, now, today, input)
                    .unwrap_err()
                    .message,
                expected
            );
        }
    }

    #[test]
    fn create_product_validates_expiry_before_references_and_checks_reference_existence() {
        let mut connection = setup_test_db();
        let now = Utc.with_ymd_and_hms(2026, 10, 4, 12, 0, 0).unwrap();
        let today = NaiveDate::from_ymd_opt(2026, 10, 4).unwrap();
        authorize_test_user(&connection, now);

        let mut input = product_input("Producto válido", "1", "1", "0");
        input.category_id = Some(101);
        input.unit_id = Some(202);
        input.expires_on = Some("2026-10-03".to_owned());
        assert_eq!(
            create_product_inner(&mut connection, now, today, input.clone())
                .unwrap_err()
                .message,
            "La fecha de caducidad no puede ser anterior a hoy"
        );

        for (expires_on, expected) in [
            ("2026-02-30", "La fecha de caducidad no es válida"),
            (
                "2101-01-01",
                "La fecha de caducidad no puede ser posterior al año 2100",
            ),
        ] {
            input.expires_on = Some(expires_on.to_owned());
            assert_eq!(
                create_product_inner(&mut connection, now, today, input.clone())
                    .unwrap_err()
                    .message,
                expected
            );
        }

        input.expires_on = None;
        assert_eq!(
            create_product_inner(&mut connection, now, today, input.clone())
                .unwrap_err()
                .message,
            "La categoría ya no existe"
        );
        input.category_id = None;
        assert_eq!(
            create_product_inner(&mut connection, now, today, input.clone())
                .unwrap_err()
                .message,
            "La unidad ya no existe"
        );

        input.unit_id = None;
        input.expires_on = Some("2026-10-04".to_owned());
        assert!(create_product_inner(&mut connection, now, today, input.clone()).is_ok());
        input.name = "Producto para 2100".to_owned();
        input.expires_on = Some("2100-12-31".to_owned());
        assert!(create_product_inner(&mut connection, now, today, input).is_ok());
    }

    #[test]
    fn create_product_duplicate_is_decided_by_active_name_index() {
        let mut connection = setup_test_db();
        let now = Utc.with_ymd_and_hms(2026, 10, 4, 12, 0, 0).unwrap();
        let today = NaiveDate::from_ymd_opt(2026, 10, 4).unwrap();
        authorize_test_user(&connection, now);
        insert_product_fixture(
            &connection,
            "Café",
            None,
            None,
            100,
            0,
            1,
            "active",
            None,
            None,
        );

        let duplicate = create_product_inner(
            &mut connection,
            now,
            today,
            product_input("CAFE", "1", "1", "0"),
        )
        .unwrap_err();
        assert_eq!(duplicate.code, "duplicateField");
        assert_eq!(duplicate.message, "Ya existe un producto con ese nombre");

        insert_product_fixture(
            &connection,
            "Producto retirado",
            None,
            None,
            100,
            0,
            1,
            "discontinued",
            Some("2026-10-03T00:00:00Z"),
            None,
        );
        let same_as_discontinued = create_product_inner(
            &mut connection,
            now,
            today,
            product_input("Producto retirado", "1", "1", "0"),
        );
        assert!(same_as_discontinued.is_ok());

        let distinct_enye = create_product_inner(
            &mut connection,
            now,
            today,
            product_input("Año", "1", "1", "0"),
        );
        assert!(distinct_enye.is_ok());
        assert!(create_product_inner(
            &mut connection,
            now,
            today,
            product_input("Ano", "1", "1", "0")
        )
        .is_ok());
    }

    #[test]
    fn preview_selling_price_uses_current_database_cost_without_writing() {
        let mut connection = setup_test_db();
        let now = Utc.with_ymd_and_hms(2026, 10, 4, 12, 0, 0).unwrap();
        authorize_test_user(&connection, now);
        let product_id = insert_product_fixture(
            &connection,
            "Producto vigente",
            None,
            None,
            1_000,
            0,
            2_500,
            "active",
            None,
            None,
        );
        connection
            .execute(
                "UPDATE products SET costCents = 2_500 WHERE id = ?1",
                [product_id],
            )
            .unwrap();
        let before: (i64, i64, String) = connection
            .query_row(
                "SELECT costCents, quantityMilli, updatedAt FROM products WHERE id = ?1",
                [product_id],
                |row| Ok((row.get(0)?, row.get(1)?, row.get(2)?)),
            )
            .unwrap();

        let quote = preview_selling_price_inner(
            &mut connection,
            now,
            preview_input("20", Some(product_id), None),
        )
        .unwrap();

        assert_eq!(quote.selling_price_cents, 3_000);
        let after: (i64, i64, String) = connection
            .query_row(
                "SELECT costCents, quantityMilli, updatedAt FROM products WHERE id = ?1",
                [product_id],
                |row| Ok((row.get(0)?, row.get(1)?, row.get(2)?)),
            )
            .unwrap();
        assert_eq!(after, before);
    }

    #[test]
    fn preview_selling_price_validates_source_and_returns_create_quote() {
        let mut connection = setup_test_db();
        let now = Utc.with_ymd_and_hms(2026, 10, 4, 12, 0, 0).unwrap();
        authorize_test_user(&connection, now);

        let quote = preview_selling_price_inner(
            &mut connection,
            now,
            preview_input("20", None, Some("12,50")),
        )
        .unwrap();
        assert_eq!(quote.selling_price_cents, 1_500);

        for input in [
            preview_input("20", None, None),
            preview_input("20", Some(9), Some("1")),
        ] {
            let error = preview_selling_price_inner(&mut connection, now, input).unwrap_err();
            assert_eq!(error.code, "validationFailed");
            assert_eq!(error.message, "Los datos enviados no son válidos");
        }
        assert_eq!(
            preview_selling_price_inner(
                &mut connection,
                now,
                preview_input("20", None, Some("0")),
            )
            .unwrap_err()
            .message,
            "El precio de entrada debe ser mayor que 0 y no superar 999.999,99"
        );
        assert_eq!(
            preview_selling_price_inner(
                &mut connection,
                now,
                preview_input("1000,01", None, Some("1")),
            )
            .unwrap_err()
            .message,
            "El porcentaje de ganancia debe estar entre 0 y 1.000,00"
        );
    }

    #[test]
    fn preview_selling_price_rejects_missing_expired_and_discontinued_products_without_purging() {
        let mut connection = setup_test_db();
        let now = Utc.with_ymd_and_hms(2026, 10, 4, 12, 0, 0).unwrap();
        authorize_test_user(&connection, now);
        let missing =
            preview_selling_price_inner(&mut connection, now, preview_input("20", Some(999), None))
                .unwrap_err();
        assert_eq!(missing.code, "notFound");
        assert_eq!(missing.message, "El producto ya no existe");

        let recent = insert_product_fixture(
            &connection,
            "Producto descontinuado",
            None,
            None,
            100,
            0,
            1,
            "discontinued",
            Some("2026-10-03T00:00:00Z"),
            None,
        );
        let state_error = preview_selling_price_inner(
            &mut connection,
            now,
            preview_input("20", Some(recent), None),
        )
        .unwrap_err();
        assert_eq!(state_error.code, "invalidState");
        assert_eq!(
            state_error.message,
            "Un producto descontinuado no se puede editar. Actívalo primero"
        );

        let expired_at = format_utc_timestamp(now - Duration::days(61));
        let expired = insert_discontinued_product(&connection, "Producto vencido", expired_at);
        let expired_error = preview_selling_price_inner(
            &mut connection,
            now,
            preview_input("20", Some(expired), None),
        )
        .unwrap_err();
        assert_eq!(expired_error.code, "notFound");
        assert_eq!(expired_error.message, "El producto ya no existe");
        let still_exists: bool = connection
            .query_row(
                "SELECT EXISTS(SELECT 1 FROM products WHERE id = ?1)",
                [expired],
                |row| row.get(0),
            )
            .unwrap();
        assert!(still_exists);
    }

    #[test]
    fn update_product_replaces_editable_fields_and_keeps_cost_and_quantity_in_sync() {
        let mut connection = setup_test_db();
        let now = Utc.with_ymd_and_hms(2026, 10, 4, 12, 0, 0).unwrap();
        authorize_test_user(&connection, now);
        let category_id = insert_category_fixture(&connection, "Alimentos", None);
        let unit_id = insert_unit_fixture(&connection, "Kilogramo", "kg");
        let today = Local::now().date_naive();
        let future_expiry = (today + Duration::days(11)).format("%Y-%m-%d").to_string();
        let product_id = insert_product_fixture(
            &connection,
            "Café original",
            Some(category_id),
            Some(unit_id),
            1_250,
            2_000,
            10_250,
            "active",
            None,
            Some(&future_expiry),
        );
        connection
            .execute(
                "UPDATE products SET description = 'Original' WHERE id = ?1",
                [product_id],
            )
            .unwrap();

        let updated = update_product_inner(
            &mut connection,
            now,
            UpdateProductInput {
                product_id,
                name: "  Café   actualizado  ".to_owned(),
                description: Some("  Nuevo texto  ".to_owned()),
                category_id: Some(category_id),
                unit_id: Some(unit_id),
                profit_percent: "30,00".to_owned(),
                expires_on: Some(future_expiry.clone()),
            },
        )
        .unwrap();

        assert_eq!(updated.product.name, "Café actualizado");
        assert_eq!(updated.product.category_id, Some(category_id));
        assert_eq!(updated.product.unit_id, Some(unit_id));
        assert_eq!(updated.product.cost_cents, 1_250);
        assert_eq!(updated.product.quantity, "10,25");
        assert_eq!(updated.product.selling_price_cents, 1_625);
        assert_eq!(updated.product.profit_percent, "30,00");
        assert_eq!(
            updated.product.expires_on.as_deref(),
            Some(future_expiry.as_str())
        );
        assert_eq!(updated.expired_cleanup_warning, None);

        let stored: StoredUpdatedProduct = connection
            .query_row(
                "SELECT name, description, costCents, profitPercentHundredths, quantityMilli, categoryId, unitId, expiresOn FROM products WHERE id = ?1",
                [product_id],
                |row| Ok((row.get(0)?, row.get(1)?, row.get(2)?, row.get(3)?, row.get(4)?, row.get(5)?, row.get(6)?, row.get(7)?)),
            )
            .unwrap();
        assert_eq!(stored.0, "Café actualizado");
        assert_eq!(stored.1.as_deref(), Some("Nuevo texto"));
        assert_eq!(stored.2, 1_250);
        assert_eq!(stored.3, 3_000);
        assert_eq!(stored.4, 10_250);
        assert_eq!(stored.5, Some(category_id));
        assert_eq!(stored.6, Some(unit_id));
        assert_eq!(stored.7.as_deref(), Some(future_expiry.as_str()));

        let cleared = update_product_inner(
            &mut connection,
            now,
            UpdateProductInput {
                product_id,
                name: "Café actualizado".to_owned(),
                description: None,
                category_id: None,
                unit_id: None,
                profit_percent: "30,00".to_owned(),
                expires_on: None,
            },
        )
        .unwrap();
        assert_eq!(cleared.product.category_id, None);
        assert_eq!(cleared.product.category_name, None);
        assert_eq!(cleared.product.unit_id, None);
        assert_eq!(cleared.product.unit_name, None);
        assert_eq!(cleared.product.expires_on, None);
        let cleared_values: (Option<String>, Option<i64>, Option<i64>, Option<String>) = connection
            .query_row(
                "SELECT description, categoryId, unitId, expiresOn FROM products WHERE id = ?1",
                [product_id],
                |row| Ok((row.get(0)?, row.get(1)?, row.get(2)?, row.get(3)?)),
            )
            .unwrap();
        assert_eq!(cleared_values, (None, None, None, None));

        let unchanged = update_product_inner(
            &mut connection,
            now,
            UpdateProductInput {
                product_id,
                name: "Café actualizado".to_owned(),
                description: None,
                category_id: None,
                unit_id: None,
                profit_percent: "30,00".to_owned(),
                expires_on: None,
            },
        )
        .unwrap();
        assert_eq!(unchanged.product.name, "Café actualizado");
    }

    #[test]
    fn update_product_validates_state_duplicate_missing_reference_and_expiry_rules() {
        let mut connection = setup_test_db();
        let now = Utc.with_ymd_and_hms(2026, 10, 4, 12, 0, 0).unwrap();
        authorize_test_user(&connection, now);

        let missing = update_product_inner(
            &mut connection,
            now,
            UpdateProductInput {
                product_id: 999,
                name: "Producto inexistente".to_owned(),
                description: None,
                category_id: None,
                unit_id: None,
                profit_percent: "10,00".to_owned(),
                expires_on: None,
            },
        )
        .unwrap_err();
        assert_eq!(missing.code, "notFound");
        assert_eq!(missing.message, "El producto ya no existe");

        let duplicated_name = insert_product_fixture(
            &connection,
            "Otro activo",
            None,
            None,
            100,
            0,
            1,
            "active",
            None,
            None,
        );
        let other_active = insert_product_fixture(
            &connection,
            "Producto rival",
            None,
            None,
            100,
            0,
            1,
            "active",
            None,
            None,
        );
        let duplicate = update_product_inner(
            &mut connection,
            now,
            UpdateProductInput {
                product_id: duplicated_name,
                name: "Producto rival".to_owned(),
                description: None,
                category_id: None,
                unit_id: None,
                profit_percent: "10,00".to_owned(),
                expires_on: None,
            },
        )
        .unwrap_err();
        assert_eq!(duplicate.code, "duplicateField");
        assert_eq!(duplicate.message, "Ya existe un producto con ese nombre");

        let _ = other_active;

        let discontinued_id = insert_discontinued_product(
            &connection,
            "Retirado",
            format_utc_timestamp(now - Duration::days(10)),
        );
        let state_error = update_product_inner(
            &mut connection,
            now,
            UpdateProductInput {
                product_id: discontinued_id,
                name: "Retirado".to_owned(),
                description: None,
                category_id: None,
                unit_id: None,
                profit_percent: "10,00".to_owned(),
                expires_on: None,
            },
        )
        .unwrap_err();
        assert_eq!(state_error.code, "invalidState");
        assert_eq!(
            state_error.message,
            "Un producto descontinuado no se puede editar. Actívalo primero"
        );

        let today = Local::now().date_naive();
        let expired_date = (today - Duration::days(1)).format("%Y-%m-%d").to_string();
        let newly_past_date = (today - Duration::days(2)).format("%Y-%m-%d").to_string();
        let expired_id = insert_product_fixture(
            &connection,
            "Con fecha vencida",
            None,
            None,
            100,
            0,
            1,
            "active",
            None,
            Some(&expired_date),
        );
        let allowed_expired = update_product_inner(
            &mut connection,
            now,
            UpdateProductInput {
                product_id: expired_id,
                name: "Con fecha vencida".to_owned(),
                description: None,
                category_id: None,
                unit_id: None,
                profit_percent: "10,00".to_owned(),
                expires_on: Some(expired_date.clone()),
            },
        );
        assert!(allowed_expired.is_ok());

        let invalid_expired = update_product_inner(
            &mut connection,
            now,
            UpdateProductInput {
                product_id: expired_id,
                name: "Con fecha vencida".to_owned(),
                description: None,
                category_id: None,
                unit_id: None,
                profit_percent: "10,00".to_owned(),
                expires_on: Some(newly_past_date),
            },
        )
        .unwrap_err();
        assert_eq!(
            invalid_expired.message,
            "La fecha de caducidad no puede ser anterior a hoy"
        );

        let missing_category = update_product_inner(
            &mut connection,
            now,
            UpdateProductInput {
                product_id: expired_id,
                name: "Con fecha vencida".to_owned(),
                description: None,
                category_id: Some(999),
                unit_id: None,
                profit_percent: "10,00".to_owned(),
                expires_on: None,
            },
        )
        .unwrap_err();
        assert_eq!(missing_category.message, "La categoría ya no existe");

        let missing_unit = update_product_inner(
            &mut connection,
            now,
            UpdateProductInput {
                product_id: expired_id,
                name: "Con fecha vencida".to_owned(),
                description: None,
                category_id: None,
                unit_id: Some(999),
                profit_percent: "10,00".to_owned(),
                expires_on: None,
            },
        )
        .unwrap_err();
        assert_eq!(missing_unit.message, "La unidad ya no existe");

        let expired_discontinued = insert_discontinued_product(
            &connection,
            "Producto vencido",
            format_utc_timestamp(now - Duration::days(61)),
        );
        let expired_purge = update_product_inner(
            &mut connection,
            now,
            UpdateProductInput {
                product_id: expired_discontinued,
                name: "Producto vencido".to_owned(),
                description: None,
                category_id: None,
                unit_id: None,
                profit_percent: "10,00".to_owned(),
                expires_on: None,
            },
        )
        .unwrap_err();
        assert_eq!(expired_purge.code, "notFound");
        assert_eq!(expired_purge.message, "El producto ya no existe");
        assert!(!product_exists(&connection, expired_discontinued));
    }

    #[test]
    fn update_product_requires_session_and_allows_both_roles() {
        assert_requires_session(|connection, now| {
            update_product_inner(
                connection,
                now,
                UpdateProductInput {
                    product_id: 1,
                    name: "Producto".to_owned(),
                    description: None,
                    category_id: None,
                    unit_id: None,
                    profit_percent: "10,00".to_owned(),
                    expires_on: None,
                },
            )
        });
        assert_allows_both_roles(|connection, now| {
            let existing = insert_product_fixture(
                connection,
                "Producto valido",
                None,
                None,
                100,
                0,
                1,
                "active",
                None,
                None,
            );
            update_product_inner(
                connection,
                now,
                UpdateProductInput {
                    product_id: existing,
                    name: "Producto valido".to_owned(),
                    description: None,
                    category_id: None,
                    unit_id: None,
                    profit_percent: "10,00".to_owned(),
                    expires_on: None,
                },
            )
        });
    }

    #[test]
    fn discontinue_and_activate_products_follow_state_machine_and_purge_rules() {
        let mut connection = setup_test_db();
        let now = Utc.with_ymd_and_hms(2026, 10, 4, 12, 0, 0).unwrap();
        authorize_test_user(&connection, now);

        let product_id = insert_product_fixture(
            &connection,
            "Botella de vidrio",
            None,
            None,
            1_000,
            0,
            25_000,
            "active",
            None,
            None,
        );
        let discontinued = discontinue_product_inner(&mut connection, now, product_id).unwrap();
        assert_eq!(discontinued.expired_cleanup_warning, None);
        let state: (String, Option<String>, i64, String) = connection
            .query_row(
                "SELECT status, discontinuedAt, quantityMilli, updatedAt FROM products WHERE id = ?1",
                [product_id],
                |row| Ok((row.get(0)?, row.get(1)?, row.get(2)?, row.get(3)?)),
            )
            .unwrap();
        assert_eq!(state.0, "discontinued");
        assert_eq!(state.1.as_deref(), Some(format_utc_timestamp(now).as_str()));
        assert_eq!(state.2, 25_000);
        assert_eq!(state.3, format_utc_timestamp(now));

        let already_discontinued =
            discontinue_product_inner(&mut connection, now, product_id).unwrap_err();
        assert_eq!(already_discontinued.code, "invalidState");
        assert_eq!(
            already_discontinued.message,
            "El producto ya está descontinuado"
        );

        let active_id = insert_product_fixture(
            &connection,
            "Botella de vidrio",
            None,
            None,
            1_000,
            0,
            25_000,
            "active",
            None,
            None,
        );
        let duplicate_on_activate =
            activate_product_inner(&mut connection, now, product_id).unwrap_err();
        assert_eq!(duplicate_on_activate.code, "duplicateField");
        assert_eq!(
            duplicate_on_activate.message,
            "No se puede activar: ya existe otro producto activo con ese nombre"
        );

        connection
            .execute("DELETE FROM products WHERE id = ?1", [active_id])
            .unwrap();

        let active_after_reactivate =
            activate_product_inner(&mut connection, now, product_id).unwrap();
        assert_eq!(active_after_reactivate.expired_cleanup_warning, None);
        let reactivated: (String, Option<String>, i64, String) = connection
            .query_row(
                "SELECT status, discontinuedAt, quantityMilli, updatedAt FROM products WHERE id = ?1",
                [product_id],
                |row| Ok((row.get(0)?, row.get(1)?, row.get(2)?, row.get(3)?)),
            )
            .unwrap();
        assert_eq!(reactivated.0, "active");
        assert_eq!(reactivated.1, None);
        assert_eq!(reactivated.2, 25_000);
        assert_eq!(reactivated.3, format_utc_timestamp(now));

        let already_active = activate_product_inner(&mut connection, now, product_id).unwrap_err();
        assert_eq!(already_active.code, "invalidState");
        assert_eq!(already_active.message, "El producto ya está activo");

        let missing = discontinue_product_inner(&mut connection, now, 999).unwrap_err();
        assert_eq!(missing.code, "notFound");
        assert_eq!(missing.message, "El producto ya no existe");
        let missing_activation = activate_product_inner(&mut connection, now, 999).unwrap_err();
        assert_eq!(missing_activation.code, "notFound");
        assert_eq!(missing_activation.message, "El producto ya no existe");

        let expired_id = insert_discontinued_product(
            &connection,
            "Producto vencido",
            format_utc_timestamp(now - Duration::days(61)),
        );
        let expired = discontinue_product_inner(&mut connection, now, expired_id).unwrap_err();
        assert_eq!(expired.code, "notFound");
        assert_eq!(expired.message, "El producto ya no existe");
        assert!(!product_exists(&connection, expired_id));

        let expired_activation_id = insert_discontinued_product(
            &connection,
            "Producto vencido para activar",
            format_utc_timestamp(now - Duration::days(61)),
        );
        let expired_activation =
            activate_product_inner(&mut connection, now, expired_activation_id).unwrap_err();
        assert_eq!(expired_activation.code, "notFound");
        assert_eq!(expired_activation.message, "El producto ya no existe");
        assert!(!product_exists(&connection, expired_activation_id));
    }

    #[test]
    fn delete_product_removes_rows_without_notifications_and_honors_purge_rules() {
        let mut connection = setup_test_db();
        let now = Utc.with_ymd_and_hms(2026, 10, 4, 12, 0, 0).unwrap();
        authorize_test_user(&connection, now);

        let category_id = insert_category_fixture(&connection, "Lácteos", None);
        let unit_id = insert_unit_fixture(&connection, "Litro", "L");

        let active_id = insert_product_fixture(
            &connection,
            "Leche entera",
            Some(category_id),
            Some(unit_id),
            8_000,
            15_000,
            25_000,
            "active",
            None,
            None,
        );
        let deleted_active = delete_product_inner(&mut connection, now, active_id).unwrap();
        assert_eq!(deleted_active.expired_cleanup_warning, None);
        assert!(!product_exists(&connection, active_id));
        assert!(connection
            .query_row(
                "SELECT EXISTS(SELECT 1 FROM categories WHERE id = ?1)",
                [category_id],
                |row| row.get::<_, bool>(0),
            )
            .unwrap());
        assert!(connection
            .query_row(
                "SELECT EXISTS(SELECT 1 FROM units WHERE id = ?1)",
                [unit_id],
                |row| row.get::<_, bool>(0),
            )
            .unwrap());

        let discontinued_id = insert_discontinued_product(
            &connection,
            "Queso artesanal",
            format_utc_timestamp(now - Duration::days(10)),
        );
        let deleted_discontinued =
            delete_product_inner(&mut connection, now, discontinued_id).unwrap();
        assert_eq!(deleted_discontinued.expired_cleanup_warning, None);
        assert!(!product_exists(&connection, discontinued_id));

        let missing = delete_product_inner(&mut connection, now, 999).unwrap_err();
        assert_eq!(missing.code, "notFound");
        assert_eq!(missing.message, "El producto ya no existe");

        let expired_id = insert_discontinued_product(
            &connection,
            "Producto vencido",
            format_utc_timestamp(now - Duration::days(61)),
        );
        let expired = delete_product_inner(&mut connection, now, expired_id).unwrap_err();
        assert_eq!(expired.code, "notFound");
        assert_eq!(expired.message, "El producto ya no existe");
        assert!(!product_exists(&connection, expired_id));

        // Borrar a mano no avisa; la purga automática sí (N-12, spec-04).
        let notifications: Vec<(String, String, String)> = connection
            .prepare("SELECT type, priority, message FROM notifications")
            .unwrap()
            .query_map([], |row| Ok((row.get(0)?, row.get(1)?, row.get(2)?)))
            .unwrap()
            .collect::<rusqlite::Result<_>>()
            .unwrap();
        assert_eq!(
            notifications,
            vec![(
                "N-12".to_owned(),
                "INFORMATIVO".to_owned(),
                "Se eliminaron automáticamente estos productos descontinuados: Producto vencido"
                    .to_owned(),
            )]
        );
    }

    #[test]
    fn delete_product_requires_session_and_allows_both_roles() {
        assert_requires_session(|connection, now| delete_product_inner(connection, now, 1));
        assert_allows_both_roles(|connection, now| {
            let product_id = insert_product_fixture(
                connection,
                "Producto borrable",
                None,
                None,
                100,
                0,
                1,
                "active",
                None,
                None,
            );
            delete_product_inner(connection, now, product_id)
        });
    }

    #[test]
    fn discontinue_and_activate_require_session_and_allow_both_roles() {
        assert_requires_session(|connection, now| discontinue_product_inner(connection, now, 1));
        assert_allows_both_roles(|connection, now| {
            let product_id = insert_product_fixture(
                connection,
                "Producto de cierre",
                None,
                None,
                100,
                0,
                1,
                "active",
                None,
                None,
            );
            discontinue_product_inner(connection, now, product_id)
        });

        assert_requires_session(|connection, now| activate_product_inner(connection, now, 1));
        assert_allows_both_roles(|connection, now| {
            let product_id = insert_product_fixture(
                connection,
                "Producto activo",
                None,
                None,
                100,
                0,
                1,
                "discontinued",
                Some(&format_utc_timestamp(now - Duration::days(1))),
                None,
            );
            activate_product_inner(connection, now, product_id)
        });
    }

    #[test]
    fn create_product_and_preview_require_session_and_allow_both_roles() {
        let today = NaiveDate::from_ymd_opt(2026, 10, 4).unwrap();
        assert_requires_session(|connection, now| {
            create_product_inner(
                connection,
                now,
                today,
                product_input("Producto", "1", "1", "0"),
            )
        });
        assert_allows_both_roles(|connection, now| {
            create_product_inner(
                connection,
                now,
                today,
                product_input("Producto", "1", "1", "0"),
            )
        });
        assert_requires_session(|connection, now| {
            preview_selling_price_inner(connection, now, preview_input("20", None, Some("1")))
        });
        assert_allows_both_roles(|connection, now| {
            preview_selling_price_inner(connection, now, preview_input("20", None, Some("1")))
        });
    }

    #[test]
    fn search_products_for_invoice_requires_session_and_purges_before_search() {
        let mut connection = setup_test_db();
        let now = Utc.with_ymd_and_hms(2026, 10, 4, 12, 0, 0).unwrap();
        let category_id = insert_category_fixture(&connection, "Cables", None);
        let active_id = insert_product_fixture(
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
        let expired_id = insert_discontinued_product(
            &connection,
            "Cable vencido",
            format_utc_timestamp(now - Duration::days(61)),
        );
        let out_of_stock_id = insert_product_fixture(
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
        connection
            .execute(
                "UPDATE products SET description = 'Cable de red' WHERE id = ?1",
                [active_id],
            )
            .unwrap();

        assert_requires_session(|connection, now| {
            search_products_for_invoice_inner(
                connection,
                now,
                "cable".to_owned(),
                Some("loss".to_owned()),
                Some(20),
            )
        });
        assert_allows_both_roles(|connection, now| {
            search_products_for_invoice_inner(
                connection,
                now,
                "cable".to_owned(),
                Some("loss".to_owned()),
                Some(20),
            )
        });

        authorize_test_user(&connection, now);
        let results = search_products_for_invoice_inner(
            &mut connection,
            now,
            "cable".to_owned(),
            None,
            Some(20),
        )
        .unwrap();
        // Por defecto (salidas) el agotado se devuelve marcado; el vencido se purgó.
        let ids = results.iter().map(|item| item.id).collect::<Vec<_>>();
        assert_eq!(ids, vec![out_of_stock_id, active_id]);
        assert!(results[0].is_out_of_stock);
        assert!(!product_exists(&connection, expired_id));
        assert!(product_exists(&connection, out_of_stock_id));

        let invalid = search_products_for_invoice_inner(
            &mut connection,
            now,
            "cable".to_owned(),
            Some("otro".to_owned()),
            Some(20),
        )
        .unwrap_err();
        assert_eq!(invalid.code, "validationFailed");
    }

    #[test]
    fn get_product_associated_invoices_requires_session_and_uses_product_id_original() {
        let connection = setup_test_db();
        let now = Utc.with_ymd_and_hms(2026, 10, 4, 12, 0, 0).unwrap();
        authorize_test_user(&connection, now);

        let product_id = insert_product_fixture(
            &connection,
            "Producto asociado",
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
             VALUES ('Ana', 'Pérez', '12345678901', '+5355551234', ?1, ?1)",
                [TS],
            )
            .unwrap();
        let client_id = connection.last_insert_rowid();
        connection.execute(
            "INSERT INTO invoices (type, number, invoiceDate, status, clientId, contactFullName, contactFullNameNormalized,
                contactNationalId, contactPhone, createdByUserId, createdByName, createdAt, updatedAt)
             VALUES ('V', 'V-00001', '2026-10-01', 'pending', ?1, 'Ana Pérez', 'ana perez', '12345678901', '+5355551234', ?2, 'admin', ?3, ?3)",
            (client_id, 1_i64, TS),
        ).unwrap();
        let older_invoice_id = connection.last_insert_rowid();
        connection.execute(
            "INSERT INTO invoices (type, number, invoiceDate, status, clientId, contactFullName, contactFullNameNormalized,
                contactNationalId, contactPhone, createdByUserId, createdByName, createdAt, updatedAt)
             VALUES ('S', 'S-00001', '2026-10-03', 'pending', ?1, 'Ana Pérez', 'ana perez', '12345678901', '+5355551234', ?2, 'admin', ?3, ?3)",
            (client_id, 1_i64, TS),
        ).unwrap();
        let newer_invoice_id = connection.last_insert_rowid();
        connection.execute(
            "INSERT INTO invoiceLines (invoiceId, position, productId, productIdOriginal, productName, productNameNormalized, quantityMilli, unitPriceCents, subtotalCents, createdAt, updatedAt)
             VALUES (?1, 1, ?2, ?2, 'Producto asociado', 'producto asociado', 2_000, 250, 500, ?3, ?3)",
            (older_invoice_id, product_id, TS)
        ).unwrap();
        connection.execute(
            "INSERT INTO invoiceLines (invoiceId, position, productId, productIdOriginal, productName, productNameNormalized, quantityMilli, unitPriceCents, subtotalCents, createdAt, updatedAt)
             VALUES (?1, 1, ?2, ?2, 'Producto asociado', 'producto asociado', 3_000, 250, 750, ?3, ?3)",
            (newer_invoice_id, product_id, TS)
        ).unwrap();
        connection
            .execute("DELETE FROM products WHERE id = ?1", [product_id])
            .unwrap();

        assert_requires_session(|connection, now| {
            get_product_associated_invoices_inner(connection, now, product_id, Some(50))
        });
        assert_allows_both_roles(|connection, now| {
            get_product_associated_invoices_inner(connection, now, product_id, Some(50))
        });

        let results =
            get_product_associated_invoices_inner(&connection, now, product_id, Some(50)).unwrap();
        assert_eq!(results.len(), 2);
        assert_eq!(results[0].invoice_number, "S-00001");
        assert_eq!(results[0].quantity, "3");
        assert_eq!(results[1].invoice_number, "V-00001");
        assert_eq!(results[1].quantity, "2");
    }

    fn product_exists(connection: &Connection, product_id: i64) -> bool {
        connection
            .query_row(
                "SELECT EXISTS(SELECT 1 FROM products WHERE id = ?1)",
                [product_id],
                |row| row.get(0),
            )
            .unwrap()
    }

    #[test]
    fn purge_uses_exact_sixty_day_boundary_and_never_removes_active_products() {
        let connection = setup_test_db();
        let now = Utc.with_ymd_and_hms(2026, 10, 3, 12, 0, 0).unwrap();
        let before_sixty_days = insert_discontinued_product(
            &connection,
            "Producto reciente",
            format_utc_timestamp(now - Duration::days(59) - Duration::seconds(86_399)),
        );
        let exactly_sixty_days = insert_discontinued_product(
            &connection,
            "Producto en frontera",
            format_utc_timestamp(now - Duration::days(60)),
        );
        let beyond_sixty_days = insert_discontinued_product(
            &connection,
            "Producto vencido",
            format_utc_timestamp(now - Duration::days(60) - Duration::seconds(1)),
        );
        let active_product = insert_product_fixture(
            &connection,
            "Producto activo",
            None,
            None,
            100,
            0,
            1,
            "active",
            None,
            None,
        );

        assert_eq!(
            expired_products_cutoff(now),
            format_utc_timestamp(now - Duration::days(60))
        );
        assert_eq!(purge_expired_products(&connection, now).unwrap(), 2);
        assert!(product_exists(&connection, before_sixty_days));
        assert!(!product_exists(&connection, exactly_sixty_days));
        assert!(!product_exists(&connection, beyond_sixty_days));
        assert!(product_exists(&connection, active_product));
    }

    #[test]
    fn failed_purge_rolls_back_to_savepoint_logs_and_keeps_outer_transaction_usable() {
        let mut connection = setup_test_db();
        let now = Utc.with_ymd_and_hms(2026, 10, 3, 12, 0, 0).unwrap();
        let expired_product = insert_discontinued_product(
            &connection,
            "Nombre que no debe llegar al log",
            format_utc_timestamp(now - Duration::days(60)),
        );
        create_fail_product_delete_trigger(&connection).unwrap();

        let transaction = connection.transaction().unwrap();
        let warning = purge_best_effort(&transaction, now).unwrap();

        assert_eq!(
            warning.as_deref(),
            Some("No se pudieron limpiar 1 productos vencidos; se reintentará en la próxima operación")
        );
        assert!(product_exists(&transaction, expired_product));
        let log_count: i64 = transaction
            .query_row("SELECT COUNT(*) FROM errorLogs", [], |row| row.get(0))
            .unwrap();
        let cause: String = transaction
            .query_row("SELECT cause FROM errorLogs LIMIT 1", [], |row| row.get(0))
            .unwrap();
        assert_eq!(log_count, 1);
        assert!(!cause.contains("Nombre que no debe llegar al log"));

        transaction
            .execute(
                "INSERT INTO notifications (type, priority, message, createdAt) VALUES ('test', 'INFORMATIVO', 'usable', '2026-10-03T12:00:00Z')",
                [],
            )
            .unwrap();
        transaction.commit().unwrap();
    }

    #[test]
    fn no_expired_products_have_no_warning() {
        let connection = setup_test_db();
        let now = Utc.with_ymd_and_hms(2026, 10, 3, 12, 0, 0).unwrap();

        assert_eq!(purge_best_effort(&connection, now).unwrap(), None);
    }

    #[test]
    fn failed_purge_with_zero_expired_products_has_no_warning() {
        let connection = setup_test_db();
        let now = Utc.with_ymd_and_hms(2026, 10, 3, 12, 0, 0).unwrap();
        let app_data_dir = unique_log_directory();
        connection.pragma_update(None, "query_only", "ON").unwrap();

        assert_eq!(
            purge_best_effort_with_app_data_dir(&connection, now, &app_data_dir).unwrap(),
            None
        );
        assert!(app_data_dir.join("errors.log").exists());
        fs::remove_dir_all(app_data_dir).unwrap();
    }

    #[test]
    fn failed_expired_count_logs_both_errors_and_returns_warning_without_a_number() {
        let connection = setup_test_db();
        let now = Utc.with_ymd_and_hms(2026, 10, 3, 12, 0, 0).unwrap();
        connection.execute_batch("DROP TABLE products").unwrap();

        assert_eq!(
            purge_best_effort(&connection, now).unwrap().as_deref(),
            Some("No se pudieron limpiar los productos vencidos; se reintentará en la próxima operación")
        );
        let log_count: i64 = connection
            .query_row("SELECT COUNT(*) FROM errorLogs", [], |row| row.get(0))
            .unwrap();
        assert_eq!(log_count, 2);
    }

    #[test]
    fn successful_best_effort_purge_deletes_expired_products_without_warning() {
        let connection = setup_test_db();
        let now = Utc.with_ymd_and_hms(2026, 10, 3, 12, 0, 0).unwrap();
        let expired_product = insert_discontinued_product(
            &connection,
            "Producto que se limpia",
            format_utc_timestamp(now - Duration::days(60)),
        );

        assert_eq!(purge_best_effort(&connection, now).unwrap(), None);
        assert!(!product_exists(&connection, expired_product));
    }
}
