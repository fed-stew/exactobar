//! Main Claude usage fetcher.
//!
//! This module provides the primary entry point for fetching Claude usage data.
//! It orchestrates multiple strategies with automatic fallback:
//!
//! 1. **OAuth API** (highest priority): Uses OAuth tokens for API access
//! 2. **Web API** (medium priority): Uses browser cookies for claude.ai
//! 3. **PTY fallback** (lowest priority): Interactive `/usage` command
//!
//! # Example
//!
//! ```ignore
//! let fetcher = ClaudeUsageFetcher::new();
//! let snapshot = fetcher.fetch_usage().await?;
//! ```

use exactobar_core::UsageSnapshot;
use tracing::{debug, info, instrument, warn};

use super::api::ClaudeApiClient;
use super::error::ClaudeError;
use super::oauth::ClaudeOAuthCredentials;
use super::pty_probe::ClaudePtyProbe;
use super::web::ClaudeWebClient;

// ============================================================================
// Data Source
// ============================================================================

/// Which data source to use for fetching.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum ClaudeDataSource {
    /// Try all strategies in priority order (OAuth -> Web -> CLI).
    #[default]
    Auto,
    /// Use OAuth API only.
    OAuth,
    /// Use Web API only.
    Web,
    /// Use CLI/PTY only.
    Cli,
}

// ============================================================================
// Fetcher
// ============================================================================

/// Main Claude usage fetcher.
///
/// This fetcher tries multiple strategies in order:
/// 1. OAuth API (if credentials exist and are valid)
/// 2. Web API (if browser cookies are available)
/// 3. PTY with `/usage` command (fallback)
#[derive(Debug, Clone, Default)]
pub struct ClaudeUsageFetcher {
    /// Which data source to use.
    data_source: ClaudeDataSource,
}

impl ClaudeUsageFetcher {
    /// Create a new fetcher with automatic strategy selection.
    pub fn new() -> Self {
        Self::default()
    }

    /// Create a fetcher with a specific data source.
    pub fn with_source(source: ClaudeDataSource) -> Self {
        Self {
            data_source: source,
        }
    }

    /// Create a fetcher that only uses OAuth.
    pub fn oauth_only() -> Self {
        Self::with_source(ClaudeDataSource::OAuth)
    }

    /// Create a fetcher that only uses web cookies.
    pub fn web_only() -> Self {
        Self::with_source(ClaudeDataSource::Web)
    }

    /// Create a fetcher that only uses CLI/PTY.
    pub fn cli_only() -> Self {
        Self::with_source(ClaudeDataSource::Cli)
    }

    /// Check if claude CLI is available.
    pub fn is_cli_available() -> bool {
        which::which("claude").is_ok()
    }

    /// Check if OAuth credentials are available.
    pub fn is_oauth_available() -> bool {
        ClaudeOAuthCredentials::load().is_ok()
    }

    /// Detect the installed claude version.
    #[instrument]
    pub fn detect_version() -> Option<String> {
        let output = std::process::Command::new("claude")
            .arg("--version")
            .output()
            .ok()?;

        if !output.status.success() {
            return None;
        }

        let stdout = String::from_utf8_lossy(&output.stdout);
        let version = stdout
            .lines()
            .next()?
            .trim()
            .trim_start_matches("claude")
            .trim()
            .to_string();

        if version.is_empty() {
            None
        } else {
            Some(version)
        }
    }

    /// Fetch usage data using the configured strategy.
    #[instrument(skip(self))]
    pub async fn fetch_usage(&self) -> Result<UsageSnapshot, ClaudeError> {
        match self.data_source {
            ClaudeDataSource::Auto => self.fetch_auto().await,
            ClaudeDataSource::OAuth => self.fetch_via_oauth().await,
            ClaudeDataSource::Web => self.fetch_via_web().await,
            ClaudeDataSource::Cli => self.fetch_via_pty().await,
        }
    }

    /// Fetch with automatic strategy selection.
    #[instrument(skip(self))]
    async fn fetch_auto(&self) -> Result<UsageSnapshot, ClaudeError> {
        // Try OAuth first
        match self.fetch_via_oauth().await {
            Ok(snapshot) => {
                info!(source = "oauth", "Fetched usage via OAuth API");
                return Ok(snapshot);
            }
            Err(e) => {
                debug!(error = %e, "OAuth fetch failed, trying web");
            }
        }

        // Try web cookies
        match self.fetch_via_web().await {
            Ok(snapshot) => {
                info!(source = "web", "Fetched usage via web API");
                return Ok(snapshot);
            }
            Err(e) => {
                debug!(error = %e, "Web fetch failed, trying PTY");
            }
        }

        // Fallback to PTY
        match self.fetch_via_pty().await {
            Ok(snapshot) => {
                info!(source = "pty", "Fetched usage via PTY");
                return Ok(snapshot);
            }
            Err(e) => {
                warn!(error = %e, "PTY fetch failed");
            }
        }

        Err(ClaudeError::AllStrategiesFailed)
    }

