//! Copilot fetch strategies.

use async_trait::async_trait;
#[allow(unused_imports)]
use exactobar_core::{FetchSource, UsageSnapshot};
use exactobar_fetch::{
    FetchContext, FetchError, FetchKind, FetchResult, FetchStrategy,
    host::keychain::{accounts, services},
};
use tracing::{debug, instrument};

use super::parser::parse_copilot_response;

const COPILOT_API_BASE: &str = "https://api.github.com";

// ============================================================================
// API Strategy (Device Flow OAuth)
// ============================================================================

/// Fetch strategy that reads Copilot usage via the GitHub API.
pub struct CopilotApiStrategy {
    api_base: &'static str,
}

impl CopilotApiStrategy {
    /// Creates a new Copilot API strategy.
    pub fn new() -> Self {
        Self {
            api_base: COPILOT_API_BASE,
        }
    }

    async fn get_oauth_token(&self, ctx: &FetchContext) -> Option<String> {
        // Try keychain first
        if let Ok(Some(token)) = ctx.keychain.get(services::GITHUB, accounts::OAUTH_TOKEN).await {
            return Some(token);
        }

        // Try gh CLI config
        let output = ctx.process.run("gh", &["auth", "token"]).await.ok()?;
        if output.success() {
            Some(output.stdout.trim().to_string())
        } else {
            None
        }
    }
}

impl Default for CopilotApiStrategy {
    fn default() -> Self {
        Self::new()
    }
}

#[async_trait]
impl FetchStrategy for CopilotApiStrategy {
    fn id(&self) -> &str {
        "copilot.api"
    }

    fn kind(&self) -> FetchKind {
        FetchKind::OAuth
    }

    #[instrument(skip(self, ctx))]
    async fn is_available(&self, ctx: &FetchContext) -> bool {
        self.get_oauth_token(ctx).await.is_some()
    }

    #[instrument(skip(self, ctx))]
    async fn fetch(&self, ctx: &FetchContext) -> Result<FetchResult, FetchError> {
        debug!("Fetching Copilot usage via GitHub API");

        let token = self
            .get_oauth_token(ctx)
            .await
            .ok_or_else(|| FetchError::AuthenticationFailed("No GitHub token".to_string()))?;

        let url = format!("{}/copilot/usage", self.api_base);
        let auth_header = format!("Bearer {}", token);

        let response = ctx
            .http
            .get_with_auth(&url, &auth_header)
            .await
            .map_err(|e| FetchError::InvalidResponse(e.to_string()))?;

        if response.status() == reqwest::StatusCode::UNAUTHORIZED {
            return Err(FetchError::AuthenticationFailed("Token rejected".to_string()));
        }

        if !response.status().is_success() {
            return Err(FetchError::InvalidResponse(format!(
                "API returned {}",
                response.status()
            )));
        }

        let body = response.text().await
            .map_err(|e| FetchError::InvalidResponse(e.to_string()))?;

        let snapshot = parse_copilot_response(&body)?;
        Ok(FetchResult::new(snapshot, self.id(), self.kind()))
    }

    fn priority(&self) -> u32 {
        100
    }
}

// ============================================================================
// Environment Variable Strategy
// ============================================================================

/// Fetch strategy that reads Copilot usage via environment variables.
pub struct CopilotEnvStrategy;

impl CopilotEnvStrategy {
    /// Creates a new Copilot environment strategy.
    pub fn new() -> Self {
        Self
    }

    fn get_env_token() -> Option<String> {
        std::env::var("COPILOT_API_TOKEN")
            .or_else(|_| std::env::var("GITHUB_TOKEN"))
            .ok()
    }
}

impl Default for CopilotEnvStrategy {
    fn default() -> Self {
        Self::new()
    }
}

#[async_trait]
impl FetchStrategy for CopilotEnvStrategy {
    fn id(&self) -> &str {
        "copilot.env"
    }

    fn kind(&self) -> FetchKind {
        FetchKind::ApiKey
    }

    #[instrument(skip(self, _ctx))]
    async fn is_available(&self, _ctx: &FetchContext) -> bool {
        Self::get_env_token().is_some()
    }

    #[instrument(skip(self, ctx))]
    async fn fetch(&self, ctx: &FetchContext) -> Result<FetchResult, FetchError> {
        debug!("Fetching Copilot usage via environment token");

        let token = Self::get_env_token()
            .ok_or_else(|| FetchError::AuthenticationFailed("No env token".to_string()))?;

        let url = format!("{}/copilot/usage", COPILOT_API_BASE);
        let auth_header = format!("Bearer {}", token);

        let response = ctx
            .http
            .get_with_auth(&url, &auth_header)
            .await
            .map_err(|e| FetchError::InvalidResponse(e.to_string()))?;

        if !response.status().is_success() {
            return Err(FetchError::InvalidResponse(format!(
                "API returned {}",
                response.status()
            )));
        }

        let body = response.text().await
            .map_err(|e| FetchError::InvalidResponse(e.to_string()))?;

        let snapshot = parse_copilot_response(&body)?;
        Ok(FetchResult::new(snapshot, self.id(), self.kind()))
    }

    fn priority(&self) -> u32 {
        60
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_api_strategy() {
        let s = CopilotApiStrategy::new();
        assert_eq!(s.id(), "copilot.api");
        assert_eq!(s.priority(), 100);
    }

    #[test]
    fn test_env_strategy() {
        let s = CopilotEnvStrategy::new();
        assert_eq!(s.id(), "copilot.env");
        assert_eq!(s.priority(), 60);
    }
}
