//! MiniMax fetch strategies.
//!
//! MiniMax supports multiple authentication sources:
//! - Web cookies from minimax.chat
//! - Web cookies from hailuoai.com (MiniMax's web interface)
//! - Browser localStorage tokens
//! - Local config file

use async_trait::async_trait;
#[allow(unused_imports)]
use exactobar_core::{FetchSource, UsageSnapshot};
use exactobar_fetch::{
    FetchContext, FetchError, FetchKind, FetchResult, FetchStrategy, host::browser::Browser,
};
use std::path::PathBuf;
use tracing::{debug, info, instrument};

use super::parser::parse_minimax_response;
use super::web::{HAILUOAI_DOMAIN, MINIMAX_DOMAIN, MiniMaxLocalStorage, MiniMaxWebClient};

const MINIMAX_API: &str = "https://api.minimax.chat/v1/usage";
const HAILUOAI_API: &str = "https://hailuoai.com/api/user/usage";

// ============================================================================
// Web Strategy
// ============================================================================

pub struct MiniMaxWebStrategy {
    domain: &'static str,
}

impl MiniMaxWebStrategy {
    pub fn new() -> Self {
        Self {
            domain: MINIMAX_DOMAIN,
        }
    }
}

impl Default for MiniMaxWebStrategy {
    fn default() -> Self {
        Self::new()
    }
}

#[async_trait]
impl FetchStrategy for MiniMaxWebStrategy {
    fn id(&self) -> &str {
        "minimax.web"
    }

    fn kind(&self) -> FetchKind {
        FetchKind::WebCookies
    }

    #[instrument(skip(self, _ctx))]
    async fn is_available(&self, _ctx: &FetchContext) -> bool {
        // Don't try to import cookies here - it may hit Chrome Safe Storage keychain!
        // Just check if any browser is installed (no keychain access).
        !Browser::default_priority()
            .iter()
            .filter(|b| b.is_installed())
            .collect::<Vec<_>>()
            .is_empty()
    }

    #[instrument(skip(self, ctx))]
    async fn fetch(&self, ctx: &FetchContext) -> Result<FetchResult, FetchError> {
        debug!("Fetching MiniMax usage via web cookies (minimax.chat)");

        let (_, cookies) = ctx
            .browser
            .import_cookies_auto(self.domain, Browser::default_priority())
            .await
            .map_err(FetchError::Browser)?;

        let cookie_header =
            exactobar_fetch::host::browser::BrowserCookieImporter::cookies_to_header(&cookies);

        // Validate we have the right cookies
        if !MiniMaxWebClient::has_session_cookie(&cookie_header) {
            return Err(FetchError::AuthenticationFailed(
                "No valid MiniMax session cookie found".to_string(),
            ));
        }

        let response = ctx
            .http
            .get_with_cookies(MINIMAX_API, &cookie_header)
            .await
            .map_err(|e| FetchError::InvalidResponse(e.to_string()))?;

        if response.status() == reqwest::StatusCode::UNAUTHORIZED {
            return Err(FetchError::AuthenticationFailed(
                "Cookies rejected".to_string(),
            ));
        }

        if !response.status().is_success() {
            return Err(FetchError::InvalidResponse(format!(
                "API returned {}",
                response.status()
            )));
        }

        let body = response
            .text()
            .await
            .map_err(|e| FetchError::InvalidResponse(e.to_string()))?;

        let snapshot = parse_minimax_response(&body)?;
        Ok(FetchResult::new(snapshot, self.id(), self.kind()))
    }

    fn priority(&self) -> u32 {
        100
    }
}

// ============================================================================
// Hailuoai Web Strategy (MiniMax's web interface)
// ============================================================================

/// Strategy for fetching from hailuoai.com (MiniMax's web interface).
///
/// Hailuoai.com is the consumer-facing web interface for MiniMax.
/// It may have different authentication cookies than api.minimax.chat.
pub struct HailuoaiWebStrategy {
    domain: &'static str,
}

impl HailuoaiWebStrategy {
    pub fn new() -> Self {
        Self {
            domain: HAILUOAI_DOMAIN,
        }
    }
}

impl Default for HailuoaiWebStrategy {
    fn default() -> Self {
        Self::new()
    }
}

#[async_trait]
impl FetchStrategy for HailuoaiWebStrategy {
    fn id(&self) -> &str {
        "minimax.hailuoai"
    }

    fn kind(&self) -> FetchKind {
        FetchKind::WebCookies
    }

    #[instrument(skip(self, _ctx))]
    async fn is_available(&self, _ctx: &FetchContext) -> bool {
        // Don't try to import cookies here - it may hit Chrome Safe Storage keychain!
        // Just check if any browser is installed (no keychain access).
        !Browser::default_priority()
            .iter()
            .filter(|b| b.is_installed())
            .collect::<Vec<_>>()
            .is_empty()
    }

