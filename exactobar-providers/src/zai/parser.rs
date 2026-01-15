//! Zai response parser.

use exactobar_core::{FetchSource, LoginMethod, ProviderIdentity, ProviderKind, UsageSnapshot, UsageWindow};
use exactobar_fetch::FetchError;
use serde::Deserialize;
use tracing::debug;

#[derive(Debug, Deserialize)]
pub struct ZaiUsageResponse {
    #[serde(default)]
    pub usage: Option<ZaiUsage>,
    #[serde(default)]
    #[allow(dead_code)]
    pub credits: Option<ZaiCredits>,
    #[serde(default)]
    pub account: Option<ZaiAccount>,
}

#[derive(Debug, Deserialize)]
pub struct ZaiUsage {
    pub requests: Option<u64>,
    pub limit: Option<u64>,
}

#[allow(dead_code)]
#[derive(Debug, Deserialize)]
pub struct ZaiCredits {
    pub remaining: Option<f64>,
    pub total: Option<f64>,
}

#[derive(Debug, Deserialize)]
pub struct ZaiAccount {
    pub email: Option<String>,
    pub plan: Option<String>,
}

pub fn parse_zai_response(json_str: &str) -> Result<UsageSnapshot, FetchError> {
    debug!(len = json_str.len(), "Parsing z.ai response");

    let response: ZaiUsageResponse = serde_json::from_str(json_str)
        .map_err(|e| FetchError::InvalidResponse(format!("Invalid JSON: {}", e)))?;

    let mut snapshot = UsageSnapshot::new();
    snapshot.fetch_source = FetchSource::Api;

    if let Some(usage) = response.usage {
        if let (Some(requests), Some(limit)) = (usage.requests, usage.limit) {
            let percent = if limit > 0 {
                (requests as f64 / limit as f64) * 100.0
            } else {
                0.0
            };
            snapshot.primary = Some(UsageWindow::new(percent));
        }
    }

    if let Some(account) = response.account {
        let mut identity = ProviderIdentity::new(ProviderKind::Zai);
        identity.account_email = account.email;
        identity.plan_name = account.plan;
        identity.login_method = Some(LoginMethod::ApiKey);
        snapshot.identity = Some(identity);
    }

    Ok(snapshot)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_parse_zai() {
        let json = r#"{
            "usage": {"requests": 50, "limit": 100},
            "account": {"email": "user@example.com"}
        }"#;
        let snapshot = parse_zai_response(json).unwrap();
        assert!(snapshot.primary.is_some());
        assert_eq!(snapshot.primary.unwrap().used_percent, 50.0);
    }

    #[test]
    fn test_parse_empty() {
        let json = r#"{}"#;
        let snapshot = parse_zai_response(json).unwrap();
        assert!(snapshot.primary.is_none());
    }
}
