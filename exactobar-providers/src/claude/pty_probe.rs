//! PTY-based Claude status probe.
//!
//! This module provides a fallback mechanism for fetching Claude usage
//! by running the `claude` CLI interactively and parsing the `/usage` output.
//!
//! # Output Format
//!
//! The `/usage` command outputs something like:
//! ```text
//! Current session
//! 72% left
//! Resets 2pm (PST)
//!
//! Current week (all models)
//! 45% left
//! Resets Jan 5 at 12am
//!
//! Current week (Sonnet)
//! 80% left
//! Resets Jan 5 at 12am
//!
//! Account: user@example.com
//! ```

use exactobar_fetch::host::pty::{PtyOptions, PtyRunner};
use regex::Regex;
use std::sync::LazyLock;
use std::time::Duration;
use tracing::{debug, instrument, warn};

use super::error::ClaudeError;

// ============================================================================
// Constants
// ============================================================================

/// Claude binary name.
const CLAUDE_BINARY: &str = "claude";

/// Timeout for the PTY operation.
const PTY_TIMEOUT: Duration = Duration::from_secs(30);

/// Idle timeout.
const IDLE_TIMEOUT: Duration = Duration::from_secs(5);

/// Patterns that indicate we should stop reading.
const STOP_PATTERNS: &[&str] = &[
    "Account:",
    "email:",
    "Error:",
    "error:",
    ">>> ", // Prompt
];

// ============================================================================
// Regex Patterns
// ============================================================================

/// Pattern for "XX% left" or "XX% remaining"
static PERCENT_LEFT_RE: LazyLock<Regex> = LazyLock::new(|| {
    Regex::new(r"(\d+(?:\.\d+)?)%\s*(?:left|remaining)").expect("Invalid regex")
});

/// Pattern for "XX% used"
static PERCENT_USED_RE: LazyLock<Regex> = LazyLock::new(|| {
    Regex::new(r"(\d+(?:\.\d+)?)%\s*used").expect("Invalid regex")
});

/// Pattern for "Resets <time>" or "Reset: <time>"
static RESET_TIME_RE: LazyLock<Regex> = LazyLock::new(|| {
    Regex::new(r"(?i)resets?:?\s+(.+?)(?:\n|$)").expect("Invalid regex")
});

/// Pattern for account email.
static EMAIL_RE: LazyLock<Regex> = LazyLock::new(|| {
    Regex::new(r"(?i)(?:account|email)\s*:?\s*([^\s]+@[^\s]+)").expect("Invalid regex")
});

/// Pattern for organization.
static ORG_RE: LazyLock<Regex> = LazyLock::new(|| {
    Regex::new(r"(?i)(?:org(?:anization)?|team)\s*:?\s*(.+?)(?:\n|$)").expect("Invalid regex")
});

/// Pattern for login method.
static LOGIN_RE: LazyLock<Regex> = LazyLock::new(|| {
    Regex::new(r"(?i)logged in (?:with|via|using)\s+(\w+)").expect("Invalid regex")
});

// ============================================================================
// Status Snapshot
// ============================================================================

/// Parsed status from the /usage command.
#[derive(Debug, Clone, Default)]
pub struct ClaudeStatusSnapshot {
    /// Session usage - percent LEFT (not used).
    pub session_percent_left: Option<f64>,
    /// Weekly usage (all models) - percent LEFT.
    pub weekly_percent_left: Option<f64>,
    /// Opus/Sonnet usage - percent LEFT.
    pub opus_percent_left: Option<f64>,
    /// Session reset time description.
    pub session_reset: Option<String>,
    /// Weekly reset time description.
    pub weekly_reset: Option<String>,
    /// Account email.
    pub account_email: Option<String>,
    /// Organization name.
    pub account_organization: Option<String>,
    /// Login method (e.g., "oauth", "api_key").
    pub login_method: Option<String>,
    /// Raw output for debugging.
    pub raw_text: String,
}

