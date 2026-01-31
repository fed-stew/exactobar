//! User preferences store.
//!
//! Manages user settings with persistence and change notification.

use exactobar_core::ProviderKind;
use serde::{Deserialize, Serialize};
use std::collections::{HashMap, HashSet};
use std::path::PathBuf;
use std::sync::Arc;
use std::time::Duration;
use tokio::sync::{RwLock, watch};
use tracing::{debug, info, warn};

use crate::error::StoreError;
use crate::persistence::{default_settings_path, load_json, save_json};

// ============================================================================
// Settings Types
// ============================================================================

/// User preferences.
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(default)]
#[allow(clippy::struct_excessive_bools)]
pub struct Settings {
    // ========================================================================
    // Core Settings (existing)
    // ========================================================================
    /// Enabled providers.
    pub enabled_providers: HashSet<ProviderKind>,

    /// Auto-refresh cadence.
    pub refresh_cadence: RefreshCadence,

    /// Refresh on wake from sleep.
    pub auto_refresh_on_wake: bool,

    /// Merge all providers into a single icon.
    pub merge_icons: bool,

    /// Show countdown vs absolute time for resets.
    pub show_reset_countdown: bool,

    /// Selected provider (for merged mode).
    pub selected_provider: Option<ProviderKind>,

    /// Debug mode.
    pub debug_mode: bool,

    /// Log level.
    pub log_level: LogLevel,

    /// Theme mode preference.
    pub theme_mode: ThemeMode,

    /// Per-provider settings.
    pub provider_settings: HashMap<ProviderKind, ProviderSettings>,

    // ========================================================================
    // Display Settings (new from CodexBar)
    // ========================================================================
    /// When true, progress bars show "percent used" instead of "percent remaining".
    pub usage_bars_show_used: bool,

    /// Show reset times as absolute clock values instead of countdowns.
    pub reset_times_show_absolute: bool,

    /// Use provider branding icons with percentage in menu bar.
    pub menu_bar_shows_brand_icon_with_percent: bool,

    /// Show provider icons in the in-menu switcher.
    pub switcher_shows_icons: bool,

    // ========================================================================
    // Feature Toggles (new from CodexBar)
    // ========================================================================
    /// Enable status page checks for provider health.
    pub status_checks_enabled: bool,

    /// Show session quota notifications when approaching limits.
    pub session_quota_notifications_enabled: bool,

    /// Enable provider cost summary from local usage logs.
    pub cost_usage_enabled: bool,

    /// Enable random blink animation on status icon.
    pub random_blink_enabled: bool,

    /// Enable Claude web extras (via browser cookies).
    pub claude_web_extras_enabled: bool,

    /// Show optional credits and extra usage sections in menu.
    pub show_optional_credits_and_extra_usage: bool,

    /// Enable `OpenAI` web dashboard access for Codex.
    pub openai_web_access_enabled: bool,

    // ========================================================================
    // Data Sources (new from CodexBar)
    // ========================================================================
    /// Codex usage data source mode.
    pub codex_usage_data_source: DataSourceMode,

    /// Claude usage data source mode.
    pub claude_usage_data_source: DataSourceMode,

    // ========================================================================
    // Provider Order & Debug (new from CodexBar)
    // ========================================================================
    /// Provider display order in menu (empty = default order).
    pub provider_order: Vec<ProviderKind>,

    /// Debug loading pattern override.
    pub debug_loading_pattern: Option<String>,

    /// Whether provider detection has completed (for first-run experience).
    pub provider_detection_completed: bool,
}

