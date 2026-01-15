//! Usage-related types.
//!
//! This module contains types related to usage tracking:
//! - [`UsageSnapshot`] - Main container with multiple windows
//! - [`UsageWindow`] - Individual usage window
//! - [`UsageData`] - Legacy simple format
//! - [`Quota`] - Quota information
//! - [`Credits`] - Credit-based systems

use chrono::{DateTime, Duration, Utc};
use serde::{Deserialize, Serialize};

use super::provider::ProviderKind;
use super::status::FetchSource;
use super::ProviderIdentity;
use crate::error::CoreError;

// ============================================================================
// Usage Snapshot & Windows
// ============================================================================

/// A snapshot of usage data with primary, secondary, and tertiary windows.
///
/// This is the main container for usage information:
/// - **Primary** = session window (e.g., 5 hours for Claude)
/// - **Secondary** = weekly/monthly window
/// - **Tertiary** = opus/premium tier (Claude-specific)
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct UsageSnapshot {
    /// Primary usage window (session-based).
    pub primary: Option<UsageWindow>,
    /// Secondary usage window (weekly/monthly).
    pub secondary: Option<UsageWindow>,
    /// Tertiary usage window (opus/premium tier).
    pub tertiary: Option<UsageWindow>,
    /// When this snapshot was last updated.
    pub updated_at: DateTime<Utc>,
    /// Account identity for this provider.
    pub identity: Option<ProviderIdentity>,
    /// How this data was fetched.
    #[serde(default)]
    pub fetch_source: FetchSource,
}

impl UsageSnapshot {
    /// Creates a new empty usage snapshot.
    pub fn new() -> Self {
        Self {
            primary: None,
            secondary: None,
            tertiary: None,
            updated_at: Utc::now(),
            identity: None,
            fetch_source: FetchSource::default(),
        }
    }

    /// Returns true if this snapshot is stale (older than threshold).
    pub fn is_stale(&self, threshold: Duration) -> bool {
        Utc::now() - self.updated_at > threshold
    }

    /// Returns true if any window is approaching its limit (>80%).
    pub fn is_approaching_limit(&self) -> bool {
        self.primary.as_ref().is_some_and(|w| w.used_percent > 80.0)
            || self.secondary.as_ref().is_some_and(|w| w.used_percent > 80.0)
            || self.tertiary.as_ref().is_some_and(|w| w.used_percent > 80.0)
    }

    /// Returns the highest usage percentage across all windows.
    pub fn max_usage_percent(&self) -> f64 {
        let mut max = 0.0_f64;
        if let Some(ref w) = self.primary {
            max = max.max(w.used_percent);
        }
        if let Some(ref w) = self.secondary {
            max = max.max(w.used_percent);
        }
        if let Some(ref w) = self.tertiary {
            max = max.max(w.used_percent);
        }
        max
    }

    /// Returns true if any window data is present.
    pub fn has_data(&self) -> bool {
        self.primary.is_some() || self.secondary.is_some() || self.tertiary.is_some()
    }
}

impl Default for UsageSnapshot {
    fn default() -> Self {
        Self::new()
    }
}

impl UsageSnapshot {
    /// Validates the snapshot data.
    ///
    /// Ensures all percentage values are within valid ranges [0, 100].
    /// This should be called after parsing API responses to catch
    /// malformed or malicious data.
    ///
    /// # Errors
    ///
    /// Returns `CoreError::InvalidData` if any usage window contains
    /// invalid percentage values (negative, > 100, or non-finite).
    pub fn validate(&self) -> Result<(), CoreError> {
        if let Some(ref primary) = self.primary {
            primary.validate().map_err(|e| {
                CoreError::InvalidData(format!("primary window: {e}"))
            })?;
        }
        if let Some(ref secondary) = self.secondary {
            secondary.validate().map_err(|e| {
                CoreError::InvalidData(format!("secondary window: {e}"))
            })?;
        }
        if let Some(ref tertiary) = self.tertiary {
            tertiary.validate().map_err(|e| {
                CoreError::InvalidData(format!("tertiary window: {e}"))
            })?;
        }
        Ok(())
    }

