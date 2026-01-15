//! Main z.ai usage fetcher.

use exactobar_core::UsageSnapshot;
use tracing::{debug, info, instrument};

use super::api::ZaiApiClient;
use super::token_store::ZaiTokenStore;
use super::error::ZaiError;

// ============================================================================
// Fetcher
// ============================================================================

/// Main z.ai usage fetcher.
#[derive(Debug, Clone, Default)]
pub struct ZaiUsageFetcher;

impl ZaiUsageFetcher {
    /// Create a new fetcher.
    pub fn new() -> Self {
        Self
    }

    /// Check if token is available.
    pub fn is_available() -> bool {
        ZaiTokenStore::is_available()
    }

    /// Fetch usage data.
    #[instrument(skip(self))]
    pub async fn fetch_usage(&self) -> Result<UsageSnapshot, ZaiError> {
        debug!("Fetching z.ai usage");

        let token = ZaiTokenStore::load().ok_or(ZaiError::NoToken)?;

        let client = ZaiApiClient::new();
        let usage = client.fetch_usage(&token).await?;

        info!("Fetched z.ai usage via API");
        Ok(usage.to_snapshot())
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_fetcher_creation() {
        let _ = ZaiUsageFetcher::new();
    }

    #[test]
    fn test_is_available() {
        let _ = ZaiUsageFetcher::is_available();
    }
}
