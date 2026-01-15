//! Core error types for `ExactoBar`.

use thiserror::Error;

/// Core error type for `ExactoBar` operations.
#[derive(Debug, Error)]
pub enum CoreError {
    /// Provider not found or not configured.
    #[error("Provider not found: {0}")]
    ProviderNotFound(String),

    /// Invalid configuration.
    #[error("Invalid configuration: {0}")]
    InvalidConfig(String),

    /// Invalid data from API response.
    #[error("Invalid data: {0}")]
    InvalidData(String),

    /// Serialization/deserialization error.
    #[error("Serialization error: {0}")]
    Serialization(#[from] serde_json::Error),

    /// Generic error with message.
    #[error("{0}")]
    Other(String),
}
