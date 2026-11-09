use chrono::{DateTime, Datelike, Duration, Months, NaiveDate, Utc};
use rusqlite::Connection;
use std::collections::{HashMap, HashSet};

use crate::commands::auth::require_session;
use crate::commands::products::purge_best_effort;
use crate::db::connection::{current_app_data_dir, lock_conn};
use crate::errors::{log_error, map_db_error, AppError};
use crate::inventory::round_half_up;
use crate::models::dashboard::{
    BestSeller, DashboardKpis, DashboardPeriod, ExpiringReservation, FrequentClient, IncomeBucket,
    KpiSeries, ReservationsSummary, SlowMover, StarItem,
};
use crate::models::product::{format_quantity_milli, DashboardSummary, OutOfStockCategory};
use crate::pricing::selling_price_cents;
use crate::text::{sort_spanish, spanish_sort_key};
use crate::timestamps::{parse_invoice_date, parse_utc_timestamp, today_local};

pub fn get_dashboard_summary_inner(
    connection: &mut Connection,
    now: DateTime<Utc>,
) -> Result<DashboardSummary, AppError> {
    require_session(connection, now)?;

    let out_of_stock_total: i64 = connection
        .query_row(
            "SELECT COUNT(*) FROM products WHERE status = 'active' AND quantityMilli = 0",
            [],
            |row| row.get(0),
        )
        .map_err(|error| map_db_error(error, "getDashboardSummary", &[]))?;

    let mut top_categories: Vec<OutOfStockCategory> = connection
        .prepare(
            "SELECT c.name, COUNT(*) AS total
             FROM products p
             INNER JOIN categories c ON c.id = p.categoryId
             WHERE p.status = 'active' AND p.quantityMilli = 0
             GROUP BY p.categoryId, c.name
             ORDER BY total DESC, c.name ASC",
        )
        .map_err(|error| map_db_error(error, "getDashboardSummary", &[]))?
        .query_map([], |row| {
            Ok(OutOfStockCategory {
                name: row.get(0)?,
                count: row.get(1)?,
            })
        })
        .map_err(|error| map_db_error(error, "getDashboardSummary", &[]))?
        .collect::<Result<Vec<_>, _>>()
        .map_err(|error| map_db_error(error, "getDashboardSummary", &[]))?;

    top_categories.sort_by(|left, right| {
        right
            .count
            .cmp(&left.count)
            .then_with(|| spanish_sort_key(&left.name).cmp(&spanish_sort_key(&right.name)))
    });
    top_categories.truncate(3);

    let without_category_count: i64 = connection
        .query_row(
            "SELECT COUNT(*) FROM products WHERE status = 'active' AND quantityMilli = 0 AND categoryId IS NULL",
            [],
            |row| row.get(0),
        )
        .map_err(|error| map_db_error(error, "getDashboardSummary", &[]))?;

    Ok(DashboardSummary {
        out_of_stock_total,
        top_categories,
        without_category_count,
    })
}

#[tauri::command]
pub fn get_dashboard_summary(
    state: tauri::State<'_, std::sync::Mutex<Connection>>,
) -> Result<DashboardSummary, AppError> {
    let mut connection = lock_conn(&state);
    let now = Utc::now();
    let result = get_dashboard_summary_inner(&mut connection, now);
    if let Err(error) = &result {
        if error.should_log() {
            if let Some(app_data_dir) = current_app_data_dir() {
                log_error(
                    &connection,
                    "dashboard",
                    "getDashboardSummary",
                    error,
                    now,
                    app_data_dir,
                );
            }
        }
    }
    result
}

// ---------- KPIs con período (spec-04 DB-01 a DB-07) ----------

/// Una línea de venta o servicio Cobrado, ubicada por su fecha de cobro.
struct IncomeLine {
    invoice_id: i64,
    invoice_type: String,
    paid_on: NaiveDate,
    client_key: String,
    client_name: String,
    product_key: i64,
    product_name: String,
    quantity_milli: i64,
    subtotal_cents: i64,
    cost_cents: i64,
}

/// Garantía, insumos o pérdida, ubicados por su fecha de emisión (S-05).
struct ExpenseRow {
    invoice_type: String,
    issued_on: NaiveDate,
    cents: i64,
}

#[derive(Default)]
struct Totals {
    income: i64,
    cost: i64,
    sales: i64,
    sales_amount: i64,
    expenses: i64,
}

impl Totals {
    fn net_profit(&self) -> i64 {
        self.income - self.cost - self.expenses
    }

    fn margin_hundredths(&self) -> i64 {
        if self.income == 0 {
            return 0;
        }
        round_half_up(
            i128::from(self.income - self.cost) * 10_000,
            i128::from(self.income),
        ) as i64
    }
}

fn validation_failed(message: &str, cause: &str) -> AppError {
    AppError::new("validationFailed", message, cause)
}

/// Fechas locales del período, ambas incluidas (DB-01).
fn period_range(
    period: &DashboardPeriod,
    today: NaiveDate,
) -> Result<(NaiveDate, NaiveDate), AppError> {
    let months_back = |months: u32| {
        today
            .checked_sub_months(Months::new(months))
            .map_or(today, |date| date + Duration::days(1))
    };
    let from = match period.preset.as_str() {
        "today" => today,
        "week" => today - Duration::days(i64::from(today.weekday().num_days_from_monday())),
        "month" => today.with_day(1).unwrap_or(today),
        "3m" => months_back(3),
        "6m" => months_back(6),
        "1y" => months_back(12),
        "custom" => {
            let (Some(from), Some(to)) = (
                period.from_date.as_deref().filter(|date| !date.is_empty()),
                period.to_date.as_deref().filter(|date| !date.is_empty()),
            ) else {
                return Err(validation_failed(
                    "Elige la fecha inicial y la final",
                    "custom dashboard period without dates",
                ));
            };
            let (from, to) = (parse_invoice_date(from)?, parse_invoice_date(to)?);
            if from > to {
                return Err(validation_failed(
                    "La fecha inicial no puede ser posterior a la final",
                    "dashboard date range is inverted",
                ));
            }
            return Ok((from, to));
        }
        _ => {
            return Err(validation_failed(
                "El período no es válido",
                "unknown dashboard period",
            ))
        }
    };
    Ok((from, today))
}