impl Default for Settings {
    fn default() -> Self {
        let mut enabled = HashSet::new();
        enabled.insert(ProviderKind::Codex);
        enabled.insert(ProviderKind::Claude);

        Self {
            // Core settings
            enabled_providers: enabled,
            refresh_cadence: RefreshCadence::default(),
            auto_refresh_on_wake: true,
            merge_icons: true,
            show_reset_countdown: true,
            selected_provider: None,
            debug_mode: false,
            log_level: LogLevel::default(),
            theme_mode: ThemeMode::Dark,
            provider_settings: HashMap::new(),

            // Display settings - sensible defaults
            usage_bars_show_used: false,
            reset_times_show_absolute: false,
            menu_bar_shows_brand_icon_with_percent: false,
            switcher_shows_icons: true,

            // Feature toggles - most enabled by default
            status_checks_enabled: true,
            session_quota_notifications_enabled: true,
            cost_usage_enabled: false, // Off by default - requires local logs
            random_blink_enabled: false, // Off by default - can be annoying
            claude_web_extras_enabled: false, // Off by default - requires cookies
            show_optional_credits_and_extra_usage: true,
            openai_web_access_enabled: true,

            // Data sources - auto-detect
            codex_usage_data_source: DataSourceMode::Auto,
            claude_usage_data_source: DataSourceMode::Auto,

            // Provider order & debug
            provider_order: vec![],
            debug_loading_pattern: None,
            provider_detection_completed: false,
        }
    }
}

/// Refresh cadence options.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, Default)]
#[serde(rename_all = "snake_case")]
pub enum RefreshCadence {
    /// Manual refresh only.
    Manual,
    /// Every minute.
    OneMinute,
    /// Every two minutes.
    #[default]
    TwoMinutes,
    /// Every five minutes.
    FiveMinutes,
    /// Every fifteen minutes.
    FifteenMinutes,
}

impl RefreshCadence {
    /// Returns the duration, or None for manual.
    pub fn as_duration(&self) -> Option<Duration> {
        match self {
            RefreshCadence::Manual => None,
            RefreshCadence::OneMinute => Some(Duration::from_secs(60)),
            RefreshCadence::TwoMinutes => Some(Duration::from_secs(120)),
            RefreshCadence::FiveMinutes => Some(Duration::from_secs(300)),
            RefreshCadence::FifteenMinutes => Some(Duration::from_secs(900)),
        }
    }

    /// All available cadences.
    pub fn all() -> &'static [RefreshCadence] {
        &[
            RefreshCadence::Manual,
            RefreshCadence::OneMinute,
            RefreshCadence::TwoMinutes,
            RefreshCadence::FiveMinutes,
            RefreshCadence::FifteenMinutes,
        ]
    }
}

impl std::fmt::Display for RefreshCadence {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            RefreshCadence::Manual => write!(f, "Manual"),
            RefreshCadence::OneMinute => write!(f, "1 minute"),
            RefreshCadence::TwoMinutes => write!(f, "2 minutes"),
            RefreshCadence::FiveMinutes => write!(f, "5 minutes"),
            RefreshCadence::FifteenMinutes => write!(f, "15 minutes"),
        }
    }
}

/// Log level.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, Default)]
#[serde(rename_all = "snake_case")]
pub enum LogLevel {
    /// Error level logging.
    Error,
    /// Warning level logging.
    Warn,
    /// Info level logging.
    #[default]
    Info,
    /// Debug level logging.
    Debug,
    /// Trace level logging.
    Trace,
}

impl std::fmt::Display for LogLevel {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            LogLevel::Error => write!(f, "error"),
            LogLevel::Warn => write!(f, "warn"),
            LogLevel::Info => write!(f, "info"),
            LogLevel::Debug => write!(f, "debug"),
            LogLevel::Trace => write!(f, "trace"),
        }
    }
}

/// Theme mode preference.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize, Default)]
#[serde(rename_all = "snake_case")]
pub enum ThemeMode {
    /// Always use dark theme (recommended for liquid glass effect).
    #[default]
    Dark,
    /// Always use light theme.
    Light,
    /// Follow system appearance.
    System,
}

impl std::fmt::Display for ThemeMode {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            ThemeMode::Dark => write!(f, "dark"),
            ThemeMode::Light => write!(f, "light"),
            ThemeMode::System => write!(f, "system"),
        }
    }
}

