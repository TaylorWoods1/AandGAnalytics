//! Context pack builder: curated metrics + supporting issues under a token budget.

use rusqlite::{params_from_iter, Connection, ToSql};
use serde::{Deserialize, Serialize};

use crate::error::BedrockError;

/// Shared analytics filter (same shape as Tauri `MetricsFilter`).
#[derive(Debug, Clone, Default, Serialize, Deserialize, PartialEq, Eq)]
pub struct MetricsFilter {
    pub project_keys: Option<Vec<String>>,
    pub from: Option<String>,
    pub to: Option<String>,
    pub issue_types: Option<Vec<String>>,
    pub assignee_ids: Option<Vec<String>>,
}

/// One supporting issue citation included in a context pack.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct IssueCite {
    pub key: String,
    pub summary: Option<String>,
    pub status: Option<String>,
    pub project_key: String,
    pub cycle_secs: Option<i64>,
}

/// Curated pack sent to Bedrock (aggregates + top supporting issues).
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct ContextPack {
    pub filter_summary: String,
    pub metrics_markdown: String,
    pub supporting_issues: Vec<IssueCite>,
    pub approx_tokens: usize,
}

/// Approximate token count (~4 chars/token) for pack contents.
pub fn approx_token_count(pack: &ContextPack) -> usize {
    let mut chars = pack.filter_summary.len() + pack.metrics_markdown.len();
    for issue in &pack.supporting_issues {
        chars += issue.key.len();
        chars += issue.summary.as_deref().unwrap_or("").len();
        chars += issue.status.as_deref().unwrap_or("").len();
        chars += issue.project_key.len();
        chars += 16; // cycle_secs formatting overhead
    }
    chars.div_ceil(4).max(1)
}

/// Render the pack as the user-facing context block for Bedrock.
pub fn format_pack_for_prompt(pack: &ContextPack) -> String {
    let mut out = String::new();
    out.push_str("## Active filters\n");
    out.push_str(&pack.filter_summary);
    out.push_str("\n\n## Metrics\n");
    out.push_str(&pack.metrics_markdown);
    out.push_str("\n\n## Supporting issues\n");
    if pack.supporting_issues.is_empty() {
        out.push_str("(none)\n");
    } else {
        for issue in &pack.supporting_issues {
            let summary = issue.summary.as_deref().unwrap_or("(no summary)");
            let status = issue.status.as_deref().unwrap_or("?");
            let cycle = issue
                .cycle_secs
                .map(|s| format!("{s}s"))
                .unwrap_or_else(|| "n/a".into());
            out.push_str(&format!(
                "- {}: {} | project={} | status={} | cycle={}\n",
                issue.key, summary, issue.project_key, status, cycle
            ));
        }
    }
    out
}

/// Build a context pack from derived metrics + top issues, respecting `token_budget`.
pub fn build_context_pack(
    conn: &Connection,
    filter: &MetricsFilter,
    token_budget: usize,
) -> Result<ContextPack, BedrockError> {
    if token_budget == 0 {
        return Err(BedrockError::Other("token_budget must be > 0".into()));
    }

    let filter_summary = summarize_filter(filter);
    let metrics_markdown = build_metrics_markdown(conn, filter)?;
    let candidates = load_supporting_issues(conn, filter, 80)?;

    let mut pack = ContextPack {
        filter_summary,
        metrics_markdown,
        supporting_issues: Vec::new(),
        approx_tokens: 0,
    };
    pack.approx_tokens = approx_token_count(&pack);

    // Always keep filter + metrics; trim metrics if they alone exceed budget.
    while pack.approx_tokens > token_budget && pack.metrics_markdown.len() > 80 {
        let keep = pack.metrics_markdown.len() * 3 / 4;
        pack.metrics_markdown.truncate(keep.max(80));
        pack.metrics_markdown.push_str("\n…(truncated)");
        pack.approx_tokens = approx_token_count(&pack);
    }

    for issue in candidates {
        pack.supporting_issues.push(issue);
        let tokens = approx_token_count(&pack);
        if tokens > token_budget {
            pack.supporting_issues.pop();
            break;
        }
        pack.approx_tokens = tokens;
    }
    pack.approx_tokens = approx_token_count(&pack);
    Ok(pack)
}

