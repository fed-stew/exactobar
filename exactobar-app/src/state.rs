//! Global application state.
//!
//! Manages settings, usage data, and UI state accessible from GPUI context.

use exactobar_core::{ProviderKind, ProviderStatus, UsageSnapshot};
use exactobar_store::{CookieSource, DataSourceMode, Settings, SettingsStore};
use gpui::*;
use std::collections::HashSet;
use std::sync::{Arc, OnceLock};
use tokio::sync::RwLock;
use tracing::{error, info};

// ============================================================================
// Tokio Runtime Bridge
// ============================================================================

/// Global Tokio runtime for async save operations.
/// We need this because GPUI runs on smol, but our store operations are tokio-based.
static TOKIO_RUNTIME: OnceLock<tokio::runtime::Runtime> = OnceLock::new();

/// Gets or creates the Tokio runtime for save operations.
///
/// # Panics
///
/// Panics if the Tokio runtime cannot be created. This is intentionally
/// unrecoverable because:
/// 1. Settings persistence requires async I/O via Tokio
/// 2. Runtime creation only fails due to OS resource exhaustion or
///    misconfiguration, making recovery impossible
/// 3. Losing the ability to save settings would corrupt user experience
///
/// Uses `OnceLock` so the panic can only occur once at initialization.
fn tokio_runtime() -> &'static tokio::runtime::Runtime {
    TOKIO_RUNTIME.get_or_init(|| {
        tokio::runtime::Builder::new_multi_thread()
            .worker_threads(1)
            .enable_all()
            .build()
            .expect(
                "Failed to create Tokio runtime for save operations. \
                This is unrecoverable - settings cannot be persisted."
            )
    })
}

// ============================================================================
// App State
// ============================================================================

/// Global application state.
pub struct AppState {
    /// User settings.
    pub settings: Entity<SettingsModel>,
    /// Usage data.
    pub usage: Entity<UsageModel>,
    /// Whether the menu is currently open.
    pub menu_open: bool,
    /// Whether a refresh is in progress.
    pub refresh_in_progress: bool,
}

impl Global for AppState {}

impl AppState {
    /// Initializes the app state.
    pub fn init(cx: &mut App) -> Self {
        // Load settings from disk (sync for simplicity at init)
        let settings_store = tokio_runtime().block_on(async {
            match SettingsStore::load_default().await {
                Ok(store) => store,
                Err(_) => SettingsStore::new(exactobar_store::default_config_dir()),
            }
        });

        let settings = cx.new(|_| SettingsModel::new(settings_store));
        let usage = cx.new(|_| UsageModel::new());

        Self {
            settings,
            usage,
            menu_open: false,
            refresh_in_progress: false,
        }
    }

    /// Gets the list of enabled providers.
    pub fn enabled_providers(&self, cx: &App) -> Vec<ProviderKind> {
        self.settings.read(cx).enabled_providers()
    }

    /// Gets a usage snapshot for a provider.
    pub fn get_snapshot(&self, provider: ProviderKind, cx: &App) -> Option<UsageSnapshot> {
        self.usage.read(cx).get_snapshot(provider)
    }

    /// Gets the status for a provider.
    pub fn get_status(&self, provider: ProviderKind, cx: &App) -> Option<ProviderStatus> {
        self.usage.read(cx).get_status(provider)
    }

    /// Checks if a provider is currently refreshing.
    pub fn is_provider_refreshing(&self, provider: ProviderKind, cx: &App) -> bool {
        self.usage.read(cx).is_refreshing(provider)
    }

    /// Gets the error for a provider.
    pub fn get_error(&self, provider: ProviderKind, cx: &App) -> Option<String> {
        self.usage.read(cx).get_error(provider)
    }

    /// Refreshes all enabled providers.
    pub fn refresh_all(&self, cx: &mut App) {
        let providers = self.enabled_providers(cx);
        info!(count = providers.len(), "Refreshing all providers");

        for provider in providers {
            self.refresh_provider(provider, cx);
        }
    }

    /// Refreshes a single provider.
    pub fn refresh_provider(&self, provider: ProviderKind, cx: &mut App) {
        let usage = self.usage.clone();

        cx.spawn(async move |mut cx| {
            // Mark as refreshing
            let _ = cx.update_entity(&usage, |model, cx| {
                model.set_refreshing(provider, true);
                cx.notify();
            });

            // Execute fetch on Tokio runtime - MUST use this bridge!
            // Direct pipeline.execute() calls will panic because tokio::process::Command
            // requires a Tokio runtime, but GPUI runs on smol.
            let result = crate::refresh::fetch_on_tokio(provider).await;

            // Update state
            let _ = cx.update_entity(&usage, |model, cx| {
                model.set_refreshing(provider, false);
                match result {
                    Ok(snapshot) => {
                        model.set_snapshot(provider, snapshot);
                        model.clear_error(provider);
                    }
                    Err(e) => {
                        model.set_error(provider, e);
                    }
                }
                cx.notify();
            });
        })
        .detach();
    }
}