/// Data source mode for usage fetching.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, Default)]
#[serde(rename_all = "snake_case")]
pub enum DataSourceMode {
    /// Automatically detect best source.
    #[default]
    Auto,
    /// Use CLI tool (e.g., `codex` or `claude`).
    Cli,
    /// Use web scraping via browser cookies.
    Web,
    /// Use official API with API key.
    Api,
}

impl std::fmt::Display for DataSourceMode {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            DataSourceMode::Auto => write!(f, "Auto"),
            DataSourceMode::Cli => write!(f, "CLI"),
            DataSourceMode::Web => write!(f, "Web"),
            DataSourceMode::Api => write!(f, "API"),
        }
    }
}

/// Cookie source for web-based data fetching.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, Default)]
#[serde(rename_all = "snake_case")]
pub enum CookieSource {
    /// Automatically detect browser.
    #[default]
    Auto,
    /// Disabled - don't use cookies.
    Off,
    /// Safari browser.
    Safari,
    /// Google Chrome.
    Chrome,
    /// Mozilla Firefox.
    Firefox,
    /// Arc browser.
    Arc,
    /// Microsoft Edge.
    Edge,
    /// Brave browser.
    Brave,
    /// Orion browser.
    Orion,
    /// Manual cookie header input.
    Manual,
}

impl CookieSource {
    /// All available cookie sources.
    pub fn all() -> &'static [CookieSource] {
        &[
            CookieSource::Auto,
            CookieSource::Off,
            CookieSource::Safari,
            CookieSource::Chrome,
            CookieSource::Firefox,
            CookieSource::Arc,
            CookieSource::Edge,
            CookieSource::Brave,
            CookieSource::Orion,
            CookieSource::Manual,
        ]
    }
}

impl std::fmt::Display for CookieSource {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            CookieSource::Auto => write!(f, "Auto"),
            CookieSource::Off => write!(f, "Off"),
            CookieSource::Safari => write!(f, "Safari"),
            CookieSource::Chrome => write!(f, "Chrome"),
            CookieSource::Firefox => write!(f, "Firefox"),
            CookieSource::Arc => write!(f, "Arc"),
            CookieSource::Edge => write!(f, "Edge"),
            CookieSource::Brave => write!(f, "Brave"),
            CookieSource::Orion => write!(f, "Orion"),
            CookieSource::Manual => write!(f, "Manual"),
        }
    }
}

/// Per-provider settings.
#[derive(Debug, Clone, Default, Serialize, Deserialize)]
#[serde(default)]
pub struct ProviderSettings {
    /// Data source mode override.
    pub source_mode: Option<DataSourceMode>,

    /// Cookie source for web-based fetching.
    pub cookie_source: Option<CookieSource>,

    /// Preferred browser for cookies (legacy, use `cookie_source` instead).
    pub browser_preference: Option<String>,

    /// Environment variable for API key.
    pub api_key_env: Option<String>,

    /// Manual cookie header (stored inline for simplicity).
    pub cookie_header: Option<String>,
}

// ============================================================================
// Settings Store
// ============================================================================

/// Persistent settings store with change notifications.
pub struct SettingsStore {
    settings: Arc<RwLock<Settings>>,
    path: PathBuf,
    notify: watch::Sender<u64>,
    version: Arc<RwLock<u64>>,
}

impl SettingsStore {
    /// Creates a new settings store.
    pub fn new(path: PathBuf) -> Self {
        let (notify, _) = watch::channel(0);
        Self {
            settings: Arc::new(RwLock::new(Settings::default())),
            path,
            notify,
            version: Arc::new(RwLock::new(0)),
        }
    }

    /// Loads settings from the default path.
    ///
    /// # Errors
    ///
    /// Returns error if settings cannot be loaded from disk.
    pub async fn load_default() -> Result<Self, StoreError> {
        Self::load(default_settings_path()).await
    }

