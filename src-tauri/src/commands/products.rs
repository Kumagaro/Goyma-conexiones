use std::path::{Path, PathBuf};

use chrono::{DateTime, Duration, Utc};
use rusqlite::Connection;

use crate::db::connection::current_app_data_dir;
use crate::errors::{log_error, map_db_error, AppError};
use crate::timestamps::format_utc_timestamp;

const PURGE_SAVEPOINT: &str = "expired_products_purge";
const PURGE_FAILED_WITH_COUNT: &str =
    "No se pudieron limpiar {count} productos vencidos; se reintentará en la próxima operación";
const PURGE_FAILED_WITHOUT_COUNT: &str =
    "No se pudieron limpiar los productos vencidos; se reintentará en la próxima operación";

pub fn expired_products_cutoff(now: DateTime<Utc>) -> String {
    format_utc_timestamp(now - Duration::days(60))
}

pub fn purge_expired_products(
    connection: &Connection,
    now: DateTime<Utc>,
) -> Result<usize, AppError> {
    let cutoff = expired_products_cutoff(now);
    connection
        .execute(
            "DELETE FROM products
             WHERE status = 'discontinued' AND discontinuedAt <= ?1",
            [&cutoff],
        )
        .map_err(|error| map_db_error(error, "purge_expired_products", &[]))
}

pub fn purge_best_effort(
    connection: &Connection,
    now: DateTime<Utc>,
) -> Result<Option<String>, AppError> {
    let fallback_directory: PathBuf = std::env::temp_dir();
    let app_data_dir = current_app_data_dir().unwrap_or(&fallback_directory);
    purge_best_effort_with_app_data_dir(connection, now, app_data_dir)
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

    use chrono::{Duration, TimeZone, Utc};
    use rusqlite::Connection;

    use crate::test_utils::{
        create_fail_product_delete_trigger, insert_product_fixture, setup_test_db,
    };
    use crate::timestamps::format_utc_timestamp;

    use super::{
        expired_products_cutoff, purge_best_effort, purge_best_effort_with_app_data_dir,
        purge_expired_products,
    };

    static NEXT_TEST_PATH: AtomicU64 = AtomicU64::new(0);

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
                "INSERT INTO notifications (type, priority, message, createdAt) VALUES ('test', 'test', 'usable', '2026-10-03T12:00:00Z')",
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
