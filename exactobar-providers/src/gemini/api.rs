//! Gemini API client.
//!
//! This module provides HTTP client functionality for the Gemini API.


use exactobar_core::{
    FetchSource, LoginMethod, ProviderIdentity, ProviderKind, UsageSnapshot, UsageWindow,
};
use reqwest::header::{HeaderMap, HeaderValue, ACCEPT, AUTHORIZATION, USER_AGENT};
use serde::Deserialize;
use tracing::{debug, instrument, warn};

use super::error::GeminiError;

// ============================================================================
// Constants
// ============================================================================

/// Gemini API base URL (Generative Language API).
const GEMINI_API_BASE: &str = "https://generativelanguage.googleapis.com";

/// AI Studio API base URL.
#[allow(dead_code)]
const AI_STUDIO_API_BASE: &str = "https://aistudio.googleapis.com";

/// Vertex AI base URL.
#[allow(dead_code)]
const VERTEX_AI_BASE: &str = "https://aiplatform.googleapis.com";

/// User agent for API requests.
const USER_AGENT_VALUE: &str = "ExactoBar/1.0";

// ============================================================================
// API Response Types
// ============================================================================

/// Response from quota/usage endpoint.
#[allow(dead_code)]
#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct GeminiQuotaResponse {
    /// Quota limits.
    #[serde(default)]
    pub limits: Vec<QuotaLimit>,

    /// Current usage.
    #[serde(default)]
    pub usage: Option<QuotaUsage>,
}

/// A quota limit.
#[allow(dead_code)]
#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct QuotaLimit {
    /// Limit name.
    #[serde(default)]
    pub name: Option<String>,

    /// Requests per minute.
    #[serde(default)]
    pub requests_per_minute: Option<u64>,

    /// Requests per day.
    #[serde(default)]
    pub requests_per_day: Option<u64>,

    /// Tokens per minute.
    #[serde(default)]
    pub tokens_per_minute: Option<u64>,
}

/// Current quota usage.
#[allow(dead_code)]
#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct QuotaUsage {
    /// Requests made today.
    #[serde(default)]
    pub requests_today: Option<u64>,

    /// Tokens used today.
    #[serde(default)]
    pub tokens_today: Option<u64>,

    /// When this usage resets.
    #[serde(default)]
    pub resets_at: Option<String>,
}

/// Model list response.
#[derive(Debug, Deserialize)]
pub struct ModelsResponse {
    /// Available models.
    #[serde(default)]
    pub models: Vec<ModelInfo>,
}

/// Model information.
#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ModelInfo {
    /// Model name (e.g., "models/gemini-pro").
    pub name: String,

    /// Display name.
    #[serde(default)]
    pub display_name: Option<String>,

    /// Model description.
    #[serde(default)]
    pub description: Option<String>,

    /// Input token limit.
    #[serde(default)]
    pub input_token_limit: Option<u64>,

    /// Output token limit.
    #[serde(default)]
    pub output_token_limit: Option<u64>,
}

// ============================================================================
// Combined Quota Data
// ============================================================================

/// Combined Gemini quota data.
#[derive(Debug, Default)]
pub struct GeminiQuota {
    /// Requests per minute limit.
    pub requests_per_minute: Option<u64>,

    /// Requests per day limit.
    pub requests_per_day: Option<u64>,

    /// Requests used today.
    pub used_today: Option<u64>,

    /// Tokens per minute limit.
    pub tokens_per_minute: Option<u64>,

    /// Tokens used today.
    pub tokens_used_today: Option<u64>,

    /// Account/project info.
    pub account: Option<String>,

    /// Project ID.
    pub project: Option<String>,

    /// Available models.
    pub models: Vec<String>,
}

impl GeminiQuota {
    /// Get daily usage percentage.
    pub fn get_daily_percent(&self) -> Option<f64> {
        let used = self.used_today? as f64;
        let limit = self.requests_per_day? as f64;

        if limit > 0.0 {
            Some((used / limit) * 100.0)
        } else {
            None
        }
    }

