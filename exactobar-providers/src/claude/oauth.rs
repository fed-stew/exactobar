//! Claude OAuth credential management.
//!
//! Claude CLI stores OAuth credentials in two locations:
//!
//! 1. **macOS Keychain**: service="Claude Code-credentials"
//! 2. **File**: `~/.claude/.credentials.json`
//!
//! # Credentials Format
//!
//! ```json
//! {
//!   "claudeAiOauth": {
//!     "accessToken": "...",
//!     "refreshToken": "...",
//!     "expiresAt": 1735000000000,
//!     "scopes": ["user:profile", "..."]
//!   }
//! }
//! ```

use chrono::{DateTime, TimeZone, Utc};
use serde::Deserialize;
use std::fs;
use std::path::PathBuf;
use tracing::{debug, instrument, warn};

use super::error::ClaudeError;

// ============================================================================
// Constants
// ============================================================================

/// Keychain service name for Claude CLI.
pub const KEYCHAIN_SERVICE: &str = "Claude Code-credentials";

/// Keychain account name (empty for Claude).
pub const KEYCHAIN_ACCOUNT: &str = "";

/// Legacy keychain service name.
pub const LEGACY_KEYCHAIN_SERVICE: &str = "claude.ai";

/// Legacy keychain account name.
pub const LEGACY_KEYCHAIN_ACCOUNT: &str = "oauth_token";

/// Scope required for usage API.
pub const REQUIRED_SCOPE: &str = "user:profile";

// ============================================================================
// Credentials File Structures
// ============================================================================

/// Root structure of .credentials.json file.
#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct CredentialsFile {
    /// Claude AI OAuth credentials.
    pub claude_ai_oauth: Option<OAuthCredentialsData>,
}

/// OAuth credentials data from file/keychain.
#[derive(Debug, Clone, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct OAuthCredentialsData {
    /// Access token.
    pub access_token: String,
    /// Refresh token.
    pub refresh_token: Option<String>,
    /// Expiration timestamp (milliseconds since epoch).
    pub expires_at: Option<i64>,
    /// Granted scopes.
    pub scopes: Option<Vec<String>>,
    /// Rate limit tier.
    pub rate_limit_tier: Option<String>,
}

// ============================================================================
// OAuth Credentials
// ============================================================================

/// Validated OAuth credentials ready for use.
#[derive(Debug, Clone)]
pub struct ClaudeOAuthCredentials {
    /// Access token.
    pub access_token: String,
    /// Refresh token (if available).
    pub refresh_token: Option<String>,
    /// Expiration time.
    pub expires_at: Option<DateTime<Utc>>,
    /// Granted scopes.
    pub scopes: Vec<String>,
    /// Rate limit tier (e.g., "free", "pro").
    pub rate_limit_tier: Option<String>,
    /// Source of the credentials.
    pub source: CredentialSource,
}

/// Where credentials were loaded from.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum CredentialSource {
    /// macOS Keychain.
    Keychain,
    /// Legacy keychain entry.
    LegacyKeychain,
    /// File (~/.claude/.credentials.json).
    File,
}

impl ClaudeOAuthCredentials {
    /// Check if the credentials are expired.
    pub fn is_expired(&self) -> bool {
        if let Some(expires_at) = self.expires_at {
            let now = Utc::now();
            // Add 5 minute buffer
            expires_at <= now + chrono::Duration::minutes(5)
        } else {
            // No expiration = assume valid
            false
        }
    }

    /// Check if the credentials have a specific scope.
    pub fn has_scope(&self, scope: &str) -> bool {
        self.scopes.iter().any(|s| s == scope)
    }

    /// Check if credentials have the required scope for usage API.
    pub fn has_required_scope(&self) -> bool {
        self.has_scope(REQUIRED_SCOPE)
    }

    /// Check if credentials are valid for use.
    pub fn is_valid(&self) -> bool {
        !self.is_expired() && (self.scopes.is_empty() || self.has_required_scope())
    }

