//! Fetch error types.

use std::time::Duration;
use thiserror::Error;

// ============================================================================
// Main Fetch Error
// ============================================================================

/// Error type for fetch operations.
#[derive(Debug, Error)]
pub enum FetchError {
    /// HTTP request failed.
    #[error("HTTP error: {0}")]
    Http(#[from] reqwest::Error),

    /// Request timed out.
    #[error("Request timed out after {0} seconds")]
    Timeout(u64),

    /// Rate limited by the provider.
    #[error("Rate limited, retry after {retry_after:?} seconds")]
    RateLimited {
        /// Seconds to wait before retrying.
        retry_after: Option<u64>,
    },

    /// Authentication failed.
    #[error("Authentication failed: {0}")]
    AuthenticationFailed(String),

    /// Invalid response from the provider.
    #[error("Invalid response: {0}")]
    InvalidResponse(String),

    /// JSON parsing error.
    #[error("JSON error: {0}")]
    Json(#[from] serde_json::Error),

    /// Core error.
    #[error("Core error: {0}")]
    Core(#[from] exactobar_core::CoreError),

    /// Keychain error.
    #[error("Keychain error: {0}")]
    Keychain(#[from] KeychainError),

    /// Process error.
    #[error("Process error: {0}")]
    Process(#[from] ProcessError),

    /// PTY error.
    #[error("PTY error: {0}")]
    Pty(#[from] PtyError),

    /// Browser error.
    #[error("Browser error: {0}")]
    Browser(#[from] BrowserError),

    /// Status page error.
    #[error("Status error: {0}")]
    Status(#[from] StatusError),

    /// Strategy not available.
    #[error("Strategy not available: {0}")]
    StrategyNotAvailable(String),

    /// All strategies failed.
    #[error("All strategies failed")]
    AllStrategiesFailed,

    /// Domain not allowed.
    #[error("Domain not allowed: {0}")]
    DomainNotAllowed(String),
}

// ============================================================================
// HTTP Error
// ============================================================================

/// HTTP-specific error type.
#[derive(Debug, Error)]
pub enum HttpError {
    /// Request error.
    #[error("Request error: {0}")]
    Request(#[from] reqwest::Error),

    /// Domain not allowed.
    #[error("Domain not allowed: {0}")]
    DomainNotAllowed(String),

    /// Invalid URL.
    #[error("Invalid URL: {0}")]
    InvalidUrl(String),

    /// Timeout.
    #[error("Request timed out")]
    Timeout,
}

// ============================================================================
// Keychain Error
// ============================================================================

/// Error type for keychain operations.
#[derive(Debug, Error)]
pub enum KeychainError {
    /// Credential not found.
    #[error("Credential not found for {service}/{account}")]
    NotFound {
        /// Service name.
        service: String,
        /// Account name.
        account: String,
    },

    /// Access denied.
    #[error("Access denied to keychain")]
    AccessDenied,

    /// Keychain unavailable.
    #[error("Keychain unavailable: {0}")]
    Unavailable(String),

    /// Platform error.
    #[error("Platform error: {0}")]
    Platform(String),

    /// Generic error.
    #[error("Keychain error: {0}")]
    Other(String),
}

impl From<keyring::Error> for KeychainError {
    fn from(err: keyring::Error) -> Self {
        match err {
            keyring::Error::NoEntry => KeychainError::NotFound {
                service: String::new(),
                account: String::new(),
            },
            keyring::Error::Ambiguous(_) => {
                KeychainError::Other("Ambiguous credential entry".to_string())
            }
            keyring::Error::PlatformFailure(e) => KeychainError::Platform(e.to_string()),
            keyring::Error::NoStorageAccess(_) => KeychainError::AccessDenied,
            _ => KeychainError::Other(err.to_string()),
        }
    }
}

// ============================================================================
// Process Error
// ============================================================================

/// Error type for process operations.
#[derive(Debug, Error)]
pub enum ProcessError {
    /// Command not found.
    #[error("Command not found: {0}")]
    NotFound(String),

    /// Command execution failed.
    #[error("Execution failed: {0}")]
    ExecutionFailed(String),

    /// Command timed out.
    #[error("Command timed out after {0:?}")]
    Timeout(Duration),

    /// Non-zero exit code.
    #[error("Command exited with code {code}: {stderr}")]
    NonZeroExit {
        /// Exit code from the process.
        code: i32,
        /// Standard error output.
        stderr: String,
    },

    /// IO error.
    #[error("IO error: {0}")]
    Io(#[from] std::io::Error),
}

// ============================================================================
// PTY Error
// ============================================================================

/// Error type for PTY operations.
#[derive(Debug, Error)]
pub enum PtyError {
    /// Command not found.
    #[error("Command not found: {0}")]
    NotFound(String),

    /// Failed to create PTY.
    #[error("Failed to create PTY: {0}")]
    CreateFailed(String),

    /// Failed to spawn process.
    #[error("Failed to spawn process: {0}")]
    SpawnFailed(String),

    /// Command timed out (total timeout).
    #[error("Command timed out after {0:?}")]
    Timeout(Duration),

    /// Command idle timed out (no output for too long).
    #[error("Command idle timed out after {0:?}")]
    IdleTimeout(Duration),

    /// Non-zero exit code.
    #[error("Command exited with code {code}: {output}")]
    NonZeroExit {
        /// Exit code from the process.
        code: i32,
        /// Command output.
        output: String,
    },

    /// IO error.
    #[error("IO error: {0}")]
    Io(#[from] std::io::Error),

    /// Stop pattern matched.
    #[error("Stopped on pattern: {pattern}")]
    StoppedOnPattern {
        /// The pattern that was matched.
        pattern: String,
        /// Command output up to the match.
        output: String,
    },

    /// PTY system unavailable.
    #[error("PTY system unavailable: {0}")]
    SystemUnavailable(String),
}

// ============================================================================
// Browser Error
// ============================================================================

/// Error type for browser cookie operations.
#[derive(Debug, Error)]
pub enum BrowserError {
    /// Browser not found.
    #[error("Browser not found: {0}")]
    BrowserNotFound(String),

    /// No browsers available.
    #[error("No browsers available")]
    NoBrowsersAvailable,

    /// Cookie database not found.
    #[error("Cookie database not found for {browser}: {path}")]
    DatabaseNotFound {
        /// Browser name.
        browser: String,
        /// Expected database path.
        path: String,
    },

    /// Failed to read cookies.
    #[error("Failed to read cookies: {0}")]
    ReadFailed(String),

    /// No cookies found for domain.
    #[error("No cookies found for domain: {0}")]
    NoCookiesFound(String),

    /// Cookie decryption failed.
    #[error("Cookie decryption failed: {0}")]
    DecryptionFailed(String),

    /// IO error.
    #[error("IO error: {0}")]
    Io(#[from] std::io::Error),
}

// ============================================================================
// Status Error
// ============================================================================

/// Error type for status page operations.
#[derive(Debug, Error)]
pub enum StatusError {
    /// HTTP error.
    #[error("HTTP error: {0}")]
    Http(#[from] reqwest::Error),

    /// Invalid status page response.
    #[error("Invalid status response: {0}")]
    InvalidResponse(String),

    /// Status page unavailable.
    #[error("Status page unavailable: {0}")]
    Unavailable(String),

    /// JSON parsing error.
    #[error("JSON error: {0}")]
    Json(#[from] serde_json::Error),
}
