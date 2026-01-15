//! Kiro CLI client.
//!
//! Kiro uses `kiro-cli /usage` command for usage information.

use exactobar_core::{
    FetchSource, LoginMethod, ProviderIdentity, ProviderKind, UsageSnapshot, UsageWindow,
};
use regex::Regex;
use std::process::Command;
use std::sync::LazyLock;
use tracing::{debug, instrument, warn};

use super::error::KiroError;

// ============================================================================
// Version Detection
// ============================================================================

/// Detect kiro-cli version.
pub fn detect_version() -> Option<String> {
    let output = Command::new("kiro-cli")
        .args(["--version"])
        .output()
        .ok()?;

    if !output.status.success() {
        return None;
    }

    let version = String::from_utf8_lossy(&output.stdout);
    let trimmed = version.trim();

    // Output is "kiro-cli 1.23.1"
    if trimmed.starts_with("kiro-cli ") {
        Some(trimmed.strip_prefix("kiro-cli ")?.to_string())
    } else if !trimmed.is_empty() {
        Some(trimmed.to_string())
    } else {
        None
    }
}

// ============================================================================
// Login Check
// ============================================================================

/// Check if user is logged in.
pub async fn ensure_logged_in() -> Result<(), KiroError> {
    let output = tokio::process::Command::new("kiro-cli")
        .args(["whoami"])
        .output()
        .await
        .map_err(|_| KiroError::CliNotFound)?;

    let stdout = String::from_utf8_lossy(&output.stdout);
    let stderr = String::from_utf8_lossy(&output.stderr);
    let combined = if stderr.is_empty() {
        stdout.to_string()
    } else {
        stderr.to_string()
    };
    let lower = combined.to_lowercase();

    if lower.contains("not logged in") || lower.contains("login required") {
        warn!("Kiro: user not logged in");
        return Err(KiroError::NotLoggedIn);
    }

    if !output.status.success() {
        return Err(KiroError::CliFailed(combined.trim().to_string()));
    }

    if combined.trim().is_empty() {
        return Err(KiroError::CliFailed("whoami returned empty output".to_string()));
    }

    debug!(output = %combined.trim(), "Kiro login check passed");
    Ok(())
}

// ============================================================================
// Regex Patterns
// ============================================================================

/// Pattern for credits: "Credits: X/Y" or "X credits used"
static CREDITS_RE: LazyLock<Regex> = LazyLock::new(|| {
    Regex::new(r"(?i)credits?:?\s*(\d+(?:\.\d+)?)[\s/]+(\d+(?:\.\d+)?)").expect("Invalid regex")
});

/// Pattern for percentage: "50%" or "50% used"
static PERCENT_RE: LazyLock<Regex> = LazyLock::new(|| {
    Regex::new(r"(\d+(?:\.\d+)?)\s*%").expect("Invalid regex")
});

/// Pattern for email.
static EMAIL_RE: LazyLock<Regex> = LazyLock::new(|| {
    Regex::new(r"[a-zA-Z0-9._%+-]+@[a-zA-Z0-9.-]+\.[a-zA-Z]{2,}").expect("Invalid regex")
});

// ============================================================================
// Parsed Usage
// ============================================================================

/// Usage parsed from CLI output.
#[derive(Debug, Default)]
pub struct KiroUsage {
    /// Credits used.
    pub credits_used: Option<f64>,

    /// Credit limit.
    pub credit_limit: Option<f64>,

    /// Usage percentage.
    pub used_percent: Option<f64>,

    /// User email.
    pub email: Option<String>,

    /// Plan name.
    pub plan: Option<String>,
}

impl KiroUsage {
    /// Check if we have data.
    pub fn has_data(&self) -> bool {
        self.credits_used.is_some()
            || self.used_percent.is_some()
    }

    /// Get usage percentage.
    pub fn get_percent(&self) -> Option<f64> {
        if let Some(percent) = self.used_percent {
            return Some(percent);
        }

        if let (Some(used), Some(limit)) = (self.credits_used, self.credit_limit) {
            if limit > 0.0 {
                return Some((used / limit) * 100.0);
            }
        }

        None
    }

    /// Convert to UsageSnapshot.
    pub fn to_snapshot(&self) -> UsageSnapshot {
        let mut snapshot = UsageSnapshot::new();
        snapshot.fetch_source = FetchSource::CLI;

        if let Some(percent) = self.get_percent() {
            snapshot.primary = Some(UsageWindow::new(percent));
        }

        let mut identity = ProviderIdentity::new(ProviderKind::Kiro);
        identity.account_email = self.email.clone();
        identity.plan_name = self.plan.clone();
        identity.login_method = Some(LoginMethod::CLI);
        snapshot.identity = Some(identity);

        snapshot
    }
}

