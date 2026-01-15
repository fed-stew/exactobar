//! Secure credential storage using the system keychain.
//!
//! This module provides access to the system's secure credential storage:
//! - macOS: Keychain Services
//! - Windows: Credential Manager
//! - Linux: Secret Service (GNOME Keyring, KDE Wallet)

use async_trait::async_trait;
use keyring::Entry;
use tracing::{debug, warn};

use crate::error::KeychainError;

/// Service name prefix for ExactoBar credentials.
const SERVICE_PREFIX: &str = "exactobar";

// ============================================================================
// Keychain API Trait
// ============================================================================

/// API for secure credential storage.
///
/// Uses the system keychain (macOS Keychain, Windows Credential Manager,
/// Linux Secret Service) for secure storage of API keys and tokens.
#[async_trait]
pub trait KeychainApi: Send + Sync {
    /// Get a credential from the keychain.
    ///
    /// # Arguments
    /// * `service` - Service identifier (e.g., "claude", "openai")
    /// * `account` - Account identifier (e.g., "api_key", "oauth_token")
    ///
    /// # Returns
    /// * `Ok(Some(secret))` - Credential found
    /// * `Ok(None)` - Credential not found
    /// * `Err(e)` - Error accessing keychain
    async fn get(&self, service: &str, account: &str) -> Result<Option<String>, KeychainError>;

    /// Set a credential in the keychain.
    ///
    /// # Arguments
    /// * `service` - Service identifier
    /// * `account` - Account identifier
    /// * `secret` - The secret to store
    async fn set(&self, service: &str, account: &str, secret: &str) -> Result<(), KeychainError>;

    /// Delete a credential from the keychain.
    ///
    /// # Arguments
    /// * `service` - Service identifier
    /// * `account` - Account identifier
    async fn delete(&self, service: &str, account: &str) -> Result<(), KeychainError>;

    /// Check if a credential exists.
    async fn exists(&self, service: &str, account: &str) -> bool {
        matches!(self.get(service, account).await, Ok(Some(_)))
    }
}

// ============================================================================
// System Keychain Implementation
// ============================================================================

/// Default implementation using the system keychain.
///
/// This uses the `keyring` crate which provides cross-platform access to:
/// - macOS Keychain Services
/// - Windows Credential Manager
/// - Linux Secret Service API
#[derive(Debug, Clone, Default)]
pub struct SystemKeychain;

impl SystemKeychain {
    /// Creates a new system keychain instance.
    pub fn new() -> Self {
        Self
    }

    /// Builds the full service name with prefix.
    fn full_service(service: &str) -> String {
        format!("{}:{}", SERVICE_PREFIX, service)
    }

    /// Creates a keyring entry.
    fn entry(service: &str, account: &str) -> Result<Entry, KeychainError> {
        let full_service = Self::full_service(service);
        Entry::new(&full_service, account).map_err(|e| KeychainError::Platform(e.to_string()))
    }
}

#[async_trait]
impl KeychainApi for SystemKeychain {
    async fn get(&self, service: &str, account: &str) -> Result<Option<String>, KeychainError> {
        debug!(service = %service, account = %account, "Getting credential from keychain");

        let entry = Self::entry(service, account)?;

        match entry.get_password() {
            Ok(secret) => {
                debug!(service = %service, account = %account, "Credential found");
                Ok(Some(secret))
            }
            Err(keyring::Error::NoEntry) => {
                debug!(service = %service, account = %account, "Credential not found");
                Ok(None)
            }
            Err(e) => {
                warn!(service = %service, account = %account, error = %e, "Failed to get credential");
                Err(e.into())
            }
        }
    }

    async fn set(&self, service: &str, account: &str, secret: &str) -> Result<(), KeychainError> {
        debug!(service = %service, account = %account, "Setting credential in keychain");

        let entry = Self::entry(service, account)?;

        entry.set_password(secret).map_err(|e| {
            warn!(service = %service, account = %account, error = %e, "Failed to set credential");
            KeychainError::from(e)
        })?;

        debug!(service = %service, account = %account, "Credential stored successfully");
        Ok(())
    }

    async fn delete(&self, service: &str, account: &str) -> Result<(), KeychainError> {
        debug!(service = %service, account = %account, "Deleting credential from keychain");

        let entry = Self::entry(service, account)?;

        match entry.delete_credential() {
            Ok(()) => {
                debug!(service = %service, account = %account, "Credential deleted");
                Ok(())
            }
            Err(keyring::Error::NoEntry) => {
                debug!(service = %service, account = %account, "Credential not found (already deleted)");
                Ok(())
            }
            Err(e) => {
                warn!(service = %service, account = %account, error = %e, "Failed to delete credential");
                Err(e.into())
            }
        }
    }
}

// ============================================================================
// Common Credential Keys
// ============================================================================

/// Common service names for providers.
pub mod services {
    pub const CLAUDE: &str = "claude";
    pub const OPENAI: &str = "openai";
    pub const CURSOR: &str = "cursor";
    pub const GEMINI: &str = "gemini";
    pub const COPILOT: &str = "copilot";
    pub const GITHUB: &str = "github";
    pub const VERTEXAI: &str = "vertexai";
    pub const FACTORY: &str = "factory";
    pub const ZAI: &str = "zai";
    pub const AUGMENT: &str = "augment";
    pub const KIRO: &str = "kiro";
    pub const MINIMAX: &str = "minimax";
    pub const ANTIGRAVITY: &str = "antigravity";
}

/// Common account names for credentials.
pub mod accounts {
    pub const API_KEY: &str = "api_key";
    pub const OAUTH_TOKEN: &str = "oauth_token";
    pub const REFRESH_TOKEN: &str = "refresh_token";
    pub const SESSION_KEY: &str = "session_key";
}

// ============================================================================
// Tests
// ============================================================================

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_full_service_name() {
        assert_eq!(SystemKeychain::full_service("claude"), "exactobar:claude");
        assert_eq!(SystemKeychain::full_service("openai"), "exactobar:openai");
    }

    // Note: Actual keychain tests require platform access and are typically
    // run as integration tests, not unit tests.
}
