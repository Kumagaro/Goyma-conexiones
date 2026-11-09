use chrono::{DateTime, Utc};
use rusqlite::{params, Connection, OptionalExtension};

use crate::errors::AppError;
use crate::models::consumption::Consumption;
use crate::models::product::format_quantity_milli;
use crate::timestamps::format_utc_timestamp;

const STOCK_LIMIT_MILLI: i64 = 999_999_999;

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Restoration {
    pub product_id: Option<i64>,
    pub lot_id: Option<i64>,
    pub quantity_milli: i64,
}

pub fn next_arrival_order(connection: &Connection, product_id: i64) -> Result<i64, AppError> {
    let current_max: i64 = connection
        .query_row(
            "SELECT COALESCE(MAX(arrivalOrder), 0) FROM lots WHERE productId = ?1",
            [product_id],
            |row| row.get(0),
        )
        .map_err(|_| internal_error("failed to read lot arrival order"))?;
    let last_lot_id: i64 = connection
        .query_row(
            "SELECT COALESCE((SELECT seq FROM sqlite_sequence WHERE name = 'lots'), 0)",
            [],
            |row| row.get(0),
        )
        .map_err(|_| internal_error("failed to read lot sequence"))?;
    Ok(current_max.max(last_lot_id).saturating_add(1))
}

pub fn next_restitution_order(connection: &Connection, product_id: i64) -> Result<i64, AppError> {
    let current_min: i64 = connection
        .query_row(
            "SELECT COALESCE(MIN(arrivalOrder), 1) FROM lots WHERE productId = ?1",
            [product_id],
            |row| row.get(0),
        )
        .map_err(|_| internal_error("failed to read restitution order"))?;
    current_min
        .checked_sub(1)
        .ok_or_else(|| internal_error("restitution order overflow"))
}

pub fn line_total_cost_cents(consumptions: &[Consumption]) -> i64 {
    let weighted_total: i128 = consumptions
        .iter()
        .map(|item| i128::from(item.quantity_milli) * i128::from(item.unit_cost_cents))
        .sum();
    round_half_up(weighted_total, 1_000) as i64
}

pub fn line_unit_cost_cents(line_total_cost_cents: i64, line_quantity_milli: i64) -> i64 {
    round_half_up(
        i128::from(line_total_cost_cents) * 1_000,
        i128::from(line_quantity_milli),
    ) as i64
}

pub(crate) fn round_half_up(numerator: i128, denominator: i128) -> i128 {
    assert!(denominator > 0, "el divisor debe ser positivo");
    if numerator < 0 {
        -((-numerator + denominator / 2) / denominator)
    } else {
        (numerator + denominator / 2) / denominator
    }
}

pub fn consume_fifo(
    connection: &Connection,
    product_id: i64,
    quantity_milli: i64,
    invoice_line_id: i64,
    now: DateTime<Utc>,
) -> Result<Vec<Consumption>, AppError> {
    if quantity_milli <= 0 {
        return Err(AppError::new(
            "validationFailed",
            "La cantidad debe ser mayor que cero",
            "fifo consumption quantity is not positive",
        ));
    }
    let product_name: Option<String> = connection
        .query_row(
            "SELECT name FROM products WHERE id = ?1",
            [product_id],
            |row| row.get(0),
        )
        .optional()
        .map_err(|_| internal_error("failed to read product for fifo consumption"))?;
    let product_name = product_name.ok_or_else(|| {
        AppError::new(
            "notFound",
            "El producto ya no existe",
            "fifo product missing",
        )
    })?;
    let mut statement = connection
        .prepare("SELECT id, arrivalOrder, remainingMilli, unitCostCents FROM lots WHERE productId = ?1 AND remainingMilli > 0 ORDER BY arrivalOrder, id")
        .map_err(|_| internal_error("failed to query fifo lots"))?;
    let lots = statement
        .query_map([product_id], |row| {
            Ok((
                row.get::<_, i64>(0)?,
                row.get::<_, i64>(1)?,
                row.get::<_, i64>(2)?,
                row.get::<_, i64>(3)?,
            ))
        })
        .map_err(|_| internal_error("failed to read fifo lots"))?
        .collect::<rusqlite::Result<Vec<_>>>()
        .map_err(|_| internal_error("failed to collect fifo lots"))?;
    drop(statement);
    let available: i64 = lots.iter().map(|lot| lot.2).sum();
    if available < quantity_milli {
        return Err(AppError::new(
            "stockInsuficiente",
            &format!(
                "No hay stock suficiente de «{product_name}» (disponible: {})",
                format_quantity_milli(available)
            ),
            "fifo stock is insufficient",
        ));
    }

    let timestamp = format_utc_timestamp(now);
    let mut remaining_to_consume = quantity_milli;
    let mut consumed = Vec::new();
    for (lot_id, _, lot_remaining, unit_cost_cents) in lots {
        if remaining_to_consume == 0 {
            break;
        }
        let quantity_from_lot = lot_remaining.min(remaining_to_consume);
        connection
            .execute(
                "UPDATE lots SET remainingMilli = ?1, updatedAt = ?2 WHERE id = ?3",
                (lot_remaining - quantity_from_lot, &timestamp, lot_id),
            )
            .map_err(|_| internal_error("failed to decrement fifo lot"))?;
        connection.execute(
            "INSERT INTO consumptions (invoiceLineId, lotId, quantityMilli, unitCostCents, createdAt) VALUES (?1, ?2, ?3, ?4, ?5)",
            (invoice_line_id, lot_id, quantity_from_lot, unit_cost_cents, &timestamp),
        ).map_err(|_| internal_error("failed to persist fifo consumption"))?;
        consumed.push(Consumption {
            id: connection.last_insert_rowid(),
            invoice_line_id,
            lot_id: Some(lot_id),
            quantity_milli: quantity_from_lot,
            unit_cost_cents,
            created_at: timestamp.clone(),
        });
        remaining_to_consume -= quantity_from_lot;
    }
    Ok(consumed)
}

