use chrono::{DateTime, Utc};
use rusqlite::{params, Connection, OptionalExtension};

use crate::commands::auth::{require_session, Author};
use crate::commands::products::purge_best_effort;
use crate::db::connection::lock_conn;
use crate::db::notifications::{insert_affected_products_notification, AffectedProductsKind};
use crate::errors::{log_error, map_db_error, AppError};
use crate::models::unit::{DeleteUnitResult, UnitList, UnitListItem, UnitMatch};
use crate::text::{clean_name, normalize_text, sort_spanish, spanish_sort_key};
use crate::timestamps::format_utc_timestamp;
use crate::validators::validate_name_with_messages;

const UNIT_NAME_DUPLICATE_MESSAGE: &str = "Ya existe una unidad con ese nombre";
const UNIT_ABBREVIATION_DUPLICATE_MESSAGE: &str = "Ya existe una unidad con esa abreviación";
const UNIT_NAME_REQUIRED_MESSAGE: &str = "El nombre es obligatorio";
const UNIT_NAME_TOO_LONG_MESSAGE: &str = "El nombre no puede tener más de 30 caracteres";
const UNIT_NAME_FORBIDDEN_MESSAGE: &str = "El nombre contiene caracteres no permitidos";
const UNIT_ABBREVIATION_REQUIRED_MESSAGE: &str = "La abreviación es obligatoria";
const UNIT_ABBREVIATION_TOO_LONG_MESSAGE: &str =
    "La abreviación no puede tener más de 10 caracteres";
const UNIT_ABBREVIATION_FORBIDDEN_MESSAGE: &str =
    "La abreviación contiene caracteres no permitidos";

pub fn create_unit_inner(
    connection: &mut Connection,
    now: DateTime<Utc>,
    name: &str,
    abbreviation: &str,
) -> Result<UnitListItem, AppError> {
    require_session(connection, now)?;

    let (cleaned_name, normalized_name) = validate_name_with_messages(
        name,
        1,
        30,
        UNIT_NAME_REQUIRED_MESSAGE,
        UNIT_NAME_TOO_LONG_MESSAGE,
        UNIT_NAME_FORBIDDEN_MESSAGE,
    )?;
    let (cleaned_abbreviation, normalized_abbreviation) = validate_name_with_messages(
        abbreviation,
        1,
        10,
        UNIT_ABBREVIATION_REQUIRED_MESSAGE,
        UNIT_ABBREVIATION_TOO_LONG_MESSAGE,
        UNIT_ABBREVIATION_FORBIDDEN_MESSAGE,
    )?;
    let timestamp = format_utc_timestamp(now);

    connection
        .execute(
            "INSERT INTO units (
                name, nameNormalized, abbreviation, abbreviationNormalized, createdAt, updatedAt
             ) VALUES (?1, ?2, ?3, ?4, ?5, ?5)",
            params![
                cleaned_name,
                normalized_name,
                cleaned_abbreviation,
                normalized_abbreviation,
                timestamp
            ],
        )
        .map_err(|error| {
            map_db_error(
                error,
                "createUnit",
                &[
                    ("units.nameNormalized", UNIT_NAME_DUPLICATE_MESSAGE),
                    (
                        "units.abbreviationNormalized",
                        UNIT_ABBREVIATION_DUPLICATE_MESSAGE,
                    ),
                ],
            )
        })?;

    Ok(UnitListItem {
        id: connection.last_insert_rowid(),
        name: cleaned_name,
        abbreviation: cleaned_abbreviation,
        product_count: 0,
        conversion_count: 0,
    })
}

#[tauri::command]
pub fn create_unit(
    state: tauri::State<'_, std::sync::Mutex<Connection>>,
    name: String,
    abbreviation: String,
) -> Result<UnitListItem, AppError> {
    let mut connection = lock_conn(&state);
    let now = Utc::now();
    let result = create_unit_inner(&mut connection, now, &name, &abbreviation);
    log_command_error(&connection, "createUnit", &result, now);
    result
}