fn summarize_filter(filter: &MetricsFilter) -> String {
    let mut parts = Vec::new();
    match &filter.project_keys {
        Some(keys) if !keys.is_empty() => parts.push(format!("projects={}", keys.join(","))),
        _ => parts.push("projects=all".into()),
    }
    match (&filter.from, &filter.to) {
        (Some(f), Some(t)) => parts.push(format!("dates={f}..{t}")),
        (Some(f), None) => parts.push(format!("dates>={f}")),
        (None, Some(t)) => parts.push(format!("dates<={t}")),
        (None, None) => parts.push("dates=all".into()),
    }
    match &filter.issue_types {
        Some(types) if !types.is_empty() => parts.push(format!("types={}", types.join(","))),
        _ => parts.push("types=all".into()),
    }
    match &filter.assignee_ids {
        Some(ids) if !ids.is_empty() => parts.push(format!("assignees={}", ids.join(","))),
        _ => parts.push("assignees=all".into()),
    }
    parts.join("; ")
}

fn build_metrics_markdown(
    conn: &Connection,
    filter: &MetricsFilter,
) -> Result<String, BedrockError> {
    let mut md = String::new();

    let f = issue_filter_sql(filter, "i");
    let cycle_sql = format!(
        "SELECT c.cycle_secs, c.lead_secs
         FROM derived_issue_cycle c
         JOIN issues i ON i.id = c.issue_id
         WHERE {}",
        f.where_sql
    );
    let mut stmt = conn.prepare(&cycle_sql)?;
    let rows: Vec<(Option<i64>, Option<i64>)> = stmt
        .query_map(
            params_from_iter(f.params.iter().map(|p| p.as_ref())),
            |row| Ok((row.get(0)?, row.get(1)?)),
        )?
        .collect::<Result<Vec<_>, _>>()?;

    let mut cycles: Vec<i64> = rows.iter().filter_map(|(c, _)| *c).collect();
    let mut leads: Vec<i64> = rows.iter().filter_map(|(_, l)| *l).collect();
    cycles.sort_unstable();
    leads.sort_unstable();

    md.push_str(&format!(
        "- cycle_p50_secs: {}\n- cycle_p85_secs: {}\n- lead_p50_secs: {}\n- lead_p85_secs: {}\n",
        fmt_opt_i64(percentile_i64(&cycles, 0.50)),
        fmt_opt_i64(percentile_i64(&cycles, 0.85)),
        fmt_opt_i64(percentile_i64(&leads, 0.50)),
        fmt_opt_i64(percentile_i64(&leads, 0.85)),
    ));

    let bottlenecks = query_bottlenecks(conn, filter)?;
    if bottlenecks.is_empty() {
        md.push_str("- bottlenecks: (none)\n");
    } else {
        md.push_str("- bottlenecks (status consuming most calendar time):\n");
        for (status, secs) in bottlenecks.iter().take(8) {
            md.push_str(&format!("  - bottleneck:{status} = {secs}s\n"));
        }
    }

    let throughput_total = query_throughput_total(conn, filter)?;
    md.push_str(&format!(
        "- throughput_completed_issues: {throughput_total}\n"
    ));

    let reopens = meta_u64(conn, "derived_events:reopens")?;
    let handoffs = meta_u64(conn, "derived_events:handoffs")?;
    md.push_str(&format!("- reopens: {reopens}\n- handoffs: {handoffs}\n"));

    let epics = query_top_epics(conn, filter, 5)?;
    if epics.is_empty() {
        md.push_str("- epic_risk: (none)\n");
    } else {
        md.push_str("- epic_risk (top):\n");
        for (key, score, prob) in epics {
            let p = prob
                .map(|v| format!("{v:.2}"))
                .unwrap_or_else(|| "n/a".into());
            md.push_str(&format!(
                "  - {key}: risk_score={score:.1}, finish_by_p={p}\n"
            ));
        }
    }

    append_performance_rollups(conn, filter, &mut md)?;

    Ok(md)
}

