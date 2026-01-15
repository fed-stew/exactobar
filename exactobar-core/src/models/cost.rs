//! Cost tracking types.
//!
//! This module contains types for tracking token usage costs:
//! - [`CostUsageSnapshot`] - Container for cost data
//! - [`DailyUsageEntry`] - Per-day usage breakdown
//! - [`ModelBreakdown`] - Per-model cost breakdown

use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};

// ============================================================================
// Cost Usage Snapshot
// ============================================================================

/// Token cost usage snapshot from local log scanning.
///
/// This tracks actual token usage and costs, typically by scanning
/// local log files or API responses.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct CostUsageSnapshot {
    /// Tokens used in current session.
    pub session_tokens: Option<u64>,
    /// Cost in USD for current session.
    pub session_cost_usd: Option<f64>,
    /// Tokens used in last 30 days.
    pub last_30_days_tokens: Option<u64>,
    /// Cost in USD for last 30 days.
    pub last_30_days_cost_usd: Option<f64>,
    /// Daily usage entries.
    #[serde(default)]
    pub daily: Vec<DailyUsageEntry>,
    /// When this snapshot was last updated.
    pub updated_at: DateTime<Utc>,
}

impl CostUsageSnapshot {
    /// Creates a new empty cost snapshot.
    pub fn new() -> Self {
        Self {
            session_tokens: None,
            session_cost_usd: None,
            last_30_days_tokens: None,
            last_30_days_cost_usd: None,
            daily: Vec::new(),
            updated_at: Utc::now(),
        }
    }

    /// Returns the total tokens across all daily entries.
    pub fn total_daily_tokens(&self) -> u64 {
        self.daily.iter().filter_map(|d| d.total_tokens).sum()
    }

    /// Returns the total cost across all daily entries.
    pub fn total_daily_cost(&self) -> f64 {
        self.daily.iter().filter_map(|d| d.cost_usd).sum()
    }

    /// Returns the number of days with usage data.
    pub fn days_with_data(&self) -> usize {
        self.daily.len()
    }

    /// Returns the average daily cost.
    pub fn average_daily_cost(&self) -> Option<f64> {
        if self.daily.is_empty() {
            return None;
        }
        Some(self.total_daily_cost() / self.daily.len() as f64)
    }

    /// Returns the average daily tokens.
    pub fn average_daily_tokens(&self) -> Option<u64> {
        if self.daily.is_empty() {
            return None;
        }
        Some(self.total_daily_tokens() / self.daily.len() as u64)
    }

    /// Returns entries sorted by date (most recent first).
    pub fn sorted_by_date(&self) -> Vec<&DailyUsageEntry> {
        let mut entries: Vec<_> = self.daily.iter().collect();
        entries.sort_by(|a, b| b.date.cmp(&a.date));
        entries
    }
}

impl Default for CostUsageSnapshot {
    fn default() -> Self {
        Self::new()
    }
}

// ============================================================================
// Daily Usage Entry
// ============================================================================

/// Daily usage entry for token/cost tracking.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct DailyUsageEntry {
    /// Date in "YYYY-MM-DD" format.
    pub date: String,
    /// Input tokens used.
    pub input_tokens: Option<u64>,
    /// Output tokens generated.
    pub output_tokens: Option<u64>,
    /// Cache read tokens.
    pub cache_read_tokens: Option<u64>,
    /// Cache creation tokens.
    pub cache_creation_tokens: Option<u64>,
    /// Total tokens (input + output + cache).
    pub total_tokens: Option<u64>,
    /// Total cost in USD.
    pub cost_usd: Option<f64>,
    /// Models used on this day.
    #[serde(default)]
    pub models_used: Option<Vec<String>>,
    /// Per-model cost breakdown.
    #[serde(default)]
    pub model_breakdowns: Option<Vec<ModelBreakdown>>,
}

impl DailyUsageEntry {
    /// Creates a new entry for the given date.
    pub fn new(date: impl Into<String>) -> Self {
        Self {
            date: date.into(),
            input_tokens: None,
            output_tokens: None,
            cache_read_tokens: None,
            cache_creation_tokens: None,
            total_tokens: None,
            cost_usd: None,
            models_used: None,
            model_breakdowns: None,
        }
    }

