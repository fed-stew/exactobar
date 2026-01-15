//! Cursor web API client.
//!
//! This module provides HTTP client functionality for the Cursor API,
//! using browser cookies for authentication.

use chrono::{DateTime, Utc};
use exactobar_core::{LoginMethod, ProviderIdentity, ProviderKind, UsageSnapshot, UsageWindow};
use reqwest::header::{HeaderMap, HeaderValue, ACCEPT, COOKIE, USER_AGENT};
use serde::Deserialize;
use tracing::{debug, instrument, warn};

use super::error::CursorError;

// ============================================================================
// Constants
// ============================================================================

/// Cursor API base URL.
pub const CURSOR_API_BASE: &str = "https://www.cursor.com";

/// Cursor usage API endpoint.
const USAGE_ENDPOINT: &str = "/api/usage";

/// Cursor auth/me endpoint.
const AUTH_ME_ENDPOINT: &str = "/api/auth/me";

/// User agent for API requests.
const USER_AGENT_VALUE: &str = "ExactoBar/1.0";

/// Required cookie name for session.
const SESSION_COOKIE_NAMES: &[&str] = &[
    "__Secure-next-auth.session-token",
    "next-auth.session-token",
    "cursor_session",
    "session",
];

// ============================================================================
// API Response Types
// ============================================================================

/// Response from Cursor usage API.
#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct CursorUsageResponse {
    /// GPT-4 requests used.
    #[serde(default, alias = "gpt4_requests", alias = "numRequests")]
    pub gpt4_requests: Option<u64>,

    /// GPT-4 request limit.
    #[serde(default, alias = "gpt4_limit", alias = "maxRequests")]
    pub gpt4_limit: Option<u64>,

    /// Premium/fast requests used.
    #[serde(default, alias = "fast_requests", alias = "numFastRequests")]
    pub premium_requests: Option<u64>,

    /// Premium request limit.
    #[serde(default, alias = "fast_limit", alias = "maxFastRequests")]
    pub premium_limit: Option<u64>,

    /// Slow requests used.
    #[serde(default, alias = "slow_requests", alias = "numSlowRequests")]
    pub slow_requests: Option<u64>,

    /// Slow request limit.
    #[serde(default, alias = "slow_limit", alias = "maxSlowRequests")]
    pub slow_limit: Option<u64>,

    /// Billing period start.
    #[serde(default, alias = "period_start", alias = "startOfMonth")]
    pub period_start: Option<String>,

    /// Billing period end / reset time.
    #[serde(default, alias = "period_end", alias = "endOfMonth")]
    pub period_end: Option<String>,

    /// Monthly cost in USD.
    #[serde(default, alias = "monthly_cost")]
    pub monthly_cost_usd: Option<f64>,

    /// User's plan.
    #[serde(default)]
    pub plan: Option<String>,

    /// User email.
    #[serde(default)]
    pub email: Option<String>,
}

impl CursorUsageResponse {
    /// Get the primary usage percentage (GPT-4 or premium requests).
    pub fn get_primary_percent(&self) -> Option<f64> {
        // Try GPT-4 first
        if let (Some(used), Some(limit)) = (self.gpt4_requests, self.gpt4_limit) {
            if limit > 0 {
                return Some((used as f64 / limit as f64) * 100.0);
            }
        }

        // Try premium requests
        if let (Some(used), Some(limit)) = (self.premium_requests, self.premium_limit) {
            if limit > 0 {
                return Some((used as f64 / limit as f64) * 100.0);
            }
        }

        None
    }

    /// Get the secondary usage percentage (slow requests).
    pub fn get_secondary_percent(&self) -> Option<f64> {
        if let (Some(used), Some(limit)) = (self.slow_requests, self.slow_limit) {
            if limit > 0 {
                return Some((used as f64 / limit as f64) * 100.0);
            }
        }
        None
    }

    /// Get the reset time.
    pub fn get_reset_time(&self) -> Option<DateTime<Utc>> {
        let end_str = self.period_end.as_ref()?;

        // Try RFC3339 first
        if let Ok(dt) = chrono::DateTime::parse_from_rfc3339(end_str) {
            return Some(dt.with_timezone(&Utc));
        }

        // Try ISO 8601
        if let Ok(dt) = chrono::DateTime::parse_from_str(end_str, "%Y-%m-%dT%H:%M:%S%.fZ") {
            return Some(dt.with_timezone(&Utc));
        }

        // Try date only
        if let Ok(date) = chrono::NaiveDate::parse_from_str(end_str, "%Y-%m-%d") {
            return Some(
                date.and_hms_opt(0, 0, 0)?
                    .and_utc(),
            );
        }

        None
    }

