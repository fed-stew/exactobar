//! Copilot API client.
//!
//! This module provides HTTP client functionality for the GitHub Copilot API.


use exactobar_core::{
    FetchSource, LoginMethod, ProviderIdentity, ProviderKind, UsageSnapshot, UsageWindow,
};
use reqwest::header::{HeaderMap, HeaderValue, ACCEPT, AUTHORIZATION, USER_AGENT};
use serde::Deserialize;
use tracing::{debug, instrument, warn};

use super::error::CopilotError;

// ============================================================================
// Constants
// ============================================================================

/// GitHub API base URL.
const GITHUB_API_BASE: &str = "https://api.github.com";

/// Copilot usage endpoint.
const COPILOT_USAGE_ENDPOINT: &str = "/user/copilot_billing/usage";

/// Copilot seats endpoint (for org admins).
#[allow(dead_code)]
const COPILOT_SEATS_ENDPOINT: &str = "/orgs/{org}/copilot/billing/seats";

/// User endpoint.
const USER_ENDPOINT: &str = "/user";

/// Copilot subscription endpoint.
const COPILOT_SUBSCRIPTION_ENDPOINT: &str = "/user/copilot_billing/seat";

/// User agent for API requests.
const USER_AGENT_VALUE: &str = "ExactoBar/1.0";

/// GitHub API version header.
const GITHUB_API_VERSION: &str = "2022-11-28";

// ============================================================================
// API Response Types
// ============================================================================

/// Response from Copilot usage API.
#[derive(Debug, Deserialize)]
pub struct CopilotUsageResponse {
    /// Total seats in the organization.
    #[serde(default)]
    pub total_seats: Option<u64>,

    /// Seats used.
    #[serde(default)]
    pub seats: Option<u64>,

    /// Suggestions accepted count.
    #[serde(default, alias = "suggestions_accepted")]
    pub acceptances: Option<u64>,

    /// Suggestions shown count.
    #[serde(default, alias = "suggestions_shown")]
    pub suggestions: Option<u64>,

    /// Lines accepted.
    #[serde(default)]
    pub lines_accepted: Option<u64>,

    /// Lines suggested.
    #[serde(default)]
    pub lines_suggested: Option<u64>,

    /// Active users in period.
    #[serde(default)]
    pub active_users: Option<u64>,

    /// Day of usage data.
    #[serde(default)]
    pub day: Option<String>,
}

impl CopilotUsageResponse {
    /// Get acceptance rate percentage.
    pub fn get_acceptance_rate(&self) -> Option<f64> {
        let accepted = self.acceptances? as f64;
        let suggested = self.suggestions? as f64;

        if suggested > 0.0 {
            Some((accepted / suggested) * 100.0)
        } else {
            None
        }
    }
}

/// Response from Copilot subscription/seat API.
#[derive(Debug, Deserialize)]
pub struct CopilotSeatResponse {
    /// Seat assignment status.
    #[serde(default)]
    pub created_at: Option<String>,

    /// Last activity time.
    #[serde(default)]
    pub last_activity_at: Option<String>,

    /// Editor used.
    #[serde(default)]
    pub last_activity_editor: Option<String>,

    /// Plan type.
    #[serde(default)]
    pub plan_type: Option<String>,

    /// Pending cancellation date.
    #[serde(default)]
    pub pending_cancellation_date: Option<String>,
}

/// Response from GitHub user API.
#[derive(Debug, Deserialize)]
pub struct GitHubUserResponse {
    /// GitHub login (username).
    pub login: String,

    /// User ID.
    pub id: u64,

    /// User email (may be null if private).
    #[serde(default)]
    pub email: Option<String>,

    /// User name.
    #[serde(default)]
    pub name: Option<String>,

    /// Plan info.
    #[serde(default)]
    pub plan: Option<GitHubPlan>,
}

/// GitHub plan info.
#[derive(Debug, Deserialize)]
pub struct GitHubPlan {
    /// Plan name (e.g., "pro", "free").
    pub name: String,
}

// ============================================================================
// Combined Usage Data
// ============================================================================

/// Combined Copilot usage data.
#[derive(Debug, Default)]
pub struct CopilotUsage {
    /// User info.
    pub user: Option<GitHubUserResponse>,

    /// Subscription/seat info.
    pub seat: Option<CopilotSeatResponse>,

    /// Usage statistics.
    pub usage: Vec<CopilotUsageResponse>,
}

impl CopilotUsage {
    /// Check if Copilot is enabled for this user.
    pub fn is_enabled(&self) -> bool {
        self.seat.is_some()
    }

    /// Get the plan type.
    pub fn plan_type(&self) -> Option<&str> {
        self.seat.as_ref()?.plan_type.as_deref()
    }

