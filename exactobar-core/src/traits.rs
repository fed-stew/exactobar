//! Trait definitions for ExactoBar.
//!
//! This module defines the core traits that provider implementations must satisfy.

use crate::error::CoreError;
use crate::models::{Credits, CostUsageSnapshot, ProviderKind, Quota, UsageData, UsageSnapshot};

/// Trait for providers that can fetch usage data.
///
/// Implementors of this trait are responsible for:
/// - Authenticating with the provider's API
/// - Fetching current usage information
/// - Parsing and normalizing the response into usage types
///
/// Providers can implement either `fetch_usage` (legacy) or `fetch_snapshot` (new),
/// but should prefer `fetch_snapshot` for richer data.
pub trait UsageProvider: Send + Sync {
    /// Returns the kind of provider this implementation handles.
    fn kind(&self) -> ProviderKind;

    /// Returns the display name for this provider.
    fn display_name(&self) -> &str {
        self.kind().display_name()
    }

    /// Fetches current usage data from the provider (legacy format).
    ///
    /// This is an async operation that may involve network requests.
    /// Prefer `fetch_snapshot` for richer data.
    fn fetch_usage(
        &self,
    ) -> impl std::future::Future<Output = Result<UsageData, CoreError>> + Send;

    /// Fetches current usage snapshot from the provider (rich format).
    ///
    /// This returns the full `UsageSnapshot` with primary, secondary, and
    /// tertiary windows, plus identity information.
    ///
    /// Default implementation converts from `fetch_usage`.
    fn fetch_snapshot(
        &self,
    ) -> impl std::future::Future<Output = Result<UsageSnapshot, CoreError>> + Send {
        async {
            let usage = self.fetch_usage().await?;
            Ok(usage.to_snapshot())
        }
    }

    /// Returns true if this provider is currently configured and ready to use.
    fn is_configured(&self) -> bool;

    /// Returns true if this provider supports the rich snapshot format.
    fn supports_snapshot(&self) -> bool {
        false
    }
}

/// Trait for providers that support quota information.
pub trait QuotaProvider: UsageProvider {
    /// Fetches quota information from the provider.
    fn fetch_quota(
        &self,
    ) -> impl std::future::Future<Output = Result<Quota, CoreError>> + Send;
}

/// Trait for providers that support credit-based systems.
pub trait CreditsProvider: UsageProvider {
    /// Fetches credits information from the provider.
    fn fetch_credits(
        &self,
    ) -> impl std::future::Future<Output = Result<Credits, CoreError>> + Send;
}

/// Trait for providers that support token cost tracking.
pub trait CostProvider: UsageProvider {
    /// Fetches cost usage snapshot from local logs or API.
    fn fetch_cost_snapshot(
        &self,
    ) -> impl std::future::Future<Output = Result<CostUsageSnapshot, CoreError>> + Send;
}
