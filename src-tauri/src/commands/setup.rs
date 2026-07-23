//! Setup commands: save and validate Jira + Gemini credentials.

use ag_credentials::{GeminiCredentials, JiraCredentials};
use ag_db::{migrate, open_db};
use ag_jira::JiraClient;
use rusqlite::{params, Connection};
use serde::{Deserialize, Serialize};

use crate::state::AppState;

const FIELD_STORY_POINTS: &str = "story_points";

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

/// One candidate Jira field for story-points mapping.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct FieldCandidateDto {
    pub id: String,
    pub name: String,
}

/// Current story-points field map status (for Setup/Sync UI).
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct StoryPointsMappingDto {
    pub status: String,
    pub jira_field_id: Option<String>,
    pub jira_field_name: Option<String>,
    pub candidates: Vec<FieldCandidateDto>,
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
    let me = match client.get_myself().await {
        Ok(me) => me,
        Err(ag_jira::JiraError::Unauthorized) => {
            return Err("unauthorized (HTTP 401): update your Jira API token".into());
        }
        Err(ag_jira::JiraError::Forbidden) => {
            return Err("forbidden (HTTP 403): token lacks permission for this site".into());
        }
        Err(e) => return Err(e.to_string()),
    };
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

/// Read story-points field map (including ambiguous candidates for UI confirmation).
pub fn get_story_points_mapping_inner(state: &AppState) -> Result<StoryPointsMappingDto, String> {
    if let Some(parent) = state.db_path.parent() {
        std::fs::create_dir_all(parent).map_err(|e| e.to_string())?;
    }
    let conn = open_db(&state.db_path).map_err(|e| e.to_string())?;
    migrate(&conn).map_err(|e| e.to_string())?;
    read_story_points_mapping(&conn)
}

/// Persist a user-selected story-points field and backfill from stored issue JSON.
pub fn set_story_points_mapping_inner(
    state: &AppState,
    jira_field_id: String,
) -> Result<StoryPointsMappingDto, String> {
    let field_id = jira_field_id.trim().to_string();
    if field_id.is_empty() {
        return Err("jira field id is required".into());
    }
    if let Some(parent) = state.db_path.parent() {
        std::fs::create_dir_all(parent).map_err(|e| e.to_string())?;
    }
    let conn = open_db(&state.db_path).map_err(|e| e.to_string())?;
    migrate(&conn).map_err(|e| e.to_string())?;

    let mapping = read_story_points_mapping(&conn)?;
    let name = mapping
        .candidates
        .iter()
        .find(|c| c.id == field_id)
        .map(|c| c.name.clone())
        .or(mapping.jira_field_name)
        .unwrap_or_else(|| field_id.clone());

    conn.execute(
        "INSERT INTO field_map (
            logical_name, jira_field_id, jira_field_name, status, candidates_json
         ) VALUES (?1, ?2, ?3, 'resolved', ?4)
         ON CONFLICT(logical_name) DO UPDATE SET
            jira_field_id = excluded.jira_field_id,
            jira_field_name = excluded.jira_field_name,
            status = 'resolved',
            candidates_json = COALESCE(excluded.candidates_json, field_map.candidates_json)",
        params![
            FIELD_STORY_POINTS,
            field_id,
            name,
            serde_json::to_string(&mapping.candidates).unwrap_or_else(|_| "[]".into()),
        ],
    )
    .map_err(|e| e.to_string())?;

    backfill_story_points_from_raw(&conn, &field_id)?;
    read_story_points_mapping(&conn)
}

fn read_story_points_mapping(conn: &Connection) -> Result<StoryPointsMappingDto, String> {
    let mut stmt = conn
        .prepare(
            "SELECT jira_field_id, jira_field_name, status, candidates_json
             FROM field_map WHERE logical_name = ?1",
        )
        .map_err(|e| e.to_string())?;
    let mut rows = stmt
        .query([FIELD_STORY_POINTS])
        .map_err(|e| e.to_string())?;
    let Some(row) = rows.next().map_err(|e| e.to_string())? else {
        return Ok(StoryPointsMappingDto {
            status: "unresolved".into(),
            jira_field_id: None,
            jira_field_name: None,
            candidates: Vec::new(),
        });
    };

    let jira_field_id: Option<String> = row.get(0).map_err(|e| e.to_string())?;
    let jira_field_name: Option<String> = row.get(1).map_err(|e| e.to_string())?;
    let status: String = row.get(2).map_err(|e| e.to_string())?;
    let candidates_json: Option<String> = row.get(3).map_err(|e| e.to_string())?;

    let mut candidates: Vec<FieldCandidateDto> = candidates_json
        .as_deref()
        .and_then(|raw| serde_json::from_str(raw).ok())
        .unwrap_or_default();

    // Legacy ambiguous rows stored comma-joined names without ids.
    if candidates.is_empty() {
        if let Some(names) = jira_field_name.as_deref() {
            if status != "resolved" && names.contains(',') {
                candidates = names
                    .split(',')
                    .map(|n| n.trim())
                    .filter(|n| !n.is_empty())
                    .map(|n| FieldCandidateDto {
                        id: String::new(),
                        name: n.to_string(),
                    })
                    .collect();
            }
        }
    }

    Ok(StoryPointsMappingDto {
        status,
        jira_field_id,
        jira_field_name,
        candidates,
    })
}