/// Costo promedio móvil (`spec-06` CO-01): parte del costo base y mezcla cada
/// entrada vigente posterior con el stock que había justo antes de ella. Solo
/// se llama desde las entradas: vender, perder o cancelar salidas no lo mueve.
pub fn recalculate_product_cost(connection: &Connection, product_id: i64) -> Result<(), AppError> {
    replay_product_cost(connection, product_id, None)
}

/// Igual que `recalculate_product_cost`, sin contar la entrada `excluded_invoice`
/// (la que se está anulando o eliminando en esta misma transacción).
fn replay_product_cost(
    connection: &Connection,
    product_id: i64,
    excluded_invoice: Option<i64>,
) -> Result<(), AppError> {
    let product: Option<(Option<i64>, i64, i64)> = connection
        .query_row(
            "SELECT costBaseCents, costBaseArrivalOrder, costCents FROM products WHERE id = ?1",
            [product_id],
            |row| Ok((row.get(0)?, row.get(1)?, row.get(2)?)),
        )
        .optional()
        .map_err(|_| internal_error("failed to read product cost"))?;
    let (base_cents, base_order, current_cost) = product.ok_or_else(|| {
        AppError::new(
            "notFound",
            "El producto ya no existe",
            "product missing during cost recalculation",
        )
    })?;
    // Sin costo base guardado (producto creado después de la migración 9), el
    // punto de partida es el costo de su lote inicial.
    let initial_cost: Option<i64> = match base_cents {
        Some(_) => None,
        None => connection
            .query_row(
                "SELECT unitCostCents FROM lots WHERE productId = ?1 AND origin = 'initial'",
                [product_id],
                |row| row.get(0),
            )
            .optional()
            .map_err(|_| internal_error("failed to read initial lot cost"))?,
    };
    let mut cost = base_cents.or(initial_cost).unwrap_or(current_cost);

    let mut statement = connection
        .prepare(
            "SELECT COALESCE(l.stockBeforeMilli, 0), l.initialMilli, l.unitCostCents
             FROM lots l
             JOIN invoiceLines il ON il.id = l.invoiceLineId
             JOIN invoices i ON i.id = il.invoiceId
             WHERE l.productId = ?1 AND l.origin = 'entry' AND l.arrivalOrder > ?2
               AND i.status <> 'cancelled' AND i.id IS NOT ?3
             ORDER BY l.arrivalOrder",
        )
        .map_err(|_| internal_error("failed to query entries for cost"))?;
    let entries = statement
        .query_map(params![product_id, base_order, excluded_invoice], |row| {
            Ok((
                row.get::<_, i64>(0)?,
                row.get::<_, i64>(1)?,
                row.get::<_, i64>(2)?,
            ))
        })
        .map_err(|_| internal_error("failed to read entries for cost"))?
        .collect::<rusqlite::Result<Vec<_>>>()
        .map_err(|_| internal_error("failed to collect entries for cost"))?;
    for (stock_before, quantity, unit_cost) in entries {
        let total = i128::from(stock_before) + i128::from(quantity);
        if total > 0 {
            let weighted = i128::from(stock_before) * i128::from(cost)
                + i128::from(quantity) * i128::from(unit_cost);
            cost = round_half_up(weighted, total) as i64;
        }
    }
    connection
        .execute(
            "UPDATE products SET costCents = ?1, updatedAt = ?2 WHERE id = ?3",
            (cost, format_utc_timestamp(Utc::now()), product_id),
        )
        .map_err(|_| internal_error("failed to update product cost"))?;
    Ok(())
}

/// Cuando una entrada pierde `delta_milli` de stock (anular, eliminar o
/// reducirla) o lo gana (aumentarla), las entradas posteriores del producto
/// tenían ese stock de más o de menos antes de llegar (`plan-06` §3.1).
pub fn shift_later_stock_before(
    connection: &Connection,
    product_id: i64,
    after_arrival_order: i64,
    delta_milli: i64,
) -> Result<(), AppError> {
    if delta_milli == 0 {
        return Ok(());
    }
    connection
        .execute(
            "UPDATE lots SET stockBeforeMilli = MAX(0, stockBeforeMilli + ?3)
             WHERE productId = ?1 AND origin = 'entry' AND arrivalOrder > ?2
               AND stockBeforeMilli IS NOT NULL",
            params![product_id, after_arrival_order, delta_milli],
        )
        .map_err(|_| internal_error("failed to shift stock before later entries"))?;
    Ok(())
}

/// Orden de llegada de un lote, para desplazar las entradas posteriores.
pub fn lot_arrival_order(connection: &Connection, lot_id: i64) -> Result<Option<i64>, AppError> {
    connection
        .query_row(
            "SELECT arrivalOrder FROM lots WHERE id = ?1",
            [lot_id],
            |row| row.get(0),
        )
        .optional()
        .map_err(|_| internal_error("failed to read lot arrival order"))
}