// ============================================================================
// Settings Model
// ============================================================================

/// Model wrapping SettingsStore for GPUI.
#[allow(dead_code)]
pub struct SettingsModel {
    store: Arc<RwLock<SettingsStore>>,
    cached_settings: Settings,
}

impl SettingsModel {
    pub fn new(store: SettingsStore) -> Self {
        let cached = tokio_runtime().block_on(async { store.get().await });
        Self {
            store: Arc::new(RwLock::new(store)),
            cached_settings: cached,
        }
    }

    /// Gets enabled providers.
    pub fn enabled_providers(&self) -> Vec<ProviderKind> {
        self.cached_settings.enabled_providers.iter().copied().collect()
    }

    /// Checks if a provider is enabled.
    pub fn is_provider_enabled(&self, provider: ProviderKind) -> bool {
        self.cached_settings.enabled_providers.contains(&provider)
    }

    /// Gets the refresh cadence.
    pub fn refresh_cadence(&self) -> exactobar_store::RefreshCadence {
        self.cached_settings.refresh_cadence
    }

    /// Gets whether icons should be merged.
    pub fn merge_icons(&self) -> bool {
        self.cached_settings.merge_icons
    }

    /// Toggles a provider.
    pub fn toggle_provider(&mut self, provider: ProviderKind) {
        if self.cached_settings.enabled_providers.contains(&provider) {
            self.cached_settings.enabled_providers.remove(&provider);
        } else {
            self.cached_settings.enabled_providers.insert(provider);
        }
        self.save_async();
    }

    /// Sets the refresh cadence.
    pub fn set_refresh_cadence(&mut self, cadence: exactobar_store::RefreshCadence) {
        self.cached_settings.refresh_cadence = cadence;
        self.save_async();
    }

    /// Sets merge icons mode.
    pub fn set_merge_icons(&mut self, merge: bool) {
        self.cached_settings.merge_icons = merge;
        self.save_async();
    }

    /// Gets the underlying settings.
    pub fn settings(&self) -> &Settings {
        &self.cached_settings
    }

    // ========================================================================
    // Display Settings
    // ========================================================================

    /// Sets whether progress bars show percent used.
    pub fn set_usage_bars_show_used(&mut self, value: bool) {
        self.cached_settings.usage_bars_show_used = value;
        self.save_async();
    }

    /// Sets whether reset times show as absolute clock values.
    pub fn set_reset_times_show_absolute(&mut self, value: bool) {
        self.cached_settings.reset_times_show_absolute = value;
        self.save_async();
    }

    /// Sets whether menu bar shows brand icon with percent.
    pub fn set_menu_bar_shows_brand_icon_with_percent(&mut self, value: bool) {
        self.cached_settings.menu_bar_shows_brand_icon_with_percent = value;
        self.save_async();
    }

    /// Sets whether the switcher shows provider icons.
    pub fn set_switcher_shows_icons(&mut self, value: bool) {
        self.cached_settings.switcher_shows_icons = value;
        self.save_async();
    }

    // ========================================================================
    // Feature Toggles
    // ========================================================================

    /// Sets debug mode.
    pub fn set_debug_mode(&mut self, value: bool) {
        self.cached_settings.debug_mode = value;
        self.save_async();
    }

    /// Sets auto-refresh on wake.
    pub fn set_auto_refresh_on_wake(&mut self, value: bool) {
        self.cached_settings.auto_refresh_on_wake = value;
        self.save_async();
    }

    /// Sets whether status page checks are enabled.
    pub fn set_status_checks_enabled(&mut self, value: bool) {
        self.cached_settings.status_checks_enabled = value;
        self.save_async();
    }

    /// Sets whether quota notifications are enabled.
    pub fn set_session_quota_notifications_enabled(&mut self, value: bool) {
        self.cached_settings.session_quota_notifications_enabled = value;
        self.save_async();
    }

    /// Sets whether cost tracking is enabled.
    pub fn set_cost_usage_enabled(&mut self, value: bool) {
        self.cached_settings.cost_usage_enabled = value;
        self.save_async();
    }

    /// Sets whether random blink animation is enabled.
    pub fn set_random_blink_enabled(&mut self, value: bool) {
        self.cached_settings.random_blink_enabled = value;
        self.save_async();
    }

