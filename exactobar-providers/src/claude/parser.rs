//! Claude response parsers.

use chrono::{DateTime, Utc};
use exactobar_core::{
    FetchSource, LoginMethod, ProviderIdentity, ProviderKind, UsageSnapshot, UsageWindow,
};
use exactobar_fetch::FetchError;
use serde::Deserialize;
use tracing::{debug, warn};

// ============================================================================
// API Response Structures
// ============================================================================

/// Response from Claude API usage endpoint.
#[derive(Debug, Deserialize)]
pub struct ClaudeApiResponse {
    /// Session usage (5-hour window).
    #[serde(default)]
    pub session: Option<ClaudeUsageData>,
    /// Weekly usage.
    #[serde(default)]
    pub weekly: Option<ClaudeUsageData>,
    /// Opus/premium tier usage.
    #[serde(default)]
    pub opus: Option<ClaudeUsageData>,
    /// Organization information.
    #[serde(default)]
    pub organization: Option<ClaudeOrganization>,
    /// User information.
    #[serde(default)]
    pub user: Option<ClaudeUser>,
}

/// Usage data from Claude API.
#[derive(Debug, Deserialize)]
pub struct ClaudeUsageData {
    /// Percentage used (0-100).
    #[serde(alias = "usage_percent", alias = "percent", alias = "pct")]
    pub used_percent: Option<f64>,
    /// Remaining percentage.
    #[serde(alias = "remaining_percent")]
    pub remaining: Option<f64>,
    /// Window duration in minutes.
    #[serde(alias = "window_minutes", alias = "duration")]
    pub window: Option<u32>,
    /// Reset timestamp (ISO 8601).
    #[serde(alias = "reset_at", alias = "resets")]
    pub resets_at: Option<String>,
    /// Human-readable reset description.
    #[serde(alias = "reset_in", alias = "time_until_reset")]
    pub reset_description: Option<String>,
}

/// Organization info from Claude API.
#[derive(Debug, Deserialize)]
pub struct ClaudeOrganization {
    #[allow(dead_code)]
    pub id: Option<String>,
    pub name: Option<String>,
}

/// User info from Claude API.
#[derive(Debug, Deserialize)]
pub struct ClaudeUser {
    pub email: Option<String>,
    pub plan: Option<String>,
}

// ============================================================================
// Parsers
// ============================================================================

/// Parses Claude API JSON response into a UsageSnapshot.
pub fn parse_claude_api_response(json_str: &str) -> Result<UsageSnapshot, FetchError> {
    debug!(len = json_str.len(), "Parsing Claude API response");

    let response: ClaudeApiResponse = serde_json::from_str(json_str).map_err(|e| {
        warn!(error = %e, "Failed to parse Claude API JSON");
        FetchError::InvalidResponse(format!("Invalid JSON: {}", e))
    })?;

    let mut snapshot = UsageSnapshot::new();
    snapshot.fetch_source = FetchSource::OAuth;

    // Parse session (primary) window
    if let Some(session) = response.session {
        snapshot.primary = Some(parse_usage_data(session));
    }

    // Parse weekly (secondary) window
    if let Some(weekly) = response.weekly {
        snapshot.secondary = Some(parse_usage_data(weekly));
    }

    // Parse opus (tertiary) window
    if let Some(opus) = response.opus {
        snapshot.tertiary = Some(parse_usage_data(opus));
    }

    // Parse identity
    if response.user.is_some() || response.organization.is_some() {
        let mut identity = ProviderIdentity::new(ProviderKind::Claude);
        if let Some(user) = response.user {
            identity.account_email = user.email;
            identity.plan_name = user.plan;
        }
        if let Some(org) = response.organization {
            identity.account_organization = org.name;
        }
        identity.login_method = Some(LoginMethod::OAuth);
        snapshot.identity = Some(identity);
    }

    debug!(
        has_primary = snapshot.primary.is_some(),
        has_secondary = snapshot.secondary.is_some(),
        has_tertiary = snapshot.tertiary.is_some(),
        "Claude API response parsed"
    );

    Ok(snapshot)
}

/// Parses Claude CLI output into a UsageSnapshot.
///
/// # Arguments
/// * `output` - The CLI output string
/// * `is_json` - Whether the output is JSON format
pub fn parse_claude_cli_output(output: &str, is_json: bool) -> Result<UsageSnapshot, FetchError> {
    if is_json {
        return parse_claude_api_response(output);
    }

    // Parse text format
    // Example output:
    // Session: 45% used (resets in 2h 15m)
    // Weekly: 20% used (resets Sunday)
    // Opus: 30% used
    debug!("Parsing Claude CLI text output");

    let mut snapshot = UsageSnapshot::new();
    snapshot.fetch_source = FetchSource::CLI;

    for line in output.lines() {
        let line = line.trim();

        if let Some(rest) = line.strip_prefix("Session:") {
            if let Some(window) = parse_text_usage_line(rest) {
                snapshot.primary = Some(window);
            }
        } else if let Some(rest) = line.strip_prefix("Weekly:") {
            if let Some(window) = parse_text_usage_line(rest) {
                snapshot.secondary = Some(window);
            }
        } else if let Some(rest) = line.strip_prefix("Opus:") {
            if let Some(window) = parse_text_usage_line(rest) {
                snapshot.tertiary = Some(window);
            }
        }
    }

    Ok(snapshot)
}

