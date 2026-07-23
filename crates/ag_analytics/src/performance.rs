//! Performance rollup helpers (person-month bucketing).

use chrono::{DateTime, Utc};

/// UTC calendar month key `YYYY-MM` for a completion timestamp.
pub fn month_key(at: DateTime<Utc>) -> String {
    at.format("%Y-%m").to_string()
}

#[cfg(test)]
mod tests {
    use super::*;
    use chrono::TimeZone;

    #[test]
    fn month_key_is_utc_yyyy_mm() {
        let at = Utc.with_ymd_and_hms(2024, 3, 15, 23, 0, 0).unwrap();
        assert_eq!(month_key(at), "2024-03");
    }
}