    /// Loads settings from a path.
    ///
    /// # Errors
    ///
    /// Returns error if settings cannot be loaded from disk.
    pub async fn load(path: PathBuf) -> Result<Self, StoreError> {
        let settings = if path.exists() {
            info!(path = %path.display(), "Loading settings");
            load_json(&path).await.unwrap_or_else(|e| {
                warn!(error = %e, "Failed to load settings, using defaults");
                Settings::default()
            })
        } else {
            debug!(path = %path.display(), "Settings file not found, using defaults");
            Settings::default()
        };

        let (notify, _) = watch::channel(0);
        Ok(Self {
            settings: Arc::new(RwLock::new(settings)),
            path,
            notify,
            version: Arc::new(RwLock::new(0)),
        })
    }

    /// Gets a copy of the current settings.
    pub async fn get(&self) -> Settings {
        self.settings.read().await.clone()
    }

    /// Updates settings and notifies subscribers.
    pub async fn update<F>(&self, f: F)
    where
        F: FnOnce(&mut Settings),
    {
        {
            let mut settings = self.settings.write().await;
            f(&mut settings);
        }
        self.notify_change().await;
    }

    /// Saves settings to disk.
    ///
    /// # Errors
    ///
    /// Returns error if settings cannot be written to disk.
    pub async fn save(&self) -> Result<(), StoreError> {
        let settings = self.settings.read().await;
        save_json(&self.path, &*settings).await?;
        info!(path = %self.path.display(), "Settings saved");
        Ok(())
    }

    /// Subscribes to settings changes.
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
    // Convenience Methods
    // ========================================================================

    /// Checks if a provider is enabled.
    pub async fn is_provider_enabled(&self, provider: ProviderKind) -> bool {
        self.settings
            .read()
            .await
            .enabled_providers
            .contains(&provider)
    }

    /// Enables or disables a provider.
    pub async fn set_provider_enabled(&self, provider: ProviderKind, enabled: bool) {
        self.update(|s| {
            if enabled {
                s.enabled_providers.insert(provider);
            } else {
                s.enabled_providers.remove(&provider);
            }
        })
        .await;
    }

    /// Gets the refresh cadence.
    pub async fn refresh_cadence(&self) -> RefreshCadence {
        self.settings.read().await.refresh_cadence
    }

    /// Sets the refresh cadence.
    pub async fn set_refresh_cadence(&self, cadence: RefreshCadence) {
        self.update(|s| s.refresh_cadence = cadence).await;
    }

    /// Gets enabled providers.
    pub async fn enabled_providers(&self) -> HashSet<ProviderKind> {
        self.settings.read().await.enabled_providers.clone()
    }

    // ========================================================================
    // Display Settings Methods
    // ========================================================================

    /// Gets whether usage bars show "used" percentage.
    pub async fn usage_bars_show_used(&self) -> bool {
        self.settings.read().await.usage_bars_show_used
    }

    /// Sets whether usage bars show "used" percentage.
    pub async fn set_usage_bars_show_used(&self, value: bool) {
        self.update(|s| s.usage_bars_show_used = value).await;
    }

    /// Gets whether reset times show absolute values.
    pub async fn reset_times_show_absolute(&self) -> bool {
        self.settings.read().await.reset_times_show_absolute
    }

    /// Sets whether reset times show absolute values.
    pub async fn set_reset_times_show_absolute(&self, value: bool) {
        self.update(|s| s.reset_times_show_absolute = value).await;
    }

    /// Gets whether menu bar shows brand icon with percent.
    pub async fn menu_bar_shows_brand_icon_with_percent(&self) -> bool {
        self.settings
            .read()
            .await
            .menu_bar_shows_brand_icon_with_percent
    }

    /// Sets whether menu bar shows brand icon with percent.
    pub async fn set_menu_bar_shows_brand_icon_with_percent(&self, value: bool) {
        self.update(|s| s.menu_bar_shows_brand_icon_with_percent = value)
            .await;
    }

    /// Gets whether switcher shows provider icons.
    pub async fn switcher_shows_icons(&self) -> bool {
        self.settings.read().await.switcher_shows_icons
    }

    /// Sets whether switcher shows provider icons.
    pub async fn set_switcher_shows_icons(&self, value: bool) {
        self.update(|s| s.switcher_shows_icons = value).await;
    }

