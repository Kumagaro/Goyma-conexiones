pub fn selling_price_cents(cost_cents: i64, profit_percent_hundredths: i64) -> i64 {
    let numerator = i128::from(cost_cents) * (10_000 + i128::from(profit_percent_hundredths));
    ((numerator + 5_000) / 10_000) as i64
}

#[cfg(test)]
mod tests {
    use super::selling_price_cents;

    #[test]
    fn selling_price_uses_exact_cents_and_percentage() {
        // Nota: esta aserción es válida solo dentro de spec-02. spec-03 cambia el comportamiento (los comandos de facturas sí modifican quantityMilli).
        for (cost_cents, profit_hundredths, expected) in [
            (1_250, 2_000, 1_500),
            (1_250, 0, 1_250),
            (2_500, 100_000, 27_500),
        ] {
            assert_eq!(selling_price_cents(cost_cents, profit_hundredths), expected);
        }
    }

    #[test]
    fn selling_price_rounds_half_up() {
        assert_eq!(selling_price_cents(1, 5_000), 2);
    }
}
