//! Sync progress events emitted to UI / callers.

/// High-level phase of a sync run.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum SyncPhase {
    Projects,
    Issues,
    Sprints,
    Derived,
    Idle,
    Failed,
}

/// Progress snapshot for a sync run.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct SyncProgress {
    pub phase: SyncPhase,
    pub projects_done: u32,
    pub projects_total: u32,
    pub issues_synced: u64,
    pub message: String,
}
