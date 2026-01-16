//! Factory fetch strategies.

use async_trait::async_trait;
#[allow(unused_imports)]
use exactobar_core::{FetchSource, UsageSnapshot};
use exactobar_fetch::{
    host::browser::Browser, FetchContext, FetchError, FetchKind, FetchResult, FetchStrategy,
};
use std::path::PathBuf;
use tracing::{debug, instrument};

use super::parser::parse_factory_response;

const FACTORY_DOMAIN: &str = "app.factory.ai";
const FACTORY_API: &str = "https://app.factory.ai/api/usage";

// ============================================================================
// Web Strategy
// ============================================================================

/// Fetch strategy that loads Factory usage via browser cookies.
pub struct FactoryWebStrategy {
    domain: &'static str,
}

impl FactoryWebStrategy {
    /// Creates a new Factory web strategy.
    pub fn new() -> Self {
        Self {
            domain: FACTORY_DOMAIN,
        }
    }
}

impl Default for FactoryWebStrategy {
    fn default() -> Self {
        Self::new()
    }
}

#[async_trait]
impl FetchStrategy for FactoryWebStrategy {
    fn id(&self) -> &str {
        "factory.web"
    }

    fn kind(&self) -> FetchKind {
        FetchKind::WebCookies
    }

    #[instrument(skip(self, ctx))]
    async fn is_available(&self, ctx: &FetchContext) -> bool {
        ctx.browser
            .import_cookies_auto(self.domain, Browser::default_priority())
            .await
            .is_ok()
    }

    #[instrument(skip(self, ctx))]
    async fn fetch(&self, ctx: &FetchContext) -> Result<FetchResult, FetchError> {
        debug!("Fetching Factory usage via web cookies");

        let (_, cookies) = ctx
            .browser
            .import_cookies_auto(self.domain, Browser::default_priority())
            .await
            .map_err(FetchError::Browser)?;

        let cookie_header =
            exactobar_fetch::host::browser::BrowserCookieImporter::cookies_to_header(&cookies);

        let response = ctx
            .http
            .get_with_cookies(FACTORY_API, &cookie_header)
            .await
            .map_err(|e| FetchError::InvalidResponse(e.to_string()))?;

        if response.status() == reqwest::StatusCode::UNAUTHORIZED {
            return Err(FetchError::AuthenticationFailed("Cookies rejected".to_string()));
        }

        if !response.status().is_success() {
            return Err(FetchError::InvalidResponse(format!(
                "API returned {}",
                response.status()
            )));
        }

        let body = response.text().await
            .map_err(|e| FetchError::InvalidResponse(e.to_string()))?;

        let snapshot = parse_factory_response(&body)?;
        Ok(FetchResult::new(snapshot, self.id(), self.kind()))
    }

    fn priority(&self) -> u32 {
        100
    }
}

// ============================================================================
// Local Strategy
// ============================================================================

/// Fetch strategy that reads Factory usage from local config.
pub struct FactoryLocalStrategy;

impl FactoryLocalStrategy {
    /// Creates a new Factory local strategy.
    pub fn new() -> Self {
        Self
    }

    fn config_path() -> Option<PathBuf> {
        dirs::config_dir().map(|c| c.join("factory").join("state.json"))
    }
}

impl Default for FactoryLocalStrategy {
    fn default() -> Self {
        Self::new()
    }
}

#[async_trait]
impl FetchStrategy for FactoryLocalStrategy {
    fn id(&self) -> &str {
        "factory.local"
    }

    fn kind(&self) -> FetchKind {
        FetchKind::LocalProbe
    }

    #[instrument(skip(self, _ctx))]
    async fn is_available(&self, _ctx: &FetchContext) -> bool {
        Self::config_path().is_some_and(|p| p.exists())
    }

    #[instrument(skip(self, _ctx))]
    async fn fetch(&self, _ctx: &FetchContext) -> Result<FetchResult, FetchError> {
        debug!("Fetching Factory usage from local config");

        let path = Self::config_path()
            .ok_or_else(|| FetchError::InvalidResponse("No config path".to_string()))?;

        let content = std::fs::read_to_string(&path)
            .map_err(|e| FetchError::InvalidResponse(e.to_string()))?;

        let snapshot = parse_factory_response(&content)?;
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
    fn test_web_strategy() {
        let s = FactoryWebStrategy::new();
        assert_eq!(s.id(), "factory.web");
        assert_eq!(s.priority(), 100);
    }

    #[test]
    fn test_local_strategy() {
        let s = FactoryLocalStrategy::new();
        assert_eq!(s.id(), "factory.local");
        assert_eq!(s.priority(), 60);
    }
}
