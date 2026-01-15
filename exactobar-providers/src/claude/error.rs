//! Claude-specific error types.

use std::time::Duration;
use thiserror::Error;

/// Errors specific to Claude operations.
#[derive(Debug, Error)]
pub enum ClaudeError {
    /// Claude binary not found on PATH.
    #[error("Claude binary not found: {0}")]
    BinaryNotFound(String),

    /// OAuth credentials not found.
    #[error("OAuth credentials not found")]
    CredentialsNotFound,

    /// OAuth token expired.
    #[error("OAuth token expired at {0}")]
    TokenExpired(String),

    /// OAuth token missing required scope.
    #[error("OAuth token missing scope: {0}")]
    MissingScope(String),

    /// Failed to load credentials.
    #[error("Failed to load credentials: {0}")]
    CredentialsLoadError(String),

    /// API request failed.
    #[error("API request failed: {0}")]
    ApiError(String),

    /// Authentication failed.
    #[error("Authentication failed: {0}")]
    AuthenticationFailed(String),

    /// Operation timed out.
    #[error("Operation timed out after {0:?}")]
    Timeout(Duration),

    /// PTY operation failed.
    #[error("PTY error: {0}")]
    PtyError(String),

    /// Failed to parse output.
    #[error("Parse error: {0}")]
    ParseError(String),

    /// No data available.
    #[error("No usage data available")]
    NoData,

    /// Browser cookie error.
    #[error("Browser cookie error: {0}")]
    BrowserError(String),

    /// All strategies failed.
    #[error("All fetch strategies failed")]
    AllStrategiesFailed,

    /// IO error.
    #[error("IO error: {0}")]
    IoError(String),

    /// HTTP error.
    #[error("HTTP error: {0}")]
    HttpError(String),
}

impl From<std::io::Error> for ClaudeError {
    fn from(e: std::io::Error) -> Self {
        ClaudeError::IoError(e.to_string())
    }
}

impl From<serde_json::Error> for ClaudeError {
    fn from(e: serde_json::Error) -> Self {
        ClaudeError::ParseError(e.to_string())
    }
}

impl From<exactobar_fetch::PtyError> for ClaudeError {
    fn from(e: exactobar_fetch::PtyError) -> Self {
        ClaudeError::PtyError(e.to_string())
    }
}

impl From<reqwest::Error> for ClaudeError {
    fn from(e: reqwest::Error) -> Self {
        ClaudeError::HttpError(e.to_string())
    }
}
