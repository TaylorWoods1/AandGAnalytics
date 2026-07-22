//! Rebuild derived analytics tables from raw SQLite data.

use std::collections::{BTreeMap, BTreeSet, HashMap, HashSet};

use ag_jira::{ChangelogHistory, ChangelogItem};
use chrono::{DateTime, Utc};
use rusqlite::{params, Connection};

use crate::changelog::{parse_jira_datetime, transitions_from_changelog, StatusTransition};
use crate::error::AnalyticsError;
use crate::events::{detect_handoffs, detect_reopens, detect_scope_changes, FieldChange};
use crate::flow::{
    cycle_and_lead, resolve_status_category, time_in_status, StatusFlowCategory,
};
use crate::sprint::compute_sprint_metrics;
use crate::throughput::daily_throughput;

const META_STATUS_CATEGORY_PREFIX: &str = "status_flow_category:";
const META_EVENTS_REOPENS: &str = "derived_events:reopens";
const META_EVENTS_HANDOFFS: &str = "derived_events:handoffs";
const META_EVENTS_SCOPE_ADDED: &str = "derived_events:scope_added";
const META_EVENTS_SCOPE_REMOVED: &str = "derived_events:scope_removed";

/// Rebuild all derived analytics tables (flow, throughput, sprint, events, epic risk).
pub fn rebuild_all_derived(conn: &Connection, now: DateTime<Utc>) -> Result<(), AnalyticsError> {
    rebuild_flow_derived(conn, now)?;
    rebuild_throughput_derived(conn)?;
    rebuild_sprint_derived(conn)?;
    rebuild_event_derived(conn)?;
    ag_risk::rebuild_epic_risk(conn, now).map_err(|e| AnalyticsError::Other(e.to_string()))?;
    Ok(())
}

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

/// Recompute `derived_throughput_daily` from `derived_issue_cycle` + issue projects.
pub fn rebuild_throughput_derived(conn: &Connection) -> Result<(), AnalyticsError> {
    conn.execute("DELETE FROM derived_throughput_daily", [])?;

    let mut stmt = conn.prepare(
        "SELECT i.project_key, i.id, c.completed_at
         FROM derived_issue_cycle c
         JOIN issues i ON i.id = c.issue_id
         WHERE c.completed_at IS NOT NULL
         ORDER BY i.project_key, c.completed_at",
    )?;

    let rows = stmt
        .query_map([], |row| {
            Ok((
                row.get::<_, String>(0)?,
                row.get::<_, String>(1)?,
                row.get::<_, String>(2)?,
            ))
        })?
        .collect::<Result<Vec<_>, _>>()?;

    let mut by_project: BTreeMap<String, Vec<(String, DateTime<Utc>)>> = BTreeMap::new();
    for (project_key, issue_id, completed_raw) in rows {
        let Some(at) = parse_jira_datetime(&completed_raw)
            .or_else(|| DateTime::parse_from_rfc3339(&completed_raw).ok().map(|d| d.with_timezone(&Utc)))
        else {
            continue;
        };
        by_project
            .entry(project_key)
            .or_default()
            .push((issue_id, at));
    }

    for (project_key, completions) in by_project {
        let daily = daily_throughput(&completions);
        for (day, count) in daily {
            conn.execute(
                "INSERT INTO derived_throughput_daily (day, project_key, completed_count)
                 VALUES (?1, ?2, ?3)
                 ON CONFLICT(day, project_key) DO UPDATE SET
                    completed_count = excluded.completed_count",
                params![day.to_string(), project_key, count as i64],
            )?;
        }
    }

    Ok(())
}

