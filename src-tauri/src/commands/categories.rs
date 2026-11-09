use chrono::{DateTime, Utc};
use rusqlite::{params, Connection, OptionalExtension};

use crate::commands::auth::{require_session, Author};
use crate::commands::products::purge_best_effort;
use crate::db::connection::lock_conn;
use crate::db::notifications::{insert_affected_products_notification, AffectedProductsKind};
use crate::errors::{log_error, map_db_error, AppError};
use crate::models::category::{
    CategoryDetail, CategoryList, CategoryListItem, CategoryMatch, DeleteCategoryResult,
    ProductBrief,
};
use crate::text::{clean_name, normalize_text, sort_spanish, spanish_sort_key};
use crate::timestamps::format_utc_timestamp;
use crate::validators::{validate_name_with_messages, validate_optional_description};

const CATEGORY_DUPLICATE_MESSAGE: &str = "Ya existe una categoría con ese nombre";
const CATEGORY_REQUIRED_MESSAGE: &str = "El nombre es obligatorio";
const CATEGORY_TOO_LONG_MESSAGE: &str = "El nombre no puede tener más de 50 caracteres";
const CATEGORY_DESCRIPTION_TOO_LONG_MESSAGE: &str =
    "La descripción no puede tener más de 200 caracteres";
const CATEGORY_NAME_FORBIDDEN_MESSAGE: &str = "El nombre contiene caracteres no permitidos";
const CATEGORY_DESCRIPTION_FORBIDDEN_MESSAGE: &str =
    "La descripción contiene caracteres no permitidos";

fn category_income_cents(connection: &Connection, category_id: i64) -> Result<i64, AppError> {
    let total: i64 = connection
        .query_row(
            "SELECT COALESCE(SUM(il.subtotalCents), 0)
             FROM invoiceLines il
             INNER JOIN invoices i ON i.id = il.invoiceId
             WHERE il.categoryId = ?1
               AND i.type IN ('V', 'S')
               AND i.status IS NOT NULL
               AND i.status <> 'cancelled'",
            [category_id],
            |row| row.get(0),
        )
        .map_err(|error| map_db_error(error, "categoryIncome", &[]))?;
    Ok(total)
}

pub fn create_category_inner(
    connection: &mut Connection,
    now: DateTime<Utc>,
    name: &str,
    description: Option<&str>,
) -> Result<CategoryListItem, AppError> {
    require_session(connection, now)?;
    let (cleaned_name, normalized_name) = validate_name_with_messages(
        name,
        1,
        50,
        CATEGORY_REQUIRED_MESSAGE,
        CATEGORY_TOO_LONG_MESSAGE,
        CATEGORY_NAME_FORBIDDEN_MESSAGE,
    )?;
    let description = validate_optional_description(
        description,
        200,
        CATEGORY_DESCRIPTION_TOO_LONG_MESSAGE,
        CATEGORY_DESCRIPTION_FORBIDDEN_MESSAGE,
    )?;
    let timestamp = format_utc_timestamp(now);

    connection
        .execute(
            "INSERT INTO categories (name, nameNormalized, description, createdAt, updatedAt)
             VALUES (?1, ?2, ?3, ?4, ?4)",
            params![cleaned_name, normalized_name, description, timestamp],
        )
        .map_err(|error| {
            map_db_error(
                error,
                "createCategory",
                &[("categories.nameNormalized", CATEGORY_DUPLICATE_MESSAGE)],
            )
        })?;

    Ok(CategoryListItem {
        id: connection.last_insert_rowid(),
        name: cleaned_name,
        description,
        product_count: 0,
        ingresos_cents: 0,
    })
}

#[tauri::command]
pub fn create_category(
    state: tauri::State<'_, std::sync::Mutex<Connection>>,
    name: String,
    description: Option<String>,
) -> Result<CategoryListItem, AppError> {
    let mut connection = lock_conn(&state);
    let now = Utc::now();
    let result = create_category_inner(&mut connection, now, &name, description.as_deref());
    log_command_error(&connection, "createCategory", &result, now);
    result
}

pub fn update_category_inner(
    connection: &mut Connection,
    now: DateTime<Utc>,
    category_id: i64,
    name: &str,
    description: Option<&str>,
) -> Result<CategoryListItem, AppError> {
    require_session(connection, now)?;

    let exists: Option<i64> = connection
        .query_row(
            "SELECT id FROM categories WHERE id = ?1",
            [category_id],
            |row| row.get(0),
        )
        .optional()
        .map_err(|error| map_db_error(error, "updateCategory", &[]))?;
    if exists.is_none() {
        return Err(AppError::new(
            "notFound",
            "La categoría ya no existe",
            "category missing during update",
        ));
    }

    let (cleaned_name, normalized_name) = validate_name_with_messages(
        name,
        1,
        50,
        CATEGORY_REQUIRED_MESSAGE,
        CATEGORY_TOO_LONG_MESSAGE,
        CATEGORY_NAME_FORBIDDEN_MESSAGE,
    )?;
    let description = validate_optional_description(
        description,
        200,
        CATEGORY_DESCRIPTION_TOO_LONG_MESSAGE,
        CATEGORY_DESCRIPTION_FORBIDDEN_MESSAGE,
    )?;
    let timestamp = format_utc_timestamp(now);

    connection
        .execute(
            "UPDATE categories
             SET name = ?1, nameNormalized = ?2, description = ?3, updatedAt = ?4
             WHERE id = ?5",
            params![
                cleaned_name,
                normalized_name,
                description,
                timestamp,
                category_id
            ],
        )
        .map_err(|error| {
            map_db_error(
                error,
                "updateCategory",
                &[("categories.nameNormalized", CATEGORY_DUPLICATE_MESSAGE)],
            )
        })?;

    let product_count: i64 = connection
        .query_row(
            "SELECT COUNT(*) FROM products WHERE categoryId = ?1",
            [category_id],
            |row| row.get(0),
        )
        .map_err(|error| map_db_error(error, "updateCategory", &[]))?;

    Ok(CategoryListItem {
        id: category_id,
        name: cleaned_name,
        description,
        product_count,
        ingresos_cents: category_income_cents(connection, category_id)?,
    })
}

