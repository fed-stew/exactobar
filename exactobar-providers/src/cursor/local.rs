//! Cursor local settings and cache reader.
//!
//! This module reads Cursor's local configuration files to extract
//! cached usage information without requiring network access.

use std::path::PathBuf;

use exactobar_core::{LoginMethod, ProviderIdentity, ProviderKind, UsageSnapshot};
use serde::Deserialize;
use tracing::{debug, instrument, warn};

use super::error::CursorError;

// ============================================================================
// Local Config Structures
// ============================================================================

/// Cursor's storage.json structure.
#[allow(dead_code)]
#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct CursorStorage {
    /// User account info.
    #[serde(default, alias = "cursorAuth/cachedUser")]
    pub user: Option<CursorLocalUser>,

    /// Usage stats.
    #[serde(default, alias = "cursorAuth/cachedUsage")]
    pub usage: Option<CursorLocalUsage>,

    /// Subscription info.
    #[serde(default, alias = "cursorAuth/cachedSubscription")]
    pub subscription: Option<CursorLocalSubscription>,
}

/// Cursor local user info.
#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct CursorLocalUser {
    /// User email.
    #[serde(default)]
    pub email: Option<String>,

    /// User name.
    #[serde(default)]
    #[allow(dead_code)]
    pub name: Option<String>,

    /// User ID.
    #[serde(default)]
    #[allow(dead_code)]
    pub id: Option<String>,
}

/// Cursor local usage info.
#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct CursorLocalUsage {
    /// GPT-4 requests used.
    #[serde(default, alias = "gpt4_requests", alias = "numRequests")]
    pub gpt4_requests: Option<u64>,

    /// GPT-4 request limit.
    #[serde(default, alias = "gpt4_limit", alias = "maxRequests")]
    pub gpt4_limit: Option<u64>,

    /// Slow requests used.
    #[serde(default, alias = "numSlowRequests")]
    pub slow_requests: Option<u64>,

    /// Slow request limit.
    #[serde(default, alias = "maxSlowRequests")]
    pub slow_limit: Option<u64>,

    /// Last updated timestamp.
    #[serde(default)]
    #[allow(dead_code)]
    pub last_updated: Option<String>,
}

impl CursorLocalUsage {
    /// Check if we have enough data to compute usage.
    pub fn has_data(&self) -> bool {
        self.gpt4_requests.is_some() && self.gpt4_limit.is_some()
    }

    /// Get the primary usage percentage.
    pub fn get_primary_percent(&self) -> Option<f64> {
        let used = self.gpt4_requests?;
        let limit = self.gpt4_limit?;
        if limit > 0 {
            Some((used as f64 / limit as f64) * 100.0)
        } else {
            None
        }
    }

    /// Get the secondary usage percentage.
    pub fn get_secondary_percent(&self) -> Option<f64> {
        let used = self.slow_requests?;
        let limit = self.slow_limit?;
        if limit > 0 {
            Some((used as f64 / limit as f64) * 100.0)
        } else {
            None
        }
    }
}

/// Cursor local subscription info.
#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct CursorLocalSubscription {
    /// Plan name.
    #[serde(default)]
    pub plan: Option<String>,

    /// Whether subscription is active.
    #[serde(default)]
    #[allow(dead_code)]
    pub is_active: Option<bool>,

    /// Subscription end date.
    #[serde(default)]
    #[allow(dead_code)]
    pub end_date: Option<String>,
}

// ============================================================================
// Local Reader
// ============================================================================

/// Cursor local settings reader.
#[derive(Debug, Clone, Default)]
pub struct CursorLocalReader;

impl CursorLocalReader {
    /// Creates a new local reader.
    pub fn new() -> Self {
        Self
    }

    /// Get the Cursor config directory.
    #[cfg(target_os = "macos")]
    pub fn config_dir() -> Option<PathBuf> {
        let home = dirs::home_dir()?;
        Some(home.join("Library/Application Support/Cursor"))
    }

    #[cfg(target_os = "linux")]
    pub fn config_dir() -> Option<PathBuf> {
        let home = dirs::home_dir()?;
        Some(home.join(".config/Cursor"))
    }

