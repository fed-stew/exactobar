//! Claude (Anthropic) provider implementation.
//!
//! Claude is Anthropic's AI assistant. This provider supports multiple
//! fetch strategies:
//!
//! ## Fetch Strategies
//!
//! 1. **OAuth API** (priority 100): Uses OAuth tokens for API access
//!    - Reads credentials from macOS Keychain or `~/.claude/.credentials.json`
//!    - Calls `https://api.anthropic.com/v1/usage`
//!
//! 2. **Web API** (priority 60): Uses browser cookies for claude.ai
//!    - Imports cookies from Chrome, Firefox, Safari, etc.
//!    - Calls `https://claude.ai/api/organizations/<org>/usage`
//!
//! 3. **PTY Fallback** (priority 40): Interactive `/usage` command
//!    - Runs `claude` interactively and parses output
//!    - Parses patterns like "72% left", "Resets 2pm (PST)"
//!
//! ## OAuth Credentials
//!
//! Credentials are stored in:
//! - macOS Keychain: service="Claude Code-credentials"
//! - File: `~/.claude/.credentials.json`
//!
//! Format:
//! ```json
//! {
//!   "claudeAiOauth": {
//!     "accessToken": "...",
//!     "refreshToken": "...",
//!     "expiresAt": 1735000000000,
//!     "scopes": ["user:profile"]
//!   }
//! }
//! ```
//!
//! ## Usage
//!
//! ```ignore
//! use exactobar_providers::claude::ClaudeUsageFetcher;
//!
//! let fetcher = ClaudeUsageFetcher::new();
//! let snapshot = fetcher.fetch_usage().await?;
//! ```

// Modules
mod api;
mod descriptor;
mod error;
mod fetcher;
mod oauth;
pub(crate) mod parser;
mod pty_probe;
mod strategies;
mod web;

// Re-exports
pub use api::{ClaudeApiClient, UsageApiResponse};
pub use descriptor::claude_descriptor;
pub use error::ClaudeError;
pub use fetcher::{ClaudeDataSource, ClaudeUsageFetcher};
pub use oauth::{ClaudeOAuthCredentials, CredentialSource};
pub use pty_probe::{parse_usage_output, ClaudePtyProbe, ClaudeStatusSnapshot};
pub use strategies::{
    ClaudeCliStrategy, ClaudeOAuthStrategy, ClaudePtyStrategy, ClaudeWebStrategy,
};
pub use web::ClaudeWebClient;
