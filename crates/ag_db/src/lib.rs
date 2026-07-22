//! SQLite schema, migrations, and connection helpers for AandG Analytics.

mod connection;
mod migrate;

pub use connection::{open_db, DbError};
pub use migrate::{migrate, SCHEMA_VERSION};

pub use rusqlite::Connection;

#[cfg(test)]
mod tests {
    #[test]
    fn workspace_compiles_and_db_crate_is_wired() {
        assert_eq!(env!("CARGO_PKG_NAME"), "ag_db");
    }
}
