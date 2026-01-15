// Lint configuration for this crate
#![warn(missing_docs)]
#![warn(clippy::all)]
#![warn(clippy::pedantic)]
#![allow(clippy::module_name_repetitions)]
#![allow(clippy::must_use_candidate)]

//! # `ExactoBar` Core
//!
//! Core types, models, and traits for the `ExactoBar` application.
//!
//! This crate provides the foundational abstractions used across all other
//! `ExactoBar` crates, including:
//!
//! - Domain models (providers, usage data, quotas)
//! - Error types
//! - Trait definitions for provider implementations
//! - Common utilities
//!
//! ## Key Types
//!
//! ### Provider Types
//! - [`ProviderKind`] - Enum of all supported LLM providers
//! - [`Provider`] - Provider configuration
//! - [`ProviderIdentity`] - Account identity (siloed per provider)
//! - [`ProviderMetadata`] - Provider capabilities and display info
//! - [`ProviderBranding`] - Visual styling for providers
//!
//! ### Usage Types
//! - [`UsageSnapshot`] - Main container for usage data with multiple windows
//! - [`UsageWindow`] - Individual usage window (session, weekly, opus)
//! - [`UsageData`] - Legacy simple usage data format
//! - [`Quota`] - Quota information
//! - [`Credits`] - Credit-based usage tracking
//!
//! ### Cost Tracking
//! - [`CostUsageSnapshot`] - Token cost tracking from local logs
//! - [`DailyUsageEntry`] - Daily usage entry
//! - [`ModelBreakdown`] - Per-model cost breakdown
//!
//! ### Status & Fetch
//! - [`ProviderStatus`] - Provider service health
//! - [`StatusIndicator`] - Status indicator levels
//! - [`FetchSource`] - How data was obtained

pub mod error;
pub mod models;
pub mod traits;

// Re-export error types
pub use error::CoreError;

// Re-export all model types
pub use models::{
    // Provider types
    IconStyle,
    LoginMethod,
    Provider,
    ProviderBranding,
    ProviderColor,
    ProviderIdentity,
    ProviderKind,
    ProviderMetadata,
    // Usage types
    Credits,
    Quota,
    UsageData,
    UsageSnapshot,
    UsageWindow,
    // Cost tracking
    CostUsageSnapshot,
    DailyUsageEntry,
    ModelBreakdown,
    // Status & Fetch
    FetchSource,
    ProviderStatus,
    StatusIndicator,
};

// Re-export traits
pub use traits::{CostProvider, CreditsProvider, QuotaProvider, UsageProvider};
