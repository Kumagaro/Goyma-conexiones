use chrono::{Datelike, NaiveDate};

use crate::db::schema::{RESERVED_NATIONAL_ID, RESERVED_PHONE};
use crate::errors::AppError;
use crate::text::{clean_description, clean_name, normalize_text};

const USERNAME_ERROR: &str = "El nombre de usuario debe tener de 3 a 50 caracteres alfanuméricos (letras y números, sin acentos ni espacios)";
const PASSWORD_ASCII_ERROR: &str =
    "La contraseña solo puede contener caracteres ASCII imprimibles, sin espacios, acentos ni ñ";
const PASSWORD_MIN_ERROR: &str = "La contraseña debe tener al menos 8 caracteres";
const PASSWORD_MAX_ERROR: &str = "La contraseña no puede tener más de 72 caracteres";
const PASSWORD_UPPERCASE_ERROR: &str = "La contraseña debe incluir al menos una mayúscula";
const PASSWORD_SPECIAL_ERROR: &str = "La contraseña debe incluir al menos un carácter especial";
const PHONE_ERROR: &str = "El teléfono debe tener el formato +53 seguido de 8 dígitos";
const NATIONAL_ID_ERROR: &str = "El carnet de identidad debe tener exactamente 11 dígitos";
const ROLE_ERROR: &str = "El rol no es válido";

pub fn normalize_username(username: &str) -> String {
    username.to_ascii_lowercase()
}

pub fn validate_username(username: &str) -> Result<String, AppError> {
    if !(3..=50).contains(&username.len())
        || !username.bytes().all(|byte| byte.is_ascii_alphanumeric())
    {
        return Err(validation_error(USERNAME_ERROR));
    }
    Ok(normalize_username(username))
}

pub fn validate_password(password: &str) -> Result<(), AppError> {
    if !password.bytes().all(|byte| (0x21..=0x7e).contains(&byte)) {
        return Err(validation_error(PASSWORD_ASCII_ERROR));
    }
    if password.len() < 8 {
        return Err(validation_error(PASSWORD_MIN_ERROR));
    }
    if password.len() > 72 {
        return Err(validation_error(PASSWORD_MAX_ERROR));
    }
    if !password.bytes().any(|byte| byte.is_ascii_uppercase()) {
        return Err(validation_error(PASSWORD_UPPERCASE_ERROR));
    }
    if !password.bytes().any(|byte| byte.is_ascii_punctuation()) {
        return Err(validation_error(PASSWORD_SPECIAL_ERROR));
    }
    Ok(())
}

pub fn validate_phone(phone: &str) -> Result<(), AppError> {
    let valid = phone.len() == 11
        && phone.starts_with("+53")
        && phone[3..].bytes().all(|byte| byte.is_ascii_digit());
    if !valid {
        return Err(validation_error(PHONE_ERROR));
    }
    Ok(())
}

pub fn validate_national_id(national_id: &str) -> Result<(), AppError> {
    if national_id.len() != 11 || !national_id.bytes().all(|byte| byte.is_ascii_digit()) {
        return Err(validation_error(NATIONAL_ID_ERROR));
    }
    Ok(())
}

pub fn validate_role(role: &str) -> Result<(), AppError> {
    if !matches!(role, "administrador" | "contador") {
        return Err(validation_error(ROLE_ERROR));
    }
    Ok(())
}

pub fn validate_not_reserved(candidate: &str, current_value: &str) -> Result<(), AppError> {
    if candidate != current_value && candidate == RESERVED_PHONE {
        return Err(validation_error(
            "Ese teléfono está reservado por el sistema. Ingresa un teléfono real",
        ));
    }
    if candidate != current_value && candidate == RESERVED_NATIONAL_ID {
        return Err(validation_error(
            "Ese carnet de identidad está reservado por el sistema. Ingresa uno real",
        ));
    }
    Ok(())
}

fn has_forbidden_format_chars(value: &str) -> bool {
    value.chars().any(|ch| (ch.is_control() && !ch.is_whitespace()) || matches!(ch,
        '\u{00AD}' |
        '\u{200B}' | '\u{200C}' | '\u{200D}' | '\u{200E}' | '\u{200F}' |
        '\u{202A}' | '\u{202B}' | '\u{202C}' | '\u{202D}' | '\u{202E}' |
        '\u{2060}' | '\u{2061}' | '\u{2062}' | '\u{2063}' | '\u{2064}' |
        '\u{2066}' | '\u{2067}' | '\u{2068}' | '\u{2069}' | '\u{206A}' |
        '\u{206B}' | '\u{206C}' | '\u{206D}' | '\u{206E}' | '\u{206F}' |
        '\u{FEFF}'
    ))
}

