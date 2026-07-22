//! Full and incremental Jira → SQLite sync engine.

use ag_credentials::JiraCredentials;
use ag_jira::{HttpDoer, Issue, JiraClient, JiraField};
use chrono::Utc;
use rusqlite::{params, Connection};
use serde_json::Value;

use crate::checkpoint;
use crate::progress::{SyncPhase, SyncProgress};
use crate::SyncError;

const ISSUES_GLOBAL: &str = "issues:global";
const META_WATERMARK: &str = "last_incremental_watermark";
const FIELD_STORY_POINTS: &str = "story_points";
const FULL_JQL: &str = "order by updated asc";

/// Orchestrates resumable full and incremental sync into the local DB.
pub struct SyncEngine<'a, H: HttpDoer> {
    db: &'a Connection,
    jira: JiraClient<H>,
    paused: bool,
    /// Test hook: fail after this many issue pages (Some(n)).
    fail_after_issue_pages: Option<u32>,
    issues_synced: u64,
}

impl<'a, H: HttpDoer> SyncEngine<'a, H> {
    /// Build a sync engine over an open, migrated database, site credentials, and an [`HttpDoer`].
    pub fn new(db: &'a Connection, creds: &JiraCredentials, http: H) -> Self {
        Self {
            db,
            jira: JiraClient::with_http(creds, http),
            paused: false,
            fail_after_issue_pages: None,
            issues_synced: 0,
        }
    }

    /// Test hook: interrupt after `n` issue pages have been written + checkpointed.
    pub fn fail_after_issues(&mut self, n: u32) {
        self.fail_after_issue_pages = Some(n);
    }

    /// Clear the interrupt test hook.
    pub fn clear_fail_hook(&mut self) {
        self.fail_after_issue_pages = None;
    }

    /// Whether a checkpoint row exists for `scope_key`.
    pub fn checkpoint_exists(&self, scope_key: &str) -> bool {
        checkpoint::checkpoint_exists(self.db, scope_key).unwrap_or(false)
    }

    /// Request pause; in-flight page finishes, then [`SyncError::Paused`] is returned.
    pub fn pause(&mut self) {
        self.paused = true;
    }

    /// Resume a full sync from checkpoints (same as [`Self::run_full`]).
    pub async fn resume_full(
        &mut self,
        on_progress: impl Fn(SyncProgress),
    ) -> Result<(), SyncError> {
        self.paused = false;
        self.run_full(on_progress).await
    }

    /// Full sync: projects → issues (paginated) → sprints → derived stub → watermark.
    pub async fn run_full(
        &mut self,
        on_progress: impl Fn(SyncProgress),
    ) -> Result<(), SyncError> {
        self.issues_synced = 0;
        let result = self.run_full_inner(&on_progress).await;
        if let Err(ref err) = result {
            emit_failed(&on_progress, self.issues_synced, err);
        }
        result
    }

    async fn run_full_inner(
        &mut self,
        on_progress: &impl Fn(SyncProgress),
    ) -> Result<(), SyncError> {
        self.sync_story_points_field().await?;
        self.sync_projects(on_progress).await?;
        self.sync_issues(FULL_JQL, true, on_progress).await?;
        self.sync_sprints(on_progress).await?;
        self.rebuild_derived(on_progress)?;
        self.write_watermark()?;
        on_progress(SyncProgress {
            phase: SyncPhase::Idle,
            projects_done: 0,
            projects_total: 0,
            issues_synced: self.issues_synced,
            message: "full sync complete".into(),
        });
        Ok(())
    }

    /// Incremental sync: issues updated since `last_incremental_watermark`.
    pub async fn run_incremental(
        &mut self,
        on_progress: impl Fn(SyncProgress),
    ) -> Result<(), SyncError> {
        let result = self.run_incremental_inner(&on_progress).await;
        if let Err(ref err) = result {
            emit_failed(&on_progress, self.issues_synced, err);
        }
        result
    }

