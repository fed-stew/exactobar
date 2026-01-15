//! Factory (Droid) provider implementation.
//!
//! Factory uses WorkOS for authentication. Supports:
//! - Web cookies from browser
//! - WorkOS token from local storage

mod descriptor;
mod error;
mod fetcher;
pub(crate) mod parser;
mod strategies;
mod web;

pub use descriptor::factory_descriptor;
pub use error::FactoryError;
pub use fetcher::{FactoryDataSource, FactoryUsageFetcher};
pub use strategies::{FactoryLocalStrategy, FactoryWebStrategy};
pub use web::{FactoryUsageResponse, FactoryWebClient};
