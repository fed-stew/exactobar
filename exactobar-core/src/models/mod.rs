//! Domain models for ExactoBar.
//!
//! This module contains the core data structures representing providers,
//! usage data, quotas, and related concepts. These types are designed to be
//! compatible with CodexBar's rich data structures.
//!
//! ## Submodules
//!
//! - [`provider`] - Provider types (ProviderKind, Identity, Metadata, Branding)
//! - [`usage`] - Usage types (UsageSnapshot, UsageWindow, Credits, Quota)
//! - [`cost`] - Cost tracking (CostUsageSnapshot, DailyUsageEntry)
//! - [`status`] - Status and fetch types (ProviderStatus, FetchSource)

mod cost;
mod provider;
mod status;
mod usage;

// Re-export everything at the models level
pub use cost::{CostUsageSnapshot, DailyUsageEntry, ModelBreakdown};
pub use provider::{
    IconStyle, LoginMethod, Provider, ProviderBranding, ProviderColor, ProviderIdentity,
    ProviderKind, ProviderMetadata,
};
pub use status::{FetchSource, ProviderStatus, StatusIndicator};
pub use usage::{Credits, Quota, UsageData, UsageSnapshot, UsageWindow};
#[cfg(test)]
mod serde_tests;
