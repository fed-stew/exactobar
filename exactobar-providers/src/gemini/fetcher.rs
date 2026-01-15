//! Main Gemini usage fetcher.
//!
//! This module provides the primary entry point for fetching Gemini usage data.
//! It orchestrates multiple strategies with automatic fallback:
//!
//! 1. **gcloud OAuth** (highest priority): Uses gcloud credentials
//! 2. **CLI** (fallback): Uses gcloud CLI commands
//!
//! # Example
//!
//! ```ignore
//! let fetcher = GeminiUsageFetcher::new();
//! let snapshot = fetcher.fetch_usage().await?;
//! ```

use exactobar_core::UsageSnapshot;
use tracing::{debug, info, instrument, warn};

use super::api::GeminiApiClient;
use super::error::GeminiError;
use super::gcloud::GcloudCredentials;
use super::probe::GeminiProbe;
use super::pty_probe::GeminiPtyProbe;

// ============================================================================
// Data Source
// ============================================================================

/// Which data source to use for fetching.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum GeminiDataSource {
    /// Try all strategies in priority order.
    #[default]
    Auto,
    /// Use gcloud OAuth only.
    OAuth,
    /// Use CLI/PTY only.
    Cli,
}

// ============================================================================
// Fetcher
// ============================================================================

/// Main Gemini usage fetcher.
#[derive(Debug, Clone, Default)]
pub struct GeminiUsageFetcher {
    /// Which data source to use.
    data_source: GeminiDataSource,
}

impl GeminiUsageFetcher {
    /// Create a new fetcher with automatic strategy selection.
    pub fn new() -> Self {
        Self::default()
    }

    /// Create a fetcher with a specific data source.
    pub fn with_source(source: GeminiDataSource) -> Self {
        Self {
            data_source: source,
        }
    }

    /// Create a fetcher that only uses OAuth.
    pub fn oauth_only() -> Self {
        Self::with_source(GeminiDataSource::OAuth)
    }

    /// Create a fetcher that only uses CLI.
    pub fn cli_only() -> Self {
        Self::with_source(GeminiDataSource::Cli)
    }

    /// Check if gcloud CLI is available.
    pub fn is_gcloud_available() -> bool {
        GcloudCredentials::is_cli_available()
    }

    /// Check if ADC credentials exist.
    pub fn has_adc() -> bool {
        GcloudCredentials::has_adc()
    }

    /// Check if Gemini CLI credentials exist (~/.gemini/).
    pub fn has_gemini_cli_creds() -> bool {
        GeminiProbe::is_available()
    }

    /// Detect the installed gcloud version.
    #[instrument]
    pub fn detect_gcloud_version() -> Option<String> {
        let output = std::process::Command::new("gcloud")
            .arg("--version")
            .output()
            .ok()?;

        if !output.status.success() {
            return None;
        }

        let stdout = String::from_utf8_lossy(&output.stdout);
        let version = stdout
            .lines()
            .find(|l| l.starts_with("Google Cloud SDK"))
            .and_then(|l| l.split_whitespace().last())
            .map(|s| s.to_string());

        version
    }

    /// Fetch usage data using the configured strategy.
    #[instrument(skip(self))]
    pub async fn fetch_usage(&self) -> Result<UsageSnapshot, GeminiError> {
        match self.data_source {
            GeminiDataSource::Auto => self.fetch_auto().await,
            GeminiDataSource::OAuth => self.fetch_via_oauth().await,
            GeminiDataSource::Cli => self.fetch_via_cli().await,
        }
    }

    /// Fetch with automatic strategy selection.
    #[instrument(skip(self))]
    async fn fetch_auto(&self) -> Result<UsageSnapshot, GeminiError> {
        // Try OAuth first
        match self.fetch_via_oauth().await {
            Ok(snapshot) => {
                info!(source = "oauth", "Fetched usage via OAuth");
                return Ok(snapshot);
            }
            Err(e) => {
                debug!(error = %e, "OAuth fetch failed, trying CLI");
            }
        }

        // Try CLI
        match self.fetch_via_cli().await {
            Ok(snapshot) => {
                info!(source = "cli", "Fetched usage via CLI");
                return Ok(snapshot);
            }
            Err(e) => {
                warn!(error = %e, "CLI fetch failed");
            }
        }

        Err(GeminiError::AllStrategiesFailed)
    }

    /// Fetch using OAuth credentials.
    ///
    /// This tries two approaches:
    /// 1. Gemini CLI credentials (~/.gemini/oauth_creds.json) - preferred
    /// 2. gcloud credentials - fallback
    #[instrument(skip(self))]
    async fn fetch_via_oauth(&self) -> Result<UsageSnapshot, GeminiError> {
        debug!("Attempting OAuth fetch");

        // Try Gemini CLI credentials first (preferred - gets actual quota data)
        if GeminiProbe::is_available() {
            debug!("Trying Gemini CLI OAuth credentials");
            let probe = GeminiProbe::new();
            match probe.fetch().await {
                Ok(snapshot_data) => {
                    if snapshot_data.has_data() {
                        info!(source = "gemini-cli", "Fetched usage via Gemini CLI OAuth");
                        return Ok(snapshot_data.to_usage_snapshot());
                    }
                    debug!("Gemini CLI OAuth returned no data, trying gcloud");
                }
                Err(e) => {
                    debug!(error = %e, "Gemini CLI OAuth failed, trying gcloud");
                }
            }
        }

        // Fallback to gcloud credentials
        debug!("Trying gcloud OAuth credentials");
        let creds = GcloudCredentials::new();
        let token = creds.load().await?;

        let client = GeminiApiClient::new();
        let quota = client
            .fetch_all(
                &token.access_token,
                token.account.clone(),
                token.project.clone(),
            )
            .await?;

        if !quota.has_data() {
            return Err(GeminiError::NoData);
        }

        let snapshot = quota.to_snapshot();
        Ok(snapshot)
    }

    /// Fetch using CLI commands.
    #[instrument(skip(self))]
    async fn fetch_via_cli(&self) -> Result<UsageSnapshot, GeminiError> {
        debug!("Attempting CLI fetch");

        let probe = GeminiPtyProbe::new();
        let quota = probe.fetch_quota().await?;

        if !quota.has_data() {
            return Err(GeminiError::NoData);
        }

        let snapshot = quota.to_snapshot();
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
        let default = GeminiUsageFetcher::new();
        assert_eq!(default.data_source, GeminiDataSource::Auto);

        let oauth = GeminiUsageFetcher::oauth_only();
        assert_eq!(oauth.data_source, GeminiDataSource::OAuth);

        let cli = GeminiUsageFetcher::cli_only();
        assert_eq!(cli.data_source, GeminiDataSource::Cli);
    }

    #[test]
    fn test_data_source_default() {
        let source = GeminiDataSource::default();
        assert_eq!(source, GeminiDataSource::Auto);
    }

    #[test]
    fn test_is_gcloud_available() {
        // Just test the function runs
        let _ = GeminiUsageFetcher::is_gcloud_available();
    }

    #[test]
    fn test_has_adc() {
        // Just test the function runs
        let _ = GeminiUsageFetcher::has_adc();
    }

    #[test]
    fn test_detect_gcloud_version() {
        // Just test the function runs
        let _ = GeminiUsageFetcher::detect_gcloud_version();
    }

    #[test]
    fn test_with_source() {
        let fetcher = GeminiUsageFetcher::with_source(GeminiDataSource::Cli);
        assert_eq!(fetcher.data_source, GeminiDataSource::Cli);
    }
}
