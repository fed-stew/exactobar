//! Status and fetch-related types.
//!
//! This module contains types for provider status and data fetching:
//! - [`ProviderStatus`] - Service health information
//! - [`StatusIndicator`] - Status levels
//! - [`FetchSource`] - How data was obtained

use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};

// ============================================================================
// Provider Status
// ============================================================================

/// Provider service status from status pages.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ProviderStatus {
    /// Status indicator level.
    pub indicator: StatusIndicator,
    /// Human-readable status description.
    pub description: String,
    /// When this status was last updated.
    pub updated_at: DateTime<Utc>,
    /// URL to the full status page.
    pub url: Option<String>,
}

impl ProviderStatus {
    /// Creates a new operational status.
    pub fn operational() -> Self {
        Self {
            indicator: StatusIndicator::None,
            description: "All systems operational".to_string(),
            updated_at: Utc::now(),
            url: None,
        }
    }

    /// Creates a new status with the given indicator and description.
    pub fn new(indicator: StatusIndicator, description: impl Into<String>) -> Self {
        Self {
            indicator,
            description: description.into(),
            updated_at: Utc::now(),
            url: None,
        }
    }

    /// Returns true if the service is fully operational.
    pub fn is_operational(&self) -> bool {
        self.indicator == StatusIndicator::None
    }

    /// Returns true if there's any degradation or outage.
    pub fn has_issues(&self) -> bool {
        !matches!(self.indicator, StatusIndicator::None | StatusIndicator::Unknown)
    }

    /// Returns true if this is a critical outage.
    pub fn is_critical(&self) -> bool {
        self.indicator == StatusIndicator::Critical
    }
}

impl Default for ProviderStatus {
    fn default() -> Self {
        Self::operational()
    }
}

// ============================================================================
// Status Indicator
// ============================================================================

/// Status indicator levels from provider status pages.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize, Default)]
#[serde(rename_all = "snake_case")]
pub enum StatusIndicator {
    /// Operational - no issues.
    #[default]
    None,
    /// Minor issues - degraded performance.
    Minor,
    /// Major issues - partial outage.
    Major,
    /// Critical issues - major outage.
    Critical,
    /// Under scheduled maintenance.
    Maintenance,
    /// Status unknown.
    Unknown,
}

impl StatusIndicator {
    /// Returns a human-readable label.
    pub fn label(&self) -> &'static str {
        match self {
            Self::None => "Operational",
            Self::Minor => "Degraded",
            Self::Major => "Partial Outage",
            Self::Critical => "Major Outage",
            Self::Maintenance => "Under Maintenance",
            Self::Unknown => "Unknown",
        }
    }

    /// Returns an emoji for the status.
    pub fn emoji(&self) -> &'static str {
        match self {
            Self::None => "🟢",
            Self::Minor => "🟡",
            Self::Major => "🟠",
            Self::Critical => "🔴",
            Self::Maintenance => "🔧",
            Self::Unknown => "⚪",
        }
    }

    /// Returns a severity score (0 = operational, 5 = unknown).
    pub fn severity(&self) -> u8 {
        match self {
            Self::None => 0,
            Self::Minor => 1,
            Self::Major => 2,
            Self::Critical => 3,
            Self::Maintenance => 4,
            Self::Unknown => 5,
        }
    }

    /// Returns all indicators in order of severity.
    pub fn all() -> &'static [StatusIndicator] {
        &[
            Self::None,
            Self::Minor,
            Self::Major,
            Self::Critical,
            Self::Maintenance,
            Self::Unknown,
        ]
    }
}

impl std::fmt::Display for StatusIndicator {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "{} {}", self.emoji(), self.label())
    }
}

// ============================================================================
// Fetch Source
// ============================================================================

/// How the usage data was fetched.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize, Default)]
#[serde(rename_all = "snake_case")]
pub enum FetchSource {
    /// Automatically determined best method.
    #[default]
    Auto,
    /// Via CLI tool (e.g., `claude` CLI).
    CLI,
    /// Via web scraping.
    Web,
    /// Via OAuth token.
    OAuth,
    /// Via API key.
    Api,
    /// Via local file/process probing.
    LocalProbe,
}

impl FetchSource {
    /// Returns a human-readable label.
    pub fn label(&self) -> &'static str {
        match self {
            Self::Auto => "Auto",
            Self::CLI => "CLI",
            Self::Web => "Web",
            Self::OAuth => "OAuth",
            Self::Api => "API",
            Self::LocalProbe => "Local",
        }
    }

    /// Returns a description of this fetch source.
    pub fn description(&self) -> &'static str {
        match self {
            Self::Auto => "Automatically determined best method",
            Self::CLI => "Via command-line tool",
            Self::Web => "Via web scraping",
            Self::OAuth => "Via OAuth authentication",
            Self::Api => "Via API key",
            Self::LocalProbe => "Via local file scanning",
        }
    }

    /// Returns all fetch sources.
    pub fn all() -> &'static [FetchSource] {
        &[
            Self::Auto,
            Self::CLI,
            Self::Web,
            Self::OAuth,
            Self::Api,
            Self::LocalProbe,
        ]
    }
}

impl std::fmt::Display for FetchSource {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "{}", self.label())
    }
}

// ============================================================================
// Tests
// ============================================================================

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_status_indicator_emoji() {
        assert_eq!(StatusIndicator::None.emoji(), "🟢");
        assert_eq!(StatusIndicator::Critical.emoji(), "🔴");
    }

    #[test]
    fn test_status_indicator_severity() {
        assert_eq!(StatusIndicator::None.severity(), 0);
        assert_eq!(StatusIndicator::Critical.severity(), 3);
        assert_eq!(StatusIndicator::Maintenance.severity(), 4);
        assert_eq!(StatusIndicator::Unknown.severity(), 5);
    }

    #[test]
    fn test_status_indicator_maintenance() {
        let maintenance = StatusIndicator::Maintenance;
        assert_eq!(maintenance.label(), "Under Maintenance");
        assert_eq!(maintenance.emoji(), "🔧");
    }

    #[test]
    fn test_provider_status_operational() {
        let status = ProviderStatus::operational();
        assert!(status.is_operational());
        assert!(!status.has_issues());
        assert!(!status.is_critical());
    }

    #[test]
    fn test_provider_status_critical() {
        let status = ProviderStatus::new(StatusIndicator::Critical, "Service outage");
        assert!(!status.is_operational());
        assert!(status.has_issues());
        assert!(status.is_critical());
    }

    #[test]
    fn test_fetch_source_display() {
        assert_eq!(FetchSource::CLI.to_string(), "CLI");
        assert_eq!(FetchSource::LocalProbe.to_string(), "Local");
    }

    #[test]
    fn test_status_indicator_display() {
        assert_eq!(StatusIndicator::None.to_string(), "🟢 Operational");
        assert_eq!(StatusIndicator::Critical.to_string(), "🔴 Major Outage");
    }
}