    /// Check if we have any quota data.
    pub fn has_data(&self) -> bool {
        self.requests_per_minute.is_some()
            || self.requests_per_day.is_some()
            || !self.models.is_empty()
    }

    /// Convert to UsageSnapshot.
    pub fn to_snapshot(&self) -> UsageSnapshot {
        let mut snapshot = UsageSnapshot::new();
        snapshot.fetch_source = FetchSource::OAuth;

        // If we have daily usage, use that as primary
        if let Some(percent) = self.get_daily_percent() {
            snapshot.primary = Some(UsageWindow::new(percent));
        }

        // Build identity
        let mut identity = ProviderIdentity::new(ProviderKind::Gemini);
        identity.account_email = self.account.clone();
        identity.account_organization = self.project.clone();
        identity.login_method = Some(LoginMethod::OAuth);

        // Set plan name based on limits
        if self.requests_per_minute.is_some() {
            identity.plan_name = Some("API Access".to_string());
        }

        snapshot.identity = Some(identity);
        snapshot
    }
}

// ============================================================================
// API Client
// ============================================================================

/// Gemini API client.
#[derive(Debug)]
pub struct GeminiApiClient {
    http: reqwest::Client,
}

impl GeminiApiClient {
    /// Creates a new Gemini API client.
    pub fn new() -> Self {
        let http = reqwest::Client::builder()
            .timeout(std::time::Duration::from_secs(30))
            .build()
            .expect("Failed to build HTTP client");

        Self { http }
    }

    /// Build request headers.
    fn build_headers(&self, token: &str) -> Result<HeaderMap, GeminiError> {
        let mut headers = HeaderMap::new();

        headers.insert(USER_AGENT, HeaderValue::from_static(USER_AGENT_VALUE));
        headers.insert(ACCEPT, HeaderValue::from_static("application/json"));

        let auth_value = format!("Bearer {}", token);
        headers.insert(
            AUTHORIZATION,
            HeaderValue::from_str(&auth_value)
                .map_err(|e| GeminiError::HttpError(format!("Invalid token: {}", e)))?,
        );

        Ok(headers)
    }

    /// List available models (verifies API access).
    #[instrument(skip(self, token))]
    pub async fn list_models(&self, token: &str) -> Result<Vec<ModelInfo>, GeminiError> {
        debug!("Listing Gemini models");

        let url = format!("{}/v1beta/models", GEMINI_API_BASE);
        let headers = self.build_headers(token)?;

        let response = self.http.get(&url).headers(headers).send().await?;

        let status = response.status();

        if status == reqwest::StatusCode::UNAUTHORIZED {
            return Err(GeminiError::AuthenticationFailed("Token rejected".to_string()));
        }

        if status == reqwest::StatusCode::TOO_MANY_REQUESTS {
            return Err(GeminiError::RateLimited("Too many requests".to_string()));
        }

        if !status.is_success() {
            return Err(GeminiError::InvalidResponse(format!("HTTP {}", status)));
        }

        let body = response.text().await?;
        let models: ModelsResponse = serde_json::from_str(&body).map_err(|e| {
            warn!(error = %e, "Failed to parse models response");
            GeminiError::InvalidResponse(format!("JSON error: {}", e))
        })?;

        Ok(models.models)
    }

