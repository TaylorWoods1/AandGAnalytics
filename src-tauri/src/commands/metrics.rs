//! Metrics / explore query commands and filter DTOs.

use std::collections::BTreeMap;
use std::path::Path;

use ag_analytics::{resolve_status_category, StatusFlowCategory};
use ag_db::{migrate, open_db};
use ag_risk::{finish_by_probability, FinishByInput};
use chrono::{DateTime, NaiveDate, Utc};
use rusqlite::{params_from_iter, Connection, ToSql};
use serde::{Deserialize, Serialize};

use crate::state::AppState;

/// Shared filter for analytics / explore queries.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct MetricsFilter {
    pub project_keys: Option<Vec<String>>,
    /// Inclusive lower bound as ISO date (`YYYY-MM-DD`).
    pub from: Option<String>,
    /// Inclusive upper bound as ISO date (`YYYY-MM-DD`).
    pub to: Option<String>,
    pub issue_types: Option<Vec<String>>,
    pub assignee_ids: Option<Vec<String>>,
}

impl MetricsFilter {
    /// Validate filter fields. Rejects inverted date ranges and malformed dates.
    pub fn validate(&self) -> Result<(), String> {
        let from = match &self.from {
            Some(s) => Some(parse_iso_date(s)?),
            None => None,
        };
        let to = match &self.to {
            Some(s) => Some(parse_iso_date(s)?),
            None => None,
        };
        if let (Some(f), Some(t)) = (from, to) {
            if f > t {
                return Err("from date must be on or before to date".into());
            }
        }
        Ok(())
    }
}

fn parse_iso_date(raw: &str) -> Result<NaiveDate, String> {
    NaiveDate::parse_from_str(raw.trim(), "%Y-%m-%d")
        .map_err(|_| format!("invalid ISO date: {raw}"))
}

