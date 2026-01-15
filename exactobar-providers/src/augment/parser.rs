//! Augment response parser.

use exactobar_core::{FetchSource, LoginMethod, ProviderIdentity, ProviderKind, UsageSnapshot, UsageWindow};
use exactobar_fetch::FetchError;
use serde::Deserialize;
use tracing::debug;

#[derive(Debug, Deserialize)]
pub struct AugmentUsageResponse {
    #[serde(default)]
    pub credits: Option<AugmentCredits>,
    #[serde(default)]
    pub user: Option<AugmentUser>,
}

#[derive(Debug, Deserialize)]
pub struct AugmentCredits {
    pub used: Option<f64>,
    pub total: Option<f64>,
    pub monthly_used: Option<f64>,
    pub monthly_total: Option<f64>,
}

#[derive(Debug, Deserialize)]
pub struct AugmentUser {
    pub email: Option<String>,
    pub plan: Option<String>,
}

pub fn parse_augment_response(json_str: &str) -> Result<UsageSnapshot, FetchError> {
    debug!(len = json_str.len(), "Parsing Augment response");

    let response: AugmentUsageResponse = serde_json::from_str(json_str)
        .map_err(|e| FetchError::InvalidResponse(format!("Invalid JSON: {}", e)))?;

    let mut snapshot = UsageSnapshot::new();
    snapshot.fetch_source = FetchSource::Web;

    if let Some(credits) = response.credits {
        if let (Some(used), Some(total)) = (credits.used, credits.total) {
            let percent = if total > 0.0 {
                (used / total) * 100.0
            } else {
                0.0
            };
            snapshot.primary = Some(UsageWindow::new(percent));
        }

        if let (Some(monthly_used), Some(monthly_total)) = (credits.monthly_used, credits.monthly_total) {
            let percent = if monthly_total > 0.0 {
                (monthly_used / monthly_total) * 100.0
            } else {
                0.0
            };
            snapshot.secondary = Some(UsageWindow::new(percent));
        }
    }

    if let Some(user) = response.user {
        let mut identity = ProviderIdentity::new(ProviderKind::Augment);
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
    fn test_parse_augment() {
        let json = r#"{
            "credits": {"used": 25.0, "total": 100.0, "monthly_used": 50.0, "monthly_total": 200.0},
            "user": {"email": "user@example.com"}
        }"#;
        let snapshot = parse_augment_response(json).unwrap();
        assert!(snapshot.primary.is_some());
        assert_eq!(snapshot.primary.unwrap().used_percent, 25.0);
        assert!(snapshot.secondary.is_some());
        assert_eq!(snapshot.secondary.unwrap().used_percent, 25.0);
    }

    #[test]
    fn test_parse_empty() {
        let json = r#"{}"#;
        let snapshot = parse_augment_response(json).unwrap();
        assert!(snapshot.primary.is_none());
    }
}