/// Recompute `derived_sprint_metrics` from sprints, sprint_issues, issues, and changelog.
pub fn rebuild_sprint_derived(conn: &Connection) -> Result<(), AnalyticsError> {
    conn.execute("DELETE FROM derived_sprint_metrics", [])?;

    let mut sprint_stmt = conn.prepare(
        "SELECT id, name, start_date, end_date, complete_date FROM sprints ORDER BY id",
    )?;
    let sprints = sprint_stmt
        .query_map([], |row| {
            Ok((
                row.get::<_, String>(0)?,
                row.get::<_, Option<String>>(1)?,
                row.get::<_, Option<String>>(2)?,
                row.get::<_, Option<String>>(3)?,
                row.get::<_, Option<String>>(4)?,
            ))
        })?
        .collect::<Result<Vec<_>, _>>()?;

    for (sprint_id, sprint_name, start_raw, end_raw, complete_raw) in sprints {
        let start = start_raw.as_deref().and_then(parse_jira_datetime);
        let end = complete_raw
            .as_deref()
            .and_then(parse_jira_datetime)
            .or_else(|| end_raw.as_deref().and_then(parse_jira_datetime));

        let members = load_sprint_members(conn, &sprint_id)?;
        let (added_mid, removed_mid) =
            load_sprint_scope_moves(conn, &sprint_id, sprint_name.as_deref(), start, end)?;

        let current_keys: HashSet<String> = members.iter().map(|m| m.key.clone()).collect();
        let mut committed_keys: BTreeSet<String> = current_keys
            .difference(&added_mid)
            .cloned()
            .collect();
        for key in &removed_mid {
            committed_keys.insert(key.clone());
        }

        let member_by_key: HashMap<String, &SprintMember> =
            members.iter().map(|m| (m.key.clone(), m)).collect();

        let mut completed_keys: Vec<String> = Vec::new();
        let mut velocity = 0.0;
        let mut has_points = false;

        // Completed = Done among committed ∪ mid-sprint adds still linked (or resolved in window).
        let mut candidates: BTreeSet<String> = committed_keys.clone();
        for key in &added_mid {
            candidates.insert(key.clone());
        }

        for key in &candidates {
            let Some(member) = member_by_key.get(key) else {
                continue;
            };
            if issue_is_done(member) {
                completed_keys.push(key.clone());
                if let Some(pts) = member.story_points {
                    velocity += pts;
                    has_points = true;
                }
            }
        }

        let committed_refs: Vec<&str> = committed_keys.iter().map(String::as_str).collect();
        let completed_refs: Vec<&str> = completed_keys.iter().map(String::as_str).collect();
        let added_refs: Vec<&str> = added_mid.iter().map(String::as_str).collect();
        let removed_refs: Vec<&str> = removed_mid.iter().map(String::as_str).collect();

        let metrics = compute_sprint_metrics(
            &sprint_id,
            &committed_refs,
            &completed_refs,
            &added_refs,
            &removed_refs,
            if has_points { Some(velocity) } else { None },
        );

        conn.execute(
            "INSERT INTO derived_sprint_metrics (
                sprint_id, committed, completed, spillover, scope_added, scope_removed, velocity_points
             ) VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7)",
            params![
                metrics.sprint_id,
                metrics.committed as i64,
                metrics.completed as i64,
                metrics.spillover as i64,
                metrics.scope_added as i64,
                metrics.scope_removed as i64,
                metrics.velocity_points,
            ],
        )?;
    }

    Ok(())
}

/// Aggregate reopen / handoff / scope-change counts into meta keys.
pub fn rebuild_event_derived(conn: &Connection) -> Result<(), AnalyticsError> {
    let mut transitions: Vec<StatusTransition> = Vec::new();
    let mut assignee_changes: Vec<(DateTime<Utc>, Option<String>, Option<String>)> = Vec::new();
    let mut field_changes: Vec<FieldChange> = Vec::new();

    let mut stmt = conn.prepare(
        "SELECT issue_id, field, from_value, to_value, from_string, to_string, created
         FROM issue_changelog
         ORDER BY issue_id ASC, created ASC, id ASC",
    )?;
    let mut rows = stmt.query([])?;
    while let Some(row) = rows.next()? {
        let issue_id: String = row.get(0)?;
        let field: String = row.get(1)?;
        let from_value: Option<String> = row.get(2)?;
        let to_value: Option<String> = row.get(3)?;
        let from_string: Option<String> = row.get(4)?;
        let to_string: Option<String> = row.get(5)?;
        let created_raw: String = row.get(6)?;
        let Some(at) = parse_jira_datetime(&created_raw) else {
            continue;
        };

        let field_lc = field.to_ascii_lowercase();
        if field_lc == "status" {
            if let Some(to_status) = to_string.clone().filter(|s| !s.is_empty()) {
                transitions.push(StatusTransition {
                    issue_id: issue_id.clone(),
                    from_status: from_string.clone(),
                    to_status,
                    at,
                });
            }
        } else if field_lc == "assignee" {
            let from = from_value.or_else(|| from_string.clone());
            let to = to_value.or_else(|| to_string.clone());
            assignee_changes.push((at, from, to));
        } else if field_lc.contains("story point") || field_lc == "sprint" {
            field_changes.push(FieldChange {
                issue_id,
                field,
                from_string,
                to_string,
                at,
            });
        }
    }

    let reopens = detect_reopens(&transitions);
    let handoffs = detect_handoffs(&assignee_changes);
    let scope = detect_scope_changes(&field_changes);

    upsert_meta(conn, META_EVENTS_REOPENS, &reopens.to_string())?;
    upsert_meta(conn, META_EVENTS_HANDOFFS, &handoffs.to_string())?;
    upsert_meta(conn, META_EVENTS_SCOPE_ADDED, &scope.scope_added.to_string())?;
    upsert_meta(
        conn,
        META_EVENTS_SCOPE_REMOVED,
        &scope.scope_removed.to_string(),
    )?;

    Ok(())
}