pub fn validate_name(raw: &str, min: usize, max: usize, message: &str) -> Result<(String, String), AppError> {
    let cleaned = clean_name(raw);
    if cleaned.is_empty() || cleaned.chars().count() < min || cleaned.chars().count() > max || has_forbidden_format_chars(&cleaned) {
        return Err(validation_error(message));
    }

    let normalized = normalize_text(&cleaned);
    Ok((cleaned, normalized))
}

pub fn validate_description(raw: &str, max: usize, message: &str) -> Result<(String, String), AppError> {
    let Some(cleaned) = clean_description(raw) else {
        return Err(validation_error(message));
    };

    if cleaned.chars().count() > max || has_forbidden_format_chars(&cleaned) {
        return Err(validation_error(message));
    }

    let normalized = normalize_text(&cleaned);
    Ok((cleaned, normalized))
}

fn parse_decimal_parts(raw: &str) -> Result<(String, Option<String>), AppError> {
    let trimmed = raw.trim();
    if trimmed.is_empty() || trimmed.bytes().any(|byte| byte.is_ascii_whitespace()) {
        return Err(validation_error("El valor ingresado no es válido"));
    }

    let has_comma = trimmed.contains(',');
    let has_dot = trimmed.contains('.');
    if has_comma && has_dot {
        return Err(validation_error("El valor ingresado no es válido"));
    }

    let mut normalized = String::new();
    for ch in trimmed.chars() {
        if ch.is_ascii_digit() {
            normalized.push(ch);
        } else if ch == '.' || ch == ',' {
            normalized.push(ch);
        } else {
            return Err(validation_error("El valor ingresado no es válido"));
        }
    }

    let (whole, fraction) = if has_comma {
        let mut parts = trimmed.split(',');
        (parts.next().unwrap_or(""), parts.next())
    } else if has_dot {
        let mut parts = trimmed.split('.');
        (parts.next().unwrap_or(""), parts.next())
    } else {
        (trimmed, None)
    };

    if whole.is_empty() || (fraction.is_some() && fraction.unwrap().is_empty()) {
        return Err(validation_error("El valor ingresado no es válido"));
    }

    Ok((normalized, fraction.map(str::to_owned)))
}

pub fn parse_scaled(
    raw: &str,
    max_integer_digits: usize,
    decimal_places: usize,
    min: i128,
    max: i128,
) -> Result<i128, AppError> {
    let (normalized, fraction) = parse_decimal_parts(raw)?;

    let (whole, fraction_part) = if fraction.is_some() {
        let mut parts = normalized.split(['.', ',']);
        let whole = parts.next().unwrap_or("");
        let frac = parts.next().unwrap_or("");
        (whole, Some(frac))
    } else {
        (normalized.as_str(), None)
    };

    let whole_digits = whole.chars().filter(|ch| ch.is_ascii_digit()).count();
    if whole_digits > max_integer_digits {
        return Err(validation_error("El valor está fuera de rango"));
    }

    if let Some(frac) = fraction_part {
        let frac_digits = frac.chars().filter(|ch| ch.is_ascii_digit()).count();
        if frac_digits > decimal_places {
            return Err(validation_error("El valor tiene demasiados decimales"));
        }
        if normalized.contains('.') && frac_digits == 3 && whole_digits > 0 && whole != "0" {
            return Err(validation_error("El valor tiene un formato ambiguo"));
        }
    }

    let scaled = if let Some(frac) = fraction_part {
        let mut digits = frac.to_owned();
        while digits.len() < decimal_places {
            digits.push('0');
        }
        let whole_part = if whole.is_empty() { "0" } else { whole };
        let value = if whole_part == "0" && digits == "0".repeat(decimal_places) {
            0
        } else {
            let mut integer_value = whole_part.parse::<i128>().unwrap_or(0);
            integer_value = integer_value * (10_i128.pow(decimal_places as u32));
            let fraction_value = digits.parse::<i128>().unwrap_or(0);
            integer_value + fraction_value
        };
        value
    } else {
        let whole_part = if whole.is_empty() { "0" } else { whole };
        whole_part.parse::<i128>().unwrap_or(0)
    };

    if scaled < min || scaled > max {
        return Err(validation_error("El valor está fuera de rango"));
    }

    Ok(scaled)
}

