//! Ask AI commands: context pack preview + Bedrock Q&A.

use std::path::Path;

use ag_bedrock::{
    build_context_pack, suggested_prompts, AiAnswer, BedrockClient, ContextPack,
    MetricsFilter as PackFilter,
};
use serde::{Deserialize, Serialize};

use crate::commands::metrics::MetricsFilter;
use crate::state::AppState;

/// Default token budget for context packs sent to Bedrock.
pub const DEFAULT_TOKEN_BUDGET: usize = 6_000;

/// IPC DTO mirroring [`ContextPack`].
pub type ContextPackDto = ContextPack;
/// IPC DTO mirroring [`AiAnswer`].
pub type AiAnswerDto = AiAnswer;

/// Suggested prompt strings for the Ask AI UI.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct SuggestedPromptsDto {
    pub prompts: Vec<String>,
}

fn to_pack_filter(filter: &MetricsFilter) -> PackFilter {
    PackFilter {
        project_keys: filter.project_keys.clone(),
        from: filter.from.clone(),
        to: filter.to.clone(),
        issue_types: filter.issue_types.clone(),
        assignee_ids: filter.assignee_ids.clone(),
    }
}

fn with_db<T>(
    state: &AppState,
    f: impl FnOnce(&rusqlite::Connection) -> Result<T, String>,
) -> Result<T, String> {
    if !Path::new(&state.db_path).is_file() {
        return Err("database not found; run setup / sync first".into());
    }
    let conn = open_db_conn(state)?;
    f(&conn)
}

fn open_db_conn(state: &AppState) -> Result<rusqlite::Connection, String> {
    let conn = ag_db::open_db(&state.db_path).map_err(|e| e.to_string())?;
    ag_db::migrate(&conn).map_err(|e| e.to_string())?;
    Ok(conn)
}

/// Preview the context pack that would be sent for the active filters.
pub fn preview_context_pack_inner(
    state: &AppState,
    filter: MetricsFilter,
) -> Result<ContextPackDto, String> {
    filter.validate()?;
    with_db(state, |conn| {
        build_context_pack(conn, &to_pack_filter(&filter), DEFAULT_TOKEN_BUDGET)
            .map_err(|e| e.to_string())
    })
}

/// Ask Bedrock a question grounded in a local context pack.
///
/// Errors are chat-only; callers must not treat failures as dashboard outages.
pub async fn ask_ai_inner(
    state: &AppState,
    filter: MetricsFilter,
    question: String,
) -> Result<AiAnswerDto, String> {
    filter.validate()?;
    let pack = with_db(state, |conn| {
        build_context_pack(conn, &to_pack_filter(&filter), DEFAULT_TOKEN_BUDGET)
            .map_err(|e| e.to_string())
    })?;

    let bedrock = state
        .credentials
        .load_bedrock()
        .map_err(|e| e.to_string())?
        .ok_or_else(|| "bedrock credentials not configured".to_string())?;

    let client = BedrockClient::new(&bedrock).map_err(|e| e.to_string())?;
    client
        .ask(&pack, &question)
        .await
        .map_err(|e| e.to_string())
}

/// Suggested starter prompts for Ask AI.
pub fn get_suggested_prompts_inner() -> SuggestedPromptsDto {
    SuggestedPromptsDto {
        prompts: suggested_prompts(),
    }
}

#[cfg(feature = "desktop")]
pub mod tauri_cmds {
    use super::*;
    use tauri::State;

    #[tauri::command]
    pub fn preview_context_pack(
        state: State<'_, std::sync::Arc<AppState>>,
        filter: MetricsFilter,
    ) -> Result<ContextPackDto, String> {
        preview_context_pack_inner(&state, filter)
    }

    #[tauri::command]
    pub async fn ask_ai(
        state: State<'_, std::sync::Arc<AppState>>,
        filter: MetricsFilter,
        question: String,
    ) -> Result<AiAnswerDto, String> {
        ask_ai_inner(&state, filter, question).await
    }

    #[tauri::command]
    pub fn get_suggested_prompts() -> SuggestedPromptsDto {
        get_suggested_prompts_inner()
    }
}

#[cfg(feature = "desktop")]
pub use tauri_cmds::{ask_ai, get_suggested_prompts, preview_context_pack};

#[cfg(test)]
mod tests {
    use super::*;
    use ag_credentials::{
        BedrockCredentials, CredentialStore, JiraCredentials, MemoryCredentialStore,
    };
    use ag_db::{migrate, open_db};
    use std::sync::Arc;
    use tempfile::tempdir;

    fn state_with_db() -> (tempfile::TempDir, AppState) {
        let dir = tempdir().unwrap();
        let db_path = dir.path().join("t.db");
        let conn = open_db(&db_path).unwrap();
        migrate(&conn).unwrap();
        conn.execute(
            "INSERT INTO issues (
                id, key, project_key, summary, issue_type, status,
                assignee_account_id, created, updated
             ) VALUES ('1', 'PROJ-1', 'PROJ', 'Review lag', 'Story', 'Code Review', 'u1',
                       '2025-01-01T00:00:00Z', '2025-06-01T00:00:00Z')",
            [],
        )
        .unwrap();
        drop(conn);

        let store = MemoryCredentialStore::default();
        store
            .save_jira(&JiraCredentials {
                site_url: "https://example.atlassian.net".into(),
                email: "a@b.c".into(),
                api_token: "t".into(),
            })
            .unwrap();
        store
            .save_bedrock(&BedrockCredentials {
                api_key: "g".into(),
                region: "ap-southeast-2".into(),
            })
            .unwrap();
        let state = AppState::with_credentials(db_path, Arc::new(store));
        (dir, state)
    }

    #[test]
    fn preview_context_pack_returns_pack() {
        let (_dir, state) = state_with_db();
        let pack = preview_context_pack_inner(
            &state,
            MetricsFilter {
                project_keys: Some(vec!["PROJ".into()]),
                from: None,
                to: None,
                issue_types: None,
                assignee_ids: None,
            },
        )
        .unwrap();
        assert!(pack.filter_summary.contains("PROJ"));
        assert!(!pack.supporting_issues.is_empty());
        assert_eq!(pack.supporting_issues[0].key, "PROJ-1");
    }

    #[test]
    fn suggested_prompts_non_empty() {
        let prompts = get_suggested_prompts_inner();
        assert!(!prompts.prompts.is_empty());
    }
}
