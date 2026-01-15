//! MiniMax provider implementation.
//!
//! MiniMax uses multiple authentication sources:
//! - Web cookies from `minimax.chat`
//! - Web cookies from `hailuoai.com` (MiniMax's web interface)
//! - Browser localStorage tokens
//! - Local config file
//!
//! Cookie domains: `minimax.chat`, `hailuoai.com`

mod descriptor;
mod error;
mod fetcher;
pub(crate) mod parser;
mod strategies;
mod web;

pub use descriptor::minimax_descriptor;
pub use error::MiniMaxError;
pub use fetcher::{MiniMaxDataSource, MiniMaxUsageFetcher};
pub use strategies::{
    HailuoaiWebStrategy, MiniMaxLocalStorageStrategy, MiniMaxLocalStrategy, MiniMaxWebStrategy,
};
pub use web::{
    MiniMaxLocalStorage, MiniMaxTokenStore, MiniMaxUsageResponse, MiniMaxWebClient,
    HAILUOAI_DOMAIN, MINIMAX_DOMAIN,
};
