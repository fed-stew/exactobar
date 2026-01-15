//! Cursor response parsers.

use chrono::Utc;
use exactobar_core::{
    FetchSource, LoginMethod, ProviderIdentity, ProviderKind, UsageSnapshot, UsageWindow,
};
use exactobar_fetch::FetchError;
use serde::Deserialize;
use tracing::{debug, warn};

// ============================================================================
// API Response Structures
// ============================================================================

/// Response from Cursor usage API.
#[allow(dead_code)]
#[derive(Debug, Deserialize)]
pub struct CursorApiResponse {
    /// Usage data.
    #[serde(default)]
    pub usage: Option<CursorUsageData>,
    /// Subscription info.
    #[serde(default)]
    pub subscription: Option<CursorSubscription>,
    /// User info.
    #[serde(default)]
    pub user: Option<CursorUser>,
}

/// Usage data from Cursor API.
#[allow(dead_code)]
#[derive(Debug, Deserialize)]
pub struct CursorUsageData {
    /// Number of requests made.
    #[serde(alias = "requestCount", alias = "request_count")]
    pub requests: Option<u64>,
    /// Request limit.
    #[serde(alias = "requestLimit", alias = "request_limit")]
    pub limit: Option<u64>,
    /// Premium requests made.
    #[serde(alias = "premiumRequests", alias = "premium_requests")]
    pub premium_requests: Option<u64>,
    /// Premium request limit.
    #[serde(alias = "premiumLimit", alias = "premium_limit")]
    pub premium_limit: Option<u64>,
    /// Period start date.
    #[serde(alias = "periodStart", alias = "period_start")]
    #[allow(dead_code)]
    pub period_start: Option<String>,
    /// Period end date.
    #[serde(alias = "periodEnd", alias = "period_end")]
    pub period_end: Option<String>,
}

/// Subscription info from Cursor API.
#[allow(dead_code)]
#[derive(Debug, Deserialize)]
pub struct CursorSubscription {
    /// Plan name (e.g., "pro", "free").
    pub plan: Option<String>,
    /// Whether subscription is active.
    #[serde(alias = "isActive", alias = "is_active")]
    #[allow(dead_code)]
    pub active: Option<bool>,
}

/// User info from Cursor API.
#[allow(dead_code)]
#[derive(Debug, Deserialize)]
pub struct CursorUser {
    /// User email.
    pub email: Option<String>,
    /// User name.
    #[allow(dead_code)]
    pub name: Option<String>,
}

// ============================================================================
// Local Config Structures
// ============================================================================

/// Cursor local state file structure.
#[allow(dead_code)]
#[derive(Debug, Deserialize)]
pub struct CursorLocalState {
    /// Usage statistics.
    #[serde(default)]
    pub usage: Option<CursorLocalUsage>,
    /// Account info.
    #[serde(default)]
    pub account: Option<CursorLocalAccount>,
}

#[allow(dead_code)]
#[derive(Debug, Deserialize)]
pub struct CursorLocalUsage {
    pub requests_today: Option<u64>,
    pub requests_this_month: Option<u64>,
}

#[allow(dead_code)]
#[derive(Debug, Deserialize)]
pub struct CursorLocalAccount {
    pub email: Option<String>,
    pub plan: Option<String>,
}

// ============================================================================
// Parsers
// ============================================================================

