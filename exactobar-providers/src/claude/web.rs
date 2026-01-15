//! Claude web client using browser cookies.
//!
//! This module provides a client for fetching Claude usage data using
//! browser cookies from claude.ai.
//!
//! # Cookie Requirements
//!
//! The session cookie (typically `__Secure-next-auth.session-token` or similar)
//! must be present for authentication.

use serde::Deserialize;
use tracing::{debug, instrument, warn};

use super::error::ClaudeError;

// ============================================================================
// Constants
// ============================================================================

/// Claude.ai domain.
pub const CLAUDE_DOMAIN: &str = "claude.ai";

/// Usage API endpoint.
pub const USAGE_ENDPOINT: &str = "https://claude.ai/api/organizations/{org}/chat_conversations/usage";

/// Default organization ID.
pub const DEFAULT_ORG: &str = "default";

/// Session cookie names to check for.
const SESSION_COOKIE_NAMES: &[&str] = &[
    "__Secure-next-auth.session-token",
    "sessionKey",
    "session",
    "__cf_bm", // Cloudflare cookie (secondary indicator)
];

// ============================================================================
// Web API Response
// ============================================================================

/// Response from the web usage API.
#[derive(Debug, Clone, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct WebUsageResponse {
    /// Usage data.
    pub usage: Option<WebUsageData>,
    /// Organization info.
    pub organization: Option<WebOrganization>,
    /// User info.
    pub user: Option<WebUser>,
}

/// Usage data from web API.
#[derive(Debug, Clone, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct WebUsageData {
    /// Session/5-hour usage.
    pub session: Option<WebUsageWindow>,
    /// Weekly usage.
    pub weekly: Option<WebUsageWindow>,
    /// Opus usage.
    pub opus: Option<WebUsageWindow>,
    /// Sonnet usage.
    pub sonnet: Option<WebUsageWindow>,
}

/// Usage window from web API.
#[derive(Debug, Clone, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct WebUsageWindow {
    /// Used percentage.
    pub used_percent: Option<f64>,
    /// Remaining percentage.
    pub remaining_percent: Option<f64>,
    /// Utilization (alternative name).
    pub utilization: Option<f64>,
    /// Reset time (ISO 8601).
    pub resets_at: Option<String>,
    /// Reset description.
    pub reset_description: Option<String>,
}

impl WebUsageWindow {
    /// Get the used percentage.
    pub fn get_used_percent(&self) -> f64 {
        if let Some(used) = self.used_percent {
            return used;
        }
        if let Some(util) = self.utilization {
            return util;
        }
        if let Some(remaining) = self.remaining_percent {
            return 100.0 - remaining;
        }
        0.0
    }
}

/// Organization info from web API.
#[derive(Debug, Clone, Deserialize)]
pub struct WebOrganization {
    pub id: Option<String>,
    pub name: Option<String>,
}

/// User info from web API.
#[derive(Debug, Clone, Deserialize)]
pub struct WebUser {
    pub email: Option<String>,
    pub name: Option<String>,
}

// ============================================================================
// Web Client
// ============================================================================

/// Claude web client using browser cookies.
#[derive(Debug, Clone, Default)]
pub struct ClaudeWebClient;

impl ClaudeWebClient {
    /// Create a new web client.
    pub fn new() -> Self {
        Self
    }

    /// Check if a cookie header has a valid session cookie.
    pub fn has_session_cookie(cookie_header: &str) -> bool {
        let cookie_lower = cookie_header.to_lowercase();
        SESSION_COOKIE_NAMES
            .iter()
            .any(|name| cookie_lower.contains(&name.to_lowercase()))
    }

