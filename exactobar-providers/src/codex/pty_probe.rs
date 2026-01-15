//! PTY-based Codex status probe.
//!
//! This module provides a fallback mechanism for fetching Codex usage
//! by running the `codex` CLI interactively and parsing the `/status` output.
//!
//! # Usage
//!
//! ```ignore
//! let probe = CodexPtyProbe::new();
//! let snapshot = probe.fetch_status().await?;
//! ```
//!
//! # Output Format
//!
//! The `/status` command outputs something like:
//! ```text
//! Account: user@example.com
//! Plan: Pro
//! 5h limit: 72% left
//! Weekly limit: 45% left
//! Credits: $112.45
//! ```

use exactobar_fetch::host::pty::{PtyOptions, PtyRunner};
use regex::Regex;
use std::sync::LazyLock;
use std::time::Duration;
use tracing::{debug, instrument, warn};

use super::error::CodexError;

// ============================================================================
// Constants
// ============================================================================

/// Codex binary name.
const CODEX_BINARY: &str = "codex";

/// Timeout for the PTY operation.
const PTY_TIMEOUT: Duration = Duration::from_secs(30);

/// Idle timeout (when to stop waiting for more output).
const IDLE_TIMEOUT: Duration = Duration::from_secs(5);

/// Patterns that indicate we should stop reading.
const STOP_PATTERNS: &[&str] = &[
    "Credits:",
    "limit:", // After seeing limit info
    "Error:",
    "error:",
];

// ============================================================================
// Regex Patterns
// ============================================================================

/// Pattern for "5h limit: XX% left" or "Session: XX% left"
static PERCENT_LEFT_RE: LazyLock<Regex> = LazyLock::new(|| {
    Regex::new(r"(?i)(\d+h|session|weekly|daily)\s*(?:limit)?\s*:\s*(\d+(?:\.\d+)?)%\s*left")
        .expect("Invalid regex")
});

/// Pattern for "XX% used" style
static PERCENT_USED_RE: LazyLock<Regex> = LazyLock::new(|| {
    Regex::new(r"(?i)(\d+h|session|weekly|daily)\s*(?:limit)?\s*:\s*(\d+(?:\.\d+)?)%\s*used")
        .expect("Invalid regex")
});

/// Pattern for credits "Credits: $XX.XX" or "Credits: XX.XX"
static CREDITS_RE: LazyLock<Regex> = LazyLock::new(|| {
    Regex::new(r"(?i)credits\s*:\s*\$?(\d+(?:\.\d+)?)")
        .expect("Invalid regex")
});

/// Pattern for account email.
static EMAIL_RE: LazyLock<Regex> = LazyLock::new(|| {
    Regex::new(r"(?i)(?:account|email)\s*:\s*([^\s]+@[^\s]+)")
        .expect("Invalid regex")
});

/// Pattern for plan type.
static PLAN_RE: LazyLock<Regex> = LazyLock::new(|| {
    Regex::new(r"(?i)plan\s*:\s*(\w+)")
        .expect("Invalid regex")
});

// ============================================================================
// Status Snapshot
// ============================================================================

/// Parsed status from the /status command.
#[derive(Debug, Clone, Default)]
pub struct CodexStatusSnapshot {
    /// Primary window (5h/session) - percentage USED (not left).
    pub primary_used_percent: Option<f64>,
    /// Secondary window (weekly) - percentage USED (not left).
    pub secondary_used_percent: Option<f64>,
    /// Credit balance.
    pub credits: Option<f64>,
    /// Account email.
    pub email: Option<String>,
    /// Plan name.
    pub plan: Option<String>,
    /// Raw output for debugging.
    pub raw_output: String,
}

impl CodexStatusSnapshot {
    /// Returns true if we have any usage data.
    pub fn has_data(&self) -> bool {
        self.primary_used_percent.is_some()
            || self.secondary_used_percent.is_some()
            || self.credits.is_some()
    }
}

// ============================================================================
// PTY Probe
// ============================================================================

/// PTY-based probe for fetching Codex status.
#[derive(Debug, Clone, Default)]
pub struct CodexPtyProbe {
    runner: PtyRunner,
}

impl CodexPtyProbe {
    /// Create a new PTY probe.
    pub fn new() -> Self {
        Self {
            runner: PtyRunner::new(120, 40),
        }
    }

    /// Check if codex is available.
    pub fn is_available() -> bool {
        PtyRunner::exists(CODEX_BINARY)
    }

