//! Kiro response parser.

use chrono::{DateTime, Utc};
use exactobar_core::{
    FetchSource, LoginMethod, ProviderIdentity, ProviderKind, UsageSnapshot, UsageWindow,
};
use exactobar_fetch::FetchError;
use serde::Deserialize;
use tracing::debug;

// ============================================================================
// JSON Response Structs
// ============================================================================

/// Kiro usage response from CLI.
#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct KiroUsageResponse {
    /// Plan name (e.g., "Pro", "Free").
    pub plan_name: Option<String>,

    /// Credits used.
    pub credits_used: Option<f64>,

    /// Total credits available.
    pub credits_total: Option<f64>,

    /// Bonus credits used.
    pub bonus_credits_used: Option<f64>,

    /// Total bonus credits available.
    pub bonus_credits_total: Option<f64>,

    /// Days until bonus credits expire.
    pub bonus_expiry_days: Option<i32>,

    /// When credits reset (ISO 8601).
    pub resets_at: Option<String>,

    /// User info.
    #[serde(default)]
    pub user: Option<KiroUser>,

    /// Nested credits object (alternative format).
    #[serde(default)]
    pub credits: Option<KiroCredits>,
}

/// Nested credits object (alternative JSON format).
#[derive(Debug, Deserialize)]
pub struct KiroCredits {
    pub used: Option<f64>,
    pub total: Option<f64>,
    pub monthly_used: Option<f64>,
    pub monthly_total: Option<f64>,
}

/// User info from response.
#[derive(Debug, Deserialize)]
pub struct KiroUser {
    pub email: Option<String>,
    pub plan: Option<String>,
}

impl KiroUsageResponse {
    /// Calculate credits percentage.
    pub fn credits_percent(&self) -> Option<f64> {
        // Try direct fields first
        if let (Some(used), Some(total)) = (self.credits_used, self.credits_total) {
            if total > 0.0 {
                return Some((used / total) * 100.0);
            }
        }

        // Try nested credits object
        if let Some(credits) = &self.credits {
            if let (Some(used), Some(total)) = (credits.used, credits.total) {
                if total > 0.0 {
                    return Some((used / total) * 100.0);
                }
            }
        }

        None
    }

    /// Calculate bonus credits percentage.
    pub fn bonus_credits_percent(&self) -> Option<f64> {
        if let (Some(used), Some(total)) = (self.bonus_credits_used, self.bonus_credits_total) {
            if total > 0.0 {
                return Some((used / total) * 100.0);
            }
        }
        None
    }

    /// Convert to UsageSnapshot.
    pub fn to_snapshot(&self) -> UsageSnapshot {
        let mut snapshot = UsageSnapshot::new();
        snapshot.fetch_source = FetchSource::CLI;

        // Primary: regular credits
        if let Some(percent) = self.credits_percent() {
            let mut window = UsageWindow::new(percent);

            // Parse reset time if available
            window.resets_at = self
                .resets_at
                .as_ref()
                .and_then(|s| DateTime::parse_from_rfc3339(s).ok())
                .map(|dt| dt.with_timezone(&Utc));

            snapshot.primary = Some(window);
        }

        // Secondary: bonus credits (if any)
        if let Some(bonus_percent) = self.bonus_credits_percent() {
            let expiry = self
                .bonus_expiry_days
                .map(|days| Utc::now() + chrono::Duration::days(days as i64));

            let mut window = UsageWindow::new(bonus_percent);
            window.resets_at = expiry;
            window.reset_description = self
                .bonus_expiry_days
                .map(|d| format!("expires in {}d", d));

            snapshot.secondary = Some(window);
        } else if let Some(credits) = &self.credits {
            // Try nested monthly credits as secondary
            if let (Some(monthly_used), Some(monthly_total)) =
                (credits.monthly_used, credits.monthly_total)
            {
                if monthly_total > 0.0 {
                    let percent = (monthly_used / monthly_total) * 100.0;
                    snapshot.secondary = Some(UsageWindow::new(percent));
                }
            }
        }

        // Identity
        let mut identity = ProviderIdentity::new(ProviderKind::Kiro);

        // Get plan name from direct field or nested user
        let plan = self
            .plan_name
            .clone()
            .or_else(|| self.user.as_ref().and_then(|u| u.plan.clone()));

        identity.plan_name = plan;
        identity.login_method = Some(LoginMethod::CLI);

        // Get email if available
        identity.account_email = self.user.as_ref().and_then(|u| u.email.clone());

        snapshot.identity = Some(identity);

        snapshot
    }
}

// ============================================================================
// Public Parser
// ============================================================================

/// Parse Kiro JSON response into UsageSnapshot.
pub fn parse_kiro_response(json_str: &str) -> Result<UsageSnapshot, FetchError> {
    debug!(len = json_str.len(), "Parsing Kiro response");

    let response: KiroUsageResponse = serde_json::from_str(json_str)
        .map_err(|e| FetchError::InvalidResponse(format!("Invalid JSON: {}", e)))?;

    Ok(response.to_snapshot())
}

// ============================================================================
// Tests
// ============================================================================

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_parse_kiro_flat_format() {
        let json = r#"{
            "planName": "Pro",
            "creditsUsed": 40.0,
            "creditsTotal": 100.0
        }"#;
        let snapshot = parse_kiro_response(json).unwrap();
        assert!(snapshot.primary.is_some());
        assert_eq!(snapshot.primary.unwrap().used_percent, 40.0);
    }

    #[test]
    fn test_parse_kiro_nested_format() {
        let json = r#"{
            "credits": {"used": 40.0, "total": 100.0},
            "user": {"email": "user@example.com", "plan": "Free"}
        }"#;
        let snapshot = parse_kiro_response(json).unwrap();
        assert!(snapshot.primary.is_some());
        assert_eq!(snapshot.primary.unwrap().used_percent, 40.0);
        assert!(snapshot.identity.is_some());
        let identity = snapshot.identity.unwrap();
        assert_eq!(identity.account_email, Some("user@example.com".to_string()));
    }

    #[test]
    fn test_parse_with_bonus_credits() {
        let json = r#"{
            "creditsUsed": 50.0,
            "creditsTotal": 100.0,
            "bonusCreditsUsed": 10.0,
            "bonusCreditsTotal": 50.0,
            "bonusExpiryDays": 7
        }"#;
        let snapshot = parse_kiro_response(json).unwrap();
        assert!(snapshot.primary.is_some());
        assert_eq!(snapshot.primary.as_ref().unwrap().used_percent, 50.0);
        assert!(snapshot.secondary.is_some());
        assert_eq!(snapshot.secondary.as_ref().unwrap().used_percent, 20.0);
        assert!(snapshot
            .secondary
            .as_ref()
            .unwrap()
            .reset_description
            .is_some());
    }

    #[test]
    fn test_parse_with_reset_time() {
        let json = r#"{
            "creditsUsed": 25.0,
            "creditsTotal": 100.0,
            "resetsAt": "2025-02-01T00:00:00Z"
        }"#;
        let snapshot = parse_kiro_response(json).unwrap();
        assert!(snapshot.primary.is_some());
        let primary = snapshot.primary.unwrap();
        assert_eq!(primary.used_percent, 25.0);
        assert!(primary.resets_at.is_some());
    }

    #[test]
    fn test_parse_empty() {
        let json = r#"{}"#;
        let snapshot = parse_kiro_response(json).unwrap();
        assert!(snapshot.primary.is_none());
    }
}
