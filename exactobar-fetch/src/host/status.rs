//! Status page polling for provider health.
//!
//! This module provides utilities for fetching status information from
//! provider status pages, primarily using the statuspage.io format.

use chrono::{DateTime, Utc};
use exactobar_core::{ProviderStatus, StatusIndicator};
use serde::Deserialize;
use tracing::{debug, instrument, warn};

use super::http::HttpClient;
use crate::error::StatusError;

// ============================================================================
// Statuspage.io Response Types
// ============================================================================

/// Response from statuspage.io /api/v2/status.json endpoint.
#[derive(Debug, Deserialize)]
struct StatuspageStatus {
    status: StatuspageIndicator,
    page: StatuspagePage,
}

#[derive(Debug, Deserialize)]
struct StatuspageIndicator {
    indicator: String,
    description: String,
}

#[derive(Debug, Deserialize)]
struct StatuspagePage {
    #[allow(dead_code)]
    id: String,
    #[allow(dead_code)]
    name: String,
    url: String,
    updated_at: Option<String>,
}

/// Response from Google Workspace Status Dashboard.
#[allow(dead_code)]
#[derive(Debug, Deserialize)]
struct GoogleWorkspaceStatus {
    products: Vec<GoogleProduct>,
}

#[allow(dead_code)]
#[derive(Debug, Deserialize)]
struct GoogleProduct {
    id: String,
    name: String,
    status: String,
}

// ============================================================================
// Status Poller
// ============================================================================

/// API for polling provider status pages.
#[derive(Debug, Clone)]
pub struct StatusPoller {
    client: HttpClient,
}

impl StatusPoller {
    /// Creates a new status poller.
    pub fn new() -> Self {
        Self {
            client: HttpClient::new(),
        }
    }

    /// Creates a status poller with a custom HTTP client.
    pub fn with_client(client: HttpClient) -> Self {
        Self { client }
    }

    /// Fetch status from a statuspage.io-compatible endpoint.
    ///
    /// URL should be like: `https://status.openai.com/api/v2/status.json`
    #[instrument(skip(self), fields(url = %status_url))]
    pub async fn fetch_status(&self, status_url: &str) -> Result<ProviderStatus, StatusError> {
        debug!("Fetching status from statuspage.io endpoint");

        let response = self.client.get(status_url).await.map_err(|e| {
            warn!(error = %e, "Failed to fetch status");
            StatusError::Unavailable(e.to_string())
        })?;

        if !response.status().is_success() {
            let status = response.status();
            return Err(StatusError::Unavailable(format!("HTTP {}", status)));
        }

        let data: StatuspageStatus = response.json().await?;

        let indicator = parse_statuspage_indicator(&data.status.indicator);
        let updated_at = data
            .page
            .updated_at
            .as_ref()
            .and_then(|s| DateTime::parse_from_rfc3339(s).ok())
            .map(|dt| dt.with_timezone(&Utc))
            .unwrap_or_else(Utc::now);

        debug!(
            indicator = ?indicator,
            description = %data.status.description,
            "Status fetched successfully"
        );

        Ok(ProviderStatus {
            indicator,
            description: data.status.description,
            updated_at,
            url: Some(data.page.url),
        })
    }

    /// Fetch status for a Google Workspace product.
    ///
    /// Product IDs can be found at: https://www.google.com/appsstatus/dashboard/
    #[instrument(skip(self), fields(product_id = %product_id))]
    pub async fn fetch_google_workspace_status(
        &self,
        product_id: &str,
    ) -> Result<ProviderStatus, StatusError> {
        debug!("Fetching Google Workspace status");

        // Google Workspace Status Dashboard JSON endpoint
        let url = "https://www.google.com/appsstatus/dashboard/incidents.json";

        let response = self.client.get(url).await.map_err(|e| {
            warn!(error = %e, "Failed to fetch Google status");
            StatusError::Unavailable(e.to_string())
        })?;

        if !response.status().is_success() {
            // Fall back to operational if we can't fetch
            debug!("Could not fetch Google status, assuming operational");
            return Ok(ProviderStatus::operational());
        }

        // For now, we'll just return operational
        // Full implementation would parse the incidents JSON
        Ok(ProviderStatus {
            indicator: StatusIndicator::None,
            description: "Operational".to_string(),
            updated_at: Utc::now(),
            url: Some("https://www.google.com/appsstatus/dashboard/".to_string()),
        })
    }

    /// Fetch status from multiple URLs and return the worst status.
    pub async fn fetch_worst_status(
        &self,
        urls: &[&str],
    ) -> Result<ProviderStatus, StatusError> {
        let mut worst_indicator = StatusIndicator::None;
        let mut worst_description = "All systems operational".to_string();
        let mut first_url = None;

        for url in urls {
            match self.fetch_status(url).await {
                Ok(status) => {
                    if first_url.is_none() {
                        first_url = status.url.clone();
                    }
                    if status.indicator.severity() > worst_indicator.severity() {
                        worst_indicator = status.indicator;
                        worst_description = status.description;
                    }
                }
                Err(e) => {
                    warn!(url = %url, error = %e, "Failed to fetch status");
                    // Continue trying other URLs
                }
            }
        }

        Ok(ProviderStatus {
            indicator: worst_indicator,
            description: worst_description,
            updated_at: Utc::now(),
            url: first_url,
        })
    }
}

