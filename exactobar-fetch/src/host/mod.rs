//! Host APIs for ExactoBar fetch strategies.
//!
//! This module provides abstractions for interacting with external systems:
//!
//! - [`keychain`] - Secure credential storage (system keychain)
//! - [`http`] - HTTP client with tracing and domain allowlist
//! - [`process`] - Subprocess execution for CLI tools
//! - [`pty`] - PTY-based execution for interactive CLI tools
//! - [`status`] - Status page polling (statuspage.io)
//! - [`browser`] - Browser cookie import

pub mod browser;
pub mod http;
pub mod keychain;
pub mod process;
pub mod pty;
pub mod status;

// Re-export key types
pub use browser::{Browser, BrowserCookieImporter, Cookie};
pub use http::HttpClient;
pub use keychain::{KeychainApi, SystemKeychain};
pub use process::{ProcessOutput, ProcessRunner};
pub use pty::{PtyOptions, PtyResult, PtyRunner};
pub use status::StatusPoller;
