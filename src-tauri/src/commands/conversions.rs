use std::collections::{HashMap, HashSet, VecDeque};

use chrono::{DateTime, Utc};
use rusqlite::{params, Connection, OptionalExtension};

use crate::commands::auth::require_session;
use crate::db::connection::lock_conn;
use crate::errors::{log_error, map_db_error, AppError};
use crate::models::conversion::{ConversionItem, ConversionResult, UnitReference};
use crate::models::product::format_factor_micro;
use crate::text::{clean_name, normalize_text, sort_spanish, spanish_sort_key};
use crate::timestamps::format_utc_timestamp;
use crate::validators::{parse_factor_micro, parse_quantity_micro};

const SAME_UNIT_MESSAGE: &str = "La unidad de origen y destino no pueden ser la mismas";
const DUPLICATE_CONVERSION_MESSAGE: &str = "Ya existe una conversión entre esas unidades";
const SECOND_PATH_MESSAGE: &str = "Las unidades ya están relacionadas entre sí";
const INVALID_FACTOR_MESSAGE: &str = "El factor debe ser un número mayor que 0";
const INVALID_QUANTITY_MESSAGE: &str =
    "La cantidad debe ser mayor que 0 y no superar 999.999.999,999999";
const NO_PATH_MESSAGE: &str = "No existe una conversión entre esas unidades";
const CONVERSION_TOO_LARGE_MESSAGE: &str = "El resultado es demasiado grande para mostrarse";
const LONG_CHAIN_MESSAGE: &str =
    "No se pudo calcular esta conversión: la cadena de unidades es demasiado larga";
const CONVERSION_NOT_FOUND_MESSAGE: &str = "La conversión ya no existe";

#[derive(Debug, Clone, PartialEq, Eq)]
struct ConversionEdge {
    from_unit_id: i64,
    to_unit_id: i64,
    factor_micro: i64,
}

fn unit_reference(connection: &Connection, unit_id: i64) -> Result<UnitReference, AppError> {
    let row: (String, String) = connection
        .query_row(
            "SELECT name, abbreviation FROM units WHERE id = ?1",
            [unit_id],
            |row| Ok((row.get(0)?, row.get(1)?)),
        )
        .map_err(|error| map_db_error(error, "unitReference", &[]))?;

    Ok(UnitReference {
        id: unit_id,
        name: row.0,
        abbreviation: row.1,
    })
}

fn direct_pair_exists(
    connection: &Connection,
    from_unit_id: i64,
    to_unit_id: i64,
) -> Result<bool, AppError> {
    let exists: Option<i64> = connection
        .query_row(
            "SELECT id FROM unitConversions WHERE min(fromUnitId, toUnitId) = ?1 AND max(fromUnitId, toUnitId) = ?2 LIMIT 1",
            [
                from_unit_id.min(to_unit_id),
                from_unit_id.max(to_unit_id),
            ],
            |row| row.get(0),
        )
        .optional()
        .map_err(|error| map_db_error(error, "directPairExists", &[]))?;

    Ok(exists.is_some())
}

fn find_path_exists(
    connection: &Connection,
    start_unit_id: i64,
    end_unit_id: i64,
    skip_direct_pair: bool,
) -> Result<bool, AppError> {
    let mut adjacency: HashMap<i64, Vec<i64>> = HashMap::new();
    let edges: Vec<ConversionEdge> = connection
        .prepare("SELECT fromUnitId, toUnitId, factorMicro FROM unitConversions ORDER BY id ASC")
        .map_err(|error| map_db_error(error, "findPathExists", &[]))?
        .query_map([], |row| {
            Ok(ConversionEdge {
                from_unit_id: row.get(0)?,
                to_unit_id: row.get(1)?,
                factor_micro: row.get(2)?,
            })
        })
        .map_err(|error| map_db_error(error, "findPathExists", &[]))?
        .collect::<Result<Vec<_>, rusqlite::Error>>()
        .map_err(|error| map_db_error(error, "findPathExists", &[]))?;

    for edge in &edges {
        if skip_direct_pair
            && ((edge.from_unit_id == start_unit_id && edge.to_unit_id == end_unit_id)
                || (edge.from_unit_id == end_unit_id && edge.to_unit_id == start_unit_id))
        {
            continue;
        }

        adjacency
            .entry(edge.from_unit_id)
            .or_default()
            .push(edge.to_unit_id);
        adjacency
            .entry(edge.to_unit_id)
            .or_default()
            .push(edge.from_unit_id);
    }

    if start_unit_id == end_unit_id {
        return Ok(true);
    }

    let mut queue = VecDeque::from([start_unit_id]);
    let mut visited = HashSet::from([start_unit_id]);

    while let Some(current) = queue.pop_front() {
        if current == end_unit_id {
            return Ok(true);
        }

        if let Some(neighbors) = adjacency.get(&current) {
            for neighbor in neighbors {
                if visited.insert(*neighbor) {
                    queue.push_back(*neighbor);
                }
            }
        }
    }

    Ok(false)
}

