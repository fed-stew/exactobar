//! Main usage state store.
//!
//! Manages provider usage data with change notifications for UI updates.

use chrono::{DateTime, Utc};
use exactobar_core::{Credits, ProviderKind, ProviderStatus, UsageSnapshot};
use std::collections::{HashMap, HashSet};
use std::sync::Arc;
use std::time::Duration;
use tokio::sync::{watch, RwLock};
use tracing::{debug, info, warn};

use crate::error::StoreError;

// ============================================================================
// Cost Usage (for token cost tracking)
// ============================================================================

/// Cost usage snapshot from local log parsing.
#[derive(Debug, Clone, Default)]
pub struct CostUsageSnapshot {
    /// Daily usage breakdown.
    pub daily: Vec<DailyCost>,
    /// Total tokens.
    pub total_tokens: u64,
    /// Total estimated cost (USD).
    pub total_cost_usd: f64,
    /// Last scan timestamp.
    pub scanned_at: Option<DateTime<Utc>>,
}

/// Daily cost breakdown.
#[derive(Debug, Clone)]
pub struct DailyCost {
    /// Date of the cost entry.
    pub date: DateTime<Utc>,
    /// Token count for this day.
    pub tokens: u64,
    /// Cost in USD.
    pub cost_usd: f64,
}

// ============================================================================
// Inner State
// ============================================================================

/// Internal state for the usage store.
struct UsageStoreInner {
    /// Usage snapshots by provider.
    snapshots: HashMap<ProviderKind, UsageSnapshot>,
    /// Credits by provider.
    credits: HashMap<ProviderKind, Credits>,
    /// Provider status.
    status: HashMap<ProviderKind, ProviderStatus>,
    /// Cost usage from local logs.
    cost_usage: HashMap<ProviderKind, CostUsageSnapshot>,
    /// Enabled providers.
    enabled_providers: HashSet<ProviderKind>,
    /// Last refresh time.
    last_refresh: Option<DateTime<Utc>>,
    /// Providers currently refreshing.
    refresh_in_progress: HashSet<ProviderKind>,
    /// Error messages by provider.
    errors: HashMap<ProviderKind, String>,
    /// Snapshot timestamps.
    snapshot_times: HashMap<ProviderKind, DateTime<Utc>>,
}

impl Default for UsageStoreInner {
    fn default() -> Self {
        let mut enabled = HashSet::new();
        enabled.insert(ProviderKind::Codex);
        enabled.insert(ProviderKind::Claude);

        Self {
            snapshots: HashMap::new(),
            credits: HashMap::new(),
            status: HashMap::new(),
            cost_usage: HashMap::new(),
            enabled_providers: enabled,
            last_refresh: None,
            refresh_in_progress: HashSet::new(),
            errors: HashMap::new(),
            snapshot_times: HashMap::new(),
        }
    }
}

// ============================================================================
// Usage Store
// ============================================================================

/// Main state store for provider usage data.
///
/// Observable via watch channels for UI updates.
pub struct UsageStore {
    inner: Arc<RwLock<UsageStoreInner>>,
    notify: watch::Sender<u64>,
    version: Arc<RwLock<u64>>,
}

impl Default for UsageStore {
    fn default() -> Self {
        Self::new()
    }
}

impl UsageStore {
    /// Creates a new usage store.
    pub fn new() -> Self {
        let (notify, _) = watch::channel(0);
        Self {
            inner: Arc::new(RwLock::new(UsageStoreInner::default())),
            notify,
            version: Arc::new(RwLock::new(0)),
        }
    }

    /// Creates a store with specific enabled providers.
    pub fn with_enabled(enabled: HashSet<ProviderKind>) -> Self {
        let store = Self::new();
        let inner = UsageStoreInner {
            enabled_providers: enabled,
            ..Default::default()
        };
        Self {
            inner: Arc::new(RwLock::new(inner)),
            ..store
        }
    }

    // ========================================================================
    // Snapshot Access
    // ========================================================================

    /// Gets a snapshot for a provider.
    pub async fn get_snapshot(&self, provider: ProviderKind) -> Option<UsageSnapshot> {
        self.inner.read().await.snapshots.get(&provider).cloned()
    }

    /// Gets all snapshots.
    pub async fn get_all_snapshots(&self) -> HashMap<ProviderKind, UsageSnapshot> {
        self.inner.read().await.snapshots.clone()
    }