/// Tramos de la gráfica: por día, por semana o por mes según la duración
/// del período (DB-04).
fn period_buckets(from: NaiveDate, to: NaiveDate) -> Vec<(String, NaiveDate, NaiveDate)> {
    let days = (to - from).num_days() + 1;
    let mut buckets = Vec::new();
    let mut start = from;
    while start <= to {
        let end = if days <= 31 {
            start
        } else if days <= 186 {
            (start + Duration::days(6)).min(to)
        } else {
            let next_month = start
                .with_day(1)
                .and_then(|first| first.checked_add_months(Months::new(1)))
                .unwrap_or(to + Duration::days(1));
            (next_month - Duration::days(1)).min(to)
        };
        let format = if days <= 186 { "%d/%m" } else { "%m/%Y" };
        buckets.push((start.format(format).to_string(), start, end));
        start = end + Duration::days(1);
    }
    buckets
}

fn period_totals(
    lines: &[IncomeLine],
    expenses: &[ExpenseRow],
    from: NaiveDate,
    to: NaiveDate,
) -> Totals {
    let mut totals = Totals::default();
    let mut sales = HashSet::new();
    for line in lines
        .iter()
        .filter(|line| line.paid_on >= from && line.paid_on <= to)
    {
        totals.income += line.subtotal_cents;
        totals.cost += line.cost_cents;
        if line.invoice_type == "V" {
            sales.insert(line.invoice_id);
            totals.sales_amount += line.subtotal_cents;
        }
    }
    totals.sales = sales.len() as i64;
    totals.expenses = expenses
        .iter()
        .filter(|row| row.issued_on >= from && row.issued_on <= to)
        .map(|row| row.cents)
        .sum();
    totals
}

fn read_income_lines(connection: &Connection) -> rusqlite::Result<Vec<IncomeLine>> {
    connection
        .prepare(
            "SELECT i.id, i.type, i.paidAt,
                    COALESCE(i.contactNationalId, 'nombre:' || i.contactFullNameNormalized), i.contactFullName,
                    il.productIdOriginal, il.productName, il.quantityMilli, il.subtotalCents,
                    COALESCE((SELECT SUM(c.quantityMilli * c.unitCostCents) FROM consumptions c
                              WHERE c.invoiceLineId = il.id), 0)
             FROM invoices i JOIN invoiceLines il ON il.invoiceId = i.id
             WHERE i.type IN ('V', 'S') AND i.status = 'completed'",
        )?
        .query_map([], |row| {
            let Some(paid_at) = parse_utc_timestamp(&row.get::<_, String>(2)?) else {
                return Ok(None);
            };
            Ok(Some(IncomeLine {
                invoice_id: row.get(0)?,
                invoice_type: row.get(1)?,
                paid_on: today_local(paid_at),
                client_key: row.get(3)?,
                client_name: row.get(4)?,
                product_key: row.get(5)?,
                product_name: row.get(6)?,
                quantity_milli: row.get(7)?,
                subtotal_cents: row.get(8)?,
                // Costo real FIFO, con un único redondeo por línea (DB-02).
                cost_cents: round_half_up(i128::from(row.get::<_, i64>(9)?), 1_000) as i64,
            }))
        })?
        .filter_map(Result::transpose)
        .collect()
}

fn read_expenses(connection: &Connection) -> rusqlite::Result<Vec<ExpenseRow>> {
    connection
        .prepare(
            "SELECT i.type, i.invoiceDate, SUM(il.subtotalCents)
             FROM invoices i JOIN invoiceLines il ON il.invoiceId = i.id
             WHERE (i.type IN ('GG', 'GI') AND i.status = 'completed') OR i.type = 'P'
             GROUP BY i.id",
        )?
        .query_map([], |row| {
            let Ok(issued_on) = parse_invoice_date(&row.get::<_, String>(1)?) else {
                return Ok(None);
            };
            Ok(Some(ExpenseRow {
                invoice_type: row.get(0)?,
                issued_on,
                cents: row.get(2)?,
            }))
        })?
        .filter_map(Result::transpose)
        .collect()
}

/// Pareto 80/20: de mayor a menor hasta acumular el 80 % (DB-05). Lo que no
/// aporta (ganancia cero o negativa) no entra.
fn pareto(mut items: Vec<(String, i64)>) -> Vec<StarItem> {
    items.retain(|(_, amount)| *amount > 0);
    sort_spanish(&mut items, |(name, amount)| {
        (-*amount, spanish_sort_key(name))
    });
    let total: i64 = items.iter().map(|(_, amount)| amount).sum();
    let mut accumulated = 0;
    let mut stars = Vec::new();
    for (name, amount) in items {
        if accumulated * 100 >= total * 80 {
            break;
        }
        accumulated += amount;
        stars.push(StarItem {
            name,
            percent: round_half_up(i128::from(amount) * 100, i128::from(total)) as i64,
        });
    }
    stars
}

#[derive(Default)]
struct ProductStats {
    name: String,
    quantity_milli: i64,
    income: i64,
    cost: i64,
}

#[derive(Default)]
struct ClientStats {
    name: String,
    invoices: HashSet<i64>,
    total: i64,
}