    #[cfg(target_os = "windows")]
    pub fn config_dir() -> Option<PathBuf> {
        let app_data = dirs::data_local_dir()?;
        Some(app_data.join("Cursor"))
    }

    /// Check if Cursor is installed.
    pub fn is_installed() -> bool {
        Self::config_dir().is_some_and(|p| p.exists())
    }

    /// Get the storage.json path.
    pub fn storage_path() -> Option<PathBuf> {
        Self::config_dir().map(|p| p.join("User/globalStorage/storage.json"))
    }

    /// Get the state.vscdb path (SQLite state database).
    pub fn state_db_path() -> Option<PathBuf> {
        Self::config_dir().map(|p| p.join("User/globalStorage/state.vscdb"))
    }

    /// Read cached usage from local storage.
    #[instrument(skip(self))]
    pub fn read_cached_usage(&self) -> Result<UsageSnapshot, CursorError> {
        debug!("Reading Cursor local storage");

        // Try storage.json first
        if let Some(storage_path) = Self::storage_path() {
            if storage_path.exists() {
                debug!(path = %storage_path.display(), "Reading storage.json");

                let content = std::fs::read_to_string(&storage_path).map_err(|e| {
                    CursorError::ConfigParseError(format!("Failed to read: {}", e))
                })?;

                if let Ok(snapshot) = self.parse_storage_json(&content) {
                    return Ok(snapshot);
                }
            }
        }

        // Try state.vscdb (SQLite)
        if let Some(db_path) = Self::state_db_path() {
            if db_path.exists() {
                debug!(path = %db_path.display(), "Reading state.vscdb");

                if let Ok(snapshot) = self.read_state_db(&db_path) {
                    return Ok(snapshot);
                }
            }
        }

        Err(CursorError::ConfigNotFound(
            "No local Cursor data found".to_string(),
        ))
    }

    /// Parse storage.json content.
    fn parse_storage_json(&self, content: &str) -> Result<UsageSnapshot, CursorError> {
        // The storage.json has keys like "cursorAuth/cachedUser" as top-level keys
        // We need to try parsing it as a flat JSON object
        let raw: serde_json::Value = serde_json::from_str(content).map_err(|e| {
            CursorError::ConfigParseError(format!("Invalid JSON: {}", e))
        })?;

        let mut snapshot = UsageSnapshot::new();
        snapshot.fetch_source = exactobar_core::FetchSource::LocalProbe;

        // Try to extract user info
        if let Some(user_str) = raw.get("cursorAuth/cachedUser").and_then(|v| v.as_str()) {
            if let Ok(user) = serde_json::from_str::<CursorLocalUser>(user_str) {
                let mut identity = ProviderIdentity::new(ProviderKind::Cursor);
                identity.account_email = user.email;
                identity.login_method = Some(LoginMethod::CLI);
                snapshot.identity = Some(identity);
            }
        }

        // Try to extract usage info
        if let Some(usage_str) = raw.get("cursorAuth/cachedUsage").and_then(|v| v.as_str()) {
            if let Ok(usage) = serde_json::from_str::<CursorLocalUsage>(usage_str) {
                if usage.has_data() {
                    if let Some(percent) = usage.get_primary_percent() {
                        snapshot.primary = Some(exactobar_core::UsageWindow::new(percent));
                    }

                    if let Some(percent) = usage.get_secondary_percent() {
                        snapshot.secondary = Some(exactobar_core::UsageWindow::new(percent));
                    }
                }
            }
        }

        // Try to extract subscription info
        if let Some(sub_str) = raw.get("cursorAuth/cachedSubscription").and_then(|v| v.as_str()) {
            if let Ok(sub) = serde_json::from_str::<CursorLocalSubscription>(sub_str) {
                if let Some(ref mut identity) = snapshot.identity {
                    identity.plan_name = sub.plan;
                }
            }
        }

        Ok(snapshot)
    }

