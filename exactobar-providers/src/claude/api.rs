//! Claude API client for OAuth-based usage fetching.
//!
//! This module provides a client for the Anthropic API to fetch usage data.
//!
//! # API Endpoint
//!
//! ```text
//! GET https://api.anthropic.com/v1/usage
//! Authorization: Bearer <access_token>
//! ```
//!
//! # Response Format
//!
//! ```json
//! {
//!   "fiveHour": {"utilization": 25.0, "resetsAt": "2025-01-01T12:00:00Z"},
//!   "sevenDay": {"utilization": 45.0, "resetsAt": "2025-01-05T00:00:00Z"},
//!   "sevenDaySonnet": {"utilization": 30.0, "resetsAt": "2025-01-05T00:00:00Z"},
//!   "extraUsage": {"isEnabled": true, "usedCredits": 500, "monthlyLimit": 10000}
//! }
//! ```

use chrono::{DateTime, Utc};
use serde::Deserialize;
use tracing::{debug, instrument, warn};

use super::error::ClaudeError;
use super::oauth::ClaudeOAuthCredentials;

// ============================================================================
// Constants
// ============================================================================

/// Base URL for Claude API.
pub const API_BASE_URL: &str = "https://api.anthropic.com";

/// Usage endpoint.
pub const USAGE_ENDPOINT: &str = "/v1/usage";

/// Alternative usage endpoint (claude.ai).
#[allow(dead_code)]
pub const CLAUDE_AI_USAGE_ENDPOINT: &str = "https://claude.ai/api/organizations";

// ============================================================================
// API Response Structures
// ============================================================================

/// Response from the usage API.
#[derive(Debug, Clone, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct UsageApiResponse {
    /// 5-hour usage window.
    pub five_hour: Option<UsageWindow>,
    /// 7-day usage window (all models).
    pub seven_day: Option<UsageWindow>,
    /// 7-day Sonnet usage window.
    pub seven_day_sonnet: Option<UsageWindow>,
    /// Extra usage/credits info.
    pub extra_usage: Option<ExtraUsage>,
    /// Account info.
    pub account: Option<AccountInfo>,
}

/// Individual usage window from API.
#[derive(Debug, Clone, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct UsageWindow {
    /// Utilization percentage (0-100).
    pub utilization: f64,
    /// When this window resets (ISO 8601).
    pub resets_at: Option<String>,
    /// Remaining percentage (alternative field).
    pub remaining: Option<f64>,
    /// Used percentage (alternative field).
    pub used_percent: Option<f64>,
}

impl UsageWindow {
    /// Get the used percentage, handling various field names.
    pub fn get_used_percent(&self) -> f64 {
        // utilization is the "used" percentage
        if self.utilization > 0.0 {
            return self.utilization;
        }
        if let Some(used) = self.used_percent {
            return used;
        }
        if let Some(remaining) = self.remaining {
            return 100.0 - remaining;
        }
        0.0
    }

    /// Parse the reset timestamp.
    pub fn get_resets_at(&self) -> Option<DateTime<Utc>> {
        self.resets_at.as_ref().and_then(|s| {
            DateTime::parse_from_rfc3339(s)
                .ok()
                .map(|dt| dt.with_timezone(&Utc))
        })
    }
}

/// Extra usage/credits information.
#[derive(Debug, Clone, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ExtraUsage {
    /// Whether extra usage is enabled.
    pub is_enabled: Option<bool>,
    /// Credits used this month.
    pub used_credits: Option<f64>,
    /// Monthly credit limit.
    pub monthly_limit: Option<f64>,
    /// Currency (e.g., "USD").
    pub currency: Option<String>,
}

/// Account information from API.
#[derive(Debug, Clone, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct AccountInfo {
    /// Account email.
    pub email: Option<String>,
    /// Plan name.
    pub plan: Option<String>,
    /// Organization name.
    pub organization: Option<String>,
}

// ============================================================================
// API Client
// ============================================================================

/// Claude API client for fetching usage data.
#[derive(Debug, Clone)]
pub struct ClaudeApiClient {
    base_url: String,
}

impl Default for ClaudeApiClient {
    fn default() -> Self {
        Self::new()
    }
}

impl ClaudeApiClient {
    /// Create a new API client.
    pub fn new() -> Self {
        Self {
            base_url: API_BASE_URL.to_string(),
        }
    }

    /// Create a client with a custom base URL.
    pub fn with_base_url(base_url: impl Into<String>) -> Self {
        Self {
            base_url: base_url.into(),
        }
    }

