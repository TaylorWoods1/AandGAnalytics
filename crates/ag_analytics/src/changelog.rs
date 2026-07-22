//! Changelog → status transitions.

use ag_jira::ChangelogHistory;
use chrono::{DateTime, Utc};

/// A single status transition for an issue.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct StatusTransition {
    pub issue_id: String,
    pub from_status: Option<String>,
    pub to_status: String,
    pub at: DateTime<Utc>,
}

/// Extract ordered status transitions from Jira changelog histories.
pub fn transitions_from_changelog(
    issue_id: &str,
    histories: &[ChangelogHistory],
) -> Vec<StatusTransition> {
    let mut out = Vec::new();
    for hist in histories {
        let Some(created) = hist.created.as_deref() else {
            continue;
        };
        let Some(at) = parse_jira_datetime(created) else {
            continue;
        };
        for item in &hist.items {
            if item.field.as_deref() != Some("status") {
                continue;
            }
            let Some(to_status) = item.to_string.clone().filter(|s| !s.is_empty()) else {
                continue;
            };
            out.push(StatusTransition {
                issue_id: issue_id.to_string(),
                from_status: item.from_string.clone(),
                to_status,
                at,
            });
        }
    }
    out.sort_by(|a, b| a.at.cmp(&b.at));
    out
}

/// Parse common Jira Cloud timestamp forms into UTC.
pub(crate) fn parse_jira_datetime(raw: &str) -> Option<DateTime<Utc>> {
    if let Ok(dt) = DateTime::parse_from_rfc3339(raw) {
        return Some(dt.with_timezone(&Utc));
    }
    // e.g. 2024-01-01T09:00:00.000+0000
    if let Ok(dt) = DateTime::parse_from_str(raw, "%Y-%m-%dT%H:%M:%S%.3f%z") {
        return Some(dt.with_timezone(&Utc));
    }
    if let Ok(dt) = DateTime::parse_from_str(raw, "%Y-%m-%dT%H:%M:%S%z") {
        return Some(dt.with_timezone(&Utc));
    }
    None
}

#[cfg(test)]
mod tests {
    use super::*;
    use ag_jira::ChangelogItem;
    use chrono::TimeZone;

    #[test]
    fn transitions_from_changelog_extracts_status_only() {
        let t1 = Utc.with_ymd_and_hms(2024, 1, 1, 10, 0, 0).unwrap();
        let histories = vec![
            ChangelogHistory {
                id: Some("1".into()),
                created: Some("2024-01-01T10:00:00.000+0000".into()),
                items: vec![
                    ChangelogItem {
                        field: Some("status".into()),
                        fieldtype: Some("jira".into()),
                        from: Some("1".into()),
                        from_string: Some("To Do".into()),
                        to: Some("2".into()),
                        to_string: Some("In Progress".into()),
                    },
                    ChangelogItem {
                        field: Some("assignee".into()),
                        fieldtype: Some("jira".into()),
                        from: None,
                        from_string: None,
                        to: Some("abc".into()),
                        to_string: Some("Ada".into()),
                    },
                ],
            },
            ChangelogHistory {
                id: Some("2".into()),
                created: Some("2024-01-01T12:00:00.000+0000".into()),
                items: vec![ChangelogItem {
                    field: Some("status".into()),
                    fieldtype: Some("jira".into()),
                    from: Some("2".into()),
                    from_string: Some("In Progress".into()),
                    to: Some("3".into()),
                    to_string: Some("Done".into()),
                }],
            },
        ];

        let got = transitions_from_changelog("10001", &histories);
        assert_eq!(got.len(), 2);
        assert_eq!(got[0].to_status, "In Progress");
        assert_eq!(got[0].from_status.as_deref(), Some("To Do"));
        assert_eq!(got[0].at, t1);
        assert_eq!(got[1].to_status, "Done");
    }
}