    /// Computes total tokens from components if not set.
    pub fn computed_total_tokens(&self) -> u64 {
        if let Some(total) = self.total_tokens {
            return total;
        }
        self.input_tokens.unwrap_or(0)
            + self.output_tokens.unwrap_or(0)
            + self.cache_read_tokens.unwrap_or(0)
            + self.cache_creation_tokens.unwrap_or(0)
    }

    /// Returns the number of unique models used.
    pub fn unique_models_count(&self) -> usize {
        self.models_used.as_ref().map(|m| m.len()).unwrap_or(0)
    }

    /// Returns true if this entry has any token data.
    pub fn has_token_data(&self) -> bool {
        self.input_tokens.is_some()
            || self.output_tokens.is_some()
            || self.total_tokens.is_some()
    }
}

// ============================================================================
// Model Breakdown
// ============================================================================

/// Per-model cost breakdown.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ModelBreakdown {
    /// Model name (e.g., "claude-3-opus", "gpt-4").
    pub model_name: String,
    /// Cost in USD for this model.
    pub cost_usd: Option<f64>,
    /// Input tokens for this model.
    pub input_tokens: Option<u64>,
    /// Output tokens for this model.
    pub output_tokens: Option<u64>,
}

impl ModelBreakdown {
    /// Creates a new breakdown for the given model.
    pub fn new(model_name: impl Into<String>) -> Self {
        Self {
            model_name: model_name.into(),
            cost_usd: None,
            input_tokens: None,
            output_tokens: None,
        }
    }

    /// Returns total tokens for this model.
    pub fn total_tokens(&self) -> u64 {
        self.input_tokens.unwrap_or(0) + self.output_tokens.unwrap_or(0)
    }
}

// ============================================================================
// Tests
// ============================================================================

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_daily_usage_computed_tokens() {
        let mut entry = DailyUsageEntry::new("2024-01-15");
        entry.input_tokens = Some(1000);
        entry.output_tokens = Some(500);
        entry.cache_read_tokens = Some(200);

        assert_eq!(entry.computed_total_tokens(), 1700);
    }

    #[test]
    fn test_daily_usage_with_total_set() {
        let mut entry = DailyUsageEntry::new("2024-01-15");
        entry.input_tokens = Some(1000);
        entry.output_tokens = Some(500);
        entry.total_tokens = Some(2000); // Explicit total

        // Should use explicit total, not computed
        assert_eq!(entry.computed_total_tokens(), 2000);
    }

    #[test]
    fn test_cost_snapshot_totals() {
        let mut snapshot = CostUsageSnapshot::new();
        snapshot.daily = vec![
            {
                let mut e = DailyUsageEntry::new("2024-01-15");
                e.total_tokens = Some(1000);
                e.cost_usd = Some(0.50);
                e
            },
            {
                let mut e = DailyUsageEntry::new("2024-01-16");
                e.total_tokens = Some(2000);
                e.cost_usd = Some(1.00);
                e
            },
        ];

        assert_eq!(snapshot.total_daily_tokens(), 3000);
        assert_eq!(snapshot.total_daily_cost(), 1.50);
        assert_eq!(snapshot.days_with_data(), 2);
    }

    #[test]
    fn test_cost_snapshot_averages() {
        let mut snapshot = CostUsageSnapshot::new();
        snapshot.daily = vec![
            {
                let mut e = DailyUsageEntry::new("2024-01-15");
                e.total_tokens = Some(1000);
                e.cost_usd = Some(1.00);
                e
            },
            {
                let mut e = DailyUsageEntry::new("2024-01-16");
                e.total_tokens = Some(3000);
                e.cost_usd = Some(3.00);
                e
            },
        ];

        assert_eq!(snapshot.average_daily_tokens(), Some(2000));
        assert_eq!(snapshot.average_daily_cost(), Some(2.00));
    }

    #[test]
    fn test_model_breakdown() {
        let mut breakdown = ModelBreakdown::new("claude-3-opus");
        breakdown.input_tokens = Some(500);
        breakdown.output_tokens = Some(300);
        breakdown.cost_usd = Some(0.25);

        assert_eq!(breakdown.total_tokens(), 800);
    }
}