    /// Fetch usage data using OAuth credentials.
    #[instrument(skip(self, credentials))]
    pub async fn fetch_usage(
        &self,
        credentials: &ClaudeOAuthCredentials,
    ) -> Result<UsageApiResponse, ClaudeError> {
        if credentials.is_expired() {
            return Err(ClaudeError::TokenExpired(
                credentials
                    .expires_at
                    .map(|t| t.to_rfc3339())
                    .unwrap_or_else(|| "unknown".to_string()),
            ));
        }

        let url = format!("{}{}", self.base_url, USAGE_ENDPOINT);

        debug!(url = %url, "Fetching usage from API");

        let client = reqwest::Client::new();
        let response = client
            .get(&url)
            .header("Authorization", format!("Bearer {}", credentials.access_token))
            .header("Content-Type", "application/json")
            .send()
            .await
            .map_err(|e| ClaudeError::HttpError(e.to_string()))?;

        let status = response.status();

        if status == reqwest::StatusCode::UNAUTHORIZED {
            return Err(ClaudeError::AuthenticationFailed(
                "OAuth token rejected".to_string(),
            ));
        }

        if status == reqwest::StatusCode::FORBIDDEN {
            return Err(ClaudeError::MissingScope("user:profile".to_string()));
        }

        if !status.is_success() {
            let body = response.text().await.unwrap_or_default();
            warn!(status = %status, body = %body, "API request failed");
            return Err(ClaudeError::ApiError(format!(
                "API returned status {}: {}",
                status, body
            )));
        }

        let body = response
            .text()
            .await
            .map_err(|e| ClaudeError::HttpError(e.to_string()))?;

        debug!(len = body.len(), "Received API response");

        let usage: UsageApiResponse = serde_json::from_str(&body)
            .map_err(|e| ClaudeError::ParseError(format!("Failed to parse response: {}", e)))?;

        Ok(usage)
    }

    /// Fetch usage using the access token directly.
    #[instrument(skip(self, access_token))]
    pub async fn fetch_usage_with_token(
        &self,
        access_token: &str,
    ) -> Result<UsageApiResponse, ClaudeError> {
        let url = format!("{}{}", self.base_url, USAGE_ENDPOINT);

        debug!(url = %url, "Fetching usage from API with token");

        let client = reqwest::Client::new();
        let response = client
            .get(&url)
            .header("Authorization", format!("Bearer {}", access_token))
            .header("Content-Type", "application/json")
            .send()
            .await?;

        let status = response.status();

        if status == reqwest::StatusCode::UNAUTHORIZED {
            return Err(ClaudeError::AuthenticationFailed(
                "Token rejected".to_string(),
            ));
        }

        if !status.is_success() {
            return Err(ClaudeError::ApiError(format!("Status: {}", status)));
        }

        let body = response.text().await?;
        let usage: UsageApiResponse = serde_json::from_str(&body)?;

        Ok(usage)
    }
}

// ============================================================================
// Conversion to Core Types
// ============================================================================

impl UsageApiResponse {
    /// Convert to a UsageSnapshot.
    pub fn to_snapshot(&self) -> exactobar_core::UsageSnapshot {
        use exactobar_core::{FetchSource, LoginMethod, ProviderIdentity, ProviderKind};

        let mut snapshot = exactobar_core::UsageSnapshot::new();
        snapshot.fetch_source = FetchSource::OAuth;

        // Primary = 5-hour window
        if let Some(ref window) = self.five_hour {
            snapshot.primary = Some(exactobar_core::UsageWindow {
                used_percent: window.get_used_percent(),
                window_minutes: Some(300), // 5 hours
                resets_at: window.get_resets_at(),
                reset_description: None,
            });
        }

        // Secondary = 7-day window (all models)
        if let Some(ref window) = self.seven_day {
            snapshot.secondary = Some(exactobar_core::UsageWindow {
                used_percent: window.get_used_percent(),
                window_minutes: Some(10080), // 7 days
                resets_at: window.get_resets_at(),
                reset_description: None,
            });
        }

        // Tertiary = 7-day Sonnet window
        if let Some(ref window) = self.seven_day_sonnet {
            snapshot.tertiary = Some(exactobar_core::UsageWindow {
                used_percent: window.get_used_percent(),
                window_minutes: Some(10080), // 7 days
                resets_at: window.get_resets_at(),
                reset_description: None,
            });
        }

        // Account identity
        if let Some(ref account) = self.account {
            let mut identity = ProviderIdentity::new(ProviderKind::Claude);
            identity.account_email = account.email.clone();
            identity.plan_name = account.plan.clone();
            identity.account_organization = account.organization.clone();
            identity.login_method = Some(LoginMethod::OAuth);
            snapshot.identity = Some(identity);
        }

        snapshot
    }
}