    /// Convert to UsageSnapshot.
    pub fn to_snapshot(&self) -> UsageSnapshot {
        let mut snapshot = UsageSnapshot::new();
        snapshot.fetch_source = exactobar_core::FetchSource::Web;

        // Primary usage window
        if let Some(percent) = self.get_primary_percent() {
            let mut window = UsageWindow::new(percent);
            window.resets_at = self.get_reset_time();

            snapshot.primary = Some(window);
        }

        // Secondary usage window (slow requests)
        if let Some(percent) = self.get_secondary_percent() {
            snapshot.secondary = Some(UsageWindow::new(percent));
        }

        // Identity
        if self.email.is_some() || self.plan.is_some() {
            let mut identity = ProviderIdentity::new(ProviderKind::Cursor);
            identity.account_email = self.email.clone();
            identity.plan_name = self.plan.clone();
            identity.login_method = Some(LoginMethod::BrowserCookies);
            snapshot.identity = Some(identity);
        }

        snapshot
    }
}

/// Response from Cursor auth/me API.
#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct CursorAuthResponse {
    /// User email.
    #[serde(default)]
    pub email: Option<String>,

    /// User name.
    #[serde(default)]
    pub name: Option<String>,

    /// User ID.
    #[serde(default)]
    pub id: Option<String>,

    /// Subscription plan.
    #[serde(default)]
    pub plan: Option<String>,

    /// Whether user is a subscriber.
    #[serde(default, alias = "is_subscriber")]
    pub subscriber: Option<bool>,
}

// ============================================================================
// Web Client
// ============================================================================

/// Cursor web API client.
#[derive(Debug)]
pub struct CursorWebClient {
    http: reqwest::Client,
}

impl CursorWebClient {
    /// Creates a new Cursor web client.
    pub fn new() -> Self {
        let http = reqwest::Client::builder()
            .timeout(std::time::Duration::from_secs(30))
            .build()
            .expect("Failed to build HTTP client");

        Self { http }
    }

    /// Check if a cookie header contains a valid session cookie.
    pub fn has_session_cookie(cookie_header: &str) -> bool {
        SESSION_COOKIE_NAMES
            .iter()
            .any(|name| cookie_header.contains(name))
    }

    /// Fetch usage data from Cursor API.
    #[instrument(skip(self, cookie_header))]
    pub async fn fetch_usage(
        &self,
        cookie_header: &str,
    ) -> Result<CursorUsageResponse, CursorError> {
        debug!("Fetching Cursor usage via web API");

        if cookie_header.is_empty() {
            return Err(CursorError::NoSessionCookie);
        }

        let url = format!("{}{}", CURSOR_API_BASE, USAGE_ENDPOINT);
        let headers = self.build_headers(cookie_header)?;

        let response = self
            .http
            .get(&url)
            .headers(headers)
            .send()
            .await?;

        let status = response.status();

        if status == reqwest::StatusCode::UNAUTHORIZED {
            return Err(CursorError::AuthenticationFailed(
                "Session expired or invalid".to_string(),
            ));
        }

        if status == reqwest::StatusCode::TOO_MANY_REQUESTS {
            return Err(CursorError::RateLimited(
                "Too many requests".to_string(),
            ));
        }

        if !status.is_success() {
            let body = response.text().await.unwrap_or_default();
            return Err(CursorError::InvalidResponse(format!(
                "HTTP {}: {}",
                status, body
            )));
        }

        let body = response.text().await?;
        debug!(len = body.len(), "Got usage response");

        let usage: CursorUsageResponse = serde_json::from_str(&body).map_err(|e| {
            warn!(error = %e, body = %body, "Failed to parse usage response");
            CursorError::InvalidResponse(format!("JSON parse error: {}", e))
        })?;

        Ok(usage)
    }

    /// Fetch auth/user info from Cursor API.
    #[instrument(skip(self, cookie_header))]
    pub async fn fetch_auth(
        &self,
        cookie_header: &str,
    ) -> Result<CursorAuthResponse, CursorError> {
        debug!("Fetching Cursor auth info via web API");

        let url = format!("{}{}", CURSOR_API_BASE, AUTH_ME_ENDPOINT);
        let headers = self.build_headers(cookie_header)?;

        let response = self
            .http
            .get(&url)
            .headers(headers)
            .send()
            .await?;

        let status = response.status();

        if status == reqwest::StatusCode::UNAUTHORIZED {
            return Err(CursorError::AuthenticationFailed(
                "Session expired or invalid".to_string(),
            ));
        }

        if !status.is_success() {
            return Err(CursorError::InvalidResponse(format!(
                "HTTP {}",
                status
            )));
        }

        let body = response.text().await?;

        let auth: CursorAuthResponse = serde_json::from_str(&body).map_err(|e| {
            CursorError::InvalidResponse(format!("JSON parse error: {}", e))
        })?;

        Ok(auth)
    }

