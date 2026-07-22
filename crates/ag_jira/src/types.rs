//! Jira Cloud REST types used by the HTTP client.

use serde::{Deserialize, Serialize};

/// Current user from `GET /rest/api/3/myself`.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct Myself {
    pub account_id: String,
    #[serde(default)]
    pub email_address: Option<String>,
    #[serde(default)]
    pub display_name: Option<String>,
    #[serde(default)]
    pub active: Option<bool>,
}

/// Field definition from `GET /rest/api/3/field`.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct JiraField {
    pub id: String,
    pub name: String,
    #[serde(default)]
    pub custom: bool,
    #[serde(default)]
    pub schema: Option<FieldSchema>,
}

/// Schema metadata for a Jira field.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct FieldSchema {
    #[serde(rename = "type")]
    pub field_type: Option<String>,
    #[serde(default)]
    pub custom: Option<String>,
    #[serde(default)]
    pub custom_id: Option<u64>,
    #[serde(default)]
    pub system: Option<String>,
}

/// One page of issue search results (`POST /rest/api/3/search/jql`).
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct IssueSearchPage {
    #[serde(default)]
    pub issues: Vec<Issue>,
    #[serde(default)]
    pub next_page_token: Option<String>,
    #[serde(default)]
    pub is_last: Option<bool>,
}

/// A Jira issue (subset needed for sync).
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct Issue {
    pub id: String,
    pub key: String,
    #[serde(default)]
    pub fields: Option<serde_json::Value>,
    #[serde(default)]
    pub changelog: Option<Changelog>,
}

/// Changelog expand payload on an issue.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct Changelog {
    #[serde(default)]
    pub histories: Vec<ChangelogHistory>,
}

/// One changelog history entry.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ChangelogHistory {
    #[serde(default)]
    pub id: Option<String>,
    #[serde(default)]
    pub created: Option<String>,
    #[serde(default)]
    pub items: Vec<ChangelogItem>,
}

/// One field change within a changelog history.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ChangelogItem {
    #[serde(default)]
    pub field: Option<String>,
    #[serde(default)]
    pub fieldtype: Option<String>,
    #[serde(default)]
    pub from: Option<String>,
    #[serde(default)]
    pub from_string: Option<String>,
    #[serde(default)]
    pub to: Option<String>,
    #[serde(default)]
    pub to_string: Option<String>,
}

/// Project from `GET /rest/api/3/project`.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct Project {
    pub id: String,
    pub key: String,
    pub name: String,
}

/// Board from `GET /rest/agile/1.0/board`.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct Board {
    pub id: i64,
    pub name: String,
    #[serde(default, rename = "type")]
    pub board_type: Option<String>,
}

/// Paginated boards response.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct BoardPage {
    #[serde(default)]
    pub values: Vec<Board>,
    #[serde(default)]
    pub is_last: Option<bool>,
    #[serde(default)]
    pub start_at: Option<u64>,
    #[serde(default)]
    pub max_results: Option<u64>,
    #[serde(default)]
    pub total: Option<u64>,
}

/// Sprint from the Agile API.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct Sprint {
    pub id: i64,
    pub name: String,
    #[serde(default)]
    pub state: Option<String>,
    #[serde(default)]
    pub start_date: Option<String>,
    #[serde(default)]
    pub end_date: Option<String>,
    #[serde(default)]
    pub complete_date: Option<String>,
    #[serde(default)]
    pub origin_board_id: Option<i64>,
    #[serde(default)]
    pub goal: Option<String>,
}

/// Paginated sprints response from a board.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct SprintPage {
    #[serde(default)]
    pub values: Vec<Sprint>,
    #[serde(default)]
    pub is_last: Option<bool>,
    #[serde(default)]
    pub start_at: Option<u64>,
    #[serde(default)]
    pub max_results: Option<u64>,
}
