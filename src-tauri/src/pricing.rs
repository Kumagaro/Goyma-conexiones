/// Precio calculado: costo × (1 + % de ganancia), en céntimos (mitad hacia arriba).
pub fn calculated_price_cents(cost_cents: i64, profit_percent_hundredths: i64) -> i64 {
    let numerator = i128::from(cost_cents) * (10_000 + i128::from(profit_percent_hundredths));
    ((numerator + 5_000) / 10_000) as i64
}

/// Precio de venta (`spec-06` PV-01): el calculado, redondeado hacia arriba
/// al entero.
pub fn selling_price_cents(cost_cents: i64, profit_percent_hundredths: i64) -> i64 {
    let calculated = calculated_price_cents(cost_cents, profit_percent_hundredths);
    (calculated + 99) / 100 * 100
}

#[cfg(test)]
mod tests {
    use super::{calculated_price_cents, selling_price_cents};

    #[test]
    fn calculated_price_uses_exact_cents_and_rounds_half_up() {
        for (cost_cents, profit_hundredths, expected) in [
            (1_250, 2_000, 1_500),
            (1_250, 0, 1_250),
            (2_500, 100_000, 27_500),
            (1, 5_000, 2),
        ] {
            assert_eq!(
                calculated_price_cents(cost_cents, profit_hundredths),
                expected
            );
        }
    }

    #[test]
    fn selling_price_rounds_the_calculated_price_up_to_the_unit() {
        // 18,37 → 19,00; 19,00 → 19,00; 0,01 → 1,00 (spec-06 PV-01).
        assert_eq!(selling_price_cents(1_837, 0), 1_900);
        assert_eq!(selling_price_cents(1_900, 0), 1_900);
        assert_eq!(selling_price_cents(1, 0), 100);
        assert_eq!(selling_price_cents(1_250, 2_000), 1_500);
        assert_eq!(selling_price_cents(1_250, 3_333), 1_700);
    }
}