pub fn list_units_inner(
    connection: &mut Connection,
    now: DateTime<Utc>,
    search: Option<&str>,
) -> Result<UnitList, AppError> {
    require_session(connection, now)?;

    let warning = match purge_best_effort(connection, now) {
        Ok(warning) => warning,
        Err(error) => {
            if let Some(app_data_dir) = crate::db::connection::current_app_data_dir() {
                log_error(connection, "units", "listUnits", &error, now, app_data_dir);
            }
            Some("No se pudieron limpiar los productos vencidos; se reintentará en la próxima operación".to_owned())
        }
    };

    let cutoff = crate::commands::products::expired_products_cutoff(now);
    let normalized_search = normalize_text(&clean_name(search.unwrap_or("")));
    let mut statement = connection
        .prepare(
            "SELECT id, name, abbreviation,
                    (SELECT COUNT(*) FROM products WHERE unitId = units.id AND (status <> 'discontinued' OR discontinuedAt > ?2)),
                    (SELECT COUNT(*) FROM unitConversions WHERE fromUnitId = units.id OR toUnitId = units.id)
             FROM units
             WHERE ?1 = ''
                OR instr(nameNormalized, ?1) > 0
                OR instr(abbreviationNormalized, ?1) > 0
             ORDER BY id ASC",
        )
        .map_err(|error| map_db_error(error, "listUnits", &[]))?;

    let rows = statement
        .query_map(params![&normalized_search, &cutoff], |row| {
            Ok(UnitListItem {
                id: row.get(0)?,
                name: row.get(1)?,
                abbreviation: row.get(2)?,
                product_count: row.get(3)?,
                conversion_count: row.get(4)?,
            })
        })
        .map_err(|error| map_db_error(error, "listUnits", &[]))?;

    let mut items = rows
        .map(|row| row.map_err(|error| map_db_error(error, "listUnits", &[])))
        .collect::<Result<Vec<_>, AppError>>()?;
    sort_spanish(&mut items, |item| (spanish_sort_key(&item.name), item.id));

    Ok(UnitList {
        items,
        expired_cleanup_warning: warning,
    })
}

#[tauri::command]
pub fn list_units(
    state: tauri::State<'_, std::sync::Mutex<Connection>>,
    search: Option<String>,
) -> Result<UnitList, AppError> {
    let mut connection = lock_conn(&state);
    let now = Utc::now();
    let result = list_units_inner(&mut connection, now, search.as_deref());
    log_command_error(&connection, "listUnits", &result, now);
    result
}

