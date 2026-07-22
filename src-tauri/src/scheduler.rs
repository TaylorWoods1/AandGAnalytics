//! Background incremental sync scheduler (10-minute interval).

use std::sync::Arc;
use std::time::Duration;

use tokio::time::{interval, MissedTickBehavior};

use crate::commands::sync as sync_cmd;
use crate::state::AppState;

/// Interval between automatic incremental syncs.
pub const INCREMENTAL_INTERVAL: Duration = Duration::from_secs(10 * 60);

/// Spawn a Tokio task that runs incremental sync every [`INCREMENTAL_INTERVAL`].
///
/// On each tick, runs only when credentials exist, the DB file exists, and no sync
/// is already in progress. Progress updates go through `on_progress`.
pub fn spawn_incremental_scheduler<F>(state: Arc<AppState>, on_progress: F)
where
    F: Fn(crate::state::SyncProgress) + Send + Sync + 'static,
{
    let on_progress = Arc::new(on_progress);
    tokio::spawn(async move {
        let mut ticker = interval(INCREMENTAL_INTERVAL);
        ticker.set_missed_tick_behavior(MissedTickBehavior::Skip);
        // Consume the immediate first tick so the first run waits a full interval.
        ticker.tick().await;
        loop {
            ticker.tick().await;
            if !state.can_auto_sync() {
                continue;
            }
            if state.is_running().unwrap_or(true) {
                continue;
            }
            let cb = Arc::clone(&on_progress);
            let _ = sync_cmd::start_incremental_sync_inner(&state, move |p| cb(p)).await;
        }
    });
}
