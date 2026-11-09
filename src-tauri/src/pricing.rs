use crate::errors::AppError;
use crate::validators::{parse_cost_cents, parse_percent_hundredths};

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

/// Precio de una línea de Venta o Servicio (`spec-06` PF-01, PF-02).
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct LinePrice {
    pub unit_price_cents: i64,
    /// Bruto (cantidad × precio); `None` cuando no hay descuento.
    pub gross_cents: Option<i64>,
    pub discount_kind: Option<String>,
    pub discount_value: Option<i64>,
    pub discount_cents: i64,
    /// Importe de la línea: bruto − descuento.
    pub subtotal_cents: i64,
}

/// El precio pedido no baja del de lista; el descuento (en centésimas de % o
/// en céntimos) no supera el bruto. `gross_of` calcula el bruto de la línea
/// con un precio por unidad (tiene en cuenta la unidad elegida, CV-01).
pub fn price_line(
    product_name: &str,
    list_price_cents: i64,
    default_price_cents: i64,
    requested_price: Option<&str>,
    discount_kind: Option<&str>,
    discount_value: Option<&str>,
    gross_of: impl Fn(i64) -> i64,
) -> Result<LinePrice, AppError> {
    let requested_price = requested_price.map(str::trim).filter(|raw| !raw.is_empty());
    let unit_price_cents = match requested_price {
        Some(raw) => parse_cost_cents(raw).map_err(|_| {
            validation_failed(
                &format!("El precio de «{product_name}» no es válido"),
                "invalid requested line price",
            )
        })?,
        None => default_price_cents,
    };
    if unit_price_cents < list_price_cents {
        return Err(AppError::new(
            "priceBelowList",
            &format!(
                "El precio de «{product_name}» no puede ser menor que {}",
                crate::export::format_money(list_price_cents)
            ),
            "line price below list price",
        ));
    }
    let gross_cents = gross_of(unit_price_cents);
    let discount_value = discount_value.map(str::trim).filter(|raw| !raw.is_empty());
    let invalid_discount = || {
        validation_failed(
            &format!("El descuento de «{product_name}» no es válido"),
            "invalid line discount",
        )
    };
    let (discount_kind, discount_value, discount_cents) = match (discount_kind, discount_value) {
        (_, None) => (None, None, 0),
        (Some("percent"), Some(raw)) => {
            let hundredths = parse_percent_hundredths(raw).map_err(|_| invalid_discount())?;
            if hundredths == 0 || hundredths > 10_000 {
                return Err(invalid_discount());
            }
            let cents = crate::inventory::round_half_up(
                i128::from(gross_cents) * i128::from(hundredths),
                10_000,
            ) as i64;
            (Some("percent".to_owned()), Some(hundredths), cents)
        }
        (Some("amount"), Some(raw)) => {
            let cents = parse_cost_cents(raw).map_err(|_| invalid_discount())?;
            (Some("amount".to_owned()), Some(cents), cents)
        }
        _ => return Err(invalid_discount()),
    };
    if discount_cents > gross_cents {
        return Err(AppError::new(
            "discountTooLarge",
            &format!("El descuento de «{product_name}» no puede superar el importe de la línea"),
            "line discount above gross amount",
        ));
    }
    Ok(LinePrice {
        unit_price_cents,
        gross_cents: discount_kind.as_ref().map(|_| gross_cents),
        discount_kind,
        discount_value,
        discount_cents,
        subtotal_cents: gross_cents - discount_cents,
    })
}

fn validation_failed(message: &str, cause: &str) -> AppError {
    AppError::new("validationFailed", message, cause)
}

#[cfg(test)]
mod tests {
    use super::{calculated_price_cents, price_line, selling_price_cents};

    fn gross(quantity_milli: i64) -> impl Fn(i64) -> i64 {
        move |price| (quantity_milli * price + 500) / 1_000
    }

    #[test]
    fn a_line_price_can_go_up_but_never_below_the_list_price() {
        let same = price_line("Panel", 1_900, 1_900, None, None, None, gross(2_000)).unwrap();
        assert_eq!(
            (same.unit_price_cents, same.subtotal_cents, same.gross_cents),
            (1_900, 3_800, None)
        );
        let higher = price_line(
            "Panel",
            1_900,
            1_900,
            Some("25,00"),
            None,
            None,
            gross(2_000),
        )
        .unwrap();
        assert_eq!(
            (higher.unit_price_cents, higher.subtotal_cents),
            (2_500, 5_000)
        );
        let equal =
            price_line("Panel", 1_900, 1_900, Some("19"), None, None, gross(1_000)).unwrap();
        assert_eq!(equal.unit_price_cents, 1_900);
        let lower = price_line(
            "Panel",
            1_900,
            1_900,
            Some("18,99"),
            None,
            None,
            gross(1_000),
        )
        .unwrap_err();
        assert_eq!(lower.code, "priceBelowList");
        assert_eq!(
            lower.message,
            "El precio de «Panel» no puede ser menor que 19,00 USD"
        );
    }

    #[test]
    fn editing_keeps_the_saved_price_and_the_saved_list_price_as_minimum() {
        // S-03: al editar, el mínimo es el precio de lista guardado.
        let kept = price_line("Panel", 1_900, 2_300, None, None, None, gross(1_000)).unwrap();
        assert_eq!(kept.unit_price_cents, 2_300);
        let error =
            price_line("Panel", 1_900, 2_300, Some("18"), None, None, gross(1_000)).unwrap_err();
        assert_eq!(error.code, "priceBelowList");
    }

    #[test]
    fn discounts_in_percent_or_amount_can_go_below_the_list_price_but_not_below_zero() {
        let percent = price_line(
            "Panel",
            1_900,
            1_900,
            None,
            Some("percent"),
            Some("10"),
            gross(2_000),
        )
        .unwrap();
        assert_eq!(
            (
                percent.gross_cents,
                percent.discount_value,
                percent.discount_cents,
                percent.subtotal_cents
            ),
            (Some(3_800), Some(1_000), 380, 3_420)
        );
        let amount = price_line(
            "Panel",
            1_900,
            1_900,
            None,
            Some("amount"),
            Some("5,00"),
            gross(1_000),
        )
        .unwrap();
        assert_eq!((amount.discount_cents, amount.subtotal_cents), (500, 1_400));
        let all = price_line(
            "Panel",
            1_900,
            1_900,
            None,
            Some("amount"),
            Some("19"),
            gross(1_000),
        )
        .unwrap();
        assert_eq!(all.subtotal_cents, 0);
        let too_large = price_line(
            "Panel",
            1_900,
            1_900,
            None,
            Some("amount"),
            Some("19,01"),
            gross(1_000),
        )
        .unwrap_err();
        assert_eq!(too_large.code, "discountTooLarge");
        for (kind, value) in [
            (Some("percent"), "0"),
            (Some("percent"), "100,01"),
            (Some("otro"), "5"),
            (None, "5"),
        ] {
            let error = price_line("Panel", 1_900, 1_900, None, kind, Some(value), gross(1_000))
                .unwrap_err();
            assert_eq!(error.code, "validationFailed", "{kind:?} {value}");
        }
    }

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