fn append_performance_rollups(
    conn: &Connection,
    filter: &MetricsFilter,
    md: &mut String,
) -> Result<(), BedrockError> {
    // Skip silently if performance tables are empty / missing (pre-migrate DBs).
    let table_ok: i64 = conn
        .query_row(
            "SELECT COUNT(*) FROM sqlite_master
             WHERE type='table' AND name='derived_completions'",
            [],
            |row| row.get(0),
        )
        .unwrap_or(0);
    if table_ok == 0 {
        return Ok(());
    }

    let mut clauses = Vec::new();
    let mut params: Vec<Box<dyn ToSql>> = Vec::new();
    if let Some(keys) = &filter.project_keys {
        if !keys.is_empty() {
            let placeholders = vec!["?"; keys.len()].join(", ");
            clauses.push(format!("dc.project_key IN ({placeholders})"));
            for k in keys {
                params.push(Box::new(k.clone()));
            }
        }
    }
    if let Some(from) = &filter.from {
        clauses.push("substr(dc.completed_at, 1, 10) >= ?".into());
        params.push(Box::new(from.clone()));
    }
    if let Some(to) = &filter.to {
        clauses.push("substr(dc.completed_at, 1, 10) <= ?".into());
        params.push(Box::new(to.clone()));
    }
    let where_sql = if clauses.is_empty() {
        "1=1".into()
    } else {
        clauses.join(" AND ")
    };

    let person_sql = format!(
        "SELECT dc.finisher_account_id, COUNT(*)
         FROM derived_completions dc
         WHERE {where_sql} AND dc.finisher_account_id IS NOT NULL
         GROUP BY dc.finisher_account_id
         ORDER BY COUNT(*) DESC
         LIMIT 5"
    );
    let mut person_stmt = conn.prepare(&person_sql)?;
    let people: Vec<(String, i64)> = person_stmt
        .query_map(
            params_from_iter(params.iter().map(|p| p.as_ref())),
            |row| Ok((row.get(0)?, row.get(1)?)),
        )?
        .collect::<Result<Vec<_>, _>>()?;

    let project_sql = format!(
        "SELECT dc.project_key, COUNT(*)
         FROM derived_completions dc
         WHERE {where_sql}
         GROUP BY dc.project_key
         ORDER BY COUNT(*) DESC
         LIMIT 5"
    );
    let mut project_stmt = conn.prepare(&project_sql)?;
    let projects: Vec<(String, i64)> = project_stmt
        .query_map(
            params_from_iter(params.iter().map(|p| p.as_ref())),
            |row| Ok((row.get(0)?, row.get(1)?)),
        )?
        .collect::<Result<Vec<_>, _>>()?;

    if people.is_empty() && projects.is_empty() {
        md.push_str("- performance_throughput: (none)\n");
        return Ok(());
    }

    md.push_str("- performance_throughput (top finishers / projects by completed tickets):\n");
    for (account_id, count) in &people {
        md.push_str(&format!("  - person:{account_id} = {count} completed\n"));
    }
    for (project_key, count) in &projects {
        md.push_str(&format!("  - project:{project_key} = {count} completed\n"));
    }
    Ok(())
}