pub fn parse_cost_cents(raw: &str) -> Result<i64, AppError> {
    let value = parse_scaled(raw, 8, 2, 1, 99_999_999)?;
    Ok(value as i64)
}

pub fn parse_percent_hundredths(raw: &str) -> Result<i64, AppError> {
    let value = parse_scaled(raw, 6, 2, 0, 100_000)?;
    Ok(value as i64)
}

pub fn parse_quantity_milli(raw: &str) -> Result<i64, AppError> {
    let value = parse_scaled(raw, 9, 3, 1, 999_999_999)?;
    Ok(value as i64)
}

pub fn parse_factor_micro(raw: &str) -> Result<i64, AppError> {
    let value = parse_scaled(raw, 15, 6, 1, 999_999_999_999_999)?;
    Ok(value as i64)
}

pub fn parse_quantity_micro(raw: &str) -> Result<i64, AppError> {
    let value = parse_scaled(raw, 15, 6, 1, 999_999_999_999_999)?;
    Ok(value as i64)
}

pub fn validate_expiry_date(
    raw: &str,
    today: NaiveDate,
    actual: Option<&str>,
) -> Result<String, AppError> {
    let cleaned = raw.trim();
    if cleaned.len() != 10 || !cleaned.chars().all(|ch| ch.is_ascii_digit() || ch == '-') {
        return Err(validation_error("La fecha de caducidad no es válida"));
    }

    let parsed = match NaiveDate::parse_from_str(cleaned, "%Y-%m-%d") {
        Ok(date) => date,
        Err(_) => return Err(validation_error("La fecha de caducidad no es válida")),
    };

    if parsed.year() > 2100 {
        return Err(validation_error("La fecha de caducidad no es válida"));
    }

    if parsed < today && actual != Some(cleaned) {
        return Err(validation_error("La fecha de caducidad no es válida"));
    }

    Ok(cleaned.to_owned())
}

fn validation_error(message: &str) -> AppError {
    AppError::new("validationFailed", message, "input validation failed")
}

#[cfg(test)]
mod tests {
    use super::{
        normalize_username, parse_cost_cents, parse_factor_micro, parse_percent_hundredths,
        parse_quantity_milli, parse_quantity_micro, validate_description, validate_expiry_date,
        validate_name, validate_national_id, validate_not_reserved, validate_password,
        validate_phone, validate_role, validate_username,
    };

    fn assert_validation_error<T: std::fmt::Debug>(
        result: Result<T, crate::errors::AppError>,
        expected_message: &str,
    ) {
        let error = result.unwrap_err();
        assert_eq!(error.code, "validationFailed");
        assert_eq!(error.message, expected_message);
    }

    #[test]
    fn username_validation_is_ascii_alphanumeric_and_normalizes_case() {
        assert_eq!(normalize_username("Contador1"), "contador1");
        assert_eq!(validate_username("Contador1").unwrap(), "contador1");
        assert_eq!(validate_username(&"a".repeat(50)).unwrap().len(), 50);
        for invalid in ["ab", &"a".repeat(51), "josé", "jose_1", "user name"] {
            assert_validation_error(
                validate_username(invalid),
                "El nombre de usuario debe tener de 3 a 50 caracteres alfanuméricos (letras y números, sin acentos ni espacios)",
            );
        }
    }

    #[test]
    fn password_validation_returns_the_first_failed_rule() {
        assert_validation_error(
            validate_password("ÑvalidPass1!"),
            "La contraseña solo puede contener caracteres ASCII imprimibles, sin espacios, acentos ni ñ",
        );
        assert_validation_error(
            validate_password("short7!"),
            "La contraseña debe tener al menos 8 caracteres",
        );
        assert_validation_error(
            validate_password(&format!("A{}!", "a".repeat(71))),
            "La contraseña no puede tener más de 72 caracteres",
        );
        assert_validation_error(
            validate_password("validpass1!"),
            "La contraseña debe incluir al menos una mayúscula",
        );
        assert_validation_error(
            validate_password("Validpass1"),
            "La contraseña debe incluir al menos un carácter especial",
        );
        assert_validation_error(
            validate_password("Valid pass1!"),
            "La contraseña solo puede contener caracteres ASCII imprimibles, sin espacios, acentos ni ñ",
        );
        assert_validation_error(
            validate_password("Válidpass1!"),
            "La contraseña solo puede contener caracteres ASCII imprimibles, sin espacios, acentos ni ñ",
        );
        assert!(validate_password("ValidPass1!").is_ok());
        assert!(validate_password(&format!("A{}!", "a".repeat(70))).is_ok());
    }

