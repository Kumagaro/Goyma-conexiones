//! Procesos automáticos de `spec-04` (`plan-04` §3.1): avisos de situación
//! (NT-02, NT-03) que se evalúan al arrancar y al final de cada operación que
//! cambie inventario o facturas.

use std::collections::HashSet;
use std::path::PathBuf;

use chrono::{DateTime, Duration, NaiveDate, Utc};
use rusqlite::{Connection, OptionalExtension};

use crate::commands::drafts::purge_old_drafts;
use crate::commands::products::{purge_best_effort, remaining_product_days};
use crate::db::connection::current_app_data_dir;
use crate::db::notifications::{
    notify, watch_enter, watch_leave, Audience, Entity, CRITICAL, WARNING,
};
use crate::errors::{log_error, map_db_error, AppError};
use crate::models::product::format_quantity_milli;
use crate::timestamps::{format_utc_timestamp, parse_utc_timestamp, today_local};

const WATCH_SAVEPOINT: &str = "notification_watches";

/// Umbrales de los avisos (CF-04).
#[derive(Clone, Copy, Debug)]
pub struct Thresholds {
    /// Factura pendiente antigua: más de estos días desde la emisión (N-10).
    pub stale_invoice_days: i64,
}

impl Default for Thresholds {
    fn default() -> Self {
        Self {
            stale_invoice_days: 30,
        }
    }
}

/// Umbrales globales de CF-04 guardados en `settings` (migración 4); si
/// falta una clave, su valor por defecto.
pub fn thresholds(connection: &Connection) -> Thresholds {
    let defaults = Thresholds::default();
    let read = |key: &str, default: i64| {
        connection
            .query_row(
                "SELECT CAST(value AS INTEGER) FROM settings WHERE key = ?1",
                [key],
                |row| row.get(0),
            )
            .unwrap_or(default)
    };
    Thresholds {
        stale_invoice_days: read("staleInvoiceDays", defaults.stale_invoice_days),
    }
}

/// Una entidad que hoy está en la situación vigilada y el aviso que le toca.
struct Situation {
    key: String,
    priority: &'static str,
    message: String,
    entity: Option<Entity>,
}

/// Avisa de las situaciones nuevas y olvida las que terminaron (NT-03).
fn sync_watches(
    connection: &Connection,
    kind: &str,
    situations: Vec<Situation>,
    now: DateTime<Utc>,
) -> rusqlite::Result<()> {
    let current: HashSet<String> = situations.iter().map(|s| s.key.clone()).collect();
    for situation in situations {
        if watch_enter(connection, kind, &situation.key, now)? {
            notify(
                connection,
                kind,
                situation.priority,
                &situation.message,
                situation.entity,
                Audience::All,
                now,
            )?;
        }
    }
    let watched: Vec<String> = connection
        .prepare("SELECT entityKey FROM notificationWatches WHERE kind = ?1")?
        .query_map([kind], |row| row.get(0))?
        .collect::<rusqlite::Result<_>>()?;
    for key in watched.iter().filter(|key| !current.contains(*key)) {
        watch_leave(connection, kind, key)?;
    }
    Ok(())
}

fn product_key(id: i64) -> String {
    format!("p:{id}")
}

fn invoice_key(id: i64) -> String {
    format!("i:{id}")
}

fn display_date(date: NaiveDate) -> String {
    date.format("%d/%m/%Y").to_string()
}

/// Evalúa todos los avisos de situación de productos y facturas.
pub fn evaluate_watches(
    connection: &Connection,
    now: DateTime<Utc>,
    thresholds: Thresholds,
) -> Result<(), AppError> {
    let today = today_local(now);
    evaluate_product_watches(connection, now, today)
        .and_then(|()| evaluate_invoice_watches(connection, now, today, thresholds))
        .map_err(|error| {
            AppError::new(
                "internal",
                "Ocurrió un error inesperado",
                &format!("failed to evaluate notification watches: {error}"),
            )
        })
}

