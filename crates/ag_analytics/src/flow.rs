//! Time-in-status, cycle time, and lead time.

use std::collections::BTreeMap;

use chrono::{DateTime, Utc};

use crate::changelog::StatusTransition;

/// Aggregated calendar time spent in a status.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct TimeInStatus {
    pub issue_id: String,
    pub status: String,
    pub duration_secs: i64,
}

/// Cycle and lead times for an issue.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct CycleLeadTimes {
    pub issue_id: String,
    pub cycle_secs: Option<i64>,
    pub lead_secs: Option<i64>,
}

/// Flow classification for a status name.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum StatusFlowCategory {
    Active,
    Waiting,
    Terminal,
}

/// Default status → flow category mapping.
///
/// Defaults: `In Progress`/active; `To Do`/waiting; `Done` terminal.
/// Unknown names are treated as waiting.
pub fn default_status_category(status: &str) -> StatusFlowCategory {
    match status {
        "In Progress" => StatusFlowCategory::Active,
        "Done" => StatusFlowCategory::Terminal,
        "To Do" => StatusFlowCategory::Waiting,
        _ => StatusFlowCategory::Waiting,
    }
}

/// Resolve a status category from an optional override map, else defaults.
pub fn resolve_status_category(
    status: &str,
    overrides: &BTreeMap<String, StatusFlowCategory>,
) -> StatusFlowCategory {
    overrides
        .get(status)
        .copied()
        .unwrap_or_else(|| default_status_category(status))
}

/// Accumulate time spent in each status across transitions (including reopens).
///
/// Durations run from each transition's `at` until the next transition.
/// The final status runs until `done_at` when present, otherwise `now`.
pub fn time_in_status(
    transitions: &[StatusTransition],
    done_at: Option<DateTime<Utc>>,
    now: DateTime<Utc>,
) -> Vec<TimeInStatus> {
    if transitions.is_empty() {
        return Vec::new();
    }

    let mut ordered: Vec<&StatusTransition> = transitions.iter().collect();
    ordered.sort_by(|a, b| a.at.cmp(&b.at));

    let issue_id = ordered[0].issue_id.clone();
    let mut totals: BTreeMap<String, i64> = BTreeMap::new();

    for (idx, tr) in ordered.iter().enumerate() {
        let start = tr.at;
        let end = if let Some(next) = ordered.get(idx + 1) {
            next.at
        } else if let Some(done) = done_at {
            done
        } else {
            now
        };
        let secs = (end - start).num_seconds();
        if secs > 0 {
            *totals.entry(tr.to_status.clone()).or_insert(0) += secs;
        }
    }

    totals
        .into_iter()
        .map(|(status, duration_secs)| TimeInStatus {
            issue_id: issue_id.clone(),
            status,
            duration_secs,
        })
        .collect()
}

/// Cycle = first in-progress → done; lead = created → done.
pub fn cycle_and_lead(
    created: DateTime<Utc>,
    first_in_progress: Option<DateTime<Utc>>,
    completed: Option<DateTime<Utc>>,
) -> CycleLeadTimes {
    let cycle_secs = match (first_in_progress, completed) {
        (Some(start), Some(end)) if end >= start => Some((end - start).num_seconds()),
        _ => None,
    };
    let lead_secs = completed
        .filter(|end| *end >= created)
        .map(|end| (end - created).num_seconds());

    CycleLeadTimes {
        issue_id: String::new(),
        cycle_secs,
        lead_secs,
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use chrono::TimeZone;

    #[test]
    #[allow(unused_variables)]
    fn time_in_status_handles_reopen_and_same_day_moves() {
        let t0 = Utc.with_ymd_and_hms(2024, 1, 1, 9, 0, 0).unwrap();
        let t1 = Utc.with_ymd_and_hms(2024, 1, 1, 10, 0, 0).unwrap(); // To Do -> In Progress
        let t2 = Utc.with_ymd_and_hms(2024, 1, 1, 12, 0, 0).unwrap(); // In Progress -> Done
        let t3 = Utc.with_ymd_and_hms(2024, 1, 2, 9, 0, 0).unwrap(); // Done -> In Progress (reopen)
        let t4 = Utc.with_ymd_and_hms(2024, 1, 2, 11, 0, 0).unwrap(); // In Progress -> Done

        let transitions = vec![
            StatusTransition {
                issue_id: "1".into(),
                from_status: Some("To Do".into()),
                to_status: "In Progress".into(),
                at: t1,
            },
            StatusTransition {
                issue_id: "1".into(),
                from_status: Some("In Progress".into()),
                to_status: "Done".into(),
                at: t2,
            },
            StatusTransition {
                issue_id: "1".into(),
                from_status: Some("Done".into()),
                to_status: "In Progress".into(),
                at: t3,
            },
            StatusTransition {
                issue_id: "1".into(),
                from_status: Some("In Progress".into()),
                to_status: "Done".into(),
                at: t4,
            },
        ];

        let tis = time_in_status(&transitions, Some(t4), t4);
        let in_progress: i64 = tis
            .iter()
            .filter(|r| r.status == "In Progress")
            .map(|r| r.duration_secs)
            .sum();
        assert_eq!(in_progress, 2 * 3600 + 2 * 3600); // 10-12 and 09-11
    }

    #[test]
    fn cycle_time_is_first_in_progress_to_done() {
        let created = Utc.with_ymd_and_hms(2024, 1, 1, 8, 0, 0).unwrap();
        let first_in_progress = Utc.with_ymd_and_hms(2024, 1, 1, 10, 0, 0).unwrap();
        let completed = Utc.with_ymd_and_hms(2024, 1, 1, 14, 0, 0).unwrap();

        let got = cycle_and_lead(created, Some(first_in_progress), Some(completed));
        assert_eq!(got.cycle_secs, Some(4 * 3600));
    }

    #[test]
    fn lead_time_is_created_to_done() {
        let created = Utc.with_ymd_and_hms(2024, 1, 1, 8, 0, 0).unwrap();
        let first_in_progress = Utc.with_ymd_and_hms(2024, 1, 1, 10, 0, 0).unwrap();
        let completed = Utc.with_ymd_and_hms(2024, 1, 2, 8, 0, 0).unwrap();

        let got = cycle_and_lead(created, Some(first_in_progress), Some(completed));
        assert_eq!(got.lead_secs, Some(24 * 3600));
    }
}
