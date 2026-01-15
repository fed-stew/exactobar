//! Augment provider implementation.
//!
//! Augment uses web cookies with session keepalive.
//!
//! Cookie domain: `augmentcode.com`

mod descriptor;
mod error;
mod fetcher;
pub(crate) mod parser;
mod strategies;
mod web;

pub use descriptor::augment_descriptor;
pub use error::AugmentError;
pub use fetcher::AugmentUsageFetcher;
pub use strategies::AugmentWebStrategy;
pub use web::{AugmentUsageResponse, AugmentWebClient};