// ============================================================================
// Tests
// ============================================================================

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_parse_usage_response() {
        let json = r#"{
            "fiveHour": {
                "utilization": 25.5,
                "resetsAt": "2025-01-01T12:00:00Z"
            },
            "sevenDay": {
                "utilization": 45.0,
                "resetsAt": "2025-01-05T00:00:00Z"
            },
            "sevenDaySonnet": {
                "utilization": 30.0,
                "resetsAt": "2025-01-05T00:00:00Z"
            },
            "extraUsage": {
                "isEnabled": true,
                "usedCredits": 500,
                "monthlyLimit": 10000,
                "currency": "USD"
            },
            "account": {
                "email": "user@example.com",
                "plan": "pro",
                "organization": "Acme Inc"
            }
        }"#;

        let response: UsageApiResponse = serde_json::from_str(json).unwrap();

        let five_hour = response.five_hour.as_ref().unwrap();
        assert!((five_hour.utilization - 25.5).abs() < 0.01);
        assert!(five_hour.get_resets_at().is_some());

        let seven_day = response.seven_day.as_ref().unwrap();
        assert!((seven_day.utilization - 45.0).abs() < 0.01);

        let sonnet = response.seven_day_sonnet.as_ref().unwrap();
        assert!((sonnet.utilization - 30.0).abs() < 0.01);

        let extra = response.extra_usage.as_ref().unwrap();
        assert_eq!(extra.is_enabled, Some(true));
        assert!((extra.used_credits.unwrap() - 500.0).abs() < 0.01);

        let account = response.account.as_ref().unwrap();
        assert_eq!(account.email, Some("user@example.com".to_string()));
        assert_eq!(account.plan, Some("pro".to_string()));
    }

    #[test]
    fn test_usage_window_get_used_percent() {
        // Test utilization field
        let window = UsageWindow {
            utilization: 25.0,
            resets_at: None,
            remaining: None,
            used_percent: None,
        };
        assert!((window.get_used_percent() - 25.0).abs() < 0.01);

        // Test remaining field (75% remaining = 25% used)
        let window = UsageWindow {
            utilization: 0.0,
            resets_at: None,
            remaining: Some(75.0),
            used_percent: None,
        };
        assert!((window.get_used_percent() - 25.0).abs() < 0.01);

        // Test used_percent field
        let window = UsageWindow {
            utilization: 0.0,
            resets_at: None,
            remaining: None,
            used_percent: Some(30.0),
        };
        assert!((window.get_used_percent() - 30.0).abs() < 0.01);
    }

    #[test]
    fn test_to_snapshot() {
        let response = UsageApiResponse {
            five_hour: Some(UsageWindow {
                utilization: 25.0,
                resets_at: Some("2025-01-01T12:00:00Z".to_string()),
                remaining: None,
                used_percent: None,
            }),
            seven_day: Some(UsageWindow {
                utilization: 45.0,
                resets_at: None,
                remaining: None,
                used_percent: None,
            }),
            seven_day_sonnet: None,
            extra_usage: None,
            account: Some(AccountInfo {
                email: Some("test@example.com".to_string()),
                plan: Some("pro".to_string()),
                organization: None,
            }),
        };

        let snapshot = response.to_snapshot();

        assert!(snapshot.primary.is_some());
        assert!((snapshot.primary.as_ref().unwrap().used_percent - 25.0).abs() < 0.01);

        assert!(snapshot.secondary.is_some());
        assert!((snapshot.secondary.as_ref().unwrap().used_percent - 45.0).abs() < 0.01);

        assert!(snapshot.tertiary.is_none());

        assert!(snapshot.identity.is_some());
        assert_eq!(
            snapshot.identity.as_ref().unwrap().account_email,
            Some("test@example.com".to_string())
        );
    }

    #[test]
    fn test_client_creation() {
        let client = ClaudeApiClient::new();
        assert_eq!(client.base_url, API_BASE_URL);

        let custom = ClaudeApiClient::with_base_url("https://custom.api.com");
        assert_eq!(custom.base_url, "https://custom.api.com");
    }
}