    #[test]
    fn phone_and_national_id_validators_enforce_cuban_formats() {
        assert!(validate_phone("+5312345678").is_ok());
        for invalid in ["+531234567", "+53123456789", "5312345678", "+53abcdefgh"] {
            assert_validation_error(
                validate_phone(invalid),
                "El teléfono debe tener el formato +53 seguido de 8 dígitos",
            );
        }

        assert!(validate_national_id("12345678901").is_ok());
        for invalid in ["1234567890", "123456789012", "1234567890a"] {
            assert_validation_error(
                validate_national_id(invalid),
                "El carnet de identidad debe tener exactamente 11 dígitos",
            );
        }
    }

    #[test]
    fn role_validator_accepts_only_supported_roles() {
        assert!(validate_role("administrador").is_ok());
        assert!(validate_role("contador").is_ok());
        assert_validation_error(validate_role("admin"), "El rol no es válido");
    }

    #[test]
    fn reserved_contact_values_are_rejected_only_when_being_assigned() {
        assert_validation_error(
            validate_not_reserved("+5300000000", "+5350000000"),
            "Ese teléfono está reservado por el sistema. Ingresa un teléfono real",
        );
        assert_validation_error(
            validate_not_reserved("00000000000", "12345678901"),
            "Ese carnet de identidad está reservado por el sistema. Ingresa uno real",
        );
        assert!(validate_not_reserved("+5300000000", "+5300000000").is_ok());
        assert!(validate_not_reserved("00000000000", "00000000000").is_ok());
        assert!(validate_not_reserved("+5350000000", "+5300000000").is_ok());
    }

    #[test]
    fn inventory_text_validation_cleans_and_normalizes_names_and_descriptions() {
        let (clean_name, normalized_name) = validate_name("  Café   con  ñ  ", 2, 50, "Nombre inválido").unwrap();
        assert_eq!(clean_name, "Café con ñ");
        assert_eq!(normalized_name, "cafe con ñ");

        let (clean_desc, normalized_desc) = validate_description("  Dos\n\t líneas  ", 200, "Descripción inválida").unwrap();
        assert_eq!(clean_desc, "Dos\nlíneas");
        assert_eq!(normalized_desc, "dos lineas");

        assert!(validate_name("   ", 2, 50, "Nombre inválido").is_err());
        assert!(validate_description("\n\n", 200, "Descripción inválida").is_err());
    }

    #[test]
    fn inventory_decimal_parsing_follows_the_feature_order_of_validation() {
        assert_eq!(parse_cost_cents("999,99").unwrap(), 99_999);
        assert_eq!(parse_percent_hundredths("1000,00").unwrap(), 100_000);
        assert_eq!(parse_quantity_milli("12,345").unwrap(), 12_345);
        assert_eq!(parse_factor_micro("1,234567").unwrap(), 1_234_567);
        assert_eq!(parse_quantity_micro("5,000001").unwrap(), 5_000_001);

        assert!(parse_cost_cents("0").is_err());
        assert!(parse_percent_hundredths("1000.01").is_err());
        assert!(parse_quantity_milli("0.0001").is_err());
        assert!(parse_factor_micro("0.0000001").is_err());
        assert!(parse_quantity_micro("0").is_err());
        assert!(parse_cost_cents("1.234").is_err());
        assert!(parse_cost_cents("1e3").is_err());
    }

    #[test]
    fn inventory_expiry_validation_accepts_valid_dates_and_keeps_unchanged_expired_values() {
        let today = chrono::NaiveDate::from_ymd_opt(2026, 10, 3).unwrap();

        assert_eq!(validate_expiry_date("2026-10-03", today, None).unwrap(), "2026-10-03");
        assert!(validate_expiry_date("2026-10-02", today, None).is_err());
        assert!(validate_expiry_date("2026-02-30", today, None).is_err());
        assert!(validate_expiry_date("2026-2-3", today, None).is_err());
        assert!(validate_expiry_date("2101-01-01", today, None).is_err());
        assert_eq!(validate_expiry_date("2100-12-31", today, None).unwrap(), "2100-12-31");

        let expired = "2026-10-02";
        assert_eq!(validate_expiry_date(expired, today, Some(expired)).unwrap(), expired);
    }
}