    // ========================================================================
    // Feature Toggle Methods
    // ========================================================================

    /// Gets whether status checks are enabled.
    pub async fn status_checks_enabled(&self) -> bool {
        self.settings.read().await.status_checks_enabled
    }

    /// Sets whether status checks are enabled.
    pub async fn set_status_checks_enabled(&self, value: bool) {
        self.update(|s| s.status_checks_enabled = value).await;
    }

    /// Gets whether session quota notifications are enabled.
    pub async fn session_quota_notifications_enabled(&self) -> bool {
        self.settings
            .read()
            .await
            .session_quota_notifications_enabled
    }

    /// Sets whether session quota notifications are enabled.
    pub async fn set_session_quota_notifications_enabled(&self, value: bool) {
        self.update(|s| s.session_quota_notifications_enabled = value)
            .await;
    }

    /// Gets whether cost usage tracking is enabled.
    pub async fn cost_usage_enabled(&self) -> bool {
        self.settings.read().await.cost_usage_enabled
    }

    /// Sets whether cost usage tracking is enabled.
    pub async fn set_cost_usage_enabled(&self, value: bool) {
        self.update(|s| s.cost_usage_enabled = value).await;
    }

    /// Gets whether random blink animation is enabled.
    pub async fn random_blink_enabled(&self) -> bool {
        self.settings.read().await.random_blink_enabled
    }

    /// Sets whether random blink animation is enabled.
    pub async fn set_random_blink_enabled(&self, value: bool) {
        self.update(|s| s.random_blink_enabled = value).await;
    }

    /// Gets whether Claude web extras are enabled.
    pub async fn claude_web_extras_enabled(&self) -> bool {
        self.settings.read().await.claude_web_extras_enabled
    }

    /// Sets whether Claude web extras are enabled.
    pub async fn set_claude_web_extras_enabled(&self, value: bool) {
        self.update(|s| s.claude_web_extras_enabled = value).await;
    }

    /// Gets whether optional credits/extra usage sections are shown.
    pub async fn show_optional_credits_and_extra_usage(&self) -> bool {
        self.settings
            .read()
            .await
            .show_optional_credits_and_extra_usage
    }

    /// Sets whether optional credits/extra usage sections are shown.
    pub async fn set_show_optional_credits_and_extra_usage(&self, value: bool) {
        self.update(|s| s.show_optional_credits_and_extra_usage = value)
            .await;
    }

    /// Gets whether `OpenAI` web access is enabled.
    pub async fn openai_web_access_enabled(&self) -> bool {
        self.settings.read().await.openai_web_access_enabled
    }

    /// Sets whether `OpenAI` web access is enabled.
    pub async fn set_openai_web_access_enabled(&self, value: bool) {
        self.update(|s| s.openai_web_access_enabled = value).await;
    }

    /// Gets the theme mode.
    pub async fn theme_mode(&self) -> ThemeMode {
        self.settings.read().await.theme_mode
    }

    /// Sets the theme mode.
    pub async fn set_theme_mode(&self, mode: ThemeMode) -> Result<(), StoreError> {
        self.update(|s| s.theme_mode = mode).await;
        Ok(())
    }

    // ========================================================================
    // Data Source Methods
    // ========================================================================

    /// Gets the Codex usage data source mode.
    pub async fn codex_usage_data_source(&self) -> DataSourceMode {
        self.settings.read().await.codex_usage_data_source
    }

    /// Sets the Codex usage data source mode.
    pub async fn set_codex_usage_data_source(&self, mode: DataSourceMode) {
        self.update(|s| s.codex_usage_data_source = mode).await;
    }

    /// Gets the Claude usage data source mode.
    pub async fn claude_usage_data_source(&self) -> DataSourceMode {
        self.settings.read().await.claude_usage_data_source
    }

    /// Sets the Claude usage data source mode.
    pub async fn set_claude_usage_data_source(&self, mode: DataSourceMode) {
        self.update(|s| s.claude_usage_data_source = mode).await;
    }

