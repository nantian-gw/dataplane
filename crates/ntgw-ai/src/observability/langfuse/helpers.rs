use std::time::{SystemTime, UNIX_EPOCH};

/// Format the current UTC time as an ISO 8601 string with millisecond precision.
pub(crate) fn iso8601_now() -> String {
    let since_epoch = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .unwrap_or_default();

    let secs = since_epoch.as_secs();
    let millis = since_epoch.subsec_millis();

    // Days since epoch, then decompose into year/month/day.
    let total_days = secs / 86400;
    let time_secs = secs % 86400;
    let hours = time_secs / 3600;
    let minutes = (time_secs % 3600) / 60;
    let seconds = time_secs % 60;

    let (year, month, day) = days_to_civil(total_days as i64);

    format!("{year:04}-{month:02}-{day:02}T{hours:02}:{minutes:02}:{seconds:02}.{millis:03}Z")
}

/// Convert days since 1970-01-01 to (year, month, day).
pub(crate) fn days_to_civil(mut days: i64) -> (i64, u32, u32) {
    // Algorithm from Howard Hinnant's civil_from_days.
    // Shift epoch from 1970-01-01 to 0000-03-01.
    days += 719468;
    let era = if days >= 0 {
        days / 146097
    } else {
        (days - 146096) / 146097
    };
    let day_of_era = days - era * 146097;
    let year_of_era =
        (day_of_era - day_of_era / 1460 + day_of_era / 36524 - day_of_era / 146096) / 365;
    let year = year_of_era + era * 400;
    let day_of_year = day_of_era - (365 * year_of_era + year_of_era / 4 - year_of_era / 100);
    let month_ordinal = (5 * day_of_year + 2) / 153;
    let day = day_of_year - (153 * month_ordinal + 2) / 5 + 1;
    let month = if month_ordinal < 10 {
        month_ordinal + 3
    } else {
        month_ordinal - 9
    };
    let year = if month <= 2 { year + 1 } else { year };

    (year, month as u32, day as u32)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_iso8601_now_produces_valid_format() {
        let ts = iso8601_now();
        assert_eq!(ts.len(), 24, "ISO 8601 string should be 24 chars");
        assert!(ts.ends_with('Z'), "should end with Z for UTC");
        assert_eq!(&ts[4..5], "-");
        assert_eq!(&ts[7..8], "-");
        assert_eq!(&ts[10..11], "T");
        assert_eq!(&ts[13..14], ":");
        assert_eq!(&ts[16..17], ":");
        assert_eq!(&ts[19..20], ".");
    }

    #[test]
    fn test_days_to_civil_epoch() {
        let (y, m, d) = days_to_civil(0);
        assert_eq!(y, 1970);
        assert_eq!(m, 1);
        assert_eq!(d, 1);
    }

    #[test]
    fn test_days_to_civil_known_date() {
        // 2026-05-30. Days since epoch: compute via known value.
        // 2026-01-01 = day 20454. May 30 is day 150 of 2026 (non-leap).
        // 20454 + 149 = 20603 (since Jan 1 is day 1).
        let (y, m, d) = days_to_civil(20603);
        assert_eq!((y, m, d), (2026, 5, 30));
    }
}
