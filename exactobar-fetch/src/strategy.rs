//! Fetch strategy trait and types.
//!
//! A strategy represents one method of fetching usage data from a provider.
//! Providers can have multiple strategies (CLI, OAuth, web cookies, etc.)
//! that are tried in priority order.

use async_trait::async_trait;
use exactobar_core::{FetchSource, UsageSnapshot};
use serde::{Deserialize, Serialize};
use std::fmt;

use crate::context::FetchContext;
use crate::error::FetchError;

// ============================================================================
// Fetch Kind
// ============================================================================

/// The kind of fetch mechanism a strategy uses.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum FetchKind {
    /// CLI tool (e.g., `claude usage`)
    CLI,
    /// OAuth token authentication
    OAuth,
    /// Web cookies from browser
    WebCookies,
    /// API key authentication
    ApiKey,
    /// Local file/process probing
    LocalProbe,
    /// Web dashboard scraping
    WebDashboard,
}

impl FetchKind {
    /// Returns the display name for this kind.
    pub fn display_name(&self) -> &'static str {
        match self {
            Self::CLI => "CLI",
            Self::OAuth => "OAuth",
            Self::WebCookies => "Web Cookies",
            Self::ApiKey => "API Key",
            Self::LocalProbe => "Local Probe",
            Self::WebDashboard => "Web Dashboard",
        }
    }

    /// Convert to FetchSource for recording in snapshots.
    pub fn to_fetch_source(&self) -> FetchSource {
        match self {
            Self::CLI => FetchSource::CLI,
            Self::OAuth => FetchSource::OAuth,
            Self::WebCookies => FetchSource::Web,
            Self::ApiKey => FetchSource::Api,
            Self::LocalProbe => FetchSource::LocalProbe,
            Self::WebDashboard => FetchSource::Web,
        }
    }
}

impl fmt::Display for FetchKind {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "{}", self.display_name())
    }
}

// ============================================================================
// Fetch Result
// ============================================================================

/// The result of a successful fetch operation.
#[derive(Debug, Clone)]
pub struct FetchResult {
    /// The fetched usage snapshot.
    pub snapshot: UsageSnapshot,
    /// The strategy that succeeded.
    pub strategy_id: String,
    /// The kind of fetch used.
    pub kind: FetchKind,
}

impl FetchResult {
    /// Creates a new fetch result.
    pub fn new(snapshot: UsageSnapshot, strategy_id: impl Into<String>, kind: FetchKind) -> Self {
        Self {
            snapshot,
            strategy_id: strategy_id.into(),
            kind,
        }
    }
}

// ============================================================================
// Fetch Strategy Trait
// ============================================================================

/// A strategy for fetching usage data from a provider.
///
/// Providers can have multiple strategies (CLI, OAuth, web cookies, etc.)
/// that are tried in priority order by the fetch pipeline.
///
/// ## Implementing a Strategy
///
/// ```ignore
/// struct ClaudeCliStrategy;
///
/// #[async_trait]
/// impl FetchStrategy for ClaudeCliStrategy {
///     fn id(&self) -> &str {
///         "claude.cli"
///     }
///
///     fn kind(&self) -> FetchKind {
///         FetchKind::CLI
///     }
///
///     async fn is_available(&self, ctx: &FetchContext) -> bool {
///         ctx.process.command_exists("claude")
///     }
///
///     async fn fetch(&self, ctx: &FetchContext) -> Result<FetchResult, FetchError> {
///         let output = ctx.process.run("claude", &["usage"]).await?;
///         // Parse output and return FetchResult
///     }
/// }
/// ```
#[async_trait]
pub trait FetchStrategy: Send + Sync {
    /// Unique identifier for this strategy (e.g., "claude.oauth", "codex.cli").
    ///
    /// Format: `{provider}.{method}` or `{provider}.{method}.{variant}`
    fn id(&self) -> &str;

    /// The kind of fetch this strategy uses.
    fn kind(&self) -> FetchKind;

    /// Human-readable name for this strategy.
    fn display_name(&self) -> String {
        format!("{} ({})", self.id(), self.kind().display_name())
    }

    /// Check if this strategy is currently available.
    ///
    /// This should be a quick check (not network-dependent):
    /// - CLI strategy: check if the CLI tool is installed
    /// - OAuth strategy: check if we have a valid token
    /// - API key strategy: check if the key is configured
    async fn is_available(&self, ctx: &FetchContext) -> bool;

    /// Fetch usage data using this strategy.
    ///
    /// Returns a `FetchResult` on success, or `FetchError` on failure.
    async fn fetch(&self, ctx: &FetchContext) -> Result<FetchResult, FetchError>;

    /// Whether to try the next strategy if this one fails with the given error.
    ///
    /// Override this to prevent fallback on certain errors (e.g., rate limiting).
    fn should_fallback(&self, error: &FetchError) -> bool {
        match error {
            // Don't fallback on rate limiting - wait and retry same strategy
            FetchError::RateLimited { .. } => false,
            // Don't fallback on auth errors - likely config issue
            FetchError::AuthenticationFailed(_) => false,
            // Fallback on most other errors
            _ => true,
        }
    }

    /// Priority of this strategy (higher = try first).
    ///
    /// Default priorities:
    /// - CLI: 100 (preferred, usually fastest)
    /// - OAuth: 80
    /// - API Key: 60
    /// - Web Cookies: 40
    /// - Web Dashboard: 20
    /// - Local Probe: 10
    fn priority(&self) -> u32 {
        match self.kind() {
            FetchKind::CLI => 100,
            FetchKind::OAuth => 80,
            FetchKind::ApiKey => 60,
            FetchKind::WebCookies => 40,
            FetchKind::WebDashboard => 20,
            FetchKind::LocalProbe => 10,
        }
    }
}

// ============================================================================
// Strategy Info
// ============================================================================

/// Information about a strategy (for reporting).
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct StrategyInfo {
    /// Strategy ID.
    pub id: String,
    /// Strategy kind.
    pub kind: FetchKind,
    /// Whether the strategy is available.
    pub available: bool,
    /// Priority.
    pub priority: u32,
}

impl StrategyInfo {
    /// Creates strategy info from a strategy implementation.
    pub async fn from_strategy(strategy: &dyn FetchStrategy, ctx: &FetchContext) -> Self {
        Self {
            id: strategy.id().to_string(),
            kind: strategy.kind(),
            available: strategy.is_available(ctx).await,
            priority: strategy.priority(),
        }
    }
}

// ============================================================================
// Tests
// ============================================================================

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_fetch_kind_display() {
        assert_eq!(FetchKind::CLI.display_name(), "CLI");
        assert_eq!(FetchKind::WebCookies.display_name(), "Web Cookies");
    }

    #[test]
    fn test_fetch_kind_to_source() {
        assert_eq!(FetchKind::CLI.to_fetch_source(), FetchSource::CLI);
        assert_eq!(FetchKind::OAuth.to_fetch_source(), FetchSource::OAuth);
    }
}
