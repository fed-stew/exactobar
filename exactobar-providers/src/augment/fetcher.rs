//! Main Augment usage fetcher.

use exactobar_core::UsageSnapshot;
use exactobar_fetch::host::browser::{Browser, BrowserCookieImporter};
use tracing::{debug, info, instrument};

use super::error::AugmentError;
use super::web::AugmentWebClient;

// ============================================================================
// Fetcher
// ============================================================================

/// Main Augment usage fetcher.
#[derive(Debug, Clone, Default)]
pub struct AugmentUsageFetcher;

impl AugmentUsageFetcher {
    /// Create a new fetcher.
    pub fn new() -> Self {
        Self
    }

    /// Fetch usage data.
    #[instrument(skip(self))]
    pub async fn fetch_usage(&self) -> Result<UsageSnapshot, AugmentError> {
        debug!("Fetching Augment usage");

        let importer = BrowserCookieImporter::new();
        let (_, cookies) = importer
            .import_cookies_auto("augmentcode.com", Browser::default_priority())
            .await
            .map_err(|e| AugmentError::BrowserError(e.to_string()))?;

        if cookies.is_empty() {
            return Err(AugmentError::NoSessionCookie);
        }

        let cookie_header = BrowserCookieImporter::cookies_to_header(&cookies);

        if !AugmentWebClient::has_session_cookie(&cookie_header) {
            return Err(AugmentError::NoSessionCookie);
        }

        let client = AugmentWebClient::new();
        let usage = client.fetch_usage(&cookie_header).await?;

        info!("Fetched Augment usage via web");
        Ok(usage.to_snapshot())
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_fetcher_creation() {
        let _ = AugmentUsageFetcher::new();
    }
}
