//! Zai fetch strategies.

use async_trait::async_trait;
#[allow(unused_imports)]
use exactobar_core::{FetchSource, UsageSnapshot};
use exactobar_fetch::{
    FetchContext, FetchError, FetchKind, FetchResult, FetchStrategy,
};
use tracing::{debug, instrument};

use super::parser::parse_zai_response;
use super::token_store::ZaiTokenStore;

const ZAI_API: &str = "https://api.z.ai/v1/usage";

/// Fetch strategy that reads z.ai usage via API key.
pub struct ZaiApiStrategy {
    api_base: &'static str,
}

impl ZaiApiStrategy {
    /// Creates a new z.ai API strategy.
    pub fn new() -> Self {
        Self { api_base: ZAI_API }
    }
}

impl Default for ZaiApiStrategy {
    fn default() -> Self {
        Self::new()
    }
}

#[async_trait]
impl FetchStrategy for ZaiApiStrategy {
    fn id(&self) -> &str {
        "zai.api"
    }

    fn kind(&self) -> FetchKind {
        FetchKind::ApiKey
    }

    #[instrument(skip(self, ctx))]
    async fn is_available(&self, ctx: &FetchContext) -> bool {
        ZaiTokenStore::has_token_async(&*ctx.keychain).await
    }

    #[instrument(skip(self, ctx))]
    async fn fetch(&self, ctx: &FetchContext) -> Result<FetchResult, FetchError> {
        debug!("Fetching z.ai usage via API key");

        let api_key = ZaiTokenStore::load_async(&*ctx.keychain)
            .await
            .ok_or_else(|| FetchError::AuthenticationFailed("No z.ai API key".to_string()))?;

        let auth_header = format!("Bearer {}", api_key);

        let response = ctx
            .http
            .get_with_auth(self.api_base, &auth_header)
            .await
            .map_err(|e| FetchError::InvalidResponse(e.to_string()))?;

        if response.status() == reqwest::StatusCode::UNAUTHORIZED {
            return Err(FetchError::AuthenticationFailed("API key rejected".to_string()));
        }

        if !response.status().is_success() {
            return Err(FetchError::InvalidResponse(format!(
                "API returned {}",
                response.status()
            )));
        }

        let body = response.text().await
            .map_err(|e| FetchError::InvalidResponse(e.to_string()))?;

        let snapshot = parse_zai_response(&body)?;
        Ok(FetchResult::new(snapshot, self.id(), self.kind()))
    }

    fn priority(&self) -> u32 {
        100
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_api_strategy() {
        let s = ZaiApiStrategy::new();
        assert_eq!(s.id(), "zai.api");
        assert_eq!(s.priority(), 100);
    }
}
