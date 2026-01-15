//! Main Codex usage fetcher.
//!
//! This module provides the primary entry point for fetching Codex usage data.
//! It orchestrates multiple strategies with automatic fallback:
//!
//! 1. **RPC Strategy** (primary): JSON-RPC to `codex app-server`
//! 2. **PTY Strategy** (fallback): Interactive `/status` command
//!
//! # Example
//!
//! ```ignore
//! let fetcher = CodexUsageFetcher::new();
//! let snapshot = fetcher.fetch_usage().await?;
//! println!("Primary: {}% used", snapshot.primary.unwrap().used_percent);
//! ```

use chrono::{DateTime, TimeZone, Utc};
use exactobar_core::{
    Credits, FetchSource, LoginMethod, ProviderIdentity, ProviderKind, UsageSnapshot, UsageWindow,
};
use tracing::{debug, info, instrument, warn};

use super::auth;
use super::error::CodexError;
use super::pty_probe::{CodexPtyProbe, CodexStatusSnapshot};
use super::rpc::{CodexRpcClient, RateLimitsResult};

// ============================================================================
// Fetcher
// ============================================================================

/// Main Codex usage fetcher.
///
/// This fetcher tries multiple strategies in order:
/// 1. JSON-RPC via `codex app-server`
/// 2. PTY with `/status` command
#[derive(Debug, Clone, Default)]
pub struct CodexUsageFetcher {
    /// Whether to skip RPC and go straight to PTY.
    skip_rpc: bool,
    /// Whether to skip PTY fallback.
    skip_pty: bool,
}

impl CodexUsageFetcher {
    /// Create a new fetcher with default settings.
    pub fn new() -> Self {
        Self::default()
    }

    /// Create a fetcher that only uses RPC.
    pub fn rpc_only() -> Self {
        Self {
            skip_rpc: false,
            skip_pty: true,
        }
    }

    /// Create a fetcher that only uses PTY.
    pub fn pty_only() -> Self {
        Self {
            skip_rpc: true,
            skip_pty: false,
        }
    }

    /// Check if codex is available.
    pub fn is_available() -> bool {
        which::which("codex").is_ok()
    }

    /// Detect the installed codex version.
    #[instrument]
    pub fn detect_version() -> Option<String> {
        let output = std::process::Command::new("codex")
            .arg("--version")
            .output()
            .ok()?;

        if !output.status.success() {
            return None;
        }

        let stdout = String::from_utf8_lossy(&output.stdout);
        // Extract version number from output like "codex 1.2.3" or "1.2.3"
        let version = stdout
            .lines()
            .next()?
            .trim()
            .trim_start_matches("codex")
            .trim()
            .to_string();

        if version.is_empty() {
            None
        } else {
            Some(version)
        }
    }

    /// Fetch usage data, trying RPC first then PTY.
    #[instrument(skip(self))]
    pub async fn fetch_usage(&self) -> Result<UsageSnapshot, CodexError> {
        if !Self::is_available() {
            return Err(CodexError::BinaryNotFound("codex".to_string()));
        }

        // Try RPC first
        if !self.skip_rpc {
            match self.fetch_via_rpc().await {
                Ok(snapshot) => {
                    info!(source = "rpc", "Fetched usage via RPC");
                    return Ok(snapshot);
                }
                Err(e) => {
                    warn!(error = %e, "RPC fetch failed, trying PTY fallback");
                }
            }
        }

        // Fallback to PTY
        if !self.skip_pty {
            match self.fetch_via_pty().await {
                Ok(snapshot) => {
                    info!(source = "pty", "Fetched usage via PTY");
                    return Ok(snapshot);
                }
                Err(e) => {
                    warn!(error = %e, "PTY fetch failed");
                }
            }
        }

        Err(CodexError::AllStrategiesFailed)
    }

    /// Fetch credits information.
    #[instrument(skip(self))]
    pub async fn fetch_credits(&self) -> Result<Credits, CodexError> {
        let _snapshot = self.fetch_usage().await?;

        // Try to extract credits from the snapshot
        // For now, credits are embedded in the RPC response
        // We'll need to add a credits field to UsageSnapshot or return separately

        // Check if we stored credits info somewhere
        // For now, return a default
        Ok(Credits::new(0.0))
    }

