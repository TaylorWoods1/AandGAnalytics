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
    tauri::Builder::default()
        .manage(AppState::production(default_db_path()))
        .setup(|app| {
            use tauri::{Emitter, Manager};

            let handle = app.handle().clone();
            let state = handle.state::<AppState>();
            if state.can_auto_sync() {
                let handle_for_sched = handle.clone();
                tauri::async_runtime::spawn(async move {
                    use tokio::time::{interval, MissedTickBehavior};

                    let mut ticker = interval(scheduler::INCREMENTAL_INTERVAL);
                    ticker.set_missed_tick_behavior(MissedTickBehavior::Skip);
                    // Skip the immediate first tick so the first auto-sync waits 10 minutes.
                    ticker.tick().await;
                    loop {
                        ticker.tick().await;
                        let state = handle_for_sched.state::<AppState>();
                        if !state.can_auto_sync() || state.is_running().unwrap_or(true) {
                            continue;
                        }
                        let app2 = handle_for_sched.clone();
                        let _ = commands::start_incremental_sync_inner(&state, move |p| {
                            let _ = app2.emit("sync-progress", &p);
                        })
                        .await;
                    }
                });
            }
            Ok(())
        })
        .invoke_handler(tauri::generate_handler![
            commands::save_setup,
            commands::validate_setup,
            commands::start_full_sync,
            commands::start_incremental_sync,
            commands::get_sync_progress,
            commands::get_flow_metrics,
            commands::get_sprint_metrics,
            commands::get_epic_risk,
            commands::get_finish_by,
            commands::list_issues,
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
