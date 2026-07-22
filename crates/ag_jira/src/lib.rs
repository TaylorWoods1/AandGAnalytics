//! Jira Cloud REST client for AandG Analytics.
//!
//! Provides a fixture-testable [`JiraClient`] over a pluggable [`HttpDoer`].
//! Authorization headers are redacted in [`HttpRequest`]'s `Debug` impl and
//! must never be logged in cleartext.

mod client;
mod error;
mod types;

pub use client::{HttpDoer, HttpRequest, HttpResponse, JiraClient, ReqwestHttpDoer};
pub use error::JiraError;
pub use types::{
    Board, BoardPage, Changelog, ChangelogHistory, ChangelogItem, FieldSchema, Issue,
    IssueSearchPage, JiraField, Myself, Project, Sprint, SprintPage,
};

#[cfg(test)]
mod tests {
    #[test]
    fn workspace_compiles_and_jira_crate_is_wired() {
        assert_eq!(env!("CARGO_PKG_NAME"), "ag_jira");
    }
}
