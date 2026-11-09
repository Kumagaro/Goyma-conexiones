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
    value.chars().any(|ch| {
        (ch.is_control() && !ch.is_whitespace())
            || matches!(
                ch,
                '\u{00AD}'
                    | '\u{200B}'
                    | '\u{200C}'
                    | '\u{200D}'
                    | '\u{200E}'
                    | '\u{200F}'
                    | '\u{202A}'
                    | '\u{202B}'
                    | '\u{202C}'
                    | '\u{202D}'
                    | '\u{202E}'
                    | '\u{2060}'
                    | '\u{2061}'
                    | '\u{2062}'
                    | '\u{2063}'
                    | '\u{2064}'
                    | '\u{2066}'
                    | '\u{2067}'
                    | '\u{2068}'
                    | '\u{2069}'
                    | '\u{206A}'
                    | '\u{206B}'
                    | '\u{206C}'
                    | '\u{206D}'
                    | '\u{206E}'
                    | '\u{206F}'
                    | '\u{FEFF}'
            )
    })
}

pub fn validate_name(
    raw: &str,
    min: usize,
    max: usize,
    message: &str,
) -> Result<(String, String), AppError> {
    validate_name_with_messages(raw, min, max, message, message, message)
}

pub fn validate_name_with_messages(
    raw: &str,
    min: usize,
    max: usize,
    required_message: &str,
    too_long_message: &str,
    forbidden_message: &str,
) -> Result<(String, String), AppError> {
    let cleaned = clean_name(raw);
    if has_forbidden_format_chars(&cleaned) {
        return Err(validation_error(forbidden_message));
    }
    let length = cleaned.chars().count();
    if length < min {
        return Err(validation_error(required_message));
    }
    if length > max {
        return Err(validation_error(too_long_message));
    }

    let normalized = normalize_text(&cleaned);
    Ok((cleaned, normalized))
}

pub fn validate_optional_description(
    raw: Option<&str>,
    max: usize,
    too_long_message: &str,
    forbidden_message: &str,
) -> Result<Option<String>, AppError> {
    let Some(cleaned) = raw.and_then(clean_description) else {
        return Ok(None);
    };

    if has_forbidden_format_chars(&cleaned) {
        return Err(validation_error(forbidden_message));
    }
    if cleaned.chars().count() > max {
        return Err(validation_error(too_long_message));
    }

    Ok(Some(cleaned))
}

pub fn validate_description(
    raw: &str,
    max: usize,
    message: &str,
) -> Result<(String, String), AppError> {
    let Some(cleaned) = clean_description(raw) else {
        return Err(validation_error(message));
    };

    if cleaned.chars().count() > max || has_forbidden_format_chars(&cleaned) {
        return Err(validation_error(message));
    }

    let normalized = normalize_text(&cleaned);
    Ok((cleaned, normalized))
}

#[derive(Debug, Clone, Copy)]
pub struct ContactInput<'a> {
    pub first_name: &'a str,
    pub last_name: &'a str,
    pub national_id: &'a str,
    pub phone: &'a str,
    pub company: Option<&'a str>,
    pub position: Option<&'a str>,
    pub email: Option<&'a str>,
    pub address: Option<&'a str>,
    pub invoice_info: Option<&'a str>,
}

#[derive(Debug, PartialEq, Eq)]
pub struct ValidatedContactInput {
    pub first_name: String,
    pub first_name_normalized: String,
    pub last_name: Option<String>,
    pub last_name_normalized: Option<String>,
    pub national_id: Option<String>,
    pub phone: Option<String>,
    pub company: Option<String>,
    pub company_normalized: Option<String>,
    pub position: Option<String>,
    pub email: Option<String>,
    pub address: Option<String>,
    pub invoice_info: Option<String>,
}

pub fn validate_client_input(input: ContactInput<'_>) -> Result<ValidatedContactInput, AppError> {
    validate_contact_input(input, true)
}

pub fn validate_supplier_input(input: ContactInput<'_>) -> Result<ValidatedContactInput, AppError> {
    validate_contact_input(input, false)
}

