//! Usage history tracking.

use chrono::{DateTime, Utc};
use exactobar_core::{ProviderKind, UsageData};
use serde::{Deserialize, Serialize};
use std::collections::{HashMap, VecDeque};

/// Maximum number of history entries per provider.
const MAX_HISTORY_ENTRIES: usize = 1000;

/// A single history entry.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct HistoryEntry {
    /// Timestamp of the entry.
    pub timestamp: DateTime<Utc>,
    /// Usage value at this point.
    pub value: f64,
    /// Unit of measurement.
    pub unit: String,
}

impl From<&UsageData> for HistoryEntry {
    fn from(usage: &UsageData) -> Self {
        Self {
            timestamp: usage.fetched_at,
            value: usage.current_usage,
            unit: usage.unit.clone(),
        }
    }
}

/// Tracks usage history for all providers.
#[derive(Debug, Default)]
pub struct UsageHistory {
    entries: HashMap<ProviderKind, VecDeque<HistoryEntry>>,
}

impl UsageHistory {
    /// Creates a new empty history.
    pub fn new() -> Self {
        Self::default()
    }

    /// Records a new usage data point.
    pub fn record(&mut self, usage: &UsageData) {
        let entry = HistoryEntry::from(usage);
        let entries = self
            .entries
            .entry(usage.provider_kind)
            .or_insert_with(VecDeque::new);

        entries.push_back(entry);

        // Trim if over limit
        while entries.len() > MAX_HISTORY_ENTRIES {
            entries.pop_front();
        }
    }

    /// Returns history for a specific provider.
    pub fn get(&self, kind: ProviderKind) -> Option<&VecDeque<HistoryEntry>> {
        self.entries.get(&kind)
    }

    /// Returns history entries within a time range.
    pub fn get_range(
        &self,
        kind: ProviderKind,
        start: DateTime<Utc>,
        end: DateTime<Utc>,
    ) -> Vec<&HistoryEntry> {
        self.entries
            .get(&kind)
            .map(|entries| {
                entries
                    .iter()
                    .filter(|e| e.timestamp >= start && e.timestamp <= end)
                    .collect()
            })
            .unwrap_or_default()
    }

    /// Returns the most recent entry for a provider.
    pub fn latest(&self, kind: ProviderKind) -> Option<&HistoryEntry> {
        self.entries.get(&kind).and_then(|e| e.back())
    }

    /// Clears history for a specific provider.
    pub fn clear_provider(&mut self, kind: ProviderKind) {
        self.entries.remove(&kind);
    }

    /// Clears all history.
    pub fn clear_all(&mut self) {
        self.entries.clear();
    }
}
