//! Rebuild derived flow tables from raw SQLite data.

use std::collections::BTreeMap;

use ag_jira::{ChangelogHistory, ChangelogItem};
use chrono::{DateTime, Utc};
use rusqlite::{params, Connection};

use crate::changelog::{parse_jira_datetime, transitions_from_changelog};
use crate::error::AnalyticsError;
use crate::flow::{
    cycle_and_lead, resolve_status_category, time_in_status, StatusFlowCategory,
};

const META_STATUS_CATEGORY_PREFIX: &str = "status_flow_category:";

/// Recompute `derived_time_in_status` and `derived_issue_cycle` from raw issues/changelog.
pub fn rebuild_flow_derived(conn: &Connection, now: DateTime<Utc>) -> Result<(), AnalyticsError> {
    let overrides = load_status_category_overrides(conn)?;

    conn.execute("DELETE FROM derived_time_in_status", [])?;
    conn.execute("DELETE FROM derived_issue_cycle", [])?;

    let mut issue_stmt =
        conn.prepare("SELECT id, created, resolved, status FROM issues ORDER BY id")?;
    let issues = issue_stmt
        .query_map([], |row| {
            Ok((
                row.get::<_, String>(0)?,
                row.get::<_, String>(1)?,
                row.get::<_, Option<String>>(2)?,
                row.get::<_, Option<String>>(3)?,
            ))
        })?
        .collect::<Result<Vec<_>, _>>()?;

    for (issue_id, created_raw, resolved_raw, current_status) in issues {
        let Some(created) = parse_jira_datetime(&created_raw) else {
            continue;
        };

        let histories = load_status_histories(conn, &issue_id)?;
        let mut transitions = transitions_from_changelog(&issue_id, &histories);

        // If changelog never recorded an initial status but the issue has one,
        // synthesize a transition at created so open issues still accumulate time.
        if transitions.is_empty() {
            if let Some(status) = current_status.filter(|s| !s.is_empty()) {
                transitions.push(crate::changelog::StatusTransition {
                    issue_id: issue_id.clone(),
                    from_status: None,
                    to_status: status,
                    at: created,
                });
            }
        }

        let first_in_progress = transitions
            .iter()
            .find(|t| {
                resolve_status_category(&t.to_status, &overrides) == StatusFlowCategory::Active
            })
            .map(|t| t.at);

        let completed = resolved_raw
            .as_deref()
            .and_then(parse_jira_datetime)
            .or_else(|| {
                transitions
                    .iter()
                    .rev()
                    .find(|t| {
                        resolve_status_category(&t.to_status, &overrides)
                            == StatusFlowCategory::Terminal
                    })
                    .map(|t| t.at)
            });

        // Prefer last terminal transition when it is later than resolved (reopen then re-done).
        let completed = match (
            completed,
            transitions.iter().rev().find(|t| {
                resolve_status_category(&t.to_status, &overrides) == StatusFlowCategory::Terminal
            }),
        ) {
            (Some(resolved), Some(last_done)) if last_done.at > resolved => Some(last_done.at),
            (c, _) => c,
        };

        let tis = time_in_status(&transitions, completed, now);
        for row in tis {
            conn.execute(
                "INSERT INTO derived_time_in_status (issue_id, status, duration_secs)
                 VALUES (?1, ?2, ?3)
                 ON CONFLICT(issue_id, status) DO UPDATE SET
                    duration_secs = excluded.duration_secs",
                params![row.issue_id, row.status, row.duration_secs],
            )?;
        }

        let mut times = cycle_and_lead(created, first_in_progress, completed);
        times.issue_id = issue_id.clone();
        let completed_at = completed.map(|dt| dt.to_rfc3339());
        conn.execute(
            "INSERT INTO derived_issue_cycle (issue_id, cycle_secs, lead_secs, completed_at)
             VALUES (?1, ?2, ?3, ?4)
             ON CONFLICT(issue_id) DO UPDATE SET
                cycle_secs = excluded.cycle_secs,
                lead_secs = excluded.lead_secs,
                completed_at = excluded.completed_at",
            params![
                times.issue_id,
                times.cycle_secs,
                times.lead_secs,
                completed_at
            ],
        )?;
    }

    Ok(())
}

fn load_status_category_overrides(
    conn: &Connection,
) -> Result<BTreeMap<String, StatusFlowCategory>, AnalyticsError> {
    let mut stmt = conn.prepare("SELECT key, value FROM meta WHERE key LIKE ?1")?;
    let rows = stmt.query_map(
        params![format!("{META_STATUS_CATEGORY_PREFIX}%")],
        |row| Ok((row.get::<_, String>(0)?, row.get::<_, String>(1)?)),
    )?;

    let mut map = BTreeMap::new();
    for row in rows {
        let (key, value) = row?;
        let status = key
            .strip_prefix(META_STATUS_CATEGORY_PREFIX)
            .unwrap_or("")
            .to_string();
        if status.is_empty() {
            continue;
        }
        let cat = match value.to_ascii_lowercase().as_str() {
            "active" => StatusFlowCategory::Active,
            "waiting" => StatusFlowCategory::Waiting,
            "terminal" | "done" => StatusFlowCategory::Terminal,
            _ => continue,
        };
        map.insert(status, cat);
    }
    Ok(map)
}

