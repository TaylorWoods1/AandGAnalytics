//! Persist and load sync checkpoints in SQLite.

use chrono::Utc;
use rusqlite::{params, Connection, OptionalExtension};

use crate::SyncError;

/// Checkpoint row keyed by `scope_key` (e.g. `issues:global`).
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Checkpoint {
    pub scope_key: String,
    pub next_page_token: Option<String>,
    pub jql_cursor: Option<String>,
    pub last_updated_watermark: Option<String>,
    pub updated_at: String,
}

/// Load a checkpoint by scope key.
pub fn load_checkpoint(
    conn: &Connection,
    scope_key: &str,
) -> Result<Option<Checkpoint>, SyncError> {
    let row = conn
        .query_row(
            "SELECT scope_key, next_page_token, jql_cursor, last_updated_watermark, updated_at
             FROM sync_checkpoints WHERE scope_key = ?1",
            params![scope_key],
            |r| {
                Ok(Checkpoint {
                    scope_key: r.get(0)?,
                    next_page_token: r.get(1)?,
                    jql_cursor: r.get(2)?,
                    last_updated_watermark: r.get(3)?,
                    updated_at: r.get(4)?,
                })
            },
        )
        .optional()?;
    Ok(row)
}

/// Upsert a checkpoint row.
pub fn save_checkpoint(
    conn: &Connection,
    scope_key: &str,
    next_page_token: Option<&str>,
    jql_cursor: Option<&str>,
    last_updated_watermark: Option<&str>,
) -> Result<(), SyncError> {
    let updated_at = Utc::now().to_rfc3339();
    conn.execute(
        "INSERT INTO sync_checkpoints (
            scope_key, next_page_token, jql_cursor, last_updated_watermark, updated_at
         ) VALUES (?1, ?2, ?3, ?4, ?5)
         ON CONFLICT(scope_key) DO UPDATE SET
            next_page_token = excluded.next_page_token,
            jql_cursor = excluded.jql_cursor,
            last_updated_watermark = excluded.last_updated_watermark,
            updated_at = excluded.updated_at",
        params![
            scope_key,
            next_page_token,
            jql_cursor,
            last_updated_watermark,
            updated_at
        ],
    )?;
    Ok(())
}

/// Whether a checkpoint row exists for `scope_key`.
pub fn checkpoint_exists(conn: &Connection, scope_key: &str) -> Result<bool, SyncError> {
    let n: i64 = conn.query_row(
        "SELECT COUNT(*) FROM sync_checkpoints WHERE scope_key = ?1",
        params![scope_key],
        |r| r.get(0),
    )?;
    Ok(n > 0)
}

/// Clear pagination state for a completed scope (keeps watermark if present).
pub fn clear_page_token(conn: &Connection, scope_key: &str) -> Result<(), SyncError> {
    conn.execute(
        "UPDATE sync_checkpoints SET next_page_token = NULL, updated_at = ?2 WHERE scope_key = ?1",
        params![scope_key, Utc::now().to_rfc3339()],
    )?;
    Ok(())
}

/// Read a meta value.
pub fn get_meta(conn: &Connection, key: &str) -> Result<Option<String>, SyncError> {
    let v = conn
        .query_row("SELECT value FROM meta WHERE key = ?1", params![key], |r| {
            r.get(0)
        })
        .optional()?;
    Ok(v)
}

/// Upsert a meta value.
pub fn set_meta(conn: &Connection, key: &str, value: &str) -> Result<(), SyncError> {
    conn.execute(
        "INSERT INTO meta (key, value) VALUES (?1, ?2)
         ON CONFLICT(key) DO UPDATE SET value = excluded.value",
        params![key, value],
    )?;
    Ok(())
}
