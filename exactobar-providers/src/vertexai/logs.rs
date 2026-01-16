//! VertexAI/Claude log reader for token cost tracking.
//!
//! Reads Claude usage logs from local storage to track token costs.
//! Log path: `~/.local/share/claude/logs/*.jsonl`

use std::path::PathBuf;

use chrono::{DateTime, Utc};
use serde::Deserialize;
use tracing::{debug, instrument, warn};

use super::error::VertexAIError;

// ============================================================================
// Log Entry Types
// ============================================================================

/// A single log entry from Claude logs.
#[derive(Debug, Deserialize)]
pub struct ClaudeLogEntry {
    /// Timestamp.
    #[serde(default)]
    pub timestamp: Option<String>,

    /// Model used.
    #[serde(default)]
    pub model: Option<String>,

    /// Input tokens.
    #[serde(default, alias = "input_tokens")]
    pub input_tokens: Option<u64>,

    /// Output tokens.
    #[serde(default, alias = "output_tokens")]
    pub output_tokens: Option<u64>,

    /// Total tokens.
    #[serde(default, alias = "total_tokens")]
    pub total_tokens: Option<u64>,

    /// Cost in USD.
    #[serde(default)]
    pub cost_usd: Option<f64>,

    /// Request type.
    #[serde(default, alias = "request_type")]
    pub request_type: Option<String>,
}

/// Aggregated token usage from logs.
#[derive(Debug, Default)]
pub struct TokenUsage {
    /// Total input tokens.
    pub input_tokens: u64,

    /// Total output tokens.
    pub output_tokens: u64,

    /// Total tokens.
    pub total_tokens: u64,

    /// Total cost in USD.
    pub total_cost_usd: f64,

    /// Number of requests.
    pub request_count: u64,

    /// Earliest entry.
    pub earliest: Option<DateTime<Utc>>,

    /// Latest entry.
    pub latest: Option<DateTime<Utc>>,
}

impl TokenUsage {
    /// Check if we have any data.
    pub fn has_data(&self) -> bool {
        self.request_count > 0
    }

    /// Add a log entry to the aggregation.
    pub fn add_entry(&mut self, entry: &ClaudeLogEntry) {
        self.input_tokens += entry.input_tokens.unwrap_or(0);
        self.output_tokens += entry.output_tokens.unwrap_or(0);
        self.total_tokens += entry.total_tokens.unwrap_or(
            entry.input_tokens.unwrap_or(0) + entry.output_tokens.unwrap_or(0),
        );
        self.total_cost_usd += entry.cost_usd.unwrap_or(0.0);
        self.request_count += 1;

        // Track time range
        if let Some(ref ts) = entry.timestamp {
            if let Ok(dt) = chrono::DateTime::parse_from_rfc3339(ts) {
                let dt = dt.with_timezone(&Utc);
                if self.earliest.is_none() || Some(dt) < self.earliest {
                    self.earliest = Some(dt);
                }
                if self.latest.is_none() || Some(dt) > self.latest {
                    self.latest = Some(dt);
                }
            }
        }
    }
}

// ============================================================================
// Log Reader
// ============================================================================

/// Claude log reader for token cost tracking.
#[derive(Debug, Clone, Default)]
pub struct ClaudeLogReader;

impl ClaudeLogReader {
    /// Creates a new log reader.
    pub fn new() -> Self {
        Self
    }

    /// Get Claude log directory.
    #[cfg(target_os = "macos")]
    pub fn log_dir() -> Option<PathBuf> {
        let home = dirs::home_dir()?;
        Some(home.join(".local/share/claude/logs"))
    }

    /// Get Claude log directory.
    #[cfg(target_os = "linux")]
    pub fn log_dir() -> Option<PathBuf> {
        let data_dir = dirs::data_local_dir()?;
        Some(data_dir.join("claude/logs"))
    }

    /// Get Claude log directory.
    #[cfg(target_os = "windows")]
    pub fn log_dir() -> Option<PathBuf> {
        let data_dir = dirs::data_local_dir()?;
        Some(data_dir.join("claude\\logs"))
    }

    /// Check if log directory exists.
    pub fn has_logs() -> bool {
        Self::log_dir().is_some_and(|p| p.exists())
    }

