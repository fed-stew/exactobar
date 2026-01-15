//! Gemini-specific errors.

use thiserror::Error;

/// Gemini-specific errors.
#[derive(Debug, Error)]
pub enum GeminiError {
    /// HTTP request failed.
    #[error("HTTP request failed: {0}")]
    HttpError(String),

    /// Authentication failed.
    #[error("Authentication failed: {0}")]
    AuthenticationFailed(String),

    /// No gcloud credentials found.
    #[error("No gcloud credentials found")]
    NoCredentials,

    /// Not logged in (no credentials file or empty token).
    #[error("Not logged in to Gemini CLI")]
    NotLoggedIn,

    /// Unsupported auth type (API key, Vertex AI).
    #[error("Unsupported auth type: {0}")]
    UnsupportedAuthType(String),

    /// Token expired.
    #[error("Token expired: {0}")]
    TokenExpired(String),

    /// Token refresh failed.
    #[error("Token refresh failed: {0}")]
    RefreshFailed(String),

    /// Invalid response from API.
    #[error("Invalid response: {0}")]
    InvalidResponse(String),

    /// Rate limited by API.
    #[error("Rate limited: {0}")]
    RateLimited(String),

    /// gcloud CLI not found.
    #[error("gcloud CLI not found")]
    GcloudNotFound,

    /// gcloud command failed.
    #[error("gcloud command failed: {0}")]
    GcloudError(String),

    /// ADC file not found.
    #[error("Application default credentials not found")]
    AdcNotFound,

    /// Failed to parse credentials.
    #[error("Credentials parse error: {0}")]
    CredentialsParseError(String),

    /// No quota data available.
    #[error("No quota data available")]
    NoData,

    /// All fetch strategies failed.
    #[error("All fetch strategies failed")]
    AllStrategiesFailed,

    /// Gemini CLI not available.
    #[error("Gemini CLI not available")]
    CliNotFound,

    /// Request timed out.
    #[error("Request timed out")]
    Timeout,
}

impl From<reqwest::Error> for GeminiError {
    fn from(err: reqwest::Error) -> Self {
        if err.is_timeout() {
            GeminiError::HttpError(format!("Request timed out: {}", err))
        } else if err.is_connect() {
            GeminiError::HttpError(format!("Connection failed: {}", err))
        } else {
            GeminiError::HttpError(err.to_string())
        }
    }
}
