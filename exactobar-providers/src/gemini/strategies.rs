//! Gemini fetch strategies.

use async_trait::async_trait;
// UsageSnapshot and FetchSource are used via the probe's to_usage_snapshot()
use exactobar_fetch::{FetchContext, FetchError, FetchKind, FetchResult, FetchStrategy};
use tracing::{debug, info, instrument, warn};

use super::parser::parse_gemini_response;
use super::probe::{GeminiCredentials, GeminiProbe};

// ============================================================================
// OAuth Strategy
// ============================================================================

/// Gemini OAuth strategy using local ~/.gemini credentials.
///
/// This strategy reads OAuth credentials from the Gemini CLI config files
/// and fetches quota data from the Cloud Code Private API.
pub struct GeminiOAuthStrategy {
    probe: GeminiProbe,
}

impl GeminiOAuthStrategy {
    /// Creates a new OAuth-based Gemini strategy.
    pub fn new() -> Self {
        Self {
            probe: GeminiProbe::new(),
        }
    }
}

impl Default for GeminiOAuthStrategy {
    fn default() -> Self {
        Self::new()
    }
}

#[async_trait]
impl FetchStrategy for GeminiOAuthStrategy {
    fn id(&self) -> &str {
        "gemini.oauth"
    }

    fn kind(&self) -> FetchKind {
        FetchKind::OAuth
    }

    #[instrument(skip(self, _ctx))]
    async fn is_available(&self, _ctx: &FetchContext) -> bool {
        // Check if ~/.gemini/oauth_creds.json exists
        GeminiCredentials::exists()
    }

    #[instrument(skip(self, _ctx))]
    async fn fetch(&self, _ctx: &FetchContext) -> Result<FetchResult, FetchError> {
        debug!("Fetching Gemini usage via OAuth probe");

        let snapshot_data = self.probe.fetch().await.map_err(|e| {
            warn!(error = %e, "Gemini OAuth probe failed");
            match e {
                super::error::GeminiError::NotLoggedIn => {
                    FetchError::AuthenticationFailed("Not logged in to Gemini CLI".to_string())
                }
                super::error::GeminiError::TokenExpired(msg) => {
                    FetchError::AuthenticationFailed(format!("Token expired: {}", msg))
                }
                super::error::GeminiError::UnsupportedAuthType(msg) => {
                    FetchError::AuthenticationFailed(format!("Unsupported auth: {}", msg))
                }
                super::error::GeminiError::RefreshFailed(msg) => {
                    FetchError::AuthenticationFailed(format!("Token refresh failed: {}", msg))
                }
                super::error::GeminiError::InvalidResponse(msg) => {
                    FetchError::InvalidResponse(msg)
                }
                super::error::GeminiError::HttpError(msg) => {
                    FetchError::AuthenticationFailed(format!("HTTP error: {}", msg))
                }
                other => FetchError::AuthenticationFailed(other.to_string()),
            }
        })?;

        if !snapshot_data.has_data() {
            return Err(FetchError::InvalidResponse(
                "No quota data returned".to_string(),
            ));
        }

        let snapshot = snapshot_data.to_usage_snapshot();
        info!("Successfully fetched Gemini quota via OAuth");

        Ok(FetchResult::new(snapshot, self.id(), self.kind()))
    }

    fn priority(&self) -> u32 {
        100
    }
}

// ============================================================================
// CLI Strategy
// ============================================================================

/// Gemini CLI strategy.
pub struct GeminiCliStrategy {
    command: &'static str,
}

impl GeminiCliStrategy {
    /// Creates a new CLI-based Gemini strategy.
    pub fn new() -> Self {
        Self { command: "gemini" }
    }
}

impl Default for GeminiCliStrategy {
    fn default() -> Self {
        Self::new()
    }
}

#[async_trait]
impl FetchStrategy for GeminiCliStrategy {
    fn id(&self) -> &str {
        "gemini.cli"
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
        debug!("Fetching Gemini usage via CLI");

        let output = ctx
            .process
            .run_with_timeout(self.command, &["usage", "--json"], ctx.timeout())
            .await
            .map_err(FetchError::Process)?;

        if !output.success() {
            return Err(FetchError::InvalidResponse(format!(
                "CLI exited with code {}",
                output.exit_code
            )));
        }

        let snapshot = parse_gemini_response(&output.stdout)?;
        Ok(FetchResult::new(snapshot, self.id(), self.kind()))
    }

    fn priority(&self) -> u32 {
        80
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_oauth_strategy() {
        let s = GeminiOAuthStrategy::new();
        assert_eq!(s.id(), "gemini.oauth");
        assert_eq!(s.priority(), 100);
    }

    #[test]
    fn test_cli_strategy() {
        let s = GeminiCliStrategy::new();
        assert_eq!(s.id(), "gemini.cli");
        assert_eq!(s.priority(), 80);
    }
}