fn slow_movers(
    connection: &Connection,
    sold: &HashMap<i64, i64>,
    today: NaiveDate,
) -> rusqlite::Result<Vec<SlowMover>> {
    let last_sales: HashMap<i64, String> = connection
        .prepare(
            "SELECT il.productIdOriginal, MAX(i.paidAt)
             FROM invoiceLines il JOIN invoices i ON i.id = il.invoiceId
             WHERE i.type IN ('V', 'S') AND i.status = 'completed'
             GROUP BY il.productIdOriginal",
        )?
        .query_map([], |row| Ok((row.get(0)?, row.get(1)?)))?
        .collect::<rusqlite::Result<_>>()?;
    let products: Vec<(i64, String, i64, i64, String)> = connection
        .prepare(
            "SELECT id, name, costCents, profitPercentHundredths, createdAt
             FROM products WHERE status = 'active'",
        )?
        .query_map([], |row| {
            Ok((
                row.get(0)?,
                row.get(1)?,
                row.get(2)?,
                row.get(3)?,
                row.get(4)?,
            ))
        })?
        .collect::<rusqlite::Result<_>>()?;
    let mut rows: Vec<(i64, SlowMover)> = products
        .into_iter()
        .map(|(id, name, cost, profit, created_at)| {
            // S-09: si nunca se vendió, los días cuentan desde que se creó.
            let since = last_sales.get(&id).unwrap_or(&created_at);
            let sold_milli = sold.get(&id).copied().unwrap_or(0);
            (
                sold_milli,
                SlowMover {
                    name,
                    sold: format_quantity_milli(sold_milli),
                    selling_price_cents: selling_price_cents(cost, profit),
                    days_without_sale: parse_utc_timestamp(since)
                        .map_or(0, |date| (today - today_local(date)).num_days().max(0)),
                },
            )
        })
        .collect();
    sort_spanish(&mut rows, |(sold_milli, row)| {
        (
            *sold_milli,
            -row.days_without_sale,
            spanish_sort_key(&row.name),
        )
    });
    Ok(rows.into_iter().map(|(_, row)| row).collect())
}

/// Cuentas por cobrar (`spec-06` KP-01): ventas y servicios Por Cobrar,
/// reservas incluidas; la lista sigue mostrando las reservas que vencen en los
/// próximos 3 días (DB-07).
fn reservations_summary(
    connection: &Connection,
    today: NaiveDate,
) -> rusqlite::Result<ReservationsSummary> {
    let (count, amount_cents): (i64, i64) = connection.query_row(
        "SELECT COUNT(DISTINCT i.id), COALESCE(SUM(il.subtotalCents), 0)
         FROM invoices i JOIN invoiceLines il ON il.invoiceId = i.id
         WHERE i.status = 'pending' AND i.type IN ('V', 'S')",
        [],
        |row| Ok((row.get(0)?, row.get(1)?)),
    )?;
    let limit = (today + Duration::days(3)).format("%Y-%m-%d").to_string();
    let expiring = connection
        .prepare(
            "SELECT id, number, type, contactFullName, reservedUntil FROM invoices
             WHERE status = 'pending' AND reservedUntil IS NOT NULL AND reservedUntil <= ?1
             ORDER BY reservedUntil, id",
        )?
        .query_map([limit], |row| {
            Ok(ExpiringReservation {
                id: row.get(0)?,
                number: row.get(1)?,
                invoice_type: row.get(2)?,
                contact_name: row.get(3)?,
                reserved_until: row.get(4)?,
            })
        })?
        .collect::<rusqlite::Result<_>>()?;
    Ok(ReservationsSummary {
        count,
        amount_cents,
        expiring,
    })
}

