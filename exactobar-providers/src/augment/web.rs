//! Augment web API client with session keepalive.

use exactobar_core::{
    FetchSource, LoginMethod, ProviderIdentity, ProviderKind, UsageSnapshot, UsageWindow,
};
use reqwest::header::{HeaderMap, HeaderValue, ACCEPT, COOKIE, USER_AGENT};
use serde::Deserialize;
use tracing::{debug, instrument, warn};

use super::error::AugmentError;

// ============================================================================
// Constants
// ============================================================================

/// Augment API base URL.
const AUGMENT_API_BASE: &str = "https://augmentcode.com";

/// Usage endpoint.
const USAGE_ENDPOINT: &str = "/api/usage";

/// Keepalive endpoint.
const KEEPALIVE_ENDPOINT: &str = "/api/keepalive";

/// User endpoint.
#[allow(dead_code)]
const USER_ENDPOINT: &str = "/api/user";

/// Session cookie names.
const SESSION_COOKIE_NAMES: &[&str] = &[
    "__session",
    "augment_session",
    "session",
    "connect.sid",
];

// ============================================================================
// API Response Types
// ============================================================================

/// Response from Augment usage API.
#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct AugmentUsageResponse {
    /// Completions used.
    #[serde(default, alias = "completions_used")]
    pub completions_used: Option<u64>,

    /// Completion limit.
    #[serde(default, alias = "completion_limit")]
    pub completion_limit: Option<u64>,

    /// Tokens used.
    #[serde(default, alias = "tokens_used")]
    pub tokens_used: Option<u64>,

    /// Token limit.
    #[serde(default, alias = "token_limit")]
    pub token_limit: Option<u64>,

    /// Reset time.
    #[serde(default, alias = "reset_at")]
    pub reset_at: Option<String>,

    /// Plan name.
    #[serde(default)]
    pub plan: Option<String>,

    /// User email.
    #[serde(default)]
    pub email: Option<String>,
}

impl AugmentUsageResponse {
    /// Get primary usage percentage.
    pub fn get_percent(&self) -> Option<f64> {
        // Try completions first
        if let (Some(used), Some(limit)) = (self.completions_used, self.completion_limit) {
            if limit > 0 {
                return Some((used as f64 / limit as f64) * 100.0);
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
        snapshot.fetch_source = FetchSource::Web;

        if let Some(percent) = self.get_percent() {
            snapshot.primary = Some(UsageWindow::new(percent));
        }

        let mut identity = ProviderIdentity::new(ProviderKind::Augment);
        identity.account_email = self.email.clone();
        identity.plan_name = self.plan.clone();
        identity.login_method = Some(LoginMethod::BrowserCookies);
        snapshot.identity = Some(identity);

        snapshot
    }
}

// ============================================================================
// Web Client
// ============================================================================

/// Augment web API client with keepalive support.
#[derive(Debug)]
pub struct AugmentWebClient {
    http: reqwest::Client,
}

impl AugmentWebClient {
    /// Creates a new client.
    pub fn new() -> Self {
        let http = reqwest::Client::builder()
            .timeout(std::time::Duration::from_secs(30))
            .build()
            .expect("Failed to build HTTP client");

        Self { http }
    }

    /// Check for session cookie.
    pub fn has_session_cookie(cookie_header: &str) -> bool {
        SESSION_COOKIE_NAMES
            .iter()
            .any(|name| cookie_header.contains(name))
    }

    /// Build request headers.
    fn build_headers(&self, cookie_header: &str) -> Result<HeaderMap, AugmentError> {
        let mut headers = HeaderMap::new();

        headers.insert(USER_AGENT, HeaderValue::from_static("ExactoBar/1.0"));
        headers.insert(ACCEPT, HeaderValue::from_static("application/json"));
        headers.insert(
            COOKIE,
            HeaderValue::from_str(cookie_header)
                .map_err(|e| AugmentError::HttpError(format!("Invalid cookie: {}", e)))?,
        );

        Ok(headers)
    }

    /// Send keepalive to prevent session timeout.
    #[instrument(skip(self, cookie_header))]
    pub async fn send_keepalive(&self, cookie_header: &str) -> Result<(), AugmentError> {
        debug!("Sending Augment keepalive");

        let url = format!("{}{}", AUGMENT_API_BASE, KEEPALIVE_ENDPOINT);
        let headers = self.build_headers(cookie_header)?;

        let response = self.http.post(&url).headers(headers).send().await?;

        if !response.status().is_success() {
            return Err(AugmentError::SessionExpired);
        }

        Ok(())
    }

    /// Fetch usage.
    #[instrument(skip(self, cookie_header))]
    pub async fn fetch_usage(
        &self,
        cookie_header: &str,
    ) -> Result<AugmentUsageResponse, AugmentError> {
        debug!("Fetching Augment usage");

        // Send keepalive first to ensure session is active
        if let Err(e) = self.send_keepalive(cookie_header).await {
            warn!(error = %e, "Keepalive failed");
        }

        let url = format!("{}{}", AUGMENT_API_BASE, USAGE_ENDPOINT);
        let headers = self.build_headers(cookie_header)?;

        let response = self.http.get(&url).headers(headers).send().await?;

        let status = response.status();

        if status == reqwest::StatusCode::UNAUTHORIZED {
            return Err(AugmentError::AuthenticationFailed(
                "Session expired".to_string(),
            ));
        }

        if !status.is_success() {
            return Err(AugmentError::InvalidResponse(format!("HTTP {}", status)));
        }

        let body = response.text().await?;
        let usage: AugmentUsageResponse = serde_json::from_str(&body).map_err(|e| {
            warn!(error = %e, "Failed to parse usage response");
            AugmentError::InvalidResponse(format!("JSON error: {}", e))
        })?;

        Ok(usage)
    }
}

impl Default for AugmentWebClient {
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
        let client = AugmentWebClient::new();
        assert!(std::mem::size_of_val(&client) > 0);
    }

    #[test]
    fn test_has_session_cookie() {
        assert!(AugmentWebClient::has_session_cookie("__session=abc"));
        assert!(AugmentWebClient::has_session_cookie("connect.sid=xyz"));
        assert!(!AugmentWebClient::has_session_cookie("random=value"));
    }

    #[test]
    fn test_parse_usage_response() {
        let json = r#"{
            "completionsUsed": 500,
            "completionLimit": 1000,
            "plan": "pro",
            "email": "user@example.com"
        }"#;

        let response: AugmentUsageResponse = serde_json::from_str(json).unwrap();
        assert_eq!(response.get_percent(), Some(50.0));
        assert_eq!(response.email, Some("user@example.com".to_string()));
    }

    #[test]
    fn test_to_snapshot() {
        let response = AugmentUsageResponse {
            completions_used: Some(500),
            completion_limit: Some(1000),
            tokens_used: None,
            token_limit: None,
            reset_at: None,
            plan: Some("pro".to_string()),
            email: Some("user@example.com".to_string()),
        };

        let snapshot = response.to_snapshot();
        assert!(snapshot.primary.is_some());
        assert_eq!(snapshot.primary.unwrap().used_percent, 50.0);
    }
}
