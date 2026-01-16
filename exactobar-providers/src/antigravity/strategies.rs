//! Antigravity fetch strategies.

use async_trait::async_trait;
use exactobar_core::UsageSnapshot;
use exactobar_fetch::{FetchContext, FetchError, FetchKind, FetchResult, FetchStrategy};
use tracing::{debug, instrument};

use super::probe::AntigravityProbe;

/// Local probe strategy for Antigravity.
/// 
/// Detects the running Antigravity process, extracts CSRF token,
/// and queries the gRPC-style API for usage quotas.
pub struct AntigravityLocalStrategy {
    probe: AntigravityProbe,
}

impl AntigravityLocalStrategy {
    /// Creates a new Antigravity local strategy.
    pub fn new() -> Self {
        Self {
            probe: AntigravityProbe::new(),
        }
    }
}

impl Default for AntigravityLocalStrategy {
    fn default() -> Self {
        Self::new()
    }
}

#[async_trait]
impl FetchStrategy for AntigravityLocalStrategy {
    fn id(&self) -> &str {
        "antigravity.local"
    }

    fn kind(&self) -> FetchKind {
        FetchKind::LocalProbe
    }

    #[instrument(skip(self, _ctx))]
    async fn is_available(&self, _ctx: &FetchContext) -> bool {
        self.probe.is_running().await
    }

    #[instrument(skip(self, _ctx))]
    async fn fetch(&self, _ctx: &FetchContext) -> Result<FetchResult, FetchError> {
        debug!("Fetching Antigravity status via local probe");

        let snapshot: UsageSnapshot = self
            .probe
            .fetch_usage()
            .await
            .map_err(|e| FetchError::InvalidResponse(e.to_string()))?;

        Ok(FetchResult::new(snapshot, self.id(), self.kind()))
    }

    fn priority(&self) -> u32 {
        100
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_local_strategy() {
        let s = AntigravityLocalStrategy::new();
        assert_eq!(s.id(), "antigravity.local");
        assert_eq!(s.priority(), 100);
    }
}