    /// Fetch usage using cookies.
    #[instrument(skip(self, cookie_header))]
    pub async fn fetch_usage(
        &self,
        cookie_header: &str,
        organization_id: Option<&str>,
    ) -> Result<WebUsageResponse, ClaudeError> {
        let org = organization_id.unwrap_or(DEFAULT_ORG);
        let url = USAGE_ENDPOINT.replace("{org}", org);

        debug!(url = %url, "Fetching usage from web API");

        let client = reqwest::Client::new();
        let response = client
            .get(&url)
            .header("Cookie", cookie_header)
            .header("Content-Type", "application/json")
            .header("Accept", "application/json")
            .header("User-Agent", "Mozilla/5.0 (Macintosh; Intel Mac OS X 10_15_7) AppleWebKit/537.36")
            .send()
            .await
            .map_err(|e| ClaudeError::HttpError(e.to_string()))?;

        let status = response.status();

        if status == reqwest::StatusCode::UNAUTHORIZED || status == reqwest::StatusCode::FORBIDDEN {
            return Err(ClaudeError::AuthenticationFailed(
                "Cookies rejected - may need to log in again".to_string(),
            ));
        }

        if !status.is_success() {
            let body = response.text().await.unwrap_or_default();
            warn!(status = %status, body = %body, "Web API request failed");
            return Err(ClaudeError::ApiError(format!("Status {}: {}", status, body)));
        }

        let body = response
            .text()
            .await
            .map_err(|e| ClaudeError::HttpError(e.to_string()))?;

        debug!(len = body.len(), "Received web API response");

        // Try to parse as our expected format
        let usage: WebUsageResponse = serde_json::from_str(&body).map_err(|e| {
            warn!(error = %e, body = %body, "Failed to parse web response");
            ClaudeError::ParseError(format!("Failed to parse response: {}", e))
        })?;

        Ok(usage)
    }

    /// Fetch usage with automatic cookie import.
    #[instrument(skip(self))]
    pub async fn fetch_usage_auto(
        &self,
        browser_cookies: &[(String, String)],
    ) -> Result<WebUsageResponse, ClaudeError> {
        // Build cookie header
        let cookie_header = browser_cookies
            .iter()
            .map(|(name, value)| format!("{}={}", name, value))
            .collect::<Vec<_>>()
            .join("; ");

        if !Self::has_session_cookie(&cookie_header) {
            return Err(ClaudeError::AuthenticationFailed(
                "No session cookie found".to_string(),
            ));
        }

        self.fetch_usage(&cookie_header, None).await
    }
}

// ============================================================================
// Conversion to Core Types
// ============================================================================

