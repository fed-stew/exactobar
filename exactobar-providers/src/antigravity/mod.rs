//! Antigravity provider implementation.
//!
//! Antigravity uses local language server probe (no external auth).
//! Detects running process, extracts CSRF token, and queries gRPC-style API.

mod descriptor;
mod error;
mod fetcher;
mod probe;
mod strategies;

pub use descriptor::antigravity_descriptor;
pub use error::AntigravityError;
pub use fetcher::AntigravityUsageFetcher;
pub use probe::{AntigravityProbe, AntigravitySnapshot, ModelQuota};
pub use strategies::AntigravityLocalStrategy;
