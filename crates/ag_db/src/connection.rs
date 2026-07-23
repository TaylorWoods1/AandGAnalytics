//! SQLite connection helpers.

use std::path::Path;

use rusqlite::Connection;
use thiserror::Error;

/// Errors from opening or migrating the local database.
#[derive(Debug, Error)]
pub enum DbError {
    #[error("sqlite error: {0}")]
    Sqlite(#[from] rusqlite::Error),
}

/// Open (or create) a SQLite database at `path`.
pub fn open_db(path: &Path) -> Result<Connection, DbError> {
    let conn = Connection::open(path)?;
    conn.execute_batch("PRAGMA foreign_keys = ON;")?;
    // WAL allows concurrent readers during writes; busy_timeout softens lock races.
    conn.pragma_update(None, "journal_mode", "WAL")?;
    conn.pragma_update(None, "busy_timeout", 5000)?;
    Ok(conn)
}

#[cfg(test)]
mod tests {
    use super::*;
    use tempfile::tempdir;

    #[test]
    fn open_db_enables_wal_and_busy_timeout() {
        let dir = tempdir().unwrap();
        let path = dir.path().join("wal.db");
        let conn = open_db(&path).unwrap();

        let mode: String = conn
            .pragma_query_value(None, "journal_mode", |row| row.get(0))
            .unwrap();
        assert_eq!(mode.to_ascii_lowercase(), "wal");

        let timeout: i64 = conn
            .pragma_query_value(None, "busy_timeout", |row| row.get(0))
            .unwrap();
        assert_eq!(timeout, 5000);
    }
}
