//! Main MiniMax usage fetcher.

use exactobar_core::{FetchSource, UsageSnapshot};
use exactobar_fetch::host::browser::{Browser, BrowserCookieImporter};
use tracing::{debug, info, instrument};

use super::error::MiniMaxError;
use super::web::{MiniMaxTokenStore, MiniMaxWebClient};

// ============================================================================
// Data Source
// ============================================================================

/// Which data source to use for fetching.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum MiniMaxDataSource {
    /// Try all strategies.
    #[default]
    Auto,
    /// Use web cookies only.
    Web,
    /// Use local token only.
    Local,
}

// ============================================================================
// Fetcher
// ============================================================================

/// Main MiniMax usage fetcher.
#[derive(Debug, Clone, Default)]
pub struct MiniMaxUsageFetcher {
    data_source: MiniMaxDataSource,
}

impl MiniMaxUsageFetcher {
    /// Create a new fetcher.
    pub fn new() -> Self {
        Self::default()
    }

    /// Create with specific source.
    pub fn with_source(source: MiniMaxDataSource) -> Self {
        Self {
            data_source: source,
        }
    }

    /// Check if local token exists.
    pub fn has_local_token() -> bool {
        MiniMaxTokenStore::is_available()
    }

    /// Fetch usage data.
    #[instrument(skip(self))]
    pub async fn fetch_usage(&self) -> Result<UsageSnapshot, MiniMaxError> {
        match self.data_source {
            MiniMaxDataSource::Auto => self.fetch_auto().await,
            MiniMaxDataSource::Web => self.fetch_via_web().await,
            MiniMaxDataSource::Local => self.fetch_via_local().await,
        }
    }

    async fn fetch_auto(&self) -> Result<UsageSnapshot, MiniMaxError> {
        // Try local token first
        if let Ok(snapshot) = self.fetch_via_local().await {
            info!(source = "local", "Fetched via local token");
            return Ok(snapshot);
        }

        // Try web cookies
        if let Ok(snapshot) = self.fetch_via_web().await {
            info!(source = "web", "Fetched via web cookies");
            return Ok(snapshot);
        }

        Err(MiniMaxError::AllStrategiesFailed)
    }

    async fn fetch_via_web(&self) -> Result<UsageSnapshot, MiniMaxError> {
        debug!("Fetching via web cookies");

        let importer = BrowserCookieImporter::new();
        let (_, cookies) = importer
            .import_cookies_auto("minimax.chat", Browser::default_priority())
            .await
            .map_err(|e| MiniMaxError::BrowserError(e.to_string()))?;

        if cookies.is_empty() {
            return Err(MiniMaxError::NoSessionCookie);
        }

        let cookie_header = BrowserCookieImporter::cookies_to_header(&cookies);

        if !MiniMaxWebClient::has_session_cookie(&cookie_header) {
            return Err(MiniMaxError::NoSessionCookie);
        }

        let client = MiniMaxWebClient::new();
        let usage = client.fetch_usage_with_cookies(&cookie_header).await?;
        Ok(usage.to_snapshot(FetchSource::Web))
    }

    async fn fetch_via_local(&self) -> Result<UsageSnapshot, MiniMaxError> {
        debug!("Fetching via local token");

        let token = MiniMaxTokenStore::load().ok_or(MiniMaxError::NoToken)?;

        let client = MiniMaxWebClient::new();
        let usage = client.fetch_usage_with_token(&token).await?;
        Ok(usage.to_snapshot(FetchSource::LocalProbe))
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_fetcher_creation() {
        let fetcher = MiniMaxUsageFetcher::new();
        assert_eq!(fetcher.data_source, MiniMaxDataSource::Auto);
    }

    #[test]
    fn test_has_local_token() {
        let _ = MiniMaxUsageFetcher::has_local_token();
    }
}