fn load_supporting_issues(
    conn: &Connection,
    filter: &MetricsFilter,
    limit: usize,
) -> Result<Vec<IssueCite>, BedrockError> {
    let f = issue_filter_sql(filter, "i");
    let sql = format!(
        "SELECT i.key, i.summary, i.status, i.project_key, c.cycle_secs
         FROM issues i
         LEFT JOIN derived_issue_cycle c ON c.issue_id = i.id
         WHERE {}
         ORDER BY COALESCE(c.cycle_secs, 0) DESC, i.updated DESC
         LIMIT ?",
        f.where_sql
    );
    let mut params = f.params;
    params.push(Box::new(limit as i64));
    let mut stmt = conn.prepare(&sql)?;
    let rows = stmt
        .query_map(params_from_iter(params.iter().map(|p| p.as_ref())), |row| {
            Ok(IssueCite {
                key: row.get(0)?,
                summary: row.get(1)?,
                status: row.get(2)?,
                project_key: row.get(3)?,
                cycle_secs: row.get(4)?,
            })
        })?
        .collect::<Result<Vec<_>, _>>()?;
    Ok(rows)
}

fn query_bottlenecks(
    conn: &Connection,
    filter: &MetricsFilter,
) -> Result<Vec<(String, i64)>, BedrockError> {
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
    let mut stmt = conn.prepare(&sql)?;
    let rows = stmt
        .query_map(
            params_from_iter(f.params.iter().map(|p| p.as_ref())),
            |row| Ok((row.get::<_, String>(0)?, row.get::<_, i64>(1)?)),
        )?
        .collect::<Result<Vec<_>, _>>()?;
    Ok(rows)
}

fn query_throughput_total(conn: &Connection, filter: &MetricsFilter) -> Result<i64, BedrockError> {
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
        "SELECT COALESCE(SUM(completed_count), 0) FROM derived_throughput_daily WHERE {where_sql}"
    );
    let mut stmt = conn.prepare(&sql)?;
    let total: i64 = stmt
        .query_row(params_from_iter(params.iter().map(|p| p.as_ref())), |row| {
            row.get(0)
        })?;
    Ok(total)
}

