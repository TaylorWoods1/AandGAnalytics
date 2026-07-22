//! Rebuild `derived_epic_risk` from raw + derived analytics tables.

use chrono::{DateTime, Duration, Utc};
use rusqlite::{params, Connection};

use crate::error::RiskError;
use crate::forecast::{finish_by_probability, FinishByInput};
use crate::score::{score_epic, EpicRiskInput};

/// Default finish-by horizon used when persisting a baseline probability.
const DEFAULT_FINISH_BY_WEEKS: f64 = 4.0;
/// Lookback window for weekly throughput (days).
const THROUGHPUT_LOOKBACK_DAYS: i64 = 56;

/// Recompute `derived_epic_risk` for every epic present on issues.
pub fn rebuild_epic_risk(conn: &Connection, now: DateTime<Utc>) -> Result<(), RiskError> {
    conn.execute("DELETE FROM derived_epic_risk", [])?;

    let epic_keys = load_epic_keys(conn)?;
    let global_spillover = load_recent_spillover_rate(conn)?;

    for epic_key in epic_keys {
        let input = build_epic_input(conn, &epic_key, now, global_spillover)?;
        let risk = score_epic(&input);

        let stddev = (input.avg_weekly_throughput_issues * 0.25).max(0.5);
        let finish = finish_by_probability(&FinishByInput {
            remaining_work_issues: input.remaining_issues as f64,
            weekly_throughput_issues: input.avg_weekly_throughput_issues,
            weeks_until_target: DEFAULT_FINISH_BY_WEEKS,
            throughput_stddev: stddev,
        });

        let assumptions_json = serde_json::to_string(&finish.assumptions)
            .map_err(|e| RiskError::Other(e.to_string()))?;

        conn.execute(
            "INSERT INTO derived_epic_risk (epic_key, risk_score, finish_by_probability, assumptions_json)
             VALUES (?1, ?2, ?3, ?4)",
            params![
                risk.epic_key,
                risk.score,
                finish.probability,
                assumptions_json,
            ],
        )?;
    }

    Ok(())
}

fn load_epic_keys(conn: &Connection) -> Result<Vec<String>, RiskError> {
    let mut stmt = conn.prepare(
        "SELECT DISTINCT epic_key FROM issues
         WHERE epic_key IS NOT NULL AND TRIM(epic_key) != ''
         ORDER BY epic_key",
    )?;
    let keys = stmt
        .query_map([], |row| row.get::<_, String>(0))?
        .collect::<Result<Vec<_>, _>>()?;
    Ok(keys)
}

fn build_epic_input(
    conn: &Connection,
    epic_key: &str,
    now: DateTime<Utc>,
    recent_spillover_rate: f64,
) -> Result<EpicRiskInput, RiskError> {
    let mut open_stmt = conn.prepare(
        "SELECT key, story_points, created, status_category, status, resolved
         FROM issues
         WHERE epic_key = ?1",
    )?;
    let rows = open_stmt
        .query_map(params![epic_key], |row| {
            Ok((
                row.get::<_, String>(0)?,
                row.get::<_, Option<f64>>(1)?,
                row.get::<_, String>(2)?,
                row.get::<_, Option<String>>(3)?,
                row.get::<_, Option<String>>(4)?,
                row.get::<_, Option<String>>(5)?,
            ))
        })?
        .collect::<Result<Vec<_>, _>>()?;

    let mut remaining_issues = 0u32;
    let mut remaining_points_sum = 0.0;
    let mut has_points = false;
    let mut open_ages: Vec<i64> = Vec::new();

    for (_key, points, created_raw, status_category, status, resolved) in &rows {
        if issue_is_open(
            status_category.as_deref(),
            status.as_deref(),
            resolved.as_deref(),
        ) {
            remaining_issues += 1;
            if let Some(p) = points {
                remaining_points_sum += p;
                has_points = true;
            }
            if let Some(created) = parse_dt(created_raw) {
                open_ages.push((now - created).num_seconds().max(0));
            }
        }
    }

    open_ages.sort_unstable();
    let open_issue_age_secs_p50 = percentile_nearest_rank(&open_ages, 0.50);

    let lookback_start = now - Duration::days(THROUGHPUT_LOOKBACK_DAYS);
    let completed = count_completed_in_window(conn, epic_key, lookback_start, now)?;
    let weeks = (THROUGHPUT_LOOKBACK_DAYS as f64) / 7.0;
    let avg_weekly_throughput_issues = completed as f64 / weeks;

    let blocked_secs_total = load_blocked_secs(conn, epic_key)?;
    let recent_scope_growth = load_recent_scope_growth(conn, epic_key, lookback_start)?;

    Ok(EpicRiskInput {
        epic_key: epic_key.to_string(),
        remaining_issues,
        remaining_points: if has_points {
            Some(remaining_points_sum)
        } else {
            None
        },
        avg_weekly_throughput_issues,
        avg_weekly_throughput_points: None,
        blocked_secs_total,
        open_issue_age_secs_p50,
        recent_scope_growth,
        recent_spillover_rate,
    })
}

