//! Fetch context providing access to host APIs.
//!
//! The fetch context is passed to all strategies and provides unified
//! access to system resources like keychain, HTTP client, process runner, etc.

use std::sync::Arc;
use std::time::Duration;

use tracing::warn;

use crate::host::{
    browser::BrowserCookieImporter, http::HttpClient, keychain::KeychainApi,
    keychain::SystemKeychain, process::ProcessRunner, status::StatusPoller,
};

// ============================================================================
// Source Mode
// ============================================================================

/// How to select fetch strategies.
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq)]
pub enum SourceMode {
    /// Automatically select the best available strategy.
    #[default]
    Auto,
    /// Only use CLI strategies.
    CLI,
    /// Only use web-based strategies (cookies, dashboard).
    Web,
    /// Only use OAuth strategies.
    OAuth,
    /// Only use API key strategies.
    ApiKey,
}

impl SourceMode {
    /// Returns true if this mode allows CLI strategies.
    pub fn allows_cli(&self) -> bool {
        matches!(self, Self::Auto | Self::CLI)
    }

    /// Returns true if this mode allows web strategies.
    pub fn allows_web(&self) -> bool {
        matches!(self, Self::Auto | Self::Web)
    }

    /// Returns true if this mode allows OAuth strategies.
    pub fn allows_oauth(&self) -> bool {
        matches!(self, Self::Auto | Self::OAuth)
    }

    /// Returns true if this mode allows API key strategies.
    pub fn allows_api_key(&self) -> bool {
        matches!(self, Self::Auto | Self::ApiKey)
    }
}

// ============================================================================
// Fetch Settings
// ============================================================================

/// Settings for fetch operations.
#[derive(Debug, Clone)]
pub struct FetchSettings {
    /// Which source modes to allow.
    pub source_mode: SourceMode,
    /// Timeout for fetch operations.
    pub timeout: Duration,
    /// Whether to dump HTML for debugging web strategies.
    pub web_debug_dump_html: bool,
    /// Maximum retries on transient failures.
    pub max_retries: u32,
    /// Delay between retries.
    pub retry_delay: Duration,
}

impl Default for FetchSettings {
    fn default() -> Self {
        Self {
            source_mode: SourceMode::Auto,
            timeout: Duration::from_secs(30),
            web_debug_dump_html: false,
            max_retries: 2,
            retry_delay: Duration::from_secs(1),
        }
    }
}

impl FetchSettings {
    /// Creates settings for CLI-only mode.
    pub fn cli_only() -> Self {
        Self {
            source_mode: SourceMode::CLI,
            ..Default::default()
        }
    }

    /// Creates settings for web-only mode.
    pub fn web_only() -> Self {
        Self {
            source_mode: SourceMode::Web,
            ..Default::default()
        }
    }

    /// Creates settings with custom timeout.
    pub fn with_timeout(mut self, timeout: Duration) -> Self {
        self.timeout = timeout;
        self
    }

    /// Creates settings with debug HTML dumping enabled.
    pub fn with_debug_html(mut self) -> Self {
        self.web_debug_dump_html = true;
        self
    }
}

// ============================================================================
// Fetch Context
// ============================================================================

/// Context provided to fetch strategies, giving access to host APIs.
///
/// The context bundles all the host APIs that strategies might need:
/// - Keychain for credential storage
/// - HTTP client for network requests
/// - Process runner for CLI commands
/// - Browser cookie importer for web strategies
/// - Status poller for health checks
pub struct FetchContext {
    /// Secure credential storage.
    pub keychain: Arc<dyn KeychainApi>,
    /// HTTP client with tracing.
    pub http: Arc<HttpClient>,
    /// Process runner for CLI tools.
    pub process: Arc<ProcessRunner>,
    /// Browser cookie importer.
    pub browser: Arc<BrowserCookieImporter>,
    /// Status page poller.
    pub status: Arc<StatusPoller>,
    /// Fetch settings.
    pub settings: FetchSettings,
}

impl FetchContext {
    /// Creates a new fetch context with default host API implementations.
    pub fn new() -> Self {
        Self::with_settings(FetchSettings::default())
    }

    /// Creates a context with custom settings.
    pub fn with_settings(settings: FetchSettings) -> Self {
        // Security warning for debug mode
        if settings.web_debug_dump_html {
            warn!(
                "⚠️  HTML debug dumping enabled - this may write sensitive content to disk. \
                DO NOT USE IN PRODUCTION."
            );
        }

        Self {
            keychain: Arc::new(SystemKeychain::new()),
            http: Arc::new(HttpClient::new()),
            process: Arc::new(ProcessRunner::new()),
            browser: Arc::new(BrowserCookieImporter::new()),
            status: Arc::new(StatusPoller::new()),
            settings,
        }
    }

