//! Maintenance actions: rebuild derived analytics and full re-sync.

use chrono::{DateTime, Utc};
use rusqlite::Connection;

use ag_db::{migrate, open_db};

use crate::commands::sync::start_full_sync_inner;
use crate::state::{AppState, SyncProgress};

const META_WATERMARK: &str = "last_incremental_watermark";

/// Rebuild all derived tables from raw SQLite rows. Never deletes raw issues.
pub fn rebuild_derived(conn: &Connection, now: DateTime<Utc>) -> Result<(), String> {
    ag_analytics::rebuild_all_derived(conn, now).map_err(|e| e.to_string())
}

/// Clear sync checkpoints and incremental watermark. Credentials are untouched.
pub fn reset_sync_checkpoints(conn: &Connection) -> Result<(), String> {
    conn.execute("DELETE FROM sync_checkpoints", [])
        .map_err(|e| e.to_string())?;
    conn.execute("DELETE FROM meta WHERE key = ?1", [META_WATERMARK])
        .map_err(|e| e.to_string())?;
    Ok(())
}

/// Remove local issues and dependent rows so a full re-sync cannot leave orphans.
///
/// Keeps credentials (keychain), `projects`, `field_map`, and non-watermark `meta`.
pub fn clear_raw_issue_data(conn: &Connection) -> Result<(), String> {
    conn.execute_batch(
        "DELETE FROM issue_changelog;
         DELETE FROM sprint_issues;
         DELETE FROM worklogs;
         DELETE FROM issue_links;
         DELETE FROM derived_time_in_status;
         DELETE FROM derived_issue_cycle;
         DELETE FROM derived_throughput_daily;
         DELETE FROM derived_sprint_metrics;
         DELETE FROM derived_epic_risk;
         DELETE FROM issues;
         DELETE FROM sprints;",
    )
    .map_err(|e| e.to_string())?;
    Ok(())
}

/// Open the app DB and rebuild derived analytics from raw data.
pub fn rebuild_derived_inner(state: &AppState) -> Result<(), String> {
    if state.is_running()? {
        return Err("sync already in progress".into());
    }
    if let Some(parent) = state.db_path.parent() {
        std::fs::create_dir_all(parent).map_err(|e| e.to_string())?;
    }
    let conn = open_db(&state.db_path).map_err(|e| e.to_string())?;
    migrate(&conn).map_err(|e| e.to_string())?;
    rebuild_derived(&conn, Utc::now())
}

/// Reset checkpoints (keep credentials) and start a full sync from scratch.
pub async fn full_resync_inner<F>(state: &AppState, on_progress: F) -> Result<(), String>
where
    F: Fn(SyncProgress) + Send + Sync + 'static,
{
    if state.is_running()? {
        return Err("sync already in progress".into());
    }
    // Ensure credentials still exist — full resync must not wipe the keychain.
    let _ = state
        .credentials
        .load_jira()
        .map_err(|e| e.to_string())?
        .ok_or_else(|| "jira credentials not configured".to_string())?;

    if let Some(parent) = state.db_path.parent() {
        std::fs::create_dir_all(parent).map_err(|e| e.to_string())?;
    }
    {
        let conn = open_db(&state.db_path).map_err(|e| e.to_string())?;
        migrate(&conn).map_err(|e| e.to_string())?;
        reset_sync_checkpoints(&conn)?;
        // Drop local issue graph so deleted Jira issues cannot inflate metrics.
        clear_raw_issue_data(&conn)?;
    }

    start_full_sync_inner(state, on_progress).await
}

#[cfg(feature = "desktop")]
pub mod tauri_cmds {
    use super::*;
    use tauri::{AppHandle, Emitter, State};

    #[tauri::command]
    pub fn rebuild_derived(state: State<'_, std::sync::Arc<AppState>>) -> Result<(), String> {
        rebuild_derived_inner(&state)
    }

    #[tauri::command]
    pub async fn full_resync(
        app: AppHandle,
        state: State<'_, std::sync::Arc<AppState>>,
    ) -> Result<(), String> {
        full_resync_inner(&state, move |p| {
            let _ = app.emit("sync-progress", &p);
        })
        .await
    }
}

