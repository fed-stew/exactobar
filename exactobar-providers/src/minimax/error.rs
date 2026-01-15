//! MiniMax-specific errors.

use thiserror::Error;

/// MiniMax-specific errors.
#[derive(Debug, Error)]
pub enum MiniMaxError {
    /// HTTP request failed.
    #[error("HTTP request failed: {0}")]
    HttpError(String),

    /// Authentication failed.
    #[error("Authentication failed: {0}")]
    AuthenticationFailed(String),

    /// No session cookie.
    #[error("No session cookie found")]
    NoSessionCookie,

    /// No API token.
    #[error("No API token found")]
    NoToken,

    /// Invalid response.
    #[error("Invalid response: {0}")]
    InvalidResponse(String),

    /// Browser error.
    #[error("Browser error: {0}")]
    BrowserError(String),

    /// Local storage error.
    #[error("Local storage error: {0}")]
    LocalStorageError(String),

    /// No usage data.
    #[error("No usage data available")]
    NoData,

    /// All strategies failed.
    #[error("All fetch strategies failed")]
    AllStrategiesFailed,
}

impl From<reqwest::Error> for MiniMaxError {
    fn from(err: reqwest::Error) -> Self {
        MiniMaxError::HttpError(err.to_string())
    }
}
