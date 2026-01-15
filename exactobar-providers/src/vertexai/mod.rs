//! VertexAI provider implementation.
//!
//! VertexAI uses OAuth credentials from Application Default Credentials (ADC)
//! and can track token costs from local logs.
//!
//! ## Authentication
//!
//! This module reads OAuth credentials from:
//! 1. `GOOGLE_APPLICATION_CREDENTIALS` environment variable (if set)
//! 2. `~/.config/gcloud/application_default_credentials.json`
//!
//! Run `gcloud auth application-default login` to create credentials.
//!
//! ## Token Cost Tracking
//!
//! Log path: `~/.local/share/claude/logs/*.jsonl`

mod credentials;
mod descriptor;
mod error;
mod fetcher;
mod logs;
pub(crate) mod parser;
mod strategies;

pub use credentials::{VertexAICredentials, VertexAITokenRefresher};
pub use descriptor::vertexai_descriptor;
pub use error::VertexAIError;
pub use fetcher::{VertexAIDataSource, VertexAIUsageFetcher};
pub use logs::{ClaudeLogReader, TokenUsage};
pub use strategies::{VertexAILocalStrategy, VertexAIOAuthStrategy};