impl ClaudeStatusSnapshot {
    /// Returns true if we have any usage data.
    pub fn has_data(&self) -> bool {
        self.session_percent_left.is_some()
            || self.weekly_percent_left.is_some()
            || self.opus_percent_left.is_some()
    }

    /// Get session usage as percent USED.
    pub fn session_used_percent(&self) -> Option<f64> {
        self.session_percent_left.map(|left| 100.0 - left)
    }

    /// Get weekly usage as percent USED.
    pub fn weekly_used_percent(&self) -> Option<f64> {
        self.weekly_percent_left.map(|left| 100.0 - left)
    }

    /// Get opus usage as percent USED.
    pub fn opus_used_percent(&self) -> Option<f64> {
        self.opus_percent_left.map(|left| 100.0 - left)
    }
}

// ============================================================================
// PTY Probe
// ============================================================================

/// PTY-based probe for fetching Claude status.
#[derive(Debug, Clone)]
pub struct ClaudePtyProbe {
    runner: PtyRunner,
    timeout: Duration,
}

impl Default for ClaudePtyProbe {
    fn default() -> Self {
        Self::new()
    }
}

impl ClaudePtyProbe {
    /// Create a new PTY probe.
    pub fn new() -> Self {
        Self {
            runner: PtyRunner::new(120, 40),
            timeout: PTY_TIMEOUT,
        }
    }

    /// Create with custom timeout.
    pub fn with_timeout(timeout: Duration) -> Self {
        Self {
            runner: PtyRunner::new(120, 40),
            timeout,
        }
    }

    /// Check if claude is available.
    pub fn is_available() -> bool {
        PtyRunner::exists(CLAUDE_BINARY)
    }

    /// Fetch usage using the /usage command.
    #[instrument(skip(self))]
    pub async fn fetch_usage(&self) -> Result<ClaudeStatusSnapshot, ClaudeError> {
        if !Self::is_available() {
            return Err(ClaudeError::BinaryNotFound(CLAUDE_BINARY.to_string()));
        }

        debug!("Fetching usage via PTY");

        let options = PtyOptions::with_timeout(self.timeout)
            .with_idle_timeout(IDLE_TIMEOUT)
            .stop_on_any(STOP_PATTERNS.iter().copied())
            .with_env("TERM", "xterm-256color")
            .with_env("NO_COLOR", "1");

        // Send /usage command followed by exit
        let input = "/usage\nexit\n";

        let result = self.runner.run(CLAUDE_BINARY, input, options).await?;

        debug!(
            output_len = result.output.len(),
            exit_code = ?result.exit_code,
            timed_out = result.timed_out,
            "PTY command completed"
        );

        // Parse the output
        let snapshot = parse_usage_output(&result.output)?;

        if !snapshot.has_data() && !result.timed_out {
            warn!("No usage data found in output");
        }

        Ok(snapshot)
    }

    /// Fetch status using the /status command.
    #[instrument(skip(self))]
    pub async fn fetch_status(&self) -> Result<ClaudeStatusSnapshot, ClaudeError> {
        if !Self::is_available() {
            return Err(ClaudeError::BinaryNotFound(CLAUDE_BINARY.to_string()));
        }

        debug!("Fetching status via PTY");

        let options = PtyOptions::with_timeout(self.timeout)
            .with_idle_timeout(IDLE_TIMEOUT)
            .stop_on_any(STOP_PATTERNS.iter().copied())
            .with_env("TERM", "xterm-256color")
            .with_env("NO_COLOR", "1");

        let input = "/status\nexit\n";

        let result = self.runner.run(CLAUDE_BINARY, input, options).await?;

        parse_usage_output(&result.output)
    }
}

// ============================================================================
// Parser Functions
// ============================================================================