    /// Build request headers.
    fn build_headers(&self, cookie_header: &str) -> Result<HeaderMap, CursorError> {
        let mut headers = HeaderMap::new();

        headers.insert(
            USER_AGENT,
            HeaderValue::from_static(USER_AGENT_VALUE),
        );
        headers.insert(
            ACCEPT,
            HeaderValue::from_static("application/json"),
        );
        headers.insert(
            COOKIE,
            HeaderValue::from_str(cookie_header).map_err(|e| {
                CursorError::HttpError(format!("Invalid cookie header: {}", e))
            })?,
        );

        Ok(headers)
    }
}

impl Default for CursorWebClient {
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
        let client = CursorWebClient::new();
        assert!(std::mem::size_of_val(&client) > 0);
    }

    #[test]
    fn test_has_session_cookie() {
        assert!(CursorWebClient::has_session_cookie(
            "__Secure-next-auth.session-token=abc123"
        ));
        assert!(CursorWebClient::has_session_cookie(
            "next-auth.session-token=abc123"
        ));
        assert!(CursorWebClient::has_session_cookie(
            "other=val; cursor_session=abc; more=stuff"
        ));
        assert!(!CursorWebClient::has_session_cookie("random_cookie=here"));
    }

    #[test]
    fn test_parse_usage_response() {
        let json = r#"{
            "gpt4Requests": 150,
            "gpt4Limit": 500,
            "slowRequests": 50,
            "slowLimit": 200,
            "plan": "pro",
            "email": "user@example.com",
            "periodEnd": "2025-02-01"
        }"#;

        let response: CursorUsageResponse = serde_json::from_str(json).unwrap();
        assert_eq!(response.gpt4_requests, Some(150));
        assert_eq!(response.gpt4_limit, Some(500));
        assert_eq!(response.get_primary_percent(), Some(30.0));
        assert_eq!(response.get_secondary_percent(), Some(25.0));
    }

    #[test]
    fn test_parse_usage_response_alt_names() {
        let json = r#"{
            "numRequests": 100,
            "maxRequests": 400,
            "numSlowRequests": 20,
            "maxSlowRequests": 100
        }"#;

        let response: CursorUsageResponse = serde_json::from_str(json).unwrap();
        assert_eq!(response.gpt4_requests, Some(100));
        assert_eq!(response.gpt4_limit, Some(400));
        assert_eq!(response.get_primary_percent(), Some(25.0));
        assert_eq!(response.get_secondary_percent(), Some(20.0));
    }

    #[test]
    fn test_to_snapshot() {
        let response = CursorUsageResponse {
            gpt4_requests: Some(100),
            gpt4_limit: Some(500),
            premium_requests: None,
            premium_limit: None,
            slow_requests: Some(50),
            slow_limit: Some(200),
            period_start: None,
            period_end: Some("2025-02-01T00:00:00Z".to_string()),
            monthly_cost_usd: None,
            plan: Some("pro".to_string()),
            email: Some("user@example.com".to_string()),
        };

        let snapshot = response.to_snapshot();

        assert!(snapshot.primary.is_some());
        let primary = snapshot.primary.unwrap();
        assert_eq!(primary.used_percent, 20.0);
        assert!(primary.resets_at.is_some());

        assert!(snapshot.secondary.is_some());
        let secondary = snapshot.secondary.unwrap();
        assert_eq!(secondary.used_percent, 25.0);

        assert!(snapshot.identity.is_some());
        let identity = snapshot.identity.unwrap();
        assert_eq!(identity.plan_name, Some("pro".to_string()));
        assert_eq!(identity.account_email, Some("user@example.com".to_string()));
    }

    #[test]
    fn test_get_reset_time() {
        let response = CursorUsageResponse {
            gpt4_requests: None,
            gpt4_limit: None,
            premium_requests: None,
            premium_limit: None,
            slow_requests: None,
            slow_limit: None,
            period_start: None,
            period_end: Some("2025-02-01T00:00:00Z".to_string()),
            monthly_cost_usd: None,
            plan: None,
            email: None,
        };

        let reset = response.get_reset_time();
        assert!(reset.is_some());
    }
}
