//! Credential storage backends.
//!
//! Production uses the OS keychain via [`KeychainCredentialStore`]. Secrets
//! (API tokens / keys) must never be logged — not even at debug level. Callers
//! must redact credentials before any logging or error formatting that might
//! leave the process.

use std::fmt;
use std::sync::Mutex;

use keyring::Entry;
use serde::{Deserialize, Serialize};
use thiserror::Error;

const SERVICE: &str = "com.aandganalytics.desktop";
const JIRA_ACCOUNT: &str = "jira";
const GEMINI_ACCOUNT: &str = "gemini";

/// Jira site + personal API token credentials.
///
/// Never log `api_token`. Prefer [`fmt::Debug`] which redacts it.
#[derive(Clone, Serialize, Deserialize)]
pub struct JiraCredentials {
    pub site_url: String,
    pub email: String,
    pub api_token: String,
}

impl fmt::Debug for JiraCredentials {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_struct("JiraCredentials")
            .field("site_url", &self.site_url)
            .field("email", &self.email)
            .field("api_token", &"[REDACTED]")
            .finish()
    }
}

/// Gemini API key credentials.
///
/// Never log `api_key`. Prefer [`fmt::Debug`] which redacts it.
#[derive(Clone, Serialize, Deserialize)]
pub struct GeminiCredentials {
    pub api_key: String,
}

impl fmt::Debug for GeminiCredentials {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_struct("GeminiCredentials")
            .field("api_key", &"[REDACTED]")
            .finish()
    }
}

/// Errors from credential store operations.
///
/// Messages must not include secret values.
#[derive(Debug, Error)]
pub enum CredentialError {
    #[error("credential store lock poisoned")]
    LockPoisoned,
    #[error("keychain error: {0}")]
    Keychain(String),
    #[error("credential data corrupt")]
    Corrupt,
}

impl From<keyring::Error> for CredentialError {
    fn from(err: keyring::Error) -> Self {
        // Map via Display only — never attach raw password material.
        CredentialError::Keychain(err.to_string())
    }
}

/// Persist and load Jira / Gemini credentials.
pub trait CredentialStore: Send + Sync {
    fn save_jira(&self, creds: &JiraCredentials) -> Result<(), CredentialError>;
    fn load_jira(&self) -> Result<Option<JiraCredentials>, CredentialError>;
    fn save_gemini(&self, creds: &GeminiCredentials) -> Result<(), CredentialError>;
    fn load_gemini(&self) -> Result<Option<GeminiCredentials>, CredentialError>;
    fn clear_all(&self) -> Result<(), CredentialError>;
}

/// In-memory credential store for tests.
#[derive(Default)]
pub struct MemoryCredentialStore {
    jira: Mutex<Option<JiraCredentials>>,
    gemini: Mutex<Option<GeminiCredentials>>,
}

impl CredentialStore for MemoryCredentialStore {
    fn save_jira(&self, creds: &JiraCredentials) -> Result<(), CredentialError> {
        let mut guard = self
            .jira
            .lock()
            .map_err(|_| CredentialError::LockPoisoned)?;
        *guard = Some(creds.clone());
        Ok(())
    }

    fn load_jira(&self) -> Result<Option<JiraCredentials>, CredentialError> {
        let guard = self
            .jira
            .lock()
            .map_err(|_| CredentialError::LockPoisoned)?;
        Ok(guard.clone())
    }

    fn save_gemini(&self, creds: &GeminiCredentials) -> Result<(), CredentialError> {
        let mut guard = self
            .gemini
            .lock()
            .map_err(|_| CredentialError::LockPoisoned)?;
        *guard = Some(creds.clone());
        Ok(())
    }

    fn load_gemini(&self) -> Result<Option<GeminiCredentials>, CredentialError> {
        let guard = self
            .gemini
            .lock()
            .map_err(|_| CredentialError::LockPoisoned)?;
        Ok(guard.clone())
    }

    fn clear_all(&self) -> Result<(), CredentialError> {
        {
            let mut guard = self
                .jira
                .lock()
                .map_err(|_| CredentialError::LockPoisoned)?;
            *guard = None;
        }
        {
            let mut guard = self
                .gemini
                .lock()
                .map_err(|_| CredentialError::LockPoisoned)?;
            *guard = None;
        }
        Ok(())
    }
}

