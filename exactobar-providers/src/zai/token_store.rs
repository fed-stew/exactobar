//! z.ai API token storage.
//!
//! This module handles loading and saving z.ai API tokens from various sources:
//!
//! 1. **Environment** - ZAI_API_TOKEN or ZAI_API_KEY
//! 2. **Keychain** - Secure storage using OS keychain (exactobar:zai)

use exactobar_fetch::host::keychain::{accounts, services, KeychainApi};
use tracing::{debug, instrument};

use super::error::ZaiError;

// ============================================================================
// Constants
// ============================================================================

/// Environment variable for z.ai token.
const ZAI_TOKEN_ENV: &str = "ZAI_API_TOKEN";

/// Alternative environment variable (key style).
const ZAI_KEY_ENV: &str = "ZAI_API_KEY";

/// Legacy keychain service names to check (for migration compatibility).
const LEGACY_KEYCHAIN_SERVICES: &[&str] = &[
    "exactobar:zai",
    "codexbar:zai",
    "zai:api",
];

/// Legacy keychain account name.
const LEGACY_KEYCHAIN_ACCOUNT: &str = "api_token";

// ============================================================================
// Token Store
// ============================================================================

/// z.ai token store.
///
/// Provides unified access to z.ai API tokens from multiple sources.
/// Priority: Environment > Keychain
#[derive(Debug, Clone, Default)]
pub struct ZaiTokenStore;

impl ZaiTokenStore {
    /// Creates a new token store.
    pub fn new() -> Self {
        Self
    }

    // ========================================================================
    // Async methods (using FetchContext keychain)
    // ========================================================================

    /// Load token from environment or keychain (async).
    ///
    /// Priority:
    /// 1. Environment variables (ZAI_API_TOKEN, ZAI_API_KEY)
    /// 2. Keychain (using provided keychain API)
    #[instrument(skip(keychain))]
    pub async fn load_async<K: KeychainApi + ?Sized>(
        keychain: &K,
    ) -> Option<String> {
        // Try environment first (fast path)
        if let Some(token) = Self::load_from_env() {
            debug!(source = "env", "Loaded z.ai token");
            return Some(token);
        }

        // Try keychain
        if let Some(token) = Self::load_from_keychain_async(keychain).await {
            debug!(source = "keychain", "Loaded z.ai token");
            return Some(token);
        }

        None
    }

    /// Load token from keychain using the async keychain API.
    #[instrument(skip(keychain))]
    pub async fn load_from_keychain_async<K: KeychainApi + ?Sized>(
        keychain: &K,
    ) -> Option<String> {
        // Try the standard exactobar keychain location
        if let Ok(Some(token)) = keychain.get(services::ZAI, accounts::API_KEY).await {
            if !token.is_empty() {
                return Some(token);
            }
        }

        None
    }

    /// Save token to keychain using the async keychain API.
    #[instrument(skip(keychain, token))]
    pub async fn save_to_keychain_async<K: KeychainApi + ?Sized>(
        keychain: &K,
        token: &str,
    ) -> Result<(), ZaiError> {
        keychain
            .set(services::ZAI, accounts::API_KEY, token)
            .await
            .map_err(|e| ZaiError::KeychainError(e.to_string()))?;

        debug!("z.ai token saved to keychain");
        Ok(())
    }

    /// Check if token is available (async).
    pub async fn has_token_async<K: KeychainApi + ?Sized>(keychain: &K) -> bool {
        Self::load_async(keychain).await.is_some()
    }

    // ========================================================================
    // Sync methods (using keyring directly - for use outside FetchContext)
    // ========================================================================

    /// Load token from any available source (sync).
    ///
    /// This is useful when you don't have access to the FetchContext.
    #[instrument]
    pub fn load() -> Option<String> {
        // Try environment first
        if let Some(token) = Self::load_from_env() {
            debug!(source = "env", "Loaded z.ai token");
            return Some(token);
        }

        // Try keychain (sync)
        if let Some(token) = Self::load_from_keychain_sync() {
            debug!(source = "keychain", "Loaded z.ai token");
            return Some(token);
        }

        None
    }

    /// Load token from environment variable.
    pub fn load_from_env() -> Option<String> {
        std::env::var(ZAI_TOKEN_ENV)
            .or_else(|_| std::env::var(ZAI_KEY_ENV))
            .ok()
            .filter(|t| !t.is_empty())
    }

    /// Load from keychain using keyring crate directly (sync).
    #[instrument]
    pub fn load_from_keychain_sync() -> Option<String> {
        // Try all known service names for compatibility
        for service in LEGACY_KEYCHAIN_SERVICES {
            if let Ok(entry) = keyring::Entry::new(service, LEGACY_KEYCHAIN_ACCOUNT) {
                if let Ok(token) = entry.get_password() {
                    if !token.is_empty() {
                        debug!(service = %service, "Found token in keychain");
                        return Some(token);
                    }
                }
            }
        }
        None
    }

    /// Save token to keychain using keyring crate directly (sync).
    #[instrument(skip(token))]
    pub fn save_to_keychain_sync(token: &str) -> Result<(), ZaiError> {
        let entry = keyring::Entry::new(LEGACY_KEYCHAIN_SERVICES[0], LEGACY_KEYCHAIN_ACCOUNT)
            .map_err(|e| ZaiError::KeychainError(e.to_string()))?;

        entry
            .set_password(token)
            .map_err(|e| ZaiError::KeychainError(e.to_string()))?;

        debug!("z.ai token saved to keychain");
        Ok(())
    }

    /// Delete token from keychain (sync).
    #[instrument]
    pub fn delete_from_keychain_sync() -> Result<(), ZaiError> {
        for service in LEGACY_KEYCHAIN_SERVICES {
            if let Ok(entry) = keyring::Entry::new(service, LEGACY_KEYCHAIN_ACCOUNT) {
                // Ignore errors - token might not exist in all locations
                let _ = entry.delete_credential();
            }
        }

        debug!("z.ai token deleted from keychain");
        Ok(())
    }

    /// Check if token is available (sync).
    pub fn is_available() -> bool {
        Self::load().is_some()
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
        let _store = ZaiTokenStore::new();
    }

    #[test]
    fn test_load_from_env() {
        // Just test the function runs without error
        let _ = ZaiTokenStore::load_from_env();
    }

    #[test]
    fn test_is_available() {
        // Just test it runs - actual availability depends on system
        let _ = ZaiTokenStore::is_available();
    }

    #[test]
    fn test_load_sync() {
        // Just test it runs - actual result depends on system state
        let _ = ZaiTokenStore::load();
    }
}