fn issue_is_open(
    status_category: Option<&str>,
    status: Option<&str>,
    resolved: Option<&str>,
) -> bool {
    if resolved.map(|r| !r.is_empty()).unwrap_or(false) {
        return false;
    }
    if let Some(cat) = status_category {
        let lower = cat.to_ascii_lowercase();
        if lower.contains("done") || lower == "complete" || lower == "completed" {
            return false;
        }
    }
    if let Some(st) = status {
        let lower = st.to_ascii_lowercase();
        if lower == "done" || lower == "closed" || lower == "resolved" || lower == "complete" {
            return false;
        }
    }
    true
}

fn count_completed_in_window(
    conn: &Connection,
    epic_key: &str,
    start: DateTime<Utc>,
    end: DateTime<Utc>,
) -> Result<u32, RiskError> {
    let mut stmt = conn.prepare(
        "SELECT i.id, COALESCE(c.completed_at, i.resolved)
         FROM issues i
         LEFT JOIN derived_issue_cycle c ON c.issue_id = i.id
         WHERE i.epic_key = ?1
           AND COALESCE(c.completed_at, i.resolved) IS NOT NULL",
    )?;
    let rows = stmt
        .query_map(params![epic_key], |row| {
            Ok((row.get::<_, String>(0)?, row.get::<_, String>(1)?))
        })?
        .collect::<Result<Vec<_>, _>>()?;

    let mut count = 0u32;
    for (_id, completed_raw) in rows {
        let Some(at) = parse_dt(&completed_raw) else {
            continue;
        };
        if at >= start && at <= end {
            count += 1;
        }
    }
    Ok(count)
}

fn load_blocked_secs(conn: &Connection, epic_key: &str) -> Result<i64, RiskError> {
    let mut stmt = conn.prepare(
        "SELECT COALESCE(SUM(t.duration_secs), 0)
         FROM derived_time_in_status t
         JOIN issues i ON i.id = t.issue_id
         WHERE i.epic_key = ?1
           AND (
             LOWER(t.status) LIKE '%block%'
             OR LOWER(t.status) LIKE '%imped%'
           )",
    )?;
    let total: i64 = stmt.query_row(params![epic_key], |row| row.get(0))?;
    Ok(total)
}

fn load_recent_scope_growth(
    conn: &Connection,
    epic_key: &str,
    since: DateTime<Utc>,
) -> Result<f64, RiskError> {
    // Net story-point adds on epic children + issues joining the epic, over lookback,
    // relative to current open issue count (soft fraction).
    let mut stmt = conn.prepare(
        "SELECT c.field, c.from_string, c.to_string, c.created
         FROM issue_changelog c
         JOIN issues i ON i.id = c.issue_id
         WHERE i.epic_key = ?1
           AND (
             LOWER(c.field) IN ('story points', 'storypoint', 'story_points')
             OR LOWER(c.field) LIKE '%epic%'
             OR LOWER(c.field) = 'parent'
           )",
    )?;
    let rows = stmt
        .query_map(params![epic_key], |row| {
            Ok((
                row.get::<_, String>(0)?,
                row.get::<_, Option<String>>(1)?,
                row.get::<_, Option<String>>(2)?,
                row.get::<_, String>(3)?,
            ))
        })?
        .collect::<Result<Vec<_>, _>>()?;

    let mut added = 0.0f64;
    let mut removed = 0.0f64;
    for (field, from_s, to_s, created_raw) in rows {
        let Some(at) = parse_dt(&created_raw) else {
            continue;
        };
        if at < since {
            continue;
        }
        let field_l = field.to_ascii_lowercase();
        if field_l.contains("story") || field_l.contains("point") {
            let from_v = parse_f64(from_s.as_deref()).unwrap_or(0.0);
            let to_v = parse_f64(to_s.as_deref()).unwrap_or(0.0);
            let delta = to_v - from_v;
            if delta > 0.0 {
                added += delta;
            } else {
                removed += -delta;
            }
        } else if to_s.as_deref() == Some(epic_key) {
            added += 1.0;
        } else if from_s.as_deref() == Some(epic_key) {
            removed += 1.0;
        }
    }

    let net = (added - removed).max(0.0);
    let denom = (added + removed).max(1.0);
    Ok((net / denom).clamp(0.0, 1.0))
}