fn evaluate_product_watches(
    connection: &Connection,
    now: DateTime<Utc>,
    today: NaiveDate,
) -> rusqlite::Result<()> {
    // (id, nombre, cantidad, unidad, caducidad, estado, descontinuado, umbral)
    type Row = (
        i64,
        String,
        i64,
        Option<String>,
        Option<String>,
        String,
        Option<String>,
        Option<i64>,
    );
    let rows: Vec<Row> = connection
        .prepare(
            "SELECT p.id, p.name, p.quantityMilli, COALESCE(u.abbreviation, u.name),
                    p.expiresOn, p.status, p.discontinuedAt, p.lowStockThresholdMilli
             FROM products p LEFT JOIN units u ON u.id = p.unitId",
        )?
        .query_map([], |row| {
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
        })?
        .collect::<rusqlite::Result<_>>()?;

    let expiry_limit = today + Duration::days(29);
    let (mut out, mut low, mut expiring, mut purging) = (vec![], vec![], vec![], vec![]);
    for (id, name, quantity, unit, expires_on, status, discontinued_at, threshold) in rows {
        if status == "discontinued" {
            // N-07: menos de 10 días para la eliminación automática (P-08).
            let remaining = discontinued_at
                .as_deref()
                .and_then(|at| remaining_product_days(now, at).ok());
            if let Some(days) = remaining.filter(|days| *days < 10) {
                purging.push(Situation {
                    key: product_key(id),
                    priority: WARNING,
                    message: format!(
                        "Producto descontinuado próximo a eliminar: {name} - Días restantes para eliminación: {days} días"
                    ),
                    entity: Some(Entity::Product(id)),
                });
            }
            continue;
        }
        if quantity == 0 {
            out.push(Situation {
                key: product_key(id),
                priority: CRITICAL,
                message: format!("Producto agotado: {name}"),
                entity: Some(Entity::Product(id)),
            });
        } else if threshold.is_some_and(|threshold| quantity < threshold) {
            let amount = format_quantity_milli(quantity);
            let amount = match &unit {
                Some(unit) => format!("{amount} {unit}"),
                None => amount,
            };
            low.push(Situation {
                key: product_key(id),
                priority: WARNING,
                message: format!("Producto con stock bajo: {name} - Cantidad actual: {amount}"),
                entity: Some(Entity::Product(id)),
            });
        }
        let expiry = expires_on
            .as_deref()
            .and_then(|date| NaiveDate::parse_from_str(date, "%Y-%m-%d").ok());
        if let Some(date) = expiry.filter(|date| *date >= today && *date <= expiry_limit) {
            let days = (date - today).num_days();
            expiring.push(Situation {
                key: product_key(id),
                priority: CRITICAL,
                message: format!(
                    "Producto próximo a vencer: {name} - Caducidad: {} - Días restantes: {days} días",
                    display_date(date)
                ),
                entity: Some(Entity::Product(id)),
            });
        }
    }
    sync_watches(connection, "N-02", out, now)?;
    sync_watches(connection, "N-03", low, now)?;
    sync_watches(connection, "N-04", expiring, now)?;
    sync_watches(connection, "N-07", purging, now)
}

fn evaluate_invoice_watches(
    connection: &Connection,
    now: DateTime<Utc>,
    today: NaiveDate,
    thresholds: Thresholds,
) -> rusqlite::Result<()> {
    // (id, tipo, número, contacto, emisión, reservada hasta)
    type Row = (i64, String, String, String, String, Option<String>);
    let rows: Vec<Row> = connection
        .prepare(
            "SELECT id, type, number, COALESCE(contactFullName, ''), invoiceDate, reservedUntil
             FROM invoices WHERE status = 'pending' AND type IN ('E','V','S')",
        )?
        .query_map([], |row| {
            Ok((
                row.get(0)?,
                row.get(1)?,
                row.get(2)?,
                row.get(3)?,
                row.get(4)?,
                row.get(5)?,
            ))
        })?
        .collect::<rusqlite::Result<_>>()?;

    let parse = |date: &str| NaiveDate::parse_from_str(date, "%Y-%m-%d").ok();
    let (mut reserved, mut stale) = (vec![], vec![]);
    for (id, invoice_type, number, contact, invoice_date, reserved_until) in rows {
        // N-05: a la reserva le quedan menos de 3 días y aún no vence.
        if let Some(limit) = reserved_until.as_deref().and_then(parse) {
            let days = (limit - today).num_days();
            if (0..3).contains(&days) {
                reserved.push(Situation {
                    key: invoice_key(id),
                    priority: WARNING,
                    message: format!(
                        "Reserva por vencerse: Factura # {number} - Cliente: {contact} - Días restantes: {days} días"
                    ),
                    entity: Some(Entity::Invoice(id)),
                });
            }
        }
        // N-10: pendiente desde hace más de los días configurados.
        if let Some(issued) = parse(&invoice_date) {
            let days = (today - issued).num_days();
            if days > thresholds.stale_invoice_days {
                let verb = if invoice_type == "E" {
                    "pagar"
                } else {
                    "cobrar"
                };
                stale.push(Situation {
                    key: invoice_key(id),
                    priority: WARNING,
                    message: format!(
                        "Factura pendiente antigua: {number} - {contact} - {days} días sin {verb}"
                    ),
                    entity: Some(Entity::Invoice(id)),
                });
            }
        }
    }
    sync_watches(connection, "N-05", reserved, now)?;
    sync_watches(connection, "N-10", stale, now)
}