    /// Load credentials from all sources, returning the first valid one.
    ///
    /// Priority order (file first to avoid keychain password prompts!):
    /// 1. File (~/.claude/.credentials.json) - no password prompt
    /// 2. Keychain (Claude Code-credentials) - may prompt
    /// 3. Legacy keychain (claude.ai) - may prompt
    #[instrument]
    pub fn load() -> Result<Self, ClaudeError> {
        // Try file FIRST (no password prompt!)
        if let Ok(creds) = Self::load_from_file() {
            if creds.is_valid() {
                debug!(source = ?creds.source, "Loaded valid credentials from file");
                return Ok(creds);
            } else {
                warn!("File credentials exist but are invalid/expired");
            }
        }

        // Then try keychain (may prompt for password)
        if let Ok(creds) = Self::load_from_keychain() {
            if creds.is_valid() {
                debug!(source = ?creds.source, "Loaded valid credentials from keychain");
                return Ok(creds);
            } else {
                warn!("Keychain credentials exist but are invalid/expired");
            }
        }

        // Legacy keychain last (may prompt for password)
        if let Ok(creds) = Self::load_from_legacy_keychain() {
            if creds.is_valid() {
                debug!(source = ?creds.source, "Loaded valid credentials from legacy keychain");
                return Ok(creds);
            }
        }

        Err(ClaudeError::CredentialsNotFound)
    }

    /// Load credentials from macOS Keychain.
    #[instrument]
    pub fn load_from_keychain() -> Result<Self, ClaudeError> {
        use exactobar_fetch::host::keychain::get_password_cached;

        debug!("Trying to load from keychain");

        // Try with current username first (Claude CLI stores credentials this way)
        let username = whoami::username();
        debug!(account = %username, "Trying keychain with username");
        if let Some(secret) = get_password_cached(KEYCHAIN_SERVICE, &username) {
            debug!("Found credentials with username account");
            return Self::parse_credentials(&secret, CredentialSource::Keychain);
        }

        // Fall back to empty account (legacy)
        debug!("Trying keychain with empty account");
        if let Some(secret) = get_password_cached(KEYCHAIN_SERVICE, KEYCHAIN_ACCOUNT) {
            return Self::parse_credentials(&secret, CredentialSource::Keychain);
        }

        Err(ClaudeError::CredentialsNotFound)
    }

    /// Load credentials from legacy keychain entry.
    fn load_from_legacy_keychain() -> Result<Self, ClaudeError> {
        use exactobar_fetch::host::keychain::get_password_cached;

        debug!("Trying to load from legacy keychain");

        let secret = get_password_cached(LEGACY_KEYCHAIN_SERVICE, LEGACY_KEYCHAIN_ACCOUNT)
            .ok_or(ClaudeError::CredentialsNotFound)?;

        // Legacy format might be just a token or JSON
        if secret.starts_with('{') {
            Self::parse_credentials(&secret, CredentialSource::LegacyKeychain)
        } else {
            // Just a raw token
            Ok(Self {
                access_token: secret,
                refresh_token: None,
                expires_at: None,
                scopes: vec![],
                rate_limit_tier: None,
                source: CredentialSource::LegacyKeychain,
            })
        }
    }

    /// Load credentials from file.
    #[instrument]
    pub fn load_from_file() -> Result<Self, ClaudeError> {
        let path = credentials_file_path().ok_or_else(|| {
            ClaudeError::CredentialsLoadError("Could not determine home directory".to_string())
        })?;

        if !path.exists() {
            return Err(ClaudeError::CredentialsNotFound);
        }

        debug!(path = %path.display(), "Reading credentials file");

        let content = fs::read_to_string(&path)
            .map_err(|e| ClaudeError::CredentialsLoadError(e.to_string()))?;

        Self::parse_credentials(&content, CredentialSource::File)
    }

    /// Parse credentials from JSON string.
    fn parse_credentials(json: &str, source: CredentialSource) -> Result<Self, ClaudeError> {
        // Try the full credentials file format
        if let Ok(creds_file) = serde_json::from_str::<CredentialsFile>(json) {
            if let Some(oauth) = creds_file.claude_ai_oauth {
                return Ok(Self::from_data(oauth, source));
            }
        }

        // Try direct OAuth data format
        if let Ok(oauth) = serde_json::from_str::<OAuthCredentialsData>(json) {
            return Ok(Self::from_data(oauth, source));
        }

        Err(ClaudeError::CredentialsLoadError(
            "Invalid credentials format".to_string(),
        ))
    }

    /// Convert raw data to validated credentials.
    fn from_data(data: OAuthCredentialsData, source: CredentialSource) -> Self {
        let expires_at = data.expires_at.and_then(|ts| {
            // Timestamp might be in milliseconds
            let secs = if ts > 10_000_000_000 { ts / 1000 } else { ts };
            Utc.timestamp_opt(secs, 0).single()
        });

        Self {
            access_token: data.access_token,
            refresh_token: data.refresh_token,
            expires_at,
            scopes: data.scopes.unwrap_or_default(),
            rate_limit_tier: data.rate_limit_tier,
            source,
        }
    }
}

// ============================================================================
// Helper Functions
// ============================================================================