    /// Read token usage from logs.
    #[instrument(skip(self))]
    pub fn read_usage(&self, since: Option<DateTime<Utc>>) -> Result<TokenUsage, VertexAIError> {
        debug!("Reading Claude logs");

        let log_dir = Self::log_dir().ok_or_else(|| {
            VertexAIError::LogNotFound("Log directory not found".to_string())
        })?;

        if !log_dir.exists() {
            return Err(VertexAIError::LogNotFound(
                format!("Log directory does not exist: {}", log_dir.display()),
            ));
        }

        let mut usage = TokenUsage::default();

        // Read all .jsonl files
        let entries = std::fs::read_dir(&log_dir).map_err(|e| {
            VertexAIError::LogParseError(format!("Failed to read log dir: {}", e))
        })?;

        for entry in entries.flatten() {
            let path = entry.path();
            if path.extension().and_then(|e| e.to_str()) != Some("jsonl") {
                continue;
            }

            if let Err(e) = self.read_log_file(&path, &mut usage, since) {
                warn!(path = %path.display(), error = %e, "Failed to read log file");
            }
        }

        if !usage.has_data() {
            return Err(VertexAIError::NoData);
        }

        Ok(usage)
    }

    /// Read a single log file.
    fn read_log_file(
        &self,
        path: &PathBuf,
        usage: &mut TokenUsage,
        since: Option<DateTime<Utc>>,
    ) -> Result<(), VertexAIError> {
        let content = std::fs::read_to_string(path).map_err(|e| {
            VertexAIError::LogParseError(format!("Failed to read file: {}", e))
        })?;

        for line in content.lines() {
            if line.trim().is_empty() {
                continue;
            }

            let entry: ClaudeLogEntry = match serde_json::from_str(line) {
                Ok(e) => e,
                Err(_) => continue, // Skip malformed lines
            };

            // Filter by time if specified
            if let Some(since) = since {
                if let Some(ref ts) = entry.timestamp {
                    if let Ok(dt) = chrono::DateTime::parse_from_rfc3339(ts) {
                        if dt.with_timezone(&Utc) < since {
                            continue;
                        }
                    }
                }
            }

            usage.add_entry(&entry);
        }

        Ok(())
    }

    /// Read usage for today only.
    pub fn read_today_usage(&self) -> Result<TokenUsage, VertexAIError> {
        let today_start = Utc::now().date_naive().and_hms_opt(0, 0, 0)
            .map(|dt| dt.and_utc());
        self.read_usage(today_start)
    }
}

// ============================================================================
// Tests
// ============================================================================

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_log_reader_creation() {
        let reader = ClaudeLogReader::new();
        assert!(std::mem::size_of_val(&reader) == 0);
    }

    #[test]
    fn test_log_dir() {
        let dir = ClaudeLogReader::log_dir();
        assert!(dir.is_some());
    }

    #[test]
    fn test_has_logs() {
        let _ = ClaudeLogReader::has_logs();
    }

    #[test]
    fn test_token_usage_has_data() {
        let empty = TokenUsage::default();
        assert!(!empty.has_data());

        let mut with_data = TokenUsage::default();
        with_data.request_count = 1;
        assert!(with_data.has_data());
    }

    #[test]
    fn test_parse_log_entry() {
        let json = r#"{
            "timestamp": "2025-01-01T00:00:00Z",
            "model": "claude-3-opus",
            "input_tokens": 100,
            "output_tokens": 50,
            "cost_usd": 0.01
        }"#;

        let entry: ClaudeLogEntry = serde_json::from_str(json).unwrap();
        assert_eq!(entry.input_tokens, Some(100));
        assert_eq!(entry.output_tokens, Some(50));
    }

    #[test]
    fn test_add_entry() {
        let mut usage = TokenUsage::default();
        let entry = ClaudeLogEntry {
            timestamp: None,
            model: Some("claude-3".to_string()),
            input_tokens: Some(100),
            output_tokens: Some(50),
            total_tokens: None,
            cost_usd: Some(0.01),
            request_type: None,
        };

        usage.add_entry(&entry);
        assert_eq!(usage.input_tokens, 100);
        assert_eq!(usage.output_tokens, 50);
        assert_eq!(usage.total_tokens, 150);
        assert_eq!(usage.request_count, 1);
    }
}
