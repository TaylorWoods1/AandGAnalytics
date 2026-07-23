//! Errors from the Jira HTTP client.

use thiserror::Error;

/// Errors returned by [`crate::JiraClient`] and [`crate::HttpDoer`] implementations.
#[derive(Debug, Error)]
pub enum JiraError {
    /// Transport or client-build failure. Never includes Authorization material.
    #[error("HTTP error: {0}")]
    Http(String),

    /// Jira returned HTTP 429. `retry_after_ms` is derived from `Retry-After` when present.
    #[error("rate limited by Jira")]
    RateLimited { retry_after_ms: Option<u64> },

    /// HTTP 401.
    #[error("unauthorized")]
    Unauthorized,

    /// HTTP 403 (generic permission / auth failure).
    #[error("forbidden")]
    Forbidden,

    /// HTTP 403 from Atlassian IP allowlist (org network policy).
    #[error("ip not allowlisted")]
    IpNotAllowlisted,

    /// Other non-success HTTP status with a redacted/truncated body.
    #[error("Jira API error {status}")]
    Api { status: u16, body: String },

    /// Response JSON could not be decoded.
    #[error("JSON error: {0}")]
    Json(#[from] serde_json::Error),
}
