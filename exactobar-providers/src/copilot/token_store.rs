//! Copilot token storage.
//!
//! This module handles loading and saving Copilot OAuth tokens from various sources:
//!
//! 1. **Keychain** - Secure storage using OS keychain
//! 2. **gh CLI** - Read token from GitHub CLI configuration
//! 3. **Environment** - COPILOT_API_TOKEN or GITHUB_TOKEN
//! 4. **File** - ~/.copilot/token.json

use std::fs::OpenOptions;
use std::io::Write;
use std::path::PathBuf;

#[cfg(unix)]
use std::os::unix::fs::OpenOptionsExt;

use serde::{Deserialize, Serialize};
use tracing::{debug, instrument, warn};

use super::error::CopilotError;

// ============================================================================
// Constants
// ============================================================================

/// Keychain service name for Copilot.
const KEYCHAIN_SERVICE: &str = "github.com/copilot";

/// Keychain account name.
const KEYCHAIN_ACCOUNT: &str = "oauth_token";

/// Alternative keychain service (GitHub CLI).
const GH_CLI_KEYCHAIN_SERVICE: &str = "gh:github.com";

/// Environment variable for Copilot token.
const COPILOT_TOKEN_ENV: &str = "COPILOT_API_TOKEN";

/// Alternative environment variable (GitHub token).
const GITHUB_TOKEN_ENV: &str = "GITHUB_TOKEN";

// ============================================================================
// Token File
// ============================================================================

/// Token stored in file.
#[derive(Debug, Serialize, Deserialize)]
pub struct StoredToken {
    /// The OAuth access token.
    pub access_token: String,

    /// Token type (usually "bearer").
    #[serde(default)]
    pub token_type: Option<String>,

    /// Scopes granted.
    #[serde(default)]
    pub scope: Option<String>,

    /// When the token was stored.
    #[serde(default)]
    pub stored_at: Option<String>,
}

// ============================================================================
// GitHub CLI Config
// ============================================================================

/// GitHub CLI hosts configuration.
#[derive(Debug, Deserialize)]
struct GhHosts {
    #[serde(rename = "github.com")]
    github: Option<GhHostConfig>,
}

/// GitHub CLI host configuration.
#[derive(Debug, Deserialize)]
struct GhHostConfig {
    oauth_token: Option<String>,
    #[allow(dead_code)]
    user: Option<String>,
}

// ============================================================================
// Token Store
// ============================================================================

/// Copilot token store.
#[derive(Debug, Clone, Default)]
pub struct CopilotTokenStore;

impl CopilotTokenStore {
    /// Creates a new token store.
    pub fn new() -> Self {
        Self
    }

    /// Load token from any available source.
    ///
    /// Priority:
    /// 1. Keychain
    /// 2. gh CLI
    /// 3. Environment
    /// 4. File
    #[instrument(skip(self))]
    pub fn load(&self) -> Option<String> {
        // Try keychain first
        if let Some(token) = self.load_from_keychain() {
            debug!(source = "keychain", "Loaded token");
            return Some(token);
        }

        // Try gh CLI
        if let Some(token) = self.load_from_gh_cli() {
            debug!(source = "gh_cli", "Loaded token");
            return Some(token);
        }

        // Try environment
        if let Some(token) = Self::load_from_env() {
            debug!(source = "env", "Loaded token");
            return Some(token);
        }

        // Try file
        if let Some(token) = self.load_from_file() {
            debug!(source = "file", "Loaded token");
            return Some(token);
        }

        None
    }

    /// Load token from OS keychain.
    #[instrument(skip(self))]
    pub fn load_from_keychain(&self) -> Option<String> {
        // Try Copilot-specific keychain entry
        if let Ok(entry) = keyring::Entry::new(KEYCHAIN_SERVICE, KEYCHAIN_ACCOUNT) {
            if let Ok(token) = entry.get_password() {
                if !token.is_empty() {
                    return Some(token);
                }
            }
        }

        // Try GitHub CLI keychain entry
        if let Ok(entry) = keyring::Entry::new(GH_CLI_KEYCHAIN_SERVICE, "") {
            if let Ok(token) = entry.get_password() {
                if !token.is_empty() {
                    return Some(token);
                }
            }
        }

        None
    }

    /// Load token from gh CLI configuration.
    #[instrument(skip(self))]
    pub fn load_from_gh_cli(&self) -> Option<String> {
        let hosts_path = Self::gh_cli_hosts_path()?;

        if !hosts_path.exists() {
            return None;
        }

        let content = std::fs::read_to_string(&hosts_path).ok()?;

        // gh CLI uses YAML format
        let hosts: GhHosts = serde_yaml::from_str(&content).ok()?;

        hosts.github?.oauth_token
    }

    /// Load token from environment variable.
    pub fn load_from_env() -> Option<String> {
        std::env::var(COPILOT_TOKEN_ENV)
            .or_else(|_| std::env::var(GITHUB_TOKEN_ENV))
            .ok()
            .filter(|t| !t.is_empty())
    }

