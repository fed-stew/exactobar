//! Cursor fetch strategies.

use async_trait::async_trait;
use exactobar_core::{FetchSource, UsageSnapshot};
use exactobar_fetch::{
    host::browser::Browser, FetchContext, FetchError, FetchKind, FetchResult, FetchStrategy,
};
use tracing::{debug, instrument, warn};

use super::local::CursorLocalReader;
use super::web::CursorWebClient;

// ============================================================================
// Web Strategy
// ============================================================================

/// Cursor web strategy using browser cookies.
///
/// This is the primary strategy for Cursor. It uses cookies from
/// the browser to access the Cursor API.
pub struct CursorWebStrategy {
    domain: &'static str,
}

impl CursorWebStrategy {
    /// Creates a new web strategy.
    pub fn new() -> Self {
        Self {
            domain: "cursor.com",
        }
    }
}

impl Default for CursorWebStrategy {
    fn default() -> Self {
        Self::new()
    }
}

#[async_trait]
impl FetchStrategy for CursorWebStrategy {
    fn id(&self) -> &str {
        "cursor.web"
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
        debug!("Fetching Cursor usage via web cookies");

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
        if !CursorWebClient::has_session_cookie(&cookie_header) {
            return Err(FetchError::AuthenticationFailed(
                "No session cookie found".to_string(),
            ));
        }

        // Fetch usage from API
        let client = CursorWebClient::new();
        let response = client
            .fetch_usage(&cookie_header)
            .await
            .map_err(|e| FetchError::InvalidResponse(e.to_string()))?;

        let snapshot = response.to_snapshot();

        Ok(FetchResult::new(snapshot, self.id(), self.kind()))
    }

    fn priority(&self) -> u32 {
        100 // Primary strategy
    }
}

// ============================================================================
// Local Strategy
// ============================================================================

/// Cursor local strategy reading from config files.
///
/// This strategy reads Cursor's local configuration and cache
/// to get usage information without network access.
pub struct CursorLocalStrategy;

impl CursorLocalStrategy {
    /// Creates a new local strategy.
    pub fn new() -> Self {
        Self
    }
}

impl Default for CursorLocalStrategy {
    fn default() -> Self {
        Self::new()
    }
}

#[async_trait]
impl FetchStrategy for CursorLocalStrategy {
    fn id(&self) -> &str {
        "cursor.local"
    }

    fn kind(&self) -> FetchKind {
        FetchKind::LocalProbe
    }

    #[instrument(skip(self, _ctx))]
    async fn is_available(&self, _ctx: &FetchContext) -> bool {
        CursorLocalReader::is_installed()
    }

    #[instrument(skip(self, _ctx))]
    async fn fetch(&self, _ctx: &FetchContext) -> Result<FetchResult, FetchError> {
        debug!("Fetching Cursor usage from local config");

        let reader = CursorLocalReader::new();

        match reader.read_cached_usage() {
            Ok(snapshot) => Ok(FetchResult::new(snapshot, self.id(), self.kind())),
            Err(e) => {
                warn!(error = %e, "Local read failed");

                // Return minimal snapshot indicating we found Cursor but couldn't get usage
                let mut snapshot = UsageSnapshot::new();
                snapshot.fetch_source = FetchSource::LocalProbe;

                Ok(FetchResult::new(snapshot, self.id(), self.kind()))
            }
        }
    }

    fn priority(&self) -> u32 {
        60 // Lower than web strategy
    }

    fn should_fallback(&self, _error: &FetchError) -> bool {
        // Always allow fallback from local strategy
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
    fn test_web_strategy_id() {
        let strategy = CursorWebStrategy::new();
        assert_eq!(strategy.id(), "cursor.web");
        assert_eq!(strategy.kind(), FetchKind::WebCookies);
        assert_eq!(strategy.priority(), 100);
    }

    #[test]
    fn test_local_strategy_id() {
        let strategy = CursorLocalStrategy::new();
        assert_eq!(strategy.id(), "cursor.local");
        assert_eq!(strategy.kind(), FetchKind::LocalProbe);
        assert_eq!(strategy.priority(), 60);
    }
}