fn format_conversion_factor(factor_micro: i128) -> String {
    let value =
        i64::try_from(factor_micro).expect("conversion factor is within the supported i64 range");
    format_factor_micro(value)
}

pub fn list_conversions_inner(
    connection: &mut Connection,
    now: DateTime<Utc>,
    search: Option<&str>,
) -> Result<Vec<ConversionItem>, AppError> {
    require_session(connection, now)?;

    let normalized_search = normalize_text(&clean_name(search.unwrap_or("")));
    let mut statement = connection
        .prepare(
            "SELECT c.id, c.fromUnitId, c.toUnitId, c.factorMicro,
                    fu.name AS fromName, fu.abbreviation AS fromAbbreviation,
                    tu.name AS toName, tu.abbreviation AS toAbbreviation
             FROM unitConversions c
             JOIN units fu ON fu.id = c.fromUnitId
             JOIN units tu ON tu.id = c.toUnitId
             WHERE ?1 = ''
                OR instr(fu.nameNormalized, ?1) > 0
                OR instr(fu.abbreviationNormalized, ?1) > 0
                OR instr(tu.nameNormalized, ?1) > 0
                OR instr(tu.abbreviationNormalized, ?1) > 0
             ORDER BY fu.id ASC, tu.id ASC",
        )
        .map_err(|error| map_db_error(error, "listConversions", &[]))?;

    let rows = statement
        .query_map([&normalized_search], |row| {
            Ok((
                row.get::<_, i64>(0)?,
                row.get::<_, i64>(1)?,
                row.get::<_, String>(4)?,
                row.get::<_, String>(5)?,
                row.get::<_, i64>(2)?,
                row.get::<_, String>(6)?,
                row.get::<_, String>(7)?,
                row.get::<_, i64>(3)?,
            ))
        })
        .map_err(|error| map_db_error(error, "listConversions", &[]))?
        .collect::<Result<Vec<_>, rusqlite::Error>>()
        .map_err(|error| map_db_error(error, "listConversions", &[]))?;

    let mut items = rows
        .into_iter()
        .map(
            |(
                id,
                from_id,
                from_name,
                from_abbreviation,
                to_id,
                to_name,
                to_abbreviation,
                factor_micro,
            )| {
                ConversionItem {
                    id,
                    from_unit: UnitReference {
                        id: from_id,
                        name: from_name,
                        abbreviation: from_abbreviation,
                    },
                    to_unit: UnitReference {
                        id: to_id,
                        name: to_name,
                        abbreviation: to_abbreviation,
                    },
                    factor: format_conversion_factor(factor_micro.into()),
                }
            },
        )
        .collect::<Vec<_>>();

    sort_spanish(&mut items, |item| {
        (
            spanish_sort_key(&item.from_unit.name),
            spanish_sort_key(&item.to_unit.name),
            item.id,
        )
    });

    Ok(items)
}

#[tauri::command]
pub fn list_conversions(
    state: tauri::State<'_, std::sync::Mutex<Connection>>,
    search: Option<String>,
) -> Result<Vec<ConversionItem>, AppError> {
    let mut connection = lock_conn(&state);
    let now = Utc::now();
    let result = list_conversions_inner(&mut connection, now, search.as_deref());
    if let Err(error) = &result {
        if error.should_log() {
            if let Some(app_data_dir) = crate::db::connection::current_app_data_dir() {
                log_error(
                    &connection,
                    "conversions",
                    "listConversions",
                    error,
                    now,
                    app_data_dir,
                );
            }
        }
    }
    result
}