    /// Load token from file.
    #[instrument(skip(self))]
    pub fn load_from_file(&self) -> Option<String> {
        let token_path = Self::token_file_path()?;

        if !token_path.exists() {
            return None;
        }

        let content = std::fs::read_to_string(&token_path).ok()?;
        let stored: StoredToken = serde_json::from_str(&content).ok()?;

        Some(stored.access_token)
    }

    /// Save token to keychain.
    #[instrument(skip(self, token))]
    pub fn save_to_keychain(&self, token: &str) -> Result<(), CopilotError> {
        let entry = keyring::Entry::new(KEYCHAIN_SERVICE, KEYCHAIN_ACCOUNT)
            .map_err(|e| CopilotError::KeychainError(e.to_string()))?;

        entry
            .set_password(token)
            .map_err(|e| CopilotError::KeychainError(e.to_string()))?;

        debug!("Token saved to keychain");
        Ok(())
    }

    /// Save token to file.
    #[instrument(skip(self, token))]
    pub fn save_to_file(&self, token: &str) -> Result<(), CopilotError> {
        let token_path = Self::token_file_path()
            .ok_or_else(|| CopilotError::KeychainError("Could not determine token path".into()))?;

        // Create directory if needed
        if let Some(parent) = token_path.parent() {
            std::fs::create_dir_all(parent).map_err(|e| {
                CopilotError::KeychainError(format!("Failed to create directory: {}", e))
            })?;
        }

        let stored = StoredToken {
            access_token: token.to_string(),
            token_type: Some("bearer".to_string()),
            scope: Some("copilot".to_string()),
            stored_at: Some(chrono::Utc::now().to_rfc3339()),
        };

        let content = serde_json::to_string_pretty(&stored).map_err(|e| {
            CopilotError::KeychainError(format!("Failed to serialize token: {}", e))
        })?;

        // Write with secure permissions (owner read/write only)
        #[cfg(unix)]
        let mut file = OpenOptions::new()
            .write(true)
            .create(true)
            .truncate(true)
            .mode(0o600) // Owner read/write only - security critical!
            .open(&token_path)
            .map_err(|e| {
                CopilotError::KeychainError(format!("Failed to open token file: {}", e))
            })?;

        #[cfg(not(unix))]
        let mut file = OpenOptions::new()
            .write(true)
            .create(true)
            .truncate(true)
            .open(&token_path)
            .map_err(|e| {
                CopilotError::KeychainError(format!("Failed to open token file: {}", e))
            })?;

        file.write_all(content.as_bytes()).map_err(|e| {
            CopilotError::KeychainError(format!("Failed to write token file: {}", e))
        })?;

        debug!(path = %token_path.display(), "Token saved to file");
        Ok(())
    }

    /// Delete token from keychain.
    #[instrument(skip(self))]
    pub fn delete_from_keychain(&self) -> Result<(), CopilotError> {
        let entry = keyring::Entry::new(KEYCHAIN_SERVICE, KEYCHAIN_ACCOUNT)
            .map_err(|e| CopilotError::KeychainError(e.to_string()))?;

        entry
            .delete_credential()
            .map_err(|e| CopilotError::KeychainError(e.to_string()))?;

        debug!("Token deleted from keychain");
        Ok(())
    }

    /// Get the path to the token file.
    pub fn token_file_path() -> Option<PathBuf> {
        let home = dirs::home_dir()?;
        Some(home.join(".copilot").join("token.json"))
    }

    /// Get the path to gh CLI hosts file.
    fn gh_cli_hosts_path() -> Option<PathBuf> {
        // Try XDG config first
        if let Some(config_dir) = dirs::config_dir() {
            let path = config_dir.join("gh").join("hosts.yml");
            if path.exists() {
                return Some(path);
            }
        }

        // Try home directory
        let home = dirs::home_dir()?;
        let path = home.join(".config").join("gh").join("hosts.yml");
        if path.exists() {
            return Some(path);
        }

        // Return the expected path even if it doesn't exist
        Some(path)
    }

    /// Check if any token source is available.
    pub fn is_available(&self) -> bool {
        self.load().is_some()
    }
}

// ============================================================================
// Tests
// ============================================================================

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_store_creation() {
        let store = CopilotTokenStore::new();
        assert!(std::mem::size_of_val(&store) == 0); // Zero-sized type
    }

    #[test]
    fn test_token_file_path() {
        let path = CopilotTokenStore::token_file_path();
        assert!(path.is_some());
        let path = path.unwrap();
        assert!(path.ends_with("token.json"));
    }

    #[test]
    fn test_load_from_env() {
        // Just test the function runs without error
        // We don't modify env vars to avoid unsafe blocks and test isolation issues
        let _ = CopilotTokenStore::load_from_env();
    }

    #[test]
    fn test_parse_stored_token() {
        let json = r#"{
            "access_token": "gho_abc123",
            "token_type": "bearer",
            "scope": "copilot",
            "stored_at": "2024-01-01T00:00:00Z"
        }"#;

        let token: StoredToken = serde_json::from_str(json).unwrap();
        assert_eq!(token.access_token, "gho_abc123");
        assert_eq!(token.token_type, Some("bearer".to_string()));
    }

    #[test]
    fn test_is_available() {
        let store = CopilotTokenStore::new();
        // Just test it runs - actual availability depends on system
        let _ = store.is_available();
    }
}
