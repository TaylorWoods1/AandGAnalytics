//! Keychain credential wrapper for AandG Analytics.
//!
//! Secrets must never be written to logs or persisted outside the credential store.

mod store;

pub use store::*;