fn validate_contact_input(
    input: ContactInput<'_>,
    include_invoice_info: bool,
) -> Result<ValidatedContactInput, AppError> {
    let (first_name, first_name_normalized) = validate_name_with_messages(
        input.first_name,
        2,
        50,
        "El nombre debe tener al menos 2 caracteres",
        "El nombre no puede tener más de 50 caracteres",
        "El nombre contiene caracteres no permitidos",
    )?;
    // spec-06 CT-01: solo el nombre es obligatorio; lo demás, si se escribe,
    // conserva su formato.
    let (last_name, last_name_normalized) = if input.last_name.trim().is_empty() {
        (None, None)
    } else {
        let (last_name, normalized) = validate_name_with_messages(
            input.last_name,
            2,
            50,
            "Los apellidos deben tener al menos 2 caracteres",
            "Los apellidos no pueden tener más de 50 caracteres",
            "Los apellidos contienen caracteres no permitidos",
        )?;
        (Some(last_name), Some(normalized))
    };
    let national_id = Some(input.national_id.trim()).filter(|value| !value.is_empty());
    if let Some(national_id) = national_id {
        validate_national_id(national_id)?;
    }
    let phone = Some(input.phone.trim()).filter(|value| !value.is_empty());
    if let Some(phone) = phone {
        validate_phone(phone)?;
    }

    let (company, company_normalized) =
        validate_optional_contact_text(input.company, 100, "La empresa")?;
    let (position, _) = validate_optional_contact_text(input.position, 100, "El cargo")?;
    let email = validate_optional_email(input.email)?;
    let (address, _) = validate_optional_contact_text(input.address, 200, "La dirección")?;
    let (invoice_info, _) = if include_invoice_info {
        validate_optional_contact_text(input.invoice_info, 200, "La información de factura")?
    } else {
        (None, None)
    };

    Ok(ValidatedContactInput {
        first_name,
        first_name_normalized,
        last_name,
        last_name_normalized,
        national_id: national_id.map(str::to_owned),
        phone: phone.map(str::to_owned),
        company,
        company_normalized,
        position,
        email,
        address,
        invoice_info,
    })
}

fn validate_optional_contact_text(
    raw: Option<&str>,
    max: usize,
    label: &str,
) -> Result<(Option<String>, Option<String>), AppError> {
    let Some(cleaned) = raw.map(clean_name) else {
        return Ok((None, None));
    };
    if cleaned.is_empty() {
        return Ok((None, None));
    }
    if has_forbidden_format_chars(&cleaned) {
        return Err(validation_error(&format!(
            "{label} contiene caracteres no permitidos"
        )));
    }
    if cleaned.chars().count() > max {
        return Err(validation_error(&format!(
            "{label} no puede tener más de {max} caracteres"
        )));
    }

    let normalized = normalize_text(&cleaned);
    Ok((Some(cleaned), Some(normalized)))
}

fn validate_optional_email(raw: Option<&str>) -> Result<Option<String>, AppError> {
    let Some(cleaned) = raw.map(clean_name) else {
        return Ok(None);
    };
    if cleaned.is_empty() {
        return Ok(None);
    }
    if !crate::text::is_valid_email(&cleaned) {
        return Err(validation_error("El correo no tiene un formato válido"));
    }
    Ok(Some(cleaned))
}

pub fn validate_invoice_notes(raw: &str) -> Result<Option<String>, AppError> {
    let Some(cleaned) = clean_description(raw) else {
        return Ok(None);
    };
    if has_forbidden_format_chars(&cleaned) {
        return Err(validation_error(
            "Las observaciones contienen caracteres no permitidos",
        ));
    }
    if cleaned.chars().count() > 500 {
        return Err(validation_error(
            "Las observaciones no pueden tener más de 500 caracteres",
        ));
    }
    Ok(Some(cleaned))
}

pub fn validate_attachment_original_name(raw: &str) -> Result<String, AppError> {
    let cleaned = clean_name(raw);
    if cleaned.is_empty()
        || cleaned.chars().count() > 100
        || cleaned.contains(['/', '\\'])
        || has_forbidden_format_chars(&cleaned)
    {
        return Err(validation_error(
            "El nombre del archivo adjunto no es válido",
        ));
    }
    Ok(cleaned)
}