// ============================================================================
// CLI Client
// ============================================================================

/// Kiro CLI client.
#[derive(Debug, Clone, Default)]
pub struct KiroCliClient;

impl KiroCliClient {
    /// Create a new client.
    pub fn new() -> Self {
        Self
    }

    /// Check if kiro-cli is available.
    pub fn is_available() -> bool {
        which::which("kiro-cli").is_ok() || which::which("kiro").is_ok()
    }

    /// Get the CLI command name.
    fn get_command() -> Option<&'static str> {
        if which::which("kiro-cli").is_ok() {
            Some("kiro-cli")
        } else if which::which("kiro").is_ok() {
            Some("kiro")
        } else {
            None
        }
    }

    /// Fetch usage via CLI.
    #[instrument(skip(self))]
    pub async fn fetch_usage(&self) -> Result<KiroUsage, KiroError> {
        debug!("Fetching Kiro usage via CLI");

        let cmd = Self::get_command().ok_or(KiroError::CliNotFound)?;

        let output = tokio::process::Command::new(cmd)
            .arg("/usage")
            .output()
            .await
            .map_err(|e| KiroError::CliFailed(e.to_string()))?;

        if !output.status.success() {
            // Try without /usage (some versions may use different syntax)
            let output = tokio::process::Command::new(cmd)
                .arg("usage")
                .output()
                .await
                .map_err(|e| KiroError::CliFailed(e.to_string()))?;

            if !output.status.success() {
                let stderr = String::from_utf8_lossy(&output.stderr);
                return Err(KiroError::CliFailed(stderr.to_string()));
            }

            return self.parse_output(&String::from_utf8_lossy(&output.stdout));
        }

        self.parse_output(&String::from_utf8_lossy(&output.stdout))
    }

    /// Parse CLI output.
    fn parse_output(&self, output: &str) -> Result<KiroUsage, KiroError> {
        let mut usage = KiroUsage::default();

        // Try to extract credits
        if let Some(caps) = CREDITS_RE.captures(output) {
            if let (Some(used), Some(limit)) = (caps.get(1), caps.get(2)) {
                usage.credits_used = used.as_str().parse().ok();
                usage.credit_limit = limit.as_str().parse().ok();
            }
        }

        // Try to extract percentage
        if let Some(caps) = PERCENT_RE.captures(output) {
            if let Some(percent) = caps.get(1) {
                usage.used_percent = percent.as_str().parse().ok();
            }
        }

        // Try to extract email
        if let Some(caps) = EMAIL_RE.captures(output) {
            if let Some(email) = caps.get(0) {
                usage.email = Some(email.as_str().to_string());
            }
        }

        if !usage.has_data() {
            return Err(KiroError::NoData);
        }

        Ok(usage)
    }
}

// ============================================================================
// Tests
// ============================================================================

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_client_creation() {
        let _ = KiroCliClient::new();
    }

    #[test]
    fn test_is_available() {
        let _ = KiroCliClient::is_available();
    }

    #[test]
    fn test_parse_credits() {
        let client = KiroCliClient::new();
        let output = "Credits: 500/1000\nPlan: Pro";
        let usage = client.parse_output(output).unwrap();
        assert_eq!(usage.credits_used, Some(500.0));
        assert_eq!(usage.credit_limit, Some(1000.0));
        assert_eq!(usage.get_percent(), Some(50.0));
    }

    #[test]
    fn test_parse_percent() {
        let client = KiroCliClient::new();
        let output = "Usage: 75% used";
        let usage = client.parse_output(output).unwrap();
        assert_eq!(usage.used_percent, Some(75.0));
    }

    #[test]
    fn test_parse_email() {
        let client = KiroCliClient::new();
        let output = "Account: user@example.com\n50% used";
        let usage = client.parse_output(output).unwrap();
        assert_eq!(usage.email, Some("user@example.com".to_string()));
    }

    #[test]
    fn test_usage_has_data() {
        let empty = KiroUsage::default();
        assert!(!empty.has_data());

        let with_credits = KiroUsage {
            credits_used: Some(100.0),
            ..Default::default()
        };
        assert!(with_credits.has_data());
    }
}
