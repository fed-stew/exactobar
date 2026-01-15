// Lint configuration for this crate
#![warn(missing_docs)]
#![warn(clippy::all)]
#![warn(clippy::pedantic)]
#![allow(clippy::module_name_repetitions)]
#![allow(clippy::must_use_candidate)]

//! # ExactoBar Store
//!
//! State management for the ExactoBar application.
//!
//! This crate provides:
//!
//! - **UsageStore**: Main state for provider usage data with watch channels
//! - **SettingsStore**: User preferences with persistence
//! - **Persistence**: File I/O helpers for JSON data
//!
//! ## Usage
//!
//! ```ignore
//! use exactobar_store::{UsageStore, SettingsStore};
//! use exactobar_core::ProviderKind;
//!
//! // Create stores
//! let usage = UsageStore::new();
//! let settings = SettingsStore::load_default().await?;
//!
//! // Update usage
//! usage.set_snapshot(ProviderKind::Claude, snapshot).await;
//!
//! // Subscribe to changes
//! let mut rx = usage.subscribe();
//! while rx.changed().await.is_ok() {
//!     println!("Usage updated!");
//! }
//! ```

pub mod error;
pub mod persistence;
pub mod settings_store;
pub mod usage_store;

pub use error::StoreError;
pub use persistence::{
    default_cache_dir, default_cache_path, default_config_dir, default_settings_path,
    load_json, load_json_or_default, save_json,
};
pub use settings_store::{
    CookieSource, DataSourceMode, LogLevel, ProviderSettings, RefreshCadence, Settings, SettingsStore,
};
pub use usage_store::{CostUsageSnapshot, DailyCost, UsageStore};
#[cfg(test)]
mod persistence_tests;
