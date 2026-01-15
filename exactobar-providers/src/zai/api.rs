//! z.ai API client.

use exactobar_core::{
    FetchSource, LoginMethod, ProviderIdentity, ProviderKind, UsageSnapshot, UsageWindow,
};
use reqwest::header::{HeaderMap, HeaderValue, ACCEPT, AUTHORIZATION, USER_AGENT};
use serde::Deserialize;
use tracing::{debug, instrument, warn};

use super::error::ZaiError;

// ============================================================================
// Constants
// ============================================================================

/// z.ai API base URL.
const ZAI_API_BASE: &str = "https://api.z.ai";

/// Usage endpoint.
const USAGE_ENDPOINT: &str = "/v1/usage";

/// User endpoint.
#[allow(dead_code)]
const USER_ENDPOINT: &str = "/v1/user";

// ============================================================================
// API Response Types
// ============================================================================

/// Response from z.ai usage API.
#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ZaiUsageResponse {
    /// Tokens used.
    #[serde(default, alias = "tokens_used")]
    pub tokens_used: Option<u64>,

    /// Token limit.
    #[serde(default, alias = "token_limit")]
    pub token_limit: Option<u64>,

    /// Credits used.
    #[serde(default, alias = "credits_used")]
    pub credits_used: Option<f64>,

    /// Credit limit.
    #[serde(default, alias = "credit_limit")]
    pub credit_limit: Option<f64>,

    /// Reset time.
    #[serde(default, alias = "reset_at")]
    pub reset_at: Option<String>,

    /// Plan name.
    #[serde(default)]
    pub plan: Option<String>,
}

impl ZaiUsageResponse {
    /// Get usage percentage.
    pub fn get_percent(&self) -> Option<f64> {
        // Try credits first
        if let (Some(used), Some(limit)) = (self.credits_used, self.credit_limit) {
            if limit > 0.0 {
                return Some((used / limit) * 100.0);
            }
        }

        // Try tokens
        if let (Some(used), Some(limit)) = (self.tokens_used, self.token_limit) {
            if limit > 0 {
                return Some((used as f64 / limit as f64) * 100.0);
            }
        }

        None
    }

    /// Convert to UsageSnapshot.
    pub fn to_snapshot(&self) -> UsageSnapshot {
        let mut snapshot = UsageSnapshot::new();
        snapshot.fetch_source = FetchSource::OAuth;

        if let Some(percent) = self.get_percent() {
            snapshot.primary = Some(UsageWindow::new(percent));
        }

        if self.plan.is_some() {
            let mut identity = ProviderIdentity::new(ProviderKind::Zai);
            identity.plan_name = self.plan.clone();
            identity.login_method = Some(LoginMethod::ApiKey);
            snapshot.identity = Some(identity);
        }

        snapshot
    }
}

/// Response from z.ai user API.
#[allow(dead_code)]
#[derive(Debug, Deserialize)]
pub struct ZaiUserResponse {
    /// User email.
    #[serde(default)]
    pub email: Option<String>,

    /// User name.
    #[serde(default)]
    pub name: Option<String>,
}

// ============================================================================
// API Client
// ============================================================================

/// z.ai API client.
#[derive(Debug)]
pub struct ZaiApiClient {
    http: reqwest::Client,
}

impl ZaiApiClient {
    /// Creates a new client.
    pub fn new() -> Self {
        let http = reqwest::Client::builder()
            .timeout(std::time::Duration::from_secs(30))
            .build()
            .expect("Failed to build HTTP client");

        Self { http }
    }

    /// Build request headers.
    fn build_headers(&self, token: &str) -> Result<HeaderMap, ZaiError> {
        let mut headers = HeaderMap::new();

        headers.insert(USER_AGENT, HeaderValue::from_static("ExactoBar/1.0"));
        headers.insert(ACCEPT, HeaderValue::from_static("application/json"));

        let auth_value = format!("Bearer {}", token);
        headers.insert(
            AUTHORIZATION,
            HeaderValue::from_str(&auth_value)
                .map_err(|e| ZaiError::HttpError(format!("Invalid token: {}", e)))?,
        );

        Ok(headers)
    }

    /// Fetch usage.
    #[instrument(skip(self, token))]
    pub async fn fetch_usage(&self, token: &str) -> Result<ZaiUsageResponse, ZaiError> {
        debug!("Fetching z.ai usage");

        let url = format!("{}{}", ZAI_API_BASE, USAGE_ENDPOINT);
        let headers = self.build_headers(token)?;

        let response = self.http.get(&url).headers(headers).send().await?;

        let status = response.status();

        if status == reqwest::StatusCode::UNAUTHORIZED {
            return Err(ZaiError::AuthenticationFailed("Token rejected".to_string()));
        }

        if !status.is_success() {
            return Err(ZaiError::InvalidResponse(format!("HTTP {}", status)));
        }

        let body = response.text().await?;
        let usage: ZaiUsageResponse = serde_json::from_str(&body).map_err(|e| {
            warn!(error = %e, "Failed to parse usage response");
            ZaiError::InvalidResponse(format!("JSON error: {}", e))
        })?;

        Ok(usage)
    }
}

impl Default for ZaiApiClient {
    fn default() -> Self {
        Self::new()
    }
}

// ============================================================================
// Tests
// ============================================================================

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_client_creation() {
        let client = ZaiApiClient::new();
        assert!(std::mem::size_of_val(&client) > 0);
    }

    #[test]
    fn test_parse_usage_response() {
        let json = r#"{
            "creditsUsed": 50.0,
            "creditLimit": 100.0,
            "plan": "pro"
        }"#;

        let response: ZaiUsageResponse = serde_json::from_str(json).unwrap();
        assert_eq!(response.get_percent(), Some(50.0));
    }

    #[test]
    fn test_to_snapshot() {
        let response = ZaiUsageResponse {
            tokens_used: Some(500),
            token_limit: Some(1000),
            credits_used: None,
            credit_limit: None,
            reset_at: None,
            plan: Some("pro".to_string()),
        };

        let snapshot = response.to_snapshot();
        assert!(snapshot.primary.is_some());
        assert_eq!(snapshot.primary.unwrap().used_percent, 50.0);
    }
}
