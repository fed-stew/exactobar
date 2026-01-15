// Lint configuration for this crate
#![warn(missing_docs)]
#![warn(clippy::all)]
#![warn(clippy::pedantic)]
#![allow(clippy::module_name_repetitions)]
#![allow(clippy::must_use_candidate)]

//! # ExactoBar Fetch
//!
//! HTTP fetching strategies and host APIs for the ExactoBar application.
//!
//! This crate provides the infrastructure for fetching usage data from
//! various LLM providers. It includes:
//!
//! ## Host APIs
//!
//! The [`host`] module provides abstractions for system interactions:
//!
//! - [`host::keychain`] - Secure credential storage (system keychain)
//! - [`host::http`] - HTTP client with tracing and domain allowlist
//! - [`host::process`] - Subprocess execution for CLI tools
//! - [`host::pty`] - PTY-based execution for interactive CLI tools
//! - [`host::status`] - Status page polling (statuspage.io)
//! - [`host::browser`] - Browser cookie import for web scraping
//!
//! ## Fetch Pipeline
//!
//! The fetch pipeline executes multiple strategies in priority order:
//!
//! - [`strategy::FetchStrategy`] - Trait for fetch implementations
//! - [`pipeline::FetchPipeline`] - Executes strategies in order
//! - [`context::FetchContext`] - Provides access to host APIs
//!
//! ## Example
//!
//! ```ignore
//! use exactobar_fetch::{FetchContext, FetchPipeline};
//!
//! // Create a fetch context with default settings
//! let ctx = FetchContext::new();
//!
//! // Create a pipeline with provider strategies
//! let pipeline = FetchPipeline::with_strategies(vec![
//!     Box::new(ClaudeCliStrategy::new()),
//!     Box::new(ClaudeOAuthStrategy::new()),
//! ]);
//!
//! // Execute and get the result
//! let outcome = pipeline.execute(&ctx).await;
//! ```

// Core modules
pub mod client;
pub mod context;
pub mod error;
pub mod host;
pub mod pipeline;
pub mod probe;
pub mod retry;
pub mod strategy;

// Re-export key types at crate root

// Errors
pub use error::{
    BrowserError, FetchError, HttpError, KeychainError, ProcessError, PtyError, StatusError,
};

// Host APIs
pub use host::{
    browser::{Browser, BrowserCookieImporter, Cookie},
    http::HttpClient,
    keychain::{KeychainApi, SystemKeychain},
    process::{ProcessOutput, ProcessRunner},
    pty::{PtyOptions, PtyResult, PtyRunner},
    status::StatusPoller,
};

// Strategy & Pipeline
pub use context::{FetchContext, FetchContextBuilder, FetchSettings, SourceMode};
pub use pipeline::{FetchAttempt, FetchOutcome, FetchPipeline};
pub use strategy::{FetchKind, FetchResult, FetchStrategy, StrategyInfo};

// Legacy exports (for compatibility)
pub use client::HttpClient as LegacyHttpClient;
pub use probe::{Probe, ProbeResult};
pub use retry::RetryStrategy;