pub fn recalculate_product_quantity(
    connection: &Connection,
    product_id: i64,
) -> Result<(), AppError> {
    let quantity: i64 = connection
        .query_row(
            "SELECT COALESCE(SUM(remainingMilli), 0) FROM lots WHERE productId = ?1",
            [product_id],
            |row| row.get(0),
        )
        .map_err(|_| internal_error("failed to sum product lot quantity"))?;
    let updated = connection
        .execute(
            "UPDATE products SET quantityMilli = ?1, updatedAt = ?2 WHERE id = ?3",
            (quantity, format_utc_timestamp(Utc::now()), product_id),
        )
        .map_err(|_| internal_error("failed to update product quantity"))?;
    if updated == 0 {
        return Err(AppError::new(
            "notFound",
            "El producto ya no existe",
            "product missing during quantity recalculation",
        ));
    }
    Ok(())
}

/// Lo apartado (`spec-06` EN-01): lo vendido en Venta y Servicio no
/// canceladas que aún no se entregó. `excluding_invoice` deja fuera una
/// factura (la que se está editando).
pub fn reserved_milli(
    connection: &Connection,
    product_id: i64,
    excluding_invoice: Option<i64>,
) -> Result<i64, AppError> {
    connection
        .query_row(
            "SELECT COALESCE(SUM(il.quantityMilli - il.deliveredMilli), 0)
             FROM invoiceLines il JOIN invoices i ON i.id = il.invoiceId
             WHERE il.productId = ?1 AND i.type IN ('V', 'S') AND i.status <> 'cancelled'
               AND i.id IS NOT ?2",
            params![product_id, excluding_invoice],
            |row| row.get(0),
        )
        .map_err(|_| internal_error("failed to read reserved quantity"))
}

/// Disponible = stock − apartado (`spec-06` EN-01).
pub fn available_milli(
    connection: &Connection,
    product_id: i64,
    excluding_invoice: Option<i64>,
) -> Result<i64, AppError> {
    let stock: i64 = connection
        .query_row(
            "SELECT quantityMilli FROM products WHERE id = ?1",
            [product_id],
            |row| row.get(0),
        )
        .map_err(|_| internal_error("failed to read product stock"))?;
    Ok(stock - reserved_milli(connection, product_id, excluding_invoice)?)
}

/// Rechaza una salida que tomaría más de lo disponible (EN-01: lo apartado
/// no se puede vender ni sacar a otro cliente).
pub fn ensure_available(
    connection: &Connection,
    product_id: i64,
    needed_milli: i64,
    excluding_invoice: Option<i64>,
) -> Result<(), AppError> {
    let available = available_milli(connection, product_id, excluding_invoice)?;
    let product_name: String = connection
        .query_row(
            "SELECT name FROM products WHERE id = ?1",
            [product_id],
            |row| row.get(0),
        )
        .map_err(|_| internal_error("failed to read product name"))?;
    if needed_milli > available {
        return Err(AppError::new(
            "stockInsuficiente",
            &format!(
                "No hay stock suficiente de «{product_name}» (disponible: {})",
                format_quantity_milli(available.max(0))
            ),
            "available stock is insufficient",
        ));
    }
    Ok(())
}

/// Las entradas no pueden dejar el stock por debajo de lo apartado (D-08):
/// se comprueba después de bajarlo, dentro de la misma transacción.
pub fn ensure_stock_covers_reserved(
    connection: &Connection,
    product_id: i64,
) -> Result<(), AppError> {
    let available = available_milli(connection, product_id, None)?;
    if available < 0 {
        let reserved = reserved_milli(connection, product_id, None)?;
        return Err(AppError::new(
            "stockBelowReserved",
            &format!(
                "No se puede: {} unidades de este producto están vendidas y sin entregar",
                format_quantity_milli(reserved)
            ),
            "entry change leaves stock below reserved",
        ));
    }
    Ok(())
}

pub fn validate_stock_limit(
    connection: &Connection,
    product_id: i64,
    adding_milli: i64,
) -> Result<(), AppError> {
    let product: Option<(String, i64)> = connection.query_row(
        "SELECT p.name, COALESCE(SUM(l.remainingMilli), 0) FROM products p LEFT JOIN lots l ON l.productId = p.id WHERE p.id = ?1 GROUP BY p.id",
        [product_id], |row| Ok((row.get(0)?, row.get(1)?)),
    ).optional().map_err(|_| internal_error("failed to read product stock for limit"))?;
    let (product_name, current_stock) = product.ok_or_else(|| {
        AppError::new(
            "notFound",
            "El producto ya no existe",
            "product missing during stock limit validation",
        )
    })?;
    if adding_milli <= 0 || current_stock.saturating_add(adding_milli) > STOCK_LIMIT_MILLI {
        return Err(AppError::new(
            "stockLimitExceeded",
            &format!("El stock total de «{product_name}» no puede superar 999.999,999"),
            "product stock limit exceeded",
        ));
    }
    Ok(())
}

