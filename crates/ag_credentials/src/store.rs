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
const BEDROCK_ACCOUNT: &str = "bedrock";

/// Default AWS region for Bedrock Runtime when the user does not specify one.
pub const DEFAULT_BEDROCK_REGION: &str = "ap-southeast-2";

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

/// Amazon Bedrock API key credentials (bearer token auth).
///
/// Never log `api_key`. Prefer [`fmt::Debug`] which redacts it.
#[derive(Clone, Serialize, Deserialize)]
pub struct BedrockCredentials {
    pub api_key: String,
    /// AWS region for the Bedrock Runtime endpoint (e.g. `ap-southeast-2`).
    #[serde(default = "default_bedrock_region")]
    pub region: String,
}

fn default_bedrock_region() -> String {
    DEFAULT_BEDROCK_REGION.to_string()
}

impl BedrockCredentials {
    /// Normalize empty region to the AU default.
    pub fn normalized(mut self) -> Self {
        if self.region.trim().is_empty() {
            self.region = DEFAULT_BEDROCK_REGION.to_string();
        } else {
            self.region = self.region.trim().to_string();
        }
        self
    }
}

impl fmt::Debug for BedrockCredentials {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_struct("BedrockCredentials")
            .field("api_key", &"[REDACTED]")
            .field("region", &self.region)
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

/// Persist and load Jira / Bedrock credentials.
pub trait CredentialStore: Send + Sync {
    fn save_jira(&self, creds: &JiraCredentials) -> Result<(), CredentialError>;
    fn load_jira(&self) -> Result<Option<JiraCredentials>, CredentialError>;
    fn save_bedrock(&self, creds: &BedrockCredentials) -> Result<(), CredentialError>;
    fn load_bedrock(&self) -> Result<Option<BedrockCredentials>, CredentialError>;
    fn clear_all(&self) -> Result<(), CredentialError>;
}

/// In-memory credential store for tests.
#[derive(Default)]
pub struct MemoryCredentialStore {
    jira: Mutex<Option<JiraCredentials>>,
    bedrock: Mutex<Option<BedrockCredentials>>,
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

    fn save_bedrock(&self, creds: &BedrockCredentials) -> Result<(), CredentialError> {
        let mut guard = self
            .bedrock
            .lock()
            .map_err(|_| CredentialError::LockPoisoned)?;
        *guard = Some(creds.clone().normalized());
        Ok(())
    }

    fn load_bedrock(&self) -> Result<Option<BedrockCredentials>, CredentialError> {
        let guard = self
            .bedrock
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
                .bedrock
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

    fn save_bedrock(&self, creds: &BedrockCredentials) -> Result<(), CredentialError> {
        let payload =
            serde_json::to_string(&creds.clone().normalized()).map_err(|_| CredentialError::Corrupt)?;
        Self::set_secret(BEDROCK_ACCOUNT, &payload)
    }

    fn load_bedrock(&self) -> Result<Option<BedrockCredentials>, CredentialError> {
        match Self::get_secret(BEDROCK_ACCOUNT)? {
            None => Ok(None),
            Some(payload) => {
                let creds: BedrockCredentials =
                    serde_json::from_str(&payload).map_err(|_| CredentialError::Corrupt)?;
                Ok(Some(creds.normalized()))
            }
        }
    }

    fn clear_all(&self) -> Result<(), CredentialError> {
        Self::delete_secret(JIRA_ACCOUNT)?;
        Self::delete_secret(BEDROCK_ACCOUNT)?;
        // Legacy Gemini account from earlier builds.
        Self::delete_secret("gemini")?;
        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn memory_store_round_trips_jira_and_bedrock() {
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
            .save_bedrock(&BedrockCredentials {
                api_key: "secret-bedrock".into(),
                region: "".into(),
            })
            .unwrap();

        let jira = store.load_jira().unwrap().unwrap();
        assert_eq!(jira.api_token, "secret-jira");
        let bedrock = store.load_bedrock().unwrap().unwrap();
        assert_eq!(bedrock.api_key, "secret-bedrock");
        assert_eq!(bedrock.region, DEFAULT_BEDROCK_REGION);

        store.clear_all().unwrap();
        assert!(store.load_jira().unwrap().is_none());
        assert!(store.load_bedrock().unwrap().is_none());
    }

    #[test]
    fn debug_redacts_secrets() {
        let jira = JiraCredentials {
            site_url: "https://example.atlassian.net".into(),
            email: "dev@example.com".into(),
            api_token: "secret-jira".into(),
        };
        let bedrock = BedrockCredentials {
            api_key: "secret-bedrock".into(),
            region: "ap-southeast-2".into(),
        };
        let jira_dbg = format!("{jira:?}");
        let bedrock_dbg = format!("{bedrock:?}");
        assert!(!jira_dbg.contains("secret-jira"));
        assert!(jira_dbg.contains("[REDACTED]"));
        assert!(!bedrock_dbg.contains("secret-bedrock"));
        assert!(bedrock_dbg.contains("[REDACTED]"));
        assert!(bedrock_dbg.contains("ap-southeast-2"));
    }
}
