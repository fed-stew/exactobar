//! Copilot (GitHub) provider implementation.
//!
//! GitHub Copilot AI assistant. This provider supports:
//!
//! - GitHub Device Flow OAuth
//! - OAuth token from keychain
//! - gh CLI token
//! - Environment variable fallback (COPILOT_API_TOKEN, GITHUB_TOKEN)
//!
//! ## Device Flow
//!
//! ```ignore
//! use exactobar_providers::copilot::CopilotDeviceFlow;
//!
//! let flow = CopilotDeviceFlow::new();
//! let start = flow.start().await?;
//! println!("Go to {} and enter: {}", start.verification_uri, start.user_code);
//!
//! // Poll until user authorizes
//! loop {
//!     match flow.poll(&start.device_code).await? {
//!         DeviceFlowResult::Pending => sleep(5).await,
//!         DeviceFlowResult::AccessToken(token) => break,
//!         DeviceFlowResult::Expired => return Err("Expired"),
//!     }
//! }
//! ```
//!
//! ## Fetch Strategies
//!
//! 1. **API Strategy** (priority 100): Uses OAuth tokens from keychain/gh CLI
//! 2. **Env Strategy** (priority 60): Uses COPILOT_API_TOKEN or GITHUB_TOKEN
//!
//! ## API Endpoints
//!
//! - `GET /user` - Get user info
//! - `GET /user/copilot_billing/seat` - Get Copilot subscription status
//! - `GET /user/copilot_billing/usage` - Get usage statistics

// Modules
mod api;
mod descriptor;
mod device_flow;
mod error;
mod fetcher;
pub(crate) mod parser;
mod strategies;
mod token_store;

// Re-exports
pub use api::{CopilotApiClient, CopilotUsage, CopilotUsageResponse};
pub use descriptor::copilot_descriptor;
pub use device_flow::{AccessTokenResponse, CopilotDeviceFlow, DeviceFlowResult, DeviceFlowStart};
pub use error::CopilotError;
pub use fetcher::{CopilotDataSource, CopilotUsageFetcher};
pub use strategies::{CopilotApiStrategy, CopilotEnvStrategy};
pub use token_store::CopilotTokenStore;
