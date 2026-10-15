use chrono::{DateTime, Local, NaiveDate, SecondsFormat, Utc};

use crate::errors::AppError;

pub fn format_utc_timestamp(value: DateTime<Utc>) -> String {
    value.to_rfc3339_opts(SecondsFormat::Secs, true)
}

pub fn parse_utc_timestamp(value: &str) -> Option<DateTime<Utc>> {
    let parsed = DateTime::parse_from_rfc3339(value).ok()?;
    let utc = parsed.with_timezone(&Utc);
    let normalized = utc.to_rfc3339_opts(SecondsFormat::Secs, true);

    if normalized == value {
        Some(utc)
    } else {
        None
    }
}

pub fn today_local(now: DateTime<Utc>) -> NaiveDate {
    now.with_timezone(&Local).date_naive()
}

pub fn parse_invoice_date(raw: &str) -> Result<NaiveDate, AppError> {
    let bytes = raw.as_bytes();
    if bytes.len() != 10
        || bytes[4] != b'-'
        || bytes[7] != b'-'
        || !bytes
            .iter()
            .enumerate()
            .all(|(index, byte)| matches!(index, 4 | 7) || byte.is_ascii_digit())
    {
        return Err(AppError::new(
            "validationFailed",
            "La fecha no es válida",
            "invalid invoice date format",
        ));
    }

    let date = NaiveDate::parse_from_str(raw, "%Y-%m-%d").map_err(|_| {
        AppError::new(
            "validationFailed",
            "La fecha no es válida",
            "invalid invoice date",
        )
    })?;

    if date < NaiveDate::from_ymd_opt(2000, 1, 1).expect("fecha constante válida") {
        return Err(AppError::new(
            "validationFailed",
            "La fecha no puede ser anterior al 1 de enero de 2000",
            "invoice date before supported range",
        ));
    }

    Ok(date)
}

pub fn is_future(date: NaiveDate, today: NaiveDate) -> bool {
    date > today
}

#[cfg(test)]
mod tests {
    use chrono::{Local, NaiveDate, TimeZone, Utc};

    use super::{
        format_utc_timestamp, is_future, parse_invoice_date, parse_utc_timestamp, today_local,
    };

    #[test]
    fn format_utc_timestamp_uses_fixed_second_precision_utc() {
        let dt = Utc.with_ymd_and_hms(2026, 10, 3, 14, 15, 59).unwrap();
        assert_eq!(format_utc_timestamp(dt), "2026-10-03T14:15:59Z");

        let with_fraction = Utc.with_ymd_and_hms(2026, 10, 3, 14, 15, 59).unwrap()
            + chrono::Duration::milliseconds(999);
        assert_eq!(format_utc_timestamp(with_fraction), "2026-10-03T14:15:59Z");
    }

    #[test]
    fn parse_utc_timestamp_round_trips_and_rejects_invalid_formats() {
        let value = "2026-10-03T14:15:59Z";
        let parsed = parse_utc_timestamp(value).unwrap();
        assert_eq!(format_utc_timestamp(parsed), value);

        assert!(parse_utc_timestamp("2026-10-03 14:15:59Z").is_none());
        assert!(parse_utc_timestamp("2026-10-03T14:15:59.123Z").is_none());
        assert!(parse_utc_timestamp("2026-10-03T14:15:59+00:00").is_none());
        assert!(parse_utc_timestamp("2026-13-01T00:00:00Z").is_none());
        assert!(parse_utc_timestamp("2026-02-30T00:00:00Z").is_none());
    }

    #[test]
    fn invoice_date_helpers_use_local_today_and_reject_invalid_dates() {
        let now = Utc.with_ymd_and_hms(2026, 1, 15, 2, 30, 0).unwrap();
        assert_eq!(today_local(now), now.with_timezone(&Local).date_naive());

        assert_eq!(
            parse_invoice_date("2026-01-15").unwrap(),
            NaiveDate::from_ymd_opt(2026, 1, 15).unwrap()
        );
        for invalid in ["2026-02-30", "2026-1-1", "1999-12-31"] {
            assert_eq!(
                parse_invoice_date(invalid).unwrap_err().code,
                "validationFailed"
            );
        }

        let today = NaiveDate::from_ymd_opt(2026, 1, 15).unwrap();
        let tomorrow = NaiveDate::from_ymd_opt(2026, 1, 16).unwrap();
        assert!(is_future(tomorrow, today));
        assert!(!is_future(today, today));
    }
}
