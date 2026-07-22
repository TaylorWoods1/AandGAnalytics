//! Background incremental sync scheduler (10-minute interval).

use std::sync::Arc;
use std::time::Duration;

use tokio::time::{interval, MissedTickBehavior};

use crate::commands::sync as sync_cmd;
use crate::state::AppState;

/// Interval between automatic incremental syncs.
pub const INCREMENTAL_INTERVAL: Duration = Duration::from_secs(10 * 60);

/// Whether a scheduled tick should start an incremental sync.
pub fn should_run_scheduled_incremental(state: &AppState) -> bool {
    state.can_auto_sync() && !state.is_running().unwrap_or(true)
}

/// Async loop that runs incremental sync every `interval_duration`.
///
/// Always runs; each tick no-ops until credentials + DB are ready and no sync
/// is already in progress. Progress updates go through `on_progress`.
pub async fn run_incremental_scheduler_loop<F>(
    state: Arc<AppState>,
    interval_duration: Duration,
    on_progress: F,
) where
    F: Fn(crate::state::SyncProgress) + Send + Sync + 'static,
{
    let on_progress = Arc::new(on_progress);
    let mut ticker = interval(interval_duration);
    ticker.set_missed_tick_behavior(MissedTickBehavior::Skip);
    // Consume the immediate first tick so the first run waits a full interval.
    ticker.tick().await;
    loop {
        ticker.tick().await;
        if !should_run_scheduled_incremental(&state) {
            continue;
        }
        let cb = Arc::clone(&on_progress);
        let _ = sync_cmd::start_incremental_sync_inner(&state, move |p| cb(p)).await;
    }
}

/// Spawn a Tokio task that runs incremental sync every [`INCREMENTAL_INTERVAL`].
///
/// The task is started unconditionally; readiness is checked on each tick.
pub fn spawn_incremental_scheduler<F>(state: Arc<AppState>, on_progress: F)
where
    F: Fn(crate::state::SyncProgress) + Send + Sync + 'static,
{
    spawn_incremental_scheduler_every(state, INCREMENTAL_INTERVAL, on_progress);
}

/// Like [`spawn_incremental_scheduler`], but with a custom interval (tests).
pub fn spawn_incremental_scheduler_every<F>(
    state: Arc<AppState>,
    interval_duration: Duration,
    on_progress: F,
) where
    F: Fn(crate::state::SyncProgress) + Send + Sync + 'static,
{
    tokio::spawn(run_incremental_scheduler_loop(
        state,
        interval_duration,
        on_progress,
    ));
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::sync::atomic::{AtomicUsize, Ordering};

    use ag_credentials::{CredentialStore, JiraCredentials, MemoryCredentialStore};

    #[test]
    fn should_run_is_false_until_creds_and_db_exist() {
        let dir = tempfile::tempdir().unwrap();
        let db_path = dir.path().join("analytics.db");
        let state = AppState::memory_for_test(db_path.clone());
        assert!(!should_run_scheduled_incremental(&state));

        let store = MemoryCredentialStore::default();
        store
            .save_jira(&JiraCredentials {
                site_url: "https://example.atlassian.net".into(),
                email: "user@example.com".into(),
                api_token: "token".into(),
            })
            .unwrap();
        let state = AppState::with_credentials(db_path.clone(), Arc::new(store));
        assert!(!should_run_scheduled_incremental(&state));

        std::fs::write(&db_path, []).unwrap();
        assert!(should_run_scheduled_incremental(&state));

        state.set_running(true).unwrap();
        assert!(!should_run_scheduled_incremental(&state));
    }

    #[tokio::test]
    async fn spawn_scheduler_noop_ticks_until_configured() {
        let dir = tempfile::tempdir().unwrap();
        let db_path = dir.path().join("analytics.db");
        let state = Arc::new(AppState::memory_for_test(db_path));
        let calls = Arc::new(AtomicUsize::new(0));
        let calls_cb = Arc::clone(&calls);

        spawn_incremental_scheduler_every(
            Arc::clone(&state),
            Duration::from_millis(15),
            move |_| {
                calls_cb.fetch_add(1, Ordering::SeqCst);
            },
        );

        tokio::time::sleep(Duration::from_millis(80)).await;
        assert_eq!(
            calls.load(Ordering::SeqCst),
            0,
            "unconfigured app must not start incremental sync on scheduler ticks"
        );
        assert!(!state.can_auto_sync());
    }
}
