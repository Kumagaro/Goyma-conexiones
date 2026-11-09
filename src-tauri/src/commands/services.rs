//! Catálogo de servicios (`spec-06` SV-01): nombre, descripción, precio
//! sugerido y productos con su cantidad sugerida. Las facturas guardan su
//! propia copia, así que editar o eliminar un servicio no las cambia (S-13).

use chrono::{DateTime, Utc};
use rusqlite::{params, Connection, OptionalExtension};

use crate::commands::auth::require_session;
use crate::db::connection::lock_conn;
use crate::errors::{log_error, map_db_error, AppError};
use crate::models::product::format_quantity_milli;
use crate::models::service::{ServiceDetail, ServiceInput, ServiceListItem, ServiceProductDetail};
use crate::text::{clean_name, normalize_text, sort_spanish, spanish_sort_key};
use crate::timestamps::format_utc_timestamp;
use crate::validators::{
    parse_quantity_milli, parse_service_price_cents, validate_name_with_messages,
    validate_optional_description,
};

const SERVICE_DUPLICATE_MESSAGE: &str = "Ya existe un servicio con ese nombre";
const SERVICE_MISSING_MESSAGE: &str = "El servicio ya no existe";

fn validation_failed(message: &str, cause: &str) -> AppError {
    AppError::new("validationFailed", message, cause)
}

/// Nombre, descripción y precio validados de un servicio.
struct ValidService {
    name: String,
    name_normalized: String,
    description: Option<String>,
    suggested_price_cents: Option<i64>,
    products: Vec<(i64, i64)>,
}

fn validate_service(
    connection: &Connection,
    input: &ServiceInput,
    previous_products: &[i64],
) -> Result<ValidService, AppError> {
    let (name, name_normalized) = validate_name_with_messages(
        &input.name,
        2,
        100,
        "El nombre debe tener al menos 2 caracteres",
        "El nombre no puede tener más de 100 caracteres",
        "El nombre contiene caracteres no permitidos",
    )?;
    let description = validate_optional_description(
        input.description.as_deref(),
        500,
        "La descripción no puede tener más de 500 caracteres",
        "La descripción contiene caracteres no permitidos",
    )?;
    let suggested_price_cents = input
        .suggested_price
        .as_deref()
        .map(str::trim)
        .filter(|raw| !raw.is_empty())
        .map(|raw| {
            parse_service_price_cents(raw).map_err(|_| {
                validation_failed(
                    "El precio sugerido no es válido",
                    "invalid service suggested price",
                )
            })
        })
        .transpose()?;
    if input.products.len() > 100 {
        return Err(validation_failed(
            "Un servicio admite hasta 100 productos",
            "too many service products",
        ));
    }
    let mut products = Vec::with_capacity(input.products.len());
    for product in &input.products {
        if products.iter().any(|(id, _)| *id == product.product_id) {
            return Err(validation_failed(
                "El producto ya está en el servicio",
                "duplicate service product",
            ));
        }
        let quantity_milli = parse_quantity_milli(&product.quantity).map_err(|_| {
            validation_failed(
                "La cantidad del producto no es válida",
                "invalid service product quantity",
            )
        })?;
        let row: Option<(String, String)> = connection
            .query_row(
                "SELECT name, status FROM products WHERE id = ?1",
                [product.product_id],
                |row| Ok((row.get(0)?, row.get(1)?)),
            )
            .optional()
            .map_err(|error| map_db_error(error, "validateService", &[]))?;
        let Some((product_name, status)) = row else {
            return Err(AppError::new(
                "notFound",
                "El producto ya no existe",
                "service product missing",
            ));
        };
        // Solo se añaden productos activos; uno que ya estaba y se
        // descontinuó después se conserva marcado (SV-01).
        if status == "discontinued" && !previous_products.contains(&product.product_id) {
            return Err(validation_failed(
                &format!("El producto «{product_name}» está descontinuado"),
                "discontinued product added to service",
            ));
        }
        products.push((product.product_id, quantity_milli));
    }
    Ok(ValidService {
        name,
        name_normalized,
        description,
        suggested_price_cents,
        products,
    })
}

