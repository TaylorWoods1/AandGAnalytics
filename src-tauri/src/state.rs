//! Shared application state for Tauri commands and the sync scheduler.

use std::path::{Path, PathBuf};
use std::sync::{Arc, Mutex};

use ag_credentials::{CredentialStore, KeychainCredentialStore, MemoryCredentialStore};
use serde::{Deserialize, Serialize};

/// Serializable sync progress snapshot for IPC / events.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct SyncProgress {
    pub phase: String,
    pub projects_done: u32,
    pub projects_total: u32,
    pub issues_synced: u64,
    pub message: String,
}

impl Default for SyncProgress {
    fn default() -> Self {
        Self {
            phase: "Idle".into(),
            projects_done: 0,
            projects_total: 0,
            issues_synced: 0,
            message: String::new(),
        }
    }
}

impl From<ag_sync::SyncProgress> for SyncProgress {
    fn from(p: ag_sync::SyncProgress) -> Self {
        Self {
            phase: phase_name(p.phase).into(),
            projects_done: p.projects_done,
            projects_total: p.projects_total,
            issues_synced: p.issues_synced,
            message: p.message,
        }
    }
}

fn phase_name(phase: ag_sync::SyncPhase) -> &'static str {
    match phase {
        ag_sync::SyncPhase::Projects => "Projects",
        ag_sync::SyncPhase::Issues => "Issues",
        ag_sync::SyncPhase::Sprints => "Sprints",
        ag_sync::SyncPhase::Derived => "Derived",
        ag_sync::SyncPhase::Idle => "Idle",
        ag_sync::SyncPhase::Failed => "Failed",
    }
}

/// Tracks in-flight sync and the latest progress snapshot.
#[derive(Debug, Default)]
pub struct SyncHandle {
    pub progress: SyncProgress,
    pub running: bool,
}

/// Process-wide state injected into Tauri commands.
///
/// `sync` is an [`Arc<Mutex<_>>`] so progress can be updated from the blocking
/// sync worker (rusqlite connections are not `Send`).
pub struct AppState {
    pub db_path: PathBuf,
    pub credentials: Arc<dyn CredentialStore>,
    pub sync: Arc<Mutex<SyncHandle>>,
}

impl AppState {
    /// Production state: OS keychain credentials + given DB path.
    pub fn production(db_path: PathBuf) -> Self {
        Self {
            db_path,
            credentials: Arc::new(KeychainCredentialStore::new()),
            sync: Arc::new(Mutex::new(SyncHandle::default())),
        }
    }

    /// Test / harness state with an injectable credential store (typically [`MemoryCredentialStore`]).
    pub fn with_credentials(db_path: PathBuf, credentials: Arc<dyn CredentialStore>) -> Self {
        Self {
            db_path,
            credentials,
            sync: Arc::new(Mutex::new(SyncHandle::default())),
        }
    }

    /// Convenience for unit tests.
    pub fn memory_for_test(db_path: PathBuf) -> Self {
        Self::with_credentials(db_path, Arc::new(MemoryCredentialStore::default()))
    }

    /// True when Jira credentials exist and the DB file is present (scheduler gate).
    pub fn can_auto_sync(&self) -> bool {
        let has_jira = self.credentials.load_jira().ok().flatten().is_some();
        has_jira && Path::new(&self.db_path).is_file()
    }

    pub fn update_progress(&self, progress: SyncProgress) -> Result<(), String> {
        let mut guard = self
            .sync
            .lock()
            .map_err(|_| "sync state lock poisoned".to_string())?;
        guard.progress = progress;
        Ok(())
    }

    pub fn set_running(&self, running: bool) -> Result<(), String> {
        let mut guard = self
            .sync
            .lock()
            .map_err(|_| "sync state lock poisoned".to_string())?;
        guard.running = running;
        Ok(())
    }

    pub fn is_running(&self) -> Result<bool, String> {
        let guard = self
            .sync
            .lock()
            .map_err(|_| "sync state lock poisoned".to_string())?;
        Ok(guard.running)
    }

    pub fn current_progress(&self) -> Result<SyncProgress, String> {
        let guard = self
            .sync
            .lock()
            .map_err(|_| "sync state lock poisoned".to_string())?;
        Ok(guard.progress.clone())
    }
}

/// Default on-disk DB location under the user data directory.
pub fn default_db_path() -> PathBuf {
    let base = dirs::data_dir().unwrap_or_else(|| PathBuf::from("."));
    base.join("aandg-analytics").join("analytics.db")
}
