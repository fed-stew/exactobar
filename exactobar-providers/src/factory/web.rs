//! Factory web API client.
//!
//! Factory (Droid) uses WorkOS for authentication.

use std::path::PathBuf;

use exactobar_core::{
    FetchSource, LoginMethod, ProviderIdentity, ProviderKind, UsageSnapshot, UsageWindow,
};
use reqwest::header::{HeaderMap, HeaderValue, ACCEPT, AUTHORIZATION, COOKIE, USER_AGENT};
use serde::Deserialize;
use tracing::{debug, instrument, warn};

use super::error::FactoryError;

// ============================================================================
// Constants
// ============================================================================

/// Factory API base URL.
pub const FACTORY_API_BASE: &str = "https://app.factory.ai";

/// Factory usage endpoint.
const USAGE_ENDPOINT: &str = "/api/usage";

/// Factory user endpoint.
const USER_ENDPOINT: &str = "/api/user";

/// User agent for API requests.
const USER_AGENT_VALUE: &str = "ExactoBar/1.0";

/// Session cookie names.
const SESSION_COOKIE_NAMES: &[&str] = &[
    "__session",
    "factory_session",
    "workos_session",
    "session",
];

// ============================================================================
// API Response Types
// ============================================================================

/// Response from Factory usage API.
#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct FactoryUsageResponse {
    /// Tokens used.
    #[serde(default, alias = "tokens_used")]
    pub tokens_used: Option<u64>,

    /// Token limit.
    #[serde(default, alias = "token_limit")]
    pub token_limit: Option<u64>,

    /// Requests made.
    #[serde(default, alias = "requests_made")]
    pub requests_made: Option<u64>,

    /// Request limit.
    #[serde(default, alias = "request_limit")]
    pub request_limit: Option<u64>,

    /// Period reset time.
    #[serde(default, alias = "reset_at")]
    pub reset_at: Option<String>,

    /// Plan name.
    #[serde(default)]
    pub plan: Option<String>,
}

impl FactoryUsageResponse {
    /// Get token usage percentage.
    pub fn get_token_percent(&self) -> Option<f64> {
        let used = self.tokens_used? as f64;
        let limit = self.token_limit? as f64;
        if limit > 0.0 {
            Some((used / limit) * 100.0)
        } else {
            None
        }
    }

    /// Get request usage percentage.
    pub fn get_request_percent(&self) -> Option<f64> {
        let used = self.requests_made? as f64;
        let limit = self.request_limit? as f64;
        if limit > 0.0 {
            Some((used / limit) * 100.0)
        } else {
            None
        }
    }

    /// Convert to UsageSnapshot.
    pub fn to_snapshot(&self) -> UsageSnapshot {
        let mut snapshot = UsageSnapshot::new();
        snapshot.fetch_source = FetchSource::Web;

        // Use token usage as primary
        if let Some(percent) = self.get_token_percent() {
            snapshot.primary = Some(UsageWindow::new(percent));
        } else if let Some(percent) = self.get_request_percent() {
            snapshot.primary = Some(UsageWindow::new(percent));
        }

        // Use request usage as secondary
        if self.get_token_percent().is_some() {
            if let Some(percent) = self.get_request_percent() {
                snapshot.secondary = Some(UsageWindow::new(percent));
            }
        }

        // Identity
        if self.plan.is_some() {
            let mut identity = ProviderIdentity::new(ProviderKind::Factory);
            identity.plan_name = self.plan.clone();
            identity.login_method = Some(LoginMethod::BrowserCookies);
            snapshot.identity = Some(identity);
        }

        snapshot
    }
}

/// Response from Factory user API.
#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct FactoryUserResponse {
    /// User email.
    #[serde(default)]
    pub email: Option<String>,

    /// User name.
    #[serde(default)]
    pub name: Option<String>,

    /// Organization.
    #[serde(default)]
    pub organization: Option<String>,
}

/// WorkOS token stored locally.
#[allow(dead_code)]
#[derive(Debug, Deserialize)]
pub struct WorkOSToken {
    /// Access token.
    #[serde(default)]
    pub access_token: Option<String>,

    /// Refresh token.
    #[serde(default)]
    pub refresh_token: Option<String>,

    /// Token expiry.
    #[serde(default)]
    pub expires_at: Option<String>,
}

// ============================================================================
// Web Client
// ============================================================================

/// Factory web API client.
#[derive(Debug)]
pub struct FactoryWebClient {
    http: reqwest::Client,
}

impl FactoryWebClient {
    /// Creates a new Factory web client.
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

    /// Get WorkOS token storage path.
    pub fn workos_token_path() -> Option<PathBuf> {
        let config_dir = dirs::config_dir()?;
        Some(config_dir.join("factory").join("auth.json"))
    }

