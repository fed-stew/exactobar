//! MiniMax response parser.

use exactobar_core::{FetchSource, LoginMethod, ProviderIdentity, ProviderKind, UsageSnapshot, UsageWindow};
use exactobar_fetch::FetchError;
use serde::Deserialize;
use tracing::debug;

#[derive(Debug, Deserialize)]
pub struct MiniMaxUsageResponse {
    #[serde(default)]
    pub tokens: Option<MiniMaxTokens>,
    #[serde(default)]
    pub credits: Option<MiniMaxCredits>,
    #[serde(default)]
    pub user: Option<MiniMaxUser>,
}

#[derive(Debug, Deserialize)]
pub struct MiniMaxTokens {
    pub used: Option<u64>,
    pub limit: Option<u64>,
}

#[derive(Debug, Deserialize)]
pub struct MiniMaxCredits {
    pub used: Option<f64>,
    pub total: Option<f64>,
}

#[derive(Debug, Deserialize)]
pub struct MiniMaxUser {
    pub email: Option<String>,
    pub plan: Option<String>,
}

pub fn parse_minimax_response(json_str: &str) -> Result<UsageSnapshot, FetchError> {
    debug!(len = json_str.len(), "Parsing MiniMax response");

    let response: MiniMaxUsageResponse = serde_json::from_str(json_str)
        .map_err(|e| FetchError::InvalidResponse(format!("Invalid JSON: {}", e)))?;

    let mut snapshot = UsageSnapshot::new();
    snapshot.fetch_source = FetchSource::Web;

    // Primary: token usage
    if let Some(tokens) = response.tokens {
        if let (Some(used), Some(limit)) = (tokens.used, tokens.limit) {
            let percent = if limit > 0 {
                (used as f64 / limit as f64) * 100.0
            } else {
                0.0
            };
            snapshot.primary = Some(UsageWindow::new(percent));
        }
    }

    // Secondary: credit usage
    if let Some(credits) = response.credits {
        if let (Some(used), Some(total)) = (credits.used, credits.total) {
            let percent = if total > 0.0 {
                (used / total) * 100.0
            } else {
                0.0
            };
            snapshot.secondary = Some(UsageWindow::new(percent));
        }
    }

    if let Some(user) = response.user {
        let mut identity = ProviderIdentity::new(ProviderKind::MiniMax);
        identity.account_email = user.email;
        identity.plan_name = user.plan;
        identity.login_method = Some(LoginMethod::BrowserCookies);
        snapshot.identity = Some(identity);
    }

    Ok(snapshot)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_parse_minimax() {
        let json = r#"{
            "tokens": {"used": 5000, "limit": 10000},
            "credits": {"used": 25.0, "total": 100.0},
            "user": {"email": "user@example.com"}
        }"#;
        let snapshot = parse_minimax_response(json).unwrap();
        assert!(snapshot.primary.is_some());
        assert_eq!(snapshot.primary.unwrap().used_percent, 50.0);
        assert!(snapshot.secondary.is_some());
        assert_eq!(snapshot.secondary.unwrap().used_percent, 25.0);
    }

    #[test]
    fn test_parse_empty() {
        let json = r#"{}"#;
        let snapshot = parse_minimax_response(json).unwrap();
        assert!(snapshot.primary.is_none());
    }
}