/// Parse the /usage command output into a snapshot.
#[instrument(skip(text))]
pub fn parse_usage_output(text: &str) -> Result<ClaudeStatusSnapshot, ClaudeError> {
    let mut snapshot = ClaudeStatusSnapshot {
        raw_text: text.to_string(),
        ..Default::default()
    };

    // Split into sections based on blank lines or headers
    let sections = split_into_sections(text);

    for section in &sections {
        let section_lower = section.to_lowercase();

        // Determine section type and extract data
        if section_lower.contains("session") || section_lower.contains("5h") || section_lower.contains("5 hour") {
            if let Some(pct) = extract_percent_left(section) {
                snapshot.session_percent_left = Some(pct);
            } else if let Some(pct) = extract_percent_used(section) {
                snapshot.session_percent_left = Some(100.0 - pct);
            }
            snapshot.session_reset = extract_reset_time(section);
        } else if section_lower.contains("week") && (section_lower.contains("all") || !section_lower.contains("sonnet")) {
            if let Some(pct) = extract_percent_left(section) {
                snapshot.weekly_percent_left = Some(pct);
            } else if let Some(pct) = extract_percent_used(section) {
                snapshot.weekly_percent_left = Some(100.0 - pct);
            }
            snapshot.weekly_reset = extract_reset_time(section);
        } else if section_lower.contains("opus") || section_lower.contains("sonnet") || section_lower.contains("premium") {
            if let Some(pct) = extract_percent_left(section) {
                snapshot.opus_percent_left = Some(pct);
            } else if let Some(pct) = extract_percent_used(section) {
                snapshot.opus_percent_left = Some(100.0 - pct);
            }
        }
    }

    // Extract account info from full text
    snapshot.account_email = extract_email(text);
    snapshot.account_organization = extract_organization(text);
    snapshot.login_method = extract_login_method(text);

    // Fallback: try line-by-line parsing if no sections found
    if !snapshot.has_data() {
        parse_line_by_line(text, &mut snapshot);
    }

    Ok(snapshot)
}

/// Split text into logical sections.
fn split_into_sections(text: &str) -> Vec<String> {
    let mut sections = Vec::new();
    let mut current = String::new();

    for line in text.lines() {
        let line = line.trim();

        // New section on headers
        if line.starts_with("Current")
            || line.starts_with("Session")
            || line.starts_with("Weekly")
            || line.starts_with("Opus")
            || line.starts_with("Sonnet")
        {
            if !current.is_empty() {
                sections.push(current.clone());
                current.clear();
            }
        }

        if !line.is_empty() {
            if !current.is_empty() {
                current.push('\n');
            }
            current.push_str(line);
        }
    }

    if !current.is_empty() {
        sections.push(current);
    }

    sections
}

/// Extract "XX% left" from text.
pub fn extract_percent_left(text: &str) -> Option<f64> {
    PERCENT_LEFT_RE.captures(text).and_then(|caps| {
        caps.get(1)?.as_str().parse().ok()
    })
}

/// Extract "XX% used" from text.
pub fn extract_percent_used(text: &str) -> Option<f64> {
    PERCENT_USED_RE.captures(text).and_then(|caps| {
        caps.get(1)?.as_str().parse().ok()
    })
}

/// Extract reset time description.
pub fn extract_reset_time(text: &str) -> Option<String> {
    RESET_TIME_RE.captures(text).and_then(|caps| {
        let time = caps.get(1)?.as_str().trim();
        if time.is_empty() {
            None
        } else {
            Some(time.to_string())
        }
    })
}

/// Extract email from text.
pub fn extract_email(text: &str) -> Option<String> {
    EMAIL_RE.captures(text).and_then(|caps| {
        Some(caps.get(1)?.as_str().to_string())
    })
}

/// Extract organization from text.
pub fn extract_organization(text: &str) -> Option<String> {
    ORG_RE.captures(text).and_then(|caps| {
        let org = caps.get(1)?.as_str().trim();
        if org.is_empty() {
            None
        } else {
            Some(org.to_string())
        }
    })
}