    async fn run_incremental_inner(
        &mut self,
        on_progress: &impl Fn(SyncProgress),
    ) -> Result<(), SyncError> {
        self.sync_story_points_field().await?;
        let watermark = checkpoint::get_meta(self.db, META_WATERMARK)?.unwrap_or_else(|| {
            // No prior sync — fall back to epoch so first incremental still works.
            "1970-01-01 00:00".into()
        });
        // Jira Cloud JQL datetime quoting.
        let jql = format!(r#"updated >= "{watermark}" order by updated asc"#);
        self.sync_issues(&jql, false, on_progress).await?;
        self.sync_sprints(on_progress).await?;
        self.rebuild_derived(on_progress)?;
        self.write_watermark()?;
        on_progress(SyncProgress {
            phase: SyncPhase::Idle,
            projects_done: 0,
            projects_total: 0,
            issues_synced: self.issues_synced,
            message: "incremental sync complete".into(),
        });
        Ok(())
    }

    async fn sync_story_points_field(&self) -> Result<(), SyncError> {
        let fields = self.jira.list_fields().await?;
        apply_story_points_mapping(self.db, &fields)
    }

    async fn sync_projects(&self, on_progress: &impl Fn(SyncProgress)) -> Result<(), SyncError> {
        on_progress(SyncProgress {
            phase: SyncPhase::Projects,
            projects_done: 0,
            projects_total: 0,
            issues_synced: self.issues_synced,
            message: "listing projects".into(),
        });
        let projects = self.jira.list_projects().await?;
        let total = projects.len() as u32;
        for (i, p) in projects.iter().enumerate() {
            let raw = serde_json::to_string(p).unwrap_or_else(|_| "{}".into());
            self.db.execute(
                "INSERT INTO projects (id, key, name, raw_json) VALUES (?1, ?2, ?3, ?4)
                 ON CONFLICT(id) DO UPDATE SET
                    key = excluded.key,
                    name = excluded.name,
                    raw_json = excluded.raw_json",
                params![p.id, p.key, p.name, raw],
            )?;
            on_progress(SyncProgress {
                phase: SyncPhase::Projects,
                projects_done: (i as u32) + 1,
                projects_total: total,
                issues_synced: self.issues_synced,
                message: format!("upserted project {}", p.key),
            });
        }
        Ok(())
    }

    async fn sync_issues(
        &mut self,
        jql: &str,
        resume_from_checkpoint: bool,
        on_progress: &impl Fn(SyncProgress),
    ) -> Result<(), SyncError> {
        on_progress(SyncProgress {
            phase: SyncPhase::Issues,
            projects_done: 0,
            projects_total: 0,
            issues_synced: self.issues_synced,
            message: format!("searching issues: {jql}"),
        });

        let mut next_token: Option<String> = None;
        let mut pages_done: u32 = 0;

        if resume_from_checkpoint {
            if let Some(cp) = checkpoint::load_checkpoint(self.db, ISSUES_GLOBAL)? {
                if cp.jql_cursor.as_deref() == Some(jql) || cp.jql_cursor.is_none() {
                    next_token = cp.next_page_token;
                }
            }
        }

        let story_points_field = resolved_story_points_field(self.db)?;

        loop {
            if self.paused {
                return Err(SyncError::Paused);
            }

            let page = self
                .jira
                .search_issues_page(jql, next_token.as_deref(), true)
                .await?;

            for issue in &page.issues {
                upsert_issue(self.db, issue, story_points_field.as_deref())?;
                self.issues_synced += 1;
            }

            pages_done += 1;
            let page_watermark = page
                .issues
                .iter()
                .filter_map(|i| issue_field_str(i, "updated"))
                .max();

            let is_last = page.is_last.unwrap_or(page.next_page_token.is_none());
            let token_to_store = if is_last {
                None
            } else {
                page.next_page_token.clone()
            };

            checkpoint::save_checkpoint(
                self.db,
                ISSUES_GLOBAL,
                token_to_store.as_deref(),
                Some(jql),
                page_watermark.as_deref(),
            )?;

            on_progress(SyncProgress {
                phase: SyncPhase::Issues,
                projects_done: 0,
                projects_total: 0,
                issues_synced: self.issues_synced,
                message: format!("synced issues page {pages_done}"),
            });

            if let Some(limit) = self.fail_after_issue_pages {
                if pages_done >= limit {
                    return Err(SyncError::Interrupted(format!(
                        "fail_after_issues({limit})"
                    )));
                }
            }

            if is_last {
                checkpoint::clear_page_token(self.db, ISSUES_GLOBAL)?;
                break;
            }
            next_token = page.next_page_token;
            if next_token.is_none() {
                break;
            }
        }

        Ok(())
    }

    async fn sync_sprints(&self, on_progress: &impl Fn(SyncProgress)) -> Result<(), SyncError> {
        on_progress(SyncProgress {
            phase: SyncPhase::Sprints,
            projects_done: 0,
            projects_total: 0,
            issues_synced: self.issues_synced,
            message: "syncing boards/sprints".into(),
        });

        let mut board_start = 0u64;
        loop {
            let boards = self.jira.list_boards_page(board_start).await?;
            for board in &boards.values {
                let mut sprint_start = 0u64;
                loop {
                    let sprints = self
                        .jira
                        .list_board_sprints_page(board.id, sprint_start)
                        .await?;
                    for sprint in &sprints.values {
                        upsert_sprint(self.db, sprint, board.id)?;
                        sync_sprint_issues(self, sprint.id).await?;
                    }
                    if sprints.is_last.unwrap_or(true) || sprints.values.is_empty() {
                        break;
                    }
                    sprint_start += sprints.values.len() as u64;
                }
            }
            if boards.is_last.unwrap_or(true) || boards.values.is_empty() {
                break;
            }
            board_start += boards.values.len() as u64;
        }
        Ok(())
    }

    async fn list_and_link_sprint_issues(&self, sprint_id: i64) -> Result<(), SyncError> {
        let mut start = 0u64;
        loop {
            let page = self.jira.list_sprint_issues_page(sprint_id, start).await?;
            for issue in &page.issues {
                // Only link issues already present (FK). Skip unknowns.
                let exists: i64 = self.db.query_row(
                    "SELECT COUNT(*) FROM issues WHERE id = ?1",
                    params![issue.id],
                    |r| r.get(0),
                )?;
                if exists == 0 {
                    continue;
                }
                self.db.execute(
                    "INSERT OR IGNORE INTO sprint_issues (sprint_id, issue_id) VALUES (?1, ?2)",
                    params![sprint_id.to_string(), issue.id],
                )?;
            }
            if page.is_last.unwrap_or(true) || page.issues.is_empty() {
                break;
            }
            start += page.issues.len() as u64;
        }
        Ok(())
    }

    fn rebuild_derived(&self, on_progress: &impl Fn(SyncProgress)) -> Result<(), SyncError> {
        on_progress(SyncProgress {
            phase: SyncPhase::Derived,
            projects_done: 0,
            projects_total: 0,
            issues_synced: self.issues_synced,
            message: "rebuilding derived analytics".into(),
        });
        ag_analytics::rebuild_all_derived(self.db, Utc::now())
            .map_err(|e| SyncError::Other(e.to_string()))?;
        on_progress(SyncProgress {
            phase: SyncPhase::Derived,
            projects_done: 0,
            projects_total: 0,
            issues_synced: self.issues_synced,
            message: "derived analytics rebuilt".into(),
        });
        Ok(())
    }

    fn write_watermark(&self) -> Result<(), SyncError> {
        let max_updated: Option<String> = self
            .db
            .query_row("SELECT MAX(updated) FROM issues", [], |r| r.get(0))
            .ok()
            .flatten();
        if let Some(wm) = max_updated {
            // Store in a JQL-friendly form (trim timezone offset if present).
            let jql_wm = jira_jql_datetime(&wm);
            checkpoint::set_meta(self.db, META_WATERMARK, &jql_wm)?;
            checkpoint::save_checkpoint(
                self.db,
                ISSUES_GLOBAL,
                None,
                None,
                Some(wm.as_str()),
            )?;
        }
        Ok(())
    }
}

async fn sync_sprint_issues<H: HttpDoer>(
    engine: &SyncEngine<'_, H>,
    sprint_id: i64,
) -> Result<(), SyncError> {
    engine.list_and_link_sprint_issues(sprint_id).await
}

fn apply_story_points_mapping(conn: &Connection, fields: &[JiraField]) -> Result<(), SyncError> {
    let mut candidates: Vec<&JiraField> = fields
        .iter()
        .filter(|f| {
            let name = f.name.to_lowercase();
            name.contains("story point")
                || name == "story points"
                || f.id == "customfield_10016"
                || f.schema
                    .as_ref()
                    .and_then(|s| s.custom.as_deref())
                    .is_some_and(|c| c.contains("story-points") || c.contains("storypoints"))
        })
        .collect();

    // Deduplicate by id.
    candidates.sort_by(|a, b| a.id.cmp(&b.id));
    candidates.dedup_by(|a, b| a.id == b.id);

    match candidates.as_slice() {
        [] => {
            conn.execute(
                "INSERT INTO field_map (logical_name, jira_field_id, jira_field_name, status)
                 VALUES (?1, NULL, NULL, 'unresolved')
                 ON CONFLICT(logical_name) DO UPDATE SET status = 'unresolved'",
                params![FIELD_STORY_POINTS],
            )?;
        }
        [one] => {
            conn.execute(
                "INSERT INTO field_map (logical_name, jira_field_id, jira_field_name, status)
                 VALUES (?1, ?2, ?3, 'resolved')
                 ON CONFLICT(logical_name) DO UPDATE SET
                    jira_field_id = excluded.jira_field_id,
                    jira_field_name = excluded.jira_field_name,
                    status = 'resolved'",
                params![FIELD_STORY_POINTS, one.id, one.name],
            )?;
        }
        many => {
            // Ambiguous — leave unresolved for UI confirmation (Task 10).
            let names: String = many
                .iter()
                .map(|f| f.name.as_str())
                .collect::<Vec<_>>()
                .join(", ");
            conn.execute(
                "INSERT INTO field_map (logical_name, jira_field_id, jira_field_name, status)
                 VALUES (?1, NULL, ?2, 'unresolved')
                 ON CONFLICT(logical_name) DO UPDATE SET
                    jira_field_id = NULL,
                    jira_field_name = excluded.jira_field_name,
                    status = 'unresolved'",
                params![FIELD_STORY_POINTS, names],
            )?;
        }
    }
    Ok(())
}

fn resolved_story_points_field(conn: &Connection) -> Result<Option<String>, SyncError> {
    let row: Option<(Option<String>, String)> = conn
        .query_row(
            "SELECT jira_field_id, status FROM field_map WHERE logical_name = ?1",
            params![FIELD_STORY_POINTS],
            |r| Ok((r.get(0)?, r.get(1)?)),
        )
        .optional_row()?;
    Ok(match row {
        Some((Some(id), status)) if status == "resolved" => Some(id),
        _ => None,
    })
}

trait OptionalRow<T> {
    fn optional_row(self) -> Result<Option<T>, SyncError>;
}

impl<T> OptionalRow<T> for Result<T, rusqlite::Error> {
    fn optional_row(self) -> Result<Option<T>, SyncError> {
        match self {
            Ok(v) => Ok(Some(v)),
            Err(rusqlite::Error::QueryReturnedNoRows) => Ok(None),
            Err(e) => Err(SyncError::Db(e)),
        }
    }
}

fn upsert_issue(
    conn: &Connection,
    issue: &Issue,
    story_points_field: Option<&str>,
) -> Result<(), SyncError> {
    let fields = issue.fields.as_ref();
    let project_key = fields
        .and_then(|f| f.get("project"))
        .and_then(|p| p.get("key"))
        .and_then(|k| k.as_str())
        .unwrap_or("UNKNOWN")
        .to_string();
    let summary = text_field(fields, "summary");
    let issue_type = fields
        .and_then(|f| f.get("issuetype"))
        .and_then(|t| t.get("name"))
        .and_then(|n| n.as_str())
        .map(|s| s.to_string());
    let status = fields
        .and_then(|f| f.get("status"))
        .and_then(|t| t.get("name"))
        .and_then(|n| n.as_str())
        .map(|s| s.to_string());
    let status_category = fields
        .and_then(|f| f.get("status"))
        .and_then(|t| t.get("statusCategory"))
        .and_then(|c| c.get("key").or_else(|| c.get("name")))
        .and_then(|n| n.as_str())
        .map(|s| s.to_string());
    let assignee = fields
        .and_then(|f| f.get("assignee"))
        .and_then(|a| a.get("accountId"))
        .and_then(|n| n.as_str())
        .map(|s| s.to_string());
    let reporter = fields
        .and_then(|f| f.get("reporter"))
        .and_then(|a| a.get("accountId"))
        .and_then(|n| n.as_str())
        .map(|s| s.to_string());
    let created = issue_field_str(issue, "created")
        .or_else(|| issue_field_str(issue, "updated"))
        .unwrap_or_else(|| Utc::now().to_rfc3339());
    let updated = issue_field_str(issue, "updated").unwrap_or_else(|| created.clone());
    let resolved = fields
        .and_then(|f| f.get("resolutiondate"))
        .and_then(|v| v.as_str())
        .map(|s| s.to_string());
    let parent_key = fields
        .and_then(|f| f.get("parent"))
        .and_then(|p| p.get("key"))
        .and_then(|k| k.as_str())
        .map(|s| s.to_string());
    let story_points = story_points_field.and_then(|fid| {
        fields
            .and_then(|f| f.get(fid))
            .and_then(|v| v.as_f64().or_else(|| v.as_i64().map(|i| i as f64)))
    });
    let raw = serde_json::to_string(issue).unwrap_or_else(|_| "{}".into());

    conn.execute(
        "INSERT INTO issues (
            id, key, project_key, summary, issue_type, status, status_category,
            assignee_account_id, reporter_account_id, story_points, parent_key, epic_key,
            created, updated, resolved, raw_json
         ) VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8, ?9, ?10, ?11, NULL, ?12, ?13, ?14, ?15)
         ON CONFLICT(id) DO UPDATE SET
            key = excluded.key,
            project_key = excluded.project_key,
            summary = excluded.summary,
            issue_type = excluded.issue_type,
            status = excluded.status,
            status_category = excluded.status_category,
            assignee_account_id = excluded.assignee_account_id,
            reporter_account_id = excluded.reporter_account_id,
            story_points = excluded.story_points,
            parent_key = excluded.parent_key,
            created = excluded.created,
            updated = excluded.updated,
            resolved = excluded.resolved,
            raw_json = excluded.raw_json",
        params![
            issue.id,
            issue.key,
            project_key,
            summary,
            issue_type,
            status,
            status_category,
            assignee,
            reporter,
            story_points,
            parent_key,
            created,
            updated,
            resolved,
            raw,
        ],
    )?;

    // Replace changelog rows for this issue.
    conn.execute(
        "DELETE FROM issue_changelog WHERE issue_id = ?1",
        params![issue.id],
    )?;
    if let Some(changelog) = &issue.changelog {
        for hist in &changelog.histories {
            let created = hist
                .created
                .clone()
                .unwrap_or_else(|| Utc::now().to_rfc3339());
            let changelog_id = hist.id.clone();
            for item in &hist.items {
                let field = item.field.clone().unwrap_or_else(|| "unknown".into());
                conn.execute(
                    "INSERT INTO issue_changelog (
                        issue_id, changelog_id, field, from_value, to_value,
                        from_string, to_string, author_account_id, created
                     ) VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, NULL, ?8)",
                    params![
                        issue.id,
                        changelog_id,
                        field,
                        item.from,
                        item.to,
                        item.from_string,
                        item.to_string,
                        created,
                    ],
                )?;
            }
        }
    }

    Ok(())
}

