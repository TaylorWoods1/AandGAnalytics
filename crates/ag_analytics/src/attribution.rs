//! Hybrid completion finisher attribution from assignee changelog.

use chrono::{DateTime, Utc};

/// How the finisher account id was chosen.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum AttributionSource {
    /// Assignee on/ before the Done transition from changelog.
    Changelog,
    /// Fallback to the issue's current assignee.
    Current,
}

/// One assignee field change (ordered by time when resolving).
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct AssigneeChange {
    pub at: DateTime<Utc>,
    /// New assignee account id after the change (`None` = unassigned).
    pub to_account_id: Option<String>,
}

/// Resolve who finished the issue at `done_at`.
///
/// Prefers the last assignee changelog entry at or before `done_at`.
/// If none exists, falls back to `current_assignee`.
pub fn resolve_finisher(
    assignee_changelog: &[AssigneeChange],
    done_at: DateTime<Utc>,
    current_assignee: Option<&str>,
) -> (Option<String>, AttributionSource) {
    let mut best: Option<&AssigneeChange> = None;
    for change in assignee_changelog {
        if change.at <= done_at {
            best = Some(change);
        }
    }
    if let Some(change) = best {
        return (change.to_account_id.clone(), AttributionSource::Changelog);
    }
    (
        current_assignee.map(str::to_string).filter(|s| !s.is_empty()),
        AttributionSource::Current,
    )
}

#[cfg(test)]
mod tests {
    use super::*;
    use chrono::TimeZone;

    fn ts(y: i32, m: u32, d: u32, h: u32) -> DateTime<Utc> {
        Utc.with_ymd_and_hms(y, m, d, h, 0, 0).unwrap()
    }

    #[test]
    fn prefers_assignee_from_changelog_at_done() {
        let changes = vec![
            AssigneeChange {
                at: ts(2024, 1, 1, 10),
                to_account_id: Some("ada".into()),
            },
            AssigneeChange {
                at: ts(2024, 1, 2, 10),
                to_account_id: Some("bob".into()),
            },
            AssigneeChange {
                at: ts(2024, 1, 5, 10),
                to_account_id: Some("carol".into()),
            },
        ];
        let done = ts(2024, 1, 3, 12);
        let (id, src) = resolve_finisher(&changes, done, Some("carol"));
        assert_eq!(id.as_deref(), Some("bob"));
        assert_eq!(src, AttributionSource::Changelog);
    }

    #[test]
    fn falls_back_to_current_when_no_changelog() {
        let done = ts(2024, 1, 3, 12);
        let (id, src) = resolve_finisher(&[], done, Some("ada"));
        assert_eq!(id.as_deref(), Some("ada"));
        assert_eq!(src, AttributionSource::Current);
    }

    #[test]
    fn unassigned_current_yields_none() {
        let done = ts(2024, 1, 3, 12);
        let (id, src) = resolve_finisher(&[], done, None);
        assert_eq!(id, None);
        assert_eq!(src, AttributionSource::Current);
    }

    #[test]
    fn changelog_unassign_at_done_yields_none() {
        let changes = vec![AssigneeChange {
            at: ts(2024, 1, 2, 10),
            to_account_id: None,
        }];
        let done = ts(2024, 1, 3, 12);
        let (id, src) = resolve_finisher(&changes, done, Some("ada"));
        assert_eq!(id, None);
        assert_eq!(src, AttributionSource::Changelog);
    }
}
