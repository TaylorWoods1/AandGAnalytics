//! Setup commands: save and validate Jira + Gemini credentials.

use ag_credentials::{GeminiCredentials, JiraCredentials};
use ag_db::{migrate, open_db};
use ag_jira::JiraClient;
use serde::{Deserialize, Serialize};

use crate::state::AppState;

/// IPC DTO for Jira credentials (serde-identical to domain type).
pub type JiraCredentialsDto = JiraCredentials;
/// IPC DTO for Gemini credentials.
pub type GeminiCredentialsDto = GeminiCredentials;

/// Result of probing Jira + Gemini with stored credentials.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct SetupStatus {
    pub jira_ok: bool,
    pub gemini_ok: bool,
    pub jira_message: String,
    pub gemini_message: String,
}

/// Persist credentials and ensure the local DB exists + is migrated.
pub fn save_setup_inner(
    state: &AppState,
    jira: JiraCredentialsDto,
    gemini: GeminiCredentialsDto,
) -> Result<(), String> {
    if jira.site_url.trim().is_empty() || jira.email.trim().is_empty() || jira.api_token.is_empty()
    {
        return Err("jira credentials incomplete".into());
    }
    if gemini.api_key.trim().is_empty() {
        return Err("gemini api key is required".into());
    }

    state
        .credentials
        .save_jira(&jira)
        .map_err(|e| e.to_string())?;
    state
        .credentials
        .save_gemini(&gemini)
        .map_err(|e| e.to_string())?;

    if let Some(parent) = state.db_path.parent() {
        std::fs::create_dir_all(parent).map_err(|e| e.to_string())?;
    }
    let conn = open_db(&state.db_path).map_err(|e| e.to_string())?;
    migrate(&conn).map_err(|e| e.to_string())?;
    Ok(())
}

/// Probe Jira `/myself` and Gemini `models` list using stored credentials.
pub async fn validate_setup_inner(state: &AppState) -> Result<SetupStatus, String> {
    let jira = state
        .credentials
        .load_jira()
        .map_err(|e| e.to_string())?
        .ok_or_else(|| "jira credentials not configured".to_string())?;
    let gemini = state
        .credentials
        .load_gemini()
        .map_err(|e| e.to_string())?
        .ok_or_else(|| "gemini credentials not configured".to_string())?;

    let (jira_ok, jira_message) = match probe_jira(&jira).await {
        Ok(msg) => (true, msg),
        Err(msg) => (false, msg),
    };
    let (gemini_ok, gemini_message) = match probe_gemini(&gemini).await {
        Ok(msg) => (true, msg),
        Err(msg) => (false, msg),
    };

    Ok(SetupStatus {
        jira_ok,
        gemini_ok,
        jira_message,
        gemini_message,
    })
}

async fn probe_jira(creds: &JiraCredentials) -> Result<String, String> {
    let client = JiraClient::new(creds).map_err(|e| e.to_string())?;
    let me = client.get_myself().await.map_err(|e| e.to_string())?;
    let label = me
        .display_name
        .or(me.email_address)
        .unwrap_or_else(|| me.account_id.clone());
    Ok(format!("authenticated as {label}"))
}

async fn probe_gemini(creds: &GeminiCredentials) -> Result<String, String> {
    let client = ag_gemini::GeminiClient::new(creds).map_err(|e| e.to_string())?;
    client.probe().await.map_err(|e| e.to_string())
}

#[cfg(feature = "desktop")]
mod tauri_cmds {
    use super::*;
    use tauri::State;

    #[tauri::command]
    pub fn save_setup(
        state: State<'_, std::sync::Arc<AppState>>,
        jira: JiraCredentialsDto,
        gemini: GeminiCredentialsDto,
    ) -> Result<(), String> {
        save_setup_inner(&state, jira, gemini)
    }

    #[tauri::command]
    pub async fn validate_setup(
        state: State<'_, std::sync::Arc<AppState>>,
    ) -> Result<SetupStatus, String> {
        validate_setup_inner(&state).await
    }
}

#[cfg(feature = "desktop")]
pub use tauri_cmds::{save_setup, validate_setup};