fn load_status_histories(
    conn: &Connection,
    issue_id: &str,
) -> Result<Vec<ChangelogHistory>, AnalyticsError> {
    let mut stmt = conn.prepare(
        "SELECT changelog_id, created, field, from_value, to_value, from_string, to_string
         FROM issue_changelog
         WHERE issue_id = ?1
         ORDER BY created ASC, id ASC",
    )?;

    // Group flat rows back into histories keyed by (changelog_id, created).
    let mut histories: Vec<ChangelogHistory> = Vec::new();
    let mut current_key: Option<(Option<String>, String)> = None;

    let mut rows = stmt.query(params![issue_id])?;
    while let Some(row) = rows.next()? {
        let changelog_id: Option<String> = row.get(0)?;
        let created: String = row.get(1)?;
        let field: String = row.get(2)?;
        let from_value: Option<String> = row.get(3)?;
        let to_value: Option<String> = row.get(4)?;
        let from_string: Option<String> = row.get(5)?;
        let to_string: Option<String> = row.get(6)?;

        let key = (changelog_id.clone(), created.clone());
        if current_key.as_ref() != Some(&key) {
            histories.push(ChangelogHistory {
                id: changelog_id,
                created: Some(created),
                items: Vec::new(),
            });
            current_key = Some(key);
        }

        if let Some(hist) = histories.last_mut() {
            hist.items.push(ChangelogItem {
                field: Some(field),
                fieldtype: None,
                from: from_value,
                from_string,
                to: to_value,
                to_string,
            });
        }
    }

    Ok(histories)
}

#[cfg(test)]
mod tests {
    use super::*;
    use ag_db::{migrate, open_db};
    use chrono::TimeZone;
    use tempfile::tempdir;

    #[test]
    fn rebuild_flow_derived_writes_time_in_status_and_cycle() {
        let dir = tempdir().unwrap();
        let path = dir.path().join("test.db");
        let conn = open_db(&path).unwrap();
        migrate(&conn).unwrap();

        let created = "2024-01-01T08:00:00.000+0000";
        conn.execute(
            "INSERT INTO issues (
                id, key, project_key, summary, issue_type, status, status_category,
                created, updated, resolved
             ) VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8, ?9, ?10)",
            params![
                "1",
                "DEMO-1",
                "DEMO",
                "Sample",
                "Story",
                "Done",
                "done",
                created,
                "2024-01-02T11:00:00.000+0000",
                "2024-01-02T11:00:00.000+0000",
            ],
        )
        .unwrap();

        let rows = [
            ("c1", "2024-01-01T10:00:00.000+0000", "To Do", "In Progress"),
            ("c2", "2024-01-01T12:00:00.000+0000", "In Progress", "Done"),
            ("c3", "2024-01-02T09:00:00.000+0000", "Done", "In Progress"),
            ("c4", "2024-01-02T11:00:00.000+0000", "In Progress", "Done"),
        ];
        for (cid, at, from, to) in rows {
            conn.execute(
                "INSERT INTO issue_changelog (
                    issue_id, changelog_id, field, from_string, to_string, created
                 ) VALUES (?1, ?2, 'status', ?3, ?4, ?5)",
                params!["1", cid, from, to, at],
            )
            .unwrap();
        }

        let now = Utc.with_ymd_and_hms(2024, 1, 2, 11, 0, 0).unwrap();
        rebuild_flow_derived(&conn, now).unwrap();

        let in_progress: i64 = conn
            .query_row(
                "SELECT duration_secs FROM derived_time_in_status
                 WHERE issue_id = '1' AND status = 'In Progress'",
                [],
                |row| row.get(0),
            )
            .unwrap();
        assert_eq!(in_progress, 4 * 3600);

        let (cycle, lead): (Option<i64>, Option<i64>) = conn
            .query_row(
                "SELECT cycle_secs, lead_secs FROM derived_issue_cycle WHERE issue_id = '1'",
                [],
                |row| Ok((row.get(0)?, row.get(1)?)),
            )
            .unwrap();
        // first In Progress 10:00 day1 → final Done 11:00 day2 = 25h
        assert_eq!(cycle, Some(25 * 3600));
        // created 08:00 day1 → Done 11:00 day2 = 27h
        assert_eq!(lead, Some(27 * 3600));
    }
}