pub fn restore_consumption(
    connection: &Connection,
    consumption_id: i64,
    now: DateTime<Utc>,
) -> Result<Restoration, AppError> {
    let consumption: Option<(Option<i64>, i64, i64, Option<i64>)> = connection.query_row(
        "SELECT c.lotId, c.quantityMilli, c.unitCostCents, il.productId FROM consumptions c JOIN invoiceLines il ON il.id = c.invoiceLineId WHERE c.id = ?1",
        [consumption_id], |row| Ok((row.get(0)?, row.get(1)?, row.get(2)?, row.get(3)?)),
    ).optional().map_err(|_| internal_error("failed to read consumption for restoration"))?;
    let Some((lot_id, quantity_milli, unit_cost_cents, product_id)) = consumption else {
        return Err(AppError::new(
            "notFound",
            "El consumo ya no existe",
            "consumption missing during restoration",
        ));
    };
    let Some(product_id) = product_id else {
        return Ok(Restoration {
            product_id: None,
            lot_id: None,
            quantity_milli: 0,
        });
    };
    validate_stock_limit(connection, product_id, quantity_milli)?;
    let timestamp = format_utc_timestamp(now);
    if let Some(lot_id) = lot_id {
        let updated = connection.execute(
            "UPDATE lots SET remainingMilli = remainingMilli + ?1, updatedAt = ?2 WHERE id = ?3 AND productId = ?4",
            (quantity_milli, &timestamp, lot_id, product_id),
        ).map_err(|_| internal_error("failed to restore original lot"))?;
        if updated > 0 {
            recalculate_product_quantity(connection, product_id)?;
            return Ok(Restoration {
                product_id: Some(product_id),
                lot_id: Some(lot_id),
                quantity_milli,
            });
        }
    }
    let reusable_lot: Option<i64> = connection.query_row(
        "SELECT id FROM lots WHERE productId = ?1 AND origin = 'restitution' AND unitCostCents = ?2 AND remainingMilli > 0 ORDER BY arrivalOrder, id LIMIT 1",
        (product_id, unit_cost_cents), |row| row.get(0),
    ).optional().map_err(|_| internal_error("failed to find reusable restitution lot"))?;
    let restored_lot_id = if let Some(reusable_lot) = reusable_lot {
        connection.execute(
            "UPDATE lots SET remainingMilli = remainingMilli + ?1, initialMilli = initialMilli + ?1, updatedAt = ?2 WHERE id = ?3",
            (quantity_milli, &timestamp, reusable_lot),
        ).map_err(|_| internal_error("failed to update restitution lot"))?;
        reusable_lot
    } else {
        let arrival_order = next_restitution_order(connection, product_id)?;
        connection.execute(
            "INSERT INTO lots (productId, origin, arrivalOrder, initialMilli, remainingMilli, unitCostCents, createdAt, updatedAt) VALUES (?1, 'restitution', ?2, ?3, ?3, ?4, ?5, ?5)",
            (product_id, arrival_order, quantity_milli, unit_cost_cents, &timestamp),
        ).map_err(|_| internal_error("failed to create restitution lot"))?;
        connection.last_insert_rowid()
    };
    recalculate_product_quantity(connection, product_id)?;
    Ok(Restoration {
        product_id: Some(product_id),
        lot_id: Some(restored_lot_id),
        quantity_milli,
    })
}

pub fn reverse_lots_of_invoice(
    connection: &Connection,
    invoice_id: i64,
    now: DateTime<Utc>,
) -> Result<(), AppError> {
    let mut statement = connection
        .prepare(
            "SELECT l.id, l.productId, l.remainingMilli, l.unitCostCents, il.productIdOriginal,
                    p.name, i.number, i.type, i.createdByName, l.arrivalOrder
             FROM lots l
             JOIN invoiceLines il ON il.id = l.invoiceLineId
             JOIN invoices i ON i.id = il.invoiceId
             JOIN products p ON p.id = l.productId
             WHERE i.id = ?1 AND l.remainingMilli > 0
             ORDER BY il.position, l.id",
        )
        .map_err(|_| internal_error("failed to query invoice lots for reversal"))?;
    let lots = statement
        .query_map([invoice_id], |row| {
            Ok((
                row.get::<_, i64>(0)?,
                row.get::<_, i64>(1)?,
                row.get::<_, i64>(2)?,
                row.get::<_, i64>(3)?,
                row.get::<_, i64>(4)?,
                row.get::<_, String>(5)?,
                row.get::<_, String>(6)?,
                row.get::<_, String>(7)?,
                row.get::<_, String>(8)?,
                row.get::<_, i64>(9)?,
            ))
        })
        .map_err(|_| internal_error("failed to read invoice lots for reversal"))?
        .collect::<rusqlite::Result<Vec<_>>>()
        .map_err(|_| internal_error("failed to collect invoice lots for reversal"))?;
    drop(statement);

    let timestamp = format_utc_timestamp(now);
    for (
        lot_id,
        product_id,
        remaining_milli,
        unit_cost_cents,
        product_id_original,
        product_name,
        invoice_number,
        invoice_type,
        created_by_name,
        arrival_order,
    ) in lots
    {
        connection
            .execute(
                "UPDATE lots SET remainingMilli = 0, updatedAt = ?1 WHERE id = ?2",
                (&timestamp, lot_id),
            )
            .map_err(|_| internal_error("failed to reverse lot stock"))?;
        recalculate_product_quantity(connection, product_id)?;
        ensure_stock_covers_reserved(connection, product_id)?;
        shift_later_stock_before(connection, product_id, arrival_order, -remaining_milli)?;
        replay_product_cost(connection, product_id, Some(invoice_id))?;
        let stock_after: i64 = connection
            .query_row(
                "SELECT quantityMilli FROM products WHERE id = ?1",
                [product_id],
                |row| row.get(0),
            )
            .map_err(|_| internal_error("failed to read stock after invoice reversal"))?;
        let user_name: Option<String> = connection
            .query_row(
                "SELECT u.username FROM sessions s JOIN users u ON u.id = s.userId WHERE s.id = 1",
                [],
                |row| row.get(0),
            )
            .optional()
            .map_err(|_| internal_error("failed to read actor for invoice reversal"))?;
        let user_name = user_name.unwrap_or(created_by_name);
        let total_price = round_half_up(
            i128::from(remaining_milli) * i128::from(unit_cost_cents),
            1_000,
        ) as i64;
        connection
            .execute(
                "INSERT INTO productMovements (createdAt, productId, productIdOriginal, productName, type,
                    quantityMilli, stockAfterMilli, unitPriceCents, totalPriceCents, invoiceId,
                    invoiceNumber, invoiceType, userId, userName)
                 VALUES (?1, ?2, ?3, ?4, 'reversal', ?5, ?6, ?7, ?8, ?9, ?10, ?11,
                    (SELECT userId FROM sessions WHERE id = 1), ?12)",
                params![
                    &timestamp,
                    product_id,
                    product_id_original,
                    product_name,
                    -remaining_milli,
                    stock_after,
                    unit_cost_cents,
                    total_price,
                    invoice_id,
                    invoice_number,
                    invoice_type,
                    user_name,
                ],
            )
            .map_err(|_| internal_error("failed to register invoice reversal movement"))?;
    }
    Ok(())
}

