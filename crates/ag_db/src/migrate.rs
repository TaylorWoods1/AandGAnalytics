//! Schema migrations for the local SQLite database.

use rusqlite::Connection;

use crate::connection::DbError;

/// Current schema version stored in `PRAGMA user_version`.
pub const SCHEMA_VERSION: i32 = 1;

/// Apply pending migrations to `conn` (idempotent).
pub fn migrate(conn: &Connection) -> Result<(), DbError> {
    let version: i32 = conn.pragma_query_value(None, "user_version", |row| row.get(0))?;
    if version < SCHEMA_VERSION {
        conn.execute_batch(include_str!("schema.sql"))?;
        conn.pragma_update(None, "user_version", SCHEMA_VERSION)?;
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use crate::{migrate, open_db};

    #[test]
    fn migrate_creates_core_tables() {
        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().join("test.db");
        let conn = open_db(&path).unwrap();
        migrate(&conn).unwrap();

        let mut stmt = conn
            .prepare("SELECT name FROM sqlite_master WHERE type='table' AND name=?")
            .unwrap();
        for table in [
            "projects",
            "issues",
            "issue_changelog",
            "sprints",
            "sync_checkpoints",
            "derived_issue_cycle",
        ] {
            let found: Option<String> = stmt.query_row([table], |r| r.get(0)).ok();
            assert_eq!(found.as_deref(), Some(table), "missing table {table}");
        }
    }
}
