//! Main Antigravity usage fetcher.

use exactobar_core::UsageSnapshot;
use tracing::{info, instrument};

use super::error::AntigravityError;
use super::probe::AntigravityProbe;

// ============================================================================
// Fetcher
// ============================================================================

/// Main Antigravity usage fetcher.
#[derive(Debug, Default)]
pub struct AntigravityUsageFetcher {
    probe: AntigravityProbe,
}

impl AntigravityUsageFetcher {
    /// Create a new fetcher.
    pub fn new() -> Self {
        Self {
            probe: AntigravityProbe::new(),
        }
    }

    /// Check if Antigravity is running.
    pub async fn is_running(&self) -> bool {
        self.probe.is_running().await
    }

    /// Fetch usage data.
    #[instrument(skip(self))]
    pub async fn fetch_usage(&self) -> Result<UsageSnapshot, AntigravityError> {
        let snapshot = self.probe.fetch_usage().await?;
        info!("Fetched Antigravity usage via local probe");
        Ok(snapshot)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_fetcher_creation() {
        let _ = AntigravityUsageFetcher::new();
    }
}
