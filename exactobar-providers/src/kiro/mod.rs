//! Kiro provider implementation.
//!
//! Kiro uses CLI-based usage: `kiro-cli /usage`

mod cli;
mod descriptor;
mod error;
mod fetcher;
pub(crate) mod parser;
mod strategies;

pub use cli::{detect_version, ensure_logged_in, KiroCliClient, KiroUsage};
pub use descriptor::kiro_descriptor;
pub use error::KiroError;
pub use fetcher::KiroUsageFetcher;
pub use strategies::KiroCliStrategy;
