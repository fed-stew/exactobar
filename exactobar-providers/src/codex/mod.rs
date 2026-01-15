//! Codex (OpenAI) provider implementation.
//!
//! Codex is OpenAI's CLI tool for interacting with GPT models.
//! This provider supports multiple fetch strategies:
//!
//! ## Fetch Strategies
//!
//! 1. **RPC Strategy** (priority 100): JSON-RPC to `codex app-server`
//!    - Spawns `codex -s read-only -a untrusted app-server`
//!    - Sends JSON-RPC messages over stdin/stdout
//!    - Methods: `initialize`, `account/rateLimits/read`, `account/read`
//!
//! 2. **PTY Strategy** (priority 90): Interactive `/status` command
//!    - Falls back to running `codex` interactively
//!    - Sends `/status` command and parses TUI output
//!    - Parses patterns like "5h limit: XX% left"
//!
//! 3. **CLI Strategy** (priority 80): `codex usage --json`
//!    - Legacy strategy using JSON output
//!
//! 4. **API Strategy** (priority 60): OpenAI API with API key
//!    - Validates API key but can't get usage data
//!
//! ## Authentication
//!
//! - Reads `~/.codex/auth.json` for account info
//! - Extracts email and plan from JWT tokens
//!
//! ## Usage
//!
//! ```ignore
//! use exactobar_providers::codex::CodexUsageFetcher;
//!
//! let fetcher = CodexUsageFetcher::new();
//! let snapshot = fetcher.fetch_usage().await?;
//! ```

// Modules
mod auth;
mod descriptor;
mod error;
mod fetcher;
#[allow(unused)] // Parser has test utilities
pub(crate) mod parser;
mod pty_probe;
mod rpc;
mod strategies;

// Re-exports
pub use auth::{read_account_info, try_read_account_info, AccountInfo};
pub use descriptor::codex_descriptor;
pub use error::CodexError;
pub use fetcher::CodexUsageFetcher;
pub use pty_probe::{parse_status_output, CodexPtyProbe, CodexStatusSnapshot};
pub use rpc::{CodexRpcClient, RateLimits, RateLimitsResult};
pub use strategies::{CodexApiStrategy, CodexCliStrategy, CodexPtyStrategy, CodexRpcStrategy};