fn insert_service_products(
    connection: &Connection,
    service_id: i64,
    products: &[(i64, i64)],
    op: &str,
) -> Result<(), AppError> {
    for (index, (product_id, quantity_milli)) in products.iter().enumerate() {
        connection
            .execute(
                "INSERT INTO serviceProducts (serviceId, productId, suggestedQuantityMilli, position)
                 VALUES (?1, ?2, ?3, ?4)",
                params![service_id, product_id, quantity_milli, index as i64 + 1],
            )
            .map_err(|error| map_db_error(error, op, &[]))?;
    }
    Ok(())
}

fn read_service(connection: &Connection, service_id: i64) -> Result<ServiceDetail, AppError> {
    let header: Option<(String, Option<String>, Option<i64>)> = connection
        .query_row(
            "SELECT name, description, suggestedPriceCents FROM services WHERE id = ?1",
            [service_id],
            |row| Ok((row.get(0)?, row.get(1)?, row.get(2)?)),
        )
        .optional()
        .map_err(|error| map_db_error(error, "getService", &[]))?;
    let Some((name, description, suggested_price_cents)) = header else {
        return Err(AppError::new(
            "notFound",
            SERVICE_MISSING_MESSAGE,
            "service missing",
        ));
    };
    let products = connection
        .prepare(
            "SELECT p.id, p.name, u.abbreviation, sp.suggestedQuantityMilli, p.status,
                    p.costCents, p.profitPercentHundredths
             FROM serviceProducts sp
             JOIN products p ON p.id = sp.productId
             LEFT JOIN units u ON u.id = p.unitId
             WHERE sp.serviceId = ?1
             ORDER BY sp.position",
        )
        .and_then(|mut statement| {
            statement
                .query_map([service_id], |row| {
                    Ok(ServiceProductDetail {
                        product_id: row.get(0)?,
                        name: row.get(1)?,
                        unit_abbreviation: row.get(2)?,
                        suggested_quantity: format_quantity_milli(row.get(3)?),
                        status: row.get(4)?,
                        selling_price_cents: crate::pricing::selling_price_cents(
                            row.get(5)?,
                            row.get(6)?,
                        ),
                    })
                })?
                .collect::<rusqlite::Result<Vec<_>>>()
        })
        .map_err(|error| map_db_error(error, "getService", &[]))?;
    Ok(ServiceDetail {
        id: service_id,
        name,
        description,
        suggested_price_cents,
        products,
    })
}

pub fn list_services_inner(
    connection: &mut Connection,
    now: DateTime<Utc>,
    search: Option<&str>,
) -> Result<Vec<ServiceListItem>, AppError> {
    require_session(connection, now)?;
    let normalized_search = normalize_text(&clean_name(search.unwrap_or("")));
    let mut items = connection
        .prepare(
            "SELECT s.id, s.name, s.description, s.suggestedPriceCents,
                    (SELECT COUNT(*) FROM serviceProducts sp WHERE sp.serviceId = s.id)
             FROM services s
             WHERE ?1 = '' OR instr(s.nameNormalized, ?1) > 0",
        )
        .and_then(|mut statement| {
            statement
                .query_map([&normalized_search], |row| {
                    Ok(ServiceListItem {
                        id: row.get(0)?,
                        name: row.get(1)?,
                        description: row.get(2)?,
                        suggested_price_cents: row.get(3)?,
                        product_count: row.get(4)?,
                    })
                })?
                .collect::<rusqlite::Result<Vec<_>>>()
        })
        .map_err(|error| map_db_error(error, "listServices", &[]))?;
    sort_spanish(&mut items, |item| (spanish_sort_key(&item.name), item.id));
    Ok(items)
}

pub fn get_service_inner(
    connection: &mut Connection,
    now: DateTime<Utc>,
    service_id: i64,
) -> Result<ServiceDetail, AppError> {
    require_session(connection, now)?;
    read_service(connection, service_id)
}