/// N-11: más de `reminder_days` sin una copia correcta. Sin ninguna copia se
/// cuenta desde `reference` (el inicio del uso de la base).
pub fn evaluate_backup_watch(
    connection: &Connection,
    now: DateTime<Utc>,
    last_backup_at: Option<DateTime<Utc>>,
    reference: DateTime<Utc>,
    reminder_days: i64,
) -> rusqlite::Result<()> {
    let days = (now - last_backup_at.unwrap_or(reference)).num_days();
    let situations = if days > reminder_days {
        vec![Situation {
            key: "backup".to_owned(),
            priority: WARNING,
            message: format!("No se ha hecho una copia de seguridad en {days} días"),
            entity: None,
        }]
    } else {
        vec![]
    };
    sync_watches(connection, "N-11", situations, now)
}

/// N-11 con los valores guardados en `settings`.
fn evaluate_saved_backup_watch(
    connection: &Connection,
    now: DateTime<Utc>,
) -> Result<(), AppError> {
    let failed = |error| map_db_error(error, "evaluateBackupWatch", &[]);
    let read = |key: &str| -> rusqlite::Result<Option<String>> {
        connection
            .query_row("SELECT value FROM settings WHERE key = ?1", [key], |row| {
                row.get(0)
            })
            .optional()
    };
    let last_backup_at = read("lastBackupAt")
        .map_err(failed)?
        .as_deref()
        .and_then(parse_utc_timestamp);
    let reminder_days = read("backupReminderDays")
        .map_err(failed)?
        .and_then(|value| value.parse().ok())
        .unwrap_or(7);
    let reference: Option<String> = connection
        .query_row("SELECT MIN(createdAt) FROM users", [], |row| row.get(0))
        .map_err(failed)?;
    let reference = reference
        .as_deref()
        .and_then(parse_utc_timestamp)
        .unwrap_or(now);
    evaluate_backup_watch(connection, now, last_backup_at, reference, reminder_days).map_err(failed)
}

pub fn evaluate_backup_watch_best_effort(connection: &Connection, now: DateTime<Utc>) {
    best_effort(
        connection,
        now,
        "backup_watch",
        "evaluate_backup_watch",
        |connection| evaluate_saved_backup_watch(connection, now),
    );
}

/// Evalúa los avisos sin poner en riesgo la operación del usuario: un fallo
/// se deshace con su `SAVEPOINT`, se registra y la operación sigue (como la
/// purga de `spec-02` §7.6).
pub fn evaluate_watches_best_effort(connection: &Connection, now: DateTime<Utc>) {
    best_effort(
        connection,
        now,
        WATCH_SAVEPOINT,
        "evaluate_watches",
        |connection| evaluate_watches(connection, now, thresholds(connection)),
    );
}

/// RS-02: vence las reservas sin poner en riesgo la operación en curso.
pub fn expire_reservations_best_effort(connection: &Connection, now: DateTime<Utc>) {
    best_effort(
        connection,
        now,
        "expire_reservations",
        "expire_reservations",
        |connection| crate::commands::invoices::expire_reservations(connection, now).map(|_| ()),
    );
}

