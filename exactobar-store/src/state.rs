//! Application state management.

use crate::config::Config;
use crate::error::StoreError;
use chrono::{DateTime, Utc};
use exactobar_core::{ProviderKind, UsageData};
use std::collections::HashMap;
use std::sync::Arc;
use tokio::sync::RwLock;
use tracing::debug;

/// Application state containing all usage data and configuration.
#[derive(Debug)]
pub struct AppState {
    /// Configuration.
    config: Arc<RwLock<Config>>,
    /// Usage data by provider.
    usage_data: Arc<RwLock<HashMap<ProviderKind, UsageData>>>,
    /// Last update time by provider.
    last_updated: Arc<RwLock<HashMap<ProviderKind, DateTime<Utc>>>>,
}

impl AppState {
    /// Creates a new application state with the given configuration.
    pub fn new(config: Config) -> Self {
        Self {
            config: Arc::new(RwLock::new(config)),
            usage_data: Arc::new(RwLock::new(HashMap::new())),
            last_updated: Arc::new(RwLock::new(HashMap::new())),
        }
    }

    /// Creates a new application state with default configuration.
    pub fn with_defaults() -> Self {
        Self::new(Config::default())
    }

    /// Returns the current configuration.
    pub async fn config(&self) -> Config {
        self.config.read().await.clone()
    }

    /// Updates the configuration.
    pub async fn update_config(&self, config: Config) {
        *self.config.write().await = config;
    }

    /// Returns usage data for a specific provider.
    pub async fn get_usage(&self, kind: ProviderKind) -> Option<UsageData> {
        self.usage_data.read().await.get(&kind).cloned()
    }

    /// Returns all usage data.
    pub async fn get_all_usage(&self) -> HashMap<ProviderKind, UsageData> {
        self.usage_data.read().await.clone()
    }

    /// Updates usage data for a provider.
    pub async fn update_usage(&self, usage: UsageData) {
        let kind = usage.provider_kind;
        debug!(provider = %kind.display_name(), "Updating usage data");
        
        self.usage_data.write().await.insert(kind, usage);
        self.last_updated.write().await.insert(kind, Utc::now());
    }

    /// Returns the last update time for a provider.
    pub async fn last_updated(&self, kind: ProviderKind) -> Option<DateTime<Utc>> {
        self.last_updated.read().await.get(&kind).copied()
    }

    /// Clears all usage data.
    pub async fn clear(&self) {
        self.usage_data.write().await.clear();
        self.last_updated.write().await.clear();
    }
}

impl Default for AppState {
    fn default() -> Self {
        Self::with_defaults()
    }
}

impl Clone for AppState {
    fn clone(&self) -> Self {
        Self {
            config: Arc::clone(&self.config),
            usage_data: Arc::clone(&self.usage_data),
            last_updated: Arc::clone(&self.last_updated),
        }
    }
}