pub fn create_conversion_inner(
    connection: &mut Connection,
    now: DateTime<Utc>,
    from_unit_id: i64,
    to_unit_id: i64,
    factor: &str,
) -> Result<ConversionItem, AppError> {
    require_session(connection, now)?;

    if from_unit_id == to_unit_id {
        return Err(AppError::new(
            "validationFailed",
            SAME_UNIT_MESSAGE,
            "conversion attempted with same unit",
        ));
    }

    let factor_micro = parse_factor_micro(factor)?;
    if factor_micro <= 0 {
        return Err(AppError::new(
            "validationFailed",
            INVALID_FACTOR_MESSAGE,
            "conversion factor must be greater than zero",
        ));
    }

    let from_exists: Option<i64> = connection
        .query_row(
            "SELECT id FROM units WHERE id = ?1",
            [from_unit_id],
            |row| row.get(0),
        )
        .optional()
        .map_err(|error| map_db_error(error, "createConversion", &[]))?;
    if from_exists.is_none() {
        return Err(AppError::new(
            "notFound",
            "La unidad de origen ya no existe",
            "missing source unit during conversion creation",
        ));
    }

    let to_exists: Option<i64> = connection
        .query_row("SELECT id FROM units WHERE id = ?1", [to_unit_id], |row| {
            row.get(0)
        })
        .optional()
        .map_err(|error| map_db_error(error, "createConversion", &[]))?;
    if to_exists.is_none() {
        return Err(AppError::new(
            "notFound",
            "La unidad de destino ya no existe",
            "missing target unit during conversion creation",
        ));
    }

    if direct_pair_exists(connection, from_unit_id, to_unit_id)? {
        return Err(AppError::new(
            "duplicateField",
            DUPLICATE_CONVERSION_MESSAGE,
            "conversion pair already exists",
        ));
    }

    if find_path_exists(connection, from_unit_id, to_unit_id, true)? {
        return Err(AppError::new(
            "validationFailed",
            SECOND_PATH_MESSAGE,
            "new conversion would create a second path between two units",
        ));
    }

    let timestamp = format_utc_timestamp(now);
    connection
        .execute(
            "INSERT INTO unitConversions (fromUnitId, toUnitId, factorMicro, createdAt, updatedAt)
             VALUES (?1, ?2, ?3, ?4, ?4)",
            params![from_unit_id, to_unit_id, factor_micro, timestamp],
        )
        .map_err(|error| {
            map_db_error(
                error,
                "createConversion",
                &[("unitConversionsPair", DUPLICATE_CONVERSION_MESSAGE)],
            )
        })?;

    let conversion_id = connection.last_insert_rowid();
    let from_unit = unit_reference(connection, from_unit_id)?;
    let to_unit = unit_reference(connection, to_unit_id)?;

    Ok(ConversionItem {
        id: conversion_id,
        from_unit,
        to_unit,
        factor: format_conversion_factor(factor_micro.into()),
    })
}

#[tauri::command]
pub fn create_conversion(
    state: tauri::State<'_, std::sync::Mutex<Connection>>,
    from_unit_id: i64,
    to_unit_id: i64,
    factor: String,
) -> Result<ConversionItem, AppError> {
    let mut connection = lock_conn(&state);
    let now = Utc::now();
    let result = create_conversion_inner(&mut connection, now, from_unit_id, to_unit_id, &factor);
    if let Err(error) = &result {
        if error.should_log() {
            if let Some(app_data_dir) = crate::db::connection::current_app_data_dir() {
                log_error(
                    &connection,
                    "conversions",
                    "createConversion",
                    error,
                    now,
                    app_data_dir,
                );
            }
        }
    }
    result
}

pub fn update_conversion_factor_inner(
    connection: &mut Connection,
    now: DateTime<Utc>,
    conversion_id: i64,
    factor: &str,
) -> Result<ConversionItem, AppError> {
    require_session(connection, now)?;

    let factor_micro = parse_factor_micro(factor)?;
    if factor_micro <= 0 {
        return Err(AppError::new(
            "validationFailed",
            INVALID_FACTOR_MESSAGE,
            "conversion factor must be greater than zero",
        ));
    }

    let existing: Option<(i64, i64, i64)> = connection
        .query_row(
            "SELECT fromUnitId, toUnitId, factorMicro FROM unitConversions WHERE id = ?1",
            [conversion_id],
            |row| Ok((row.get(0)?, row.get(1)?, row.get(2)?)),
        )
        .optional()
        .map_err(|error| map_db_error(error, "updateConversionFactor", &[]))?;
    if existing.is_none() {
        return Err(AppError::new(
            "notFound",
            CONVERSION_NOT_FOUND_MESSAGE,
            "conversion missing during update",
        ));
    }

    let timestamp = format_utc_timestamp(now);
    connection
        .execute(
            "UPDATE unitConversions SET factorMicro = ?1, updatedAt = ?2 WHERE id = ?3",
            params![factor_micro, timestamp, conversion_id],
        )
        .map_err(|error| map_db_error(error, "updateConversionFactor", &[]))?;

    let (from_unit_id, to_unit_id, _) = existing.unwrap();
    let from_unit = unit_reference(connection, from_unit_id)?;
    let to_unit = unit_reference(connection, to_unit_id)?;

    Ok(ConversionItem {
        id: conversion_id,
        from_unit,
        to_unit,
        factor: format_conversion_factor(factor_micro.into()),
    })
}

