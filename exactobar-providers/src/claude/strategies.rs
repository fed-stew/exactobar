//! Claude fetch strategies.
//!
//! This module provides multiple strategies for fetching Claude usage data:
//!
//! 1. **OAuth Strategy** - Uses OAuth tokens for API access
//! 2. **PTY Strategy** - Interactive `/usage` command
//! 3. **CLI Strategy** - `claude usage` command (legacy)
//! 4. **Web Strategy** - Browser cookies for claude.ai

use async_trait::async_trait;
use exactobar_fetch::{
    FetchContext, FetchError, FetchKind, FetchResult, FetchStrategy, host::browser::Browser,
};
use tracing::{debug, info, instrument};

use super::api::ClaudeApiClient;
use super::fetcher::ClaudeUsageFetcher;
use super::oauth::ClaudeOAuthCredentials;
use super::parser::parse_claude_cli_output;
use super::pty_probe::ClaudePtyProbe;
use super::web::ClaudeWebClient;

// ============================================================================
// OAuth Strategy (Highest Priority)
// ============================================================================

/// Claude OAuth strategy using tokens from Claude CLI.
///
/// This is the primary strategy for Claude. It uses OAuth tokens
/// stored by the Claude CLI to access the Anthropic API directly.
pub struct ClaudeOAuthStrategy;

impl ClaudeOAuthStrategy {
    /// Creates a new OAuth strategy.
    pub fn new() -> Self {
        Self
    }
}

impl Default for ClaudeOAuthStrategy {
    fn default() -> Self {
        Self::new()
    }
}

#[async_trait]
impl FetchStrategy for ClaudeOAuthStrategy {
    fn id(&self) -> &str {
        "claude.oauth"
    }

    fn kind(&self) -> FetchKind {
        FetchKind::OAuth
    }

    #[instrument(skip(self, _ctx))]
    async fn is_available(&self, _ctx: &FetchContext) -> bool {
        // Don't check credentials here - it may hit keychain and cause password prompts!
        // Let fetch() handle credential loading and return appropriate errors.
        // This is the "lazy" approach - we assume OAuth might be available and try.
        true
    }

    #[instrument(skip(self, _ctx))]
    async fn fetch(&self, _ctx: &FetchContext) -> Result<FetchResult, FetchError> {
        debug!("Fetching Claude usage via OAuth");

        let credentials = ClaudeOAuthCredentials::load()
            .map_err(|e| FetchError::AuthenticationFailed(e.to_string()))?;

        if credentials.is_expired() {
            return Err(FetchError::AuthenticationFailed(
                "OAuth token expired".to_string(),
            ));
        }

        let client = ClaudeApiClient::new();
        let response = client
            .fetch_usage(&credentials)
            .await
            .map_err(|e| FetchError::InvalidResponse(e.to_string()))?;

        // Debug logging to trace data flow
        info!(
            "OAuth API Response: five_hour={:?}, seven_day={:?}, seven_day_sonnet={:?}",
            response.five_hour, response.seven_day, response.seven_day_sonnet
        );

        let snapshot = response.to_snapshot();

        info!(
            "OAuth Snapshot: primary={:?}, secondary={:?}, tertiary={:?}",
            snapshot.primary, snapshot.secondary, snapshot.tertiary
        );

        Ok(FetchResult::new(snapshot, self.id(), self.kind()))
    }

    fn priority(&self) -> u32 {
        100 // Highest priority
    }

    fn should_fallback(&self, error: &FetchError) -> bool {
        // Don't fallback on auth errors
        !matches!(error, FetchError::AuthenticationFailed(_))
    }
}

// ============================================================================
// PTY Strategy
// ============================================================================

/// Claude PTY strategy using interactive `/usage` command.
///
/// This strategy runs claude interactively and parses the TUI output
/// from the `/usage` command.
pub struct ClaudePtyStrategy;