    /// Sets whether Claude web extras are enabled.
    pub fn set_claude_web_extras_enabled(&mut self, value: bool) {
        self.cached_settings.claude_web_extras_enabled = value;
        self.save_async();
    }

    /// Sets whether optional credits and extra usage are shown.
    pub fn set_show_optional_credits_and_extra_usage(&mut self, value: bool) {
        self.cached_settings.show_optional_credits_and_extra_usage = value;
        self.save_async();
    }

    /// Sets whether OpenAI web access is enabled.
    pub fn set_openai_web_access_enabled(&mut self, value: bool) {
        self.cached_settings.openai_web_access_enabled = value;
        self.save_async();
    }

    // ========================================================================
    // Per-Provider Settings
    // ========================================================================

    /// Gets the cookie source for a provider.
    pub fn cookie_source(&self, provider: ProviderKind) -> CookieSource {
        self.cached_settings
            .provider_settings
            .get(&provider)
            .and_then(|ps| ps.cookie_source)
            .unwrap_or_default()
    }

    /// Sets the cookie source for a provider.
    pub fn set_cookie_source(&mut self, provider: ProviderKind, source: CookieSource) {
        self.cached_settings
            .provider_settings
            .entry(provider)
            .or_default()
            .cookie_source = Some(source);
        self.save_async();
    }

    /// Gets the data source mode for Codex.
    pub fn codex_data_source(&self) -> DataSourceMode {
        self.cached_settings.codex_usage_data_source
    }

    /// Sets the data source mode for Codex.
    pub fn set_codex_data_source(&mut self, mode: DataSourceMode) {
        self.cached_settings.codex_usage_data_source = mode;
        self.save_async();
    }

    /// Gets the data source mode for Claude.
    pub fn claude_data_source(&self) -> DataSourceMode {
        self.cached_settings.claude_usage_data_source
    }

    /// Sets the data source mode for Claude.
    pub fn set_claude_data_source(&mut self, mode: DataSourceMode) {
        self.cached_settings.claude_usage_data_source = mode;
        self.save_async();
    }

    fn save_async(&self) {
        let store = self.store.clone();
        let settings = self.cached_settings.clone();

        // Bridge from smol (GPUI's runtime) to tokio (our store's runtime)
        // Direct tokio::spawn() will panic - there's no Tokio runtime running!
        smol::spawn(async move {
            smol::unblock(move || {
                tokio_runtime().block_on(async move {
                    let s = store.write().await;
                    s.update(|current| {
                        *current = settings;
                    })
                    .await;
                    if let Err(e) = s.save().await {
                        error!(error = %e, "Failed to save settings");
                    }
                })
            })
            .await;
        })
        .detach();
    }
}

// ============================================================================
// Usage Model
// ============================================================================

/// Model wrapping usage data for GPUI.
#[allow(dead_code)]
pub struct UsageModel {
    snapshots: std::collections::HashMap<ProviderKind, UsageSnapshot>,
    status: std::collections::HashMap<ProviderKind, ProviderStatus>,
    errors: std::collections::HashMap<ProviderKind, String>,
    refreshing: HashSet<ProviderKind>,
}

impl UsageModel {
    pub fn new() -> Self {
        Self {
            snapshots: std::collections::HashMap::new(),
            status: std::collections::HashMap::new(),
            errors: std::collections::HashMap::new(),
            refreshing: HashSet::new(),
        }
    }

    pub fn get_snapshot(&self, provider: ProviderKind) -> Option<UsageSnapshot> {
        self.snapshots.get(&provider).cloned()
    }

    pub fn set_snapshot(&mut self, provider: ProviderKind, snapshot: UsageSnapshot) {
        self.snapshots.insert(provider, snapshot);
    }

    pub fn get_status(&self, provider: ProviderKind) -> Option<ProviderStatus> {
        self.status.get(&provider).cloned()
    }

    pub fn set_status(&mut self, provider: ProviderKind, status: ProviderStatus) {
        self.status.insert(provider, status);
    }

    pub fn get_error(&self, provider: ProviderKind) -> Option<String> {
        self.errors.get(&provider).cloned()
    }

    pub fn set_error(&mut self, provider: ProviderKind, error: String) {
        self.errors.insert(provider, error);
    }

    pub fn clear_error(&mut self, provider: ProviderKind) {
        self.errors.remove(&provider);
    }

    pub fn is_refreshing(&self, provider: ProviderKind) -> bool {
        self.refreshing.contains(&provider)
    }

    pub fn set_refreshing(&mut self, provider: ProviderKind, refreshing: bool) {
        if refreshing {
            self.refreshing.insert(provider);
        } else {
            self.refreshing.remove(&provider);
        }
    }
}

impl Default for UsageModel {
    fn default() -> Self {
        Self::new()
    }
}