fn internal_error(cause: &str) -> AppError {
    AppError::new("internal", "Ocurrió un error inesperado", cause)
}

// ---------- Unidad por línea (spec-04 CV-01, D-10) ----------

const NO_CONVERSION_MESSAGE: &str = "No hay una conversión entre esas unidades";

/// Unidad que el usuario eligió en una línea, distinta de la del producto.
/// La conversión es de un paso, directa o inversa (S-08), y se guarda como
/// fracción exacta: 1 unidad elegida = `numerator / denominator` unidades del
/// producto. El inventario siempre va en la unidad del producto.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct LineUnit {
    pub unit_id: i64,
    pub name: String,
    pub abbreviation: String,
    pub entered_milli: i64,
    numerator: i128,
    denominator: i128,
}

impl LineUnit {
    /// Cantidad en la unidad del producto, a 3 decimales (mitad hacia arriba).
    pub fn product_milli(&self) -> i64 {
        round_half_up(
            i128::from(self.entered_milli) * self.numerator,
            self.denominator,
        ) as i64
    }

    /// Factor guardado en la línea (unidades del producto por unidad elegida).
    pub fn factor_micro(&self) -> i64 {
        round_half_up(self.numerator * 1_000_000, self.denominator) as i64
    }

    /// Precio por unidad elegida → por unidad del producto (costo del lote).
    pub fn per_product_unit(&self, cents: i64) -> i64 {
        round_half_up(i128::from(cents) * self.denominator, self.numerator) as i64
    }

    /// Precio por unidad del producto → por unidad elegida (precio de venta).
    pub fn per_entered_unit(&self, cents: i64) -> i64 {
        round_half_up(i128::from(cents) * self.numerator, self.denominator) as i64
    }
}

/// Resuelve la unidad elegida para una línea. `None` si no se eligió o es la
/// del producto; error si no hay conversión de un paso con la del producto.
pub fn resolve_line_unit(
    connection: &Connection,
    product_id: i64,
    unit_id: Option<i64>,
    entered_milli: i64,
) -> Result<Option<LineUnit>, AppError> {
    let Some(unit_id) = unit_id else {
        return Ok(None);
    };
    let product_unit: Option<i64> = connection
        .query_row(
            "SELECT unitId FROM products WHERE id = ?1",
            [product_id],
            |row| row.get(0),
        )
        .optional()
        .map_err(|_| internal_error("failed to read product unit for line"))?
        .flatten();
    if product_unit == Some(unit_id) {
        return Ok(None);
    }
    let no_conversion = || {
        AppError::new(
            "validationFailed",
            NO_CONVERSION_MESSAGE,
            "line unit has no one-step conversion to the product unit",
        )
    };
    let product_unit = product_unit.ok_or_else(no_conversion)?;
    let unit: (String, String) = connection
        .query_row(
            "SELECT name, abbreviation FROM units WHERE id = ?1",
            [unit_id],
            |row| Ok((row.get(0)?, row.get(1)?)),
        )
        .optional()
        .map_err(|_| internal_error("failed to read line unit"))?
        .ok_or_else(|| AppError::new("notFound", "La unidad ya no existe", "line unit missing"))?;
    // Directa: 1 elegida = f producto. Inversa: 1 producto = f elegidas.
    let (from_id, factor_micro): (i64, i64) = connection
        .query_row(
            "SELECT fromUnitId, factorMicro FROM unitConversions
             WHERE (fromUnitId = ?1 AND toUnitId = ?2) OR (fromUnitId = ?2 AND toUnitId = ?1)",
            [unit_id, product_unit],
            |row| Ok((row.get(0)?, row.get(1)?)),
        )
        .optional()
        .map_err(|_| internal_error("failed to read line unit conversion"))?
        .ok_or_else(no_conversion)?;
    let (numerator, denominator) = if from_id == unit_id {
        (i128::from(factor_micro), 1_000_000)
    } else {
        (1_000_000, i128::from(factor_micro))
    };
    let line_unit = LineUnit {
        unit_id,
        name: unit.0,
        abbreviation: unit.1,
        entered_milli,
        numerator,
        denominator,
    };
    if !(1..=999_999_999).contains(&line_unit.product_milli()) {
        return Err(AppError::new(
            "validationFailed",
            "La cantidad convertida a la unidad del producto no es válida",
            "converted line quantity out of range",
        ));
    }
    Ok(Some(line_unit))
}