impl ClaudePtyStrategy {
    /// Creates a new PTY strategy.
    pub fn new() -> Self {
        Self
    }
}

impl Default for ClaudePtyStrategy {
    fn default() -> Self {
        Self::new()
    }
}

#[async_trait]
impl FetchStrategy for ClaudePtyStrategy {
    fn id(&self) -> &str {
        "claude.pty"
    }

    fn kind(&self) -> FetchKind {
        FetchKind::CLI
    }

    #[instrument(skip(self, _ctx))]
    async fn is_available(&self, _ctx: &FetchContext) -> bool {
        ClaudePtyProbe::is_available()
    }

    #[instrument(skip(self, _ctx))]
    async fn fetch(&self, _ctx: &FetchContext) -> Result<FetchResult, FetchError> {
        debug!("Fetching Claude usage via PTY");

        let fetcher = ClaudeUsageFetcher::cli_only();
        let snapshot = fetcher.fetch_usage().await.map_err(|e| {
            FetchError::Process(exactobar_fetch::ProcessError::ExecutionFailed(
                e.to_string(),
            ))
        })?;

        Ok(FetchResult::new(snapshot, self.id(), self.kind()))
    }

    fn priority(&self) -> u32 {
        40 // Fallback priority
    }
}

// ============================================================================
// CLI Strategy (Legacy)
// ============================================================================

/// Claude CLI strategy using `claude` command.
///
/// This strategy runs the Claude CLI to get usage information.
/// This is a legacy strategy - prefer PTY for interactive commands.
pub struct ClaudeCliStrategy {
    command: &'static str,
}

impl ClaudeCliStrategy {
    /// Creates a new CLI strategy.
    pub fn new() -> Self {
        Self { command: "claude" }
    }
}

impl Default for ClaudeCliStrategy {
    fn default() -> Self {
        Self::new()
    }
}

#[async_trait]
impl FetchStrategy for ClaudeCliStrategy {
    fn id(&self) -> &str {
        "claude.cli"
    }

    fn kind(&self) -> FetchKind {
        FetchKind::CLI
    }

    #[instrument(skip(self, ctx))]
    async fn is_available(&self, ctx: &FetchContext) -> bool {
        ctx.process.command_exists(self.command)
    }

    #[instrument(skip(self, ctx))]
    async fn fetch(&self, ctx: &FetchContext) -> Result<FetchResult, FetchError> {
        debug!("Fetching Claude usage via CLI");

        // Run claude usage command
        // Note: The exact command may vary depending on Claude CLI version
        let output = ctx
            .process
            .run_with_timeout(self.command, &["usage", "--json"], ctx.timeout())
            .await
            .map_err(FetchError::Process)?;

        if !output.success() {
            // Try without --json flag
            let output = ctx
                .process
                .run_with_timeout(self.command, &["usage"], ctx.timeout())
                .await
                .map_err(FetchError::Process)?;

            if !output.success() {
                return Err(FetchError::InvalidResponse(format!(
                    "CLI exited with code {}: {}",
                    output.exit_code, output.stderr
                )));
            }

            // Parse non-JSON output
            let snapshot = parse_claude_cli_output(&output.stdout, false)?;
            return Ok(FetchResult::new(snapshot, self.id(), self.kind()));
        }

        let snapshot = parse_claude_cli_output(&output.stdout, true)?;
        Ok(FetchResult::new(snapshot, self.id(), self.kind()))
    }

    fn priority(&self) -> u32 {
        80 // Lower than OAuth, higher than PTY
    }
}

// ============================================================================
// Web Strategy
// ============================================================================

/// Claude web strategy using browser cookies.
///
/// This strategy uses cookies from the browser to access claude.ai
/// and fetch usage information from the web interface.
pub struct ClaudeWebStrategy {
    domain: &'static str,
}

impl ClaudeWebStrategy {
    /// Creates a new web strategy.
    pub fn new() -> Self {
        Self {
            domain: "claude.ai",
        }
    }
}