pub fn update_unit_inner(
    connection: &mut Connection,
    now: DateTime<Utc>,
    unit_id: i64,
    name: &str,
    abbreviation: &str,
) -> Result<UnitListItem, AppError> {
    require_session(connection, now)?;

    let exists: Option<i64> = connection
        .query_row("SELECT id FROM units WHERE id = ?1", [unit_id], |row| {
            row.get(0)
        })
        .optional()
        .map_err(|error| map_db_error(error, "updateUnit", &[]))?;
    if exists.is_none() {
        return Err(AppError::new(
            "notFound",
            "La unidad ya no existe",
            "unit missing during update",
        ));
    }

    let (cleaned_name, normalized_name) = validate_name_with_messages(
        name,
        1,
        30,
        UNIT_NAME_REQUIRED_MESSAGE,
        UNIT_NAME_TOO_LONG_MESSAGE,
        UNIT_NAME_FORBIDDEN_MESSAGE,
    )?;
    let (cleaned_abbreviation, normalized_abbreviation) = validate_name_with_messages(
        abbreviation,
        1,
        10,
        UNIT_ABBREVIATION_REQUIRED_MESSAGE,
        UNIT_ABBREVIATION_TOO_LONG_MESSAGE,
        UNIT_ABBREVIATION_FORBIDDEN_MESSAGE,
    )?;
    let timestamp = format_utc_timestamp(now);

    connection
        .execute(
            "UPDATE units
             SET name = ?1, nameNormalized = ?2, abbreviation = ?3, abbreviationNormalized = ?4, updatedAt = ?5
             WHERE id = ?6",
            params![
                cleaned_name,
                normalized_name,
                cleaned_abbreviation,
                normalized_abbreviation,
                timestamp,
                unit_id
            ],
        )
        .map_err(|error| {
            map_db_error(
                error,
                "updateUnit",
                &[
                    ("units.nameNormalized", UNIT_NAME_DUPLICATE_MESSAGE),
                    (
                        "units.abbreviationNormalized",
                        UNIT_ABBREVIATION_DUPLICATE_MESSAGE,
                    ),
                ],
            )
        })?;

    let product_count: i64 = connection
        .query_row(
            "SELECT COUNT(*) FROM products WHERE unitId = ?1",
            [unit_id],
            |row| row.get(0),
        )
        .map_err(|error| map_db_error(error, "updateUnit", &[]))?;
    let conversion_count: i64 = connection
        .query_row(
            "SELECT COUNT(*) FROM unitConversions WHERE fromUnitId = ?1 OR toUnitId = ?1",
            [unit_id],
            |row| row.get(0),
        )
        .map_err(|error| map_db_error(error, "updateUnit", &[]))?;

    Ok(UnitListItem {
        id: unit_id,
        name: cleaned_name,
        abbreviation: cleaned_abbreviation,
        product_count,
        conversion_count,
    })
}

#[tauri::command]
pub fn update_unit(
    state: tauri::State<'_, std::sync::Mutex<Connection>>,
    unit_id: i64,
    name: String,
    abbreviation: String,
) -> Result<UnitListItem, AppError> {
    let mut connection = lock_conn(&state);
    let now = Utc::now();
    let result = update_unit_inner(&mut connection, now, unit_id, &name, &abbreviation);
    log_command_error(&connection, "updateUnit", &result, now);
    result
}

pub fn delete_unit_inner(
    connection: &mut Connection,
    now: DateTime<Utc>,
    unit_id: i64,
) -> Result<DeleteUnitResult, AppError> {
    let actor = require_session(connection, now)?;

    let tx = connection
        .transaction()
        .map_err(|error| map_db_error(error, "deleteUnit", &[]))?;

    let expired_cleanup_warning = match purge_best_effort(&tx, now) {
        Ok(warning) => warning,
        Err(error) => {
            if let Some(app_data_dir) = crate::db::connection::current_app_data_dir() {
                log_error(&tx, "units", "deleteUnit", &error, now, app_data_dir);
            }
            Some("No se pudieron limpiar los productos vencidos; se reintentará en la próxima operación".to_owned())
        }
    };

    let unit_exists = tx
        .query_row("SELECT id FROM units WHERE id = ?1", [unit_id], |row| {
            row.get::<_, i64>(0)
        })
        .optional()
        .map_err(|error| map_db_error(error, "deleteUnit", &[]))?;
    if unit_exists.is_none() {
        return Err(AppError::new(
            "notFound",
            "La unidad ya no existe",
            "unit missing during deletion",
        ));
    }

    let cutoff = crate::commands::products::expired_products_cutoff(now);
    let deleted_conversions: usize = tx
        .query_row(
            "SELECT COUNT(*) FROM unitConversions WHERE fromUnitId = ?1 OR toUnitId = ?1",
            [unit_id],
            |row| row.get(0),
        )
        .map_err(|error| map_db_error(error, "deleteUnit", &[]))?;
    let affected_products: Vec<(i64, String)> = tx
        .prepare(
            "SELECT id, name FROM products WHERE unitId = ?1 AND (status <> 'discontinued' OR discontinuedAt > ?2) ORDER BY id ASC",
        )
        .map_err(|error| map_db_error(error, "deleteUnit", &[]))?
        .query_map(params![unit_id, cutoff], |row| Ok((row.get(0)?, row.get(1)?)))
        .map_err(|error| map_db_error(error, "deleteUnit", &[]))?
        .collect::<Result<Vec<_>, rusqlite::Error>>()
        .map_err(|error| map_db_error(error, "deleteUnit", &[]))?;

    if !affected_products.is_empty() {
        insert_affected_products_notification(
            &tx,
            AffectedProductsKind::Unit,
            &affected_products,
            now,
        )
        .map_err(|error| map_db_error(error, "deleteUnit", &[]))?;

        let updated_at = format_utc_timestamp(now);
        tx.execute(
            "UPDATE products SET unitId = NULL, updatedAt = ?1 WHERE unitId = ?2 AND (status <> 'discontinued' OR discontinuedAt > ?3)",
            params![updated_at, unit_id, cutoff],
        )
        .map_err(|error| map_db_error(error, "deleteUnit", &[]))?;
    }

    crate::activity::record_deletion(
        &tx,
        &Author::User(&actor),
        now,
        "unit",
        "SELECT name FROM units WHERE id = ?1",
        unit_id,
    )
    .map_err(|error| map_db_error(error, "deleteUnit", &[]))?;

    tx.execute(
        "DELETE FROM unitConversions WHERE fromUnitId = ?1 OR toUnitId = ?1",
        [unit_id],
    )
    .map_err(|error| map_db_error(error, "deleteUnit", &[]))?;

    tx.execute("DELETE FROM units WHERE id = ?1", [unit_id])
        .map_err(|error| map_db_error(error, "deleteUnit", &[]))?;

    tx.commit()
        .map_err(|error| map_db_error(error, "deleteUnit", &[]))?;

    Ok(DeleteUnitResult {
        affected_products: affected_products.len(),
        deleted_conversions,
        expired_cleanup_warning,
    })
}