    #[instrument(skip(self, ctx))]
    async fn fetch(&self, ctx: &FetchContext) -> Result<FetchResult, FetchError> {
        debug!("Fetching MiniMax usage via hailuoai.com cookies");

        let (_, cookies) = ctx
            .browser
            .import_cookies_auto(self.domain, Browser::default_priority())
            .await
            .map_err(FetchError::Browser)?;

        let cookie_header =
            exactobar_fetch::host::browser::BrowserCookieImporter::cookies_to_header(&cookies);

        // Validate we have Hailuoai-specific cookies
        if !MiniMaxWebClient::has_hailuoai_session_cookie(&cookie_header) {
            return Err(FetchError::AuthenticationFailed(
                "No valid Hailuoai session cookie found".to_string(),
            ));
        }

        let response = ctx
            .http
            .get_with_cookies(HAILUOAI_API, &cookie_header)
            .await
            .map_err(|e| FetchError::InvalidResponse(e.to_string()))?;

        if response.status() == reqwest::StatusCode::UNAUTHORIZED {
            return Err(FetchError::AuthenticationFailed(
                "Hailuoai cookies rejected".to_string(),
            ));
        }

        if !response.status().is_success() {
            return Err(FetchError::InvalidResponse(format!(
                "Hailuoai API returned {}",
                response.status()
            )));
        }

        let body = response
            .text()
            .await
            .map_err(|e| FetchError::InvalidResponse(e.to_string()))?;

        let snapshot = parse_minimax_response(&body)?;
        info!("Fetched MiniMax usage from hailuoai.com");
        Ok(FetchResult::new(snapshot, self.id(), self.kind()))
    }

    fn priority(&self) -> u32 {
        90 // Slightly lower than minimax.chat direct
    }
}

// ============================================================================
// LocalStorage Strategy
// ============================================================================

/// Strategy for fetching using browser localStorage tokens.
///
/// MiniMax stores auth tokens in browser localStorage under hailuoai.com.
/// This strategy attempts to extract and use those tokens.
pub struct MiniMaxLocalStorageStrategy;

impl MiniMaxLocalStorageStrategy {
    pub fn new() -> Self {
        Self
    }
}

impl Default for MiniMaxLocalStorageStrategy {
    fn default() -> Self {
        Self::new()
    }
}

#[async_trait]
impl FetchStrategy for MiniMaxLocalStorageStrategy {
    fn id(&self) -> &str {
        "minimax.localstorage"
    }

    fn kind(&self) -> FetchKind {
        FetchKind::LocalProbe
    }

    #[instrument(skip(self, _ctx))]
    async fn is_available(&self, _ctx: &FetchContext) -> bool {
        MiniMaxLocalStorage::has_storage()
    }

    #[instrument(skip(self, ctx))]
    async fn fetch(&self, ctx: &FetchContext) -> Result<FetchResult, FetchError> {
        debug!("Fetching MiniMax usage via localStorage token");

        let token = MiniMaxLocalStorage::find_token().ok_or_else(|| {
            FetchError::AuthenticationFailed("No MiniMax token found in localStorage".to_string())
        })?;

        // Use the token with the API
        let auth_header = format!("Bearer {}", token);
        let response = ctx
            .http
            .get_with_auth(MINIMAX_API, &auth_header)
            .await
            .map_err(|e| FetchError::InvalidResponse(e.to_string()))?;

        if response.status() == reqwest::StatusCode::UNAUTHORIZED {
            return Err(FetchError::AuthenticationFailed(
                "localStorage token rejected".to_string(),
            ));
        }

        if !response.status().is_success() {
            return Err(FetchError::InvalidResponse(format!(
                "API returned {}",
                response.status()
            )));
        }

        let body = response
            .text()
            .await
            .map_err(|e| FetchError::InvalidResponse(e.to_string()))?;

        let snapshot = parse_minimax_response(&body)?;
        info!("Fetched MiniMax usage from localStorage token");
        Ok(FetchResult::new(snapshot, self.id(), self.kind()))
    }

    fn priority(&self) -> u32 {
        80 // Lower than web cookies
    }
}

// ============================================================================
// Local Strategy
// ============================================================================

pub struct MiniMaxLocalStrategy;

impl MiniMaxLocalStrategy {
    pub fn new() -> Self {
        Self
    }

    fn config_path() -> Option<PathBuf> {
        dirs::config_dir().map(|c| c.join("minimax").join("state.json"))
    }
}

impl Default for MiniMaxLocalStrategy {
    fn default() -> Self {
        Self::new()
    }
}

#[async_trait]
impl FetchStrategy for MiniMaxLocalStrategy {
    fn id(&self) -> &str {
        "minimax.local"
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
        debug!("Fetching MiniMax usage from local config");

        let path = Self::config_path()
            .ok_or_else(|| FetchError::InvalidResponse("No config path".to_string()))?;

        let content = std::fs::read_to_string(&path)
            .map_err(|e| FetchError::InvalidResponse(e.to_string()))?;

        let snapshot = parse_minimax_response(&content)?;
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
        let s = MiniMaxWebStrategy::new();
        assert_eq!(s.id(), "minimax.web");
        assert_eq!(s.priority(), 100);
    }

    #[test]
    fn test_hailuoai_strategy() {
        let s = HailuoaiWebStrategy::new();
        assert_eq!(s.id(), "minimax.hailuoai");
        assert_eq!(s.priority(), 90);
    }

    #[test]
    fn test_localstorage_strategy() {
        let s = MiniMaxLocalStorageStrategy::new();
        assert_eq!(s.id(), "minimax.localstorage");
        assert_eq!(s.priority(), 80);
    }

    #[test]
    fn test_local_strategy() {
        let s = MiniMaxLocalStrategy::new();
        assert_eq!(s.id(), "minimax.local");
        assert_eq!(s.priority(), 60);
    }

    #[test]
    fn test_strategy_priority_order() {
        // Verify priority order: web > hailuoai > localstorage > local
        let web = MiniMaxWebStrategy::new();
        let hailuoai = HailuoaiWebStrategy::new();
        let localstorage = MiniMaxLocalStorageStrategy::new();
        let local = MiniMaxLocalStrategy::new();

        assert!(web.priority() > hailuoai.priority());
        assert!(hailuoai.priority() > localstorage.priority());
        assert!(localstorage.priority() > local.priority());
    }
}
