//! Codex-specific error types.

use std::time::Duration;
use thiserror::Error;

/// Errors specific to Codex operations.
#[derive(Debug, Error)]
pub enum CodexError {
    /// Codex binary not found on PATH.
    #[error("Codex binary not found: {0}")]
    BinaryNotFound(String),

    /// Failed to spawn Codex process.
    #[error("Failed to spawn Codex: {0}")]
    SpawnFailed(String),

    /// RPC not initialized.
    #[error("RPC client not initialized - call initialize() first")]
    NotInitialized,

    /// RPC operation timed out.
    #[error("RPC operation timed out after {0:?}")]
    Timeout(Duration),

    /// RPC connection closed unexpectedly.
    #[error("RPC connection closed unexpectedly")]
    ConnectionClosed,

    /// RPC returned an error.
    #[error("RPC error ({code}): {message}")]
    RpcError {
        /// RPC error code.
        code: i32,
        /// RPC error message.
        message: String,
    },

    /// RPC returned an empty response.
    #[error("RPC returned empty response")]
    EmptyResponse,

    /// IO error.
    #[error("IO error: {0}")]
    IoError(String),

    /// Serialization error.
    #[error("Serialization error: {0}")]
    SerializationError(String),

    /// PTY operation failed.
    #[error("PTY error: {0}")]
    PtyError(String),

    /// Failed to parse output.
    #[error("Parse error: {0}")]
    ParseError(String),

    /// Auth file not found.
    #[error("Auth file not found: {0}")]
    AuthNotFound(String),

    /// Invalid auth file format.
    #[error("Invalid auth file: {0}")]
    InvalidAuth(String),

    /// JWT decode error.
    #[error("JWT decode error: {0}")]
    JwtError(String),

    /// No data available.
    #[error("No usage data available")]
    NoData,

    /// All strategies failed.
    #[error("All fetch strategies failed")]
    AllStrategiesFailed,
}

impl From<std::io::Error> for CodexError {
    fn from(e: std::io::Error) -> Self {
        CodexError::IoError(e.to_string())
    }
}

impl From<serde_json::Error> for CodexError {
    fn from(e: serde_json::Error) -> Self {
        CodexError::ParseError(e.to_string())
    }
}

impl From<exactobar_fetch::PtyError> for CodexError {
    fn from(e: exactobar_fetch::PtyError) -> Self {
        CodexError::PtyError(e.to_string())
    }
}
