//! Cursor-specific errors.

use thiserror::Error;

/// Cursor-specific errors.
#[derive(Debug, Error)]
pub enum CursorError {
    /// HTTP request failed.
    #[error("HTTP request failed: {0}")]
    HttpError(String),

    /// Authentication failed.
    #[error("Authentication failed: {0}")]
    AuthenticationFailed(String),

    /// No valid session cookie found.
    #[error("No valid session cookie found")]
    NoSessionCookie,

    /// Invalid response from API.
    #[error("Invalid response: {0}")]
    InvalidResponse(String),

    /// Rate limited by API.
    #[error("Rate limited: {0}")]
    RateLimited(String),

    /// Browser cookie import failed.
    #[error("Browser error: {0}")]
    BrowserError(String),

    /// Local config not found.
    #[error("Local config not found: {0}")]
    ConfigNotFound(String),

    /// Failed to parse local config.
    #[error("Config parse error: {0}")]
    ConfigParseError(String),

    /// No usage data available.
    #[error("No usage data available")]
    NoData,

    /// All fetch strategies failed.
    #[error("All fetch strategies failed")]
    AllStrategiesFailed,
}

impl From<reqwest::Error> for CursorError {
    fn from(err: reqwest::Error) -> Self {
        if err.is_timeout() {
            CursorError::HttpError(format!("Request timed out: {}", err))
        } else if err.is_connect() {
            CursorError::HttpError(format!("Connection failed: {}", err))
        } else {
            CursorError::HttpError(err.to_string())
        }
    }
}