#[tauri::command]
pub fn update_conversion_factor(
    state: tauri::State<'_, std::sync::Mutex<Connection>>,
    conversion_id: i64,
    factor: String,
) -> Result<ConversionItem, AppError> {
    let mut connection = lock_conn(&state);
    let now = Utc::now();
    let result = update_conversion_factor_inner(&mut connection, now, conversion_id, &factor);
    if let Err(error) = &result {
        if error.should_log() {
            if let Some(app_data_dir) = crate::db::connection::current_app_data_dir() {
                log_error(
                    &connection,
                    "conversions",
                    "updateConversionFactor",
                    error,
                    now,
                    app_data_dir,
                );
            }
        }
    }
    result
}

pub fn delete_conversion_inner(
    connection: &mut Connection,
    now: DateTime<Utc>,
    conversion_id: i64,
) -> Result<(), AppError> {
    require_session(connection, now)?;

    let exists: Option<i64> = connection
        .query_row(
            "SELECT id FROM unitConversions WHERE id = ?1",
            [conversion_id],
            |row| row.get(0),
        )
        .optional()
        .map_err(|error| map_db_error(error, "deleteConversion", &[]))?;
    if exists.is_none() {
        return Err(AppError::new(
            "notFound",
            CONVERSION_NOT_FOUND_MESSAGE,
            "conversion missing during deletion",
        ));
    }

    connection
        .execute("DELETE FROM unitConversions WHERE id = ?1", [conversion_id])
        .map_err(|error| map_db_error(error, "deleteConversion", &[]))?;
    Ok(())
}

#[tauri::command]
pub fn delete_conversion(
    state: tauri::State<'_, std::sync::Mutex<Connection>>,
    conversion_id: i64,
) -> Result<(), AppError> {
    let mut connection = lock_conn(&state);
    let now = Utc::now();
    let result = delete_conversion_inner(&mut connection, now, conversion_id);
    if let Err(error) = &result {
        if error.should_log() {
            if let Some(app_data_dir) = crate::db::connection::current_app_data_dir() {
                log_error(
                    &connection,
                    "conversions",
                    "deleteConversion",
                    error,
                    now,
                    app_data_dir,
                );
            }
        }
    }
    result
}

fn find_conversion_path(
    connection: &Connection,
    from_unit_id: i64,
    to_unit_id: i64,
) -> Result<Vec<i64>, AppError> {
    let mut adjacency: HashMap<i64, Vec<i64>> = HashMap::new();
    let edges = connection
        .prepare("SELECT fromUnitId, toUnitId FROM unitConversions ORDER BY id ASC")
        .map_err(|error| map_db_error(error, "convertQuantity", &[]))?
        .query_map([], |row| Ok((row.get::<_, i64>(0)?, row.get::<_, i64>(1)?)))
        .map_err(|error| map_db_error(error, "convertQuantity", &[]))?
        .collect::<Result<Vec<_>, rusqlite::Error>>()
        .map_err(|error| map_db_error(error, "convertQuantity", &[]))?;

    for (current, next) in edges {
        adjacency.entry(current).or_default().push(next);
        adjacency.entry(next).or_default().push(current);
    }

    if from_unit_id == to_unit_id {
        return Ok(vec![from_unit_id]);
    }

    let mut queue = VecDeque::from([(from_unit_id, vec![from_unit_id])]);
    let mut visited = HashSet::from([from_unit_id]);

    while let Some((current, path)) = queue.pop_front() {
        if current == to_unit_id {
            return Ok(path);
        }

        if let Some(neighbors) = adjacency.get(&current) {
            for neighbor in neighbors {
                if visited.insert(*neighbor) {
                    let mut next_path = path.clone();
                    next_path.push(*neighbor);
                    queue.push_back((*neighbor, next_path));
                }
            }
        }
    }

    Ok(Vec::new())
}