#[tauri::command]
pub fn delete_unit(
    state: tauri::State<'_, std::sync::Mutex<Connection>>,
    unit_id: i64,
) -> Result<DeleteUnitResult, AppError> {
    let mut connection = lock_conn(&state);
    let now = Utc::now();
    let result = delete_unit_inner(&mut connection, now, unit_id);
    log_command_error(&connection, "deleteUnit", &result, now);
    result
}

pub fn search_units_inner(
    connection: &mut Connection,
    now: DateTime<Utc>,
    text: &str,
) -> Result<Vec<UnitMatch>, AppError> {
    require_session(connection, now)?;
    let normalized_text = normalize_text(&clean_name(text));
    let mut statement = connection
        .prepare(
            "SELECT id, name, abbreviation, nameNormalized, abbreviationNormalized
             FROM units
             WHERE ?1 = ''
                OR instr(nameNormalized, ?1) > 0
                OR instr(abbreviationNormalized, ?1) > 0
             ORDER BY id ASC",
        )
        .map_err(|error| map_db_error(error, "searchUnits", &[]))?;
    let rows = statement
        .query_map([&normalized_text], |row| {
            let name_normalized: String = row.get(3)?;
            let abbreviation_normalized: String = row.get(4)?;
            Ok(UnitMatch {
                id: row.get(0)?,
                name: row.get(1)?,
                abbreviation: row.get(2)?,
                is_exact_match: name_normalized == normalized_text
                    || abbreviation_normalized == normalized_text,
            })
        })
        .map_err(|error| map_db_error(error, "searchUnits", &[]))?;

    let mut matches = rows
        .map(|row| row.map_err(|error| map_db_error(error, "searchUnits", &[])))
        .collect::<Result<Vec<_>, AppError>>()?;
    sort_spanish(&mut matches, |item| (spanish_sort_key(&item.name), item.id));
    Ok(matches)
}