    /// Read from state.vscdb SQLite database.
    fn read_state_db(&self, db_path: &PathBuf) -> Result<UsageSnapshot, CursorError> {
        use rusqlite::{Connection, OpenFlags};

        // Copy to temp to avoid locking
        let temp_dir = std::env::temp_dir();
        let temp_path = temp_dir.join(format!("cursor_state_{}.db", std::process::id()));

        std::fs::copy(db_path, &temp_path).map_err(|e| {
            CursorError::ConfigParseError(format!("Failed to copy db: {}", e))
        })?;

        let conn = Connection::open_with_flags(&temp_path, OpenFlags::SQLITE_OPEN_READ_ONLY)
            .map_err(|e| CursorError::ConfigParseError(format!("SQLite error: {}", e)))?;

        // Try to read state table
        let mut snapshot = UsageSnapshot::new();
        snapshot.fetch_source = exactobar_core::FetchSource::LocalProbe;

        // Look for cursorAuth keys
        let mut stmt = conn
            .prepare("SELECT key, value FROM ItemTable WHERE key LIKE 'cursorAuth%'")
            .map_err(|e| CursorError::ConfigParseError(format!("Query error: {}", e)))?;

        let rows = stmt
            .query_map([], |row| {
                let key: String = row.get(0)?;
                let value: String = row.get(1)?;
                Ok((key, value))
            })
            .map_err(|e| CursorError::ConfigParseError(format!("Query error: {}", e)))?;

        for row in rows.flatten() {
            let (key, value) = row;

            match key.as_str() {
                "cursorAuth/cachedUser" => {
                    if let Ok(user) = serde_json::from_str::<CursorLocalUser>(&value) {
                        let mut identity = ProviderIdentity::new(ProviderKind::Cursor);
                        identity.account_email = user.email;
                        identity.login_method = Some(LoginMethod::CLI);
                        snapshot.identity = Some(identity);
                    }
                }
                "cursorAuth/cachedUsage" => {
                    if let Ok(usage) = serde_json::from_str::<CursorLocalUsage>(&value) {
                        if let Some(percent) = usage.get_primary_percent() {
                            snapshot.primary =
                                Some(exactobar_core::UsageWindow::new(percent));
                        }
                    }
                }
                _ => {}
            }
        }

        // Clean up
        let _ = std::fs::remove_file(&temp_path);

        Ok(snapshot)
    }
}

// ============================================================================
// Tests
// ============================================================================

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_config_dir() {
        // Should return Some on all platforms
        let dir = CursorLocalReader::config_dir();
        assert!(dir.is_some());
    }

    #[test]
    fn test_is_installed() {
        // Just test it runs without error
        let _ = CursorLocalReader::is_installed();
    }

    #[test]
    fn test_local_usage_has_data() {
        let usage = CursorLocalUsage {
            gpt4_requests: Some(100),
            gpt4_limit: Some(500),
            slow_requests: None,
            slow_limit: None,
            last_updated: None,
        };
        assert!(usage.has_data());

        let empty = CursorLocalUsage {
            gpt4_requests: None,
            gpt4_limit: None,
            slow_requests: None,
            slow_limit: None,
            last_updated: None,
        };
        assert!(!empty.has_data());
    }

    #[test]
    fn test_local_usage_percent() {
        let usage = CursorLocalUsage {
            gpt4_requests: Some(100),
            gpt4_limit: Some(500),
            slow_requests: Some(25),
            slow_limit: Some(100),
            last_updated: None,
        };

        assert_eq!(usage.get_primary_percent(), Some(20.0));
        assert_eq!(usage.get_secondary_percent(), Some(25.0));
    }

    #[test]
    fn test_parse_storage_json() {
        let reader = CursorLocalReader::new();

        // Test with nested JSON strings (how Cursor actually stores it)
        let json = r#"{
            "cursorAuth/cachedUser": "{\"email\":\"user@example.com\",\"name\":\"Test\"}",
            "cursorAuth/cachedUsage": "{\"gpt4_requests\":100,\"gpt4_limit\":500}"
        }"#;

        let snapshot = reader.parse_storage_json(json).unwrap();
        assert!(snapshot.identity.is_some());
        assert_eq!(
            snapshot.identity.unwrap().account_email,
            Some("user@example.com".to_string())
        );
    }
}