pub fn convert_quantity_inner(
    connection: &mut Connection,
    now: DateTime<Utc>,
    from_unit_id: i64,
    to_unit_id: i64,
    quantity: &str,
) -> Result<ConversionResult, AppError> {
    require_session(connection, now)?;

    let from_exists: Option<i64> = connection
        .query_row(
            "SELECT id FROM units WHERE id = ?1",
            [from_unit_id],
            |row| row.get(0),
        )
        .optional()
        .map_err(|error| map_db_error(error, "convertQuantity", &[]))?;
    if from_exists.is_none() {
        return Err(AppError::new(
            "notFound",
            "La unidad ya no existe",
            "source unit missing during conversion",
        ));
    }

    let to_exists: Option<i64> = connection
        .query_row("SELECT id FROM units WHERE id = ?1", [to_unit_id], |row| {
            row.get(0)
        })
        .optional()
        .map_err(|error| map_db_error(error, "convertQuantity", &[]))?;
    if to_exists.is_none() {
        return Err(AppError::new(
            "notFound",
            "La unidad ya no existe",
            "target unit missing during conversion",
        ));
    }

    if from_unit_id == to_unit_id {
        let quantity_micro = parse_quantity_micro(quantity)?;
        return Ok(ConversionResult {
            result: format_conversion_factor(quantity_micro.into()),
        });
    }

    let path = find_conversion_path(connection, from_unit_id, to_unit_id)?;
    if path.is_empty() {
        return Err(AppError::new(
            "validationFailed",
            NO_PATH_MESSAGE,
            "no conversion path exists between the selected units",
        ));
    }

    let quantity_micro = parse_quantity_micro(quantity)?;
    if quantity_micro <= 0 {
        return Err(AppError::new(
            "validationFailed",
            INVALID_QUANTITY_MESSAGE,
            "quantity must be greater than zero",
        ));
    }

    let mut numerator = i128::from(quantity_micro);
    let mut denominator = 1_000_000_i128;

    for index in 0..path.len().saturating_sub(1) {
        let current_unit_id = path[index];
        let next_unit_id = path[index + 1];

        let row: Option<(i64, i64, i64)> = connection
            .query_row(
                "SELECT fromUnitId, toUnitId, factorMicro FROM unitConversions WHERE (fromUnitId = ?1 AND toUnitId = ?2) OR (fromUnitId = ?2 AND toUnitId = ?1)",
                [current_unit_id, next_unit_id],
                |row| Ok((row.get(0)?, row.get(1)?, row.get(2)?)),
            )
            .optional()
            .map_err(|error| map_db_error(error, "convertQuantity", &[]))?;

        let Some((from_id, to_id, factor_micro)) = row else {
            return Err(AppError::new(
                "validationFailed",
                NO_PATH_MESSAGE,
                "conversion path is no longer valid",
            ));
        };

        let step_numerator = if current_unit_id == from_id {
            numerator
                .checked_mul(i128::from(factor_micro))
                .ok_or_else(|| {
                    AppError::new(
                        "validationFailed",
                        LONG_CHAIN_MESSAGE,
                        "conversion calculation overflowed on forward step",
                    )
                })?
        } else if current_unit_id == to_id {
            numerator.checked_mul(1_000_000_i128).ok_or_else(|| {
                AppError::new(
                    "validationFailed",
                    LONG_CHAIN_MESSAGE,
                    "conversion calculation overflowed on reverse step",
                )
            })?
        } else {
            return Err(AppError::new(
                "validationFailed",
                NO_PATH_MESSAGE,
                "conversion path is inconsistent",
            ));
        };

        let step_denominator = if current_unit_id == from_id {
            denominator.checked_mul(1_000_000_i128).ok_or_else(|| {
                AppError::new(
                    "validationFailed",
                    LONG_CHAIN_MESSAGE,
                    "conversion calculation overflowed on forward denominator",
                )
            })?
        } else if current_unit_id == to_id {
            denominator
                .checked_mul(i128::from(factor_micro))
                .ok_or_else(|| {
                    AppError::new(
                        "validationFailed",
                        LONG_CHAIN_MESSAGE,
                        "conversion calculation overflowed on reverse denominator",
                    )
                })?
        } else {
            return Err(AppError::new(
                "validationFailed",
                NO_PATH_MESSAGE,
                "conversion path is inconsistent",
            ));
        };

        numerator = step_numerator;
        denominator = step_denominator;

        let gcd = gcd_u128(numerator as u128, denominator as u128);
        if gcd > 0 {
            numerator /= gcd as i128;
            denominator /= gcd as i128;
        }
    }

    if numerator > 999_999_999_999_999_i128 || denominator > 1_000_000_i128 {
        let result_micro = numerator
            .checked_mul(1_000_000_i128)
            .and_then(|value| value.checked_div(denominator))
            .ok_or_else(|| {
                AppError::new(
                    "validationFailed",
                    LONG_CHAIN_MESSAGE,
                    "conversion calculation overflowed during rounding",
                )
            })?;
        if result_micro > 999_999_999_999_999_i128 {
            return Err(AppError::new(
                "validationFailed",
                CONVERSION_TOO_LARGE_MESSAGE,
                "converted value is larger than the display limit",
            ));
        }
    }

    let result_micro = (numerator.checked_mul(1_000_000_i128).ok_or_else(|| {
        AppError::new(
            "validationFailed",
            LONG_CHAIN_MESSAGE,
            "conversion calculation overflowed during rounding",
        )
    })? + denominator / 2)
        .checked_div(denominator)
        .ok_or_else(|| {
            AppError::new(
                "validationFailed",
                LONG_CHAIN_MESSAGE,
                "conversion calculation overflowed during rounding",
            )
        })?;

    if result_micro > 999_999_999_999_999_i128 {
        return Err(AppError::new(
            "validationFailed",
            CONVERSION_TOO_LARGE_MESSAGE,
            "converted value is larger than the display limit",
        ));
    }

    Ok(ConversionResult {
        result: format_conversion_factor(result_micro),
    })
}

