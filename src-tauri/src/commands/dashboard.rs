use chrono::{DateTime, Utc};
use rusqlite::Connection;

use crate::commands::auth::require_session;
use crate::db::connection::{current_app_data_dir, lock_conn};
use crate::errors::{log_error, map_db_error, AppError};
use crate::models::product::{DashboardSummary, OutOfStockCategory};
use crate::text::spanish_sort_key;

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

#[cfg(test)]
mod tests {
    use super::get_dashboard_summary_inner;
    use crate::db::connection::initialize_database;
    use chrono::Utc;
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
}