struct SprintMember {
    key: String,
    status: Option<String>,
    status_category: Option<String>,
    story_points: Option<f64>,
    resolved: Option<String>,
}

fn load_sprint_members(
    conn: &Connection,
    sprint_id: &str,
) -> Result<Vec<SprintMember>, AnalyticsError> {
    let mut stmt = conn.prepare(
        "SELECT i.key, i.status, i.status_category, i.story_points, i.resolved
         FROM sprint_issues si
         JOIN issues i ON i.id = si.issue_id
         WHERE si.sprint_id = ?1
         ORDER BY i.key",
    )?;
    let rows = stmt
        .query_map(params![sprint_id], |row| {
            Ok(SprintMember {
                key: row.get(0)?,
                status: row.get(1)?,
                status_category: row.get(2)?,
                story_points: row.get(3)?,
                resolved: row.get(4)?,
            })
        })?
        .collect::<Result<Vec<_>, _>>()?;
    Ok(rows)
}

fn load_sprint_scope_moves(
    conn: &Connection,
    sprint_id: &str,
    sprint_name: Option<&str>,
    start: Option<DateTime<Utc>>,
    end: Option<DateTime<Utc>>,
) -> Result<(HashSet<String>, HashSet<String>), AnalyticsError> {
    let mut added = HashSet::new();
    let mut removed = HashSet::new();

    let mut stmt = conn.prepare(
        "SELECT i.key, c.from_string, c.to_string, c.created
         FROM issue_changelog c
         JOIN issues i ON i.id = c.issue_id
         WHERE lower(c.field) = 'sprint'
         ORDER BY c.created ASC",
    )?;
    let mut rows = stmt.query([])?;
    while let Some(row) = rows.next()? {
        let key: String = row.get(0)?;
        let from_string: Option<String> = row.get(1)?;
        let to_string: Option<String> = row.get(2)?;
        let created_raw: String = row.get(3)?;
        let Some(at) = parse_jira_datetime(&created_raw) else {
            continue;
        };
        if let Some(start) = start {
            if at < start {
                continue;
            }
        }
        if let Some(end) = end {
            if at > end {
                continue;
            }
        }

        let mentions_from = sprint_field_mentions(from_string.as_deref(), sprint_id, sprint_name);
        let mentions_to = sprint_field_mentions(to_string.as_deref(), sprint_id, sprint_name);
        if !mentions_from && mentions_to {
            added.insert(key);
        } else if mentions_from && !mentions_to {
            removed.insert(key);
        }
    }

    Ok((added, removed))
}

fn sprint_field_mentions(raw: Option<&str>, sprint_id: &str, sprint_name: Option<&str>) -> bool {
    let Some(text) = raw.map(str::trim).filter(|s| !s.is_empty()) else {
        return false;
    };
    if text == sprint_id || text.contains(sprint_id) {
        return true;
    }
    if let Some(name) = sprint_name {
        if !name.is_empty() && text.contains(name) {
            return true;
        }
    }
    false
}

fn issue_is_done(member: &SprintMember) -> bool {
    if member.resolved.as_deref().map(str::trim).is_some_and(|s| !s.is_empty()) {
        return true;
    }
    if member
        .status_category
        .as_deref()
        .map(|s| s.eq_ignore_ascii_case("done"))
        .unwrap_or(false)
    {
        return true;
    }
    member
        .status
        .as_deref()
        .map(|s| resolve_status_category(s, &BTreeMap::new()) == StatusFlowCategory::Terminal)
        .unwrap_or(false)
}