    /// Convert to UsageSnapshot.
    pub fn to_snapshot(&self) -> UsageSnapshot {
        let mut snapshot = UsageSnapshot::new();
        snapshot.fetch_source = FetchSource::OAuth;

        // If we have usage data, compute acceptance rate as "usage"
        if let Some(usage) = self.usage.first() {
            if let Some(rate) = usage.get_acceptance_rate() {
                // Use acceptance rate as the "usage" metric
                // Higher acceptance rate = better usage
                snapshot.primary = Some(UsageWindow::new(rate));
            }
        }

        // Build identity
        let mut identity = ProviderIdentity::new(ProviderKind::Copilot);

        if let Some(ref user) = self.user {
            identity.account_email = user.email.clone();
            identity.account_organization = Some(user.login.clone());
            if let Some(ref plan) = user.plan {
                identity.plan_name = Some(plan.name.clone());
            }
        }

        if let Some(ref seat) = self.seat {
            if let Some(ref plan_type) = seat.plan_type {
                identity.plan_name = Some(plan_type.clone());
            }
        }

        identity.login_method = Some(LoginMethod::OAuth);
        snapshot.identity = Some(identity);

        snapshot
    }
}

// ============================================================================
// API Client
// ============================================================================

/// Copilot API client.
#[derive(Debug)]
pub struct CopilotApiClient {
    http: reqwest::Client,
}

impl CopilotApiClient {
    /// Creates a new Copilot API client.
    pub fn new() -> Self {
        let http = reqwest::Client::builder()
            .timeout(std::time::Duration::from_secs(30))
            .build()
            .expect("Failed to build HTTP client");

        Self { http }
    }

    /// Build request headers.
    fn build_headers(&self, token: &str) -> Result<HeaderMap, CopilotError> {
        let mut headers = HeaderMap::new();

        headers.insert(USER_AGENT, HeaderValue::from_static(USER_AGENT_VALUE));
        headers.insert(ACCEPT, HeaderValue::from_static("application/vnd.github+json"));
        headers.insert(
            "X-GitHub-Api-Version",
            HeaderValue::from_static(GITHUB_API_VERSION),
        );

        let auth_value = format!("Bearer {}", token);
        headers.insert(
            AUTHORIZATION,
            HeaderValue::from_str(&auth_value)
                .map_err(|e| CopilotError::HttpError(format!("Invalid token: {}", e)))?,
        );

        Ok(headers)
    }

    /// Fetch user info.
    #[instrument(skip(self, token))]
    pub async fn fetch_user(&self, token: &str) -> Result<GitHubUserResponse, CopilotError> {
        debug!("Fetching GitHub user info");

        let url = format!("{}{}", GITHUB_API_BASE, USER_ENDPOINT);
        let headers = self.build_headers(token)?;

        let response = self.http.get(&url).headers(headers).send().await?;

        let status = response.status();

        if status == reqwest::StatusCode::UNAUTHORIZED {
            return Err(CopilotError::AuthenticationFailed("Token rejected".to_string()));
        }

        if !status.is_success() {
            return Err(CopilotError::InvalidResponse(format!("HTTP {}", status)));
        }

        let body = response.text().await?;
        let user: GitHubUserResponse = serde_json::from_str(&body).map_err(|e| {
            warn!(error = %e, "Failed to parse user response");
            CopilotError::InvalidResponse(format!("JSON error: {}", e))
        })?;

        Ok(user)
    }

    /// Fetch Copilot subscription status.
    #[instrument(skip(self, token))]
    pub async fn fetch_seat(&self, token: &str) -> Result<CopilotSeatResponse, CopilotError> {
        debug!("Fetching Copilot seat info");

        let url = format!("{}{}", GITHUB_API_BASE, COPILOT_SUBSCRIPTION_ENDPOINT);
        let headers = self.build_headers(token)?;

        let response = self.http.get(&url).headers(headers).send().await?;

        let status = response.status();

        if status == reqwest::StatusCode::NOT_FOUND {
            return Err(CopilotError::NotEnabled);
        }

        if status == reqwest::StatusCode::UNAUTHORIZED {
            return Err(CopilotError::AuthenticationFailed("Token rejected".to_string()));
        }

        if !status.is_success() {
            return Err(CopilotError::InvalidResponse(format!("HTTP {}", status)));
        }

        let body = response.text().await?;
        let seat: CopilotSeatResponse = serde_json::from_str(&body).map_err(|e| {
            CopilotError::InvalidResponse(format!("JSON error: {}", e))
        })?;

        Ok(seat)
    }