    /// Fetch using JSON-RPC to app-server.
    #[instrument(skip(self))]
    async fn fetch_via_rpc(&self) -> Result<UsageSnapshot, CodexError> {
        debug!("Attempting RPC fetch");

        // Spawn app-server (this is blocking, so wrap in spawn_blocking)
        let result = tokio::task::spawn_blocking(|| {
            let mut client = CodexRpcClient::spawn()?;
            client.initialize()?;
            let limits = client.fetch_rate_limits()?;
            let account = client.fetch_account().ok();
            client.shutdown();
            Ok::<_, CodexError>((limits, account))
        })
        .await
        .map_err(|e| CodexError::SpawnFailed(format!("Task join error: {}", e)))??;

        let (limits, rpc_account) = result;

        // Convert to UsageSnapshot
        let mut snapshot = convert_rpc_to_snapshot(limits);
        snapshot.fetch_source = FetchSource::CLI;

        // Try to get account info from auth.json (more reliable than RPC)
        let auth_account = auth::try_read_account_info();

        // Build identity from both sources
        let mut identity = ProviderIdentity::new(ProviderKind::Codex);
        identity.login_method = Some(LoginMethod::CLI);

        // Prefer auth.json data, fall back to RPC
        if let Some(auth) = auth_account {
            identity.account_email = auth.email;
            identity.plan_name = auth.plan;
            identity.account_organization = auth.organization;
        }

        // Fill in any missing fields from RPC
        if let Some(account) = rpc_account {
            if identity.account_email.is_none() {
                identity.account_email = account.email;
            }
            if identity.plan_name.is_none() {
                identity.plan_name = account.plan;
            }
            if identity.account_organization.is_none() {
                identity.account_organization = account.organization;
            }
        }

        snapshot.identity = Some(identity);

        Ok(snapshot)
    }

    /// Fetch using PTY with /status command.
    #[instrument(skip(self))]
    async fn fetch_via_pty(&self) -> Result<UsageSnapshot, CodexError> {
        debug!("Attempting PTY fetch");

        let probe = CodexPtyProbe::new();
        let status = probe.fetch_status().await?;

        if !status.has_data() {
            return Err(CodexError::NoData);
        }

        // Convert to UsageSnapshot
        let snapshot = convert_pty_to_snapshot(status);

        Ok(snapshot)
    }
}

// ============================================================================
// Conversion Functions
// ============================================================================

/// Convert RPC rate limits to UsageSnapshot.
fn convert_rpc_to_snapshot(limits: RateLimitsResult) -> UsageSnapshot {
    let mut snapshot = UsageSnapshot::new();

    // Primary window (5-hour)
    if let Some(primary) = limits.rate_limits.primary {
        snapshot.primary = Some(UsageWindow {
            used_percent: primary.used_percent,
            window_minutes: primary.window_duration_mins,
            resets_at: primary.resets_at.map(|ts| timestamp_to_datetime(ts)),
            reset_description: None,
        });
    }

    // Secondary window (weekly)
    if let Some(secondary) = limits.rate_limits.secondary {
        snapshot.secondary = Some(UsageWindow {
            used_percent: secondary.used_percent,
            window_minutes: secondary.window_duration_mins,
            resets_at: secondary.resets_at.map(|ts| timestamp_to_datetime(ts)),
            reset_description: None,
        });
    }

    // Note: Credits are in limits.rate_limits.credits but we don't have
    // a place for them in UsageSnapshot yet. We could add a credits field.

    snapshot
}