#[tauri::command]
pub fn convert_quantity(
    state: tauri::State<'_, std::sync::Mutex<Connection>>,
    from_unit_id: i64,
    to_unit_id: i64,
    quantity: String,
) -> Result<ConversionResult, AppError> {
    let mut connection = lock_conn(&state);
    let now = Utc::now();
    let result = convert_quantity_inner(&mut connection, now, from_unit_id, to_unit_id, &quantity);
    if let Err(error) = &result {
        if error.should_log() {
            if let Some(app_data_dir) = crate::db::connection::current_app_data_dir() {
                log_error(
                    &connection,
                    "conversions",
                    "convertQuantity",
                    error,
                    now,
                    app_data_dir,
                );
            }
        }
    }
    result
}

fn gcd_u128(a: u128, b: u128) -> u128 {
    let mut x = a;
    let mut y = b;
    while y != 0 {
        let remainder = x % y;
        x = y;
        y = remainder;
    }
    x
}

#[cfg(test)]
mod tests {
    use chrono::Utc;
    use rusqlite::Connection;

    use crate::test_utils::{
        create_test_session, create_test_user, insert_unit_fixture, setup_test_db,
    };

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
    fn list_conversions_orders_by_spanish_name_and_filters_search() {
        let mut connection = setup_test_db();
        add_session(&connection, "observer", "contador");

        let kg = insert_unit_fixture(&connection, "Kilogramo", "kg");
        let g = insert_unit_fixture(&connection, "Gramo", "g");
        let lb = insert_unit_fixture(&connection, "Libra", "lb");
        let now = Utc::now();

        super::create_conversion_inner(&mut connection, now, g, kg, "1000").unwrap();
        super::create_conversion_inner(&mut connection, now, kg, lb, "2,20462").unwrap();

        let items = super::list_conversions_inner(&mut connection, now, Some("gra")).unwrap();
        assert_eq!(items.len(), 2);
        assert_eq!(items[0].from_unit.name, "Gramo");
        assert_eq!(items[0].to_unit.name, "Kilogramo");
        assert_eq!(items[0].factor, "1000");

        let all_items = super::list_conversions_inner(&mut connection, now, None).unwrap();
        assert_eq!(all_items[0].from_unit.name, "Gramo");
        assert_eq!(all_items[1].from_unit.name, "Kilogramo");
    }

    #[test]
    fn create_and_update_conversion_respect_duplicate_and_second_path_rules() {
        let mut connection = setup_test_db();
        add_session(&connection, "adminconv", "administrador");

        let unit_a = insert_unit_fixture(&connection, "Unidad A", "UA");
        let unit_b = insert_unit_fixture(&connection, "Unidad B", "UB");
        let unit_c = insert_unit_fixture(&connection, "Unidad C", "UC");
        let now = Utc::now();

        let created =
            super::create_conversion_inner(&mut connection, now, unit_a, unit_b, "12").unwrap();
        assert_eq!(created.factor, "12");

        let duplicate_error =
            super::create_conversion_inner(&mut connection, now, unit_b, unit_a, "10").unwrap_err();
        assert_eq!(duplicate_error.code, "duplicateField");

        super::create_conversion_inner(&mut connection, now, unit_b, unit_c, "2").unwrap();
        let second_path_error =
            super::create_conversion_inner(&mut connection, now, unit_a, unit_c, "24").unwrap_err();
        assert_eq!(second_path_error.code, "validationFailed");
        assert_eq!(second_path_error.message, super::SECOND_PATH_MESSAGE);

        let updated =
            super::update_conversion_factor_inner(&mut connection, now, created.id, "13,5")
                .unwrap();
        assert_eq!(updated.factor, "13,5");
    }