    // ========================================================================
    // Provider Order Methods
    // ========================================================================

    /// Gets the provider display order.
    /// Returns enabled providers in default order if not customized.
    pub async fn provider_order(&self) -> Vec<ProviderKind> {
        let settings = self.settings.read().await;
        if settings.provider_order.is_empty() {
            // Return default order if not customized
            settings.enabled_providers.iter().copied().collect()
        } else {
            settings.provider_order.clone()
        }
    }

    /// Sets the provider display order.
    pub async fn set_provider_order(&self, order: Vec<ProviderKind>) {
        self.update(|s| s.provider_order = order).await;
    }

    // ========================================================================
    // Per-Provider Cookie Source Methods
    // ========================================================================

    /// Gets the cookie source for a provider.
    pub async fn cookie_source(&self, provider: ProviderKind) -> CookieSource {
        self.settings
            .read()
            .await
            .provider_settings
            .get(&provider)
            .and_then(|ps| ps.cookie_source)
            .unwrap_or_default()
    }

    /// Sets the cookie source for a provider.
    pub async fn set_cookie_source(&self, provider: ProviderKind, source: CookieSource) {
        self.update(|s| {
            s.provider_settings
                .entry(provider)
                .or_default()
                .cookie_source = Some(source);
        })
        .await;
    }

    /// Gets the data source mode for a provider.
    pub async fn provider_source_mode(&self, provider: ProviderKind) -> DataSourceMode {
        self.settings
            .read()
            .await
            .provider_settings
            .get(&provider)
            .and_then(|ps| ps.source_mode)
            .unwrap_or_default()
    }

    /// Sets the data source mode for a provider.
    pub async fn set_provider_source_mode(&self, provider: ProviderKind, mode: DataSourceMode) {
        self.update(|s| {
            s.provider_settings.entry(provider).or_default().source_mode = Some(mode);
        })
        .await;
    }

    /// Gets the manual cookie header for a provider.
    pub async fn cookie_header(&self, provider: ProviderKind) -> Option<String> {
        self.settings
            .read()
            .await
            .provider_settings
            .get(&provider)
            .and_then(|ps| ps.cookie_header.clone())
    }

    /// Sets the manual cookie header for a provider.
    pub async fn set_cookie_header(&self, provider: ProviderKind, header: Option<String>) {
        self.update(|s| {
            s.provider_settings
                .entry(provider)
                .or_default()
                .cookie_header = header;
        })
        .await;
    }

    // ========================================================================
    // Debug & Detection Methods
    // ========================================================================

    /// Gets whether provider detection has completed.
    pub async fn provider_detection_completed(&self) -> bool {
        self.settings.read().await.provider_detection_completed
    }

    /// Sets whether provider detection has completed.
    pub async fn set_provider_detection_completed(&self, value: bool) {
        self.update(|s| s.provider_detection_completed = value)
            .await;
    }

    /// Gets the debug loading pattern.
    pub async fn debug_loading_pattern(&self) -> Option<String> {
        self.settings.read().await.debug_loading_pattern.clone()
    }

    /// Sets the debug loading pattern.
    pub async fn set_debug_loading_pattern(&self, pattern: Option<String>) {
        self.update(|s| s.debug_loading_pattern = pattern).await;
    }
}

