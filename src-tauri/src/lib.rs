//! Tauri application shell for AandG Analytics.
//!
//! Command logic is library-testable via `*_inner` helpers. The full webview app
//! is compiled behind the `desktop` feature (`--features desktop`).

pub mod commands;
pub mod scheduler;
pub mod state;

pub use commands::MetricsFilter;
pub use state::{default_db_path, AppState, SyncHandle, SyncProgress};

pub fn app_name() -> &'static str {
    "AandG Analytics"
}

/// Run the Tauri desktop application (requires `--features desktop`).
#[cfg(feature = "desktop")]
pub fn run() {
    use std::sync::Arc;

    tauri::Builder::default()
        .manage(Arc::new(AppState::production(default_db_path())))
        .setup(|app| {
            use tauri::{Emitter, Manager};

            // Always spawn the scheduler at launch. Ticks no-op until
            // `can_auto_sync()` (credentials + DB) so first-run `save_setup`
            // in this session is picked up without restart.
            let handle = app.handle().clone();
            let state = Arc::clone(app.state::<Arc<AppState>>().inner());
            tauri::async_runtime::spawn(scheduler::run_incremental_scheduler_loop(
                state,
                scheduler::INCREMENTAL_INTERVAL,
                move |p| {
                    let _ = handle.emit("sync-progress", &p);
                },
            ));
            Ok(())
        })
        // Paths must point at the modules that own `#[tauri::command]` so the
        // generated `__cmd__*` companions resolve (re-exports are not enough).
        .invoke_handler(tauri::generate_handler![
            commands::setup::tauri_cmds::save_setup,
            commands::setup::tauri_cmds::validate_setup,
            commands::setup::tauri_cmds::get_story_points_mapping,
            commands::setup::tauri_cmds::set_story_points_mapping,
            commands::sync::tauri_cmds::start_full_sync,
            commands::sync::tauri_cmds::start_incremental_sync,
            commands::sync::tauri_cmds::get_sync_progress,
            commands::metrics::tauri_cmds::get_flow_metrics,
            commands::metrics::tauri_cmds::get_sprint_metrics,
            commands::metrics::tauri_cmds::get_epic_risk,
            commands::metrics::tauri_cmds::get_finish_by,
            commands::metrics::tauri_cmds::list_issues,
            commands::ai::tauri_cmds::preview_context_pack,
            commands::ai::tauri_cmds::ask_ai,
            commands::ai::tauri_cmds::get_suggested_prompts,
            commands::maintenance::tauri_cmds::rebuild_derived,
            commands::maintenance::tauri_cmds::full_resync,
        ])
        .run(tauri::generate_context!())
        .expect("error while running AandG Analytics");
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn app_name_is_wired() {
        assert_eq!(app_name(), "AandG Analytics");
    }
}
