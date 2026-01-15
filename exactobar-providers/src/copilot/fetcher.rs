//! Main Copilot usage fetcher.
//!
//! This module provides the primary entry point for fetching Copilot usage data.
//! It orchestrates multiple strategies with automatic fallback:
//!
//! 1. **OAuth API** (highest priority): Uses OAuth tokens from keychain/gh CLI
//! 2. **Environment** (fallback): Uses COPILOT_API_TOKEN or GITHUB_TOKEN
//!
//! # Example
//!
//! ```ignore
//! let fetcher = CopilotUsageFetcher::new();
//! let snapshot = fetcher.fetch_usage().await?;
//! ```

use exactobar_core::UsageSnapshot;
use tracing::{debug, info, instrument, warn};

use super::api::CopilotApiClient;
use super::device_flow::{CopilotDeviceFlow, DeviceFlowStart};
use super::error::CopilotError;
use super::token_store::CopilotTokenStore;

// ============================================================================
// Data Source
// ============================================================================

/// Which data source to use for fetching.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum CopilotDataSource {
    /// Try all strategies in priority order.
    #[default]
    Auto,
    /// Use OAuth API with keychain/file token.
    OAuth,
    /// Use environment variable.
    Env,
}

// ============================================================================
// Fetcher
// ============================================================================

/// Main Copilot usage fetcher.
#[derive(Debug, Clone, Default)]
pub struct CopilotUsageFetcher {
    /// Which data source to use.
    data_source: CopilotDataSource,
}

impl CopilotUsageFetcher {
    /// Create a new fetcher with automatic strategy selection.
    pub fn new() -> Self {
        Self::default()
    }

    /// Create a fetcher with a specific data source.
    pub fn with_source(source: CopilotDataSource) -> Self {
        Self {
            data_source: source,
        }
    }

    /// Create a fetcher that only uses OAuth.
    pub fn oauth_only() -> Self {
        Self::with_source(CopilotDataSource::OAuth)
    }

    /// Create a fetcher that only uses environment variables.
    pub fn env_only() -> Self {
        Self::with_source(CopilotDataSource::Env)
    }

    /// Check if any token source is available.
    pub fn is_available() -> bool {
        let store = CopilotTokenStore::new();
        store.is_available()
    }

    /// Check if gh CLI is installed.
    pub fn is_gh_cli_available() -> bool {
        which::which("gh").is_ok()
    }

    /// Detect the installed gh CLI version.
    #[instrument]
    pub fn detect_gh_version() -> Option<String> {
        let output = std::process::Command::new("gh").arg("--version").output().ok()?;

        if !output.status.success() {
            return None;
        }

        let stdout = String::from_utf8_lossy(&output.stdout);
        let version = stdout
            .lines()
            .next()?
            .trim()
            .strip_prefix("gh version ")
            .map(|s| s.split_whitespace().next().unwrap_or(s))
            .map(|s| s.to_string());

        version
    }

    /// Start device flow authentication.
    ///
    /// Returns the device flow start info. The caller should:
    /// 1. Display the verification URL and user code to the user
    /// 2. Call `complete_device_flow` with the device code
    #[instrument]
    pub async fn start_device_flow() -> Result<DeviceFlowStart, CopilotError> {
        let flow = CopilotDeviceFlow::new();
        flow.start().await
    }

    /// Complete device flow and store token.
    ///
    /// Polls GitHub until the user authorizes, then stores the token.
    #[instrument(skip(device_code))]
    pub async fn complete_device_flow(device_code: &str) -> Result<String, CopilotError> {
        let flow = CopilotDeviceFlow::new();

        loop {
            match flow.poll(device_code).await? {
                super::device_flow::DeviceFlowResult::Pending => {
                    tokio::time::sleep(std::time::Duration::from_secs(5)).await;
                }
                super::device_flow::DeviceFlowResult::SlowDown => {
                    tokio::time::sleep(std::time::Duration::from_secs(10)).await;
                }
                super::device_flow::DeviceFlowResult::AccessToken(response) => {
                    // Store the token
                    let store = CopilotTokenStore::new();
                    if let Err(e) = store.save_to_keychain(&response.access_token) {
                        warn!(error = %e, "Failed to save to keychain, trying file");
                        store.save_to_file(&response.access_token)?;
                    }

                    return Ok(response.access_token);
                }
                super::device_flow::DeviceFlowResult::Expired => {
                    return Err(CopilotError::DeviceFlowExpired);
                }
                super::device_flow::DeviceFlowResult::AccessDenied => {
                    return Err(CopilotError::AuthenticationFailed(
                        "User denied access".to_string(),
                    ));
                }
            }
        }
    }