impl Default for ClaudeWebStrategy {
    fn default() -> Self {
        Self::new()
    }
}

#[async_trait]
impl FetchStrategy for ClaudeWebStrategy {
    fn id(&self) -> &str {
        "claude.web"
    }

    fn kind(&self) -> FetchKind {
        FetchKind::WebCookies
    }

    #[instrument(skip(self, _ctx))]
    async fn is_available(&self, _ctx: &FetchContext) -> bool {
        // Don't try to import cookies here - it may hit Chrome Safe Storage keychain!
        // Just check if any browser is installed (no keychain access).
        // Let fetch() handle the actual cookie import and return appropriate errors.
        !Browser::default_priority()
            .iter()
            .filter(|b| b.is_installed())
            .collect::<Vec<_>>()
            .is_empty()
    }

    #[instrument(skip(self, ctx))]
    async fn fetch(&self, ctx: &FetchContext) -> Result<FetchResult, FetchError> {
        debug!("Fetching Claude usage via web cookies");

        // Get cookies from browser
        let (browser, cookies) = ctx
            .browser
            .import_cookies_auto(self.domain, Browser::default_priority())
            .await
            .map_err(FetchError::Browser)?;

        debug!(browser = ?browser, cookie_count = cookies.len(), "Got cookies");

        // Build cookie header
        let cookie_header =
            exactobar_fetch::host::browser::BrowserCookieImporter::cookies_to_header(&cookies);

        // Check for session cookie
        if !ClaudeWebClient::has_session_cookie(&cookie_header) {
            return Err(FetchError::AuthenticationFailed(
                "No session cookie found".to_string(),
            ));
        }

        // Fetch usage
        let client = ClaudeWebClient::new();
        let response = client
            .fetch_usage(&cookie_header, None)
            .await
            .map_err(|e| FetchError::InvalidResponse(e.to_string()))?;

        let snapshot = response.to_snapshot();

        Ok(FetchResult::new(snapshot, self.id(), self.kind()))
    }

    fn priority(&self) -> u32 {
        60 // Medium priority
    }

    fn should_fallback(&self, _error: &FetchError) -> bool {
        // Always allow fallback from web strategy
        true
    }
}

// ============================================================================
// Tests
// ============================================================================

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_oauth_strategy_id() {
        let strategy = ClaudeOAuthStrategy::new();
        assert_eq!(strategy.id(), "claude.oauth");
        assert_eq!(strategy.kind(), FetchKind::OAuth);
        assert_eq!(strategy.priority(), 100);
    }

    #[test]
    fn test_pty_strategy_id() {
        let strategy = ClaudePtyStrategy::new();
        assert_eq!(strategy.id(), "claude.pty");
        assert_eq!(strategy.kind(), FetchKind::CLI);
        assert_eq!(strategy.priority(), 40);
    }

    #[test]
    fn test_cli_strategy_id() {
        let strategy = ClaudeCliStrategy::new();
        assert_eq!(strategy.id(), "claude.cli");
        assert_eq!(strategy.kind(), FetchKind::CLI);
        assert_eq!(strategy.priority(), 80);
    }

    #[test]
    fn test_web_strategy_id() {
        let strategy = ClaudeWebStrategy::new();
        assert_eq!(strategy.id(), "claude.web");
        assert_eq!(strategy.kind(), FetchKind::WebCookies);
        assert_eq!(strategy.priority(), 60);
    }

    #[test]
    fn test_strategy_priority_order() {
        let oauth = ClaudeOAuthStrategy::new().priority();
        let cli = ClaudeCliStrategy::new().priority();
        let web = ClaudeWebStrategy::new().priority();
        let pty = ClaudePtyStrategy::new().priority();

        assert!(oauth > cli);
        assert!(cli > web);
        assert!(web > pty);
    }
}
