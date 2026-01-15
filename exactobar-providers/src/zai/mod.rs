//! z.ai provider implementation.
//!
//! z.ai uses API tokens stored in keychain.
//!
//! Keychain services: `exactobar:zai`, `codexbar:zai`, `zai:api`

mod api;
mod descriptor;
mod error;
mod fetcher;
pub(crate) mod parser;
mod strategies;
mod token_store;

pub use api::{ZaiApiClient, ZaiUsageResponse};
pub use token_store::ZaiTokenStore;
pub use descriptor::zai_descriptor;
pub use error::ZaiError;
pub use fetcher::ZaiUsageFetcher;
pub use strategies::ZaiApiStrategy;