fn upsert_sprint(
    conn: &Connection,
    sprint: &ag_jira::Sprint,
    board_id: i64,
) -> Result<(), SyncError> {
    let raw = serde_json::to_string(sprint).unwrap_or_else(|_| "{}".into());
    conn.execute(
        "INSERT INTO sprints (
            id, name, state, board_id, start_date, end_date, complete_date, goal, raw_json
         ) VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8, ?9)
         ON CONFLICT(id) DO UPDATE SET
            name = excluded.name,
            state = excluded.state,
            board_id = excluded.board_id,
            start_date = excluded.start_date,
            end_date = excluded.end_date,
            complete_date = excluded.complete_date,
            goal = excluded.goal,
            raw_json = excluded.raw_json",
        params![
            sprint.id.to_string(),
            sprint.name,
            sprint.state,
            board_id.to_string(),
            sprint.start_date,
            sprint.end_date,
            sprint.complete_date,
            sprint.goal,
            raw,
        ],
    )?;
    Ok(())
}

fn text_field(fields: Option<&Value>, key: &str) -> Option<String> {
    fields
        .and_then(|f| f.get(key))
        .and_then(|v| v.as_str())
        .map(|s| s.to_string())
}

fn issue_field_str(issue: &Issue, key: &str) -> Option<String> {
    issue
        .fields
        .as_ref()
        .and_then(|f| f.get(key))
        .and_then(|v| v.as_str())
        .map(|s| s.to_string())
}