#[cfg(test)]
mod line_unit_tests {
    use super::resolve_line_unit;
    use crate::test_utils::{
        insert_conversion_fixture, insert_product_fixture, insert_unit_fixture, setup_test_db,
    };

    #[test]
    fn direct_and_inverse_one_step_conversions_round_to_three_decimals() {
        let connection = setup_test_db();
        let unit = insert_unit_fixture(&connection, "Unidad", "u");
        let box_unit = insert_unit_fixture(&connection, "Caja", "cj");
        let dozen = insert_unit_fixture(&connection, "Docena", "dz");
        let pallet = insert_unit_fixture(&connection, "Palé", "pl");
        // 1 Caja = 12 u (directa); 1 u = 0,083333 Docena (se usa al revés).
        insert_conversion_fixture(&connection, box_unit, unit, 12_000_000);
        insert_conversion_fixture(&connection, unit, dozen, 83_333);
        insert_conversion_fixture(&connection, pallet, box_unit, 40_000_000);
        let product = insert_product_fixture(
            &connection,
            "Cable",
            None,
            Some(unit),
            100,
            0,
            0,
            "active",
            None,
            None,
        );

        assert_eq!(
            resolve_line_unit(&connection, product, None, 2_000).unwrap(),
            None
        );
        assert_eq!(
            resolve_line_unit(&connection, product, Some(unit), 2_000).unwrap(),
            None
        );

        let boxes = resolve_line_unit(&connection, product, Some(box_unit), 2_000)
            .unwrap()
            .unwrap();
        assert_eq!(
            (boxes.product_milli(), boxes.factor_micro()),
            (24_000, 12_000_000)
        );
        assert_eq!(
            (boxes.name.as_str(), boxes.abbreviation.as_str()),
            ("Caja", "cj")
        );
        // Compra: 30,00 la caja → 2,50 la unidad. Venta: 2,50 la unidad → 30,00 la caja.
        assert_eq!(boxes.per_product_unit(3_000), 250);
        assert_eq!(boxes.per_entered_unit(250), 3_000);

        // Inversa: 1 u = 0,083333 Docena → 1 Docena ≈ 12,000048 u; 1,5 docenas = 18 u.
        let dozens = resolve_line_unit(&connection, product, Some(dozen), 1_500)
            .unwrap()
            .unwrap();
        assert_eq!(dozens.product_milli(), 18_000);

        // Sin conversión de un paso (Palé → Caja → u no se encadena).
        let error = resolve_line_unit(&connection, product, Some(pallet), 1_000).unwrap_err();
        assert_eq!(error.message, "No hay una conversión entre esas unidades");

        let without_unit = insert_product_fixture(
            &connection,
            "Suelto",
            None,
            None,
            100,
            0,
            0,
            "active",
            None,
            None,
        );
        let error =
            resolve_line_unit(&connection, without_unit, Some(box_unit), 1_000).unwrap_err();
        assert_eq!(error.message, "No hay una conversión entre esas unidades");
        assert_eq!(
            resolve_line_unit(&connection, product, Some(9_999), 1_000)
                .unwrap_err()
                .code,
            "notFound"
        );
    }
}

#[cfg(test)]
mod tests {
    use chrono::{TimeZone, Utc};
    use rusqlite::Connection;

    use crate::models::consumption::Consumption;
    use crate::test_utils::{create_test_user, insert_product_fixture, setup_test_db};

    use super::{
        consume_fifo, line_total_cost_cents, line_unit_cost_cents, next_arrival_order,
        next_restitution_order, recalculate_product_cost, recalculate_product_quantity,
        restore_consumption, validate_stock_limit,
    };

    const TS: &str = "2026-10-03T00:00:00Z";

    fn create_product_with_stock(
        connection: &Connection,
        quantity_milli: i64,
        cost_cents: i64,
    ) -> i64 {
        insert_product_fixture(
            connection,
            "Producto de prueba",
            None,
            None,
            cost_cents,
            0,
            quantity_milli,
            "active",
            None,
            None,
        )
    }