    /// Creates a builder for customizing the context.
    pub fn builder() -> FetchContextBuilder {
        FetchContextBuilder::new()
    }

    /// Returns the effective timeout for fetch operations.
    pub fn timeout(&self) -> Duration {
        self.settings.timeout
    }

    /// Returns true if the given source mode is allowed.
    pub fn allows_source(&self, mode: SourceMode) -> bool {
        self.settings.source_mode == SourceMode::Auto || self.settings.source_mode == mode
    }
}

impl Default for FetchContext {
    fn default() -> Self {
        Self::new()
    }
}

impl std::fmt::Debug for FetchContext {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("FetchContext")
            .field("settings", &self.settings)
            .finish_non_exhaustive()
    }
}

// ============================================================================
// Fetch Context Builder
// ============================================================================

/// Builder for constructing a `FetchContext`.
pub struct FetchContextBuilder {
    keychain: Option<Arc<dyn KeychainApi>>,
    http: Option<Arc<HttpClient>>,
    process: Option<Arc<ProcessRunner>>,
    browser: Option<Arc<BrowserCookieImporter>>,
    status: Option<Arc<StatusPoller>>,
    settings: FetchSettings,
}

impl FetchContextBuilder {
    /// Creates a new builder.
    pub fn new() -> Self {
        Self {
            keychain: None,
            http: None,
            process: None,
            browser: None,
            status: None,
            settings: FetchSettings::default(),
        }
    }

    /// Sets the keychain implementation.
    pub fn keychain(mut self, keychain: Arc<dyn KeychainApi>) -> Self {
        self.keychain = Some(keychain);
        self
    }

    /// Sets the HTTP client.
    pub fn http(mut self, http: Arc<HttpClient>) -> Self {
        self.http = Some(http);
        self
    }

    /// Sets the process runner.
    pub fn process(mut self, process: Arc<ProcessRunner>) -> Self {
        self.process = Some(process);
        self
    }

    /// Sets the browser cookie importer.
    pub fn browser(mut self, browser: Arc<BrowserCookieImporter>) -> Self {
        self.browser = Some(browser);
        self
    }

    /// Sets the status poller.
    pub fn status(mut self, status: Arc<StatusPoller>) -> Self {
        self.status = Some(status);
        self
    }

    /// Sets the fetch settings.
    pub fn settings(mut self, settings: FetchSettings) -> Self {
        self.settings = settings;
        self
    }

    /// Sets the source mode.
    pub fn source_mode(mut self, mode: SourceMode) -> Self {
        self.settings.source_mode = mode;
        self
    }

    /// Sets the timeout.
    pub fn timeout(mut self, timeout: Duration) -> Self {
        self.settings.timeout = timeout;
        self
    }

    /// Builds the fetch context.
    pub fn build(self) -> FetchContext {
        FetchContext {
            keychain: self.keychain.unwrap_or_else(|| Arc::new(SystemKeychain::new())),
            http: self.http.unwrap_or_else(|| Arc::new(HttpClient::new())),
            process: self.process.unwrap_or_else(|| Arc::new(ProcessRunner::new())),
            browser: self.browser.unwrap_or_else(|| Arc::new(BrowserCookieImporter::new())),
            status: self.status.unwrap_or_else(|| Arc::new(StatusPoller::new())),
            settings: self.settings,
        }
    }
}

impl Default for FetchContextBuilder {
    fn default() -> Self {
        Self::new()
    }
}

// ============================================================================
// Tests
// ============================================================================

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_source_mode_allows() {
        assert!(SourceMode::Auto.allows_cli());
        assert!(SourceMode::Auto.allows_web());
        assert!(SourceMode::Auto.allows_oauth());
        assert!(SourceMode::Auto.allows_api_key());

        assert!(SourceMode::CLI.allows_cli());
        assert!(!SourceMode::CLI.allows_web());

        assert!(!SourceMode::Web.allows_cli());
        assert!(SourceMode::Web.allows_web());
    }

    #[test]
    fn test_context_builder() {
        let ctx = FetchContext::builder()
            .source_mode(SourceMode::CLI)
            .timeout(Duration::from_secs(60))
            .build();

        assert_eq!(ctx.settings.source_mode, SourceMode::CLI);
        assert_eq!(ctx.settings.timeout, Duration::from_secs(60));
    }

    #[test]
    fn test_default_context() {
        let ctx = FetchContext::new();
        assert_eq!(ctx.settings.source_mode, SourceMode::Auto);
        assert_eq!(ctx.settings.timeout, Duration::from_secs(30));
    }
}