    /// Gets snapshots for enabled providers only.
    pub async fn get_enabled_snapshots(&self) -> HashMap<ProviderKind, UsageSnapshot> {
        let inner = self.inner.read().await;
        inner
            .snapshots
            .iter()
            .filter(|(k, _)| inner.enabled_providers.contains(k))
            .map(|(k, v)| (*k, v.clone()))
            .collect()
    }

    /// Sets a snapshot for a provider.
    pub async fn set_snapshot(&self, provider: ProviderKind, snapshot: UsageSnapshot) {
        {
            let mut inner = self.inner.write().await;
            inner.snapshots.insert(provider, snapshot);
            inner.snapshot_times.insert(provider, Utc::now());
            inner.errors.remove(&provider);
        }
        self.notify_change().await;
        debug!(provider = ?provider, "Snapshot updated");
    }

    // ========================================================================
    // Provider Management
    // ========================================================================

    /// Gets enabled providers.
    pub async fn enabled_providers(&self) -> HashSet<ProviderKind> {
        self.inner.read().await.enabled_providers.clone()
    }

    /// Enables or disables a provider.
    pub async fn set_enabled(&self, provider: ProviderKind, enabled: bool) {
        {
            let mut inner = self.inner.write().await;
            if enabled {
                inner.enabled_providers.insert(provider);
            } else {
                inner.enabled_providers.remove(&provider);
            }
        }
        self.notify_change().await;
        info!(provider = ?provider, enabled = enabled, "Provider enabled state changed");
    }

    /// Checks if a provider is enabled.
    pub async fn is_enabled(&self, provider: ProviderKind) -> bool {
        self.inner
            .read()
            .await
            .enabled_providers
            .contains(&provider)
    }

    // ========================================================================
    // Refresh Management
    // ========================================================================

    /// Marks a provider as refreshing.
    pub async fn start_refresh(&self, provider: ProviderKind) -> Result<(), StoreError> {
        let mut inner = self.inner.write().await;
        if inner.refresh_in_progress.contains(&provider) {
            return Err(StoreError::RefreshInProgress(format!("{provider:?}")));
        }
        inner.refresh_in_progress.insert(provider);
        Ok(())
    }

    /// Marks a provider as done refreshing.
    pub async fn end_refresh(&self, provider: ProviderKind) {
        {
            let mut inner = self.inner.write().await;
            inner.refresh_in_progress.remove(&provider);
            inner.last_refresh = Some(Utc::now());
        }
        self.notify_change().await;
    }

    /// Checks if a provider is currently refreshing.
    pub async fn is_refreshing(&self, provider: ProviderKind) -> bool {
        self.inner
            .read()
            .await
            .refresh_in_progress
            .contains(&provider)
    }

    /// Gets the last refresh time.
    pub async fn last_refresh(&self) -> Option<DateTime<Utc>> {
        self.inner.read().await.last_refresh
    }

    // ========================================================================
    // Status
    // ========================================================================

    /// Gets provider status.
    pub async fn get_status(&self, provider: ProviderKind) -> Option<ProviderStatus> {
        self.inner.read().await.status.get(&provider).cloned()
    }

    /// Sets provider status.
    pub async fn set_status(&self, provider: ProviderKind, status: ProviderStatus) {
        {
            let mut inner = self.inner.write().await;
            inner.status.insert(provider, status);
        }
        self.notify_change().await;
    }

    // ========================================================================
    // Credits
    // ========================================================================

    /// Gets credits for a provider.
    pub async fn get_credits(&self, provider: ProviderKind) -> Option<Credits> {
        self.inner.read().await.credits.get(&provider).cloned()
    }

    /// Sets credits for a provider.
    pub async fn set_credits(&self, provider: ProviderKind, credits: Credits) {
        {
            let mut inner = self.inner.write().await;
            inner.credits.insert(provider, credits);
        }
        self.notify_change().await;
    }

    // ========================================================================
    // Cost Usage
    // ========================================================================

    /// Gets cost usage for a provider.
    pub async fn get_cost_usage(&self, provider: ProviderKind) -> Option<CostUsageSnapshot> {
        self.inner.read().await.cost_usage.get(&provider).cloned()
    }

    /// Sets cost usage for a provider.
    pub async fn set_cost_usage(&self, provider: ProviderKind, usage: CostUsageSnapshot) {
        {
            let mut inner = self.inner.write().await;
            inner.cost_usage.insert(provider, usage);
        }
        self.notify_change().await;
    }

    // ========================================================================
    // Errors
    // ========================================================================

    /// Gets the error for a provider.
    pub async fn get_error(&self, provider: ProviderKind) -> Option<String> {
        self.inner.read().await.errors.get(&provider).cloned()
    }

