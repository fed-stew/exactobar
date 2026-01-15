//! Codex response parsers.

use chrono::{DateTime, Utc};
use exactobar_core::{
    FetchSource, LoginMethod, ProviderIdentity, ProviderKind, UsageSnapshot, UsageWindow,
};
use exactobar_fetch::FetchError;
use serde::Deserialize;
use tracing::{debug, warn};

// ============================================================================
// CLI Output Structures
// ============================================================================

/// Response from `codex usage --json`.
#[derive(Debug, Deserialize)]
pub struct CodexCliResponse {
    /// Session usage data.
    #[serde(default)]
    pub session: Option<CodexUsageWindow>,
    /// Weekly usage data.
    #[serde(default)]
    pub weekly: Option<CodexUsageWindow>,
    /// Account information.
    #[serde(default)]
    pub account: Option<CodexAccount>,
    /// Credits information.
    #[serde(default)]
    #[allow(dead_code)]
    pub credits: Option<CodexCredits>,
}

/// Usage window from Codex CLI.
#[derive(Debug, Deserialize)]
pub struct CodexUsageWindow {
    /// Usage percentage (0-100).
    #[serde(alias = "usage_percent", alias = "percent")]
    pub used_percent: Option<f64>,
    /// Window duration in minutes.
    #[serde(alias = "duration_minutes")]
    pub window_minutes: Option<u32>,
    /// Reset timestamp.
    #[serde(alias = "reset_at")]
    pub resets_at: Option<String>,
    /// Human-readable reset description.
    #[serde(alias = "reset_in")]
    pub reset_description: Option<String>,
}

/// Account info from Codex CLI.
#[derive(Debug, Deserialize)]
pub struct CodexAccount {
    /// Email address.
    pub email: Option<String>,
    /// Organization name.
    #[serde(alias = "org")]
    pub organization: Option<String>,
    /// Plan name.
    pub plan: Option<String>,
}

/// Credits info from Codex CLI.
#[allow(dead_code)]
#[derive(Debug, Deserialize)]
pub struct CodexCredits {
    /// Remaining credits.
    pub remaining: Option<f64>,
    /// Total credits.
    pub total: Option<f64>,
    /// Credits unit (e.g., "USD").
    pub unit: Option<String>,
}

// ============================================================================
// API Response Structures
// ============================================================================

/// Response from OpenAI API (models endpoint for validation).
#[allow(dead_code)]
#[derive(Debug, Deserialize)]
pub struct OpenAiModelsResponse {
    pub data: Vec<OpenAiModel>,
}

#[allow(dead_code)]
#[derive(Debug, Deserialize)]
pub struct OpenAiModel {
    pub id: String,
}

// ============================================================================
// Parsers
// ============================================================================

/// Parses Codex CLI JSON output into a UsageSnapshot.
pub fn parse_codex_cli_output(json_str: &str) -> Result<UsageSnapshot, FetchError> {
    debug!(len = json_str.len(), "Parsing Codex CLI output");

    // Try to parse as JSON
    let response: CodexCliResponse = serde_json::from_str(json_str).map_err(|e| {
        warn!(error = %e, "Failed to parse Codex CLI JSON");
        FetchError::InvalidResponse(format!("Invalid JSON: {}", e))
    })?;

    // Build the snapshot
    let mut snapshot = UsageSnapshot::new();
    snapshot.fetch_source = FetchSource::CLI;

    // Parse session window
    if let Some(session) = response.session {
        snapshot.primary = Some(parse_usage_window(session));
    }

    // Parse weekly window
    if let Some(weekly) = response.weekly {
        snapshot.secondary = Some(parse_usage_window(weekly));
    }

    // Parse identity
    if let Some(account) = response.account {
        let mut identity = ProviderIdentity::new(ProviderKind::Codex);
        identity.account_email = account.email;
        identity.account_organization = account.organization;
        identity.plan_name = account.plan;
        identity.login_method = Some(LoginMethod::CLI);
        snapshot.identity = Some(identity);
    }

    debug!(
        has_primary = snapshot.primary.is_some(),
        has_secondary = snapshot.secondary.is_some(),
        has_identity = snapshot.identity.is_some(),
        "Codex CLI output parsed"
    );

    Ok(snapshot)
}

/// Parses a Codex usage window into our UsageWindow type.
fn parse_usage_window(window: CodexUsageWindow) -> UsageWindow {
    let mut result = UsageWindow::new(window.used_percent.unwrap_or(0.0));
    result.window_minutes = window.window_minutes;
    result.reset_description = window.reset_description;

    // Parse reset timestamp if present
    if let Some(reset_str) = window.resets_at {
        if let Ok(dt) = DateTime::parse_from_rfc3339(&reset_str) {
            result.resets_at = Some(dt.with_timezone(&Utc));
        }
    }

    result
}

/// Parses OpenAI API response (for validation).
#[allow(dead_code)]
pub fn parse_codex_api_response(json_str: &str) -> Result<UsageSnapshot, FetchError> {
    // This just validates the response is valid JSON
    let _: OpenAiModelsResponse = serde_json::from_str(json_str).map_err(|e| {
        FetchError::InvalidResponse(format!("Invalid API response: {}", e))
    })?;

    // Return minimal snapshot - API doesn't provide usage data
    let mut snapshot = UsageSnapshot::new();
    snapshot.fetch_source = FetchSource::Api;
    Ok(snapshot)
}

// ============================================================================
// Tests
// ============================================================================

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_parse_codex_cli_full() {
        let json = r#"{
            "session": {
                "used_percent": 45.5,
                "window_minutes": 300,
                "reset_description": "in 2 hours"
            },
            "weekly": {
                "used_percent": 20.0,
                "window_minutes": 10080
            },
            "account": {
                "email": "user@example.com",
                "organization": "Acme Inc",
                "plan": "Pro"
            }
        }"#;

        let snapshot = parse_codex_cli_output(json).unwrap();

        assert!(snapshot.primary.is_some());
        let primary = snapshot.primary.unwrap();
        assert_eq!(primary.used_percent, 45.5);
        assert_eq!(primary.window_minutes, Some(300));
        assert_eq!(primary.reset_description, Some("in 2 hours".to_string()));

        assert!(snapshot.secondary.is_some());
        let secondary = snapshot.secondary.unwrap();
        assert_eq!(secondary.used_percent, 20.0);

        assert!(snapshot.identity.is_some());
        let identity = snapshot.identity.unwrap();
        assert_eq!(identity.account_email, Some("user@example.com".to_string()));
    }

    #[test]
    fn test_parse_codex_cli_minimal() {
        let json = r#"{}"#;

        let snapshot = parse_codex_cli_output(json).unwrap();
        assert!(snapshot.primary.is_none());
        assert!(snapshot.secondary.is_none());
    }

    #[test]
    fn test_parse_codex_cli_invalid() {
        let result = parse_codex_cli_output("not json");
        assert!(result.is_err());
    }
}
