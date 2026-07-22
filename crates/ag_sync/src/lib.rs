//! Sync orchestration between Jira and local storage for AandG Analytics.

mod checkpoint;
mod engine;
mod error;
mod progress;

#[cfg(test)]
mod fake;

pub use engine::SyncEngine;
pub use error::SyncError;
pub use progress::{SyncPhase, SyncProgress};

#[cfg(test)]
mod tests {
    #[test]
    fn workspace_compiles_and_sync_crate_is_wired() {
        assert_eq!(env!("CARGO_PKG_NAME"), "ag_sync");
    }
}