/// Returns the path to the credentials file.
pub fn credentials_file_path() -> Option<PathBuf> {
    dirs::home_dir().map(|h| h.join(".claude").join(".credentials.json"))
}

/// Check if credentials file exists.
#[allow(dead_code)]
pub fn credentials_file_exists() -> bool {
    credentials_file_path().is_some_and(|p| p.exists())
}

/// Check if any OAuth credentials are available.
#[allow(dead_code)]
pub fn oauth_available() -> bool {
    ClaudeOAuthCredentials::load().is_ok()
}

// ============================================================================
// Tests
// ============================================================================

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_parse_credentials_full() {
        let json = r#"{
            "claudeAiOauth": {
                "accessToken": "test-token",
                "refreshToken": "refresh-token",
                "expiresAt": 1735000000000,
                "scopes": ["user:profile", "conversations:read"],
                "rateLimitTier": "pro"
            }
        }"#;

        let creds =
            ClaudeOAuthCredentials::parse_credentials(json, CredentialSource::File).unwrap();

        assert_eq!(creds.access_token, "test-token");
        assert_eq!(creds.refresh_token, Some("refresh-token".to_string()));
        assert!(creds.expires_at.is_some());
        assert!(creds.has_scope("user:profile"));
        assert!(creds.has_required_scope());
        assert_eq!(creds.rate_limit_tier, Some("pro".to_string()));
    }

    #[test]
    fn test_parse_credentials_direct() {
        let json = r#"{
            "accessToken": "direct-token",
            "expiresAt": 1735000000
        }"#;

        let creds =
            ClaudeOAuthCredentials::parse_credentials(json, CredentialSource::Keychain).unwrap();
        assert_eq!(creds.access_token, "direct-token");
    }

    #[test]
    fn test_is_expired_future() {
        let future = Utc::now() + chrono::Duration::hours(1);
        let creds = ClaudeOAuthCredentials {
            access_token: "token".to_string(),
            refresh_token: None,
            expires_at: Some(future),
            scopes: vec![],
            rate_limit_tier: None,
            source: CredentialSource::File,
        };
        assert!(!creds.is_expired());
    }

    #[test]
    fn test_is_expired_past() {
        let past = Utc::now() - chrono::Duration::hours(1);
        let creds = ClaudeOAuthCredentials {
            access_token: "token".to_string(),
            refresh_token: None,
            expires_at: Some(past),
            scopes: vec![],
            rate_limit_tier: None,
            source: CredentialSource::File,
        };
        assert!(creds.is_expired());
    }

    #[test]
    fn test_is_expired_none() {
        let creds = ClaudeOAuthCredentials {
            access_token: "token".to_string(),
            refresh_token: None,
            expires_at: None,
            scopes: vec![],
            rate_limit_tier: None,
            source: CredentialSource::File,
        };
        assert!(!creds.is_expired());
    }

    #[test]
    fn test_has_scope() {
        let creds = ClaudeOAuthCredentials {
            access_token: "token".to_string(),
            refresh_token: None,
            expires_at: None,
            scopes: vec!["user:profile".to_string(), "conversations:read".to_string()],
            rate_limit_tier: None,
            source: CredentialSource::File,
        };
        assert!(creds.has_scope("user:profile"));
        assert!(creds.has_scope("conversations:read"));
        assert!(!creds.has_scope("admin:write"));
    }

    #[test]
    fn test_credentials_file_path() {
        let path = credentials_file_path();
        assert!(path.is_some());
        assert!(path.unwrap().ends_with(".credentials.json"));
    }

    #[test]
    fn test_is_valid_with_scope() {
        let creds = ClaudeOAuthCredentials {
            access_token: "token".to_string(),
            refresh_token: None,
            expires_at: None,
            scopes: vec!["user:profile".to_string()],
            rate_limit_tier: None,
            source: CredentialSource::File,
        };
        assert!(creds.is_valid());
    }

    #[test]
    fn test_is_valid_no_scopes() {
        // Empty scopes = assume valid (legacy tokens)
        let creds = ClaudeOAuthCredentials {
            access_token: "token".to_string(),
            refresh_token: None,
            expires_at: None,
            scopes: vec![],
            rate_limit_tier: None,
            source: CredentialSource::File,
        };
        assert!(creds.is_valid());
    }

    #[test]
    fn test_is_valid_wrong_scope() {
        let creds = ClaudeOAuthCredentials {
            access_token: "token".to_string(),
            refresh_token: None,
            expires_at: None,
            scopes: vec!["other:scope".to_string()],
            rate_limit_tier: None,
            source: CredentialSource::File,
        };
        assert!(!creds.is_valid());
    }
}