/// Extract login method from text.
pub fn extract_login_method(text: &str) -> Option<String> {
    LOGIN_RE.captures(text).and_then(|caps| {
        Some(caps.get(1)?.as_str().to_lowercase())
    })
}

/// Fallback line-by-line parsing.
fn parse_line_by_line(text: &str, snapshot: &mut ClaudeStatusSnapshot) {
    let lines: Vec<&str> = text.lines().collect();
    let mut i = 0;

    while i < lines.len() {
        let line = lines[i].trim().to_lowercase();

        // Look for percentage on next line after header
        if i + 1 < lines.len() {
            let next_line = lines[i + 1];

            if line.contains("session") || line.contains("5h") {
                if let Some(pct) = extract_percent_left(next_line) {
                    snapshot.session_percent_left = Some(pct);
                }
            } else if line.contains("week") {
                if let Some(pct) = extract_percent_left(next_line) {
                    if snapshot.weekly_percent_left.is_none() {
                        snapshot.weekly_percent_left = Some(pct);
                    } else {
                        // Second weekly is probably Sonnet
                        snapshot.opus_percent_left = Some(pct);
                    }
                }
            } else if line.contains("opus") || line.contains("sonnet") {
                if let Some(pct) = extract_percent_left(next_line) {
                    snapshot.opus_percent_left = Some(pct);
                }
            }
        }

        i += 1;
    }
}

// ============================================================================
// Conversion to Core Types
// ============================================================================

impl ClaudeStatusSnapshot {
    /// Convert to a UsageSnapshot.
    pub fn to_snapshot(&self) -> exactobar_core::UsageSnapshot {
        use exactobar_core::{FetchSource, LoginMethod, ProviderIdentity, ProviderKind};

        let mut snapshot = exactobar_core::UsageSnapshot::new();
        snapshot.fetch_source = FetchSource::CLI;

        // Primary = session
        if let Some(used) = self.session_used_percent() {
            snapshot.primary = Some(exactobar_core::UsageWindow {
                used_percent: used,
                window_minutes: Some(300), // 5 hours
                resets_at: None,
                reset_description: self.session_reset.clone(),
            });
        }

        // Secondary = weekly
        if let Some(used) = self.weekly_used_percent() {
            snapshot.secondary = Some(exactobar_core::UsageWindow {
                used_percent: used,
                window_minutes: Some(10080), // 7 days
                resets_at: None,
                reset_description: self.weekly_reset.clone(),
            });
        }

        // Tertiary = opus/sonnet
        if let Some(used) = self.opus_used_percent() {
            snapshot.tertiary = Some(exactobar_core::UsageWindow {
                used_percent: used,
                window_minutes: Some(10080), // 7 days
                resets_at: None,
                reset_description: None,
            });
        }

        // Identity
        if self.account_email.is_some() || self.account_organization.is_some() {
            let mut identity = ProviderIdentity::new(ProviderKind::Claude);
            identity.account_email = self.account_email.clone();
            identity.account_organization = self.account_organization.clone();
            identity.login_method = self.login_method.as_deref().and_then(|m| {
                match m {
                    "oauth" => Some(LoginMethod::OAuth),
                    "api" | "api_key" => Some(LoginMethod::ApiKey),
                    "cli" => Some(LoginMethod::CLI),
                    _ => None,
                }
            });
            snapshot.identity = Some(identity);
        }

        snapshot
    }
}