    /// Fetch Copilot usage statistics.
    #[instrument(skip(self, token))]
    pub async fn fetch_usage(
        &self,
        token: &str,
    ) -> Result<Vec<CopilotUsageResponse>, CopilotError> {
        debug!("Fetching Copilot usage stats");

        let url = format!("{}{}", GITHUB_API_BASE, COPILOT_USAGE_ENDPOINT);
        let headers = self.build_headers(token)?;

        let response = self.http.get(&url).headers(headers).send().await?;

        let status = response.status();

        if status == reqwest::StatusCode::NOT_FOUND {
            // Usage endpoint may not exist for all users
            return Ok(Vec::new());
        }

        if !status.is_success() {
            return Err(CopilotError::InvalidResponse(format!("HTTP {}", status)));
        }

        let body = response.text().await?;

        // Response could be an array or an object with a "usage" key
        if let Ok(usage) = serde_json::from_str::<Vec<CopilotUsageResponse>>(&body) {
            return Ok(usage);
        }

        if let Ok(single) = serde_json::from_str::<CopilotUsageResponse>(&body) {
            return Ok(vec![single]);
        }

        // Try as object with usage key
        #[derive(Deserialize)]
        struct UsageWrapper {
            usage: Vec<CopilotUsageResponse>,
        }

        if let Ok(wrapper) = serde_json::from_str::<UsageWrapper>(&body) {
            return Ok(wrapper.usage);
        }

        Ok(Vec::new())
    }

    /// Fetch all Copilot data.
    #[instrument(skip(self, token))]
    pub async fn fetch_all(&self, token: &str) -> Result<CopilotUsage, CopilotError> {
        debug!("Fetching all Copilot data");

        let mut data = CopilotUsage::default();

        // Fetch user info
        match self.fetch_user(token).await {
            Ok(user) => data.user = Some(user),
            Err(e) => warn!(error = %e, "Failed to fetch user info"),
        }

        // Fetch seat info
        match self.fetch_seat(token).await {
            Ok(seat) => data.seat = Some(seat),
            Err(CopilotError::NotEnabled) => {
                debug!("Copilot not enabled for user");
            }
            Err(e) => warn!(error = %e, "Failed to fetch seat info"),
        }

        // Fetch usage stats
        match self.fetch_usage(token).await {
            Ok(usage) => data.usage = usage,
            Err(e) => warn!(error = %e, "Failed to fetch usage stats"),
        }

        Ok(data)
    }
}

impl Default for CopilotApiClient {
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
        let client = CopilotApiClient::new();
        assert!(std::mem::size_of_val(&client) > 0);
    }

    #[test]
    fn test_parse_user_response() {
        let json = r#"{
            "login": "octocat",
            "id": 1,
            "email": "octocat@github.com",
            "name": "The Octocat",
            "plan": {
                "name": "pro"
            }
        }"#;

        let user: GitHubUserResponse = serde_json::from_str(json).unwrap();
        assert_eq!(user.login, "octocat");
        assert_eq!(user.email, Some("octocat@github.com".to_string()));
        assert_eq!(user.plan.unwrap().name, "pro");
    }

    #[test]
    fn test_parse_seat_response() {
        let json = r#"{
            "created_at": "2024-01-01T00:00:00Z",
            "plan_type": "copilot_business",
            "last_activity_at": "2024-06-15T12:00:00Z",
            "last_activity_editor": "vscode"
        }"#;

        let seat: CopilotSeatResponse = serde_json::from_str(json).unwrap();
        assert_eq!(seat.plan_type, Some("copilot_business".to_string()));
    }

    #[test]
    fn test_parse_usage_response() {
        let json = r#"{
            "total_seats": 100,
            "seats": 75,
            "suggestions_accepted": 1000,
            "suggestions_shown": 5000,
            "active_users": 50
        }"#;

        let usage: CopilotUsageResponse = serde_json::from_str(json).unwrap();
        assert_eq!(usage.total_seats, Some(100));
        assert_eq!(usage.get_acceptance_rate(), Some(20.0));
    }

    #[test]
    fn test_usage_to_snapshot() {
        let usage = CopilotUsage {
            user: Some(GitHubUserResponse {
                login: "test".to_string(),
                id: 123,
                email: Some("test@example.com".to_string()),
                name: Some("Test User".to_string()),
                plan: Some(GitHubPlan {
                    name: "pro".to_string(),
                }),
            }),
            seat: Some(CopilotSeatResponse {
                created_at: None,
                last_activity_at: None,
                last_activity_editor: None,
                plan_type: Some("copilot_individual".to_string()),
                pending_cancellation_date: None,
            }),
            usage: vec![CopilotUsageResponse {
                total_seats: None,
                seats: None,
                acceptances: Some(100),
                suggestions: Some(500),
                lines_accepted: None,
                lines_suggested: None,
                active_users: None,
                day: None,
            }],
        };

        let snapshot = usage.to_snapshot();
        assert!(snapshot.identity.is_some());
        let identity = snapshot.identity.unwrap();
        assert_eq!(identity.account_email, Some("test@example.com".to_string()));
        assert_eq!(identity.plan_name, Some("copilot_individual".to_string()));

        // Should have acceptance rate as primary usage
        assert!(snapshot.primary.is_some());
        assert_eq!(snapshot.primary.unwrap().used_percent, 20.0);
    }
}
