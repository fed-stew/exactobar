//! Main VertexAI usage fetcher.
//!
//! VertexAI uses OAuth credentials from Application Default Credentials (ADC)
//! and can track token costs from local Claude logs.

use exactobar_core::{FetchSource, LoginMethod, ProviderIdentity, ProviderKind, UsageSnapshot};
use tracing::{debug, info, instrument};

use super::credentials::{VertexAICredentials, VertexAITokenRefresher};
use super::error::VertexAIError;
use super::logs::ClaudeLogReader;

// ============================================================================
// Data Source
// ============================================================================

/// Which data source to use for fetching.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum VertexAIDataSource {
    /// Try all strategies.
    #[default]
    Auto,
    /// Use gcloud OAuth only.
    OAuth,
    /// Use local logs only.
    Logs,
}

// ============================================================================
// Fetcher
// ============================================================================

/// Main VertexAI usage fetcher.
#[derive(Debug, Clone, Default)]
pub struct VertexAIUsageFetcher {
    data_source: VertexAIDataSource,
}

impl VertexAIUsageFetcher {
    /// Create a new fetcher.
    pub fn new() -> Self {
        Self::default()
    }

    /// Create with specific source.
    pub fn with_source(source: VertexAIDataSource) -> Self {
        Self {
            data_source: source,
        }
    }

    /// Check if OAuth credentials are available.
    pub fn has_oauth_credentials() -> bool {
        VertexAICredentials::load().is_ok_and(|c| c.has_oauth())
    }

    /// Check if Claude logs exist.
    pub fn has_logs() -> bool {
        ClaudeLogReader::has_logs()
    }

    /// Fetch usage data.
    #[instrument(skip(self))]
    pub async fn fetch_usage(&self) -> Result<UsageSnapshot, VertexAIError> {
        match self.data_source {
            VertexAIDataSource::Auto => self.fetch_auto().await,
            VertexAIDataSource::OAuth => self.fetch_via_oauth().await,
            VertexAIDataSource::Logs => self.fetch_via_logs(),
        }
    }

    async fn fetch_auto(&self) -> Result<UsageSnapshot, VertexAIError> {
        // Try OAuth first
        if let Ok(mut snapshot) = self.fetch_via_oauth().await {
            info!(source = "oauth", "Fetched via gcloud OAuth");

            // Enrich with log data if available
            if let Ok(log_snapshot) = self.fetch_via_logs() {
                // Merge log data into OAuth snapshot
                if snapshot.primary.is_none() {
                    snapshot.primary = log_snapshot.primary;
                }
            }

            return Ok(snapshot);
        }

        // Try logs
        if let Ok(snapshot) = self.fetch_via_logs() {
            info!(source = "logs", "Fetched via local logs");
            return Ok(snapshot);
        }

        Err(VertexAIError::AllStrategiesFailed)
    }

    async fn fetch_via_oauth(&self) -> Result<UsageSnapshot, VertexAIError> {
        debug!("Fetching via OAuth (ADC)");

        // Load credentials from ADC
        let creds = VertexAICredentials::load()?;

        if !creds.has_oauth() {
            return Err(VertexAIError::NotLoggedIn);
        }

        // Refresh the token to verify credentials are valid
        let refresher = VertexAITokenRefresher::new();
        let _token = refresher.refresh(&creds).await?;

        let mut snapshot = UsageSnapshot::new();
        snapshot.fetch_source = FetchSource::OAuth;

        // Build identity from credentials
        let mut identity = ProviderIdentity::new(ProviderKind::VertexAI);
        identity.account_organization = creds.quota_project_id.clone();
        identity.login_method = Some(LoginMethod::OAuth);
        identity.plan_name = Some("Vertex AI".to_string());
        snapshot.identity = Some(identity);

        Ok(snapshot)
    }

    fn fetch_via_logs(&self) -> Result<UsageSnapshot, VertexAIError> {
        debug!("Fetching via local logs");

        let reader = ClaudeLogReader::new();
        let usage = reader.read_today_usage()?;

        let mut snapshot = UsageSnapshot::new();
        snapshot.fetch_source = FetchSource::LocalProbe;

        // We don't have a quota, so just report the usage counts
        // The UI can show "X tokens used today" instead of percentage
        let mut identity = ProviderIdentity::new(ProviderKind::VertexAI);
        identity.login_method = Some(LoginMethod::CLI);
        identity.plan_name = Some(format!(
            "{} tokens / ${:.2}",
            usage.total_tokens, usage.total_cost_usd
        ));
        snapshot.identity = Some(identity);

        Ok(snapshot)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_fetcher_creation() {
        let fetcher = VertexAIUsageFetcher::new();
        assert_eq!(fetcher.data_source, VertexAIDataSource::Auto);
    }

    #[test]
    fn test_has_oauth_credentials() {
        let _ = VertexAIUsageFetcher::has_oauth_credentials();
    }

    #[test]
    fn test_has_logs() {
        let _ = VertexAIUsageFetcher::has_logs();
    }
}