    #[test]
    fn delete_conversion_and_convert_quantity_work_for_valid_paths() {
        let mut connection = setup_test_db();
        add_session(&connection, "converter", "contador");

        let from = insert_unit_fixture(&connection, "Metro", "m");
        let to = insert_unit_fixture(&connection, "Centímetro", "cm");
        let now = Utc::now();

        let conversion =
            super::create_conversion_inner(&mut connection, now, from, to, "100").unwrap();
        let converted =
            super::convert_quantity_inner(&mut connection, now, from, to, "2,5").unwrap();
        assert_eq!(converted.result, "250");

        super::delete_conversion_inner(&mut connection, now, conversion.id).unwrap();
        let missing =
            super::delete_conversion_inner(&mut connection, now, conversion.id).unwrap_err();
        assert_eq!(missing.code, "notFound");
    }

    #[test]
    fn convert_quantity_works_in_both_directions() {
        let mut connection = setup_test_db();
        add_session(&connection, "bidirectional", "contador");

        let kg = insert_unit_fixture(&connection, "Kilogramo", "kg");
        let g = insert_unit_fixture(&connection, "Gramo", "g");
        let now = Utc::now();

        super::create_conversion_inner(&mut connection, now, kg, g, "1000").unwrap();

        let forward = super::convert_quantity_inner(&mut connection, now, kg, g, "5").unwrap();
        assert_eq!(forward.result, "5000");

        let reverse = super::convert_quantity_inner(&mut connection, now, g, kg, "2000").unwrap();
        assert_eq!(reverse.result, "2");
    }

    #[test]
    fn convert_quantity_handles_multi_step_paths_without_overflow() {
        let mut connection = setup_test_db();
        add_session(&connection, "multistep", "contador");

        let ton = insert_unit_fixture(&connection, "Tonelada", "t");
        let kg = insert_unit_fixture(&connection, "Kilogramo", "kg");
        let g = insert_unit_fixture(&connection, "Gramo", "g");
        let mg = insert_unit_fixture(&connection, "Miligramo", "mg");
        let now = Utc::now();

        super::create_conversion_inner(&mut connection, now, ton, kg, "1000").unwrap();
        super::create_conversion_inner(&mut connection, now, kg, g, "1000").unwrap();
        super::create_conversion_inner(&mut connection, now, g, mg, "1000").unwrap();

        let result = super::convert_quantity_inner(&mut connection, now, ton, mg, "0,5").unwrap();
        assert_eq!(result.result, "500000000");
    }

    #[test]
    fn convert_quantity_same_unit_returns_quantity_without_trailing_zeros() {
        let mut connection = setup_test_db();
        add_session(&connection, "sameunit", "contador");

        let kg = insert_unit_fixture(&connection, "Kilogramo", "kg");
        let now = Utc::now();

        let result = super::convert_quantity_inner(&mut connection, now, kg, kg, "10,5").unwrap();
        assert_eq!(result.result, "10,5");

        let result_int = super::convert_quantity_inner(&mut connection, now, kg, kg, "5").unwrap();
        assert_eq!(result_int.result, "5");
    }

    #[test]
    fn convert_quantity_without_path_returns_validation_error() {
        let mut connection = setup_test_db();
        add_session(&connection, "nopath", "contador");

        let kg = insert_unit_fixture(&connection, "Kilogramo", "kg");
        let m = insert_unit_fixture(&connection, "Metro", "m");
        let now = Utc::now();

        let error = super::convert_quantity_inner(&mut connection, now, kg, m, "5").unwrap_err();
        assert_eq!(error.code, "validationFailed");
        assert_eq!(error.message, super::NO_PATH_MESSAGE);
    }

    #[test]
    fn convert_quantity_with_nonexistent_unit_returns_not_found() {
        let mut connection = setup_test_db();
        add_session(&connection, "missingunit", "contador");

        let kg = insert_unit_fixture(&connection, "Kilogramo", "kg");
        let fake_id = 99999;
        let now = Utc::now();

        let error =
            super::convert_quantity_inner(&mut connection, now, kg, fake_id, "5").unwrap_err();
        assert_eq!(error.code, "notFound");
        assert_eq!(error.message, "La unidad ya no existe");
    }