/// Ejecuta un paso automático en su `SAVEPOINT`: si falla, se deshace solo
/// ese paso y se registra el error.
fn best_effort(
    connection: &Connection,
    now: DateTime<Utc>,
    savepoint: &str,
    operation: &str,
    step: impl FnOnce(&Connection) -> Result<(), AppError>,
) {
    let internal = |cause: String| AppError::new("internal", "Ocurrió un error inesperado", &cause);
    let result = connection
        .execute_batch(&format!("SAVEPOINT {savepoint}"))
        .map_err(|error| internal(format!("failed to open {savepoint} savepoint: {error}")))
        .and_then(|()| match step(connection) {
            Ok(()) => connection
                .execute_batch(&format!("RELEASE SAVEPOINT {savepoint}"))
                .map_err(|error| {
                    internal(format!("failed to release {savepoint} savepoint: {error}"))
                }),
            Err(error) => {
                let _ = connection.execute_batch(&format!(
                    "ROLLBACK TO SAVEPOINT {savepoint}; RELEASE SAVEPOINT {savepoint}"
                ));
                Err(error)
            }
        });
    if let Err(error) = result {
        let fallback: PathBuf = std::env::temp_dir();
        let app_data_dir = current_app_data_dir().unwrap_or(&fallback);
        log_error(
            connection,
            "automation",
            operation,
            &error,
            now,
            app_data_dir,
        );
    }
}

/// N-02 "al intentar vender un agotado" (NT-02): la salida rechazada (por
/// `stockInsuficiente` o por producto agotado) ya se deshizo, así que los
/// avisos se evalúan en una transacción aparte (`plan-04` §3.3) y siguen la
/// regla de NT-03.
pub fn after_rejected_outgoing<T>(
    connection: &mut Connection,
    now: DateTime<Utc>,
    result: Result<T, AppError>,
) -> Result<T, AppError> {
    if result.is_err() {
        if let Ok(transaction) = connection.transaction() {
            evaluate_watches_best_effort(&transaction, now);
            let _ = transaction.commit();
        }
    }
    result
}

/// NT-01: una notificación se borra sola a los 90 días de generarse.
pub fn purge_old_notifications(
    connection: &Connection,
    now: DateTime<Utc>,
) -> rusqlite::Result<usize> {
    connection.execute(
        "DELETE FROM notifications WHERE createdAt < ?1",
        [format_utc_timestamp(now - Duration::days(90))],
    )
}

/// Procesos al arrancar (`plan-04` §3.1): purga de descontinuados (con
/// N-12), avisos de situación y notificaciones antiguas. Cada paso es
/// best-effort: un fallo se registra y el siguiente sigue.
pub fn run_startup_processes(connection: &mut Connection, now: DateTime<Utc>) {
    let Ok(transaction) = connection.transaction() else {
        return;
    };
    // Registra su propio fallo; su aviso solo viaja en respuestas de comandos.
    let _ = purge_best_effort(&transaction, now);
    evaluate_watches_best_effort(&transaction, now);
    evaluate_backup_watch_best_effort(&transaction, now);
    let report = |step: &str, result: rusqlite::Result<usize>| {
        let Err(error) = result else { return };
        let error = AppError::new(
            "internal",
            "Ocurrió un error inesperado",
            &format!("failed to {step}: {error}"),
        );
        let fallback: PathBuf = std::env::temp_dir();
        let app_data_dir = current_app_data_dir().unwrap_or(&fallback);
        log_error(&transaction, "automation", step, &error, now, app_data_dir);
    };
    report(
        "purge_old_notifications",
        purge_old_notifications(&transaction, now),
    );
    report("purge_old_drafts", purge_old_drafts(&transaction, now));
    let _ = transaction.commit();
}

#[cfg(test)]
mod tests {
    use chrono::{DateTime, Duration, TimeZone, Utc};
    use rusqlite::Connection;

    use super::{
        evaluate_backup_watch, evaluate_watches, evaluate_watches_best_effort,
        run_startup_processes, Thresholds,
    };
    use crate::test_utils::{insert_product_fixture, insert_unit_fixture, setup_test_db};
    use crate::timestamps::format_utc_timestamp;

    // Mediodía en Cuba: la fecha local coincide con la UTC.
    fn now() -> DateTime<Utc> {
        Utc.with_ymd_and_hms(2026, 10, 28, 16, 0, 0).unwrap()
    }

    // (tipo, prioridad, mensaje, entidad, id de la entidad)
    type NotificationRow = (String, String, String, Option<String>, Option<i64>);