#[cfg(feature = "desktop")]
pub use tauri_cmds::{full_resync, rebuild_derived as rebuild_derived_cmd};

#[cfg(test)]
mod tests {
    use super::*;
    use ag_db::{migrate, open_db};
    use rusqlite::params;
    use tempfile::tempdir;

    fn db_with_one_issue_and_stale_derived() -> (tempfile::TempDir, Connection) {
        let dir = tempdir().unwrap();
        let path = dir.path().join("maint.db");
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
        // Stale derived row that should be replaced, not cause raw deletion.
        conn.execute(
            "INSERT INTO derived_issue_cycle (issue_id, cycle_secs, lead_secs, completed_at)
             VALUES ('1', 999, 999, '2020-01-02T00:00:00Z')",
            [],
        )
        .unwrap();
        conn.execute(
            "INSERT INTO sync_checkpoints (
                scope_key, next_page_token, jql_cursor, last_updated_watermark, updated_at
             ) VALUES ('issues:global', 'tok', NULL, '2024-01-02', '2024-01-02T12:00:00Z')",
            [],
        )
        .unwrap();
        conn.execute(
            "INSERT INTO meta (key, value) VALUES (?1, '2024-01-02 11:00')",
            params![META_WATERMARK],
        )
        .unwrap();
        (dir, conn)
    }

    #[test]
    fn rebuild_derived_keeps_raw_issue_rows() {
        let (_dir, db) = db_with_one_issue_and_stale_derived();
        rebuild_derived(&db, Utc::now()).unwrap();
        let n: i64 = db
            .query_row("SELECT COUNT(*) FROM issues", [], |r| r.get(0))
            .unwrap();
        assert_eq!(n, 1);
    }

    #[test]
    fn reset_sync_checkpoints_clears_watermark_keeps_issues() {
        let (_dir, db) = db_with_one_issue_and_stale_derived();
        reset_sync_checkpoints(&db).unwrap();
        let checkpoints: i64 = db
            .query_row("SELECT COUNT(*) FROM sync_checkpoints", [], |r| r.get(0))
            .unwrap();
        assert_eq!(checkpoints, 0);
        let watermark: i64 = db
            .query_row(
                "SELECT COUNT(*) FROM meta WHERE key = ?1",
                params![META_WATERMARK],
                |r| r.get(0),
            )
            .unwrap();
        assert_eq!(watermark, 0);
        let issues: i64 = db
            .query_row("SELECT COUNT(*) FROM issues", [], |r| r.get(0))
            .unwrap();
        assert_eq!(issues, 1);
    }

    #[test]
    fn clear_raw_issue_data_removes_issues_keeps_field_map() {
        let (_dir, db) = db_with_one_issue_and_stale_derived();
        db.execute(
            "INSERT INTO field_map (logical_name, jira_field_id, jira_field_name, status)
             VALUES ('story_points', 'customfield_10016', 'Story Points', 'resolved')",
            [],
        )
        .unwrap();
        db.execute(
            "INSERT INTO projects (id, key, name) VALUES ('1', 'DEMO', 'Demo')",
            [],
        )
        .unwrap();

        clear_raw_issue_data(&db).unwrap();

        let issues: i64 = db
            .query_row("SELECT COUNT(*) FROM issues", [], |r| r.get(0))
            .unwrap();
        let derived: i64 = db
            .query_row("SELECT COUNT(*) FROM derived_issue_cycle", [], |r| r.get(0))
            .unwrap();
        let field_maps: i64 = db
            .query_row("SELECT COUNT(*) FROM field_map", [], |r| r.get(0))
            .unwrap();
        let projects: i64 = db
            .query_row("SELECT COUNT(*) FROM projects", [], |r| r.get(0))
            .unwrap();
        assert_eq!(issues, 0);
        assert_eq!(derived, 0);
        assert_eq!(field_maps, 1);
        assert_eq!(projects, 1);
    }
}
