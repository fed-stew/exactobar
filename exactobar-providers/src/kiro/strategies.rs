//! Kiro fetch strategies.

use async_trait::async_trait;
use exactobar_fetch::{FetchContext, FetchError, FetchKind, FetchResult, FetchStrategy, ProcessError};
use tracing::{debug, instrument, warn};

use super::cli::ensure_logged_in;
use super::error::KiroError;
use super::parser::parse_kiro_response;

// ============================================================================
// CLI Strategy
// ============================================================================

/// Kiro CLI fetch strategy.
pub struct KiroCliStrategy {
    command: &'static str,
}

impl KiroCliStrategy {
    /// Create a new CLI strategy.
    pub fn new() -> Self {
        Self { command: "kiro-cli" }
    }
}

impl Default for KiroCliStrategy {
    fn default() -> Self {
        Self::new()
    }
}

#[async_trait]
impl FetchStrategy for KiroCliStrategy {
    fn id(&self) -> &str {
        "kiro.cli"
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
        debug!("Fetching Kiro usage via CLI");

        // Check login first
        ensure_logged_in().await.map_err(|e| match e {
            KiroError::NotLoggedIn => {
                warn!("Kiro: user not logged in");
                FetchError::AuthenticationFailed("Not logged in to Kiro".to_string())
            }
            KiroError::CliNotFound => {
                FetchError::Process(ProcessError::NotFound("kiro-cli".to_string()))
            }
            KiroError::CliFailed(msg) => FetchError::Process(ProcessError::ExecutionFailed(msg)),
            _ => FetchError::InvalidResponse(e.to_string()),
        })?;

        // Then fetch usage
        let output = ctx
            .process
            .run_with_timeout(self.command, &["/usage", "--json"], ctx.timeout())
            .await
            .map_err(FetchError::Process)?;

        if !output.success() {
            return Err(FetchError::InvalidResponse(format!(
                "CLI exited with code {}",
                output.exit_code
            )));
        }

        let snapshot = parse_kiro_response(&output.stdout)?;
        Ok(FetchResult::new(snapshot, self.id(), self.kind()))
    }

    fn priority(&self) -> u32 {
        100
    }
}

// ============================================================================
// Tests
// ============================================================================

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_cli_strategy() {
        let s = KiroCliStrategy::new();
        assert_eq!(s.id(), "kiro.cli");
        assert_eq!(s.priority(), 100);
        assert_eq!(s.kind(), FetchKind::CLI);
    }

    #[test]
    fn test_default() {
        let s = KiroCliStrategy::default();
        assert_eq!(s.command, "kiro-cli");
    }
}
