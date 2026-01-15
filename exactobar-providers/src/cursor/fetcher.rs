//! Main Cursor usage fetcher.
//!
//! This module provides the primary entry point for fetching Cursor usage data.
//! It orchestrates multiple strategies with automatic fallback:
//!
//! 1. **Web API** (highest priority): Uses browser cookies for cursor.com
//! 2. **Local** (fallback): Reads cached data from local storage
//!
//! # Example
//!
//! ```ignore
//! let fetcher = CursorUsageFetcher::new();
//! let snapshot = fetcher.fetch_usage().await?;
//! ```

use exactobar_core::UsageSnapshot;
use exactobar_fetch::host::browser::{Browser, BrowserCookieImporter};
use tracing::{debug, info, instrument, warn};

use super::error::CursorError;
use super::local::CursorLocalReader;
use super::web::CursorWebClient;

// ============================================================================
// Data Source
// ============================================================================

/// Which data source to use for fetching.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum CursorDataSource {
    /// Try all strategies in priority order (Web -> Local).
    #[default]
    Auto,
    /// Use Web API only.
    Web,
    /// Use local cache only.
    Local,
}

// ============================================================================
// Fetcher
// ============================================================================

/// Main Cursor usage fetcher.
///
/// This fetcher tries multiple strategies in order:
/// 1. Web API (if browser cookies are available)
/// 2. Local cache (fallback)
#[derive(Debug, Clone, Default)]
pub struct CursorUsageFetcher {
    /// Which data source to use.
    data_source: CursorDataSource,
}

impl CursorUsageFetcher {
    /// Create a new fetcher with automatic strategy selection.
    pub fn new() -> Self {
        Self::default()
    }

    /// Create a fetcher with a specific data source.
    pub fn with_source(source: CursorDataSource) -> Self {
        Self {
            data_source: source,
        }
    }

    /// Create a fetcher that only uses web API.
    pub fn web_only() -> Self {
        Self::with_source(CursorDataSource::Web)
    }

    /// Create a fetcher that only uses local cache.
    pub fn local_only() -> Self {
        Self::with_source(CursorDataSource::Local)
    }

    /// Check if Cursor is installed.
    pub fn is_installed() -> bool {
        CursorLocalReader::is_installed()
    }

    /// Check if web cookies are available.
    pub async fn is_web_available() -> bool {
        let importer = BrowserCookieImporter::new();
        importer
            .import_cookies_auto("cursor.com", Browser::default_priority())
            .await
            .is_ok()
    }

    /// Detect the installed Cursor version.
    #[instrument]
    pub fn detect_version() -> Option<String> {
        // Try to read version from package.json or similar
        let config_dir = CursorLocalReader::config_dir()?;
        let version_path = config_dir.join("product.json");

        if version_path.exists() {
            let content = std::fs::read_to_string(&version_path).ok()?;
            let json: serde_json::Value = serde_json::from_str(&content).ok()?;
            return json.get("version")?.as_str().map(|s| s.to_string());
        }

        None
    }

    /// Fetch usage data using the configured strategy.
    #[instrument(skip(self))]
    pub async fn fetch_usage(&self) -> Result<UsageSnapshot, CursorError> {
        match self.data_source {
            CursorDataSource::Auto => self.fetch_auto().await,
            CursorDataSource::Web => self.fetch_via_web().await,
            CursorDataSource::Local => self.fetch_via_local(),
        }
    }

    /// Fetch with automatic strategy selection.
    #[instrument(skip(self))]
    async fn fetch_auto(&self) -> Result<UsageSnapshot, CursorError> {
        // Try web first
        match self.fetch_via_web().await {
            Ok(snapshot) => {
                info!(source = "web", "Fetched usage via web API");
                return Ok(snapshot);
            }
            Err(e) => {
                debug!(error = %e, "Web fetch failed, trying local");
            }
        }

        // Try local cache
        match self.fetch_via_local() {
            Ok(snapshot) => {
                info!(source = "local", "Fetched usage from local cache");
                return Ok(snapshot);
            }
            Err(e) => {
                warn!(error = %e, "Local fetch failed");
            }
        }

        Err(CursorError::AllStrategiesFailed)
    }

    /// Fetch using web cookies.
    #[instrument(skip(self))]
    async fn fetch_via_web(&self) -> Result<UsageSnapshot, CursorError> {
        debug!("Attempting web fetch");

        // Try to get cookies from browser
        let browser_importer = BrowserCookieImporter::new();
        let (browser, cookies) = browser_importer
            .import_cookies_auto("cursor.com", Browser::default_priority())
            .await
            .map_err(|e| CursorError::BrowserError(e.to_string()))?;

        debug!(browser = ?browser, cookie_count = cookies.len(), "Got browser cookies");

        if cookies.is_empty() {
            return Err(CursorError::BrowserError("No cookies found".to_string()));
        }

        // Build cookie header
        let cookie_header = BrowserCookieImporter::cookies_to_header(&cookies);

        if !CursorWebClient::has_session_cookie(&cookie_header) {
            return Err(CursorError::NoSessionCookie);
        }

        let client = CursorWebClient::new();
        let response = client.fetch_usage(&cookie_header).await?;
        let snapshot = response.to_snapshot();

        Ok(snapshot)
    }

    /// Fetch from local cache.
    #[instrument(skip(self))]
    fn fetch_via_local(&self) -> Result<UsageSnapshot, CursorError> {
        debug!("Attempting local fetch");

        if !Self::is_installed() {
            return Err(CursorError::ConfigNotFound(
                "Cursor not installed".to_string(),
            ));
        }

        let reader = CursorLocalReader::new();
        reader.read_cached_usage()
    }
}

// ============================================================================
// Tests
// ============================================================================

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_fetcher_creation() {
        let default = CursorUsageFetcher::new();
        assert_eq!(default.data_source, CursorDataSource::Auto);

        let web = CursorUsageFetcher::web_only();
        assert_eq!(web.data_source, CursorDataSource::Web);

        let local = CursorUsageFetcher::local_only();
        assert_eq!(local.data_source, CursorDataSource::Local);
    }

    #[test]
    fn test_data_source_default() {
        let source = CursorDataSource::default();
        assert_eq!(source, CursorDataSource::Auto);
    }

    #[test]
    fn test_is_installed() {
        // Just test the function runs
        let _ = CursorUsageFetcher::is_installed();
    }

    #[test]
    fn test_detect_version() {
        // Just test the function runs
        let _ = CursorUsageFetcher::detect_version();
    }

    #[test]
    fn test_with_source() {
        let fetcher = CursorUsageFetcher::with_source(CursorDataSource::Local);
        assert_eq!(fetcher.data_source, CursorDataSource::Local);
    }
}