fn parse_decimal_parts(raw: &str) -> Result<(String, Option<String>), AppError> {
    let trimmed = raw.trim();
    if trimmed.is_empty() || trimmed.bytes().any(|byte| byte.is_ascii_whitespace()) {
        return Err(validation_error("El valor ingresado no es válido"));
    }

    let has_comma = trimmed.contains(',');
    let has_dot = trimmed.contains('.');
    if (has_comma && has_dot) || trimmed.matches(',').count() + trimmed.matches('.').count() > 1 {
        return Err(validation_error("El valor ingresado no es válido"));
    }

    let mut normalized = String::new();
    for ch in trimmed.chars() {
        if ch.is_ascii_digit() || ch == '.' || ch == ',' {
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

    if let Some(frac) = fraction_part {
        let frac_digits = frac.chars().filter(|ch| ch.is_ascii_digit()).count();
        let whole_is_nonzero = !whole.trim_start_matches('0').is_empty();
        if normalized.contains('.') && frac_digits == 3 && whole_is_nonzero {
            return Err(validation_error("El valor tiene un formato ambiguo"));
        }
        if frac_digits > decimal_places {
            return Err(validation_error("El valor tiene demasiados decimales"));
        }
    }

    if whole_digits > max_integer_digits {
        return Err(validation_error("El valor está fuera de rango"));
    }

    let scaled = if let Some(frac) = fraction_part {
        let mut digits = frac.to_owned();
        while digits.len() < decimal_places {
            digits.push('0');
        }
        let whole_part = if whole.is_empty() { "0" } else { whole };
        if whole_part == "0" && digits == "0".repeat(decimal_places) {
            0
        } else {
            let mut integer_value = whole_part.parse::<i128>().unwrap_or(0);
            integer_value *= 10_i128.pow(decimal_places as u32);
            let fraction_value = digits.parse::<i128>().unwrap_or(0);
            integer_value + fraction_value
        }
    } else {
        let whole_part = if whole.is_empty() { "0" } else { whole };
        whole_part
            .parse::<i128>()
            .unwrap_or(0)
            .checked_mul(10_i128.pow(decimal_places as u32))
            .ok_or_else(|| validation_error("El valor está fuera de rango"))?
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
        return Err(validation_error(
            "La fecha de caducidad no puede ser posterior al año 2100",
        ));
    }

    if parsed < today && actual != Some(cleaned) {
        return Err(validation_error(
            "La fecha de caducidad no puede ser anterior a hoy",
        ));
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
        parse_quantity_micro, parse_quantity_milli, validate_attachment_original_name,
        validate_client_input, validate_description, validate_expiry_date, validate_invoice_notes,
        validate_name, validate_national_id, validate_not_reserved, validate_password,
        validate_phone, validate_role, validate_supplier_input, validate_username, ContactInput,
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
        let (clean_name, normalized_name) =
            validate_name("  Café   con  ñ  ", 2, 50, "Nombre inválido").unwrap();
        assert_eq!(clean_name, "Café con ñ");
        assert_eq!(normalized_name, "cafe con ñ");

        let (clean_desc, normalized_desc) =
            validate_description("  Dos\n\t líneas  ", 200, "Descripción inválida").unwrap();
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

        assert_eq!(
            validate_expiry_date("2026-10-03", today, None).unwrap(),
            "2026-10-03"
        );
        assert!(validate_expiry_date("2026-10-02", today, None).is_err());
        assert!(validate_expiry_date("2026-02-30", today, None).is_err());
        assert!(validate_expiry_date("2026-2-3", today, None).is_err());
        assert!(validate_expiry_date("2101-01-01", today, None).is_err());
        assert_eq!(
            validate_expiry_date("2100-12-31", today, None).unwrap(),
            "2100-12-31"
        );

        let expired = "2026-10-02";
        assert_eq!(
            validate_expiry_date(expired, today, Some(expired)).unwrap(),
            expired
        );
    }

    #[test]
    fn contact_validators_clean_fields_normalize_search_values_and_validate_optional_data() {
        let valid_input = ContactInput {
            first_name: "  Ana   María ",
            last_name: " Pérez  ",
            national_id: "12345678901",
            phone: "+5355551234",
            company: Some("  Servicios   Norte  "),
            position: Some("  Técnica  "),
            email: Some(" ana@example.com "),
            address: Some("  Calle   10  "),
            invoice_info: Some("  Nombre fiscal  "),
        };
        let validated = validate_client_input(valid_input).unwrap();
        assert_eq!(validated.first_name, "Ana María");
        assert_eq!(validated.first_name_normalized, "ana maria");
        assert_eq!(validated.last_name.as_deref(), Some("Pérez"));
        assert_eq!(validated.last_name_normalized.as_deref(), Some("perez"));
        assert_eq!(validated.national_id.as_deref(), Some("12345678901"));
        assert_eq!(validated.phone.as_deref(), Some("+5355551234"));

        // spec-06 CT-01: solo el nombre es obligatorio.
        let only_name = validate_client_input(ContactInput {
            first_name: "Taller",
            last_name: " ",
            national_id: "",
            phone: "",
            company: None,
            position: None,
            email: None,
            address: None,
            invoice_info: None,
        })
        .unwrap();
        assert_eq!(
            (only_name.last_name, only_name.national_id, only_name.phone),
            (None, None, None)
        );
        assert!(validate_client_input(ContactInput {
            first_name: "",
            last_name: "",
            national_id: "",
            phone: "",
            company: None,
            position: None,
            email: None,
            address: None,
            invoice_info: None,
        })
        .is_err());
        assert_eq!(validated.company.as_deref(), Some("Servicios Norte"));
        assert_eq!(
            validated.company_normalized.as_deref(),
            Some("servicios norte")
        );
        assert_eq!(validated.position.as_deref(), Some("Técnica"));
        assert_eq!(validated.email.as_deref(), Some("ana@example.com"));
        assert_eq!(validated.address.as_deref(), Some("Calle 10"));
        assert_eq!(validated.invoice_info.as_deref(), Some("Nombre fiscal"));

        let empty_optional_fields = ContactInput {
            first_name: "Ana",
            last_name: "Pérez",
            national_id: "12345678901",
            phone: "+5355551234",
            company: Some("  "),
            position: None,
            email: None,
            address: Some(""),
            invoice_info: Some("\t"),
        };
        let validated = validate_client_input(empty_optional_fields).unwrap();
        assert_eq!(validated.company, None);
        assert_eq!(validated.position, None);
        assert_eq!(validated.email, None);
        assert_eq!(validated.address, None);
        assert_eq!(validated.invoice_info, None);

        let supplier = validate_supplier_input(ContactInput {
            first_name: "Ana",
            last_name: "Pérez",
            national_id: "12345678901",
            phone: "+5355551234",
            company: None,
            position: None,
            email: None,
            address: None,
            invoice_info: None,
        })
        .unwrap();
        assert_eq!(supplier.first_name_normalized, "ana");
        assert_eq!(supplier.invoice_info, None);

        assert_eq!(
            validate_invoice_notes("  Nota   breve\nsegunda  ")
                .unwrap()
                .as_deref(),
            Some("Nota breve\nsegunda")
        );
        assert_eq!(validate_invoice_notes("  \n ").unwrap(), None);
        assert_eq!(
            validate_attachment_original_name("  factura.pdf  ").unwrap(),
            "factura.pdf"
        );
    }

    #[test]
    fn contact_validators_reject_invalid_lengths_formats_and_control_characters() {
        for invalid_name in ["A", &"a".repeat(51)] {
            assert_eq!(
                validate_client_input(ContactInput {
                    first_name: invalid_name,
                    last_name: "Pérez",
                    national_id: "12345678901",
                    phone: "+5355551234",
                    company: None,
                    position: None,
                    email: None,
                    address: None,
                    invoice_info: None,
                })
                .unwrap_err()
                .code,
                "validationFailed"
            );
        }

        for (field, value) in [
            ("last_name", "P"),
            ("national_id", "1234567890x"),
            ("phone", "+535555123"),
            ("company", &"c".repeat(101)),
            ("position", &"p".repeat(101)),
            ("address", &"d".repeat(201)),
            ("invoice_info", &"i".repeat(201)),
        ] {
            let mut input = ContactInput {
                first_name: "Ana",
                last_name: "Pérez",
                national_id: "12345678901",
                phone: "+5355551234",
                company: None,
                position: None,
                email: None,
                address: None,
                invoice_info: None,
            };
            match field {
                "last_name" => input.last_name = value,
                "national_id" => input.national_id = value,
                "phone" => input.phone = value,
                "company" => input.company = Some(value),
                "position" => input.position = Some(value),
                "address" => input.address = Some(value),
                "invoice_info" => input.invoice_info = Some(value),
                _ => unreachable!(),
            }
            assert_eq!(
                validate_client_input(input).unwrap_err().code,
                "validationFailed"
            );
        }

        let invalid_email = ContactInput {
            first_name: "Ana",
            last_name: "Pérez",
            national_id: "12345678901",
            phone: "+5355551234",
            company: None,
            position: None,
            email: Some("ana@dominio"),
            address: None,
            invoice_info: None,
        };
        assert_validation_error(
            validate_client_input(invalid_email),
            "El correo no tiene un formato válido",
        );

        let forbidden_format = ContactInput {
            first_name: "Ana\u{202e}",
            last_name: "Pérez",
            national_id: "12345678901",
            phone: "+5355551234",
            company: None,
            position: None,
            email: None,
            address: None,
            invoice_info: None,
        };
        assert_eq!(
            validate_supplier_input(forbidden_format).unwrap_err().code,
            "validationFailed"
        );

        assert_eq!(
            validate_invoice_notes(&"n".repeat(501)).unwrap_err().code,
            "validationFailed"
        );
        for invalid_name in [
            "folder/factura.pdf",
            "folder\\factura.pdf",
            "",
            &"f".repeat(101),
        ] {
            assert_eq!(
                validate_attachment_original_name(invalid_name)
                    .unwrap_err()
                    .code,
                "validationFailed"
            );
        }
    }
}
