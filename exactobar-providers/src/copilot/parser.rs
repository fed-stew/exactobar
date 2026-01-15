//! Copilot response parser.

use exactobar_core::{FetchSource, LoginMethod, ProviderIdentity, ProviderKind, UsageSnapshot, UsageWindow};
use exactobar_fetch::FetchError;
use serde::Deserialize;
use tracing::debug;

#[derive(Debug, Deserialize)]
pub struct CopilotUsageResponse {
    #[serde(default)]
    pub completions: Option<CopilotCompletions>,
    #[serde(default)]
    pub user: Option<CopilotUser>,
}

#[derive(Debug, Deserialize)]
pub struct CopilotCompletions {
    #[allow(dead_code)]
    pub accepted: Option<u64>,
    #[allow(dead_code)]
    pub suggested: Option<u64>,
    pub acceptance_rate: Option<f64>,
}

#[derive(Debug, Deserialize)]
pub struct CopilotUser {
    pub login: Option<String>,
    pub plan: Option<String>,
}

pub fn parse_copilot_response(json_str: &str) -> Result<UsageSnapshot, FetchError> {
    debug!(len = json_str.len(), "Parsing Copilot response");

    let response: CopilotUsageResponse = serde_json::from_str(json_str)
        .map_err(|e| FetchError::InvalidResponse(format!("Invalid JSON: {}", e)))?;

    let mut snapshot = UsageSnapshot::new();
    snapshot.fetch_source = FetchSource::OAuth;

    if let Some(completions) = response.completions {
        // Use acceptance rate as a proxy for "usage"
        if let Some(rate) = completions.acceptance_rate {
            snapshot.primary = Some(UsageWindow::new(rate * 100.0));
        }
    }

    if let Some(user) = response.user {
        let mut identity = ProviderIdentity::new(ProviderKind::Copilot);
        identity.account_email = user.login;
        identity.plan_name = user.plan;
        identity.login_method = Some(LoginMethod::OAuth);
        snapshot.identity = Some(identity);
    }

    Ok(snapshot)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_parse_copilot() {
        let json = r#"{
            "completions": {"accepted": 100, "suggested": 200, "acceptance_rate": 0.5},
            "user": {"login": "octocat", "plan": "pro"}
        }"#;
        let snapshot = parse_copilot_response(json).unwrap();
        assert!(snapshot.primary.is_some());
        assert_eq!(snapshot.primary.unwrap().used_percent, 50.0);
        assert!(snapshot.identity.is_some());
    }

    #[test]
    fn test_parse_empty() {
        let json = r#"{}"#;
        let snapshot = parse_copilot_response(json).unwrap();
        assert!(snapshot.primary.is_none());
    }
}
