//! Factory-specific errors.

use thiserror::Error;

/// Factory-specific errors.
#[derive(Debug, Error)]
pub enum FactoryError {
    /// HTTP request failed.
    #[error("HTTP request failed: {0}")]
    HttpError(String),

    /// Authentication failed.
    #[error("Authentication failed: {0}")]
    AuthenticationFailed(String),

    /// No session cookie found.
    #[error("No session cookie found")]
    NoSessionCookie,

    /// WorkOS token not found.
    #[error("WorkOS token not found")]
    NoWorkOSToken,

    /// Invalid response from API.
    #[error("Invalid response: {0}")]
    InvalidResponse(String),

    /// Browser cookie import failed.
    #[error("Browser error: {0}")]
    BrowserError(String),

    /// Local config not found.
    #[error("Local config not found: {0}")]
    ConfigNotFound(String),

    /// No usage data available.
    #[error("No usage data available")]
    NoData,

    /// All fetch strategies failed.
    #[error("All fetch strategies failed")]
    AllStrategiesFailed,
}

impl From<reqwest::Error> for FactoryError {
    fn from(err: reqwest::Error) -> Self {
        FactoryError::HttpError(err.to_string())
    }
}