    /// Fetch usage data using the configured strategy.
    #[instrument(skip(self))]
    pub async fn fetch_usage(&self) -> Result<UsageSnapshot, CopilotError> {
        match self.data_source {
            CopilotDataSource::Auto => self.fetch_auto().await,
            CopilotDataSource::OAuth => self.fetch_via_oauth().await,
            CopilotDataSource::Env => self.fetch_via_env().await,
        }
    }

    /// Fetch with automatic strategy selection.
    #[instrument(skip(self))]
    async fn fetch_auto(&self) -> Result<UsageSnapshot, CopilotError> {
        // Try OAuth first
        match self.fetch_via_oauth().await {
            Ok(snapshot) => {
                info!(source = "oauth", "Fetched usage via OAuth");
                return Ok(snapshot);
            }
            Err(e) => {
                debug!(error = %e, "OAuth fetch failed, trying env");
            }
        }

        // Try environment
        match self.fetch_via_env().await {
            Ok(snapshot) => {
                info!(source = "env", "Fetched usage via environment token");
                return Ok(snapshot);
            }
            Err(e) => {
                warn!(error = %e, "Env fetch failed");
            }
        }

        Err(CopilotError::AllStrategiesFailed)
    }

    /// Fetch using OAuth token from keychain/file.
    #[instrument(skip(self))]
    async fn fetch_via_oauth(&self) -> Result<UsageSnapshot, CopilotError> {
        debug!("Attempting OAuth fetch");

        let store = CopilotTokenStore::new();
        let token = store.load().ok_or(CopilotError::NoToken)?;

        let client = CopilotApiClient::new();
        let data = client.fetch_all(&token).await?;

        if !data.is_enabled() && data.user.is_none() {
            return Err(CopilotError::NoData);
        }

        let snapshot = data.to_snapshot();
        Ok(snapshot)
    }

    /// Fetch using environment variable token.
    #[instrument(skip(self))]
    async fn fetch_via_env(&self) -> Result<UsageSnapshot, CopilotError> {
        debug!("Attempting env fetch");

        let token = CopilotTokenStore::load_from_env().ok_or(CopilotError::NoToken)?;

        let client = CopilotApiClient::new();
        let data = client.fetch_all(&token).await?;

        let snapshot = data.to_snapshot();
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
        let default = CopilotUsageFetcher::new();
        assert_eq!(default.data_source, CopilotDataSource::Auto);

        let oauth = CopilotUsageFetcher::oauth_only();
        assert_eq!(oauth.data_source, CopilotDataSource::OAuth);

        let env = CopilotUsageFetcher::env_only();
        assert_eq!(env.data_source, CopilotDataSource::Env);
    }

    #[test]
    fn test_data_source_default() {
        let source = CopilotDataSource::default();
        assert_eq!(source, CopilotDataSource::Auto);
    }

    #[test]
    fn test_is_available() {
        // Just test the function runs
        let _ = CopilotUsageFetcher::is_available();
    }

    #[test]
    fn test_is_gh_cli_available() {
        // Just test the function runs
        let _ = CopilotUsageFetcher::is_gh_cli_available();
    }

    #[test]
    fn test_detect_gh_version() {
        // Just test the function runs
        let _ = CopilotUsageFetcher::detect_gh_version();
    }

    #[test]
    fn test_with_source() {
        let fetcher = CopilotUsageFetcher::with_source(CopilotDataSource::Env);
        assert_eq!(fetcher.data_source, CopilotDataSource::Env);
    }
}
