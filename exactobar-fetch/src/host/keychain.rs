//! Secure credential storage using the system keychain.
//!
//! This module provides access to the system's secure credential storage:
//! - macOS: Keychain Services
//! - Windows: Credential Manager
//! - Linux: Secret Service (GNOME Keyring, KDE Wallet)
//!
//! ## Caching
//!
//! To avoid multiple keychain password prompts on startup, this module provides
//! a caching layer. Use `get_password_cached()` for sync access that caches results.
//! The cache is global and persists for the lifetime of the application.

use std::collections::HashMap;
use std::sync::{Mutex, OnceLock};

use async_trait::async_trait;
use keyring::Entry;
use tracing::{debug, trace, warn};

use crate::error::KeychainError;

/// Service name prefix for `ExactoBar` credentials.
const SERVICE_PREFIX: &str = "exactobar";

// ============================================================================
// Keychain Cache (Global)
// ============================================================================

/// Global cache for keychain values to avoid repeated password prompts.
/// Key: "service:account", Value: Option<String> (None means "no entry found")
static KEYCHAIN_CACHE: OnceLock<Mutex<HashMap<String, Option<String>>>> = OnceLock::new();

/// Get the global keychain cache.
fn get_cache() -> &'static Mutex<HashMap<String, Option<String>>> {
    KEYCHAIN_CACHE.get_or_init(|| Mutex::new(HashMap::new()))
}

/// Get a password from the keychain with caching (sync).
///
/// This function caches keychain lookups to avoid multiple password prompts.
/// Once a value is cached (including "not found"), subsequent calls return
/// the cached value without hitting the keychain.
///
/// # Arguments
/// * `service` - Service identifier (e.g., "Chrome Safe Storage")
/// * `account` - Account identifier (e.g., "", "`api_key`")
///
/// # Returns
/// * `Some(password)` - Password found (from cache or keychain)
/// * `None` - No password found
///
/// # Panics
///
/// Panics if the cache mutex is poisoned (only happens if a thread panicked
/// while holding the lock, which indicates a serious bug).
///
/// # Example
/// ```ignore
/// use exactobar_fetch::host::keychain::get_password_cached;
///
/// if let Some(key) = get_password_cached("Chrome Safe Storage", "") {
///     // Use the key...
/// }
/// ```
pub fn get_password_cached(service: &str, account: &str) -> Option<String> {
    let cache_key = format!("{service}:{account}");

    // Check cache first
    {
        let cache = get_cache().lock().unwrap();
        if let Some(cached) = cache.get(&cache_key) {
            trace!(service = %service, account = %account, hit = true, "Keychain cache lookup");
            return cached.clone();
        }
    }

    trace!(service = %service, account = %account, hit = false, "Keychain cache miss, reading from keychain");

    // Not in cache, read from keychain
    let result = match Entry::new(service, account) {
        Ok(entry) => match entry.get_password() {
            Ok(password) if !password.is_empty() => Some(password),
            // Empty password or no entry both mean "not found"
            Ok(_) | Err(keyring::Error::NoEntry) => None,
            Err(e) => {
                warn!(service = %service, account = %account, error = %e, "Failed to get password from keychain");
                None
            }
        },
        Err(e) => {
            warn!(service = %service, account = %account, error = %e, "Failed to create keychain entry");
            None
        }
    };

    // Store in cache (even None values to avoid repeated lookups)
    {
        let mut cache = get_cache().lock().unwrap();
        cache.insert(cache_key, result.clone());
    }

    result
}

/// Invalidate a specific cache entry.
///
/// Call this when you know a credential has changed (e.g., after storing a new value).
pub fn invalidate_cache_entry(service: &str, account: &str) {
    let cache_key = format!("{service}:{account}");
    if let Ok(mut cache) = get_cache().lock() {
        cache.remove(&cache_key);
        debug!(service = %service, account = %account, "Invalidated keychain cache entry");
    }
}

/// Clear the entire keychain cache.
///
/// Call this if credentials may have changed externally (e.g., user edited keychain).
pub fn clear_cache() {
    if let Ok(mut cache) = get_cache().lock() {
        cache.clear();
        debug!("Cleared keychain cache");
    }
}

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
    /// * `account` - Account identifier (e.g., `api_key`, `oauth_token`)
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
        format!("{SERVICE_PREFIX}:{service}")
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
    /// Anthropic Claude service.
    pub const CLAUDE: &str = "claude";
    /// `OpenAI` service.
    pub const OPENAI: &str = "openai";
    /// Cursor IDE service.
    pub const CURSOR: &str = "cursor";
    /// Google Gemini service.
    pub const GEMINI: &str = "gemini";
    /// GitHub Copilot service.
    pub const COPILOT: &str = "copilot";
    /// GitHub service.
    pub const GITHUB: &str = "github";
    /// Google Vertex AI service.
    pub const VERTEXAI: &str = "vertexai";
    /// Factory AI service.
    pub const FACTORY: &str = "factory";
    /// z.ai service.
    pub const ZAI: &str = "zai";
    /// Augment Code service.
    pub const AUGMENT: &str = "augment";
    /// Kiro AI service.
    pub const KIRO: &str = "kiro";
    /// `MiniMax` service.
    pub const MINIMAX: &str = "minimax";
    /// Antigravity AI service.
    pub const ANTIGRAVITY: &str = "antigravity";
}

/// Common account names for credentials.
pub mod accounts {
    /// API key credential.
    pub const API_KEY: &str = "api_key";
    /// OAuth access token.
    pub const OAUTH_TOKEN: &str = "oauth_token";
    /// OAuth refresh token.
    pub const REFRESH_TOKEN: &str = "refresh_token";
    /// Session key credential.
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