fn backfill_story_points_from_raw(conn: &Connection, field_id: &str) -> Result<(), String> {
    let path = format!("$.fields.{field_id}");
    conn.execute(
        "UPDATE issues
         SET story_points = CAST(json_extract(raw_json, ?1) AS REAL)
         WHERE json_extract(raw_json, ?1) IS NOT NULL",
        params![path],
    )
    .map_err(|e| e.to_string())?;
    Ok(())
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

    #[tauri::command]
    pub fn get_story_points_mapping(
        state: State<'_, std::sync::Arc<AppState>>,
    ) -> Result<StoryPointsMappingDto, String> {
        get_story_points_mapping_inner(&state)
    }

    #[tauri::command]
    pub fn set_story_points_mapping(
        state: State<'_, std::sync::Arc<AppState>>,
        jira_field_id: String,
    ) -> Result<StoryPointsMappingDto, String> {
        set_story_points_mapping_inner(&state, jira_field_id)
    }
}

#[cfg(feature = "desktop")]
pub use tauri_cmds::{
    get_story_points_mapping, save_setup, set_story_points_mapping, validate_setup,
};

#[cfg(test)]
mod tests {
    use super::*;
    use ag_credentials::{CredentialStore, MemoryCredentialStore};
    use std::sync::Arc;
    use tempfile::tempdir;

    #[test]
    fn set_story_points_mapping_persists_and_backfills() {
        let dir = tempdir().unwrap();
        let db_path = dir.path().join("setup.db");
        {
            let conn = open_db(&db_path).unwrap();
            migrate(&conn).unwrap();
            conn.execute(
                "INSERT INTO field_map (
                    logical_name, jira_field_id, jira_field_name, status, candidates_json
                 ) VALUES (
                    'story_points', NULL, 'Story Points, Story point estimate', 'unresolved',
                    '[{\"id\":\"customfield_10016\",\"name\":\"Story Points\"},{\"id\":\"customfield_10028\",\"name\":\"Story point estimate\"}]'
                 )",
                [],
            )
            .unwrap();
            conn.execute(
                "INSERT INTO issues (
                    id, key, project_key, summary, issue_type, status,
                    created, updated, raw_json
                 ) VALUES (
                    '1', 'DEMO-1', 'DEMO', 'Sample', 'Story', 'To Do',
                    '2024-01-01T00:00:00Z', '2024-01-01T00:00:00Z',
                    '{\"id\":\"1\",\"key\":\"DEMO-1\",\"fields\":{\"customfield_10016\":5}}'
                 )",
                [],
            )
            .unwrap();
        }

        let store = MemoryCredentialStore::default();
        store
            .save_jira(&JiraCredentials {
                site_url: "https://example.atlassian.net".into(),
                email: "a@b.c".into(),
                api_token: "t".into(),
            })
            .unwrap();
        store
            .save_gemini(&GeminiCredentials {
                api_key: "g".into(),
            })
            .unwrap();
        let state = AppState::with_credentials(db_path.clone(), Arc::new(store));

        let before = get_story_points_mapping_inner(&state).unwrap();
        assert_eq!(before.status, "unresolved");
        assert_eq!(before.candidates.len(), 2);

        let after = set_story_points_mapping_inner(&state, "customfield_10016".into()).unwrap();
        assert_eq!(after.status, "resolved");
        assert_eq!(after.jira_field_id.as_deref(), Some("customfield_10016"));

        let conn = open_db(&db_path).unwrap();
        let points: f64 = conn
            .query_row("SELECT story_points FROM issues WHERE id = '1'", [], |r| {
                r.get(0)
            })
            .unwrap();
        assert_eq!(points, 5.0);
    }
}
