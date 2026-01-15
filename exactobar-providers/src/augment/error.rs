//! Augment-specific errors.

use thiserror::Error;

/// Augment-specific errors.
#[derive(Debug, Error)]
pub enum AugmentError {
    /// HTTP request failed.
    #[error("HTTP request failed: {0}")]
    HttpError(String),

    /// Authentication failed.
    #[error("Authentication failed: {0}")]
    AuthenticationFailed(String),

    /// No session cookie.
    #[error("No session cookie found")]
    NoSessionCookie,

    /// Session expired.
    #[error("Session expired - keepalive failed")]
    SessionExpired,

    /// Invalid response.
    #[error("Invalid response: {0}")]
    InvalidResponse(String),

    /// Browser error.
    #[error("Browser error: {0}")]
    BrowserError(String),

    /// No usage data.
    #[error("No usage data available")]
    NoData,

    /// All strategies failed.
    #[error("All fetch strategies failed")]
    AllStrategiesFailed,
}

impl From<reqwest::Error> for AugmentError {
    fn from(err: reqwest::Error) -> Self {
        AugmentError::HttpError(err.to_string())
    }
}
