//! Store error types.

use thiserror::Error;

/// Errors that can occur in the store.
#[derive(Debug, Error)]
pub enum StoreError {
    /// Provider not found.
    #[error("Provider not found: {0}")]
    ProviderNotFound(String),

    /// Provider not enabled.
    #[error("Provider not enabled: {0}")]
    ProviderNotEnabled(String),

    /// Refresh already in progress.
    #[error("Refresh already in progress for {0}")]
    RefreshInProgress(String),

    /// Fetch error.
    #[error("Fetch failed: {0}")]
    FetchFailed(String),

    /// IO error.
    #[error("IO error: {0}")]
    Io(#[from] std::io::Error),

    /// Serialization error.
    #[error("Serialization error: {0}")]
    Serialization(#[from] serde_json::Error),

    /// Configuration error.
    #[error("Configuration error: {0}")]
    Config(String),

    /// Timeout error.
    #[error("Operation timed out")]
    Timeout,

    /// Parse error.
    #[error("Parse error: {0}")]
    Parse(String),
}

impl StoreError {
    /// Returns true if this is a transient error that might succeed on retry.
    pub fn is_transient(&self) -> bool {
        matches!(
            self,
            StoreError::FetchFailed(_) | StoreError::Timeout | StoreError::Io(_)
        )
    }
}