    /// Fetch quota information.
    ///
    /// Note: Gemini doesn't have a direct quota endpoint like other providers.
    /// We infer quota from the models endpoint and any rate limit headers.
    #[instrument(skip(self, token))]
    pub async fn fetch_quota(&self, token: &str) -> Result<GeminiQuota, GeminiError> {
        debug!("Fetching Gemini quota");

        let mut quota = GeminiQuota::default();

        // Try to list models to verify API access and get rate limits
        let url = format!("{}/v1beta/models", GEMINI_API_BASE);
        let headers = self.build_headers(token)?;

        let response = self.http.get(&url).headers(headers).send().await?;

        // Check for rate limit headers
        if let Some(rpm) = response
            .headers()
            .get("x-ratelimit-limit-requests")
            .and_then(|v| v.to_str().ok())
            .and_then(|s| s.parse().ok())
        {
            quota.requests_per_minute = Some(rpm);
        }

        if let Some(remaining) = response
            .headers()
            .get("x-ratelimit-remaining-requests")
            .and_then(|v| v.to_str().ok())
            .and_then(|s| s.parse::<u64>().ok())
        {
            if let Some(limit) = quota.requests_per_minute {
                quota.used_today = Some(limit.saturating_sub(remaining));
            }
        }

        let status = response.status();

        if status == reqwest::StatusCode::UNAUTHORIZED {
            return Err(GeminiError::AuthenticationFailed("Token rejected".to_string()));
        }

        if !status.is_success() {
            return Err(GeminiError::InvalidResponse(format!("HTTP {}", status)));
        }

        // Parse models
        let body = response.text().await?;
        if let Ok(models) = serde_json::from_str::<ModelsResponse>(&body) {
            quota.models = models.models.iter().map(|m| m.name.clone()).collect();
        }

        // Set some default free tier limits if we couldn't get them from headers
        if quota.requests_per_minute.is_none() {
            // Free tier limits (approximate)
            quota.requests_per_minute = Some(60);
            quota.requests_per_day = Some(1500);
        }

        Ok(quota)
    }

    /// Fetch all available data.
    #[instrument(skip(self, token))]
    pub async fn fetch_all(
        &self,
        token: &str,
        account: Option<String>,
        project: Option<String>,
    ) -> Result<GeminiQuota, GeminiError> {
        let mut quota = self.fetch_quota(token).await?;
        quota.account = account;
        quota.project = project;
        Ok(quota)
    }
}

impl Default for GeminiApiClient {
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
        let client = GeminiApiClient::new();
        assert!(std::mem::size_of_val(&client) > 0);
    }

    #[test]
    fn test_parse_models_response() {
        let json = r#"{
            "models": [
                {
                    "name": "models/gemini-pro",
                    "displayName": "Gemini Pro",
                    "description": "The best model for general use",
                    "inputTokenLimit": 30720,
                    "outputTokenLimit": 2048
                },
                {
                    "name": "models/gemini-pro-vision",
                    "displayName": "Gemini Pro Vision"
                }
            ]
        }"#;

        let response: ModelsResponse = serde_json::from_str(json).unwrap();
        assert_eq!(response.models.len(), 2);
        assert_eq!(response.models[0].name, "models/gemini-pro");
        assert_eq!(response.models[0].input_token_limit, Some(30720));
    }

    #[test]
    fn test_quota_get_daily_percent() {
        let quota = GeminiQuota {
            requests_per_day: Some(1500),
            used_today: Some(750),
            ..Default::default()
        };

        assert_eq!(quota.get_daily_percent(), Some(50.0));
    }

    #[test]
    fn test_quota_has_data() {
        let empty = GeminiQuota::default();
        assert!(!empty.has_data());

        let with_limits = GeminiQuota {
            requests_per_minute: Some(60),
            ..Default::default()
        };
        assert!(with_limits.has_data());

        let with_models = GeminiQuota {
            models: vec!["gemini-pro".to_string()],
            ..Default::default()
        };
        assert!(with_models.has_data());
    }

    #[test]
    fn test_quota_to_snapshot() {
        let quota = GeminiQuota {
            requests_per_day: Some(1500),
            used_today: Some(750),
            account: Some("user@example.com".to_string()),
            project: Some("my-project".to_string()),
            ..Default::default()
        };

        let snapshot = quota.to_snapshot();

        assert!(snapshot.primary.is_some());
        assert_eq!(snapshot.primary.unwrap().used_percent, 50.0);

        assert!(snapshot.identity.is_some());
        let identity = snapshot.identity.unwrap();
        assert_eq!(identity.account_email, Some("user@example.com".to_string()));
        assert_eq!(identity.account_organization, Some("my-project".to_string()));
    }
}
