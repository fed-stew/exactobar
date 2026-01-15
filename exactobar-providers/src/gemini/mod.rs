//! Gemini (Google) provider implementation.
//!
//! Gemini is Google's AI model. This provider supports:
//!
//! - OAuth via gcloud CLI credentials
//! - Application Default Credentials (ADC)
//! - Direct CLI fallback
//!
//! ## gcloud Credentials
//!
//! The provider looks for credentials in this order:
//!
//! 1. `gcloud auth print-access-token` (CLI)
//! 2. `~/.config/gcloud/credentials.db` (SQLite cache)
//! 3. `~/.config/gcloud/application_default_credentials.json` (ADC)
//!
//! ## ADC File Format
//!
//! ```json
//! {
//!   "client_id": "...",
//!   "client_secret": "...",
//!   "refresh_token": "...",
//!   "type": "authorized_user"
//! }
//! ```
//!
//! ## Fetch Strategies
//!
//! 1. **OAuth Strategy** (priority 100): Uses gcloud OAuth credentials
//! 2. **CLI Strategy** (priority 80): Uses `gemini` CLI if available
//!
//! ## API Endpoints
//!
//! - `GET /v1beta/models` - List available models
//! - Rate limit info comes from response headers
//!
//! ## Usage
//!
//! ```ignore
//! use exactobar_providers::gemini::GeminiUsageFetcher;
//!
//! let fetcher = GeminiUsageFetcher::new();
//! let snapshot = fetcher.fetch_usage().await?;
//! ```

// Modules
mod api;
mod descriptor;
mod error;
mod fetcher;
pub mod gcloud;
pub(crate) mod parser;
mod probe;
mod pty_probe;
mod strategies;

// Re-exports
pub use api::{GeminiApiClient, GeminiQuota};
pub use descriptor::gemini_descriptor;
pub use error::GeminiError;
pub use fetcher::{GeminiDataSource, GeminiUsageFetcher};
pub use gcloud::{AdcCredentials, GcloudCredentials, GcloudToken};
pub use probe::{GeminiAuthType, GeminiCredentials, GeminiModelQuota, GeminiProbe, GeminiSnapshot};
pub use pty_probe::{GeminiCliQuota, GeminiPtyProbe};
pub use strategies::{GeminiCliStrategy, GeminiOAuthStrategy};