    /// Load WorkOS token from local storage.
    pub fn load_workos_token() -> Option<String> {
        let path = Self::workos_token_path()?;
        if !path.exists() {
            return None;
        }

        let content = std::fs::read_to_string(&path).ok()?;
        let token: WorkOSToken = serde_json::from_str(&content).ok()?;
        token.access_token
    }

    /// Build request headers.
    fn build_headers(&self, auth: &str, is_bearer: bool) -> Result<HeaderMap, FactoryError> {
        let mut headers = HeaderMap::new();

        headers.insert(USER_AGENT, HeaderValue::from_static(USER_AGENT_VALUE));
        headers.insert(ACCEPT, HeaderValue::from_static("application/json"));

        if is_bearer {
            let auth_value = format!("Bearer {}", auth);
            headers.insert(
                AUTHORIZATION,
                HeaderValue::from_str(&auth_value)
                    .map_err(|e| FactoryError::HttpError(format!("Invalid auth: {}", e)))?,
            );
        } else {
            headers.insert(
                COOKIE,
                HeaderValue::from_str(auth)
                    .map_err(|e| FactoryError::HttpError(format!("Invalid cookie: {}", e)))?,
            );
        }

        Ok(headers)
    }

    /// Fetch usage data.
    #[instrument(skip(self, auth))]
    pub async fn fetch_usage(
        &self,
        auth: &str,
        is_bearer: bool,
    ) -> Result<FactoryUsageResponse, FactoryError> {
        debug!("Fetching Factory usage");

        let url = format!("{}{}", FACTORY_API_BASE, USAGE_ENDPOINT);
        let headers = self.build_headers(auth, is_bearer)?;

        let response = self.http.get(&url).headers(headers).send().await?;

        let status = response.status();

        if status == reqwest::StatusCode::UNAUTHORIZED {
            return Err(FactoryError::AuthenticationFailed(
                "Session expired".to_string(),
            ));
        }

        if !status.is_success() {
            return Err(FactoryError::InvalidResponse(format!("HTTP {}", status)));
        }

        let body = response.text().await?;
        let usage: FactoryUsageResponse = serde_json::from_str(&body).map_err(|e| {
            warn!(error = %e, "Failed to parse usage response");
            FactoryError::InvalidResponse(format!("JSON error: {}", e))
        })?;

        Ok(usage)
    }

    /// Fetch user info.
    #[instrument(skip(self, auth))]
    pub async fn fetch_user(
        &self,
        auth: &str,
        is_bearer: bool,
    ) -> Result<FactoryUserResponse, FactoryError> {
        debug!("Fetching Factory user info");

        let url = format!("{}{}", FACTORY_API_BASE, USER_ENDPOINT);
        let headers = self.build_headers(auth, is_bearer)?;

        let response = self.http.get(&url).headers(headers).send().await?;

        let status = response.status();

        if !status.is_success() {
            return Err(FactoryError::InvalidResponse(format!("HTTP {}", status)));
        }

        let body = response.text().await?;
        let user: FactoryUserResponse = serde_json::from_str(&body).map_err(|e| {
            FactoryError::InvalidResponse(format!("JSON error: {}", e))
        })?;

        Ok(user)
    }
}

impl Default for FactoryWebClient {
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
        let client = FactoryWebClient::new();
        assert!(std::mem::size_of_val(&client) > 0);
    }

    #[test]
    fn test_has_session_cookie() {
        assert!(FactoryWebClient::has_session_cookie("__session=abc"));
        assert!(FactoryWebClient::has_session_cookie("workos_session=xyz"));
        assert!(!FactoryWebClient::has_session_cookie("random=value"));
    }

    #[test]
    fn test_parse_usage_response() {
        let json = r#"{
            "tokensUsed": 5000,
            "tokenLimit": 10000,
            "requestsMade": 100,
            "requestLimit": 500,
            "plan": "pro"
        }"#;

        let response: FactoryUsageResponse = serde_json::from_str(json).unwrap();
        assert_eq!(response.get_token_percent(), Some(50.0));
        assert_eq!(response.get_request_percent(), Some(20.0));
    }

    #[test]
    fn test_to_snapshot() {
        let response = FactoryUsageResponse {
            tokens_used: Some(5000),
            token_limit: Some(10000),
            requests_made: None,
            request_limit: None,
            reset_at: None,
            plan: Some("pro".to_string()),
        };

        let snapshot = response.to_snapshot();
        assert!(snapshot.primary.is_some());
        assert_eq!(snapshot.primary.unwrap().used_percent, 50.0);
    }
}