/// Parses Claude web response (could be JSON or HTML).
#[allow(dead_code)]
pub fn parse_claude_web_response(body: &str) -> Result<UsageSnapshot, FetchError> {
    // Try JSON first
    if body.trim().starts_with('{') {
        return parse_claude_api_response(body);
    }

    // HTML parsing would go here
    // For now, return error if not JSON
    warn!("Claude web response is not JSON - HTML parsing not implemented");
    Err(FetchError::InvalidResponse(
        "HTML parsing not yet implemented".to_string(),
    ))
}

// ============================================================================
// Helper Functions
// ============================================================================

/// Parses a ClaudeUsageData into a UsageWindow.
fn parse_usage_data(data: ClaudeUsageData) -> UsageWindow {
    // Calculate used_percent from remaining if not provided directly
    let used_percent = data.used_percent.unwrap_or_else(|| {
        data.remaining.map(|r| 100.0 - r).unwrap_or(0.0)
    });

    let mut window = UsageWindow::new(used_percent);
    window.window_minutes = data.window;
    window.reset_description = data.reset_description;

    // Parse reset timestamp
    if let Some(reset_str) = data.resets_at {
        if let Ok(dt) = DateTime::parse_from_rfc3339(&reset_str) {
            window.resets_at = Some(dt.with_timezone(&Utc));
        }
    }

    window
}

/// Parses a text usage line like "45% used (resets in 2h 15m)".
pub(crate) fn parse_text_usage_line(text: &str) -> Option<UsageWindow> {
    let text = text.trim();

    // Look for percentage
    let percent_idx = text.find('%')?;
    let percent_str = text[..percent_idx].trim();
    let percent: f64 = percent_str.parse().ok()?;

    let mut window = UsageWindow::new(percent);

    // Look for reset description in parentheses
    if let Some(start) = text.find('(') {
        if let Some(end) = text.find(')') {
            let reset_desc = text[start + 1..end].trim();
            if reset_desc.starts_with("resets ") {
                window.reset_description = Some(reset_desc[7..].to_string());
            } else {
                window.reset_description = Some(reset_desc.to_string());
            }
        }
    }

    Some(window)
}

// ============================================================================
// Tests
// ============================================================================

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_parse_claude_api_full() {
        let json = r#"{
            "session": {
                "used_percent": 45.5,
                "window": 300,
                "reset_description": "in 2 hours"
            },
            "weekly": {
                "used_percent": 20.0,
                "window": 10080
            },
            "opus": {
                "used_percent": 30.0
            },
            "user": {
                "email": "user@example.com",
                "plan": "Pro"
            },
            "organization": {
                "name": "Acme Inc"
            }
        }"#;

        let snapshot = parse_claude_api_response(json).unwrap();

        assert!(snapshot.primary.is_some());
        assert_eq!(snapshot.primary.as_ref().unwrap().used_percent, 45.5);

        assert!(snapshot.secondary.is_some());
        assert_eq!(snapshot.secondary.as_ref().unwrap().used_percent, 20.0);

        assert!(snapshot.tertiary.is_some());
        assert_eq!(snapshot.tertiary.as_ref().unwrap().used_percent, 30.0);

        assert!(snapshot.identity.is_some());
        let identity = snapshot.identity.unwrap();
        assert_eq!(identity.account_email, Some("user@example.com".to_string()));
        assert_eq!(identity.account_organization, Some("Acme Inc".to_string()));
    }

    #[test]
    fn test_parse_claude_api_with_remaining() {
        let json = r#"{
            "session": {
                "remaining": 60.0
            }
        }"#;

        let snapshot = parse_claude_api_response(json).unwrap();
        assert!(snapshot.primary.is_some());
        // remaining 60% means used 40%
        assert_eq!(snapshot.primary.unwrap().used_percent, 40.0);
    }

    #[test]
    fn test_parse_text_usage_line() {
        let window = parse_text_usage_line("45% used (resets in 2h 15m)").unwrap();
        assert_eq!(window.used_percent, 45.0);
        assert_eq!(window.reset_description, Some("in 2h 15m".to_string()));

        let window = parse_text_usage_line("20% used").unwrap();
        assert_eq!(window.used_percent, 20.0);
        assert!(window.reset_description.is_none());
    }

    #[test]
    fn test_parse_claude_cli_text() {
        let output = r#"
Session: 45% used (resets in 2h 15m)
Weekly: 20% used (resets Sunday)
Opus: 30% used
"#;

        let snapshot = parse_claude_cli_output(output, false).unwrap();

        assert!(snapshot.primary.is_some());
        assert_eq!(snapshot.primary.as_ref().unwrap().used_percent, 45.0);

        assert!(snapshot.secondary.is_some());
        assert_eq!(snapshot.secondary.as_ref().unwrap().used_percent, 20.0);

        assert!(snapshot.tertiary.is_some());
        assert_eq!(snapshot.tertiary.as_ref().unwrap().used_percent, 30.0);
    }
}
