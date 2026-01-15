//! Main Factory usage fetcher.

use exactobar_core::UsageSnapshot;
use exactobar_fetch::host::browser::{Browser, BrowserCookieImporter};
use tracing::{debug, info, instrument};

use super::error::FactoryError;
use super::web::FactoryWebClient;

// ============================================================================
// Data Source
// ============================================================================

/// Which data source to use for fetching.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum FactoryDataSource {
    /// Try all strategies.
    #[default]
    Auto,
    /// Use web cookies only.
    Web,
    /// Use WorkOS token only.
    WorkOS,
}

// ============================================================================
// Fetcher
// ============================================================================

/// Main Factory usage fetcher.
#[derive(Debug, Clone, Default)]
pub struct FactoryUsageFetcher {
    data_source: FactoryDataSource,
}

impl FactoryUsageFetcher {
    /// Create a new fetcher.
    pub fn new() -> Self {
        Self::default()
    }

    /// Create with specific source.
    pub fn with_source(source: FactoryDataSource) -> Self {
        Self {
            data_source: source,
        }
    }

    /// Check if WorkOS token exists.
    pub fn has_workos_token() -> bool {
        FactoryWebClient::load_workos_token().is_some()
    }

    /// Fetch usage data.
    #[instrument(skip(self))]
    pub async fn fetch_usage(&self) -> Result<UsageSnapshot, FactoryError> {
        match self.data_source {
            FactoryDataSource::Auto => self.fetch_auto().await,
            FactoryDataSource::Web => self.fetch_via_web().await,
            FactoryDataSource::WorkOS => self.fetch_via_workos().await,
        }
    }

    async fn fetch_auto(&self) -> Result<UsageSnapshot, FactoryError> {
        // Try WorkOS first
        if let Ok(snapshot) = self.fetch_via_workos().await {
            info!(source = "workos", "Fetched via WorkOS token");
            return Ok(snapshot);
        }

        // Try web cookies
        if let Ok(snapshot) = self.fetch_via_web().await {
            info!(source = "web", "Fetched via web cookies");
            return Ok(snapshot);
        }

        Err(FactoryError::AllStrategiesFailed)
    }

    async fn fetch_via_web(&self) -> Result<UsageSnapshot, FactoryError> {
        debug!("Fetching via web cookies");

        let importer = BrowserCookieImporter::new();
        let (_, cookies) = importer
            .import_cookies_auto("factory.ai", Browser::default_priority())
            .await
            .map_err(|e| FactoryError::BrowserError(e.to_string()))?;

        if cookies.is_empty() {
            return Err(FactoryError::NoSessionCookie);
        }

        let cookie_header = BrowserCookieImporter::cookies_to_header(&cookies);

        if !FactoryWebClient::has_session_cookie(&cookie_header) {
            return Err(FactoryError::NoSessionCookie);
        }

        let client = FactoryWebClient::new();
        let usage = client.fetch_usage(&cookie_header, false).await?;
        Ok(usage.to_snapshot())
    }

    async fn fetch_via_workos(&self) -> Result<UsageSnapshot, FactoryError> {
        debug!("Fetching via WorkOS token");

        let token = FactoryWebClient::load_workos_token()
            .ok_or(FactoryError::NoWorkOSToken)?;

        let client = FactoryWebClient::new();
        let usage = client.fetch_usage(&token, true).await?;
        Ok(usage.to_snapshot())
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_fetcher_creation() {
        let fetcher = FactoryUsageFetcher::new();
        assert_eq!(fetcher.data_source, FactoryDataSource::Auto);
    }

    #[test]
    fn test_has_workos_token() {
        let _ = FactoryUsageFetcher::has_workos_token();
    }
}
