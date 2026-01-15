//! VertexAI fetch strategies.

use async_trait::async_trait;
use exactobar_core::{FetchSource, UsageSnapshot};
use exactobar_fetch::{FetchContext, FetchError, FetchKind, FetchResult, FetchStrategy};
use std::path::PathBuf;
use tracing::{debug, info, instrument};

use super::credentials::{VertexAICredentials, VertexAITokenRefresher};
use super::error::VertexAIError;

#[allow(dead_code)]
const VERTEXAI_API: &str = "https://us-central1-aiplatform.googleapis.com/v1";

// ============================================================================
// OAuth Strategy
// ============================================================================

/// OAuth strategy for VertexAI using Application Default Credentials.
///
/// This strategy reads OAuth credentials from the ADC file and refreshes
/// access tokens via Google's OAuth2 endpoint - no gcloud CLI required!
pub struct VertexAIOAuthStrategy {
    refresher: VertexAITokenRefresher,
}

impl VertexAIOAuthStrategy {
    pub fn new() -> Self {
        Self {
            refresher: VertexAITokenRefresher::new(),
        }
    }

    /// Get an access token using OAuth refresh flow.
    async fn get_access_token(&self) -> Result<String, VertexAIError> {
        let creds = VertexAICredentials::load()?;

        if !creds.has_oauth() {
            return Err(VertexAIError::NotLoggedIn);
        }

        // Always refresh since we don't store expiry
        self.refresher.refresh(&creds).await
    }

    /// Get the project ID from credentials.
    fn get_project_id(&self) -> Result<String, VertexAIError> {
        let creds = VertexAICredentials::load()?;
        creds
            .quota_project_id
            .clone()
            .ok_or(VertexAIError::NoProject)
    }
}

impl Default for VertexAIOAuthStrategy {
    fn default() -> Self {
        Self::new()
    }
}

#[async_trait]
impl FetchStrategy for VertexAIOAuthStrategy {
    fn id(&self) -> &str {
        "vertexai.oauth"
    }

    fn kind(&self) -> FetchKind {
        FetchKind::OAuth
    }

    #[instrument(skip(self, _ctx))]
    async fn is_available(&self, _ctx: &FetchContext) -> bool {
        // Check if we have valid ADC credentials
        VertexAICredentials::load().is_ok_and(|c| c.has_oauth())
    }

    #[instrument(skip(self, _ctx))]
    async fn fetch(&self, _ctx: &FetchContext) -> Result<FetchResult, FetchError> {
        debug!("Fetching VertexAI usage via OAuth (ADC)");

        let token = self
            .get_access_token()
            .await
            .map_err(|e| FetchError::AuthenticationFailed(e.to_string()))?;

        let project = self
            .get_project_id()
            .map_err(|e| FetchError::InvalidResponse(e.to_string()))?;

        info!(
            project = %project,
            token_len = token.len(),
            "Successfully obtained OAuth token"
        );

        // Vertex AI usage is typically tracked via Cloud Monitoring
        // The token can be used to query the monitoring API
        let mut snapshot = UsageSnapshot::new();
        snapshot.fetch_source = FetchSource::OAuth;

        Ok(FetchResult::new(snapshot, self.id(), self.kind()))
    }

    fn priority(&self) -> u32 {
        100
    }
}

// ============================================================================
// Local Strategy (Token Cost Tracking)
// ============================================================================

pub struct VertexAILocalStrategy;

impl VertexAILocalStrategy {
    pub fn new() -> Self {
        Self
    }

    fn log_directory() -> Option<PathBuf> {
        dirs::home_dir().map(|h| h.join(".config").join("gcloud").join("logs"))
    }
}

impl Default for VertexAILocalStrategy {
    fn default() -> Self {
        Self::new()
    }
}

#[async_trait]
impl FetchStrategy for VertexAILocalStrategy {
    fn id(&self) -> &str {
        "vertexai.local"
    }

    fn kind(&self) -> FetchKind {
        FetchKind::LocalProbe
    }

    #[instrument(skip(self, _ctx))]
    async fn is_available(&self, _ctx: &FetchContext) -> bool {
        Self::log_directory().is_some_and(|p| p.exists())
    }

    #[instrument(skip(self, _ctx))]
    async fn fetch(&self, _ctx: &FetchContext) -> Result<FetchResult, FetchError> {
        debug!("Fetching VertexAI usage from local logs");

        // Token cost tracking from local logs would be implemented here
        let mut snapshot = UsageSnapshot::new();
        snapshot.fetch_source = FetchSource::LocalProbe;

        Ok(FetchResult::new(snapshot, self.id(), self.kind()))
    }

    fn priority(&self) -> u32 {
        60
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_oauth_strategy() {
        let s = VertexAIOAuthStrategy::new();
        assert_eq!(s.id(), "vertexai.oauth");
        assert_eq!(s.priority(), 100);
    }

    #[test]
    fn test_local_strategy() {
        let s = VertexAILocalStrategy::new();
        assert_eq!(s.id(), "vertexai.local");
        assert_eq!(s.priority(), 60);
    }
}
