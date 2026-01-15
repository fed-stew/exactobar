//! z.ai-specific errors.

use thiserror::Error;

/// z.ai-specific errors.
#[derive(Debug, Error)]
pub enum ZaiError {
    /// HTTP request failed.
    #[error("HTTP request failed: {0}")]
    HttpError(String),

    /// Authentication failed.
    #[error("Authentication failed: {0}")]
    AuthenticationFailed(String),

    /// No API token found.
    #[error("No API token found")]
    NoToken,

    /// Invalid response.
    #[error("Invalid response: {0}")]
    InvalidResponse(String),

    /// Keychain error.
    #[error("Keychain error: {0}")]
    KeychainError(String),

    /// No usage data.
    #[error("No usage data available")]
    NoData,

    /// All strategies failed.
    #[error("All fetch strategies failed")]
    AllStrategiesFailed,
}

impl From<reqwest::Error> for ZaiError {
    fn from(err: reqwest::Error) -> Self {
        ZaiError::HttpError(err.to_string())
    }
}
