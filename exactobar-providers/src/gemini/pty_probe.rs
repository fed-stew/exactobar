//! Gemini PTY probe for CLI fallback.
//!
//! This module provides a fallback method for getting Gemini quota
//! information by running CLI commands.

use exactobar_core::{FetchSource, UsageSnapshot};
use tracing::{debug, instrument};

use super::error::GeminiError;
use super::gcloud::GcloudCredentials;

// ============================================================================
// Parsed Quota
// ============================================================================

/// Quota information parsed from CLI output.
#[derive(Debug, Default)]
pub struct GeminiCliQuota {
    /// Whether Gemini is available.
    pub is_available: bool,

    /// Account email.
    pub account: Option<String>,

    /// Project ID.
    pub project: Option<String>,

    /// Whether AI Studio is configured.
    pub ai_studio_configured: bool,

    /// Whether Vertex AI is enabled.
    pub vertex_ai_enabled: bool,
}

impl GeminiCliQuota {
    /// Check if we have any useful data.
    pub fn has_data(&self) -> bool {
        self.is_available || self.account.is_some() || self.project.is_some()
    }

    /// Convert to UsageSnapshot.
    pub fn to_snapshot(&self) -> UsageSnapshot {
        let mut snapshot = UsageSnapshot::new();
        snapshot.fetch_source = FetchSource::CLI;

        if let Some(ref account) = self.account {
            let mut identity =
                exactobar_core::ProviderIdentity::new(exactobar_core::ProviderKind::Gemini);
            identity.account_email = Some(account.clone());
            identity.account_organization = self.project.clone();
            identity.login_method = Some(exactobar_core::LoginMethod::CLI);

            if self.ai_studio_configured {
                identity.plan_name = Some("AI Studio".to_string());
            } else if self.vertex_ai_enabled {
                identity.plan_name = Some("Vertex AI".to_string());
            }

            snapshot.identity = Some(identity);
        }

        snapshot
    }
}

// ============================================================================
// PTY Probe
// ============================================================================

/// Gemini PTY probe for CLI fallback.
#[derive(Debug, Clone, Default)]
pub struct GeminiPtyProbe;

impl GeminiPtyProbe {
    /// Creates a new PTY probe.
    pub fn new() -> Self {
        Self
    }

    /// Check if gcloud is available.
    pub fn is_available() -> bool {
        GcloudCredentials::is_cli_available()
    }

    /// Fetch quota information via CLI.
    #[instrument(skip(self))]
    pub async fn fetch_quota(&self) -> Result<GeminiCliQuota, GeminiError> {
        debug!("Fetching Gemini quota via CLI");

        if !Self::is_available() {
            return Err(GeminiError::GcloudNotFound);
        }

        let mut quota = GeminiCliQuota::default();

        // Get current account
        if let Ok(output) = tokio::process::Command::new("gcloud")
            .args(["config", "get-value", "account"])
            .output()
            .await
        {
            if output.status.success() {
                let account = String::from_utf8_lossy(&output.stdout).trim().to_string();
                if !account.is_empty() && account != "(unset)" {
                    quota.account = Some(account);
                }
            }
        }

        // Get current project
        if let Ok(output) = tokio::process::Command::new("gcloud")
            .args(["config", "get-value", "project"])
            .output()
            .await
        {
            if output.status.success() {
                let project = String::from_utf8_lossy(&output.stdout).trim().to_string();
                if !project.is_empty() && project != "(unset)" {
                    quota.project = Some(project.clone());

                    // Check if AI Platform API is enabled
                    if let Ok(api_output) = tokio::process::Command::new("gcloud")
                        .args([
                            "services",
                            "list",
                            "--enabled",
                            "--filter=name:aiplatform.googleapis.com",
                            "--format=value(name)",
                            &format!("--project={}", project),
                        ])
                        .output()
                        .await
                    {
                        if api_output.status.success() {
                            let apis = String::from_utf8_lossy(&api_output.stdout);
                            quota.vertex_ai_enabled = apis.contains("aiplatform");
                        }
                    }
                }
            }
        }

        // Check for AI Studio by trying to get an access token
        // If we can get a token, assume AI Studio is configured
        if let Ok(output) = tokio::process::Command::new("gcloud")
            .args(["auth", "print-access-token"])
            .output()
            .await
        {
            if output.status.success() {
                let token = String::from_utf8_lossy(&output.stdout).trim().to_string();
                if !token.is_empty() {
                    quota.is_available = true;
                    quota.ai_studio_configured = true;
                }
            }
        }

        if !quota.has_data() {
            return Err(GeminiError::NoData);
        }

        Ok(quota)
    }

    /// Check if user is logged in to gcloud.
    #[instrument(skip(self))]
    pub async fn is_logged_in(&self) -> bool {
        if !Self::is_available() {
            return false;
        }

        let output = tokio::process::Command::new("gcloud")
            .args(["auth", "list", "--filter=status:ACTIVE", "--format=value(account)"])
            .output()
            .await;

        output.is_ok_and(|o| o.status.success() && !o.stdout.is_empty())
    }

    /// Get the active gcloud account.
    #[instrument(skip(self))]
    pub async fn get_active_account(&self) -> Option<String> {
        if !Self::is_available() {
            return None;
        }

        let output = tokio::process::Command::new("gcloud")
            .args(["config", "get-value", "account"])
            .output()
            .await
            .ok()?;

        if output.status.success() {
            let account = String::from_utf8_lossy(&output.stdout).trim().to_string();
            if !account.is_empty() && account != "(unset)" {
                return Some(account);
            }
        }

        None
    }
}

// ============================================================================
// Tests
// ============================================================================

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_probe_creation() {
        let probe = GeminiPtyProbe::new();
        assert!(std::mem::size_of_val(&probe) == 0); // Zero-sized type
    }

    #[test]
    fn test_is_available() {
        // Just test the function runs
        let _ = GeminiPtyProbe::is_available();
    }

    #[test]
    fn test_cli_quota_has_data() {
        let empty = GeminiCliQuota::default();
        assert!(!empty.has_data());

        let with_account = GeminiCliQuota {
            account: Some("user@example.com".to_string()),
            ..Default::default()
        };
        assert!(with_account.has_data());

        let available = GeminiCliQuota {
            is_available: true,
            ..Default::default()
        };
        assert!(available.has_data());
    }

    #[test]
    fn test_cli_quota_to_snapshot() {
        let quota = GeminiCliQuota {
            is_available: true,
            account: Some("user@example.com".to_string()),
            project: Some("my-project".to_string()),
            ai_studio_configured: true,
            vertex_ai_enabled: false,
        };

        let snapshot = quota.to_snapshot();
        assert!(snapshot.identity.is_some());

        let identity = snapshot.identity.unwrap();
        assert_eq!(identity.account_email, Some("user@example.com".to_string()));
        assert_eq!(identity.plan_name, Some("AI Studio".to_string()));
    }
}
