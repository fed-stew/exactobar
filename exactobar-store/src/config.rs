//! Configuration management.

use crate::error::StoreError;
use exactobar_core::ProviderKind;
use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use std::path::{Path, PathBuf};
use tracing::{debug, info};

/// Application configuration.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Config {
    /// General settings.
    #[serde(default)]
    pub general: GeneralConfig,
    /// Provider-specific configurations.
    #[serde(default)]
    pub providers: HashMap<String, ProviderConfig>,
}

/// General application settings.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct GeneralConfig {
    /// Refresh interval in seconds.
    #[serde(default = "default_refresh_interval")]
    pub refresh_interval: u64,
    /// Whether to show notifications.
    #[serde(default = "default_true")]
    pub show_notifications: bool,
    /// Log level.
    #[serde(default = "default_log_level")]
    pub log_level: String,
}

/// Provider-specific configuration.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ProviderConfig {
    /// Whether this provider is enabled.
    #[serde(default = "default_true")]
    pub enabled: bool,
    /// Environment variable name for the API key.
    pub api_key_env: Option<String>,
    /// Custom display name.
    pub display_name: Option<String>,
}

fn default_refresh_interval() -> u64 {
    60
}

fn default_true() -> bool {
    true
}

fn default_log_level() -> String {
    "info".to_string()
}

impl Default for GeneralConfig {
    fn default() -> Self {
        Self {
            refresh_interval: default_refresh_interval(),
            show_notifications: true,
            log_level: default_log_level(),
        }
    }
}

impl Default for Config {
    fn default() -> Self {
        Self {
            general: GeneralConfig::default(),
            providers: HashMap::new(),
        }
    }
}

impl Config {
    /// Returns the default configuration file path.
    pub fn default_path() -> PathBuf {
        dirs::config_dir()
            .unwrap_or_else(|| PathBuf::from("."))
            .join("exactobar")
            .join("config.toml")
    }

    /// Loads configuration from the default path.
    pub fn load() -> Result<Self, StoreError> {
        Self::load_from(&Self::default_path())
    }

    /// Loads configuration from a specific path.
    pub fn load_from(path: &Path) -> Result<Self, StoreError> {
        if !path.exists() {
            debug!(path = %path.display(), "Config file not found, using defaults");
            return Ok(Self::default());
        }

        let content = std::fs::read_to_string(path)?;
        
        // Parse as TOML (we'd need to add toml dependency, using JSON for now)
        let config: Config = serde_json::from_str(&content)?;
        
        info!(path = %path.display(), "Loaded configuration");
        Ok(config)
    }

    /// Saves configuration to the default path.
    pub fn save(&self) -> Result<(), StoreError> {
        self.save_to(&Self::default_path())
    }

    /// Saves configuration to a specific path.
    pub fn save_to(&self, path: &Path) -> Result<(), StoreError> {
        if let Some(parent) = path.parent() {
            std::fs::create_dir_all(parent)?;
        }

        let content = serde_json::to_string_pretty(self)?;
        std::fs::write(path, content)?;

        info!(path = %path.display(), "Saved configuration");
        Ok(())
    }

    /// Returns whether a provider is enabled.
    pub fn is_provider_enabled(&self, kind: ProviderKind) -> bool {
        let key = kind.display_name().to_lowercase();
        self.providers
            .get(&key)
            .map(|p| p.enabled)
            .unwrap_or(true)
    }
}
