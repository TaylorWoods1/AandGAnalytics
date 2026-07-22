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
    Ok(conn)
}