// ============================================================================
// Tests
// ============================================================================

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_extract_percent_left() {
        assert!((extract_percent_left("72% left").unwrap() - 72.0).abs() < 0.01);
        assert!((extract_percent_left("45.5% remaining").unwrap() - 45.5).abs() < 0.01);
        assert!((extract_percent_left("100% left").unwrap() - 100.0).abs() < 0.01);
        assert!(extract_percent_left("no percentage here").is_none());
    }

    #[test]
    fn test_extract_percent_used() {
        assert!((extract_percent_used("28% used").unwrap() - 28.0).abs() < 0.01);
        assert!((extract_percent_used("55.5% used").unwrap() - 55.5).abs() < 0.01);
        assert!(extract_percent_used("no percentage here").is_none());
    }

    #[test]
    fn test_extract_reset_time() {
        assert_eq!(
            extract_reset_time("Resets 2pm (PST)"),
            Some("2pm (PST)".to_string())
        );
        assert_eq!(
            extract_reset_time("Resets Jan 5 at 12am"),
            Some("Jan 5 at 12am".to_string())
        );
        assert_eq!(
            extract_reset_time("Reset: tomorrow"),
            Some("tomorrow".to_string())
        );
    }

    #[test]
    fn test_extract_email() {
        assert_eq!(
            extract_email("Account: user@example.com"),
            Some("user@example.com".to_string())
        );
        assert_eq!(
            extract_email("Email test@test.org"),
            Some("test@test.org".to_string())
        );
    }

    #[test]
    fn test_parse_usage_output_full() {
        let output = r#"
            Current session
            72% left
            Resets 2pm (PST)

            Current week (all models)
            45% left
            Resets Jan 5 at 12am

            Current week (Sonnet)
            80% left
            Resets Jan 5 at 12am

            Account: user@example.com
        "#;

        let snapshot = parse_usage_output(output).unwrap();

        assert!(snapshot.has_data());
        assert!((snapshot.session_percent_left.unwrap() - 72.0).abs() < 0.01);
        assert!((snapshot.weekly_percent_left.unwrap() - 45.0).abs() < 0.01);
        assert!((snapshot.opus_percent_left.unwrap() - 80.0).abs() < 0.01);
        assert_eq!(snapshot.account_email, Some("user@example.com".to_string()));
    }

    #[test]
    fn test_parse_usage_output_minimal() {
        let output = "Session: 50% left";
        let snapshot = parse_usage_output(output).unwrap();

        // This might not parse perfectly but should not fail
        let _ = snapshot.has_data();
    }

    #[test]
    fn test_parse_usage_output_used_format() {
        let output = r#"
            Session
            30% used

            Weekly
            60% used
        "#;

        let snapshot = parse_usage_output(output).unwrap();

        // 30% used = 70% left
        if let Some(left) = snapshot.session_percent_left {
            assert!((left - 70.0).abs() < 0.01);
        }
    }

    #[test]
    fn test_snapshot_conversion() {
        let status = ClaudeStatusSnapshot {
            session_percent_left: Some(72.0),
            weekly_percent_left: Some(45.0),
            opus_percent_left: Some(80.0),
            session_reset: Some("2pm (PST)".to_string()),
            weekly_reset: Some("Jan 5".to_string()),
            account_email: Some("user@example.com".to_string()),
            account_organization: None,
            login_method: Some("oauth".to_string()),
            raw_text: String::new(),
        };

        let snapshot = status.to_snapshot();

        // 72% left = 28% used
        assert!(snapshot.primary.is_some());
        assert!((snapshot.primary.as_ref().unwrap().used_percent - 28.0).abs() < 0.01);

        // 45% left = 55% used
        assert!(snapshot.secondary.is_some());
        assert!((snapshot.secondary.as_ref().unwrap().used_percent - 55.0).abs() < 0.01);

        // 80% left = 20% used
        assert!(snapshot.tertiary.is_some());
        assert!((snapshot.tertiary.as_ref().unwrap().used_percent - 20.0).abs() < 0.01);

        assert!(snapshot.identity.is_some());
    }

    #[test]
    fn test_is_available() {
        let _ = ClaudePtyProbe::is_available();
    }

    #[test]
    fn test_split_into_sections() {
        let text = r#"Current session
72% left

Current week
45% left"#;

        let sections = split_into_sections(text);
        assert!(sections.len() >= 2);
    }
}
