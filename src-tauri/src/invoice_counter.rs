use rusqlite::{Connection, OptionalExtension};

use crate::errors::AppError;

pub fn next_invoice_number(
    connection: &Connection,
    invoice_type: &str,
) -> Result<String, AppError> {
    let next_number = connection
        .query_row(
            "SELECT nextNumber FROM invoiceCounters WHERE type = ?1",
            [invoice_type],
            |row| row.get::<_, i64>(0),
        )
        .optional()
        .map_err(|_| {
            AppError::new(
                "internal",
                "Ocurrió un error inesperado",
                "invoice counter read failed",
            )
        })?
        .ok_or_else(|| {
            AppError::new(
                "invalidState",
                "El tipo de factura no es válido",
                "invoice counter type missing",
            )
        })?;
    let next_value = next_number.checked_add(1).ok_or_else(|| {
        AppError::new(
            "internal",
            "Ocurrió un error inesperado",
            "invoice counter overflow",
        )
    })?;
    connection
        .execute(
            "UPDATE invoiceCounters SET nextNumber = ?1 WHERE type = ?2",
            (next_value, invoice_type),
        )
        .map_err(|_| {
            AppError::new(
                "internal",
                "Ocurrió un error inesperado",
                "invoice counter update failed",
            )
        })?;
    Ok(format!("{invoice_type}-{next_number:05}"))
}

#[cfg(test)]
mod tests {
    use crate::test_utils::setup_test_db;

    use super::next_invoice_number;

    #[test]
    fn invoice_numbers_increment_by_type_and_keep_width_after_99999() {
        let connection = setup_test_db();
        assert_eq!(next_invoice_number(&connection, "E").unwrap(), "E-00001");
        assert_eq!(next_invoice_number(&connection, "E").unwrap(), "E-00002");
        assert_eq!(next_invoice_number(&connection, "V").unwrap(), "V-00001");
        connection
            .execute(
                "UPDATE invoiceCounters SET nextNumber = 99999 WHERE type = 'S'",
                [],
            )
            .unwrap();
        assert_eq!(next_invoice_number(&connection, "S").unwrap(), "S-99999");
        assert_eq!(next_invoice_number(&connection, "S").unwrap(), "S-100000");
    }
}