    fn notifications(connection: &Connection) -> Vec<NotificationRow> {
        connection
            .prepare("SELECT type, priority, message, entityType, entityId FROM notifications ORDER BY id")
            .unwrap()
            .query_map([], |row| {
                Ok((row.get(0)?, row.get(1)?, row.get(2)?, row.get(3)?, row.get(4)?))
            })
            .unwrap()
            .collect::<rusqlite::Result<_>>()
            .unwrap()
    }

    fn messages(connection: &Connection) -> Vec<String> {
        notifications(connection)
            .into_iter()
            .map(|row| row.2)
            .collect()
    }

    fn evaluate(connection: &Connection) {
        evaluate_watches(connection, now(), Thresholds::default()).unwrap();
    }

    fn set_quantity(connection: &Connection, id: i64, quantity_milli: i64) {
        connection
            .execute(
                "UPDATE products SET quantityMilli = ?1 WHERE id = ?2",
                [quantity_milli, id],
            )
            .unwrap();
    }

    fn product(connection: &Connection, name: &str, quantity_milli: i64) -> i64 {
        let unit = insert_unit_fixture(connection, &format!("Unidad {name}"), &format!("u{name}"));
        let id = insert_product_fixture(
            connection,
            name,
            None,
            Some(unit),
            100,
            0,
            quantity_milli,
            "active",
            None,
            None,
        );
        set_threshold(connection, id, 5_000);
        id
    }

    fn set_threshold(connection: &Connection, product_id: i64, threshold_milli: i64) {
        connection
            .execute(
                "UPDATE products SET lowStockThresholdMilli = ?1 WHERE id = ?2",
                [threshold_milli, product_id],
            )
            .unwrap();
    }

    fn invoice(
        connection: &Connection,
        kind: &str,
        number: &str,
        invoice_date: &str,
        status: &str,
        reserved_until: Option<&str>,
    ) -> i64 {
        connection
            .execute(
                "INSERT INTO invoices (type, number, invoiceDate, status, contactFullName, contactFullNameNormalized, contactNationalId, contactPhone, createdByName, createdAt, updatedAt, reservedUntil, paidAt, paymentMethod)
                 VALUES (?1, ?2, ?3, ?4, 'Rosa Díaz', 'rosa diaz', '90010112345', '+5355550000', 'admin', '2026-10-01T12:00:00Z', '2026-10-01T12:00:00Z', ?5, ?6, ?7)",
                rusqlite::params![
                    kind,
                    number,
                    invoice_date,
                    status,
                    reserved_until,
                    (status == "completed").then_some("2026-10-02T12:00:00Z"),
                    (status == "completed").then_some("cash"),
                ],
            )
            .unwrap();
        connection.last_insert_rowid()
    }

    #[test]
    fn a_situation_warns_once_and_again_only_after_leaving_it() {
        let connection = setup_test_db();
        let id = product(&connection, "Café", 3_000);

        evaluate(&connection);
        evaluate(&connection);
        assert_eq!(
            messages(&connection),
            vec!["Producto con stock bajo: Café - Cantidad actual: 3 uCafé"]
        );

        // Se repone: sale de la situación sin aviso nuevo.
        set_quantity(&connection, id, 20_000);
        evaluate(&connection);
        assert_eq!(messages(&connection).len(), 1);

        // Vuelve a bajar: avisa de nuevo.
        set_quantity(&connection, id, 2_500);
        evaluate(&connection);
        assert_eq!(messages(&connection).len(), 2);
        assert_eq!(
            messages(&connection)[1],
            "Producto con stock bajo: Café - Cantidad actual: 2,5 uCafé"
        );

        // De stock bajo a agotado es otra situación: N-02.
        set_quantity(&connection, id, 0);
        evaluate(&connection);
        let last = notifications(&connection).pop().unwrap();
        assert_eq!(
            last,
            (
                "N-02".into(),
                "CRÍTICO".into(),
                "Producto agotado: Café".into(),
                Some("product".into()),
                Some(id)
            )
        );
        evaluate(&connection);
        assert_eq!(messages(&connection).len(), 3);
    }

