//! Errors from the sync engine.

use thiserror::Error;

/// Errors returned by [`crate::SyncEngine`].
#[derive(Debug, Error)]
pub enum SyncError {
    #[error("database error: {0}")]
    Db(#[from] rusqlite::Error),

    #[error("jira error: {0}")]
    Jira(#[from] ag_jira::JiraError),

    #[error("sync interrupted: {0}")]
    Interrupted(String),

    #[error("sync paused")]
    Paused,

    #[error("sync error: {0}")]
    Other(String),
}