fn build_dashboard_kpis(
    connection: &Connection,
    today: NaiveDate,
    from: NaiveDate,
    to: NaiveDate,
) -> rusqlite::Result<DashboardKpis> {
    let lines = read_income_lines(connection)?;
    let expenses = read_expenses(connection)?;
    let days = (to - from).num_days() + 1;
    let previous_to = from - Duration::days(1);
    let previous_from = previous_to - Duration::days(days - 1);
    let current = period_totals(&lines, &expenses, from, to);
    let previous = period_totals(&lines, &expenses, previous_from, previous_to);
    let buckets = period_buckets(from, to);
    let series: Vec<Totals> = buckets
        .iter()
        .map(|(_, start, end)| period_totals(&lines, &expenses, *start, *end))
        .collect();
    let kpi = |value: fn(&Totals) -> i64| KpiSeries {
        value: value(&current),
        previous: Some(value(&previous)),
        trend: series.iter().map(value).collect(),
    };
    let daily_sales =
        |sales: i64, days: i64| round_half_up(i128::from(sales) * 1_000, i128::from(days)) as i64;

    let in_period: Vec<&IncomeLine> = lines
        .iter()
        .filter(|line| line.paid_on >= from && line.paid_on <= to)
        .collect();
    let products: Vec<(i64, ProductStats)> = {
        let mut groups: HashMap<i64, ProductStats> = HashMap::new();
        for line in &in_period {
            let stats = groups.entry(line.product_key).or_default();
            stats.name.clone_from(&line.product_name);
            stats.quantity_milli += line.quantity_milli;
            stats.income += line.subtotal_cents;
            stats.cost += line.cost_cents;
        }
        groups.into_iter().collect()
    };
    let sold: HashMap<i64, i64> = products
        .iter()
        .map(|(id, stats)| (*id, stats.quantity_milli))
        .collect();
    let clients: Vec<ClientStats> = {
        let mut groups: HashMap<&str, ClientStats> = HashMap::new();
        for line in &in_period {
            let stats = groups.entry(&line.client_key).or_default();
            stats.name.clone_from(&line.client_name);
            stats.invoices.insert(line.invoice_id);
            stats.total += line.subtotal_cents;
        }
        groups.into_values().collect()
    };

    let mut best_sellers: Vec<&ProductStats> = products.iter().map(|(_, stats)| stats).collect();
    sort_spanish(&mut best_sellers, |stats| {
        (-stats.quantity_milli, spanish_sort_key(&stats.name))
    });
    let mut frequent: Vec<&ClientStats> = clients.iter().collect();
    sort_spanish(&mut frequent, |stats| {
        (
            -(stats.invoices.len() as i64),
            -stats.total,
            spanish_sort_key(&stats.name),
        )
    });

    let inventory_value = inventory_value_cents(connection)?;
    let (inventory_milli, out_of_stock, average_profit): (i64, i64, f64) = connection.query_row(
        "SELECT COALESCE(SUM(quantityMilli), 0),
                COALESCE(SUM(quantityMilli = 0), 0),
                COALESCE(AVG(profitPercentHundredths), 0.0)
         FROM products WHERE status = 'active'",
        [],
        |row| Ok((row.get(0)?, row.get(1)?, row.get(2)?)),
    )?;
    let without_history = |value: i64| KpiSeries {
        value,
        previous: None,
        trend: Vec::new(),
    };
    let spent = |kind: &str| -> i64 {
        expenses
            .iter()
            .filter(|row| row.invoice_type == kind && row.issued_on >= from && row.issued_on <= to)
            .map(|row| row.cents)
            .sum()
    };

    Ok(DashboardKpis {
        from_date: from.format("%Y-%m-%d").to_string(),
        to_date: to.format("%Y-%m-%d").to_string(),
        income: kpi(|totals| totals.income),
        sales: kpi(|totals| totals.sales),
        daily_sales_milli: KpiSeries {
            value: daily_sales(current.sales, days),
            previous: Some(daily_sales(previous.sales, days)),
            trend: series
                .iter()
                .zip(&buckets)
                .map(|(totals, (_, start, end))| {
                    daily_sales(totals.sales, (*end - *start).num_days() + 1)
                })
                .collect(),
        },
        daily_sales_amount_cents: round_half_up(i128::from(current.sales_amount), i128::from(days))
            as i64,
        net_profit: kpi(Totals::net_profit),
        margin_hundredths: kpi(Totals::margin_hundredths),
        gross_margin_cents: current.income - current.cost,
        average_profit_hundredths: average_profit.round() as i64,
        expenses: kpi(|totals| totals.expenses),
        inventory_value: without_history(inventory_value),
        out_of_stock: without_history(out_of_stock),
        inventory_quantity: format_quantity_milli(inventory_milli),
        buckets: buckets
            .iter()
            .zip(&series)
            .map(|((label, _, _), totals)| IncomeBucket {
                label: label.clone(),
                sales_cents: totals.sales_amount,
                services_cents: totals.income - totals.sales_amount,
            })
            .collect(),
        guarantee_cents: spent("GG"),
        installation_cents: spent("GI"),
        loss_cents: spent("P"),
        best_sellers: best_sellers
            .iter()
            .take(10)
            .map(|stats| BestSeller {
                name: stats.name.clone(),
                quantity: format_quantity_milli(stats.quantity_milli),
                margin_cents: stats.income - stats.cost,
            })
            .collect(),
        frequent_clients: frequent
            .iter()
            .take(10)
            .map(|stats| FrequentClient {
                name: stats.name.clone(),
                purchases: stats.invoices.len() as i64,
                total_cents: stats.total,
            })
            .collect(),
        star_products: pareto(
            products
                .iter()
                .map(|(_, stats)| (stats.name.clone(), stats.income - stats.cost))
                .collect(),
        ),
        star_clients: pareto(
            clients
                .iter()
                .map(|stats| (stats.name.clone(), stats.total))
                .collect(),
        ),
        slow_movers: slow_movers(connection, &sold, today)?,
        reservations: reservations_summary(connection, today)?,
    })
}

/// Valor del inventario: stock × costo de los productos activos, con un
/// redondeo por producto como "Valor Total" (`spec-06` VI-01).
fn inventory_value_cents(connection: &Connection) -> rusqlite::Result<i64> {
    connection
        .prepare(
            "SELECT quantityMilli * costCents FROM products
             WHERE status = 'active' AND quantityMilli > 0",
        )?
        .query_map([], |row| {
            Ok(round_half_up(i128::from(row.get::<_, i64>(0)?), 1_000) as i64)
        })?
        .sum()
}

pub fn get_dashboard_kpis_inner(
    connection: &mut Connection,
    now: DateTime<Utc>,
    period: DashboardPeriod,
) -> Result<DashboardKpis, AppError> {
    require_session(connection, now)?;
    let today = today_local(now);
    let (from, to) = period_range(&period, today)?;
    // Las reservas vencidas se cancelan antes de cada operación (D-09).
    purge_best_effort(connection, now)?;
    build_dashboard_kpis(connection, today, from, to)
        .map_err(|error| map_db_error(error, "getDashboardKpis", &[]))
}

