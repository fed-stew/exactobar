//! Copilot-specific errors.

use thiserror::Error;

/// Copilot-specific errors.
#[derive(Debug, Error)]
pub enum CopilotError {
    /// HTTP request failed.
    #[error("HTTP request failed: {0}")]
    HttpError(String),

    /// Authentication failed.
    #[error("Authentication failed: {0}")]
    AuthenticationFailed(String),

    /// No OAuth token found.
    #[error("No OAuth token found")]
    NoToken,

    /// Token expired.
    #[error("Token expired: {0}")]
    TokenExpired(String),

    /// Device flow failed.
    #[error("Device flow failed: {0}")]
    DeviceFlowFailed(String),

    /// Device flow expired.
    #[error("Device flow expired - user did not authorize in time")]
    DeviceFlowExpired,

    /// Invalid response from API.
    #[error("Invalid response: {0}")]
    InvalidResponse(String),

    /// Rate limited by API.
    #[error("Rate limited: {0}")]
    RateLimited(String),

    /// Keychain access failed.
    #[error("Keychain error: {0}")]
    KeychainError(String),

    /// No usage data available.
    #[error("No usage data available")]
    NoData,

    /// All fetch strategies failed.
    #[error("All fetch strategies failed")]
    AllStrategiesFailed,

    /// Copilot not enabled for user.
    #[error("Copilot not enabled for this account")]
    NotEnabled,
}

impl From<reqwest::Error> for CopilotError {
    fn from(err: reqwest::Error) -> Self {
        if err.is_timeout() {
            CopilotError::HttpError(format!("Request timed out: {}", err))
        } else if err.is_connect() {
            CopilotError::HttpError(format!("Connection failed: {}", err))
        } else {
            CopilotError::HttpError(err.to_string())
        }
    }
}
