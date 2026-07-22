//! Reopen, handoff, and scope-change event detection from changelog-shaped inputs.

use chrono::{DateTime, Utc};

use crate::changelog::StatusTransition;
use crate::flow::{default_status_category, StatusFlowCategory};

/// A single field change (e.g. Story Points or Sprint) from changelog.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct FieldChange {
    pub issue_id: String,
    pub field: String,
    pub from_string: Option<String>,
    pub to_string: Option<String>,
    pub at: DateTime<Utc>,
}

/// Counts of mid-flight scope additions and removals.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub struct ScopeChangeStats {
    pub scope_added: u32,
    pub scope_removed: u32,
}

/// Count Done → non-Done transitions (reopens).
pub fn detect_reopens(transitions: &[StatusTransition]) -> u32 {
    transitions
        .iter()
        .filter(|tr| {
            let from_done = tr.from_status.as_deref().map(is_terminal).unwrap_or(false);
            from_done && !is_terminal(&tr.to_status)
        })
        .count() as u32
}

/// Count assignee changes where from ≠ to (including assign / unassign).
pub fn detect_handoffs(
    assignee_changes: &[(DateTime<Utc>, Option<String>, Option<String>)],
) -> u32 {
    assignee_changes
        .iter()
        .filter(|(_, from, to)| from != to)
        .count() as u32
}

/// Detect scope adds/removes from story-point and sprint field changes.
pub fn detect_scope_changes(field_changes: &[FieldChange]) -> ScopeChangeStats {
    let mut stats = ScopeChangeStats::default();
    for change in field_changes {
        let field = change.field.to_ascii_lowercase();
        if field.contains("story point") {
            let from = parse_points(change.from_string.as_deref());
            let to = parse_points(change.to_string.as_deref());
            match (from, to) {
                (Some(a), Some(b)) if b > a => stats.scope_added += 1,
                (Some(a), Some(b)) if b < a => stats.scope_removed += 1,
                (None, Some(_)) => stats.scope_added += 1,
                (Some(_), None) => stats.scope_removed += 1,
                _ => {}
            }
        } else if field == "sprint" {
            let from_empty = change
                .from_string
                .as_deref()
                .map(str::trim)
                .unwrap_or("")
                .is_empty();
            let to_empty = change
                .to_string
                .as_deref()
                .map(str::trim)
                .unwrap_or("")
                .is_empty();
            match (from_empty, to_empty) {
                (true, false) => stats.scope_added += 1,
                (false, true) => stats.scope_removed += 1,
                (false, false) if change.from_string != change.to_string => {
                    // Sprint-to-sprint move: leave one, join another.
                    stats.scope_removed += 1;
                    stats.scope_added += 1;
                }
                _ => {}
            }
        }
    }
    stats
}

fn is_terminal(status: &str) -> bool {
    default_status_category(status) == StatusFlowCategory::Terminal
}

fn parse_points(raw: Option<&str>) -> Option<f64> {
    raw.and_then(|s| s.trim().parse::<f64>().ok())
}

#[cfg(test)]
mod tests {
    use super::*;
    use chrono::TimeZone;

    #[test]
    fn detect_reopens_counts_done_to_non_done() {
        let t1 = Utc.with_ymd_and_hms(2024, 1, 1, 10, 0, 0).unwrap();
        let t2 = Utc.with_ymd_and_hms(2024, 1, 1, 12, 0, 0).unwrap();
        let t3 = Utc.with_ymd_and_hms(2024, 1, 2, 9, 0, 0).unwrap();
        let transitions = vec![
            StatusTransition {
                issue_id: "1".into(),
                from_status: Some("In Progress".into()),
                to_status: "Done".into(),
                at: t1,
            },
            StatusTransition {
                issue_id: "1".into(),
                from_status: Some("Done".into()),
                to_status: "In Progress".into(),
                at: t2,
            },
            StatusTransition {
                issue_id: "1".into(),
                from_status: Some("In Progress".into()),
                to_status: "Done".into(),
                at: t3,
            },
        ];
        assert_eq!(detect_reopens(&transitions), 1);
    }

    #[test]
    fn detect_handoffs_counts_assignee_changes() {
        let t = Utc.with_ymd_and_hms(2024, 1, 1, 10, 0, 0).unwrap();
        let changes = vec![
            (t, None, Some("ada".into())),
            (t, Some("ada".into()), Some("bob".into())),
            (t, Some("bob".into()), Some("bob".into())), // no-op
        ];
        assert_eq!(detect_handoffs(&changes), 2);
    }

    #[test]
    fn detect_scope_changes_tracks_points_and_sprint() {
        let t = Utc.with_ymd_and_hms(2024, 1, 1, 10, 0, 0).unwrap();
        let changes = vec![
            FieldChange {
                issue_id: "1".into(),
                field: "Story Points".into(),
                from_string: Some("3".into()),
                to_string: Some("5".into()),
                at: t,
            },
            FieldChange {
                issue_id: "1".into(),
                field: "Story Points".into(),
                from_string: Some("5".into()),
                to_string: Some("2".into()),
                at: t,
            },
            FieldChange {
                issue_id: "2".into(),
                field: "Sprint".into(),
                from_string: None,
                to_string: Some("Sprint 1".into()),
                at: t,
            },
            FieldChange {
                issue_id: "3".into(),
                field: "Sprint".into(),
                from_string: Some("Sprint 1".into()),
                to_string: None,
                at: t,
            },
        ];
        let got = detect_scope_changes(&changes);
        assert_eq!(got.scope_added, 2); // +points event + sprint join
        assert_eq!(got.scope_removed, 2); // -points event + sprint leave
    }
}