impl WebUsageResponse {
    /// Convert to a UsageSnapshot.
    pub fn to_snapshot(&self) -> exactobar_core::UsageSnapshot {
        use chrono::{DateTime, Utc};
        use exactobar_core::{FetchSource, LoginMethod, ProviderIdentity, ProviderKind};

        let mut snapshot = exactobar_core::UsageSnapshot::new();
        snapshot.fetch_source = FetchSource::Web;

        if let Some(ref usage) = self.usage {
            // Primary = session
            if let Some(ref session) = usage.session {
                snapshot.primary = Some(exactobar_core::UsageWindow {
                    used_percent: session.get_used_percent(),
                    window_minutes: Some(300),
                    resets_at: session.resets_at.as_ref().and_then(|s| {
                        DateTime::parse_from_rfc3339(s)
                            .ok()
                            .map(|dt| dt.with_timezone(&Utc))
                    }),
                    reset_description: session.reset_description.clone(),
                });
            }

            // Secondary = weekly
            if let Some(ref weekly) = usage.weekly {
                snapshot.secondary = Some(exactobar_core::UsageWindow {
                    used_percent: weekly.get_used_percent(),
                    window_minutes: Some(10080),
                    resets_at: weekly.resets_at.as_ref().and_then(|s| {
                        DateTime::parse_from_rfc3339(s)
                            .ok()
                            .map(|dt| dt.with_timezone(&Utc))
                    }),
                    reset_description: weekly.reset_description.clone(),
                });
            }

            // Tertiary = opus or sonnet
            let opus_or_sonnet = usage.opus.as_ref().or(usage.sonnet.as_ref());
            if let Some(window) = opus_or_sonnet {
                snapshot.tertiary = Some(exactobar_core::UsageWindow {
                    used_percent: window.get_used_percent(),
                    window_minutes: Some(10080),
                    resets_at: window.resets_at.as_ref().and_then(|s| {
                        DateTime::parse_from_rfc3339(s)
                            .ok()
                            .map(|dt| dt.with_timezone(&Utc))
                    }),
                    reset_description: window.reset_description.clone(),
                });
            }
        }

        // Identity
        if self.user.is_some() || self.organization.is_some() {
            let mut identity = ProviderIdentity::new(ProviderKind::Claude);
            if let Some(ref user) = self.user {
                identity.account_email = user.email.clone();
            }
            if let Some(ref org) = self.organization {
                identity.account_organization = org.name.clone();
            }
            identity.login_method = Some(LoginMethod::BrowserCookies);
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
    fn test_has_session_cookie() {
        assert!(ClaudeWebClient::has_session_cookie(
            "__Secure-next-auth.session-token=abc123"
        ));
        assert!(ClaudeWebClient::has_session_cookie("sessionKey=xyz"));
        assert!(ClaudeWebClient::has_session_cookie("other=1; session=abc; foo=2"));
        assert!(!ClaudeWebClient::has_session_cookie("other=123; foo=bar"));
    }

    #[test]
    fn test_parse_web_response() {
        let json = r#"{
            "usage": {
                "session": {
                    "usedPercent": 25.0,
                    "resetsAt": "2025-01-01T12:00:00Z"
                },
                "weekly": {
                    "usedPercent": 45.0
                },
                "opus": {
                    "remainingPercent": 70.0
                }
            },
            "user": {
                "email": "user@example.com"
            },
            "organization": {
                "name": "Acme Inc"
            }
        }"#;

        let response: WebUsageResponse = serde_json::from_str(json).unwrap();

        let usage = response.usage.as_ref().unwrap();
        let session = usage.session.as_ref().unwrap();
        assert!((session.get_used_percent() - 25.0).abs() < 0.01);

        let opus = usage.opus.as_ref().unwrap();
        // 70% remaining = 30% used
        assert!((opus.get_used_percent() - 30.0).abs() < 0.01);

        assert_eq!(
            response.user.as_ref().unwrap().email,
            Some("user@example.com".to_string())
        );
    }

    #[test]
    fn test_web_usage_window_get_used_percent() {
        // Test used_percent field
        let window = WebUsageWindow {
            used_percent: Some(25.0),
            remaining_percent: None,
            utilization: None,
            resets_at: None,
            reset_description: None,
        };
        assert!((window.get_used_percent() - 25.0).abs() < 0.01);

        // Test remaining_percent field
        let window = WebUsageWindow {
            used_percent: None,
            remaining_percent: Some(75.0),
            utilization: None,
            resets_at: None,
            reset_description: None,
        };
        assert!((window.get_used_percent() - 25.0).abs() < 0.01);

        // Test utilization field
        let window = WebUsageWindow {
            used_percent: None,
            remaining_percent: None,
            utilization: Some(30.0),
            resets_at: None,
            reset_description: None,
        };
        assert!((window.get_used_percent() - 30.0).abs() < 0.01);
    }

    #[test]
    fn test_to_snapshot() {
        let response = WebUsageResponse {
            usage: Some(WebUsageData {
                session: Some(WebUsageWindow {
                    used_percent: Some(25.0),
                    remaining_percent: None,
                    utilization: None,
                    resets_at: Some("2025-01-01T12:00:00Z".to_string()),
                    reset_description: None,
                }),
                weekly: Some(WebUsageWindow {
                    used_percent: Some(45.0),
                    remaining_percent: None,
                    utilization: None,
                    resets_at: None,
                    reset_description: None,
                }),
                opus: None,
                sonnet: None,
            }),
            user: Some(WebUser {
                email: Some("test@example.com".to_string()),
                name: None,
            }),
            organization: None,
        };

        let snapshot = response.to_snapshot();

        assert!(snapshot.primary.is_some());
        assert!((snapshot.primary.as_ref().unwrap().used_percent - 25.0).abs() < 0.01);

        assert!(snapshot.secondary.is_some());

        assert!(snapshot.identity.is_some());
    }
}