impl Default for StatusPoller {
    fn default() -> Self {
        Self::new()
    }
}

// ============================================================================
// Helper Functions
// ============================================================================

/// Parse a statuspage.io indicator string into our enum.
fn parse_statuspage_indicator(indicator: &str) -> StatusIndicator {
    match indicator.to_lowercase().as_str() {
        "none" => StatusIndicator::None,
        "minor" => StatusIndicator::Minor,
        "major" => StatusIndicator::Major,
        "critical" => StatusIndicator::Critical,
        "maintenance" => StatusIndicator::Maintenance,
        _ => StatusIndicator::Unknown,
    }
}

// ============================================================================
// Known Status Page URLs
// ============================================================================

/// Known status page URLs for providers.
pub mod urls {
    // === API Endpoints (statuspage.io format) ===
    /// OpenAI status page API endpoint.
    pub const OPENAI: &str = "https://status.openai.com/api/v2/status.json";
    /// Anthropic status page API endpoint.
    pub const ANTHROPIC: &str = "https://status.anthropic.com/api/v2/status.json";
    /// GitHub status page API endpoint.
    pub const GITHUB: &str = "https://www.githubstatus.com/api/v2/status.json";

    // === User-facing status pages ===
    /// OpenAI status page URL.
    pub const OPENAI_PAGE: &str = "https://status.openai.com";
    /// Anthropic status page URL.
    pub const ANTHROPIC_PAGE: &str = "https://status.anthropic.com";
    /// GitHub status page URL.
    pub const GITHUB_PAGE: &str = "https://www.githubstatus.com";
    /// Google Cloud status page URL.
    pub const GOOGLE_CLOUD_PAGE: &str = "https://status.cloud.google.com";
    /// Cursor status page URL.
    pub const CURSOR_PAGE: &str = "https://status.cursor.com";

    /// Returns the API URL for a given provider name (lowercase).
    pub fn api_url_for_provider(provider: &str) -> Option<&'static str> {
        match provider {
            "codex" | "openai" => Some(OPENAI),
            "claude" | "anthropic" => Some(ANTHROPIC),
            "copilot" | "github" => Some(GITHUB),
            _ => None,
        }
    }

    /// Returns the user-facing status page URL for a given provider name.
    pub fn page_url_for_provider(provider: &str) -> Option<&'static str> {
        match provider {
            "codex" | "openai" => Some(OPENAI_PAGE),
            "claude" | "anthropic" => Some(ANTHROPIC_PAGE),
            "copilot" | "github" => Some(GITHUB_PAGE),
            "gemini" | "vertexai" => Some(GOOGLE_CLOUD_PAGE),
            "cursor" => Some(CURSOR_PAGE),
            _ => None,
        }
    }
}

// ============================================================================
// Tests
// ============================================================================

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_parse_indicator() {
        assert_eq!(parse_statuspage_indicator("none"), StatusIndicator::None);
        assert_eq!(parse_statuspage_indicator("None"), StatusIndicator::None);
        assert_eq!(parse_statuspage_indicator("minor"), StatusIndicator::Minor);
        assert_eq!(parse_statuspage_indicator("major"), StatusIndicator::Major);
        assert_eq!(
            parse_statuspage_indicator("critical"),
            StatusIndicator::Critical
        );
        assert_eq!(
            parse_statuspage_indicator("maintenance"),
            StatusIndicator::Maintenance
        );
        assert_eq!(
            parse_statuspage_indicator("Maintenance"),
            StatusIndicator::Maintenance
        );
        assert_eq!(
            parse_statuspage_indicator("unknown_value"),
            StatusIndicator::Unknown
        );
    }

    #[test]
    fn test_api_url_for_provider() {
        assert_eq!(urls::api_url_for_provider("codex"), Some(urls::OPENAI));
        assert_eq!(urls::api_url_for_provider("openai"), Some(urls::OPENAI));
        assert_eq!(urls::api_url_for_provider("claude"), Some(urls::ANTHROPIC));
        assert_eq!(urls::api_url_for_provider("copilot"), Some(urls::GITHUB));
        assert_eq!(urls::api_url_for_provider("cursor"), None);
    }

    #[test]
    fn test_page_url_for_provider() {
        assert_eq!(urls::page_url_for_provider("codex"), Some(urls::OPENAI_PAGE));
        assert_eq!(urls::page_url_for_provider("cursor"), Some(urls::CURSOR_PAGE));
        assert_eq!(urls::page_url_for_provider("gemini"), Some(urls::GOOGLE_CLOUD_PAGE));
    }
}
