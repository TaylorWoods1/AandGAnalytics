//! Risk crate errors.

use thiserror::Error;

/// Error type for epic risk rebuild and forecasting.
#[derive(Debug, Error)]
pub enum RiskError {
    #[error("database error: {0}")]
    Db(#[from] rusqlite::Error),
    #[error("risk error: {0}")]
    Other(String),
}