#[tauri::command]
pub fn get_dashboard_kpis(
    state: tauri::State<'_, std::sync::Mutex<Connection>>,
    period: DashboardPeriod,
) -> Result<DashboardKpis, AppError> {
    let mut connection = lock_conn(&state);
    let now = Utc::now();
    let result = get_dashboard_kpis_inner(&mut connection, now, period);
    if let Err(error) = &result {
        if error.should_log() {
            if let Some(app_data_dir) = current_app_data_dir() {
                log_error(
                    &connection,
                    "dashboard",
                    "getDashboardKpis",
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
    use super::{get_dashboard_kpis_inner, get_dashboard_summary_inner, pareto};
    use crate::db::connection::initialize_database;
    use crate::models::dashboard::{DashboardKpis, DashboardPeriod};
    use crate::models::product::format_quantity_milli;
    use crate::test_utils::{
        assert_allows_both_roles, assert_requires_session, create_test_session, create_test_user,
        insert_product_fixture, setup_test_db,
    };
    use crate::timestamps::parse_utc_timestamp;
    use chrono::{DateTime, Utc};
    use rusqlite::Connection;
    use std::time::{SystemTime, UNIX_EPOCH};

    #[test]
    fn get_dashboard_summary_counts_only_active_out_of_stock_products() {
        let unique = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .unwrap()
            .as_nanos();
        let app_data_dir = std::env::temp_dir().join(format!("goyma-dashboard-{unique}"));
        std::fs::create_dir_all(&app_data_dir).unwrap();
        let mut conn = initialize_database(&app_data_dir, Utc::now()).unwrap();

        conn.execute(
            "INSERT INTO users (username, passwordHash, role, phone, nationalId, createdAt) VALUES ('tester', 'hash', 'contador', '+5355555555', '00000000001', '2025-01-01T00:00:00Z')",
            [],
        )
        .unwrap();
        let user_id = conn.last_insert_rowid();
        let now = Utc::now();
        conn.execute(
            "INSERT INTO sessions (id, userId, rememberSession, createdAt, lastActivityAt) VALUES (1, ?1, 0, ?2, ?2)",
            rusqlite::params![user_id, now.to_rfc3339_opts(chrono::SecondsFormat::Secs, true)],
        )
        .unwrap();

        conn.execute(
            "INSERT INTO categories (id, name, nameNormalized, createdAt, updatedAt) VALUES (1, 'Lácteos', 'lacteos', '2025-01-01T00:00:00Z', '2025-01-01T00:00:00Z')",
            [],
        )
        .unwrap();
        conn.execute(
            "INSERT INTO categories (id, name, nameNormalized, createdAt, updatedAt) VALUES (2, 'Bebidas', 'bebidas', '2025-01-01T00:00:00Z', '2025-01-01T00:00:00Z')",
            [],
        )
        .unwrap();

        conn.execute(
            "INSERT INTO products (name, nameNormalized, categoryId, unitId, costCents, profitPercentHundredths, quantityMilli, status, discontinuedAt, createdAt, updatedAt) VALUES ('Leche', 'leche', 1, NULL, 100, 0, 0, 'active', NULL, '2025-01-01T00:00:00Z', '2025-01-01T00:00:00Z')",
            [],
        ).unwrap();
        conn.execute(
            "INSERT INTO products (name, nameNormalized, categoryId, unitId, costCents, profitPercentHundredths, quantityMilli, status, discontinuedAt, createdAt, updatedAt) VALUES ('Soda', 'soda', 2, NULL, 100, 0, 0, 'active', NULL, '2025-01-01T00:00:00Z', '2025-01-01T00:00:00Z')",
            [],
        ).unwrap();
        conn.execute(
            "INSERT INTO products (name, nameNormalized, categoryId, unitId, costCents, profitPercentHundredths, quantityMilli, status, discontinuedAt, createdAt, updatedAt) VALUES ('Cafe', 'cafe', NULL, NULL, 100, 0, 0, 'active', NULL, '2025-01-01T00:00:00Z', '2025-01-01T00:00:00Z')",
            [],
        ).unwrap();
        conn.execute(
            "INSERT INTO products (name, nameNormalized, categoryId, unitId, costCents, profitPercentHundredths, quantityMilli, status, discontinuedAt, createdAt, updatedAt) VALUES ('Viejo', 'viejo', 1, NULL, 100, 0, 0, 'active', NULL, '2025-01-01T00:00:00Z', '2025-01-01T00:00:00Z')",
            [],
        ).unwrap();
        conn.execute(
            "INSERT INTO products (name, nameNormalized, categoryId, unitId, costCents, profitPercentHundredths, quantityMilli, status, discontinuedAt, createdAt, updatedAt) VALUES ('Desc', 'desc', 1, NULL, 100, 0, 0, 'discontinued', '2025-01-01T00:00:00Z', '2025-01-01T00:00:00Z', '2025-01-01T00:00:00Z')",
            [],
        ).unwrap();

        let summary = get_dashboard_summary_inner(&mut conn, Utc::now()).unwrap();

        assert_eq!(summary.out_of_stock_total, 4);
        assert_eq!(summary.without_category_count, 1);
        assert_eq!(summary.top_categories.len(), 2);
        assert_eq!(summary.top_categories[0].name, "Lácteos");
        assert_eq!(summary.top_categories[0].count, 2);
        assert_eq!(summary.top_categories[1].name, "Bebidas");
        assert_eq!(summary.top_categories[1].count, 1);
    }

    // ---------- getDashboardKpis (spec-04 DB-01 a DB-07) ----------

    const NOW: &str = "2026-11-18T15:00:00Z";

    fn now() -> DateTime<Utc> {
        parse_utc_timestamp(NOW).unwrap()
    }

    fn period(preset: &str) -> DashboardPeriod {
        DashboardPeriod {
            preset: preset.to_string(),
            from_date: None,
            to_date: None,
        }
    }

    fn custom(from: &str, to: &str) -> DashboardPeriod {
        DashboardPeriod {
            preset: "custom".to_string(),
            from_date: Some(from.to_string()),
            to_date: Some(to.to_string()),
        }
    }

    struct Fixture<'a> {
        kind: &'a str,
        number: &'a str,
        date: &'a str,
        status: Option<&'a str>,
        paid_at: Option<&'a str>,
        client: Option<(&'a str, &'a str)>,
        reserved_until: Option<&'a str>,
    }

    impl Default for Fixture<'_> {
        fn default() -> Self {
            Self {
                kind: "V",
                number: "",
                date: "2026-01-01",
                status: Some("completed"),
                paid_at: None,
                client: Some(("Ana Pérez", "90010112345")),
                reserved_until: None,
            }
        }
    }

    fn insert_invoice(connection: &Connection, fixture: Fixture) -> i64 {
        let associated = matches!(fixture.kind, "GG" | "GI").then_some("S-00001");
        let associated_type = associated.map(|_| "S");
        let (name, national_id) = fixture.client.unzip();
        let phone = name.map(|_| "+5355555555");
        connection
            .execute(
                "INSERT INTO invoices (type, number, invoiceDate, status, paidAt, contactFullName,
                    contactFullNameNormalized, contactNationalId, contactPhone, associatedNumber,
                    associatedType, reservedUntil, createdByName, createdAt, updatedAt)
                 VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?6, ?7, ?8, ?9, ?10, ?11, 'tester', ?12, ?12)",
                rusqlite::params![
                    fixture.kind,
                    fixture.number,
                    fixture.date,
                    fixture.status,
                    fixture.paid_at,
                    name,
                    national_id,
                    phone,
                    associated,
                    associated_type,
                    fixture.reserved_until,
                    NOW,
                ],
            )
            .unwrap();
        connection.last_insert_rowid()
    }

    /// Línea con su consumo FIFO (`cost` = costo por unidad del lote).
    fn insert_line(
        connection: &Connection,
        invoice_id: i64,
        product: (i64, &str),
        quantity_milli: i64,
        subtotal_cents: i64,
        cost: i64,
    ) {
        connection
            .execute(
                "INSERT INTO invoiceLines (invoiceId, position, productIdOriginal, productName,
                    productNameNormalized, quantityMilli, unitPriceCents, subtotalCents, createdAt,
                    updatedAt)
                 VALUES (?1, (SELECT COUNT(*) + 1 FROM invoiceLines WHERE invoiceId = ?1), ?2, ?3,
                    ?3, ?4, ?5, ?5, ?6, ?6)",
                rusqlite::params![
                    invoice_id,
                    product.0,
                    product.1,
                    quantity_milli,
                    subtotal_cents,
                    NOW,
                ],
            )
            .unwrap();
        let line_id = connection.last_insert_rowid();
        connection
            .execute(
                "INSERT INTO consumptions (invoiceLineId, quantityMilli, unitCostCents, createdAt)
                 VALUES (?1, ?2, ?3, ?4)",
                rusqlite::params![line_id, quantity_milli, cost, NOW],
            )
            .unwrap();
    }

    fn insert_lot(connection: &Connection, product_id: i64, remaining_milli: i64, cost: i64) {
        connection
            .execute(
                "INSERT INTO lots (productId, origin, arrivalOrder, initialMilli, remainingMilli,
                    unitCostCents, createdAt, updatedAt)
                 VALUES (?1, 'initial', 1, ?2, ?2, ?3, ?4, ?4)",
                rusqlite::params![product_id, remaining_milli, cost, NOW],
            )
            .unwrap();
    }

    /// Escenario de task-04 §4. Hoy es miércoles 2026-11-18.
    fn scenario() -> Connection {
        let connection = setup_test_db();
        let user_id = create_test_user(
            &connection,
            "kpiuser",
            "contador",
            "+5350000010",
            "12345678910",
            "ValidPass1!",
        );
        create_test_session(&connection, user_id, false, now(), now());

        let product = |name, cost, profit, quantity, status, discontinued| {
            insert_product_fixture(
                &connection,
                name,
                None,
                None,
                cost,
                profit,
                quantity,
                status,
                discontinued,
                None,
            )
        };
        let cable = product("Cable", 1000, 5000, 2000, "active", None);
        let bomba = product("Bomba", 2000, 10000, 1500, "active", None);
        let nunca = product("Nunca", 500, 2000, 0, "active", None);
        let retirado = product(
            "Retirado",
            500,
            0,
            1000,
            "discontinued",
            Some("2026-10-03T12:00:00Z"),
        );
        connection
            .execute(
                "UPDATE products SET createdAt = '2026-10-03T12:00:00Z' WHERE id = ?1",
                [nunca],
            )
            .unwrap();
        insert_lot(&connection, cable, 2000, 1000);
        insert_lot(&connection, bomba, 1500, 1333);
        insert_lot(&connection, retirado, 1000, 500);
        let (cable, bomba, viejo) = ((cable, "Cable"), (bomba, "Bomba"), (999, "Viejo"));
        // Cliente sin ficha: solo queda la copia guardada en la factura.
        let luis = Some(("Luis Gómez", "85020254321"));

        let invoice = insert_invoice(
            &connection,
            Fixture {
                number: "V-00001",
                date: "2026-11-10",
                paid_at: Some("2026-11-18T12:00:00Z"),
                ..Fixture::default()
            },
        );
        insert_line(&connection, invoice, cable, 2000, 3000, 1000);
        insert_line(&connection, invoice, bomba, 1000, 4000, 2000);
        let invoice = insert_invoice(
            &connection,
            Fixture {
                kind: "S",
                number: "S-00001",
                date: "2026-11-17",
                paid_at: Some("2026-11-17T12:00:00Z"),
                ..Fixture::default()
            },
        );
        insert_line(&connection, invoice, bomba, 1000, 5000, 2000);
        let invoice = insert_invoice(
            &connection,
            Fixture {
                number: "V-00002",
                date: "2026-11-10",
                paid_at: Some("2026-11-10T12:00:00Z"),
                client: luis,
                ..Fixture::default()
            },
        );
        insert_line(&connection, invoice, viejo, 1000, 1000, 900);
        let invoice = insert_invoice(
            &connection,
            Fixture {
                number: "V-00003",
                date: "2026-09-01",
                paid_at: Some("2026-09-01T12:00:00Z"),
                client: luis,
                ..Fixture::default()
            },
        );
        insert_line(&connection, invoice, cable, 1000, 1500, 1000);
        // Por Cobrar y Cancelada no suman; las dos primeras son reservas.
        let invoice = insert_invoice(
            &connection,
            Fixture {
                number: "V-00004",
                date: "2026-11-17",
                status: Some("pending"),
                reserved_until: Some("2026-11-20"),
                ..Fixture::default()
            },
        );
        insert_line(&connection, invoice, cable, 1000, 9999, 1000);
        let invoice = insert_invoice(
            &connection,
            Fixture {
                number: "V-00005",
                date: "2026-11-17",
                status: Some("pending"),
                reserved_until: Some("2026-12-30"),
                ..Fixture::default()
            },
        );
        insert_line(&connection, invoice, cable, 1000, 100, 1000);
        let invoice = insert_invoice(
            &connection,
            Fixture {
                number: "V-00006",
                date: "2026-11-18",
                status: Some("cancelled"),
                paid_at: Some("2026-11-18T12:00:00Z"),
                ..Fixture::default()
            },
        );
        insert_line(&connection, invoice, cable, 1000, 5555, 1000);

        for (kind, number, date, status, cents) in [
            ("GG", "GG-00001", "2026-11-18", Some("completed"), 300),
            ("GI", "GI-00001", "2026-11-17", Some("completed"), 200),
            ("GG", "GG-00002", "2026-11-18", Some("pending"), 777),
            ("P", "P-00001", "2026-11-16", None, 100),
        ] {
            let invoice = insert_invoice(
                &connection,
                Fixture {
                    kind,
                    number,
                    date,
                    status,
                    client: (kind != "P").then_some(("Ana Pérez", "90010112345")),
                    ..Fixture::default()
                },
            );
            insert_line(&connection, invoice, cable, 1000, cents, cents);
        }
        connection
    }

    fn kpis(connection: &mut Connection, period: DashboardPeriod) -> DashboardKpis {
        get_dashboard_kpis_inner(connection, now(), period).unwrap()
    }

    #[test]
    fn kpis_add_paid_income_by_payment_date_for_each_period() {
        let mut connection = scenario();

        // (período, desde, ingresos, ventas, gastos, ganancia neta, margen)
        for (preset, from, income, sales, expenses, profit, margin) in [
            ("today", "2026-11-18", 7000, 1, 300, 2700, 4286),
            ("week", "2026-11-16", 12000, 1, 600, 5400, 5000),
            ("month", "2026-11-01", 13000, 2, 600, 5500, 4692),
            ("3m", "2026-08-19", 14500, 3, 600, 6000, 4552),
            ("6m", "2026-05-19", 14500, 3, 600, 6000, 4552),
            ("1y", "2025-11-19", 14500, 3, 600, 6000, 4552),
        ] {
            let result = kpis(&mut connection, period(preset));
            assert_eq!(result.from_date, from, "{preset}");
            assert_eq!(result.to_date, "2026-11-18", "{preset}");
            assert_eq!(result.income.value, income, "{preset}");
            assert_eq!(result.sales.value, sales, "{preset}");
            assert_eq!(result.expenses.value, expenses, "{preset}");
            assert_eq!(result.net_profit.value, profit, "{preset}");
            assert_eq!(result.margin_hundredths.value, margin, "{preset}");
        }

        let result = kpis(&mut connection, custom("2026-09-01", "2026-09-01"));
        assert_eq!(result.income.value, 1500);
        assert_eq!(result.net_profit.value, 500);
        assert_eq!(result.expenses.value, 0);
    }

    #[test]
    fn kpis_compare_with_the_previous_period_of_equal_length() {
        let mut connection = scenario();

        let today = kpis(&mut connection, period("today"));
        assert_eq!(today.income.previous, Some(5000));
        assert_eq!(today.sales.previous, Some(0));
        assert_eq!(today.expenses.previous, Some(200));
        assert_eq!(today.net_profit.previous, Some(2800));
        assert_eq!(today.margin_hundredths.previous, Some(6000));

        // Semana del 16 al 18: la anterior es del 13 al 15, sin movimientos.
        let week = kpis(&mut connection, period("week"));
        assert_eq!(week.income.previous, Some(0));
        assert_eq!(week.margin_hundredths.previous, Some(0));

        // Rango del 10 al 11: el anterior es del 8 al 9.
        let range = kpis(&mut connection, custom("2026-11-10", "2026-11-11"));
        assert_eq!(range.income.value, 1000);
        assert_eq!(range.income.trend, vec![1000, 0]);
        assert_eq!(range.income.previous, Some(0));
    }

    #[test]
    fn kpis_average_daily_sales_over_the_days_of_the_period() {
        let mut connection = scenario();

        let week = kpis(&mut connection, period("week"));
        assert_eq!(week.daily_sales_milli.value, 333);
        assert_eq!(week.daily_sales_amount_cents, 2333);
        assert_eq!(week.daily_sales_milli.trend, vec![0, 0, 1000]);

        let month = kpis(&mut connection, period("month"));
        assert_eq!(month.daily_sales_milli.value, 111);
        assert_eq!(month.daily_sales_amount_cents, 444);
    }

    #[test]
    fn kpis_group_income_by_day_week_or_month_with_length() {
        let mut connection = scenario();

        let week = kpis(&mut connection, period("week"));
        let days: Vec<_> = week
            .buckets
            .iter()
            .map(|b| (b.label.as_str(), b.sales_cents, b.services_cents))
            .collect();
        assert_eq!(
            days,
            vec![("16/11", 0, 0), ("17/11", 0, 5000), ("18/11", 7000, 0)]
        );
        assert_eq!(kpis(&mut connection, period("today")).buckets.len(), 1);

        let quarter = kpis(&mut connection, period("3m"));
        assert_eq!(quarter.buckets.len(), 14);
        assert_eq!(quarter.buckets[0].label, "19/08");

        let year = kpis(&mut connection, period("1y"));
        assert_eq!(year.buckets.len(), 13);
        assert_eq!(year.buckets[0].label, "11/2025");
        assert_eq!(year.buckets[12].label, "11/2026");
        assert_eq!(year.buckets[12].sales_cents, 8000);
        assert_eq!(year.buckets[12].services_cents, 5000);
    }

    #[test]
    fn kpis_split_expenses_and_exclude_pending_guarantees() {
        let mut connection = scenario();

        let week = kpis(&mut connection, period("week"));
        assert_eq!(week.guarantee_cents, 300);
        assert_eq!(week.installation_cents, 200);
        assert_eq!(week.loss_cents, 100);
    }

    #[test]
    fn kpis_report_inventory_of_active_products_without_history() {
        let mut connection = scenario();

        let result = kpis(&mut connection, period("month"));
        // Stock × costo (spec-06 VI-01): Cable 2 × 10,00 y Bomba 1,5 × 20,00.
        assert_eq!(result.inventory_value.value, 5000);
        assert_eq!(result.inventory_value.previous, None);
        assert!(result.inventory_value.trend.is_empty());
        assert_eq!(result.out_of_stock.value, 1);
        assert_eq!(result.out_of_stock.previous, None);
        assert_eq!(result.inventory_quantity, format_quantity_milli(3500));
        assert_eq!(result.gross_margin_cents, 6100);
        assert_eq!(result.average_profit_hundredths, 5667);
    }

    #[test]
    fn kpis_rank_products_and_clients_with_saved_names() {
        let mut connection = scenario();

        let result = kpis(&mut connection, period("month"));
        let sellers: Vec<_> = result
            .best_sellers
            .iter()
            .map(|s| (s.name.as_str(), s.margin_cents))
            .collect();
        assert_eq!(
            sellers,
            vec![("Bomba", 5000), ("Cable", 1000), ("Viejo", 100)]
        );
        assert_eq!(result.best_sellers[0].quantity, format_quantity_milli(2000));

        let clients: Vec<_> = result
            .frequent_clients
            .iter()
            .map(|c| (c.name.as_str(), c.purchases, c.total_cents))
            .collect();
        assert_eq!(
            clients,
            vec![("Ana Pérez", 2, 12000), ("Luis Gómez", 1, 1000)]
        );

        // Pareto: Bomba sola supera el 80 % de 61,00 de ganancia.
        let star = &result.star_products;
        assert_eq!(star.len(), 1);
        assert_eq!((star[0].name.as_str(), star[0].percent), ("Bomba", 82));
        let star = &result.star_clients;
        assert_eq!(star.len(), 1);
        assert_eq!((star[0].name.as_str(), star[0].percent), ("Ana Pérez", 92));
    }

    #[test]
    fn pareto_accumulates_until_eighty_percent_and_skips_losses() {
        let items = vec![
            ("Uno".to_string(), 50),
            ("Dos".to_string(), 30),
            ("Tres".to_string(), 20),
            ("Pierde".to_string(), -10),
        ];
        let names: Vec<_> = pareto(items).into_iter().map(|s| s.name).collect();
        assert_eq!(names, vec!["Uno", "Dos"]);
        assert!(pareto(vec![("Nada".to_string(), 0)]).is_empty());
    }

    #[test]
    fn kpis_list_slow_movers_from_least_sold() {
        let mut connection = scenario();

        let result = kpis(&mut connection, custom("2026-09-01", "2026-09-30"));
        let rows: Vec<_> = result
            .slow_movers
            .iter()
            .map(|s| (s.name.as_str(), s.selling_price_cents, s.days_without_sale))
            .collect();
        // S-09: Nunca cuenta desde que se creó; el descontinuado no aparece.
        assert_eq!(
            rows,
            vec![("Nunca", 600, 46), ("Bomba", 4000, 0), ("Cable", 1500, 0)]
        );
        assert_eq!(result.slow_movers[0].sold, format_quantity_milli(0));
        assert_eq!(result.slow_movers[2].sold, format_quantity_milli(1000));
    }

    #[test]
    fn kpis_summarize_active_reservations_and_those_about_to_expire() {
        let mut connection = scenario();

        let reservations = kpis(&mut connection, period("today")).reservations;
        assert_eq!(reservations.count, 2);
        assert_eq!(reservations.amount_cents, 10099);
        assert_eq!(reservations.expiring.len(), 1);
        let expiring = &reservations.expiring[0];
        assert_eq!(expiring.number, "V-00004");
        assert_eq!(expiring.invoice_type, "V");
        assert_eq!(expiring.contact_name, "Ana Pérez");
        assert_eq!(expiring.reserved_until, "2026-11-20");
    }

    #[test]
    fn kpis_reject_invalid_periods_with_a_clear_message() {
        let mut connection = scenario();

        for (candidate, message) in [
            (period("custom"), "Elige la fecha inicial y la final"),
            (
                custom("2026-11-10", "2026-11-01"),
                "La fecha inicial no puede ser posterior a la final",
            ),
            (period("decade"), "El período no es válido"),
        ] {
            let error = get_dashboard_kpis_inner(&mut connection, now(), candidate).unwrap_err();
            assert_eq!(error.code, "validationFailed");
            assert_eq!(error.message, message);
        }
    }

    #[test]
    fn get_dashboard_kpis_requires_session_and_allows_both_roles() {
        assert_requires_session(|connection, now| {
            get_dashboard_kpis_inner(connection, now, period("month"))
        });
        assert_allows_both_roles(|connection, now| {
            get_dashboard_kpis_inner(connection, now, period("month"))
        });
    }
}