    #[test]
    fn n02_and_n03_only_for_active_products_and_with_the_given_threshold() {
        let connection = setup_test_db();
        product(&connection, "Agotado", 0);
        product(&connection, "Justo", 5_000);
        product(&connection, "Casi", 4_999);
        insert_product_fixture(
            &connection,
            "Retirado",
            None,
            None,
            100,
            0,
            0,
            "discontinued",
            Some("2026-10-27T12:00:00Z"),
            None,
        );

        evaluate(&connection);
        let rows = notifications(&connection);
        assert_eq!(rows.len(), 2);
        assert_eq!(
            (rows[0].0.as_str(), rows[0].2.as_str()),
            ("N-02", "Producto agotado: Agotado")
        );
        assert_eq!(
            (rows[1].0.as_str(), rows[1].1.as_str(), rows[1].2.as_str()),
            (
                "N-03",
                "ADVERTENCIA",
                "Producto con stock bajo: Casi - Cantidad actual: 4,999 uCasi"
            )
        );

        // Con su propio umbral más alto, "Justo" (5) entra en stock bajo; sin
        // umbral, "Libre" (1) no avisa (spec-06 UM-01).
        let connection = setup_test_db();
        let justo = product(&connection, "Justo", 5_000);
        set_threshold(&connection, justo, 10_000);
        let libre = product(&connection, "Libre", 1_000);
        connection
            .execute(
                "UPDATE products SET lowStockThresholdMilli = NULL WHERE id = ?1",
                [libre],
            )
            .unwrap();
        evaluate_watches(&connection, now(), Thresholds::default()).unwrap();
        assert_eq!(
            messages(&connection),
            vec!["Producto con stock bajo: Justo - Cantidad actual: 5 uJusto"]
        );
    }

    #[test]
    fn n04_from_today_to_today_plus_29_and_n07_under_10_days() {
        let connection = setup_test_db();
        for (name, expires) in [
            ("Hoy", "2026-10-28"),
            ("Limite", "2026-11-26"),
            ("Fuera", "2026-11-27"),
            ("Vencido", "2026-10-27"),
        ] {
            insert_product_fixture(
                &connection,
                name,
                None,
                None,
                100,
                0,
                50_000,
                "active",
                None,
                Some(expires),
            );
        }
        // Descontinuados: la purga es a los 60 días.
        let discontinued = |days_ago: i64| format_utc_timestamp(now() - Duration::days(days_ago));
        insert_product_fixture(
            &connection,
            "Nueve",
            None,
            None,
            100,
            0,
            1_000,
            "discontinued",
            Some(&discontinued(51)),
            None,
        );
        insert_product_fixture(
            &connection,
            "Diez",
            None,
            None,
            100,
            0,
            1_000,
            "discontinued",
            Some(&discontinued(50)),
            None,
        );

        evaluate(&connection);
        let rows = notifications(&connection);
        assert_eq!(
            rows.iter().map(|row| (row.0.as_str(), row.1.as_str(), row.2.as_str())).collect::<Vec<_>>(),
            vec![
                ("N-04", "CRÍTICO", "Producto próximo a vencer: Hoy - Caducidad: 28/10/2026 - Días restantes: 0 días"),
                ("N-04", "CRÍTICO", "Producto próximo a vencer: Limite - Caducidad: 26/11/2026 - Días restantes: 29 días"),
                ("N-07", "ADVERTENCIA", "Producto descontinuado próximo a eliminar: Nueve - Días restantes para eliminación: 9 días"),
            ]
        );
    }

    #[test]
    fn n05_reservations_under_3_days_and_n10_stale_pending_invoices() {
        let connection = setup_test_db();
        let soon = invoice(
            &connection,
            "V",
            "V-00001",
            "2026-10-27",
            "pending",
            Some("2026-10-30"),
        );
        invoice(
            &connection,
            "S",
            "S-00001",
            "2026-10-27",
            "pending",
            Some("2026-10-31"),
        );
        invoice(
            &connection,
            "V",
            "V-00002",
            "2026-10-20",
            "completed",
            Some("2026-10-29"),
        );
        let purchase = invoice(&connection, "E", "E-00001", "2026-09-27", "pending", None);
        invoice(&connection, "V", "V-00003", "2026-09-28", "pending", None);
        invoice(&connection, "S", "S-00002", "2026-09-20", "pending", None);
        invoice(&connection, "V", "V-00004", "2026-09-01", "completed", None);

        evaluate(&connection);
        let rows = notifications(&connection);
        assert_eq!(
            rows.iter().map(|row| (row.0.as_str(), row.2.as_str())).collect::<Vec<_>>(),
            vec![
                ("N-05", "Reserva por vencerse: Factura # V-00001 - Cliente: Rosa Díaz - Días restantes: 2 días"),
                ("N-10", "Factura pendiente antigua: E-00001 - Rosa Díaz - 31 días sin pagar"),
                ("N-10", "Factura pendiente antigua: S-00002 - Rosa Díaz - 38 días sin cobrar"),
            ]
        );
        assert_eq!(rows[0].3.as_deref(), Some("invoice"));
        assert_eq!(rows[0].4, Some(soon));
        assert_eq!(rows[1].4, Some(purchase));

        // Cobrar la reserva la saca de N-05; volver a evaluar no repite N-10.
        connection
            .execute("UPDATE invoices SET status = 'completed', paidAt = '2026-10-28T12:00:00Z', paymentMethod = 'cash' WHERE id = ?1", [soon])
            .unwrap();
        evaluate(&connection);
        assert_eq!(notifications(&connection).len(), 3);
        let watched: i64 = connection
            .query_row(
                "SELECT COUNT(*) FROM notificationWatches WHERE kind = 'N-05'",
                [],
                |row| row.get(0),
            )
            .unwrap();
        assert_eq!(watched, 0);
    }