fn load_recent_spillover_rate(conn: &Connection) -> Result<f64, RiskError> {
    let mut stmt = conn.prepare(
        "SELECT committed, spillover FROM derived_sprint_metrics
         WHERE committed IS NOT NULL AND committed > 0",
    )?;
    let rows = stmt
        .query_map([], |row| Ok((row.get::<_, i64>(0)?, row.get::<_, i64>(1)?)))?
        .collect::<Result<Vec<_>, _>>()?;

    if rows.is_empty() {
        return Ok(0.0);
    }
    let mut committed_total = 0i64;
    let mut spillover_total = 0i64;
    for (c, s) in rows {
        committed_total += c;
        spillover_total += s;
    }
    if committed_total <= 0 {
        return Ok(0.0);
    }
    Ok((spillover_total as f64 / committed_total as f64).clamp(0.0, 1.0))
}

fn percentile_nearest_rank(sorted: &[i64], p: f64) -> i64 {
    if sorted.is_empty() {
        return 0;
    }
    let n = sorted.len();
    let rank = ((p * n as f64).ceil() as usize).clamp(1, n);
    sorted[rank - 1]
}

fn parse_dt(raw: &str) -> Option<DateTime<Utc>> {
    if let Ok(d) = DateTime::parse_from_rfc3339(raw) {
        return Some(d.with_timezone(&Utc));
    }
    // Jira-style: 2024-01-15T10:00:00.000+0000
    if raw.len() >= 5 {
        let (head, tail) = raw.split_at(raw.len() - 5);
        if (tail.starts_with('+') || tail.starts_with('-')) && !tail.contains(':') {
            let normalized = format!("{head}{tail}:00");
            if let Ok(d) = DateTime::parse_from_rfc3339(&normalized) {
                return Some(d.with_timezone(&Utc));
            }
        }
    }
    chrono::NaiveDateTime::parse_from_str(raw, "%Y-%m-%dT%H:%M:%S%.f")
        .ok()
        .or_else(|| chrono::NaiveDateTime::parse_from_str(raw, "%Y-%m-%d %H:%M:%S").ok())
        .map(|n| n.and_utc())
}

fn parse_f64(raw: Option<&str>) -> Option<f64> {
    raw.and_then(|s| s.trim().parse::<f64>().ok())
}

#[cfg(test)]
mod tests {
    use super::*;
    use ag_db::{migrate, open_db};
    use chrono::TimeZone;
    use tempfile::tempdir;

    #[test]
    fn rebuild_epic_risk_persists_score_and_probability() {
        let dir = tempdir().unwrap();
        let path = dir.path().join("risk.db");
        let conn = open_db(&path).unwrap();
        migrate(&conn).unwrap();

        conn.execute(
            "INSERT INTO issues (
                id, key, project_key, summary, issue_type, status, status_category,
                epic_key, created, updated, resolved
             ) VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8, ?9, ?10, ?11)",
            params![
                "1",
                "DEMO-1",
                "DEMO",
                "Open child",
                "Story",
                "In Progress",
                "indeterminate",
                "EPIC-1",
                "2024-01-01T08:00:00.000+0000",
                "2024-01-10T08:00:00.000+0000",
                Option::<String>::None,
            ],
        )
        .unwrap();
        conn.execute(
            "INSERT INTO issues (
                id, key, project_key, summary, issue_type, status, status_category,
                epic_key, created, updated, resolved
             ) VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8, ?9, ?10, ?11)",
            params![
                "2",
                "DEMO-2",
                "DEMO",
                "Done child",
                "Story",
                "Done",
                "done",
                "EPIC-1",
                "2024-01-01T08:00:00.000+0000",
                "2024-01-08T08:00:00.000+0000",
                "2024-01-08T08:00:00.000+0000",
            ],
        )
        .unwrap();

        let now = Utc.with_ymd_and_hms(2024, 1, 15, 12, 0, 0).unwrap();
        rebuild_epic_risk(&conn, now).unwrap();

        let (score, prob, assumptions): (f64, f64, String) = conn
            .query_row(
                "SELECT risk_score, finish_by_probability, assumptions_json
                 FROM derived_epic_risk WHERE epic_key = 'EPIC-1'",
                [],
                |row| Ok((row.get(0)?, row.get(1)?, row.get(2)?)),
            )
            .unwrap();

        assert!((0.0..=100.0).contains(&score));
        assert!((0.0..=1.0).contains(&prob));
        assert!(assumptions.contains("throughput") || assumptions.contains("Weekly"));
    }
}