    /// Fetch status using the /status command.
    #[instrument(skip(self))]
    pub async fn fetch_status(&self) -> Result<CodexStatusSnapshot, CodexError> {
        if !Self::is_available() {
            return Err(CodexError::BinaryNotFound(CODEX_BINARY.to_string()));
        }

        debug!("Fetching status via PTY");

        let options = PtyOptions::with_timeout(PTY_TIMEOUT)
            .with_idle_timeout(IDLE_TIMEOUT)
            .stop_on_any(STOP_PATTERNS.iter().copied())
            .with_env("TERM", "xterm-256color")
            .with_env("NO_COLOR", "1"); // Try to disable colors

        // Send /status command followed by exit
        let input = "/status\nexit\n";

        let result = self.runner.run(CODEX_BINARY, input, options).await?;

        debug!(
            output_len = result.output.len(),
            exit_code = ?result.exit_code,
            timed_out = result.timed_out,
            "PTY command completed"
        );

        // Parse the output
        let snapshot = parse_status_output(&result.output)?;

        if !snapshot.has_data() && !result.timed_out {
            warn!("No usage data found in output");
            // Still return the snapshot - it might have partial data
        }

        Ok(snapshot)
    }
}

// ============================================================================
// Parser Functions
// ============================================================================

/// Parse the /status command output into a snapshot.
#[instrument(skip(text))]
pub fn parse_status_output(text: &str) -> Result<CodexStatusSnapshot, CodexError> {
    let mut snapshot = CodexStatusSnapshot {
        raw_output: text.to_string(),
        ..Default::default()
    };

    // Parse line by line
    for line in text.lines() {
        let line = line.trim();
        if line.is_empty() {
            continue;
        }

        // Try to parse percent left patterns
        if let Some((window_type, percent_left)) = parse_percent_left(line) {
            let used_percent = 100.0 - percent_left;
            match window_type.to_lowercase().as_str() {
                s if s.contains("5h") || s.contains("session") => {
                    snapshot.primary_used_percent = Some(used_percent);
                    debug!(window = %window_type, left = percent_left, used = used_percent, "Parsed primary window");
                }
                s if s.contains("week") || s.contains("7d") => {
                    snapshot.secondary_used_percent = Some(used_percent);
                    debug!(window = %window_type, left = percent_left, used = used_percent, "Parsed secondary window");
                }
                s if s.contains("daily") || s.contains("24h") => {
                    // Some versions might show daily - treat as primary
                    if snapshot.primary_used_percent.is_none() {
                        snapshot.primary_used_percent = Some(used_percent);
                    }
                }
                _ => {
                    debug!(window = %window_type, "Unknown window type");
                }
            }
            continue;
        }

        // Try to parse percent used patterns
        if let Some((window_type, percent_used)) = parse_percent_used(line) {
            match window_type.to_lowercase().as_str() {
                s if s.contains("5h") || s.contains("session") => {
                    snapshot.primary_used_percent = Some(percent_used);
                }
                s if s.contains("week") || s.contains("7d") => {
                    snapshot.secondary_used_percent = Some(percent_used);
                }
                _ => {}
            }
            continue;
        }

        // Try to parse credits
        if let Some(credits) = parse_credits(line) {
            snapshot.credits = Some(credits);
            debug!(credits = credits, "Parsed credits");
            continue;
        }

        // Try to parse email
        if let Some(email) = parse_email(line) {
            snapshot.email = Some(email);
            continue;
        }

        // Try to parse plan
        if let Some(plan) = parse_plan(line) {
            snapshot.plan = Some(plan);
            continue;
        }
    }

    Ok(snapshot)
}

/// Parse "XX% left" pattern.
/// Returns (window_type, percent_left)
pub fn parse_percent_left(line: &str) -> Option<(String, f64)> {
    PERCENT_LEFT_RE.captures(line).and_then(|caps| {
        let window_type = caps.get(1)?.as_str().to_string();
        let percent: f64 = caps.get(2)?.as_str().parse().ok()?;
        Some((window_type, percent))
    })
}

/// Parse "XX% used" pattern.
/// Returns (window_type, percent_used)
pub fn parse_percent_used(line: &str) -> Option<(String, f64)> {
    PERCENT_USED_RE.captures(line).and_then(|caps| {
        let window_type = caps.get(1)?.as_str().to_string();
        let percent: f64 = caps.get(2)?.as_str().parse().ok()?;
        Some((window_type, percent))
    })
}

