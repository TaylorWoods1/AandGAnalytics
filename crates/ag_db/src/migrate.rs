//! Schema migrations for the local SQLite database.

use rusqlite::Connection;

use crate::connection::DbError;

/// Current schema version stored in `PRAGMA user_version`.
pub const SCHEMA_VERSION: i32 = 3;

/// Apply pending migrations to `conn` (idempotent).
pub fn migrate(conn: &Connection) -> Result<(), DbError> {
    let version: i32 = conn.pragma_query_value(None, "user_version", |row| row.get(0))?;
    if version < 1 {
        conn.execute_batch(include_str!("schema.sql"))?;
        conn.pragma_update(None, "user_version", SCHEMA_VERSION)?;
        return Ok(());
    }
    if version < 2 {
        migrate_v2(conn)?;
    }
    if version < 3 {
        migrate_v3(conn)?;
    }
    conn.pragma_update(None, "user_version", SCHEMA_VERSION)?;
    Ok(())
}

fn migrate_v2(conn: &Connection) -> Result<(), DbError> {
    if !column_exists(conn, "derived_epic_risk", "drivers_json")? {
        conn.execute(
            "ALTER TABLE derived_epic_risk ADD COLUMN drivers_json TEXT",
            [],
        )?;
    }
    if !column_exists(conn, "field_map", "candidates_json")? {
        conn.execute("ALTER TABLE field_map ADD COLUMN candidates_json TEXT", [])?;
    }
    Ok(())
}

fn migrate_v3(conn: &Connection) -> Result<(), DbError> {
    conn.execute_batch(
        "CREATE TABLE IF NOT EXISTS derived_completions (
            issue_id TEXT PRIMARY KEY NOT NULL,
            project_key TEXT NOT NULL,
            completed_at TEXT NOT NULL,
            finisher_account_id TEXT,
            story_points REAL,
            attribution TEXT NOT NULL
         );
         CREATE INDEX IF NOT EXISTS idx_derived_completions_finisher_month
            ON derived_completions (finisher_account_id, completed_at);
         CREATE INDEX IF NOT EXISTS idx_derived_completions_project
            ON derived_completions (project_key, completed_at);
         CREATE TABLE IF NOT EXISTS derived_person_month (
            month TEXT NOT NULL,
            account_id TEXT NOT NULL,
            completed_count INTEGER NOT NULL,
            points REAL,
            PRIMARY KEY (month, account_id)
         );",
    )?;
    Ok(())
}

fn column_exists(conn: &Connection, table: &str, column: &str) -> Result<bool, DbError> {
    let mut stmt = conn.prepare(&format!("PRAGMA table_info({table})"))?;
    let rows = stmt.query_map([], |row| row.get::<_, String>(1))?;
    for row in rows {
        if row? == column {
            return Ok(true);
        }
    }
    Ok(false)
}

#[cfg(test)]
mod tests {
    use crate::{migrate, open_db, SCHEMA_VERSION};
    use rusqlite::Connection;

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
            "derived_epic_risk",
            "derived_completions",
            "derived_person_month",
            "field_map",
        ] {
            let found: Option<String> = stmt.query_row([table], |r| r.get(0)).ok();
            assert_eq!(found.as_deref(), Some(table), "missing table {table}");
        }

        let version: i32 = conn
            .pragma_query_value(None, "user_version", |row| row.get(0))
            .unwrap();
        assert_eq!(version, SCHEMA_VERSION);
    }

    #[test]
    fn migrate_v2_adds_drivers_and_candidates_columns() {
        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().join("v1.db");
        let conn = Connection::open(&path).unwrap();
        conn.execute_batch(
            "CREATE TABLE derived_epic_risk (
                epic_key TEXT PRIMARY KEY NOT NULL,
                risk_score REAL,
                finish_by_probability REAL,
                assumptions_json TEXT
             );
             CREATE TABLE field_map (
                logical_name TEXT PRIMARY KEY NOT NULL,
                jira_field_id TEXT,
                jira_field_name TEXT,
                status TEXT NOT NULL DEFAULT 'unresolved'
             );",
        )
        .unwrap();
        conn.pragma_update(None, "user_version", 1).unwrap();

        migrate(&conn).unwrap();

        let drivers: i64 = conn
            .query_row(
                "SELECT COUNT(*) FROM pragma_table_info('derived_epic_risk')
                 WHERE name = 'drivers_json'",
                [],
                |r| r.get(0),
            )
            .unwrap();
        let candidates: i64 = conn
            .query_row(
                "SELECT COUNT(*) FROM pragma_table_info('field_map')
                 WHERE name = 'candidates_json'",
                [],
                |r| r.get(0),
            )
            .unwrap();
        assert_eq!(drivers, 1);
        assert_eq!(candidates, 1);
        let version: i32 = conn
            .pragma_query_value(None, "user_version", |row| row.get(0))
            .unwrap();
        assert_eq!(version, SCHEMA_VERSION);
    }

    #[test]
    fn migrate_v3_adds_performance_tables() {
        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().join("v2.db");
        let conn = Connection::open(&path).unwrap();
        conn.execute_batch("CREATE TABLE meta (key TEXT PRIMARY KEY, value TEXT NOT NULL);")
            .unwrap();
        conn.pragma_update(None, "user_version", 2).unwrap();

        migrate(&conn).unwrap();

        let mut stmt = conn
            .prepare("SELECT name FROM sqlite_master WHERE type='table' AND name=?")
            .unwrap();
        for table in ["derived_completions", "derived_person_month"] {
            let found: Option<String> = stmt.query_row([table], |r| r.get(0)).ok();
            assert_eq!(found.as_deref(), Some(table), "missing table {table}");
        }
        let version: i32 = conn
            .pragma_query_value(None, "user_version", |row| row.get(0))
            .unwrap();
        assert_eq!(version, 3);
    }
}