    #[test]
    fn n11_after_more_than_the_reminder_days_without_a_backup() {
        let connection = setup_test_db();
        let reference = now() - Duration::days(30);

        evaluate_backup_watch(
            &connection,
            now(),
            Some(now() - Duration::days(7)),
            reference,
            7,
        )
        .unwrap();
        assert!(messages(&connection).is_empty());

        evaluate_backup_watch(
            &connection,
            now(),
            Some(now() - Duration::days(8)),
            reference,
            7,
        )
        .unwrap();
        evaluate_backup_watch(
            &connection,
            now(),
            Some(now() - Duration::days(8)),
            reference,
            7,
        )
        .unwrap();
        assert_eq!(
            messages(&connection),
            vec!["No se ha hecho una copia de seguridad en 8 días"]
        );

        // Una copia nueva cierra la situación; sin ninguna, cuenta desde el inicio.
        evaluate_backup_watch(&connection, now(), Some(now()), reference, 7).unwrap();
        evaluate_backup_watch(&connection, now(), None, reference, 7).unwrap();
        assert_eq!(
            messages(&connection)[1],
            "No se ha hecho una copia de seguridad en 30 días"
        );
    }

    #[test]
    fn a_failing_evaluation_is_logged_and_does_not_break_the_transaction() {
        let mut connection = setup_test_db();
        product(&connection, "Café", 0);
        crate::test_utils::create_fail_notification_insert_trigger(&connection).unwrap();

        let transaction = connection.transaction().unwrap();
        evaluate_watches_best_effort(&transaction, now());
        // Lo hecho dentro del SAVEPOINT (la vigilancia) se deshizo.
        let watched: i64 = transaction
            .query_row("SELECT COUNT(*) FROM notificationWatches", [], |row| {
                row.get(0)
            })
            .unwrap();
        assert_eq!(watched, 0);
        let logged: i64 = transaction
            .query_row(
                "SELECT COUNT(*) FROM errorLogs WHERE operation = 'evaluate_watches'",
                [],
                |row| row.get(0),
            )
            .unwrap();
        assert_eq!(logged, 1);
        transaction.commit().unwrap();
    }

    #[test]
    fn startup_evaluates_watches_purges_and_drops_notifications_older_than_90_days() {
        let mut connection = setup_test_db();
        product(&connection, "Café", 0);
        let discontinued = format_utc_timestamp(now() - Duration::days(61));
        insert_product_fixture(
            &connection,
            "Viejo",
            None,
            None,
            100,
            0,
            0,
            "discontinued",
            Some(&discontinued),
            None,
        );
        for days in [91, 89] {
            connection
                .execute(
                    "INSERT INTO notifications (type, priority, message, createdAt) VALUES ('N-09', 'ADVERTENCIA', ?1, ?2)",
                    [format!("Hace {days} días"), format_utc_timestamp(now() - Duration::days(days))],
                )
                .unwrap();
        }

        run_startup_processes(&mut connection, now());
        assert_eq!(
            messages(&connection),
            vec![
                "Hace 89 días",
                "Se eliminaron automáticamente estos productos descontinuados: Viejo",
                "Producto agotado: Café",
            ]
        );
    }
}