/// Convert Jira issue datetime to a form usable in JQL `updated >= "..."`.
fn jira_jql_datetime(raw: &str) -> String {
    // "2024-01-03T10:00:00.000+0000" → "2024-01-03 10:00"
    let s = raw.replace('T', " ");
    if s.len() >= 16 {
        s[..16].to_string()
    } else {
        s
    }
}

fn emit_failed(on_progress: &impl Fn(SyncProgress), issues_synced: u64, err: &SyncError) {
    on_progress(SyncProgress {
        phase: SyncPhase::Failed,
        projects_done: 0,
        projects_total: 0,
        issues_synced,
        message: err.to_string(),
    });
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::fake::FakeJira;
    use ag_db::{migrate, open_db};
    use tempfile::tempdir;

    fn test_creds() -> JiraCredentials {
        JiraCredentials {
            site_url: "https://fake.atlassian.net".into(),
            email: "dev@example.com".into(),
            api_token: "token".into(),
        }
    }

    fn open_temp_migrated_db() -> (tempfile::TempDir, Connection) {
        let dir = tempdir().unwrap();
        let path = dir.path().join("sync.db");
        let conn = open_db(&path).unwrap();
        migrate(&conn).unwrap();
        (dir, conn)
    }

    #[tokio::test]
    async fn full_sync_is_resumable_after_interrupt() {
        let (_dir, db) = open_temp_migrated_db();
        let jira = FakeJira::from_fixtures_two_pages();
        let mut engine = SyncEngine::new(&db, &test_creds(), jira);

        engine.fail_after_issues(1); // test hook: stop after first page
        let err = engine.run_full(|_| {}).await;
        assert!(err.is_err() || engine.checkpoint_exists("issues:global"));

        engine.clear_fail_hook();
        engine.run_full(|_| {}).await.unwrap();

        let count: i64 = db
            .query_row("SELECT COUNT(*) FROM issues", [], |r| r.get(0))
            .unwrap();
        assert_eq!(count, 3); // fixture issue count
    }

    #[tokio::test]
    async fn incremental_sync_only_fetches_updated_since_watermark() {
        let (_dir, db) = open_temp_migrated_db();
        let jira = FakeJira::from_fixtures_two_pages();
        let mut engine = SyncEngine::new(&db, &test_creds(), jira.clone());

        engine.run_full(|_| {}).await.unwrap();

        let watermark: String = db
            .query_row(
                "SELECT value FROM meta WHERE key = 'last_incremental_watermark'",
                [],
                |r| r.get(0),
            )
            .unwrap();
        assert!(!watermark.is_empty());

        let before = jira.search_call_count();
        engine.run_incremental(|_| {}).await.unwrap();
        assert!(jira.search_call_count() > before);

        let bodies = jira.search_bodies();
        let last = bodies.last().expect("incremental search body");
        assert!(
            last.contains("updated >=") && last.contains(&watermark),
            "expected incremental JQL with watermark, got: {last}"
        );
    }

    #[tokio::test]
    async fn full_sync_persists_story_points_field_map() {
        let (_dir, db) = open_temp_migrated_db();
        let jira = FakeJira::from_fixtures_two_pages();
        let mut engine = SyncEngine::new(&db, &test_creds(), jira);
        engine.run_full(|_| {}).await.unwrap();

        let (id, status): (String, String) = db
            .query_row(
                "SELECT jira_field_id, status FROM field_map WHERE logical_name = 'story_points'",
                [],
                |r| Ok((r.get(0)?, r.get(1)?)),
            )
            .unwrap();
        assert_eq!(id, "customfield_10016");
        assert_eq!(status, "resolved");
    }

    #[tokio::test]
    async fn full_sync_emits_failed_phase_on_interrupt() {
        use std::sync::atomic::{AtomicBool, Ordering};

        let (_dir, db) = open_temp_migrated_db();
        let jira = FakeJira::from_fixtures_two_pages();
        let mut engine = SyncEngine::new(&db, &test_creds(), jira);
        engine.fail_after_issues(1);

        let saw_failed = AtomicBool::new(false);
        let err = engine
            .run_full(|p| {
                if p.phase == SyncPhase::Failed {
                    saw_failed.store(true, Ordering::SeqCst);
                }
            })
            .await;
        assert!(err.is_err());
        assert!(saw_failed.load(Ordering::SeqCst));
    }
}