// ============================================================================
// Tests
// ============================================================================

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_default_settings() {
        let settings = Settings::default();
        assert!(settings.enabled_providers.contains(&ProviderKind::Codex));
        assert!(settings.enabled_providers.contains(&ProviderKind::Claude));
        assert_eq!(settings.refresh_cadence, RefreshCadence::TwoMinutes);
    }

    #[test]
    fn test_refresh_cadence_duration() {
        assert_eq!(RefreshCadence::Manual.as_duration(), None);
        assert_eq!(
            RefreshCadence::TwoMinutes.as_duration(),
            Some(Duration::from_secs(120))
        );
    }

    #[tokio::test]
    async fn test_settings_store_update() {
        let store = SettingsStore::new(PathBuf::from("/tmp/test_settings.json"));

        store
            .update(|s| {
                s.debug_mode = true;
            })
            .await;

        let settings = store.get().await;
        assert!(settings.debug_mode);
    }

    #[tokio::test]
    async fn test_provider_toggle() {
        let store = SettingsStore::new(PathBuf::from("/tmp/test_settings.json"));

        assert!(store.is_provider_enabled(ProviderKind::Codex).await);

        store.set_provider_enabled(ProviderKind::Codex, false).await;
        assert!(!store.is_provider_enabled(ProviderKind::Codex).await);

        store.set_provider_enabled(ProviderKind::Codex, true).await;
        assert!(store.is_provider_enabled(ProviderKind::Codex).await);
    }

    #[test]
    fn test_default_settings_new_fields() {
        let settings = Settings::default();

        // Display settings defaults
        assert!(!settings.usage_bars_show_used);
        assert!(!settings.reset_times_show_absolute);
        assert!(!settings.menu_bar_shows_brand_icon_with_percent);
        assert!(settings.switcher_shows_icons);

        // Feature toggle defaults
        assert!(settings.status_checks_enabled);
        assert!(settings.session_quota_notifications_enabled);
        assert!(!settings.cost_usage_enabled);
        assert!(!settings.random_blink_enabled);
        assert!(!settings.claude_web_extras_enabled);
        assert!(settings.show_optional_credits_and_extra_usage);
        assert!(settings.openai_web_access_enabled);

        // Data source defaults
        assert_eq!(settings.codex_usage_data_source, DataSourceMode::Auto);
        assert_eq!(settings.claude_usage_data_source, DataSourceMode::Auto);

        // Provider order defaults
        assert!(settings.provider_order.is_empty());
        assert!(!settings.provider_detection_completed);
    }

    #[tokio::test]
    async fn test_display_settings_toggle() {
        let store = SettingsStore::new(PathBuf::from("/tmp/test_display_settings.json"));

        // Test usage bars toggle
        assert!(!store.usage_bars_show_used().await);
        store.set_usage_bars_show_used(true).await;
        assert!(store.usage_bars_show_used().await);

        // Test reset times toggle
        assert!(!store.reset_times_show_absolute().await);
        store.set_reset_times_show_absolute(true).await;
        assert!(store.reset_times_show_absolute().await);
    }

    #[tokio::test]
    async fn test_feature_toggles() {
        let store = SettingsStore::new(PathBuf::from("/tmp/test_feature_toggles.json"));

        // Status checks (default enabled)
        assert!(store.status_checks_enabled().await);
        store.set_status_checks_enabled(false).await;
        assert!(!store.status_checks_enabled().await);

        // Cost usage (default disabled)
        assert!(!store.cost_usage_enabled().await);
        store.set_cost_usage_enabled(true).await;
        assert!(store.cost_usage_enabled().await);
    }

    #[tokio::test]
    async fn test_provider_cookie_source() {
        let store = SettingsStore::new(PathBuf::from("/tmp/test_cookie_source.json"));

        // Default should be Auto
        assert_eq!(
            store.cookie_source(ProviderKind::Claude).await,
            CookieSource::Auto
        );

        // Set to Safari
        store
            .set_cookie_source(ProviderKind::Claude, CookieSource::Safari)
            .await;
        assert_eq!(
            store.cookie_source(ProviderKind::Claude).await,
            CookieSource::Safari
        );

        // Different provider should still be Auto
        assert_eq!(
            store.cookie_source(ProviderKind::Codex).await,
            CookieSource::Auto
        );
    }

    #[tokio::test]
    async fn test_provider_order() {
        let store = SettingsStore::new(PathBuf::from("/tmp/test_provider_order.json"));

        // Default should return enabled providers
        let order = store.provider_order().await;
        assert!(!order.is_empty());

        // Set custom order
        let custom_order = vec![ProviderKind::Claude, ProviderKind::Codex];
        store.set_provider_order(custom_order.clone()).await;
        assert_eq!(store.provider_order().await, custom_order);
    }

    #[test]
    fn test_data_source_mode_display() {
        assert_eq!(format!("{}", DataSourceMode::Auto), "Auto");
        assert_eq!(format!("{}", DataSourceMode::Cli), "CLI");
        assert_eq!(format!("{}", DataSourceMode::Web), "Web");
        assert_eq!(format!("{}", DataSourceMode::Api), "API");
    }

    #[test]
    fn test_cookie_source_display() {
        assert_eq!(format!("{}", CookieSource::Auto), "Auto");
        assert_eq!(format!("{}", CookieSource::Safari), "Safari");
        assert_eq!(format!("{}", CookieSource::Chrome), "Chrome");
        assert_eq!(format!("{}", CookieSource::Manual), "Manual");
    }

    #[tokio::test]
    async fn test_theme_mode_default() {
        let store = SettingsStore::new(PathBuf::from("/tmp/test_theme_default.json"));
        let settings = store.get().await;

        // Should default to Dark
        assert_eq!(settings.theme_mode, ThemeMode::Dark);
    }

    #[tokio::test]
    async fn test_theme_mode_persistence() {
        let store = SettingsStore::new(PathBuf::from("/tmp/test_theme_persist.json"));

        // Set to Light
        store.set_theme_mode(ThemeMode::Light).await.unwrap();
        assert_eq!(store.theme_mode().await, ThemeMode::Light);

        // Set to System
        store.set_theme_mode(ThemeMode::System).await.unwrap();
        assert_eq!(store.theme_mode().await, ThemeMode::System);

        // Set back to Dark
        store.set_theme_mode(ThemeMode::Dark).await.unwrap();
        assert_eq!(store.theme_mode().await, ThemeMode::Dark);
    }

    #[tokio::test]
    async fn test_theme_mode_serialization() {
        use crate::persistence::{load_json, save_json};
        use tempfile::tempdir;

        let dir = tempdir().unwrap();
        let path = dir.path().join("test_theme.json");

        // Create settings with Light theme
        let mut settings = Settings::default();
        settings.theme_mode = ThemeMode::Light;

        // Save to disk
        save_json(&path, &settings).await.unwrap();
        assert!(path.exists());

        // Load from disk
        let loaded: Settings = load_json(&path).await.unwrap();
        assert_eq!(loaded.theme_mode, ThemeMode::Light);

        // Test all modes
        settings.theme_mode = ThemeMode::System;
        save_json(&path, &settings).await.unwrap();
        let loaded: Settings = load_json(&path).await.unwrap();
        assert_eq!(loaded.theme_mode, ThemeMode::System);

        settings.theme_mode = ThemeMode::Dark;
        save_json(&path, &settings).await.unwrap();
        let loaded: Settings = load_json(&path).await.unwrap();
        assert_eq!(loaded.theme_mode, ThemeMode::Dark);
    }

    #[test]
    fn test_theme_mode_display() {
        assert_eq!(format!("{}", ThemeMode::Dark), "dark");
        assert_eq!(format!("{}", ThemeMode::Light), "light");
        assert_eq!(format!("{}", ThemeMode::System), "system");
    }

    #[test]
    fn test_theme_mode_equality() {
        assert_eq!(ThemeMode::Dark, ThemeMode::Dark);
        assert_eq!(ThemeMode::Light, ThemeMode::Light);
        assert_eq!(ThemeMode::System, ThemeMode::System);
        assert_ne!(ThemeMode::Dark, ThemeMode::Light);
        assert_ne!(ThemeMode::Dark, ThemeMode::System);
        assert_ne!(ThemeMode::Light, ThemeMode::System);
    }

    #[test]
    fn test_cookie_source_all() {
        let all = CookieSource::all();
        assert_eq!(all.len(), 10); // Auto, Off, Safari, Chrome, Firefox, Arc, Edge, Brave, Orion, Manual
        assert_eq!(all[0], CookieSource::Auto);
        assert_eq!(all[9], CookieSource::Manual);
    }
}