/// Convert PTY status to UsageSnapshot.
fn convert_pty_to_snapshot(status: CodexStatusSnapshot) -> UsageSnapshot {
    let mut snapshot = UsageSnapshot::new();
    snapshot.fetch_source = FetchSource::CLI;

    // Primary window
    if let Some(used) = status.primary_used_percent {
        snapshot.primary = Some(UsageWindow::new(used));
    }

    // Secondary window
    if let Some(used) = status.secondary_used_percent {
        snapshot.secondary = Some(UsageWindow::new(used));
    }

    // Build identity from PTY output
    if status.email.is_some() || status.plan.is_some() {
        let mut identity = ProviderIdentity::new(ProviderKind::Codex);
        identity.account_email = status.email;
        identity.plan_name = status.plan;
        identity.login_method = Some(LoginMethod::CLI);
        snapshot.identity = Some(identity);
    }

    snapshot
}

/// Convert Unix timestamp to DateTime<Utc>.
fn timestamp_to_datetime(timestamp: i64) -> DateTime<Utc> {
    Utc.timestamp_opt(timestamp, 0)
        .single()
        .unwrap_or_else(Utc::now)
}

// ============================================================================
// Tests
// ============================================================================

#[cfg(test)]
mod tests {
    use super::*;
    use crate::codex::rpc::{CreditsInfo, RateLimitWindow, RateLimits};

    #[test]
    fn test_convert_rpc_to_snapshot() {
        let limits = RateLimitsResult {
            rate_limits: RateLimits {
                primary: Some(RateLimitWindow {
                    used_percent: 28.5,
                    window_duration_mins: Some(300),
                    resets_at: Some(1735000000),
                }),
                secondary: Some(RateLimitWindow {
                    used_percent: 59.2,
                    window_duration_mins: Some(10080),
                    resets_at: Some(1735100000),
                }),
                credits: Some(CreditsInfo {
                    has_credits: Some(true),
                    unlimited: Some(false),
                    balance: Some("112.45".to_string()),
                }),
            },
        };

        let snapshot = convert_rpc_to_snapshot(limits);

        assert!(snapshot.primary.is_some());
        let primary = snapshot.primary.unwrap();
        assert!((primary.used_percent - 28.5).abs() < 0.01);
        assert_eq!(primary.window_minutes, Some(300));

        assert!(snapshot.secondary.is_some());
        let secondary = snapshot.secondary.unwrap();
        assert!((secondary.used_percent - 59.2).abs() < 0.01);
    }

    #[test]
    fn test_convert_pty_to_snapshot() {
        let status = CodexStatusSnapshot {
            primary_used_percent: Some(28.0),
            secondary_used_percent: Some(55.0),
            credits: Some(112.45),
            email: Some("user@example.com".to_string()),
            plan: Some("Pro".to_string()),
            raw_output: String::new(),
        };

        let snapshot = convert_pty_to_snapshot(status);

        assert!(snapshot.primary.is_some());
        assert!((snapshot.primary.unwrap().used_percent - 28.0).abs() < 0.01);

        assert!(snapshot.secondary.is_some());
        assert!((snapshot.secondary.unwrap().used_percent - 55.0).abs() < 0.01);

        assert!(snapshot.identity.is_some());
        let identity = snapshot.identity.unwrap();
        assert_eq!(identity.account_email, Some("user@example.com".to_string()));
        assert_eq!(identity.plan_name, Some("Pro".to_string()));
    }

    #[test]
    fn test_fetcher_creation() {
        let default = CodexUsageFetcher::new();
        assert!(!default.skip_rpc);
        assert!(!default.skip_pty);

        let rpc_only = CodexUsageFetcher::rpc_only();
        assert!(!rpc_only.skip_rpc);
        assert!(rpc_only.skip_pty);

        let pty_only = CodexUsageFetcher::pty_only();
        assert!(pty_only.skip_rpc);
        assert!(!pty_only.skip_pty);
    }

    #[test]
    fn test_is_available() {
        // Just test the function runs
        let _ = CodexUsageFetcher::is_available();
    }

    #[test]
    fn test_detect_version() {
        // Just test the function runs
        let _ = CodexUsageFetcher::detect_version();
    }

    #[test]
    fn test_timestamp_conversion() {
        let ts = 1735000000_i64; // Some future timestamp
        let dt = timestamp_to_datetime(ts);
        assert_eq!(dt.timestamp(), ts);
    }
}