/// Parses Cursor API JSON response into a UsageSnapshot.
#[allow(dead_code)]
pub fn parse_cursor_api_response(json_str: &str) -> Result<UsageSnapshot, FetchError> {
    debug!(len = json_str.len(), "Parsing Cursor API response");

    let response: CursorApiResponse = serde_json::from_str(json_str).map_err(|e| {
        warn!(error = %e, "Failed to parse Cursor API JSON");
        FetchError::InvalidResponse(format!("Invalid JSON: {}", e))
    })?;

    let mut snapshot = UsageSnapshot::new();
    snapshot.fetch_source = FetchSource::Web;

    // Parse usage data
    if let Some(usage) = response.usage {
        // Primary window: regular requests
        if let (Some(requests), Some(limit)) = (usage.requests, usage.limit) {
            let percent = if limit > 0 {
                (requests as f64 / limit as f64) * 100.0
            } else {
                0.0
            };

            let mut window = UsageWindow::new(percent);

            // Parse reset time from period_end
            if let Some(end) = usage.period_end {
                if let Ok(dt) = chrono::DateTime::parse_from_rfc3339(&end) {
                    window.resets_at = Some(dt.with_timezone(&Utc));
                }
            }

            snapshot.primary = Some(window);
        }

        // Secondary window: premium requests
        if let (Some(premium), Some(premium_limit)) = (usage.premium_requests, usage.premium_limit) {
            let percent = if premium_limit > 0 {
                (premium as f64 / premium_limit as f64) * 100.0
            } else {
                0.0
            };
            snapshot.secondary = Some(UsageWindow::new(percent));
        }
    }

    // Parse identity
    if response.user.is_some() || response.subscription.is_some() {
        let mut identity = ProviderIdentity::new(ProviderKind::Cursor);

        if let Some(user) = response.user {
            identity.account_email = user.email;
        }

        if let Some(sub) = response.subscription {
            identity.plan_name = sub.plan;
        }

        identity.login_method = Some(LoginMethod::BrowserCookies);
        snapshot.identity = Some(identity);
    }

    debug!(
        has_primary = snapshot.primary.is_some(),
        has_secondary = snapshot.secondary.is_some(),
        "Cursor API response parsed"
    );

    Ok(snapshot)
}

/// Parses Cursor local config/state file.
#[allow(dead_code)]
pub fn parse_cursor_local_config(content: &str) -> Result<UsageSnapshot, FetchError> {
    debug!(len = content.len(), "Parsing Cursor local config");

    let state: CursorLocalState = serde_json::from_str(content).map_err(|e| {
        warn!(error = %e, "Failed to parse Cursor local state");
        FetchError::InvalidResponse(format!("Invalid local state: {}", e))
    })?;

    let mut snapshot = UsageSnapshot::new();
    snapshot.fetch_source = FetchSource::LocalProbe;

    // Parse local usage if available
    if let Some(usage) = state.usage {
        // We don't know the limits from local state, so just record the counts
        // This is limited but better than nothing
        if usage.requests_today.is_some() || usage.requests_this_month.is_some() {
            debug!(
                today = ?usage.requests_today,
                month = ?usage.requests_this_month,
                "Found local usage data"
            );
        }
    }

    // Parse account info
    if let Some(account) = state.account {
        let mut identity = ProviderIdentity::new(ProviderKind::Cursor);
        identity.account_email = account.email;
        identity.plan_name = account.plan;
        identity.login_method = Some(LoginMethod::CLI);
        snapshot.identity = Some(identity);
    }

    Ok(snapshot)
}

// ============================================================================
// Tests
// ============================================================================

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_parse_cursor_api_full() {
        let json = r#"{
            "usage": {
                "requests": 150,
                "limit": 500,
                "premiumRequests": 10,
                "premiumLimit": 50
            },
            "subscription": {
                "plan": "pro",
                "isActive": true
            },
            "user": {
                "email": "user@example.com"
            }
        }"#;

        let snapshot = parse_cursor_api_response(json).unwrap();

        assert!(snapshot.primary.is_some());
        let primary = snapshot.primary.unwrap();
        assert_eq!(primary.used_percent, 30.0); // 150/500 * 100

        assert!(snapshot.secondary.is_some());
        let secondary = snapshot.secondary.unwrap();
        assert_eq!(secondary.used_percent, 20.0); // 10/50 * 100

        assert!(snapshot.identity.is_some());
        let identity = snapshot.identity.unwrap();
        assert_eq!(identity.account_email, Some("user@example.com".to_string()));
        assert_eq!(identity.plan_name, Some("pro".to_string()));
    }

    #[test]
    fn test_parse_cursor_api_minimal() {
        let json = r#"{}"#;
        let snapshot = parse_cursor_api_response(json).unwrap();
        assert!(snapshot.primary.is_none());
    }

    #[test]
    fn test_parse_cursor_local_config() {
        let json = r#"{
            "account": {
                "email": "local@example.com",
                "plan": "free"
            }
        }"#;

        let snapshot = parse_cursor_local_config(json).unwrap();
        assert!(snapshot.identity.is_some());
        let identity = snapshot.identity.unwrap();
        assert_eq!(identity.account_email, Some("local@example.com".to_string()));
    }
}