    /// Sets an error for a provider.
    pub async fn set_error(&self, provider: ProviderKind, error: String) {
        {
            let mut inner = self.inner.write().await;
            inner.errors.insert(provider, error);
        }
        self.notify_change().await;
        warn!(provider = ?provider, "Error set for provider");
    }

    /// Clears the error for a provider.
    pub async fn clear_error(&self, provider: ProviderKind) {
        {
            let mut inner = self.inner.write().await;
            inner.errors.remove(&provider);
        }
        self.notify_change().await;
    }

    /// Gets all errors.
    pub async fn get_all_errors(&self) -> HashMap<ProviderKind, String> {
        self.inner.read().await.errors.clone()
    }

    // ========================================================================
    // Observable
    // ========================================================================

    /// Subscribes to store changes.
    pub fn subscribe(&self) -> watch::Receiver<u64> {
        self.notify.subscribe()
    }

    /// Notifies subscribers of a change.
    async fn notify_change(&self) {
        let mut version = self.version.write().await;
        *version += 1;
        let _ = self.notify.send(*version);
    }

    // ========================================================================
    // Staleness
    // ========================================================================

    /// Checks if a provider's data is stale.
    pub async fn is_stale(&self, provider: ProviderKind, threshold: Duration) -> bool {
        let inner = self.inner.read().await;
        match inner.snapshot_times.get(&provider) {
            Some(time) => {
                let age = Utc::now().signed_duration_since(*time);
                age > chrono::Duration::from_std(threshold).unwrap_or(chrono::Duration::MAX)
            }
            None => true, // No snapshot = stale
        }
    }

    /// Gets the age of a snapshot.
    pub async fn snapshot_age(&self, provider: ProviderKind) -> Option<chrono::Duration> {
        self.inner
            .read()
            .await
            .snapshot_times
            .get(&provider)
            .map(|t| Utc::now().signed_duration_since(*t))
    }
}

// ============================================================================
// Tests
// ============================================================================

#[cfg(test)]
mod tests {
    use super::*;

    #[tokio::test]
    async fn test_new_store() {
        let store = UsageStore::new();
        let enabled = store.enabled_providers().await;
        assert!(enabled.contains(&ProviderKind::Codex));
        assert!(enabled.contains(&ProviderKind::Claude));
    }

    #[tokio::test]
    async fn test_snapshot_operations() {
        let store = UsageStore::new();

        // Initially no snapshot
        assert!(store.get_snapshot(ProviderKind::Codex).await.is_none());

        // Set snapshot
        let snapshot = UsageSnapshot::new();
        store.set_snapshot(ProviderKind::Codex, snapshot).await;

        // Now we have a snapshot
        assert!(store.get_snapshot(ProviderKind::Codex).await.is_some());
    }

    #[tokio::test]
    async fn test_provider_toggle() {
        let store = UsageStore::new();

        assert!(store.is_enabled(ProviderKind::Codex).await);
        store.set_enabled(ProviderKind::Codex, false).await;
        assert!(!store.is_enabled(ProviderKind::Codex).await);
    }

    #[tokio::test]
    async fn test_refresh_tracking() {
        let store = UsageStore::new();

        assert!(!store.is_refreshing(ProviderKind::Codex).await);

        store.start_refresh(ProviderKind::Codex).await.unwrap();
        assert!(store.is_refreshing(ProviderKind::Codex).await);

        // Second refresh should fail
        assert!(store.start_refresh(ProviderKind::Codex).await.is_err());

        store.end_refresh(ProviderKind::Codex).await;
        assert!(!store.is_refreshing(ProviderKind::Codex).await);
    }

    #[tokio::test]
    async fn test_error_handling() {
        let store = UsageStore::new();

        assert!(store.get_error(ProviderKind::Codex).await.is_none());

        store
            .set_error(ProviderKind::Codex, "Test error".to_string())
            .await;
        assert!(store.get_error(ProviderKind::Codex).await.is_some());

        store.clear_error(ProviderKind::Codex).await;
        assert!(store.get_error(ProviderKind::Codex).await.is_none());
    }

    #[tokio::test]
    async fn test_staleness() {
        let store = UsageStore::new();

        // No snapshot = stale
        assert!(store
            .is_stale(ProviderKind::Codex, Duration::from_secs(60))
            .await);

        // After setting, not stale
        store
            .set_snapshot(ProviderKind::Codex, UsageSnapshot::new())
            .await;
        assert!(!store
            .is_stale(ProviderKind::Codex, Duration::from_secs(60))
            .await);
    }
}