fn query_top_epics(
    conn: &Connection,
    filter: &MetricsFilter,
    limit: usize,
) -> Result<Vec<(String, f64, Option<f64>)>, BedrockError> {
    let mut sql = String::from(
        "SELECT r.epic_key, COALESCE(r.risk_score, 0), r.finish_by_probability
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
    sql.push_str(" ORDER BY COALESCE(r.risk_score, 0) DESC, r.epic_key LIMIT ?");
    params.push(Box::new(limit as i64));

    let mut stmt = conn.prepare(&sql)?;
    let rows = stmt
        .query_map(params_from_iter(params.iter().map(|p| p.as_ref())), |row| {
            Ok((
                row.get::<_, String>(0)?,
                row.get::<_, f64>(1)?,
                row.get::<_, Option<f64>>(2)?,
            ))
        })?
        .collect::<Result<Vec<_>, _>>()?;
    Ok(rows)
}

fn meta_u64(conn: &Connection, key: &str) -> Result<u64, BedrockError> {
    let mut stmt = conn.prepare("SELECT value FROM meta WHERE key = ?1")?;
    let value: Option<String> = match stmt.query_row([key], |row| row.get(0)) {
        Ok(v) => Some(v),
        Err(rusqlite::Error::QueryReturnedNoRows) => None,
        Err(e) => return Err(e.into()),
    };
    Ok(value.and_then(|v| v.parse().ok()).unwrap_or(0))
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

fn percentile_i64(sorted: &[i64], p: f64) -> Option<i64> {
    if sorted.is_empty() {
        return None;
    }
    let n = sorted.len();
    let rank = ((p * n as f64).ceil() as usize).clamp(1, n);
    Some(sorted[rank - 1])
}

fn fmt_opt_i64(v: Option<i64>) -> String {
    v.map(|n| n.to_string()).unwrap_or_else(|| "n/a".into())
}

#[cfg(test)]
mod tests {
    use super::*;
    use ag_db::{migrate, open_db};

    fn build_context_pack_from_fixture(token_budget: usize) -> ContextPack {
        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().join("fixture.db");
        let conn = open_db(&path).unwrap();
        migrate(&conn).unwrap();

        for i in 1..=40 {
            let id = format!("id-{i}");
            let key = format!("PROJ-{i}");
            conn.execute(
                "INSERT INTO issues (
                    id, key, project_key, summary, issue_type, status,
                    assignee_account_id, created, updated
                 ) VALUES (?1, ?2, 'PROJ', ?3, 'Story', 'In Progress', 'u1',
                           '2025-01-01T00:00:00Z', '2025-06-01T00:00:00Z')",
                rusqlite::params![
                    id,
                    key,
                    format!("Issue {i} with a longer summary for tokens")
                ],
            )
            .unwrap();
            conn.execute(
                "INSERT INTO derived_issue_cycle (issue_id, cycle_secs, lead_secs, completed_at)
                 VALUES (?1, ?2, ?3, NULL)",
                rusqlite::params![id, (i as i64) * 3600, (i as i64) * 4000],
            )
            .unwrap();
            conn.execute(
                "INSERT INTO derived_time_in_status (issue_id, status, duration_secs)
                 VALUES (?1, 'Code Review', ?2)",
                rusqlite::params![id, (i as i64) * 1000],
            )
            .unwrap();
        }
        conn.execute(
            "INSERT INTO derived_throughput_daily (day, project_key, completed_count)
             VALUES ('2025-05-01', 'PROJ', 12)",
            [],
        )
        .unwrap();
        conn.execute(
            "INSERT INTO derived_epic_risk (epic_key, risk_score, finish_by_probability, assumptions_json)
             VALUES ('PROJ-EPIC', 72.5, 0.35, '[\"low throughput\"]')",
            [],
        )
        .unwrap();
        conn.execute(
            "INSERT INTO meta (key, value) VALUES ('derived_events:reopens', '3')",
            [],
        )
        .unwrap();
        conn.execute(
            "INSERT INTO meta (key, value) VALUES ('derived_events:handoffs', '5')",
            [],
        )
        .unwrap();
        conn.execute(
            "INSERT INTO derived_completions (
                issue_id, project_key, completed_at, finisher_account_id, story_points, attribution
             ) VALUES
             ('id-1', 'PROJ', '2025-05-10T00:00:00+00:00', 'finisher-ada', 3.0, 'current'),
             ('id-2', 'PROJ', '2025-05-11T00:00:00+00:00', 'finisher-ada', 2.0, 'current'),
             ('id-3', 'PROJ', '2025-05-12T00:00:00+00:00', 'finisher-bob', 1.0, 'changelog')",
            [],
        )
        .unwrap();

        let filter = MetricsFilter {
            project_keys: Some(vec!["PROJ".into()]),
            from: None,
            to: None,
            issue_types: None,
            assignee_ids: None,
        };
        build_context_pack(&conn, &filter, token_budget).unwrap()
    }

    #[test]
    fn context_pack_respects_token_budget() {
        let pack = build_context_pack_from_fixture(1_000);
        assert!(pack.approx_tokens <= 1_000);
        assert!(!pack.supporting_issues.is_empty());
    }

    #[test]
    fn context_pack_includes_filter_and_metrics() {
        let pack = build_context_pack_from_fixture(8_000);
        assert!(pack.filter_summary.contains("PROJ"));
        assert!(pack.metrics_markdown.contains("bottleneck:Code Review"));
        assert!(pack.approx_tokens > 0);
    }

    #[test]
    fn context_pack_includes_performance_person_and_project_lines() {
        let pack = build_context_pack_from_fixture(8_000);
        assert!(
            pack.metrics_markdown.contains("person:finisher-ada"),
            "expected person throughput line, got:\n{}",
            pack.metrics_markdown
        );
        assert!(
            pack.metrics_markdown.contains("project:PROJ"),
            "expected project throughput line, got:\n{}",
            pack.metrics_markdown
        );
    }
}
