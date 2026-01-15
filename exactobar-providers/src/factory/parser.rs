//! Factory response parser.

use exactobar_core::{FetchSource, LoginMethod, ProviderIdentity, ProviderKind, UsageSnapshot, UsageWindow};
use exactobar_fetch::FetchError;
use serde::Deserialize;
use tracing::debug;

#[derive(Debug, Deserialize)]
pub struct FactoryUsageResponse {
    #[serde(default)]
    pub usage: Option<FactoryUsage>,
    #[serde(default)]
    #[allow(dead_code)]
    pub credits: Option<FactoryCredits>,
    #[serde(default)]
    pub user: Option<FactoryUser>,
}

#[derive(Debug, Deserialize)]
pub struct FactoryUsage {
    pub session_percent: Option<f64>,
    pub monthly_percent: Option<f64>,
}

#[allow(dead_code)]
#[derive(Debug, Deserialize)]
pub struct FactoryCredits {
    pub remaining: Option<f64>,
    pub total: Option<f64>,
}

#[derive(Debug, Deserialize)]
pub struct FactoryUser {
    pub email: Option<String>,
    pub plan: Option<String>,
}

pub fn parse_factory_response(json_str: &str) -> Result<UsageSnapshot, FetchError> {
    debug!(len = json_str.len(), "Parsing Factory response");

    let response: FactoryUsageResponse = serde_json::from_str(json_str)
        .map_err(|e| FetchError::InvalidResponse(format!("Invalid JSON: {}", e)))?;

    let mut snapshot = UsageSnapshot::new();
    snapshot.fetch_source = FetchSource::Web;

    if let Some(usage) = response.usage {
        if let Some(session) = usage.session_percent {
            snapshot.primary = Some(UsageWindow::new(session));
        }
        if let Some(monthly) = usage.monthly_percent {
            snapshot.secondary = Some(UsageWindow::new(monthly));
        }
    }

    if let Some(user) = response.user {
        let mut identity = ProviderIdentity::new(ProviderKind::Factory);
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
    fn test_parse_factory() {
        let json = r#"{
            "usage": {"session_percent": 30.0, "monthly_percent": 15.0},
            "user": {"email": "user@example.com", "plan": "pro"}
        }"#;
        let snapshot = parse_factory_response(json).unwrap();
        assert!(snapshot.primary.is_some());
        assert_eq!(snapshot.primary.unwrap().used_percent, 30.0);
        assert!(snapshot.secondary.is_some());
    }

    #[test]
    fn test_parse_empty() {
        let json = r#"{}"#;
        let snapshot = parse_factory_response(json).unwrap();
        assert!(snapshot.primary.is_none());
    }
}