#[tauri::command]
pub fn update_category(
    state: tauri::State<'_, std::sync::Mutex<Connection>>,
    category_id: i64,
    name: String,
    description: Option<String>,
) -> Result<CategoryListItem, AppError> {
    let mut connection = lock_conn(&state);
    let now = Utc::now();
    let result = update_category_inner(
        &mut connection,
        now,
        category_id,
        &name,
        description.as_deref(),
    );
    log_command_error(&connection, "updateCategory", &result, now);
    result
}

pub fn list_categories_inner(
    connection: &mut Connection,
    now: DateTime<Utc>,
    search: Option<&str>,
) -> Result<CategoryList, AppError> {
    require_session(connection, now)?;

    let warning = match purge_best_effort(connection, now) {
        Ok(warning) => warning,
        Err(error) => {
            if let Some(app_data_dir) = crate::db::connection::current_app_data_dir() {
                log_error(
                    connection,
                    "categories",
                    "listCategories",
                    &error,
                    now,
                    app_data_dir,
                );
            }
            Some("No se pudieron limpiar las categorías vencidas; se reintentará en la próxima operación".to_owned())
        }
    };

    let cutoff = crate::commands::products::expired_products_cutoff(now);
    let normalized_search = normalize_text(&clean_name(search.unwrap_or("")));
    let mut statement = connection
        .prepare(
            "SELECT id, name, description,
                    (SELECT COUNT(*) FROM products WHERE categoryId = categories.id AND (status <> 'discontinued' OR discontinuedAt > ?2))
             FROM categories
             WHERE ?1 = '' OR instr(nameNormalized, ?1) > 0
             ORDER BY id ASC",
        )
        .map_err(|error| map_db_error(error, "listCategories", &[]))?;

    let rows = statement
        .query_map(params![&normalized_search, &cutoff], |row| {
            Ok(CategoryListItem {
                id: row.get(0)?,
                name: row.get(1)?,
                description: row.get(2)?,
                product_count: row.get(3)?,
                ingresos_cents: 0,
            })
        })
        .map_err(|error| map_db_error(error, "listCategories", &[]))?;

    let mut items = rows
        .map(|row| row.map_err(|error| map_db_error(error, "listCategories", &[])))
        .collect::<Result<Vec<_>, AppError>>()?;
    for item in &mut items {
        item.ingresos_cents = category_income_cents(connection, item.id)?;
    }
    sort_spanish(&mut items, |item| (spanish_sort_key(&item.name), item.id));

    Ok(CategoryList {
        items,
        expired_cleanup_warning: warning,
    })
}

#[tauri::command]
pub fn list_categories(
    state: tauri::State<'_, std::sync::Mutex<Connection>>,
    search: Option<String>,
) -> Result<CategoryList, AppError> {
    let mut connection = lock_conn(&state);
    let now = Utc::now();
    let result = list_categories_inner(&mut connection, now, search.as_deref());
    log_command_error(&connection, "listCategories", &result, now);
    result
}

