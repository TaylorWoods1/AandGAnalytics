//! Analytics errors.

use thiserror::Error;

/// Error type for analytics rebuild and flow derivation.
#[derive(Debug, Error)]
pub enum AnalyticsError {
    #[error("database error: {0}")]
    Db(#[from] rusqlite::Error),
    #[error("analytics error: {0}")]
    Other(String),
}