/// Parse credits line.
pub fn parse_credits(line: &str) -> Option<f64> {
    CREDITS_RE.captures(line).and_then(|caps| {
        caps.get(1)?.as_str().parse().ok()
    })
}

/// Parse email from line.
pub fn parse_email(line: &str) -> Option<String> {
    EMAIL_RE.captures(line).and_then(|caps| {
        Some(caps.get(1)?.as_str().to_string())
    })
}

/// Parse plan from line.
pub fn parse_plan(line: &str) -> Option<String> {
    PLAN_RE.captures(line).and_then(|caps| {
        Some(caps.get(1)?.as_str().to_string())
    })
}

// ============================================================================
// Tests
// ============================================================================

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_parse_percent_left_5h() {
        let line = "5h limit: 72% left";
        let (window, pct) = parse_percent_left(line).unwrap();
        assert_eq!(window, "5h");
        assert!((pct - 72.0).abs() < 0.01);
    }

    #[test]
    fn test_parse_percent_left_weekly() {
        let line = "Weekly limit: 45.5% left";
        let (window, pct) = parse_percent_left(line).unwrap();
        assert!(window.to_lowercase().contains("weekly"));
        assert!((pct - 45.5).abs() < 0.01);
    }

    #[test]
    fn test_parse_percent_left_session() {
        let line = "Session: 80% left";
        let (window, pct) = parse_percent_left(line).unwrap();
        assert!(window.to_lowercase().contains("session"));
        assert!((pct - 80.0).abs() < 0.01);
    }

    #[test]
    fn test_parse_credits() {
        assert!((parse_credits("Credits: $112.45").unwrap() - 112.45).abs() < 0.01);
        assert!((parse_credits("Credits: 99.99").unwrap() - 99.99).abs() < 0.01);
        assert!((parse_credits("credits: $0.50").unwrap() - 0.50).abs() < 0.01);
    }

    #[test]
    fn test_parse_email() {
        assert_eq!(
            parse_email("Account: user@example.com"),
            Some("user@example.com".to_string())
        );
        assert_eq!(
            parse_email("Email: test@test.org"),
            Some("test@test.org".to_string())
        );
    }

    #[test]
    fn test_parse_plan() {
        assert_eq!(parse_plan("Plan: Pro"), Some("Pro".to_string()));
        assert_eq!(parse_plan("plan: plus"), Some("plus".to_string()));
    }

    #[test]
    fn test_parse_status_output_full() {
        let output = r#"
            Welcome to Codex!
            Account: user@example.com
            Plan: Pro
            5h limit: 72% left
            Weekly limit: 45% left
            Credits: $112.45
        "#;

        let snapshot = parse_status_output(output).unwrap();

        assert!(snapshot.has_data());
        assert!((snapshot.primary_used_percent.unwrap() - 28.0).abs() < 0.01); // 100 - 72
        assert!((snapshot.secondary_used_percent.unwrap() - 55.0).abs() < 0.01); // 100 - 45
        assert!((snapshot.credits.unwrap() - 112.45).abs() < 0.01);
        assert_eq!(snapshot.email, Some("user@example.com".to_string()));
        assert_eq!(snapshot.plan, Some("Pro".to_string()));
    }

    #[test]
    fn test_parse_status_output_partial() {
        let output = "5h limit: 50% left";
        let snapshot = parse_status_output(output).unwrap();

        assert!(snapshot.has_data());
        assert!((snapshot.primary_used_percent.unwrap() - 50.0).abs() < 0.01);
        assert!(snapshot.secondary_used_percent.is_none());
        assert!(snapshot.credits.is_none());
    }

    #[test]
    fn test_parse_status_output_empty() {
        let output = "Some random text with no data";
        let snapshot = parse_status_output(output).unwrap();
        assert!(!snapshot.has_data());
    }

    #[test]
    fn test_parse_status_with_ansi_stripped() {
        // Simulating output after ANSI stripping
        let output = "Session: 25% left\nWeekly: 60% left";
        let snapshot = parse_status_output(output).unwrap();

        assert!(snapshot.has_data());
        assert!((snapshot.primary_used_percent.unwrap() - 75.0).abs() < 0.01);
        assert!((snapshot.secondary_used_percent.unwrap() - 40.0).abs() < 0.01);
    }

    #[test]
    fn test_is_available() {
        // This just tests the function exists and runs
        let _ = CodexPtyProbe::is_available();
    }
}