pub fn get_category_inner(
    connection: &mut Connection,
    now: DateTime<Utc>,
    category_id: i64,
) -> Result<CategoryDetail, AppError> {
    require_session(connection, now)?;

    let warning = match purge_best_effort(connection, now) {
        Ok(warning) => warning,
        Err(error) => {
            if let Some(app_data_dir) = crate::db::connection::current_app_data_dir() {
                log_error(
                    connection,
                    "categories",
                    "getCategory",
                    &error,
                    now,
                    app_data_dir,
                );
            }
            Some("No se pudieron limpiar los productos vencidos; se reintentará en la próxima operación".to_owned())
        }
    };

    let category = connection
        .query_row(
            "SELECT id, name, description FROM categories WHERE id = ?1",
            [category_id],
            |row| {
                Ok((
                    row.get::<_, i64>(0)?,
                    row.get::<_, String>(1)?,
                    row.get::<_, Option<String>>(2)?,
                ))
            },
        )
        .optional()
        .map_err(|error| map_db_error(error, "getCategory", &[]))?;

    let (id, name, description) = category.ok_or_else(|| {
        AppError::new(
            "notFound",
            "La categoría ya no existe",
            "category missing during detail retrieval",
        )
    })?;

    let cutoff = crate::commands::products::expired_products_cutoff(now);
    let product_count: i64 = connection
        .query_row(
            "SELECT COUNT(*) FROM products WHERE categoryId = ?1 AND (status <> 'discontinued' OR discontinuedAt > ?2)",
            params![category_id, cutoff],
            |row| row.get(0),
        )
        .map_err(|error| map_db_error(error, "getCategory", &[]))?;

    let mut statement = connection
        .prepare(
            "SELECT id, name, quantityMilli - COALESCE((SELECT reservedMilli FROM productAvailability WHERE productId = products.id), 0), status
             FROM products
             WHERE categoryId = ?1 AND (status <> 'discontinued' OR discontinuedAt > ?2)
             ORDER BY id ASC",
        )
        .map_err(|error| map_db_error(error, "getCategory", &[]))?;

    let rows = statement
        .query_map(params![category_id, cutoff], |row| {
            let product_id: i64 = row.get(0)?;
            let product_name: String = row.get(1)?;
            let quantity_milli: i64 = row.get(2)?;
            let status: String = row.get(3)?;
            Ok(ProductBrief {
                id: product_id,
                name: product_name,
                is_out_of_stock: quantity_milli <= 0,
                is_discontinued: status == "discontinued",
            })
        })
        .map_err(|error| map_db_error(error, "getCategory", &[]))?;

    let mut products = rows
        .map(|row| row.map_err(|error| map_db_error(error, "getCategory", &[])))
        .collect::<Result<Vec<_>, AppError>>()?;
    sort_spanish(&mut products, |item| {
        (spanish_sort_key(&item.name), item.id)
    });

    Ok(CategoryDetail {
        id,
        name,
        description,
        product_count,
        ingresos_cents: category_income_cents(connection, category_id)?,
        products,
        expired_cleanup_warning: warning,
    })
}

#[tauri::command]
pub fn get_category(
    state: tauri::State<'_, std::sync::Mutex<Connection>>,
    category_id: i64,
) -> Result<CategoryDetail, AppError> {
    let mut connection = lock_conn(&state);
    let now = Utc::now();
    let result = get_category_inner(&mut connection, now, category_id);
    log_command_error(&connection, "getCategory", &result, now);
    result
}

pub fn search_categories_inner(
    connection: &mut Connection,
    now: DateTime<Utc>,
    text: &str,
) -> Result<Vec<CategoryMatch>, AppError> {
    require_session(connection, now)?;
    let normalized_text = normalize_text(&clean_name(text));
    let mut statement = connection
        .prepare(
            "SELECT id, name, nameNormalized
             FROM categories
             WHERE ?1 = '' OR instr(nameNormalized, ?1) > 0
             ORDER BY id ASC",
        )
        .map_err(|error| map_db_error(error, "searchCategories", &[]))?;
    let rows = statement
        .query_map([&normalized_text], |row| {
            let name_normalized: String = row.get(2)?;
            Ok(CategoryMatch {
                id: row.get(0)?,
                name: row.get(1)?,
                is_exact_match: name_normalized == normalized_text,
            })
        })
        .map_err(|error| map_db_error(error, "searchCategories", &[]))?;

    rows.map(|row| row.map_err(|error| map_db_error(error, "searchCategories", &[])))
        .collect()
}

pub fn delete_category_inner(
    connection: &mut Connection,
    now: DateTime<Utc>,
    category_id: i64,
) -> Result<DeleteCategoryResult, AppError> {
    let actor = require_session(connection, now)?;

    let tx = connection
        .transaction()
        .map_err(|error| map_db_error(error, "deleteCategory", &[]))?;

    let expired_cleanup_warning = match purge_best_effort(&tx, now) {
        Ok(warning) => warning,
        Err(error) => {
            if let Some(app_data_dir) = crate::db::connection::current_app_data_dir() {
                log_error(
                    &tx,
                    "categories",
                    "deleteCategory",
                    &error,
                    now,
                    app_data_dir,
                );
            }
            Some("No se pudieron limpiar los productos vencidos; se reintentará en la próxima operación".to_owned())
        }
    };

    let category_exists = tx
        .query_row(
            "SELECT id FROM categories WHERE id = ?1",
            [category_id],
            |row| row.get::<_, i64>(0),
        )
        .optional()
        .map_err(|error| map_db_error(error, "deleteCategory", &[]))?;
    if category_exists.is_none() {
        return Err(AppError::new(
            "notFound",
            "La categoría ya no existe",
            "category missing during deletion",
        ));
    }

    let cutoff = crate::commands::products::expired_products_cutoff(now);
    let affected_products: Vec<(i64, String)> = tx
        .prepare(
            "SELECT id, name FROM products WHERE categoryId = ?1 AND (status <> 'discontinued' OR discontinuedAt > ?2) ORDER BY id ASC",
        )
        .map_err(|error| map_db_error(error, "deleteCategory", &[]))?
        .query_map(params![category_id, cutoff], |row| Ok((row.get(0)?, row.get(1)?)))
        .map_err(|error| map_db_error(error, "deleteCategory", &[]))?
        .collect::<Result<Vec<_>, rusqlite::Error>>()
        .map_err(|error| map_db_error(error, "deleteCategory", &[]))?;

    if !affected_products.is_empty() {
        insert_affected_products_notification(
            &tx,
            AffectedProductsKind::Category,
            &affected_products,
            now,
        )
        .map_err(|error| map_db_error(error, "deleteCategory", &[]))?;

        let updated_at = format_utc_timestamp(now);
        tx.execute(
            "UPDATE products SET categoryId = NULL, updatedAt = ?1 WHERE categoryId = ?2 AND (status <> 'discontinued' OR discontinuedAt > ?3)",
            params![updated_at, category_id, cutoff],
        )
        .map_err(|error| map_db_error(error, "deleteCategory", &[]))?;
    }

    crate::activity::record_deletion(
        &tx,
        &Author::User(&actor),
        now,
        "category",
        "SELECT name FROM categories WHERE id = ?1",
        category_id,
    )
    .map_err(|error| map_db_error(error, "deleteCategory", &[]))?;

    tx.execute("DELETE FROM categories WHERE id = ?1", [category_id])
        .map_err(|error| map_db_error(error, "deleteCategory", &[]))?;

    tx.commit()
        .map_err(|error| map_db_error(error, "deleteCategory", &[]))?;

    Ok(DeleteCategoryResult {
        affected_products: affected_products.len(),
        expired_cleanup_warning,
    })
}