    fn create_entry_line(connection: &Connection, product_id: i64) -> i64 {
        connection.execute(
            "INSERT INTO suppliers (firstName, lastName, nationalId, phone, createdAt, updatedAt)
             VALUES ('Juan', 'Pérez', '12345678901', '+5355551234', ?1, ?1)",
            [TS],
        ).unwrap();
        let supplier_id = connection.last_insert_rowid();
        let user_id = create_test_user(
            connection,
            "inventoryuser",
            "administrador",
            "+5355551200",
            "12345678902",
            "ValidPass1!",
        );
        connection.execute(
            "INSERT INTO invoices (type, number, invoiceDate, status, supplierId, contactFullName,
                contactFullNameNormalized, contactNationalId, contactPhone, createdByUserId, createdByName, createdAt, updatedAt)
             VALUES ('E', 'E-00001', '2026-10-03', 'pending', ?1, 'Juan Pérez', 'juan perez', '12345678901', '+5355551234', ?2, 'admin', ?3, ?3)",
            (supplier_id, user_id, TS),
        ).unwrap();
        let invoice_id = connection.last_insert_rowid();
        connection.execute(
            "INSERT INTO invoiceLines (invoiceId, position, productId, productIdOriginal, productName, productNameNormalized,
                quantityMilli, unitPriceCents, subtotalCents, createdAt, updatedAt)
             VALUES (?1, 1, ?2, ?2, 'Producto de prueba', 'producto de prueba', 1, 100, 1, ?3, ?3)",
            (invoice_id, product_id, TS),
        ).unwrap();
        connection.last_insert_rowid()
    }

    fn insert_lot(
        connection: &Connection,
        product_id: i64,
        arrival: i64,
        initial: i64,
        remaining: i64,
        cost: i64,
    ) -> i64 {
        connection.execute(
            "INSERT INTO lots (productId, origin, arrivalOrder, initialMilli, remainingMilli, unitCostCents, createdAt, updatedAt)
             VALUES (?1, 'restitution', ?2, ?3, ?4, ?5, ?6, ?6)",
            (product_id, arrival, initial, remaining, cost, TS),
        ).unwrap();
        connection.last_insert_rowid()
    }

    fn insert_entry_lot(
        connection: &Connection,
        product_id: i64,
        line_id: i64,
        initial: i64,
        remaining: i64,
        cost: i64,
    ) -> i64 {
        connection.execute(
            "INSERT INTO lots (productId, origin, invoiceLineId, arrivalOrder, initialMilli, remainingMilli, unitCostCents, createdAt, updatedAt)
             VALUES (?1, 'entry', ?2, 1, ?3, ?4, ?5, ?6, ?6)",
            (product_id, line_id, initial, remaining, cost, TS),
        ).unwrap();
        connection.last_insert_rowid()
    }

    fn sample_consumption(quantity_milli: i64, unit_cost_cents: i64) -> Consumption {
        Consumption {
            id: 1,
            invoice_line_id: 1,
            lot_id: Some(1),
            quantity_milli,
            unit_cost_cents,
            created_at: TS.to_owned(),
        }
    }

    #[test]
    fn fifo_consumption_crosses_lots_and_records_each_original_cost() {
        let connection = setup_test_db();
        let product_id = create_product_with_stock(&connection, 23_000, 100);
        let line_id = create_entry_line(&connection, product_id);
        let first_lot = insert_lot(&connection, product_id, 1, 10_000, 10_000, 100);
        let second_lot = insert_lot(&connection, product_id, 2, 5_000, 5_000, 300);
        insert_lot(&connection, product_id, 3, 8_000, 8_000, 200);

        let consumed = consume_fifo(
            &connection,
            product_id,
            12_000,
            line_id,
            Utc.with_ymd_and_hms(2026, 10, 6, 12, 0, 0).unwrap(),
        )
        .unwrap();
        assert_eq!(consumed.len(), 2);
        assert_eq!(
            (
                consumed[0].lot_id,
                consumed[0].quantity_milli,
                consumed[0].unit_cost_cents
            ),
            (Some(first_lot), 10_000, 100)
        );
        assert_eq!(
            (
                consumed[1].lot_id,
                consumed[1].quantity_milli,
                consumed[1].unit_cost_cents
            ),
            (Some(second_lot), 2_000, 300)
        );
    }

    #[test]
    fn fifo_rejects_insufficient_stock_without_mutating_lots() {
        let connection = setup_test_db();
        let product_id = create_product_with_stock(&connection, 5_000, 100);
        let line_id = create_entry_line(&connection, product_id);
        let lot_id = insert_lot(&connection, product_id, 1, 5_000, 5_000, 100);
        let error = consume_fifo(&connection, product_id, 6_000, line_id, Utc::now()).unwrap_err();
        assert_eq!(error.code, "stockInsuficiente");
        let remaining: i64 = connection
            .query_row(
                "SELECT remainingMilli FROM lots WHERE id = ?1",
                [lot_id],
                |row| row.get(0),
            )
            .unwrap();
        assert_eq!(remaining, 5_000);
    }

    #[test]
    fn line_costs_round_once_and_unit_cost_rounds_half_up() {
        let consumptions = [
            sample_consumption(10_000, 100),
            sample_consumption(2_000, 300),
        ];
        assert_eq!(line_total_cost_cents(&consumptions), 1_600);
        assert_eq!(line_unit_cost_cents(1_600, 12_000), 133);
        assert_eq!(line_unit_cost_cents(1, 2_000), 1);
    }

    #[test]
    fn lot_order_and_product_cache_recalculation_follow_remaining_stock() {
        let connection = setup_test_db();
        let product_id = create_product_with_stock(&connection, 15_000, 100);
        assert_eq!(next_arrival_order(&connection, product_id).unwrap(), 1);
        insert_lot(&connection, product_id, 1, 10_000, 10_000, 100);
        insert_lot(&connection, product_id, 2, 5_000, 5_000, 300);
        assert_eq!(next_arrival_order(&connection, product_id).unwrap(), 3);
        assert_eq!(next_restitution_order(&connection, product_id).unwrap(), 0);
        connection
            .execute(
                "DELETE FROM lots WHERE productId = ?1 AND arrivalOrder = 2",
                [product_id],
            )
            .unwrap();
        assert_eq!(next_arrival_order(&connection, product_id).unwrap(), 3);
        recalculate_product_quantity(&connection, product_id).unwrap();
        recalculate_product_cost(&connection, product_id).unwrap();
        let cached: (i64, i64) = connection
            .query_row(
                "SELECT quantityMilli, costCents FROM products WHERE id = ?1",
                [product_id],
                |row| Ok((row.get(0)?, row.get(1)?)),
            )
            .unwrap();
        assert_eq!(cached, (10_000, 100));

        connection
            .execute(
                "UPDATE lots SET remainingMilli = 0 WHERE productId = ?1",
                [product_id],
            )
            .unwrap();
        recalculate_product_quantity(&connection, product_id).unwrap();
        recalculate_product_cost(&connection, product_id).unwrap();
        let empty_cache: (i64, i64) = connection
            .query_row(
                "SELECT quantityMilli, costCents FROM products WHERE id = ?1",
                [product_id],
                |row| Ok((row.get(0)?, row.get(1)?)),
            )
            .unwrap();
        assert_eq!(empty_cache, (0, 100));
    }

    #[test]
    fn stock_limit_uses_sum_of_remaining_lots_and_rejects_overflowing_addition() {
        let connection = setup_test_db();
        let product_id = create_product_with_stock(&connection, 999_999_998, 100);
        insert_lot(&connection, product_id, 1, 999_999_998, 999_999_998, 100);
        assert!(validate_stock_limit(&connection, product_id, 1).is_ok());
        assert_eq!(
            validate_stock_limit(&connection, product_id, 2)
                .unwrap_err()
                .code,
            "stockLimitExceeded"
        );
    }

    #[test]
    fn restore_consumption_returns_units_to_original_lot_or_restitution_lot() {
        let connection = setup_test_db();
        let product_id = create_product_with_stock(&connection, 5_000, 100);
        let line_id = create_entry_line(&connection, product_id);
        let lot_id = insert_lot(&connection, product_id, 1, 5_000, 0, 100);
        connection.execute("INSERT INTO consumptions (invoiceLineId, lotId, quantityMilli, unitCostCents, createdAt) VALUES (?1, ?2, 5_000, 100, ?3)", (line_id, lot_id, TS)).unwrap();
        let consumption_id = connection.last_insert_rowid();
        let restored = restore_consumption(&connection, consumption_id, Utc::now()).unwrap();
        assert_eq!(restored.lot_id, Some(lot_id));
        let remaining: i64 = connection
            .query_row(
                "SELECT remainingMilli FROM lots WHERE id = ?1",
                [lot_id],
                |row| row.get(0),
            )
            .unwrap();
        assert_eq!(remaining, 5_000);

        connection
            .execute("DELETE FROM lots WHERE id = ?1", [lot_id])
            .unwrap();
        let restored_again = restore_consumption(&connection, consumption_id, Utc::now()).unwrap();
        assert_ne!(restored_again.lot_id, Some(lot_id));
        let restitution: (String, i64, i64, i64) = connection.query_row("SELECT origin, arrivalOrder, remainingMilli, unitCostCents FROM lots WHERE id = ?1", [restored_again.lot_id.unwrap()], |row| Ok((row.get(0)?, row.get(1)?, row.get(2)?, row.get(3)?))).unwrap();
        assert_eq!(restitution, ("restitution".to_owned(), 0, 5_000, 100));
    }

    #[test]
    fn reversing_entry_invoice_removes_remaining_lot_stock_and_records_movement() {
        let connection = setup_test_db();
        let product_id = create_product_with_stock(&connection, 5_000, 100);
        let line_id = create_entry_line(&connection, product_id);
        let lot_id = insert_entry_lot(&connection, product_id, line_id, 5_000, 5_000, 100);
        super::reverse_lots_of_invoice(
            &connection,
            1,
            Utc.with_ymd_and_hms(2026, 10, 6, 12, 0, 0).unwrap(),
        )
        .unwrap();

        let remaining: i64 = connection
            .query_row(
                "SELECT remainingMilli FROM lots WHERE id = ?1",
                [lot_id],
                |row| row.get(0),
            )
            .unwrap();
        let cache: (i64, i64) = connection
            .query_row(
                "SELECT quantityMilli, costCents FROM products WHERE id = ?1",
                [product_id],
                |row| Ok((row.get(0)?, row.get(1)?)),
            )
            .unwrap();
        let movement: (i64, i64, i64, String, String) = connection
            .query_row("SELECT quantityMilli, stockAfterMilli, unitPriceCents, invoiceNumber, type FROM productMovements", [], |row| Ok((row.get(0)?, row.get(1)?, row.get(2)?, row.get(3)?, row.get(4)?)))
            .unwrap();
        assert_eq!(remaining, 0);
        assert_eq!(cache, (0, 100));
        assert_eq!(
            movement,
            (-5_000, 0, 100, "E-00001".to_owned(), "reversal".to_owned())
        );
    }
}