pub fn create_service_inner(
    connection: &mut Connection,
    now: DateTime<Utc>,
    input: ServiceInput,
) -> Result<ServiceDetail, AppError> {
    require_session(connection, now)?;
    let transaction = connection
        .transaction()
        .map_err(|error| map_db_error(error, "createService", &[]))?;
    let service = validate_service(&transaction, &input, &[])?;
    let timestamp = format_utc_timestamp(now);
    transaction
        .execute(
            "INSERT INTO services (name, nameNormalized, description, suggestedPriceCents,
                createdAt, updatedAt)
             VALUES (?1, ?2, ?3, ?4, ?5, ?5)",
            params![
                service.name,
                service.name_normalized,
                service.description,
                service.suggested_price_cents,
                timestamp
            ],
        )
        .map_err(|error| {
            map_db_error(
                error,
                "createService",
                &[("services.nameNormalized", SERVICE_DUPLICATE_MESSAGE)],
            )
        })?;
    let service_id = transaction.last_insert_rowid();
    insert_service_products(&transaction, service_id, &service.products, "createService")?;
    let detail = read_service(&transaction, service_id)?;
    transaction
        .commit()
        .map_err(|error| map_db_error(error, "createService", &[]))?;
    Ok(detail)
}

pub fn update_service_inner(
    connection: &mut Connection,
    now: DateTime<Utc>,
    service_id: i64,
    input: ServiceInput,
) -> Result<ServiceDetail, AppError> {
    require_session(connection, now)?;
    let transaction = connection
        .transaction()
        .map_err(|error| map_db_error(error, "updateService", &[]))?;
    let previous = read_service(&transaction, service_id)?;
    let previous_products: Vec<i64> = previous
        .products
        .iter()
        .map(|product| product.product_id)
        .collect();
    let service = validate_service(&transaction, &input, &previous_products)?;
    transaction
        .execute(
            "UPDATE services SET name = ?1, nameNormalized = ?2, description = ?3,
                suggestedPriceCents = ?4, updatedAt = ?5
             WHERE id = ?6",
            params![
                service.name,
                service.name_normalized,
                service.description,
                service.suggested_price_cents,
                format_utc_timestamp(now),
                service_id
            ],
        )
        .map_err(|error| {
            map_db_error(
                error,
                "updateService",
                &[("services.nameNormalized", SERVICE_DUPLICATE_MESSAGE)],
            )
        })?;
    transaction
        .execute(
            "DELETE FROM serviceProducts WHERE serviceId = ?1",
            [service_id],
        )
        .map_err(|error| map_db_error(error, "updateService", &[]))?;
    insert_service_products(&transaction, service_id, &service.products, "updateService")?;
    let detail = read_service(&transaction, service_id)?;
    transaction
        .commit()
        .map_err(|error| map_db_error(error, "updateService", &[]))?;
    Ok(detail)
}

/// Eliminar no toca las facturas: su `serviceId` queda vacío y conservan
/// nombre y precio copiados (S-13).
pub fn delete_service_inner(
    connection: &mut Connection,
    now: DateTime<Utc>,
    service_id: i64,
) -> Result<(), AppError> {
    require_session(connection, now)?;
    let deleted = connection
        .execute("DELETE FROM services WHERE id = ?1", [service_id])
        .map_err(|error| map_db_error(error, "deleteService", &[]))?;
    if deleted == 0 {
        return Err(AppError::new(
            "notFound",
            SERVICE_MISSING_MESSAGE,
            "service missing during deletion",
        ));
    }
    Ok(())
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
                log_error(connection, "services", operation, error, now, app_data_dir);
            }
        }
    }
}

#[tauri::command]
pub fn list_services(
    state: tauri::State<'_, std::sync::Mutex<Connection>>,
    search: Option<String>,
) -> Result<Vec<ServiceListItem>, AppError> {
    let mut connection = lock_conn(&state);
    let now = Utc::now();
    let result = list_services_inner(&mut connection, now, search.as_deref());
    log_command_error(&connection, "listServices", &result, now);
    result
}