    /// Validates and clamps values to valid ranges.
    ///
    /// Unlike `validate()`, this method fixes invalid values instead
    /// of returning an error. Use when you want to be lenient with
    /// potentially buggy API responses.
    pub fn sanitize(&mut self) {
        if let Some(ref mut primary) = self.primary {
            primary.sanitize();
        }
        if let Some(ref mut secondary) = self.secondary {
            secondary.sanitize();
        }
        if let Some(ref mut tertiary) = self.tertiary {
            tertiary.sanitize();
        }
    }
}

/// Represents a single usage window (session, weekly, or tier).
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct UsageWindow {
    /// Percentage of quota used (0-100).
    pub used_percent: f64,
    /// Window duration in minutes (300 = 5 hours, 10080 = 1 week).
    pub window_minutes: Option<u32>,
    /// When this window resets.
    pub resets_at: Option<DateTime<Utc>>,
    /// Human-readable reset description (e.g., "in 2 hours").
    pub reset_description: Option<String>,
}

impl UsageWindow {
    /// Creates a new usage window with the given percentage.
    pub fn new(used_percent: f64) -> Self {
        Self {
            used_percent,
            window_minutes: None,
            resets_at: None,
            reset_description: None,
        }
    }

    /// Returns the remaining percentage (100 - used).
    pub fn remaining_percent(&self) -> f64 {
        (100.0 - self.used_percent).max(0.0)
    }

    /// Returns true if usage is over the limit.
    pub fn is_over_limit(&self) -> bool {
        self.used_percent >= 100.0
    }

    /// Returns true if usage is approaching the limit (>80%).
    pub fn is_approaching_limit(&self) -> bool {
        self.used_percent > 80.0
    }

    /// Returns the window duration as a chrono Duration.
    pub fn window_duration(&self) -> Option<Duration> {
        self.window_minutes.map(|m| Duration::minutes(i64::from(m)))
    }

    /// Returns time until reset, if known.
    pub fn time_until_reset(&self) -> Option<Duration> {
        self.resets_at.map(|reset| reset - Utc::now())
    }
}

impl Default for UsageWindow {
    fn default() -> Self {
        Self::new(0.0)
    }
}

impl UsageWindow {
    /// Validates the window data.
    ///
    /// Ensures percentage values are within valid ranges [0, 100].
    ///
    /// # Errors
    ///
    /// Returns `CoreError::InvalidData` if `used_percent` is negative,
    /// greater than 100, or not a finite number.
    pub fn validate(&self) -> Result<(), CoreError> {
        if self.used_percent < 0.0 || self.used_percent > 100.0 {
            return Err(CoreError::InvalidData(format!(
                "used_percent {} out of valid range [0, 100]",
                self.used_percent
            )));
        }
        if !self.used_percent.is_finite() {
            return Err(CoreError::InvalidData(
                "used_percent is not a finite number".to_string()
            ));
        }
        Ok(())
    }

    /// Sanitizes window data by clamping to valid ranges.
    ///
    /// - Clamps `used_percent` to [0, 100]
    /// - Replaces NaN/Infinity with 0.0
    pub fn sanitize(&mut self) {
        if !self.used_percent.is_finite() {
            self.used_percent = 0.0;
        }
        self.used_percent = self.used_percent.clamp(0.0, 100.0);
    }
}

// ============================================================================
// Credits
// ============================================================================

/// Credits information for providers that use credit systems.
///
/// Some providers (like Cursor) use a credit system instead of
/// percentage-based quotas.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Credits {
    /// Remaining credits.
    pub remaining: f64,
    /// Total credits (if known).
    pub total: Option<f64>,
    /// When this was last updated.
    pub updated_at: DateTime<Utc>,
}

impl Credits {
    /// Creates new credits with the given remaining amount.
    pub fn new(remaining: f64) -> Self {
        Self {
            remaining,
            total: None,
            updated_at: Utc::now(),
        }
    }