#[tauri::command]
pub fn delete_category(
    state: tauri::State<'_, std::sync::Mutex<Connection>>,
    category_id: i64,
) -> Result<DeleteCategoryResult, AppError> {
    let mut connection = lock_conn(&state);
    let now = Utc::now();
    let result = delete_category_inner(&mut connection, now, category_id);
    log_command_error(&connection, "deleteCategory", &result, now);
    result
}

#[tauri::command]
pub fn search_categories(
    state: tauri::State<'_, std::sync::Mutex<Connection>>,
    text: String,
) -> Result<Vec<CategoryMatch>, AppError> {
    let mut connection = lock_conn(&state);
    let now = Utc::now();
    let result = search_categories_inner(&mut connection, now, &text);
    log_command_error(&connection, "searchCategories", &result, now);
    result
}

fn log_command_error<T>(
    connection: &Connection,
    operation: &str,
    result: &Result<T, AppError>,
    now: DateTime<Utc>,
) {
    if let Err(error) = result {
        if error.should_log() {
            if let Some(app_data_dir) = crate::db::connection::current_app_data_dir() {
                log_error(
                    connection,
                    "categories",
                    operation,
                    error,
                    now,
                    app_data_dir,
                );
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use chrono::{Duration, TimeZone, Utc};
    use rusqlite::Connection;

    use crate::errors::AppError;
    use crate::models::category::{CategoryList, CategoryListItem};
    use crate::test_utils::{
        assert_allows_both_roles, assert_requires_session, create_fail_notification_insert_trigger,
        create_fail_product_delete_trigger, create_test_session, create_test_user,
        insert_category_fixture, insert_product_fixture, insert_unit_fixture, setup_test_db,
    };
    use crate::timestamps::format_utc_timestamp;

    fn create_category_for_test(
        connection: &mut Connection,
        now: chrono::DateTime<Utc>,
        name: &str,
        description: Option<&str>,
    ) -> Result<CategoryListItem, AppError> {
        crate::commands::categories::create_category_inner(connection, now, name, description)
    }

    fn search_categories_for_test(
        connection: &mut Connection,
        now: chrono::DateTime<Utc>,
        text: &str,
    ) -> Result<Vec<crate::models::category::CategoryMatch>, AppError> {
        crate::commands::categories::search_categories_inner(connection, now, text)
    }

    fn list_categories_for_test(
        connection: &mut Connection,
        now: chrono::DateTime<Utc>,
        search: Option<&str>,
    ) -> Result<CategoryList, AppError> {
        crate::commands::categories::list_categories_inner(connection, now, search)
    }

    fn get_category_for_test(
        connection: &mut Connection,
        now: chrono::DateTime<Utc>,
        category_id: i64,
    ) -> Result<crate::models::category::CategoryDetail, AppError> {
        crate::commands::categories::get_category_inner(connection, now, category_id)
    }

    #[test]
    fn create_category_saves_clean_name_and_description_and_rejects_duplicates_ignoring_case_and_accents(
    ) {
        let mut connection = setup_test_db();
        let now = Utc::now();
        let user_id = create_test_user(
            &connection,
            "catadmin",
            "administrador",
            "+5350000001",
            "12345678901",
            "ValidPass1!",
        );
        create_test_session(&connection, user_id, false, now, now);

        let item =
            create_category_for_test(&mut connection, now, "  Camión  ", Some("  Café molido  "))
                .unwrap();
        assert_eq!(item.name, "Camión");
        assert_eq!(item.description, Some("Café molido".to_owned()));

        let duplicate: Result<CategoryListItem, AppError> =
            create_category_for_test(&mut connection, now, "camion", Some("otra"));
        assert_eq!(duplicate.unwrap_err().code, "duplicateField");

        let duplicate_again = create_category_for_test(&mut connection, now, "CAMIÓN", None);
        let duplicate_error = duplicate_again.unwrap_err();
        assert_eq!(duplicate_error.code, "duplicateField");
        assert_eq!(
            duplicate_error.message,
            "Ya existe una categoría con ese nombre"
        );

        let distinct_by_ntilde =
            create_category_for_test(&mut connection, now, "Año", Some("   ")).unwrap();
        assert_eq!(distinct_by_ntilde.description, None);
        create_category_for_test(&mut connection, now, "Ano", None).unwrap();

        let blank = create_category_for_test(&mut connection, now, "", None);
        let blank_error = blank.unwrap_err();
        assert_eq!(blank_error.code, "validationFailed");
        assert_eq!(blank_error.message, "El nombre es obligatorio");

        let too_long = "x".repeat(51);
        let long_name = create_category_for_test(&mut connection, now, &too_long, None);
        let long_name_error = long_name.unwrap_err();
        assert_eq!(long_name_error.code, "validationFailed");
        assert_eq!(
            long_name_error.message,
            "El nombre no puede tener más de 50 caracteres"
        );

        let long_description = "x".repeat(201);
        let bad_desc =
            create_category_for_test(&mut connection, now, "Pan", Some(&long_description));
        let bad_description_error = bad_desc.unwrap_err();
        assert_eq!(bad_description_error.code, "validationFailed");
        assert_eq!(
            bad_description_error.message,
            "La descripción no puede tener más de 200 caracteres"
        );

        let row: (String, Option<String>, String) = connection
            .query_row(
                "SELECT name, description, nameNormalized FROM categories WHERE id = ?1",
                [item.id],
                |row| Ok((row.get(0)?, row.get(1)?, row.get(2)?)),
            )
            .unwrap();
        assert_eq!(
            row,
            (
                "Camión".to_owned(),
                Some("Café molido".to_owned()),
                "camion".to_owned()
            )
        );

        let empty_description = create_category_for_test(&mut connection, now, "Pan", Some(" \n "));
        assert_eq!(empty_description.unwrap().description, None);
    }

    #[test]
    fn search_categories_matches_normalized_names_and_exact_match_flag() {
        let mut connection = setup_test_db();
        let now = Utc::now();
        let user_id = create_test_user(
            &connection,
            "catsearch",
            "contador",
            "+5350000002",
            "12345678902",
            "ValidPass1!",
        );
        create_test_session(&connection, user_id, false, now, now);

        insert_category_fixture(&connection, "Café", None);
        insert_category_fixture(&connection, "Panadería", None);
        insert_category_fixture(&connection, "Ñame", None);
        insert_category_fixture(&connection, "50% descuento", None);
        insert_category_fixture(&connection, "guion_bajo", None);
        insert_category_fixture(&connection, "Ano", None);

        let results = search_categories_for_test(&mut connection, now, "caf").unwrap();
        assert!(!results.is_empty());
        assert_eq!(results[0].name, "Café");
        assert!(!results[0].is_exact_match);

        let exact = search_categories_for_test(&mut connection, now, "CAFÉ").unwrap();
        assert!(exact
            .iter()
            .any(|item| item.name == "Café" && item.is_exact_match));

        let all = search_categories_for_test(&mut connection, now, "").unwrap();
        assert_eq!(all.len(), 6);

        let with_n = search_categories_for_test(&mut connection, now, "n").unwrap();
        assert!(with_n.iter().any(|item| item.name == "Panadería"));
        assert!(!with_n.iter().any(|item| item.name == "Ñame"));

        let exact = search_categories_for_test(&mut connection, now, "ñame").unwrap();
        assert!(exact
            .iter()
            .any(|item| item.name == "Ñame" && item.is_exact_match));
        assert!(!search_categories_for_test(&mut connection, now, "name")
            .unwrap()
            .iter()
            .any(|item| item.name == "Ñame"));
        assert!(search_categories_for_test(&mut connection, now, "%")
            .unwrap()
            .iter()
            .any(|item| item.name == "50% descuento"));
        assert!(!search_categories_for_test(&mut connection, now, "%")
            .unwrap()
            .iter()
            .any(|item| item.name == "Panadería"));
        assert!(search_categories_for_test(&mut connection, now, "_")
            .unwrap()
            .iter()
            .any(|item| item.name == "guion_bajo"));
        assert!(!search_categories_for_test(&mut connection, now, "_")
            .unwrap()
            .iter()
            .any(|item| item.name == "Panadería"));
    }

    #[test]
    fn list_categories_returns_sorted_items_and_filters_by_normalized_name() {
        let mut connection = setup_test_db();
        let now = Utc::now();
        let user_id = create_test_user(
            &connection,
            "catlist",
            "contador",
            "+5350000006",
            "12345678906",
            "ValidPass1!",
        );
        create_test_session(&connection, user_id, false, now, now);

        let angel_id = insert_category_fixture(&connection, "Ángel", None);
        let nuez_id = insert_category_fixture(&connection, "nuez", None);
        let name_id = insert_category_fixture(&connection, "ñame", None);
        let oso_id = insert_category_fixture(&connection, "oso", None);

        insert_product_fixture(
            &connection,
            "Caja 1",
            Some(angel_id),
            None,
            1000,
            2000,
            10,
            "active",
            None,
            None,
        );
        insert_product_fixture(
            &connection,
            "Caja 2",
            Some(nuez_id),
            None,
            1200,
            2000,
            20,
            "active",
            None,
            None,
        );
        insert_product_fixture(
            &connection,
            "Caja 3",
            Some(nuez_id),
            None,
            1500,
            2000,
            30,
            "active",
            None,
            None,
        );
        insert_product_fixture(
            &connection,
            "Caja 4",
            Some(oso_id),
            None,
            900,
            2000,
            7,
            "discontinued",
            Some("2025-01-01T00:00:00Z"),
            None,
        );

        let list = list_categories_for_test(&mut connection, now, None).unwrap();
        assert_eq!(
            list.items
                .iter()
                .map(|item| item.name.as_str())
                .collect::<Vec<_>>(),
            vec!["Ángel", "nuez", "ñame", "oso"]
        );
        assert_eq!(
            list.items
                .iter()
                .find(|item| item.id == angel_id)
                .unwrap()
                .product_count,
            1
        );
        assert_eq!(
            list.items
                .iter()
                .find(|item| item.id == nuez_id)
                .unwrap()
                .product_count,
            2
        );
        assert_eq!(
            list.items
                .iter()
                .find(|item| item.id == name_id)
                .unwrap()
                .product_count,
            0
        );

        let filtered = list_categories_for_test(&mut connection, now, Some("n")).unwrap();
        assert!(filtered.items.iter().any(|item| item.name == "nuez"));
        assert!(!filtered.items.iter().any(|item| item.name == "ñame"));

        let filtered_exact = list_categories_for_test(&mut connection, now, Some("ñame")).unwrap();
        assert!(filtered_exact.items.iter().any(|item| item.name == "ñame"));
        assert_eq!(filtered_exact.items.len(), 1);
    }

    #[test]
    fn purge_failure_excludes_expired_products_from_category_and_unit_counts_and_lists() {
        let now = Utc.with_ymd_and_hms(2026, 10, 4, 12, 0, 0).unwrap();

        let mut category_connection = setup_test_db();
        let user_id = create_test_user(
            &category_connection,
            "purgecat",
            "contador",
            "+5350000010",
            "12345678910",
            "ValidPass1!",
        );
        create_test_session(&category_connection, user_id, false, now, now);

        let category_id = insert_category_fixture(&category_connection, "Panadería", None);
        let expired_category_product = insert_product_fixture(
            &category_connection,
            "Viejo",
            Some(category_id),
            None,
            700,
            0,
            5,
            "discontinued",
            Some(&format_utc_timestamp(now - Duration::days(61))),
            None,
        );
        let active_category_product = insert_product_fixture(
            &category_connection,
            "Activo",
            Some(category_id),
            None,
            800,
            0,
            8,
            "active",
            None,
            None,
        );
        create_fail_product_delete_trigger(&category_connection).unwrap();

        let list = list_categories_for_test(&mut category_connection, now, None).unwrap();
        assert_eq!(
            list.expired_cleanup_warning.as_deref(),
            Some("No se pudieron limpiar 1 productos vencidos; se reintentará en la próxima operación")
        );
        let category_row = list
            .items
            .iter()
            .find(|item| item.id == category_id)
            .unwrap();
        assert_eq!(category_row.product_count, 1);
        assert!(category_row.product_count < 2);

        let detail = get_category_for_test(&mut category_connection, now, category_id).unwrap();
        assert_eq!(detail.product_count, 1);
        assert!(detail
            .products
            .iter()
            .all(|item| item.id != expired_category_product));
        assert!(detail
            .products
            .iter()
            .any(|item| item.id == active_category_product));

        let delete_result = crate::commands::categories::delete_category_inner(
            &mut category_connection,
            now,
            category_id,
        )
        .unwrap();
        assert_eq!(delete_result.affected_products, 1);
        assert_eq!(
            delete_result.expired_cleanup_warning.as_deref(),
            Some("No se pudieron limpiar 1 productos vencidos; se reintentará en la próxima operación")
        );

        let mut unit_connection = setup_test_db();
        let unit_user_id = create_test_user(
            &unit_connection,
            "purgeunit",
            "contador",
            "+5350000011",
            "12345678911",
            "ValidPass1!",
        );
        create_test_session(&unit_connection, unit_user_id, false, now, now);

        let unit_id = insert_unit_fixture(&unit_connection, "Kilogramo", "kg");
        let expired_unit_product = insert_product_fixture(
            &unit_connection,
            "Viejo kilo",
            None,
            Some(unit_id),
            700,
            0,
            5,
            "discontinued",
            Some(&format_utc_timestamp(now - Duration::days(61))),
            None,
        );
        let active_unit_product = insert_product_fixture(
            &unit_connection,
            "Activo kilo",
            None,
            Some(unit_id),
            800,
            0,
            8,
            "active",
            None,
            None,
        );
        create_fail_product_delete_trigger(&unit_connection).unwrap();

        let unit_list =
            crate::commands::units::list_units_inner(&mut unit_connection, now, None).unwrap();
        assert_eq!(
            unit_list.expired_cleanup_warning.as_deref(),
            Some("No se pudieron limpiar 1 productos vencidos; se reintentará en la próxima operación")
        );
        let unit_row = unit_list
            .items
            .iter()
            .find(|item| item.id == unit_id)
            .unwrap();
        assert_eq!(unit_row.product_count, 1);
        assert!(unit_row.product_count < 2);

        let delete_result =
            crate::commands::units::delete_unit_inner(&mut unit_connection, now, unit_id).unwrap();
        assert_eq!(delete_result.affected_products, 1);
        assert_eq!(
            delete_result.expired_cleanup_warning.as_deref(),
            Some("No se pudieron limpiar 1 productos vencidos; se reintentará en la próxima operación")
        );
        assert!(unit_connection
            .query_row(
                "SELECT EXISTS(SELECT 1 FROM products WHERE id = ?1)",
                [active_unit_product],
                |row| row.get::<_, bool>(0),
            )
            .unwrap());
        assert!(unit_connection
            .query_row(
                "SELECT EXISTS(SELECT 1 FROM products WHERE id = ?1)",
                [expired_unit_product],
                |row| row.get::<_, bool>(0),
            )
            .unwrap());
    }

    #[test]
    fn category_ids_are_not_reused_after_deletion() {
        let mut connection = setup_test_db();
        let now = Utc::now();
        let user_id = create_test_user(
            &connection,
            "catids",
            "contador",
            "+5350000006",
            "12345678906",
            "ValidPass1!",
        );
        create_test_session(&connection, user_id, false, now, now);

        let first = create_category_for_test(&mut connection, now, "Primera", None).unwrap();
        connection
            .execute("DELETE FROM categories WHERE id = ?1", [first.id])
            .unwrap();
        let second = create_category_for_test(&mut connection, now, "Segunda", None).unwrap();
        assert!(second.id > first.id);
    }

    #[test]
    fn create_category_requires_session_and_allows_both_roles() {
        assert_requires_session(|connection, now| {
            create_category_for_test(connection, now, "Permiso", None)
        });
        assert_allows_both_roles(|connection, now| {
            create_category_for_test(connection, now, "Permiso", None)
        });
    }

    #[test]
    fn list_categories_requires_session_and_allows_both_roles() {
        assert_requires_session(|connection, now| list_categories_for_test(connection, now, None));
        assert_allows_both_roles(|connection, now| list_categories_for_test(connection, now, None));
    }

    #[test]
    fn get_category_returns_detail_with_products_in_spanish_order_after_purge() {
        let mut connection = setup_test_db();
        let now = Utc::now();
        let user_id = create_test_user(
            &connection,
            "catdetail",
            "contador",
            "+5350000008",
            "12345678908",
            "ValidPass1!",
        );
        create_test_session(&connection, user_id, false, now, now);

        let category_id = insert_category_fixture(&connection, "Panadería", Some("Desc de prueba"));
        insert_product_fixture(
            &connection,
            "Zanahoria",
            Some(category_id),
            None,
            1000,
            2000,
            0,
            "active",
            None,
            None,
        );
        insert_product_fixture(
            &connection,
            "Árbol",
            Some(category_id),
            None,
            1200,
            2000,
            2,
            "active",
            None,
            None,
        );
        insert_product_fixture(
            &connection,
            "Viejo",
            Some(category_id),
            None,
            500,
            1500,
            3,
            "discontinued",
            Some("2020-01-01T00:00:00Z"),
            None,
        );

        let detail = get_category_for_test(&mut connection, now, category_id).unwrap();
        assert_eq!(detail.name, "Panadería");
        assert_eq!(detail.description, Some("Desc de prueba".to_owned()));
        assert_eq!(detail.product_count, 2);
        assert_eq!(
            detail
                .products
                .iter()
                .map(|item| item.name.as_str())
                .collect::<Vec<_>>(),
            vec!["Árbol", "Zanahoria"]
        );
        assert!(detail
            .products
            .iter()
            .any(|item| item.name == "Árbol" && !item.is_out_of_stock && !item.is_discontinued));
        assert!(detail
            .products
            .iter()
            .any(|item| item.name == "Zanahoria" && item.is_out_of_stock && !item.is_discontinued));

        let missing = get_category_for_test(&mut connection, now, 999).unwrap_err();
        assert_eq!(missing.code, "notFound");
        assert_eq!(missing.message, "La categoría ya no existe");
    }

    #[test]
    fn update_category_validates_name_and_description_and_rejects_missing_or_duplicate_rows() {
        let mut connection = setup_test_db();
        let now = Utc::now();
        let user_id = create_test_user(
            &connection,
            "catupdate",
            "administrador",
            "+5350000007",
            "12345678907",
            "ValidPass1!",
        );
        create_test_session(&connection, user_id, false, now, now);

        let existing_id = insert_category_fixture(&connection, "Panadería", Some("Vieja"));
        insert_category_fixture(&connection, "Bebidas", None);

        let unchanged = crate::commands::categories::update_category_inner(
            &mut connection,
            now,
            existing_id,
            "  Panadería  ",
            Some("  Descripción nueva  "),
        )
        .unwrap();
        assert_eq!(unchanged.name, "Panadería");
        assert_eq!(unchanged.description, Some("Descripción nueva".to_owned()));

        let duplicate = crate::commands::categories::update_category_inner(
            &mut connection,
            now,
            existing_id,
            "bebidas",
            None,
        );
        let duplicate_error = duplicate.unwrap_err();
        assert_eq!(duplicate_error.code, "duplicateField");
        assert_eq!(
            duplicate_error.message,
            "Ya existe una categoría con ese nombre"
        );

        let missing = crate::commands::categories::update_category_inner(
            &mut connection,
            now,
            999,
            "Nuevo",
            None,
        );
        let missing_error = missing.unwrap_err();
        assert_eq!(missing_error.code, "notFound");
        assert_eq!(missing_error.message, "La categoría ya no existe");

        let invalid = crate::commands::categories::update_category_inner(
            &mut connection,
            now,
            existing_id,
            "",
            Some(&"x".repeat(201)),
        );
        let invalid_error = invalid.unwrap_err();
        assert_eq!(invalid_error.code, "validationFailed");
        assert_eq!(invalid_error.message, "El nombre es obligatorio");
    }

    #[test]
    fn update_category_requires_session_and_allows_both_roles() {
        assert_requires_session(|connection, now| {
            crate::commands::categories::update_category_inner(connection, now, 1, "Permiso", None)
        });
        assert_allows_both_roles(|connection, now| {
            crate::commands::categories::update_category_inner(connection, now, 1, "Permiso", None)
        });
    }

    #[test]
    fn search_categories_requires_session_and_allows_both_roles() {
        assert_requires_session(|connection, now| search_categories_for_test(connection, now, ""));
        assert_allows_both_roles(|connection, now| search_categories_for_test(connection, now, ""));
    }

    fn delete_category_for_test(
        connection: &mut Connection,
        now: chrono::DateTime<Utc>,
        category_id: i64,
    ) -> Result<crate::models::category::DeleteCategoryResult, AppError> {
        crate::commands::categories::delete_category_inner(connection, now, category_id)
    }

    #[test]
    fn delete_category_removes_category_with_notification_and_keeps_transaction_atomic() {
        let mut connection = setup_test_db();
        let now = Utc::now();
        let user_id = create_test_user(
            &connection,
            "catdelete",
            "administrador",
            "+5350000010",
            "12345678910",
            "ValidPass1!",
        );
        create_test_session(&connection, user_id, false, now, now);

        let category_id = insert_category_fixture(&connection, "Panadería", Some("Desc"));
        insert_product_fixture(
            &connection,
            "Árbol",
            Some(category_id),
            None,
            1200,
            2000,
            2,
            "active",
            None,
            None,
        );
        insert_product_fixture(
            &connection,
            "Zanahoria",
            Some(category_id),
            None,
            900,
            2000,
            0,
            "active",
            None,
            None,
        );
        insert_product_fixture(
            &connection,
            "Viejo",
            Some(category_id),
            None,
            500,
            1500,
            3,
            "discontinued",
            Some("2020-01-01T00:00:00Z"),
            None,
        );

        let result = delete_category_for_test(&mut connection, now, category_id).unwrap();
        assert_eq!(result.affected_products, 2);
        assert_eq!(result.expired_cleanup_warning, None);

        let notification: (String, String, String) = connection
            .query_row(
                "SELECT type, priority, message FROM notifications WHERE type = 'N-09'",
                [],
                |row| Ok((row.get(0)?, row.get(1)?, row.get(2)?)),
            )
            .unwrap();
        assert_eq!(notification.0, "N-09");
        assert_eq!(notification.1, "ADVERTENCIA");
        assert_eq!(
            notification.2,
            "Los siguientes productos quedaron sin categoría: Árbol, Zanahoria"
        );

        let remaining_products: Vec<(i64, Option<i64>, String)> = connection
            .prepare(
                "SELECT id, categoryId, name FROM products WHERE categoryId IS NULL ORDER BY id",
            )
            .unwrap()
            .query_map([], |row| Ok((row.get(0)?, row.get(1)?, row.get(2)?)))
            .unwrap()
            .collect::<Result<Vec<_>, _>>()
            .unwrap();
        assert_eq!(remaining_products.len(), 2);
        assert!(remaining_products
            .iter()
            .all(|(_, category_id, _)| category_id.is_none()));

        let category_exists: i64 = connection
            .query_row(
                "SELECT COUNT(*) FROM categories WHERE id = ?1",
                [category_id],
                |row| row.get(0),
            )
            .unwrap();
        assert_eq!(category_exists, 0);

        let mut rollback_connection = setup_test_db();
        let rollback_user_id = create_test_user(
            &rollback_connection,
            "catrollback",
            "contador",
            "+5350000011",
            "12345678911",
            "ValidPass1!",
        );
        create_test_session(&rollback_connection, rollback_user_id, false, now, now);
        let rollback_category_id = insert_category_fixture(&rollback_connection, "Otro", None);
        let rollback_product_id = insert_product_fixture(
            &rollback_connection,
            "Producto a conservar",
            Some(rollback_category_id),
            None,
            1000,
            2000,
            1,
            "active",
            None,
            None,
        );
        create_fail_notification_insert_trigger(&rollback_connection).unwrap();

        let rollback_error =
            delete_category_for_test(&mut rollback_connection, now, rollback_category_id)
                .unwrap_err();
        assert_eq!(rollback_error.code, "internal");
        assert_eq!(rollback_error.message, "Ocurrió un error inesperado");

        let rollback_category_count: i64 = rollback_connection
            .query_row(
                "SELECT COUNT(*) FROM categories WHERE id = ?1",
                [rollback_category_id],
                |row| row.get(0),
            )
            .unwrap();
        assert_eq!(rollback_category_count, 1);

        let rollback_product_category: Option<i64> = rollback_connection
            .query_row(
                "SELECT categoryId FROM products WHERE id = ?1",
                [rollback_product_id],
                |row| row.get(0),
            )
            .unwrap();
        assert_eq!(rollback_product_category, Some(rollback_category_id));

        let notification_count: i64 = rollback_connection
            .query_row("SELECT COUNT(*) FROM notifications", [], |row| row.get(0))
            .unwrap();
        assert_eq!(notification_count, 0);
    }
}