fn upsert_meta(conn: &Connection, key: &str, value: &str) -> Result<(), AnalyticsError> {
    conn.execute(
        "INSERT INTO meta (key, value) VALUES (?1, ?2)
         ON CONFLICT(key) DO UPDATE SET value = excluded.value",
        params![key, value],
    )?;
    Ok(())
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

    #[test]
    fn rebuild_sprint_derived_writes_commitment_and_spillover() {
        let dir = tempdir().unwrap();
        let path = dir.path().join("sprint.db");
        let conn = open_db(&path).unwrap();
        migrate(&conn).unwrap();

        conn.execute(
            "INSERT INTO sprints (id, name, state, start_date, end_date, complete_date)
             VALUES ('10', 'Sprint 1', 'closed',
                     '2024-01-01T00:00:00.000+0000',
                     '2024-01-14T00:00:00.000+0000',
                     '2024-01-14T00:00:00.000+0000')",
            [],
        )
        .unwrap();

        let issues = [
            ("1", "A-1", "Done", "done", Some(3.0), Some("2024-01-10T00:00:00.000+0000")),
            ("2", "A-2", "Done", "done", Some(2.0), Some("2024-01-11T00:00:00.000+0000")),
            ("3", "A-3", "To Do", "new", None, None),
            ("4", "A-4", "Done", "done", Some(1.0), Some("2024-01-12T00:00:00.000+0000")),
        ];
        for (id, key, status, cat, pts, resolved) in issues {
            conn.execute(
                "INSERT INTO issues (
                    id, key, project_key, summary, issue_type, status, status_category,
                    story_points, created, updated, resolved
                 ) VALUES (?1, ?2, 'DEMO', 'x', 'Story', ?3, ?4, ?5,
                           '2023-12-01T00:00:00.000+0000',
                           '2024-01-12T00:00:00.000+0000', ?6)",
                params![id, key, status, cat, pts, resolved],
            )
            .unwrap();
        }

        // Current membership: A-1, A-2, A-4 (A-3 removed mid-sprint).
        for id in ["1", "2", "4"] {
            conn.execute(
                "INSERT INTO sprint_issues (sprint_id, issue_id) VALUES ('10', ?1)",
                params![id],
            )
            .unwrap();
        }

        // A-4 added mid; A-3 removed mid.
        conn.execute(
            "INSERT INTO issue_changelog (
                issue_id, changelog_id, field, from_string, to_string, created
             ) VALUES ('4', 'c1', 'Sprint', NULL, 'Sprint 1', '2024-01-05T00:00:00.000+0000')",
            [],
        )
        .unwrap();
        conn.execute(
            "INSERT INTO issue_changelog (
                issue_id, changelog_id, field, from_string, to_string, created
             ) VALUES ('3', 'c2', 'Sprint', 'Sprint 1', NULL, '2024-01-06T00:00:00.000+0000')",
            [],
        )
        .unwrap();

        rebuild_sprint_derived(&conn).unwrap();

        let (committed, completed, spillover, scope_added, scope_removed, velocity): (
            i64,
            i64,
            i64,
            i64,
            i64,
            Option<f64>,
        ) = conn
            .query_row(
                "SELECT committed, completed, spillover, scope_added, scope_removed, velocity_points
                 FROM derived_sprint_metrics WHERE sprint_id = '10'",
                [],
                |row| {
                    Ok((
                        row.get(0)?,
                        row.get(1)?,
                        row.get(2)?,
                        row.get(3)?,
                        row.get(4)?,
                        row.get(5)?,
                    ))
                },
            )
            .unwrap();

        assert_eq!(committed, 3); // A-1, A-2, A-3
        assert_eq!(completed, 3); // A-1, A-2, A-4 (mid-sprint add that finished)
        assert_eq!(spillover, 1); // A-3 committed, not Done at end
        assert_eq!(scope_added, 1);
        assert_eq!(scope_removed, 1);
        assert_eq!(velocity, Some(6.0)); // 3 + 2 + 1
    }

    #[test]
    fn rebuild_all_derived_writes_throughput_and_event_meta() {
        let dir = tempdir().unwrap();
        let path = dir.path().join("all.db");
        let conn = open_db(&path).unwrap();
        migrate(&conn).unwrap();

        conn.execute(
            "INSERT INTO issues (
                id, key, project_key, summary, issue_type, status, status_category,
                created, updated, resolved
             ) VALUES ('1', 'DEMO-1', 'DEMO', 'Sample', 'Story', 'Done', 'done',
                       '2024-01-01T08:00:00.000+0000',
                       '2024-01-02T11:00:00.000+0000',
                       '2024-01-02T11:00:00.000+0000')",
            [],
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
        conn.execute(
            "INSERT INTO issue_changelog (
                issue_id, changelog_id, field, from_value, to_value, created
             ) VALUES ('1', 'c5', 'assignee', 'ada', 'bob', '2024-01-01T11:00:00.000+0000')",
            [],
        )
        .unwrap();

        let now = Utc.with_ymd_and_hms(2024, 1, 2, 11, 0, 0).unwrap();
        rebuild_all_derived(&conn, now).unwrap();

        let count: i64 = conn
            .query_row(
                "SELECT completed_count FROM derived_throughput_daily
                 WHERE day = '2024-01-02' AND project_key = 'DEMO'",
                [],
                |row| row.get(0),
            )
            .unwrap();
        assert_eq!(count, 1);

        let reopens: String = conn
            .query_row(
                "SELECT value FROM meta WHERE key = 'derived_events:reopens'",
                [],
                |row| row.get(0),
            )
            .unwrap();
        assert_eq!(reopens, "1");

        let handoffs: String = conn
            .query_row(
                "SELECT value FROM meta WHERE key = 'derived_events:handoffs'",
                [],
                |row| row.get(0),
            )
            .unwrap();
        assert_eq!(handoffs, "1");
    }
}
