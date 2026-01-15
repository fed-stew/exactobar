//! Codex fetch strategies.
//!
//! This module provides multiple strategies for fetching Codex usage data:
//!
//! 1. **RPC Strategy** - JSON-RPC to `codex app-server`
//! 2. **PTY Strategy** - Interactive `/status` command
//! 3. **CLI Strategy** - `codex usage --json`
//! 4. **API Strategy** - OpenAI API with API key

use async_trait::async_trait;
use exactobar_core::{FetchSource, UsageSnapshot};
use exactobar_fetch::{
    FetchContext, FetchError, FetchKind, FetchResult, FetchStrategy,
    host::keychain::{accounts, services},
};
use tracing::{debug, instrument, warn};

use super::fetcher::CodexUsageFetcher;
use super::parser::parse_codex_cli_output;
use super::pty_probe::CodexPtyProbe;

// ============================================================================
// RPC Strategy (Highest Priority)
// ============================================================================

/// Codex RPC strategy using JSON-RPC to `codex app-server`.
///
/// This is the primary strategy for Codex. It spawns the app-server
/// and communicates via JSON-RPC over stdin/stdout.
pub struct CodexRpcStrategy;

impl CodexRpcStrategy {
    /// Creates a new RPC strategy.
    pub fn new() -> Self {
        Self
    }
}

impl Default for CodexRpcStrategy {
    fn default() -> Self {
        Self::new()
    }
}

#[async_trait]
impl FetchStrategy for CodexRpcStrategy {
    fn id(&self) -> &str {
        "codex.rpc"
    }

    fn kind(&self) -> FetchKind {
        FetchKind::CLI
    }

    #[instrument(skip(self, _ctx))]
    async fn is_available(&self, _ctx: &FetchContext) -> bool {
        CodexUsageFetcher::is_available()
    }

    #[instrument(skip(self, _ctx))]
    async fn fetch(&self, _ctx: &FetchContext) -> Result<FetchResult, FetchError> {
        debug!("Fetching Codex usage via RPC");

        let fetcher = CodexUsageFetcher::rpc_only();
        let snapshot = fetcher
            .fetch_usage()
            .await
            .map_err(|e| FetchError::Process(exactobar_fetch::ProcessError::ExecutionFailed(e.to_string())))?;

        Ok(FetchResult::new(snapshot, self.id(), self.kind()))
    }

    fn priority(&self) -> u32 {
        100 // Highest priority - RPC is most reliable
    }
}

// ============================================================================
// PTY Strategy (Fallback)
// ============================================================================

/// Codex PTY strategy using interactive `/status` command.
///
/// This is a fallback strategy that runs codex interactively and
/// parses the TUI output from the `/status` command.
pub struct CodexPtyStrategy;

impl CodexPtyStrategy {
    /// Creates a new PTY strategy.
    pub fn new() -> Self {
        Self
    }
}

impl Default for CodexPtyStrategy {
    fn default() -> Self {
        Self::new()
    }
}

#[async_trait]
impl FetchStrategy for CodexPtyStrategy {
    fn id(&self) -> &str {
        "codex.pty"
    }

    fn kind(&self) -> FetchKind {
        FetchKind::CLI
    }

    #[instrument(skip(self, _ctx))]
    async fn is_available(&self, _ctx: &FetchContext) -> bool {
        CodexPtyProbe::is_available()
    }

    #[instrument(skip(self, _ctx))]
    async fn fetch(&self, _ctx: &FetchContext) -> Result<FetchResult, FetchError> {
        debug!("Fetching Codex usage via PTY");

        let fetcher = CodexUsageFetcher::pty_only();
        let snapshot = fetcher
            .fetch_usage()
            .await
            .map_err(|e| FetchError::Process(exactobar_fetch::ProcessError::ExecutionFailed(e.to_string())))?;

        Ok(FetchResult::new(snapshot, self.id(), self.kind()))
    }

    fn priority(&self) -> u32 {
        90 // High priority - good fallback
    }
}

// ============================================================================
// CLI Strategy (Legacy)
// ============================================================================

/// Codex CLI strategy using `codex usage --json`.
///
/// This is a legacy strategy that uses the JSON output mode.
pub struct CodexCliStrategy {
    command: &'static str,
    args: &'static [&'static str],
}

impl CodexCliStrategy {
    /// Creates a new CLI strategy.
    pub fn new() -> Self {
        Self {
            command: "codex",
            args: &["usage", "--json"],
        }
    }
}

impl Default for CodexCliStrategy {
    fn default() -> Self {
        Self::new()
    }
}

#[async_trait]
impl FetchStrategy for CodexCliStrategy {
    fn id(&self) -> &str {
        "codex.cli"
    }

    fn kind(&self) -> FetchKind {
        FetchKind::CLI
    }

    #[instrument(skip(self, ctx))]
    async fn is_available(&self, ctx: &FetchContext) -> bool {
        let exists = ctx.process.command_exists(self.command);
        debug!(command = self.command, exists = exists, "Checking CLI availability");
        exists
    }