#[tauri::command]
pub fn search_units(
    state: tauri::State<'_, std::sync::Mutex<Connection>>,
    text: String,
) -> Result<Vec<UnitMatch>, AppError> {
    let mut connection = lock_conn(&state);
    let now = Utc::now();
    let result = search_units_inner(&mut connection, now, &text);
    log_command_error(&connection, "searchUnits", &result, now);
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
                log_error(connection, "units", operation, error, now, app_data_dir);
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use chrono::Utc;
    use rusqlite::Connection;

    use crate::errors::AppError;
    use crate::models::unit::{UnitListItem, UnitMatch};
    use crate::test_utils::{
        assert_allows_both_roles, assert_requires_session, create_fail_notification_insert_trigger,
        create_test_session, create_test_user, insert_conversion_fixture, insert_product_fixture,
        insert_unit_fixture, setup_test_db,
    };

    fn create_unit_for_test(
        connection: &mut Connection,
        now: chrono::DateTime<Utc>,
        name: &str,
        abbreviation: &str,
    ) -> Result<UnitListItem, AppError> {
        crate::commands::units::create_unit_inner(connection, now, name, abbreviation)
    }

    fn search_units_for_test(
        connection: &mut Connection,
        now: chrono::DateTime<Utc>,
        text: &str,
    ) -> Result<Vec<UnitMatch>, AppError> {
        crate::commands::units::search_units_inner(connection, now, text)
    }

    fn list_units_for_test(
        connection: &mut Connection,
        now: chrono::DateTime<Utc>,
        search: Option<&str>,
    ) -> Result<crate::models::unit::UnitList, AppError> {
        crate::commands::units::list_units_inner(connection, now, search)
    }

    fn update_unit_for_test(
        connection: &mut Connection,
        now: chrono::DateTime<Utc>,
        unit_id: i64,
        name: &str,
        abbreviation: &str,
    ) -> Result<UnitListItem, AppError> {
        crate::commands::units::update_unit_inner(connection, now, unit_id, name, abbreviation)
    }

    fn delete_unit_for_test(
        connection: &mut Connection,
        now: chrono::DateTime<Utc>,
        unit_id: i64,
    ) -> Result<crate::models::unit::DeleteUnitResult, AppError> {
        crate::commands::units::delete_unit_inner(connection, now, unit_id)
    }

    fn add_session(connection: &Connection, username: &str, role: &str) {
        let now = Utc::now();
        let user_id = create_test_user(
            connection,
            username,
            role,
            "+5350000001",
            "12345678901",
            "ValidPass1!",
        );
        create_test_session(connection, user_id, false, now, now);
    }

    #[test]
    fn create_unit_saves_clean_values_and_normalized_keys() {
        let mut connection = setup_test_db();
        add_session(&connection, "unitcreate", "administrador");
        let now = Utc::now();

        let unit = create_unit_for_test(&mut connection, now, "  Kilógramo  ", "  KÍ  ").unwrap();

        assert_eq!(unit.name, "Kilógramo");
        assert_eq!(unit.abbreviation, "KÍ");
        assert_eq!(unit.product_count, 0);
        assert_eq!(unit.conversion_count, 0);
        let normalized: (String, String) = connection
            .query_row(
                "SELECT nameNormalized, abbreviationNormalized FROM units WHERE id = ?1",
                [unit.id],
                |row| Ok((row.get(0)?, row.get(1)?)),
            )
            .unwrap();
        assert_eq!(normalized, ("kilogramo".to_owned(), "ki".to_owned()));
    }

    #[test]
    fn create_unit_rejects_duplicate_name_and_abbreviation_with_field_specific_messages() {
        let mut connection = setup_test_db();
        add_session(&connection, "unitduplicate", "administrador");
        let now = Utc::now();
        create_unit_for_test(&mut connection, now, "Kilógramo", "kí").unwrap();

        let duplicate_name =
            create_unit_for_test(&mut connection, now, "KILOGRAMO", "kg").unwrap_err();
        assert_eq!(duplicate_name.code, "duplicateField");
        assert_eq!(
            duplicate_name.message,
            "Ya existe una unidad con ese nombre"
        );

        let duplicate_abbreviation =
            create_unit_for_test(&mut connection, now, "Otra unidad", "KI").unwrap_err();
        assert_eq!(duplicate_abbreviation.code, "duplicateField");
        assert_eq!(
            duplicate_abbreviation.message,
            "Ya existe una unidad con esa abreviación"
        );
    }

    #[test]
    fn create_unit_rejects_empty_oversized_and_forbidden_text() {
        let mut connection = setup_test_db();
        add_session(&connection, "unitvalidation", "contador");
        let now = Utc::now();

        for (name, abbreviation, message) in [
            ("", "kg", "El nombre es obligatorio"),
            (
                &"x".repeat(31),
                "kg",
                "El nombre no puede tener más de 30 caracteres",
            ),
            ("Kilogramo", "", "La abreviación es obligatoria"),
            (
                "Kilogramo",
                &"x".repeat(11),
                "La abreviación no puede tener más de 10 caracteres",
            ),
            (
                "Kilo\u{200B}gramo",
                "kg",
                "El nombre contiene caracteres no permitidos",
            ),
            (
                "Kilogramo",
                "k\u{200B}g",
                "La abreviación contiene caracteres no permitidos",
            ),
        ] {
            let error = create_unit_for_test(&mut connection, now, name, abbreviation).unwrap_err();
            assert_eq!(error.code, "validationFailed");
            assert_eq!(error.message, message);
        }
    }

    #[test]
    fn search_units_matches_name_and_abbreviation_and_preserves_double_exact_match() {
        let mut connection = setup_test_db();
        add_session(&connection, "unitsearch", "contador");
        let now = Utc::now();
        let kilogram_id = insert_unit_fixture(&connection, "Kilogramo", "kg");
        let key_unit_id = insert_unit_fixture(&connection, "kg", "k");
        insert_unit_fixture(&connection, "Porcentaje", "%");
        insert_unit_fixture(&connection, "Unidad", "u_1");

        let exact_matches = search_units_for_test(&mut connection, now, "KG").unwrap();
        assert_eq!(exact_matches.len(), 2);
        assert!(exact_matches.iter().all(|item| item.is_exact_match));
        assert!(exact_matches.iter().any(|item| item.id == kilogram_id));
        assert!(exact_matches.iter().any(|item| item.id == key_unit_id));

        let name_match = search_units_for_test(&mut connection, now, "kil").unwrap();
        assert_eq!(name_match.len(), 1);
        assert_eq!(name_match[0].id, kilogram_id);
        assert!(!name_match[0].is_exact_match);

        assert_eq!(
            search_units_for_test(&mut connection, now, "%")
                .unwrap()
                .len(),
            1
        );
        assert_eq!(
            search_units_for_test(&mut connection, now, "u_1")
                .unwrap()
                .len(),
            1
        );
    }

    #[test]
    fn list_units_returns_product_and_conversion_counts_and_searches_by_name_or_abbreviation() {
        let mut connection = setup_test_db();
        add_session(&connection, "unitlist", "contador");
        let now = Utc::now();

        let kilogram_id = insert_unit_fixture(&connection, "Kilogramo", "kg");
        let gram_id = insert_unit_fixture(&connection, "Gramo", "g");
        let unit_id = insert_unit_fixture(&connection, "Unidad", "u_1");
        insert_conversion_fixture(&connection, kilogram_id, gram_id, 1000);
        insert_conversion_fixture(&connection, unit_id, kilogram_id, 10);

        insert_product_fixture(
            &connection,
            "Arroz",
            None,
            Some(kilogram_id),
            1200,
            25,
            5000,
            "active",
            None,
            None,
        );
        insert_product_fixture(
            &connection,
            "Viejo",
            None,
            Some(gram_id),
            1000,
            10,
            2000,
            "discontinued",
            Some("2020-01-01T00:00:00Z"),
            None,
        );

        let list = list_units_for_test(&mut connection, now, Some("kg")).unwrap();
        assert_eq!(list.items.len(), 1);
        assert_eq!(list.items[0].name, "Kilogramo");
        assert_eq!(list.items[0].abbreviation, "kg");
        assert_eq!(list.items[0].product_count, 1);
        assert_eq!(list.items[0].conversion_count, 2);
        assert_eq!(list.expired_cleanup_warning, None);

        let by_abbreviation = list_units_for_test(&mut connection, now, Some("u_1")).unwrap();
        assert_eq!(by_abbreviation.items.len(), 1);
        assert_eq!(by_abbreviation.items[0].name, "Unidad");

        let all_units = list_units_for_test(&mut connection, now, None).unwrap();
        assert_eq!(all_units.items.len(), 3);
        assert!(all_units.items.iter().any(|item| item.name == "Kilogramo"));
        assert!(all_units.items.iter().any(|item| item.name == "Gramo"));
        assert!(all_units.items.iter().any(|item| item.name == "Unidad"));
    }

    #[test]
    fn update_unit_changes_values_and_rejects_duplicates_or_missing_rows() {
        let mut connection = setup_test_db();
        add_session(&connection, "unitupdate", "administrador");
        let now = Utc::now();

        let unit_id = create_unit_for_test(&mut connection, now, "Kilogramo", "kg")
            .unwrap()
            .id;
        let other_id = create_unit_for_test(&mut connection, now, "Litro", "l")
            .unwrap()
            .id;

        let updated =
            update_unit_for_test(&mut connection, now, unit_id, "  Kilo  ", "  KG  ").unwrap();
        assert_eq!(updated.name, "Kilo");
        assert_eq!(updated.abbreviation, "KG");
        assert_eq!(updated.product_count, 0);
        assert_eq!(updated.conversion_count, 0);

        let duplicate_name =
            update_unit_for_test(&mut connection, now, other_id, "KILO", "ml").unwrap_err();
        assert_eq!(duplicate_name.code, "duplicateField");
        assert_eq!(
            duplicate_name.message,
            "Ya existe una unidad con ese nombre"
        );

        let duplicate_abbreviation =
            update_unit_for_test(&mut connection, now, other_id, "Metro", "KG").unwrap_err();
        assert_eq!(duplicate_abbreviation.code, "duplicateField");
        assert_eq!(
            duplicate_abbreviation.message,
            "Ya existe una unidad con esa abreviación"
        );

        let missing = update_unit_for_test(&mut connection, now, 9999, "Nada", "n").unwrap_err();
        assert_eq!(missing.code, "notFound");
        assert_eq!(missing.message, "La unidad ya no existe");
    }

    #[test]
    fn delete_unit_removes_conversions_and_products_without_breaking_atomicity() {
        let mut connection = setup_test_db();
        add_session(&connection, "unitdelete", "contador");
        let now = Utc::now();

        let unit_id = insert_unit_fixture(&connection, "Kilogramo", "kg");
        let target_id = insert_unit_fixture(&connection, "Gramo", "g");
        insert_conversion_fixture(&connection, unit_id, target_id, 1000);
        insert_product_fixture(
            &connection,
            "Arroz",
            None,
            Some(unit_id),
            1200,
            25,
            5000,
            "active",
            None,
            None,
        );
        insert_product_fixture(
            &connection,
            "Azúcar",
            None,
            Some(unit_id),
            800,
            10,
            1000,
            "discontinued",
            Some("2020-01-01T00:00:00Z"),
            None,
        );

        let result = delete_unit_for_test(&mut connection, now, unit_id).unwrap();
        assert_eq!(result.affected_products, 1);
        assert_eq!(result.deleted_conversions, 1);
        assert_eq!(result.expired_cleanup_warning, None);

        let notification_count: i64 = connection
            .query_row(
                "SELECT COUNT(*) FROM notifications WHERE type = 'N-08'",
                [],
                |row| row.get(0),
            )
            .unwrap();
        assert_eq!(notification_count, 1);

        let remaining_products: i64 = connection
            .query_row(
                "SELECT COUNT(*) FROM products WHERE unitId IS NULL",
                [],
                |row| row.get(0),
            )
            .unwrap();
        assert_eq!(remaining_products, 1);

        let missing = delete_unit_for_test(&mut connection, now, 9999).unwrap_err();
        assert_eq!(missing.code, "notFound");
        assert_eq!(missing.message, "La unidad ya no existe");

        let mut atomic_connection = setup_test_db();
        let user_id = create_test_user(
            &atomic_connection,
            "unitdeleteatomic",
            "administrador",
            "+5350000011",
            "12345678911",
            "ValidPass1!",
        );
        create_test_session(&atomic_connection, user_id, false, now, now);
        let atomic_unit = insert_unit_fixture(&atomic_connection, "Litro", "l");
        let atomic_target = insert_unit_fixture(&atomic_connection, "Gramo", "g");
        insert_conversion_fixture(&atomic_connection, atomic_unit, atomic_target, 2);
        insert_product_fixture(
            &atomic_connection,
            "Leche",
            None,
            Some(atomic_unit),
            1500,
            10,
            750,
            "active",
            None,
            None,
        );
        create_fail_notification_insert_trigger(&atomic_connection).unwrap();

        let atomic_error =
            delete_unit_for_test(&mut atomic_connection, now, atomic_unit).unwrap_err();
        assert_eq!(atomic_error.code, "internal");
        assert!(
            atomic_connection
                .query_row(
                    "SELECT COUNT(*) FROM units WHERE id = ?1",
                    [atomic_unit],
                    |row| row.get::<_, i64>(0)
                )
                .unwrap()
                == 1
        );
    }

    #[test]
    fn create_update_and_search_units_require_session_and_allow_both_roles() {
        assert_requires_session(|connection, now| {
            create_unit_for_test(connection, now, "Unidad", "u")
        });
        assert_allows_both_roles(|connection, now| {
            create_unit_for_test(connection, now, "Unidad", "u")
        });
        assert_requires_session(|connection, now| search_units_for_test(connection, now, ""));
        assert_allows_both_roles(|connection, now| search_units_for_test(connection, now, ""));
        assert_requires_session(|connection, now| list_units_for_test(connection, now, None));
        assert_allows_both_roles(|connection, now| list_units_for_test(connection, now, None));

        let mut connection = setup_test_db();
        let user_id = create_test_user(
            &connection,
            "unitupdrole",
            "administrador",
            "+5350000010",
            "12345678910",
            "ValidPass1!",
        );
        create_test_session(&connection, user_id, false, Utc::now(), Utc::now());
        let unit_id = create_unit_for_test(&mut connection, Utc::now(), "Unidad", "u")
            .unwrap()
            .id;

        assert_requires_session(|connection, now| {
            update_unit_for_test(connection, now, 1, "Otro", "o")
        });
        assert_allows_both_roles(|connection, now| {
            update_unit_for_test(connection, now, unit_id, "Otro", "o")
        });

        let mut delete_connection = setup_test_db();
        let delete_user_id = create_test_user(
            &delete_connection,
            "unitdeleterole",
            "contador",
            "+5350000012",
            "12345678912",
            "ValidPass1!",
        );
        create_test_session(
            &delete_connection,
            delete_user_id,
            false,
            Utc::now(),
            Utc::now(),
        );
        let delete_unit_id =
            create_unit_for_test(&mut delete_connection, Utc::now(), "Unidad a borrar", "ub")
                .unwrap()
                .id;
        assert_requires_session(|connection, now| delete_unit_for_test(connection, now, 1));
        assert_allows_both_roles(|connection, now| {
            delete_unit_for_test(connection, now, delete_unit_id)
        });
    }
}