    #[test]
    fn convert_quantity_rounds_half_up_to_6_decimals() {
        let mut connection = setup_test_db();
        add_session(&connection, "rounding", "contador");

        let kg = insert_unit_fixture(&connection, "Kilogramo", "kg");
        let g = insert_unit_fixture(&connection, "Gramo", "g");
        let now = Utc::now();

        super::create_conversion_inner(&mut connection, now, kg, g, "1000").unwrap();

        let result = super::convert_quantity_inner(&mut connection, now, kg, g, "0,0015").unwrap();
        assert_eq!(result.result, "1,5");

        let result_down =
            super::convert_quantity_inner(&mut connection, now, kg, g, "0,0014").unwrap();
        assert_eq!(result_down.result, "1,4");
    }

    #[test]
    fn convert_quantity_removes_trailing_zeros_from_result() {
        let mut connection = setup_test_db();
        add_session(&connection, "trailing", "contador");

        let kg = insert_unit_fixture(&connection, "Kilogramo", "kg");
        let g = insert_unit_fixture(&connection, "Gramo", "g");
        let now = Utc::now();

        super::create_conversion_inner(&mut connection, now, kg, g, "1000").unwrap();

        let result = super::convert_quantity_inner(&mut connection, now, kg, g, "1").unwrap();
        assert_eq!(result.result, "1000");
    }

    #[test]
    fn convert_quantity_returns_zero_if_rounding_annihilates_value() {
        let mut connection = setup_test_db();
        add_session(&connection, "annihilate", "contador");

        let kg = insert_unit_fixture(&connection, "Kilogramo", "kg");
        let g = insert_unit_fixture(&connection, "Gramo", "g");
        let now = Utc::now();

        super::create_conversion_inner(&mut connection, now, kg, g, "1000").unwrap();

        let result =
            super::convert_quantity_inner(&mut connection, now, kg, g, "0,000001").unwrap();
        assert_eq!(result.result, "0,001");
    }

    #[test]
    fn convert_quantity_rejects_result_larger_than_limit() {
        let mut connection = setup_test_db();
        add_session(&connection, "toobig", "contador");

        let kg = insert_unit_fixture(&connection, "Kilogramo", "kg");
        let g = insert_unit_fixture(&connection, "Gramo", "g");
        let now = Utc::now();

        super::create_conversion_inner(&mut connection, now, kg, g, "1000").unwrap();

        let error = super::convert_quantity_inner(&mut connection, now, kg, g, "999999999,999999")
            .unwrap_err();
        assert_eq!(error.code, "validationFailed");
        assert_eq!(error.message, super::CONVERSION_TOO_LARGE_MESSAGE);
    }

    #[test]
    fn convert_quantity_rejects_calculation_overflow_on_long_chain() {
        let mut connection = setup_test_db();
        add_session(&connection, "overflow", "contador");

        let unit_a = insert_unit_fixture(&connection, "Unidad A", "UA");
        let unit_b = insert_unit_fixture(&connection, "Unidad B", "UB");
        let unit_c = insert_unit_fixture(&connection, "Unidad C", "UC");
        let unit_d = insert_unit_fixture(&connection, "Unidad D", "UD");
        let unit_e = insert_unit_fixture(&connection, "Unidad E", "UE");
        let now = Utc::now();

        super::create_conversion_inner(&mut connection, now, unit_a, unit_b, "1000000").unwrap();
        super::create_conversion_inner(&mut connection, now, unit_b, unit_c, "1000000").unwrap();
        super::create_conversion_inner(&mut connection, now, unit_c, unit_d, "1000000").unwrap();
        super::create_conversion_inner(&mut connection, now, unit_d, unit_e, "1000000").unwrap();

        let error = super::convert_quantity_inner(&mut connection, now, unit_a, unit_e, "100000")
            .unwrap_err();
        assert_eq!(error.code, "validationFailed");
        // Puede ser LONG_CHAIN_MESSAGE o CONVERSION_TOO_LARGE_MESSAGE dependiendo de si
        // el desborde ocurre antes o después de la reducción por MCD
        assert!(
            error.message == super::LONG_CHAIN_MESSAGE
                || error.message == super::CONVERSION_TOO_LARGE_MESSAGE
        );
    }

    #[test]
    fn convert_quantity_requires_session() {
        let mut connection = setup_test_db();

        let kg = insert_unit_fixture(&connection, "Kilogramo", "kg");
        let g = insert_unit_fixture(&connection, "Gramo", "g");
        let now = Utc::now();

        let error = super::convert_quantity_inner(&mut connection, now, kg, g, "5").unwrap_err();
        assert_eq!(error.code, "sessionExpired");
    }
}