/// Pagination for issue lists.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct Page {
    pub offset: u32,
    pub limit: u32,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct BottleneckDto {
    pub status: String,
    pub total_secs: i64,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct ThroughputPointDto {
    pub day: String,
    pub completed_count: u64,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct FlowMetricsDto {
    pub cycle_p50_secs: Option<f64>,
    pub cycle_p85_secs: Option<f64>,
    pub lead_p50_secs: Option<f64>,
    pub lead_p85_secs: Option<f64>,
    pub flow_efficiency: Option<f64>,
    pub throughput: Vec<ThroughputPointDto>,
    pub bottlenecks: Vec<BottleneckDto>,
    pub reopens: u64,
    pub handoffs: u64,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct SprintMetricsDto {
    pub sprint_id: String,
    pub name: Option<String>,
    pub committed: Option<i64>,
    pub completed: Option<i64>,
    pub spillover: Option<i64>,
    pub scope_added: Option<i64>,
    pub scope_removed: Option<i64>,
    pub velocity_points: Option<f64>,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct EpicRiskDto {
    pub epic_key: String,
    pub score: f64,
    pub finish_by_probability: Option<f64>,
    pub drivers: Vec<String>,
    pub assumptions: Vec<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct FinishByResultDto {
    pub probability: f64,
    pub assumptions: Vec<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct IssueRowDto {
    pub key: String,
    pub summary: Option<String>,
    pub project_key: String,
    pub status: Option<String>,
    pub assignee: Option<String>,
    pub story_points: Option<f64>,
    pub cycle_secs: Option<i64>,
    pub updated: String,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct IssuePageDto {
    pub items: Vec<IssueRowDto>,
    pub total: u64,
}

/// Flow metrics for the dashboard.
pub fn get_flow_metrics_inner(
    state: &AppState,
    filter: MetricsFilter,
) -> Result<FlowMetricsDto, String> {
    filter.validate()?;
    with_db(state, |conn| query_flow_metrics(conn, &filter))
}

/// Sprint metrics rows.
pub fn get_sprint_metrics_inner(
    state: &AppState,
    filter: MetricsFilter,
) -> Result<Vec<SprintMetricsDto>, String> {
    filter.validate()?;
    with_db(state, |conn| query_sprint_metrics(conn, &filter))
}

/// Epic risk rows from derived storage.
pub fn get_epic_risk_inner(
    state: &AppState,
    filter: MetricsFilter,
) -> Result<Vec<EpicRiskDto>, String> {
    filter.validate()?;
    with_db(state, |conn| query_epic_risk(conn, &filter))
}

/// Finish-by probability for an epic and target ISO date.
pub fn get_finish_by_inner(
    state: &AppState,
    epic_key: String,
    target_date: String,
) -> Result<FinishByResultDto, String> {
    let target = parse_iso_date(&target_date)?;
    with_db(state, |conn| query_finish_by(conn, &epic_key, target))
}

/// Paginated issue list.
pub fn list_issues_inner(
    state: &AppState,
    filter: MetricsFilter,
    page: Page,
) -> Result<IssuePageDto, String> {
    filter.validate()?;
    if page.limit == 0 {
        return Err("page.limit must be > 0".into());
    }
    let limit = page.limit.min(500);
    with_db(state, |conn| {
        query_issues(conn, &filter, page.offset, limit)
    })
}

fn with_db<T>(
    state: &AppState,
    f: impl FnOnce(&Connection) -> Result<T, String>,
) -> Result<T, String> {
    if !Path::new(&state.db_path).is_file() {
        return Err("database not found; run setup / sync first".into());
    }
    let conn = open_db(&state.db_path).map_err(|e| e.to_string())?;
    migrate(&conn).map_err(|e| e.to_string())?;
    f(&conn)
}

struct FilterSql {
    where_sql: String,
    params: Vec<Box<dyn ToSql>>,
}

fn issue_filter_sql(filter: &MetricsFilter, alias: &str) -> FilterSql {
    let mut clauses = Vec::new();
    let mut params: Vec<Box<dyn ToSql>> = Vec::new();

    if let Some(keys) = &filter.project_keys {
        if !keys.is_empty() {
            let placeholders = vec!["?"; keys.len()].join(", ");
            clauses.push(format!("{alias}.project_key IN ({placeholders})"));
            for k in keys {
                params.push(Box::new(k.clone()));
            }
        }
    }
    if let Some(types) = &filter.issue_types {
        if !types.is_empty() {
            let placeholders = vec!["?"; types.len()].join(", ");
            clauses.push(format!("{alias}.issue_type IN ({placeholders})"));
            for t in types {
                params.push(Box::new(t.clone()));
            }
        }
    }
    if let Some(ids) = &filter.assignee_ids {
        if !ids.is_empty() {
            let placeholders = vec!["?"; ids.len()].join(", ");
            clauses.push(format!("{alias}.assignee_account_id IN ({placeholders})"));
            for id in ids {
                params.push(Box::new(id.clone()));
            }
        }
    }
    if let Some(from) = &filter.from {
        clauses.push(format!("substr({alias}.updated, 1, 10) >= ?"));
        params.push(Box::new(from.clone()));
    }
    if let Some(to) = &filter.to {
        clauses.push(format!("substr({alias}.updated, 1, 10) <= ?"));
        params.push(Box::new(to.clone()));
    }

    let where_sql = if clauses.is_empty() {
        "1=1".into()
    } else {
        clauses.join(" AND ")
    };
    FilterSql { where_sql, params }
}

fn query_flow_metrics(conn: &Connection, filter: &MetricsFilter) -> Result<FlowMetricsDto, String> {
    let f = issue_filter_sql(filter, "i");
    let sql = format!(
        "SELECT c.cycle_secs, c.lead_secs
         FROM derived_issue_cycle c
         JOIN issues i ON i.id = c.issue_id
         WHERE {}",
        f.where_sql
    );
    let mut stmt = conn.prepare(&sql).map_err(|e| e.to_string())?;
    let rows = stmt
        .query_map(
            params_from_iter(f.params.iter().map(|p| p.as_ref())),
            |row| Ok((row.get::<_, Option<i64>>(0)?, row.get::<_, Option<i64>>(1)?)),
        )
        .map_err(|e| e.to_string())?
        .collect::<Result<Vec<_>, _>>()
        .map_err(|e| e.to_string())?;

    let mut cycles: Vec<f64> = rows
        .iter()
        .filter_map(|(c, _)| c.map(|v| v as f64))
        .collect();
    let mut leads: Vec<f64> = rows
        .iter()
        .filter_map(|(_, l)| l.map(|v| v as f64))
        .collect();
    cycles.sort_by(|a, b| a.partial_cmp(b).unwrap_or(std::cmp::Ordering::Equal));
    leads.sort_by(|a, b| a.partial_cmp(b).unwrap_or(std::cmp::Ordering::Equal));

    let flow_efficiency = query_flow_efficiency(conn, filter)?;

    let throughput = query_throughput(conn, filter)?;
    let bottlenecks = query_bottlenecks(conn, filter)?;
    let reopens = meta_u64(conn, "derived_events:reopens")?;
    let handoffs = meta_u64(conn, "derived_events:handoffs")?;

    Ok(FlowMetricsDto {
        cycle_p50_secs: percentile(&cycles, 0.50),
        cycle_p85_secs: percentile(&cycles, 0.85),
        lead_p50_secs: percentile(&leads, 0.50),
        lead_p85_secs: percentile(&leads, 0.85),
        flow_efficiency,
        throughput,
        bottlenecks,
        reopens,
        handoffs,
    })
}

/// Active / (active + waiting) from `derived_time_in_status` + status flow categories.
fn query_flow_efficiency(conn: &Connection, filter: &MetricsFilter) -> Result<Option<f64>, String> {
    let f = issue_filter_sql(filter, "i");
    let sql = format!(
        "SELECT t.status, SUM(t.duration_secs)
         FROM derived_time_in_status t
         JOIN issues i ON i.id = t.issue_id
         WHERE {}
         GROUP BY t.status",
        f.where_sql
    );
    let mut stmt = conn.prepare(&sql).map_err(|e| e.to_string())?;
    let rows = stmt
        .query_map(
            params_from_iter(f.params.iter().map(|p| p.as_ref())),
            |row| Ok((row.get::<_, String>(0)?, row.get::<_, i64>(1)?)),
        )
        .map_err(|e| e.to_string())?
        .collect::<Result<Vec<_>, _>>()
        .map_err(|e| e.to_string())?;

    let overrides = load_status_category_overrides(conn)?;
    let mut active: i64 = 0;
    let mut waiting: i64 = 0;
    for (status, secs) in rows {
        match resolve_status_category(&status, &overrides) {
            StatusFlowCategory::Active => active += secs,
            StatusFlowCategory::Waiting => waiting += secs,
            StatusFlowCategory::Terminal => {}
        }
    }
    let denom = active + waiting;
    if denom <= 0 {
        return Ok(None);
    }
    Ok(Some((active as f64 / denom as f64).clamp(0.0, 1.0)))
}

fn load_status_category_overrides(
    conn: &Connection,
) -> Result<BTreeMap<String, StatusFlowCategory>, String> {
    const PREFIX: &str = "status_flow_category:";
    let mut stmt = conn
        .prepare("SELECT key, value FROM meta WHERE key LIKE ?1")
        .map_err(|e| e.to_string())?;
    let rows = stmt
        .query_map([format!("{PREFIX}%")], |row| {
            Ok((row.get::<_, String>(0)?, row.get::<_, String>(1)?))
        })
        .map_err(|e| e.to_string())?;

    let mut map = BTreeMap::new();
    for row in rows {
        let (key, value) = row.map_err(|e| e.to_string())?;
        let status = key.strip_prefix(PREFIX).unwrap_or("").to_string();
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

fn query_throughput(
    conn: &Connection,
    filter: &MetricsFilter,
) -> Result<Vec<ThroughputPointDto>, String> {
    let mut clauses = Vec::new();
    let mut params: Vec<Box<dyn ToSql>> = Vec::new();
    if let Some(keys) = &filter.project_keys {
        if !keys.is_empty() {
            let placeholders = vec!["?"; keys.len()].join(", ");
            clauses.push(format!("project_key IN ({placeholders})"));
            for k in keys {
                params.push(Box::new(k.clone()));
            }
        }
    }
    if let Some(from) = &filter.from {
        clauses.push("day >= ?".into());
        params.push(Box::new(from.clone()));
    }
    if let Some(to) = &filter.to {
        clauses.push("day <= ?".into());
        params.push(Box::new(to.clone()));
    }
    let where_sql = if clauses.is_empty() {
        "1=1".into()
    } else {
        clauses.join(" AND ")
    };
    let sql = format!(
        "SELECT day, SUM(completed_count) as cnt
         FROM derived_throughput_daily
         WHERE {where_sql}
         GROUP BY day
         ORDER BY day"
    );
    let mut stmt = conn.prepare(&sql).map_err(|e| e.to_string())?;
    let rows = stmt
        .query_map(params_from_iter(params.iter().map(|p| p.as_ref())), |row| {
            Ok(ThroughputPointDto {
                day: row.get(0)?,
                completed_count: row.get::<_, i64>(1)? as u64,
            })
        })
        .map_err(|e| e.to_string())?
        .collect::<Result<Vec<_>, _>>()
        .map_err(|e| e.to_string())?;
    Ok(rows)
}

fn query_bottlenecks(
    conn: &Connection,
    filter: &MetricsFilter,
) -> Result<Vec<BottleneckDto>, String> {
    let f = issue_filter_sql(filter, "i");
    let sql = format!(
        "SELECT t.status, SUM(t.duration_secs) as total
         FROM derived_time_in_status t
         JOIN issues i ON i.id = t.issue_id
         WHERE {}
         GROUP BY t.status
         ORDER BY total DESC
         LIMIT 20",
        f.where_sql
    );
    let mut stmt = conn.prepare(&sql).map_err(|e| e.to_string())?;
    let rows = stmt
        .query_map(
            params_from_iter(f.params.iter().map(|p| p.as_ref())),
            |row| {
                Ok(BottleneckDto {
                    status: row.get(0)?,
                    total_secs: row.get(1)?,
                })
            },
        )
        .map_err(|e| e.to_string())?
        .collect::<Result<Vec<_>, _>>()
        .map_err(|e| e.to_string())?;
    Ok(rows)
}

fn query_sprint_metrics(
    conn: &Connection,
    filter: &MetricsFilter,
) -> Result<Vec<SprintMetricsDto>, String> {
    let mut sql = String::from(
        "SELECT m.sprint_id, s.name, m.committed, m.completed, m.spillover,
                m.scope_added, m.scope_removed, m.velocity_points
         FROM derived_sprint_metrics m
         LEFT JOIN sprints s ON s.id = m.sprint_id
         WHERE 1=1",
    );
    let mut params: Vec<Box<dyn ToSql>> = Vec::new();

    // Optional project filter via sprint_issues → issues.
    if let Some(keys) = &filter.project_keys {
        if !keys.is_empty() {
            let placeholders = vec!["?"; keys.len()].join(", ");
            sql.push_str(&format!(
                " AND EXISTS (
                    SELECT 1 FROM sprint_issues si
                    JOIN issues i ON i.id = si.issue_id
                    WHERE si.sprint_id = m.sprint_id
                      AND i.project_key IN ({placeholders})
                  )"
            ));
            for k in keys {
                params.push(Box::new(k.clone()));
            }
        }
    }
    if let Some(from) = &filter.from {
        sql.push_str(" AND (s.start_date IS NULL OR substr(s.start_date, 1, 10) >= ?)");
        params.push(Box::new(from.clone()));
    }
    if let Some(to) = &filter.to {
        sql.push_str(" AND (s.end_date IS NULL OR substr(s.end_date, 1, 10) <= ?)");
        params.push(Box::new(to.clone()));
    }
    sql.push_str(" ORDER BY s.start_date IS NULL, s.start_date DESC, m.sprint_id");

    let mut stmt = conn.prepare(&sql).map_err(|e| e.to_string())?;
    let rows = stmt
        .query_map(params_from_iter(params.iter().map(|p| p.as_ref())), |row| {
            Ok(SprintMetricsDto {
                sprint_id: row.get(0)?,
                name: row.get(1)?,
                committed: row.get(2)?,
                completed: row.get(3)?,
                spillover: row.get(4)?,
                scope_added: row.get(5)?,
                scope_removed: row.get(6)?,
                velocity_points: row.get(7)?,
            })
        })
        .map_err(|e| e.to_string())?
        .collect::<Result<Vec<_>, _>>()
        .map_err(|e| e.to_string())?;
    Ok(rows)
}

fn query_epic_risk(conn: &Connection, filter: &MetricsFilter) -> Result<Vec<EpicRiskDto>, String> {
    let mut sql = String::from(
        "SELECT r.epic_key, r.risk_score, r.finish_by_probability,
                r.assumptions_json, r.drivers_json
         FROM derived_epic_risk r
         WHERE 1=1",
    );
    let mut params: Vec<Box<dyn ToSql>> = Vec::new();
    if let Some(keys) = &filter.project_keys {
        if !keys.is_empty() {
            let placeholders = vec!["?"; keys.len()].join(", ");
            sql.push_str(&format!(
                " AND EXISTS (
                    SELECT 1 FROM issues i
                    WHERE i.epic_key = r.epic_key
                      AND i.project_key IN ({placeholders})
                  )"
            ));
            for k in keys {
                params.push(Box::new(k.clone()));
            }
        }
    }
    sql.push_str(" ORDER BY r.risk_score DESC, r.epic_key");

    let mut stmt = conn.prepare(&sql).map_err(|e| e.to_string())?;
    let rows = stmt
        .query_map(params_from_iter(params.iter().map(|p| p.as_ref())), |row| {
            let assumptions_json: Option<String> = row.get(3)?;
            let drivers_json: Option<String> = row.get(4)?;
            let assumptions = parse_string_list(assumptions_json.as_deref());
            let drivers = parse_string_list(drivers_json.as_deref());
            Ok(EpicRiskDto {
                epic_key: row.get(0)?,
                score: row.get::<_, Option<f64>>(1)?.unwrap_or(0.0),
                finish_by_probability: row.get(2)?,
                drivers,
                assumptions,
            })
        })
        .map_err(|e| e.to_string())?
        .collect::<Result<Vec<_>, _>>()
        .map_err(|e| e.to_string())?;
    Ok(rows)
}

fn query_finish_by(
    conn: &Connection,
    epic_key: &str,
    target: NaiveDate,
) -> Result<FinishByResultDto, String> {
    let now = Utc::now();
    let target_end = target
        .and_hms_opt(23, 59, 59)
        .ok_or_else(|| "invalid target date".to_string())?
        .and_utc();
    let weeks_until_target =
        ((target_end - now).num_seconds().max(0) as f64) / (7.0 * 24.0 * 3600.0);

    let remaining_issues: u32 = conn
        .query_row(
            "SELECT COUNT(*) FROM issues
             WHERE epic_key = ?1
               AND (resolved IS NULL OR TRIM(resolved) = '')
               AND LOWER(COALESCE(status_category, '')) NOT LIKE '%done%'
               AND LOWER(COALESCE(status, '')) NOT IN ('done', 'closed', 'resolved', 'complete')",
            [epic_key],
            |row| row.get(0),
        )
        .map_err(|e| e.to_string())?;

    let lookback_start = now - chrono::Duration::days(56);
    let completed = count_completed_in_window(conn, epic_key, lookback_start, now)?;
    let avg_weekly = completed as f64 / 8.0;
    let stddev = (avg_weekly * 0.25).max(0.5);

    let result = finish_by_probability(&FinishByInput {
        remaining_work_issues: remaining_issues as f64,
        weekly_throughput_issues: avg_weekly,
        weeks_until_target,
        throughput_stddev: stddev,
    });

    Ok(FinishByResultDto {
        probability: result.probability,
        assumptions: result.assumptions,
    })
}

fn count_completed_in_window(
    conn: &Connection,
    epic_key: &str,
    start: DateTime<Utc>,
    end: DateTime<Utc>,
) -> Result<u32, String> {
    let mut stmt = conn
        .prepare(
            "SELECT COALESCE(c.completed_at, i.resolved)
             FROM issues i
             LEFT JOIN derived_issue_cycle c ON c.issue_id = i.id
             WHERE i.epic_key = ?1
               AND COALESCE(c.completed_at, i.resolved) IS NOT NULL",
        )
        .map_err(|e| e.to_string())?;
    let rows = stmt
        .query_map([epic_key], |row| row.get::<_, String>(0))
        .map_err(|e| e.to_string())?
        .collect::<Result<Vec<_>, _>>()
        .map_err(|e| e.to_string())?;

    let mut count = 0u32;
    for raw in rows {
        if let Some(at) = parse_dt(&raw) {
            if at >= start && at <= end {
                count += 1;
            }
        }
    }
    Ok(count)
}

fn query_issues(
    conn: &Connection,
    filter: &MetricsFilter,
    offset: u32,
    limit: u32,
) -> Result<IssuePageDto, String> {
    let f = issue_filter_sql(filter, "i");
    let count_sql = format!("SELECT COUNT(*) FROM issues i WHERE {}", f.where_sql);
    let total: u64 = {
        let mut stmt = conn.prepare(&count_sql).map_err(|e| e.to_string())?;
        stmt.query_row(
            params_from_iter(f.params.iter().map(|p| p.as_ref())),
            |row| row.get::<_, i64>(0),
        )
        .map_err(|e| e.to_string())? as u64
    };

    let mut params = f.params;
    params.push(Box::new(limit as i64));
    params.push(Box::new(offset as i64));
    let list_sql = format!(
        "SELECT i.key, i.summary, i.project_key, i.status, i.assignee_account_id,
                i.story_points, c.cycle_secs, i.updated
         FROM issues i
         LEFT JOIN derived_issue_cycle c ON c.issue_id = i.id
         WHERE {}
         ORDER BY i.updated DESC
         LIMIT ? OFFSET ?",
        f.where_sql
    );
    let mut stmt = conn.prepare(&list_sql).map_err(|e| e.to_string())?;
    let items = stmt
        .query_map(params_from_iter(params.iter().map(|p| p.as_ref())), |row| {
            Ok(IssueRowDto {
                key: row.get(0)?,
                summary: row.get(1)?,
                project_key: row.get(2)?,
                status: row.get(3)?,
                assignee: row.get(4)?,
                story_points: row.get(5)?,
                cycle_secs: row.get(6)?,
                updated: row.get(7)?,
            })
        })
        .map_err(|e| e.to_string())?
        .collect::<Result<Vec<_>, _>>()
        .map_err(|e| e.to_string())?;

    Ok(IssuePageDto { items, total })
}

fn meta_u64(conn: &Connection, key: &str) -> Result<u64, String> {
    let value: Option<String> = conn
        .query_row("SELECT value FROM meta WHERE key = ?1", [key], |row| {
            row.get(0)
        })
        .optional()
        .map_err(|e| e.to_string())?;
    Ok(value.and_then(|v| v.parse().ok()).unwrap_or(0))
}

fn parse_string_list(raw: Option<&str>) -> Vec<String> {
    let Some(raw) = raw else {
        return Vec::new();
    };
    serde_json::from_str(raw).unwrap_or_default()
}

fn percentile(sorted: &[f64], p: f64) -> Option<f64> {
    if sorted.is_empty() {
        return None;
    }
    let n = sorted.len();
    let rank = ((p * n as f64).ceil() as usize).clamp(1, n);
    Some(sorted[rank - 1])
}

fn parse_dt(raw: &str) -> Option<DateTime<Utc>> {
    if let Ok(d) = DateTime::parse_from_rfc3339(raw) {
        return Some(d.with_timezone(&Utc));
    }
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

trait OptionalExt<T> {
    fn optional(self) -> Result<Option<T>, rusqlite::Error>;
}

impl<T> OptionalExt<T> for Result<T, rusqlite::Error> {
    fn optional(self) -> Result<Option<T>, rusqlite::Error> {
        match self {
            Ok(v) => Ok(Some(v)),
            Err(rusqlite::Error::QueryReturnedNoRows) => Ok(None),
            Err(e) => Err(e),
        }
    }
}

#[cfg(feature = "desktop")]
pub mod tauri_cmds {
    use super::*;
    use tauri::State;

    #[tauri::command]
    pub fn get_flow_metrics(
        state: State<'_, std::sync::Arc<AppState>>,
        filter: MetricsFilter,
    ) -> Result<FlowMetricsDto, String> {
        get_flow_metrics_inner(&state, filter)
    }

    #[tauri::command]
    pub fn get_sprint_metrics(
        state: State<'_, std::sync::Arc<AppState>>,
        filter: MetricsFilter,
    ) -> Result<Vec<SprintMetricsDto>, String> {
        get_sprint_metrics_inner(&state, filter)
    }

    #[tauri::command]
    pub fn get_epic_risk(
        state: State<'_, std::sync::Arc<AppState>>,
        filter: MetricsFilter,
    ) -> Result<Vec<EpicRiskDto>, String> {
        get_epic_risk_inner(&state, filter)
    }

    #[tauri::command]
    pub fn get_finish_by(
        state: State<'_, std::sync::Arc<AppState>>,
        epic_key: String,
        target_date: String,
    ) -> Result<FinishByResultDto, String> {
        get_finish_by_inner(&state, epic_key, target_date)
    }

    #[tauri::command]
    pub fn list_issues(
        state: State<'_, std::sync::Arc<AppState>>,
        filter: MetricsFilter,
        page: Page,
    ) -> Result<IssuePageDto, String> {
        list_issues_inner(&state, filter, page)
    }
}

#[cfg(feature = "desktop")]
pub use tauri_cmds::{
    get_epic_risk, get_finish_by, get_flow_metrics, get_sprint_metrics, list_issues,
};

#[cfg(test)]
mod tests {
    use super::*;
    use ag_credentials::{
        CredentialStore, GeminiCredentials, JiraCredentials, MemoryCredentialStore,
    };
    use std::sync::Arc;
    use tempfile::tempdir;

    #[test]
    fn metrics_filter_rejects_inverted_date_range() {
        let err = MetricsFilter {
            project_keys: None,
            from: Some("2025-01-10".into()),
            to: Some("2025-01-01".into()),
            issue_types: None,
            assignee_ids: None,
        }
        .validate();
        assert!(err.is_err());
    }

    #[test]
    fn metrics_filter_accepts_ordered_dates() {
        MetricsFilter {
            project_keys: None,
            from: Some("2025-01-01".into()),
            to: Some("2025-01-10".into()),
            issue_types: None,
            assignee_ids: None,
        }
        .validate()
        .unwrap();
    }

    #[test]
    fn list_issues_respects_filter_and_page() {
        let dir = tempdir().unwrap();
        let db_path = dir.path().join("t.db");
        let conn = open_db(&db_path).unwrap();
        migrate(&conn).unwrap();
        conn.execute(
            "INSERT INTO issues (
                id, key, project_key, summary, issue_type, status,
                assignee_account_id, created, updated
             ) VALUES ('1', 'A-1', 'A', 'one', 'Story', 'Done', 'u1',
                       '2025-01-01T00:00:00Z', '2025-01-05T00:00:00Z')",
            [],
        )
        .unwrap();
        conn.execute(
            "INSERT INTO issues (
                id, key, project_key, summary, issue_type, status,
                assignee_account_id, created, updated
             ) VALUES ('2', 'B-1', 'B', 'two', 'Bug', 'Open', 'u2',
                       '2025-01-01T00:00:00Z', '2025-01-06T00:00:00Z')",
            [],
        )
        .unwrap();
        drop(conn);

        let store = MemoryCredentialStore::default();
        store
            .save_jira(&JiraCredentials {
                site_url: "https://example.atlassian.net".into(),
                email: "a@b.c".into(),
                api_token: "t".into(),
            })
            .unwrap();
        store
            .save_gemini(&GeminiCredentials {
                api_key: "g".into(),
            })
            .unwrap();
        let state = AppState::with_credentials(db_path, Arc::new(store));

        let page = list_issues_inner(
            &state,
            MetricsFilter {
                project_keys: Some(vec!["A".into()]),
                from: None,
                to: None,
                issue_types: None,
                assignee_ids: None,
            },
            Page {
                offset: 0,
                limit: 10,
            },
        )
        .unwrap();
        assert_eq!(page.total, 1);
        assert_eq!(page.items[0].key, "A-1");
    }

    #[test]
    fn flow_efficiency_uses_active_vs_waiting_time_in_status() {
        let dir = tempdir().unwrap();
        let db_path = dir.path().join("flow.db");
        {
            let conn = open_db(&db_path).unwrap();
            migrate(&conn).unwrap();
            conn.execute(
                "INSERT INTO issues (
                    id, key, project_key, summary, issue_type, status, created, updated
                 ) VALUES ('1', 'A-1', 'A', 'one', 'Story', 'Done',
                           '2025-01-01T00:00:00Z', '2025-01-05T00:00:00Z')",
                [],
            )
            .unwrap();
            conn.execute(
                "INSERT INTO derived_time_in_status (issue_id, status, duration_secs)
                 VALUES ('1', 'In Progress', 3600), ('1', 'To Do', 3600)",
                [],
            )
            .unwrap();
            conn.execute(
                "INSERT INTO derived_issue_cycle (issue_id, cycle_secs, lead_secs, completed_at)
                 VALUES ('1', 100, 10000, '2025-01-05T00:00:00Z')",
                [],
            )
            .unwrap();
        }

        let store = MemoryCredentialStore::default();
        store
            .save_jira(&JiraCredentials {
                site_url: "https://example.atlassian.net".into(),
                email: "a@b.c".into(),
                api_token: "t".into(),
            })
            .unwrap();
        store
            .save_gemini(&GeminiCredentials {
                api_key: "g".into(),
            })
            .unwrap();
        let state = AppState::with_credentials(db_path, Arc::new(store));
        let metrics = get_flow_metrics_inner(
            &state,
            MetricsFilter {
                project_keys: None,
                from: None,
                to: None,
                issue_types: None,
                assignee_ids: None,
            },
        )
        .unwrap();
        // Active 3600 / (active 3600 + waiting 3600) = 0.5; not cycle/lead proxy (~0.01).
        assert!((metrics.flow_efficiency.unwrap() - 0.5).abs() < 1e-9);
    }

    #[test]
    fn epic_risk_reads_drivers_json_not_assumptions() {
        let dir = tempdir().unwrap();
        let db_path = dir.path().join("epic.db");
        {
            let conn = open_db(&db_path).unwrap();
            migrate(&conn).unwrap();
            conn.execute(
                "INSERT INTO issues (
                    id, key, project_key, summary, issue_type, status, epic_key, created, updated
                 ) VALUES ('1', 'A-1', 'A', 'one', 'Story', 'To Do', 'EPIC-1',
                           '2025-01-01T00:00:00Z', '2025-01-05T00:00:00Z')",
                [],
            )
            .unwrap();
            conn.execute(
                "INSERT INTO derived_epic_risk (
                    epic_key, risk_score, finish_by_probability, assumptions_json, drivers_json
                 ) VALUES (
                    'EPIC-1', 80.0, 0.2,
                    '[\"Weekly throughput assumption\"]',
                    '[\"Throughput pressure (40 pts): remaining work vs weekly completions\"]'
                 )",
                [],
            )
            .unwrap();
        }

        let store = MemoryCredentialStore::default();
        store
            .save_jira(&JiraCredentials {
                site_url: "https://example.atlassian.net".into(),
                email: "a@b.c".into(),
                api_token: "t".into(),
            })
            .unwrap();
        store
            .save_gemini(&GeminiCredentials {
                api_key: "g".into(),
            })
            .unwrap();
        let state = AppState::with_credentials(db_path, Arc::new(store));
        let epics = get_epic_risk_inner(
            &state,
            MetricsFilter {
                project_keys: None,
                from: None,
                to: None,
                issue_types: None,
                assignee_ids: None,
            },
        )
        .unwrap();
        assert_eq!(epics.len(), 1);
        assert!(epics[0].drivers[0].contains("Throughput pressure"));
        assert!(epics[0].assumptions[0].contains("Weekly throughput"));
        assert_ne!(epics[0].drivers, epics[0].assumptions);
    }
}
