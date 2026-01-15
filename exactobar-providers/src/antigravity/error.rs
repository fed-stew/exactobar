//! Antigravity-specific errors.

use thiserror::Error;

/// Antigravity-specific errors.
#[derive(Debug, Error)]
pub enum AntigravityError {
    /// App not running.
    #[error("Antigravity app not running")]
    NotRunning,

    /// Connection failed.
    #[error("Connection failed: {0}")]
    ConnectionFailed(String),

    /// Invalid response.
    #[error("Invalid response: {0}")]
    InvalidResponse(String),

    /// No usage data.
    #[error("No usage data available")]
    NoData,

    /// Port detection failed.
    #[error("Port detection failed: {0}")]
    PortDetectionFailed(String),

    /// CSRF token not found.
    #[error("CSRF token not found in process arguments")]
    CsrfTokenNotFound,

    /// API error from Antigravity server.
    #[error("API error: {0}")]
    ApiError(String),
}