#[tauri::command]
pub fn get_service(
    state: tauri::State<'_, std::sync::Mutex<Connection>>,
    service_id: i64,
) -> Result<ServiceDetail, AppError> {
    let mut connection = lock_conn(&state);
    let now = Utc::now();
    let result = get_service_inner(&mut connection, now, service_id);
    log_command_error(&connection, "getService", &result, now);
    result
}

#[tauri::command]
pub fn create_service(
    state: tauri::State<'_, std::sync::Mutex<Connection>>,
    input: ServiceInput,
) -> Result<ServiceDetail, AppError> {
    let mut connection = lock_conn(&state);
    let now = Utc::now();
    let result = create_service_inner(&mut connection, now, input);
    log_command_error(&connection, "createService", &result, now);
    result
}

#[tauri::command]
pub fn update_service(
    state: tauri::State<'_, std::sync::Mutex<Connection>>,
    service_id: i64,
    input: ServiceInput,
) -> Result<ServiceDetail, AppError> {
    let mut connection = lock_conn(&state);
    let now = Utc::now();
    let result = update_service_inner(&mut connection, now, service_id, input);
    log_command_error(&connection, "updateService", &result, now);
    result
}

#[tauri::command]
pub fn delete_service(
    state: tauri::State<'_, std::sync::Mutex<Connection>>,
    service_id: i64,
) -> Result<(), AppError> {
    let mut connection = lock_conn(&state);
    let now = Utc::now();
    let result = delete_service_inner(&mut connection, now, service_id);
    log_command_error(&connection, "deleteService", &result, now);
    result
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::models::service::ServiceProductInput;
    use crate::test_utils::{
        assert_allows_both_roles, assert_requires_session, create_test_session, create_test_user,
        insert_product_fixture, setup_test_db,
    };

    fn fixture() -> (Connection, DateTime<Utc>) {
        let now = Utc::now();
        let connection = setup_test_db();
        let user_id = create_test_user(
            &connection,
            "serviceuser",
            "contador",
            "+5350000020",
            "12345678920",
            "ValidPass1!",
        );
        create_test_session(&connection, user_id, false, now, now);
        (connection, now)
    }

    fn product(connection: &Connection, name: &str, status: &str) -> i64 {
        let discontinued = (status == "discontinued").then_some("2026-10-03T00:00:00Z");
        insert_product_fixture(
            connection,
            name,
            None,
            None,
            1_000,
            5_000,
            10_000,
            status,
            discontinued,
            None,
        )
    }

    fn input(name: &str, price: Option<&str>, products: &[(i64, &str)]) -> ServiceInput {
        ServiceInput {
            name: name.to_owned(),
            description: None,
            suggested_price: price.map(str::to_owned),
            products: products
                .iter()
                .map(|(product_id, quantity)| ServiceProductInput {
                    product_id: *product_id,
                    quantity: (*quantity).to_owned(),
                })
                .collect(),
        }
    }

    #[test]
    fn services_are_created_listed_edited_and_deleted() {
        let (mut connection, now) = fixture();
        let panel = product(&connection, "Panel", "active");
        let cable = product(&connection, "Cable", "active");

        // Sin productos, como un mantenimiento.
        let maintenance = create_service_inner(
            &mut connection,
            now,
            input("Mantenimiento de paneles", None, &[]),
        )
        .unwrap();
        assert!(maintenance.products.is_empty());
        assert_eq!(maintenance.suggested_price_cents, None);

        let installation = create_service_inner(
            &mut connection,
            now,
            input(
                "Instalación",
                Some("50,00"),
                &[(panel, "2"), (cable, "10,5")],
            ),
        )
        .unwrap();
        assert_eq!(installation.suggested_price_cents, Some(5_000));
        assert_eq!(
            installation
                .products
                .iter()
                .map(|item| (
                    item.name.as_str(),
                    item.suggested_quantity.as_str(),
                    item.selling_price_cents
                ))
                .collect::<Vec<_>>(),
            vec![("Panel", "2", 1_500), ("Cable", "10,5", 1_500)]
        );

        let error = create_service_inner(&mut connection, now, input("INSTALACIÓN", None, &[]))
            .unwrap_err();
        assert_eq!(error.message, SERVICE_DUPLICATE_MESSAGE);

        let listed = list_services_inner(&mut connection, now, Some("insta")).unwrap();
        assert_eq!(listed.len(), 1);
        assert_eq!(listed[0].product_count, 2);

        let edited = update_service_inner(
            &mut connection,
            now,
            installation.id,
            input("Instalación solar", Some("0"), &[(cable, "3")]),
        )
        .unwrap();
        assert_eq!(edited.name, "Instalación solar");
        assert_eq!(edited.suggested_price_cents, Some(0));
        assert_eq!(edited.products.len(), 1);

        delete_service_inner(&mut connection, now, installation.id).unwrap();
        assert_eq!(
            get_service_inner(&mut connection, now, installation.id)
                .unwrap_err()
                .code,
            "notFound"
        );
        assert_eq!(
            list_services_inner(&mut connection, now, None)
                .unwrap()
                .len(),
            1
        );
    }

    #[test]
    fn service_products_are_validated() {
        let (mut connection, now) = fixture();
        let panel = product(&connection, "Panel", "active");
        let old = product(&connection, "Viejo", "discontinued");

        let error = create_service_inner(
            &mut connection,
            now,
            input("Instalación", None, &[(panel, "1"), (panel, "2")]),
        )
        .unwrap_err();
        assert_eq!(error.message, "El producto ya está en el servicio");
        let error = create_service_inner(
            &mut connection,
            now,
            input("Instalación", None, &[(panel, "0")]),
        )
        .unwrap_err();
        assert_eq!(error.message, "La cantidad del producto no es válida");
        let error = create_service_inner(&mut connection, now, input("I", None, &[])).unwrap_err();
        assert_eq!(error.code, "validationFailed");
        let error = create_service_inner(
            &mut connection,
            now,
            input("Instalación", None, &[(old, "1")]),
        )
        .unwrap_err();
        assert_eq!(error.message, "El producto «Viejo» está descontinuado");

        // Un producto que se descontinúa después queda marcado en la ficha
        // y se puede conservar al editar.
        let service = create_service_inner(
            &mut connection,
            now,
            input("Instalación", None, &[(panel, "1")]),
        )
        .unwrap();
        connection
            .execute(
                "UPDATE products SET status = 'discontinued', discontinuedAt = '2026-10-03T00:00:00Z' WHERE id = ?1",
                [panel],
            )
            .unwrap();
        let detail = get_service_inner(&mut connection, now, service.id).unwrap();
        assert_eq!(detail.products[0].status, "discontinued");
        update_service_inner(
            &mut connection,
            now,
            service.id,
            input("Instalación", None, &[(panel, "2")]),
        )
        .unwrap();
    }

    #[test]
    fn service_commands_require_session_and_allow_both_roles() {
        // Filas de la matriz de autorización (`spec-06` §4.1).
        assert_requires_session(|connection, now| list_services_inner(connection, now, None));
        assert_allows_both_roles(|connection, now| list_services_inner(connection, now, None));
        assert_requires_session(|connection, now| get_service_inner(connection, now, 1));
        assert_allows_both_roles(|connection, now| get_service_inner(connection, now, 1));
        assert_requires_session(|connection, now| {
            create_service_inner(connection, now, input("Instalación", None, &[]))
        });
        assert_allows_both_roles(|connection, now| {
            create_service_inner(connection, now, input("Instalación", None, &[]))
        });
        assert_requires_session(|connection, now| {
            update_service_inner(connection, now, 1, input("Instalación", None, &[]))
        });
        assert_allows_both_roles(|connection, now| {
            update_service_inner(connection, now, 1, input("Instalación", None, &[]))
        });
        assert_requires_session(|connection, now| delete_service_inner(connection, now, 1));
        assert_allows_both_roles(|connection, now| delete_service_inner(connection, now, 1));
    }
}
