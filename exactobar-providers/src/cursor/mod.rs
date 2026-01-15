//! Cursor IDE provider implementation.
//!
//! Cursor is an AI-powered code editor. This provider supports:
//!
//! - Web cookies from cursor.com session
//! - Local settings reading
//!
//! ## Fetch Strategies
//!
//! 1. **Web Strategy** (priority 100): Uses browser cookies for cursor.com API
//! 2. **Local Strategy** (priority 60): Reads local Cursor settings/cache
//!
//! ## Cookie Domains
//!
//! - `cursor.com`
//! - `www.cursor.com`
//!
//! ## API Endpoints
//!
//! - `https://www.cursor.com/api/usage` - Get usage data
//! - `https://www.cursor.com/api/auth/me` - Get account info
//!
//! ## Usage
//!
//! ```ignore
//! use exactobar_providers::cursor::CursorUsageFetcher;
//!
//! let fetcher = CursorUsageFetcher::new();
//! let snapshot = fetcher.fetch_usage().await?;
//! ```

// Modules
mod descriptor;
mod error;
mod fetcher;
mod local;
pub(crate) mod parser;
mod strategies;
mod web;

// Re-exports
pub use descriptor::cursor_descriptor;
pub use error::CursorError;
pub use fetcher::{CursorDataSource, CursorUsageFetcher};
pub use local::CursorLocalReader;
pub use strategies::{CursorLocalStrategy, CursorWebStrategy};
pub use web::{CursorUsageResponse, CursorWebClient};
