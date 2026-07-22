//! Bottleneck ranking from time-in-status aggregates.

use std::collections::HashMap;

use crate::flow::TimeInStatus;

/// Sum time-in-status across issues, returning `(status, total_secs)` ordered descending.
pub fn bottleneck_by_status(time_in_status: &[TimeInStatus]) -> Vec<(String, i64)> {
    let mut totals: HashMap<String, i64> = HashMap::new();
    for row in time_in_status {
        *totals.entry(row.status.clone()).or_insert(0) += row.duration_secs;
    }
    let mut out: Vec<(String, i64)> = totals.into_iter().collect();
    out.sort_by(|a, b| b.1.cmp(&a.1).then_with(|| a.0.cmp(&b.0)));
    out
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn bottleneck_by_status_sums_and_sorts_descending() {
        let rows = vec![
            TimeInStatus {
                issue_id: "1".into(),
                status: "Code Review".into(),
                duration_secs: 10_000,
            },
            TimeInStatus {
                issue_id: "2".into(),
                status: "In Progress".into(),
                duration_secs: 4_000,
            },
            TimeInStatus {
                issue_id: "3".into(),
                status: "Code Review".into(),
                duration_secs: 5_000,
            },
        ];
        let got = bottleneck_by_status(&rows);
        assert_eq!(
            got,
            vec![
                ("Code Review".into(), 15_000),
                ("In Progress".into(), 4_000),
            ]
        );
    }
}
