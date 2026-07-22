//! Daily throughput from issue completion timestamps.

use std::collections::{BTreeMap, BTreeSet};

use chrono::{DateTime, NaiveDate, Utc};

/// Count completed issues per UTC calendar day.
///
/// Each `(issue_id, completed_at)` pair contributes one completion on that day.
/// Duplicate issue ids on the same day are counted once.
pub fn daily_throughput(
    completed_at_by_issue: &[(String, DateTime<Utc>)],
) -> BTreeMap<NaiveDate, u64> {
    let mut seen: BTreeSet<(NaiveDate, &str)> = BTreeSet::new();
    let mut counts: BTreeMap<NaiveDate, u64> = BTreeMap::new();

    for (issue_id, at) in completed_at_by_issue {
        let day = at.date_naive();
        if seen.insert((day, issue_id.as_str())) {
            *counts.entry(day).or_insert(0) += 1;
        }
    }
    counts
}

#[cfg(test)]
mod tests {
    use super::*;
    use chrono::TimeZone;

    #[test]
    fn daily_throughput_groups_by_utc_day_and_dedupes_issue() {
        let d1a = Utc.with_ymd_and_hms(2024, 3, 1, 9, 0, 0).unwrap();
        let d1b = Utc.with_ymd_and_hms(2024, 3, 1, 18, 0, 0).unwrap();
        let d2 = Utc.with_ymd_and_hms(2024, 3, 2, 12, 0, 0).unwrap();

        let got = daily_throughput(&[
            ("A-1".into(), d1a),
            ("A-2".into(), d1b),
            ("A-1".into(), d1b), // same issue, same day → once
            ("A-3".into(), d2),
        ]);

        assert_eq!(
            got.get(&NaiveDate::from_ymd_opt(2024, 3, 1).unwrap()),
            Some(&2)
        );
        assert_eq!(
            got.get(&NaiveDate::from_ymd_opt(2024, 3, 2).unwrap()),
            Some(&1)
        );
    }
}
