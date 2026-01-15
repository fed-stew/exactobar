//! Main Kiro usage fetcher.

use exactobar_core::UsageSnapshot;
use tracing::{info, instrument};

use super::cli::KiroCliClient;
use super::error::KiroError;

// ============================================================================
// Fetcher
// ============================================================================

/// Main Kiro usage fetcher.
#[derive(Debug, Clone, Default)]
pub struct KiroUsageFetcher;

impl KiroUsageFetcher {
    /// Create a new fetcher.
    pub fn new() -> Self {
        Self
    }

    /// Check if Kiro CLI is available.
    pub fn is_available() -> bool {
        KiroCliClient::is_available()
    }

    /// Fetch usage data.
    #[instrument(skip(self))]
    pub async fn fetch_usage(&self) -> Result<UsageSnapshot, KiroError> {
        let client = KiroCliClient::new();
        let usage = client.fetch_usage().await?;

        info!("Fetched Kiro usage via CLI");
        Ok(usage.to_snapshot())
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_fetcher_creation() {
        let _ = KiroUsageFetcher::new();
    }

    #[test]
    fn test_is_available() {
        let _ = KiroUsageFetcher::is_available();
    }
}
