//! Sync control commands and progress IPC.

use std::sync::Arc;

use ag_db::{migrate, open_db};
use ag_jira::ReqwestHttpDoer;
use ag_sync::SyncEngine;

use crate::state::{AppState, SyncProgress};

/// Start a full sync if one is not already running.
pub async fn start_full_sync_inner<F>(state: &AppState, on_progress: F) -> Result<(), String>
where
    F: Fn(SyncProgress) + Send + Sync + 'static,
{
    run_sync(state, SyncKind::Full, on_progress).await
}

/// Start an incremental sync if one is not already running.
pub async fn start_incremental_sync_inner<F>(state: &AppState, on_progress: F) -> Result<(), String>
where
    F: Fn(SyncProgress) + Send + Sync + 'static,
{
    run_sync(state, SyncKind::Incremental, on_progress).await
}

/// Latest progress snapshot.
pub fn get_sync_progress_inner(state: &AppState) -> Result<SyncProgress, String> {
    state.current_progress()
}

#[derive(Clone, Copy)]
enum SyncKind {
    Full,
    Incremental,
}

async fn run_sync<F>(state: &AppState, kind: SyncKind, on_progress: F) -> Result<(), String>
where
    F: Fn(SyncProgress) + Send + Sync + 'static,
{
    if state.is_running()? {
        return Err("sync already in progress".into());
    }

    let creds = state
        .credentials
        .load_jira()
        .map_err(|e| e.to_string())?
        .ok_or_else(|| "jira credentials not configured".to_string())?;

    if let Some(parent) = state.db_path.parent() {
        std::fs::create_dir_all(parent).map_err(|e| e.to_string())?;
    }

    state.set_running(true)?;

    let db_path = state.db_path.clone();
    let sync = Arc::clone(&state.sync);
    let on_progress = Arc::new(on_progress);

    // rusqlite::Connection is not Send; run the async SyncEngine on a
    // current-thread runtime inside the blocking pool.
    let result = tokio::task::spawn_blocking(move || {
        let rt = tokio::runtime::Builder::new_current_thread()
            .enable_all()
            .build()
            .map_err(|e| e.to_string())?;
        rt.block_on(async {
            let conn = open_db(&db_path).map_err(|e| e.to_string())?;
            migrate(&conn).map_err(|e| e.to_string())?;
            let http = ReqwestHttpDoer::new().map_err(|e| e.to_string())?;
            let mut engine = SyncEngine::new(&conn, &creds, http);

            let emit = |p: ag_sync::SyncProgress| {
                let dto = SyncProgress::from(p);
                if let Ok(mut guard) = sync.lock() {
                    guard.progress = dto.clone();
                }
                on_progress(dto);
            };

            match kind {
                SyncKind::Full => engine.run_full(emit).await.map_err(|e| e.to_string()),
                SyncKind::Incremental => engine
                    .run_incremental(emit)
                    .await
                    .map_err(|e| e.to_string()),
            }
        })
    })
    .await
    .map_err(|e| format!("sync worker join error: {e}"))?;

    state.set_running(false)?;
    result
}

#[cfg(feature = "desktop")]
mod tauri_cmds {
    use super::*;
    use tauri::{AppHandle, Emitter, State};

    #[tauri::command]
    pub async fn start_full_sync(app: AppHandle, state: State<'_, AppState>) -> Result<(), String> {
        start_full_sync_inner(&state, move |p| {
            let _ = app.emit("sync-progress", &p);
        })
        .await
    }

    #[tauri::command]
    pub async fn start_incremental_sync(
        app: AppHandle,
        state: State<'_, AppState>,
    ) -> Result<(), String> {
        start_incremental_sync_inner(&state, move |p| {
            let _ = app.emit("sync-progress", &p);
        })
        .await
    }

    #[tauri::command]
    pub fn get_sync_progress(state: State<'_, AppState>) -> Result<SyncProgress, String> {
        get_sync_progress_inner(&state)
    }
}

#[cfg(feature = "desktop")]
pub use tauri_cmds::{get_sync_progress, start_full_sync, start_incremental_sync};
