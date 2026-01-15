//! VertexAI-specific errors.

use thiserror::Error;

/// VertexAI-specific errors.
#[derive(Debug, Error)]
pub enum VertexAIError {
    /// Not logged in / no OAuth credentials available.
    #[error("Not logged in: run `gcloud auth application-default login`")]
    NotLoggedIn,

    /// No project configured.
    #[error("No project configured: set `quota_project_id` in ADC or run `gcloud config set project <project>`")]
    NoProject,

    /// Failed to parse credentials or response.
    #[error("Parse error: {0}")]
    ParseError(String),

    /// API request failed.
    #[error("API error: {0}")]
    ApiError(String),

    /// Request timed out.
    #[error("Request timed out")]
    Timeout,

    /// HTTP request failed.
    #[error("HTTP request failed: {0}")]
    HttpError(String),

    /// Authentication failed.
    #[error("Authentication failed: {0}")]
    AuthenticationFailed(String),

    /// No gcloud credentials (legacy alias for NotLoggedIn).
    #[error("No gcloud credentials found")]
    NoCredentials,

    /// Invalid response.
    #[error("Invalid response: {0}")]
    InvalidResponse(String),

    /// Log file not found.
    #[error("Log file not found: {0}")]
    LogNotFound(String),

    /// Log parse error.
    #[error("Log parse error: {0}")]
    LogParseError(String),

    /// No usage data.
    #[error("No usage data available")]
    NoData,

    /// All strategies failed.
    #[error("All fetch strategies failed")]
    AllStrategiesFailed,
}

impl From<reqwest::Error> for VertexAIError {
    fn from(err: reqwest::Error) -> Self {
        if err.is_timeout() {
            VertexAIError::Timeout
        } else {
            VertexAIError::HttpError(err.to_string())
        }
    }
}