/// OS keychain-backed credential store (thin `keyring` wrapper).
///
/// Unit tests should use [`MemoryCredentialStore`]. This type is always compiled
/// so production binaries can depend on it; exercise it with macOS integration
/// tests rather than unit tests.
#[derive(Debug, Default, Clone)]
pub struct KeychainCredentialStore;

impl KeychainCredentialStore {
    pub fn new() -> Self {
        Self
    }

    fn entry(account: &str) -> Result<Entry, CredentialError> {
        Entry::new(SERVICE, account).map_err(CredentialError::from)
    }

    fn set_secret(account: &str, secret: &str) -> Result<(), CredentialError> {
        Self::entry(account)?.set_password(secret)?;
        Ok(())
    }

    fn get_secret(account: &str) -> Result<Option<String>, CredentialError> {
        match Self::entry(account)?.get_password() {
            Ok(password) => Ok(Some(password)),
            Err(keyring::Error::NoEntry) => Ok(None),
            Err(err) => Err(CredentialError::from(err)),
        }
    }

    fn delete_secret(account: &str) -> Result<(), CredentialError> {
        match Self::entry(account)?.delete_password() {
            Ok(()) => Ok(()),
            Err(keyring::Error::NoEntry) => Ok(()),
            Err(err) => Err(CredentialError::from(err)),
        }
    }
}

impl CredentialStore for KeychainCredentialStore {
    fn save_jira(&self, creds: &JiraCredentials) -> Result<(), CredentialError> {
        let payload = serde_json::to_string(creds).map_err(|_| CredentialError::Corrupt)?;
        Self::set_secret(JIRA_ACCOUNT, &payload)
    }

    fn load_jira(&self) -> Result<Option<JiraCredentials>, CredentialError> {
        match Self::get_secret(JIRA_ACCOUNT)? {
            None => Ok(None),
            Some(payload) => {
                let creds = serde_json::from_str(&payload).map_err(|_| CredentialError::Corrupt)?;
                Ok(Some(creds))
            }
        }
    }

    fn save_gemini(&self, creds: &GeminiCredentials) -> Result<(), CredentialError> {
        let payload = serde_json::to_string(creds).map_err(|_| CredentialError::Corrupt)?;
        Self::set_secret(GEMINI_ACCOUNT, &payload)
    }

    fn load_gemini(&self) -> Result<Option<GeminiCredentials>, CredentialError> {
        match Self::get_secret(GEMINI_ACCOUNT)? {
            None => Ok(None),
            Some(payload) => {
                let creds = serde_json::from_str(&payload).map_err(|_| CredentialError::Corrupt)?;
                Ok(Some(creds))
            }
        }
    }

    fn clear_all(&self) -> Result<(), CredentialError> {
        Self::delete_secret(JIRA_ACCOUNT)?;
        Self::delete_secret(GEMINI_ACCOUNT)?;
        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn memory_store_round_trips_jira_and_gemini() {
        let store = MemoryCredentialStore::default();
        assert!(store.load_jira().unwrap().is_none());

        store
            .save_jira(&JiraCredentials {
                site_url: "https://example.atlassian.net".into(),
                email: "dev@example.com".into(),
                api_token: "secret-jira".into(),
            })
            .unwrap();
        store
            .save_gemini(&GeminiCredentials {
                api_key: "secret-gemini".into(),
            })
            .unwrap();

        let jira = store.load_jira().unwrap().unwrap();
        assert_eq!(jira.api_token, "secret-jira");
        assert_eq!(
            store.load_gemini().unwrap().unwrap().api_key,
            "secret-gemini"
        );

        store.clear_all().unwrap();
        assert!(store.load_jira().unwrap().is_none());
    }

    #[test]
    fn debug_redacts_secrets() {
        let jira = JiraCredentials {
            site_url: "https://example.atlassian.net".into(),
            email: "dev@example.com".into(),
            api_token: "secret-jira".into(),
        };
        let gemini = GeminiCredentials {
            api_key: "secret-gemini".into(),
        };
        let jira_dbg = format!("{jira:?}");
        let gemini_dbg = format!("{gemini:?}");
        assert!(!jira_dbg.contains("secret-jira"));
        assert!(jira_dbg.contains("[REDACTED]"));
        assert!(!gemini_dbg.contains("secret-gemini"));
        assert!(gemini_dbg.contains("[REDACTED]"));
    }
}