    #[instrument(skip(self, ctx))]
    async fn fetch(&self, ctx: &FetchContext) -> Result<FetchResult, FetchError> {
        debug!("Fetching Codex usage via CLI");

        // Run the codex command
        let output = ctx
            .process
            .run_with_timeout(self.command, self.args, ctx.timeout())
            .await
            .map_err(|e| FetchError::Process(e))?;

        if !output.success() {
            warn!(
                exit_code = output.exit_code,
                stderr = %output.stderr,
                "Codex CLI failed"
            );
            return Err(FetchError::InvalidResponse(format!(
                "Codex CLI exited with code {}: {}",
                output.exit_code, output.stderr
            )));
        }

        // Parse the output
        let snapshot = parse_codex_cli_output(&output.stdout)?;

        Ok(FetchResult::new(snapshot, self.id(), self.kind()))
    }

    fn priority(&self) -> u32 {
        80 // Lower than RPC/PTY
    }
}

// ============================================================================
// API Strategy
// ============================================================================

/// Codex API strategy using OpenAI API with API key.
///
/// This strategy uses the OpenAI API directly with an API key
/// stored in the system keychain or environment.
pub struct CodexApiStrategy {
    api_base: &'static str,
}

impl CodexApiStrategy {
    /// Creates a new API strategy.
    pub fn new() -> Self {
        Self {
            api_base: "https://api.openai.com/v1",
        }
    }

    /// Gets the API key from keychain or environment.
    async fn get_api_key(&self, ctx: &FetchContext) -> Option<String> {
        // Try keychain first
        if let Ok(Some(key)) = ctx.keychain.get(services::OPENAI, accounts::API_KEY).await {
            return Some(key);
        }

        // Fall back to environment
        std::env::var("OPENAI_API_KEY").ok()
    }
}

impl Default for CodexApiStrategy {
    fn default() -> Self {
        Self::new()
    }
}

#[async_trait]
impl FetchStrategy for CodexApiStrategy {
    fn id(&self) -> &str {
        "codex.api"
    }

    fn kind(&self) -> FetchKind {
        FetchKind::ApiKey
    }

    #[instrument(skip(self, ctx))]
    async fn is_available(&self, ctx: &FetchContext) -> bool {
        self.get_api_key(ctx).await.is_some()
    }

    #[instrument(skip(self, ctx))]
    async fn fetch(&self, ctx: &FetchContext) -> Result<FetchResult, FetchError> {
        debug!("Fetching Codex usage via API");

        let api_key = self
            .get_api_key(ctx)
            .await
            .ok_or_else(|| FetchError::AuthenticationFailed("No OpenAI API key found".to_string()))?;

        // OpenAI doesn't have a direct usage API endpoint that works with API keys
        // in the same way - the usage endpoint requires organization-level access.
        // For now, we return a placeholder indicating the API key is valid.
        let url = format!("{}/models", self.api_base);
        let auth_header = format!("Bearer {}", api_key);

        let response = ctx
            .http
            .get_with_auth(&url, &auth_header)
            .await
            .map_err(|e| FetchError::InvalidResponse(e.to_string()))?;

        if !response.status().is_success() {
            return Err(FetchError::AuthenticationFailed(
                "API key validation failed".to_string(),
            ));
        }

        // We can verify the key is valid but can't get usage from this endpoint
        // Return minimal snapshot indicating connection works
        let mut snapshot = UsageSnapshot::new();
        snapshot.fetch_source = FetchSource::Api;

        // Note: Real usage would require dashboard scraping or different auth
        warn!("OpenAI API key validated but usage data requires dashboard access");

        Ok(FetchResult::new(snapshot, self.id(), self.kind()))
    }

    fn priority(&self) -> u32 {
        60 // Lower than CLI-based strategies
    }

    fn should_fallback(&self, error: &FetchError) -> bool {
        // Don't fallback on auth errors - no point trying other strategies
        !matches!(error, FetchError::AuthenticationFailed(_))
    }
}

// ============================================================================
// Tests
// ============================================================================

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_rpc_strategy_id() {
        let strategy = CodexRpcStrategy::new();
        assert_eq!(strategy.id(), "codex.rpc");
        assert_eq!(strategy.kind(), FetchKind::CLI);
        assert_eq!(strategy.priority(), 100);
    }

    #[test]
    fn test_pty_strategy_id() {
        let strategy = CodexPtyStrategy::new();
        assert_eq!(strategy.id(), "codex.pty");
        assert_eq!(strategy.kind(), FetchKind::CLI);
        assert_eq!(strategy.priority(), 90);
    }

    #[test]
    fn test_cli_strategy_id() {
        let strategy = CodexCliStrategy::new();
        assert_eq!(strategy.id(), "codex.cli");
        assert_eq!(strategy.kind(), FetchKind::CLI);
        assert_eq!(strategy.priority(), 80);
    }

    #[test]
    fn test_api_strategy_id() {
        let strategy = CodexApiStrategy::new();
        assert_eq!(strategy.id(), "codex.api");
        assert_eq!(strategy.kind(), FetchKind::ApiKey);
        assert_eq!(strategy.priority(), 60);
    }

    #[test]
    fn test_strategy_priority_order() {
        // Ensure strategies are in correct priority order
        let rpc = CodexRpcStrategy::new().priority();
        let pty = CodexPtyStrategy::new().priority();
        let cli = CodexCliStrategy::new().priority();
        let api = CodexApiStrategy::new().priority();

        assert!(rpc > pty);
        assert!(pty > cli);
        assert!(cli > api);
    }
}
