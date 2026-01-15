//! Kiro-specific errors.

use thiserror::Error;

/// Kiro-specific errors.
#[derive(Debug, Error)]
pub enum KiroError {
    /// CLI not found.
    #[error("kiro-cli not found")]
    CliNotFound,

    /// User is not logged in.
    #[error("Not logged in to Kiro")]
    NotLoggedIn,

    /// CLI command failed.
    #[error("CLI command failed: {0}")]
    CliFailed(String),

    /// Parse error.
    #[error("Parse error: {0}")]
    ParseError(String),

    /// No usage data.
    #[error("No usage data available")]
    NoData,

    /// Command timed out.
    #[error("Command timed out")]
    Timeout,

    /// All strategies failed.
    #[error("All fetch strategies failed")]
    AllStrategiesFailed,
}
