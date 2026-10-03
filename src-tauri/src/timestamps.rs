use chrono::{DateTime, SecondsFormat, Utc};

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

#[cfg(test)]
mod tests {
    use chrono::{TimeZone, Utc};

    use super::{format_utc_timestamp, parse_utc_timestamp};

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
}