    /// Fetch using OAuth API.
    #[instrument(skip(self))]
    async fn fetch_via_oauth(&self) -> Result<UsageSnapshot, ClaudeError> {
        debug!("Attempting OAuth fetch");

        let credentials = ClaudeOAuthCredentials::load()?;

        if credentials.is_expired() {
            return Err(ClaudeError::TokenExpired(
                credentials
                    .expires_at
                    .map(|t| t.to_rfc3339())
                    .unwrap_or_else(|| "unknown".to_string()),
            ));
        }

        let client = ClaudeApiClient::new();
        let response = client.fetch_usage(&credentials).await?;
        let snapshot = response.to_snapshot();

        Ok(snapshot)
    }

    /// Fetch using web cookies.
    #[instrument(skip(self))]
    async fn fetch_via_web(&self) -> Result<UsageSnapshot, ClaudeError> {
        debug!("Attempting web fetch");

        // Try to get cookies from browser
        let browser_importer = exactobar_fetch::host::browser::BrowserCookieImporter::new();
        let (browser, cookies) = browser_importer
            .import_cookies_auto(
                super::web::CLAUDE_DOMAIN,
                exactobar_fetch::host::browser::Browser::default_priority(),
            )
            .await
            .map_err(|e| ClaudeError::BrowserError(e.to_string()))?;

        debug!(browser = ?browser, cookie_count = cookies.len(), "Got browser cookies");

        if cookies.is_empty() {
            return Err(ClaudeError::BrowserError("No cookies found".to_string()));
        }

        // Build cookie header
        let cookie_header =
            exactobar_fetch::host::browser::BrowserCookieImporter::cookies_to_header(&cookies);

        if !ClaudeWebClient::has_session_cookie(&cookie_header) {
            return Err(ClaudeError::BrowserError(
                "No session cookie found".to_string(),
            ));
        }

        let client = ClaudeWebClient::new();
        let response = client.fetch_usage(&cookie_header, None).await?;
        let snapshot = response.to_snapshot();

        Ok(snapshot)
    }

    /// Fetch using PTY with /usage command.
    #[instrument(skip(self))]
    async fn fetch_via_pty(&self) -> Result<UsageSnapshot, ClaudeError> {
        debug!("Attempting PTY fetch");

        if !Self::is_cli_available() {
            return Err(ClaudeError::BinaryNotFound("claude".to_string()));
        }

        let probe = ClaudePtyProbe::new();
        let status = probe.fetch_usage().await?;

        if !status.has_data() {
            return Err(ClaudeError::NoData);
        }

        let snapshot = status.to_snapshot();
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
    fn test_fetcher_creation() {
        let default = ClaudeUsageFetcher::new();
        assert_eq!(default.data_source, ClaudeDataSource::Auto);

        let oauth = ClaudeUsageFetcher::oauth_only();
        assert_eq!(oauth.data_source, ClaudeDataSource::OAuth);

        let web = ClaudeUsageFetcher::web_only();
        assert_eq!(web.data_source, ClaudeDataSource::Web);

        let cli = ClaudeUsageFetcher::cli_only();
        assert_eq!(cli.data_source, ClaudeDataSource::Cli);
    }

    #[test]
    fn test_data_source_default() {
        let source = ClaudeDataSource::default();
        assert_eq!(source, ClaudeDataSource::Auto);
    }

    #[test]
    fn test_is_cli_available() {
        // Just test the function runs
        let _ = ClaudeUsageFetcher::is_cli_available();
    }

    #[test]
    fn test_is_oauth_available() {
        // Just test the function runs
        let _ = ClaudeUsageFetcher::is_oauth_available();
    }

    #[test]
    fn test_detect_version() {
        // Just test the function runs
        let _ = ClaudeUsageFetcher::detect_version();
    }

    #[test]
    fn test_with_source() {
        let fetcher = ClaudeUsageFetcher::with_source(ClaudeDataSource::Web);
        assert_eq!(fetcher.data_source, ClaudeDataSource::Web);
    }
}