    /// Returns the usage percentage if total is known.
    pub fn usage_percent(&self) -> Option<f64> {
        self.total.map(|total| {
            if total > 0.0 {
                ((total - self.remaining) / total) * 100.0
            } else {
                0.0
            }
        })
    }

    /// Returns remaining as a percentage of total.
    pub fn remaining_percent(&self) -> Option<f64> {
        self.total.map(|total| {
            if total > 0.0 {
                (self.remaining / total) * 100.0
            } else {
                100.0
            }
        })
    }
}

impl Default for Credits {
    fn default() -> Self {
        Self::new(0.0)
    }
}

// ============================================================================
// Legacy Types
// ============================================================================

/// Usage data for a provider (legacy simple format).
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct UsageData {
    /// The provider this data is for.
    pub provider_kind: ProviderKind,
    /// Timestamp when this data was fetched.
    pub fetched_at: DateTime<Utc>,
    /// Current usage amount (provider-specific units).
    pub current_usage: f64,
    /// Usage limit/quota (if known).
    pub limit: Option<f64>,
    /// Unit of measurement (e.g., "tokens", "requests", "USD").
    pub unit: String,
    /// Billing period start (if applicable).
    pub period_start: Option<DateTime<Utc>>,
    /// Billing period end (if applicable).
    pub period_end: Option<DateTime<Utc>>,
    /// Additional provider-specific metadata.
    #[serde(default)]
    pub metadata: serde_json::Value,
}

impl UsageData {
    /// Returns the usage percentage if a limit is set.
    pub fn usage_percentage(&self) -> Option<f64> {
        self.limit.map(|limit| {
            if limit > 0.0 {
                (self.current_usage / limit) * 100.0
            } else {
                0.0
            }
        })
    }

    /// Returns true if usage is approaching the limit (>80%).
    pub fn is_approaching_limit(&self) -> bool {
        self.usage_percentage().is_some_and(|pct| pct > 80.0)
    }

    /// Returns true if usage has exceeded the limit.
    pub fn is_over_limit(&self) -> bool {
        self.usage_percentage().is_some_and(|pct| pct >= 100.0)
    }

    /// Converts to a `UsageSnapshot`.
    pub fn to_snapshot(&self) -> UsageSnapshot {
        let window = self.usage_percentage().map(|pct| UsageWindow {
            used_percent: pct,
            window_minutes: None,
            resets_at: self.period_end,
            reset_description: None,
        });

        UsageSnapshot {
            primary: window,
            secondary: None,
            tertiary: None,
            updated_at: self.fetched_at,
            identity: None,
            fetch_source: FetchSource::Auto,
        }
    }
}

/// Quota information for a provider.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Quota {
    /// The provider this quota is for.
    pub provider_kind: ProviderKind,
    /// Total quota amount.
    pub total: f64,
    /// Used amount.
    pub used: f64,
    /// Remaining amount.
    pub remaining: f64,
    /// Unit of measurement.
    pub unit: String,
    /// When the quota resets.
    pub resets_at: Option<DateTime<Utc>>,
}

impl Quota {
    /// Returns the usage percentage.
    pub fn usage_percentage(&self) -> f64 {
        if self.total > 0.0 {
            (self.used / self.total) * 100.0
        } else {
            0.0
        }
    }

    /// Converts to a `UsageWindow`.
    pub fn to_window(&self) -> UsageWindow {
        UsageWindow {
            used_percent: self.usage_percentage(),
            window_minutes: None,
            resets_at: self.resets_at,
            reset_description: None,
        }
    }
}

