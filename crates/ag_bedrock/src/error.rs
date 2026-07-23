//! Errors for Bedrock client and context pack building.

use thiserror::Error;

/// Failures from context pack construction or Bedrock HTTP calls.
#[derive(Debug, Error)]
pub enum BedrockError {
    #[error("database error: {0}")]
    Db(String),
    #[error("http error: {0}")]
    Http(String),
    #[error("api error: HTTP {status}")]
    Api { status: u16 },
    #[error("parse error: {0}")]
    Parse(String),
    #[error("missing api key")]
    MissingApiKey,
    #[error("{0}")]
    Other(String),
}

impl From<rusqlite::Error> for BedrockError {
    fn from(value: rusqlite::Error) -> Self {
        Self::Db(value.to_string())
    }
}