// ============================================================================
// Tests
// ============================================================================

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_usage_window_remaining() {
        let window = UsageWindow::new(75.0);
        assert_eq!(window.remaining_percent(), 25.0);
        assert!(!window.is_approaching_limit()); // 75% is not > 80%
        assert!(!window.is_over_limit());

        let high_window = UsageWindow::new(85.0);
        assert!(high_window.is_approaching_limit()); // 85% is > 80%
    }

    #[test]
    fn test_usage_window_over_limit() {
        let window = UsageWindow::new(100.0);
        assert_eq!(window.remaining_percent(), 0.0);
        assert!(window.is_over_limit());
    }

    #[test]
    fn test_usage_snapshot_max_usage() {
        let mut snapshot = UsageSnapshot::new();
        snapshot.primary = Some(UsageWindow::new(50.0));
        snapshot.secondary = Some(UsageWindow::new(85.0)); // > 80%
        snapshot.tertiary = Some(UsageWindow::new(30.0));

        assert_eq!(snapshot.max_usage_percent(), 85.0);
        assert!(snapshot.is_approaching_limit());
    }

    #[test]
    fn test_credits_percentage() {
        let mut credits = Credits::new(25.0);
        credits.total = Some(100.0);

        assert_eq!(credits.usage_percent(), Some(75.0));
        assert_eq!(credits.remaining_percent(), Some(25.0));
    }

    #[test]
    fn test_usage_percentage() {
        let usage = UsageData {
            provider_kind: ProviderKind::Claude,
            fetched_at: Utc::now(),
            current_usage: 85.0, // > 80%
            limit: Some(100.0),
            unit: "USD".to_string(),
            period_start: None,
            period_end: None,
            metadata: serde_json::Value::Null,
        };

        assert_eq!(usage.usage_percentage(), Some(85.0));
        assert!(usage.is_approaching_limit());
        assert!(!usage.is_over_limit());
    }

    #[test]
    fn test_usage_data_to_snapshot() {
        let usage = UsageData {
            provider_kind: ProviderKind::Gemini,
            fetched_at: Utc::now(),
            current_usage: 50.0,
            limit: Some(100.0),
            unit: "requests".to_string(),
            period_start: None,
            period_end: None,
            metadata: serde_json::Value::Null,
        };

        let snapshot = usage.to_snapshot();
        assert!(snapshot.primary.is_some());
        assert_eq!(snapshot.primary.unwrap().used_percent, 50.0);
    }

    // ==========================================================================
    // Security: Validation Tests
    // ==========================================================================

    #[test]
    fn test_usage_window_validate_valid() {
        let window = UsageWindow::new(50.0);
        assert!(window.validate().is_ok());

        let window_zero = UsageWindow::new(0.0);
        assert!(window_zero.validate().is_ok());

        let window_full = UsageWindow::new(100.0);
        assert!(window_full.validate().is_ok());
    }

    #[test]
    fn test_usage_window_validate_invalid() {
        let window_negative = UsageWindow::new(-10.0);
        assert!(window_negative.validate().is_err());

        let window_over = UsageWindow::new(150.0);
        assert!(window_over.validate().is_err());

        let window_nan = UsageWindow::new(f64::NAN);
        assert!(window_nan.validate().is_err());

        let window_inf = UsageWindow::new(f64::INFINITY);
        assert!(window_inf.validate().is_err());
    }

    #[test]
    fn test_usage_window_sanitize() {
        let mut window = UsageWindow::new(-10.0);
        window.sanitize();
        assert_eq!(window.used_percent, 0.0);

        let mut window = UsageWindow::new(150.0);
        window.sanitize();
        assert_eq!(window.used_percent, 100.0);

        let mut window = UsageWindow::new(f64::NAN);
        window.sanitize();
        assert_eq!(window.used_percent, 0.0);
    }

    #[test]
    fn test_usage_snapshot_validate() {
        let mut snapshot = UsageSnapshot::new();
        snapshot.primary = Some(UsageWindow::new(50.0));
        assert!(snapshot.validate().is_ok());

        // Invalid primary
        snapshot.primary = Some(UsageWindow::new(150.0));
        assert!(snapshot.validate().is_err());
    }

    #[test]
    fn test_usage_snapshot_sanitize() {
        let mut snapshot = UsageSnapshot::new();
        snapshot.primary = Some(UsageWindow::new(150.0));
        snapshot.secondary = Some(UsageWindow::new(-20.0));

        snapshot.sanitize();

        assert_eq!(snapshot.primary.as_ref().unwrap().used_percent, 100.0);
        assert_eq!(snapshot.secondary.as_ref().unwrap().used_percent, 0.0);
    }
}
