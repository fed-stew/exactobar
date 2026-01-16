//! Augment fetch strategies.
//!
//! Augment requires session keepalive to maintain authentication.
//! This strategy sends a keepalive ping before fetching usage data.

use async_trait::async_trait;
#[allow(unused_imports)]
use exactobar_core::{FetchSource, UsageSnapshot};
use exactobar_fetch::{
    FetchContext, FetchError, FetchKind, FetchResult, FetchStrategy, host::browser::Browser,
};
use tracing::{debug, info, instrument, warn};

use super::parser::parse_augment_response;
use super::web::AugmentWebClient;

const AUGMENT_DOMAIN: &str = "augmentcode.com";
const AUGMENT_API: &str = "https://api.augmentcode.com/v1/usage";
const AUGMENT_KEEPALIVE: &str = "https://augmentcode.com/api/keepalive";

pub struct AugmentWebStrategy {
    domain: &'static str,
}

impl AugmentWebStrategy {
    pub fn new() -> Self {
        Self {
            domain: AUGMENT_DOMAIN,
        }
    }
}

impl Default for AugmentWebStrategy {
    fn default() -> Self {
        Self::new()
    }
}

#[async_trait]
impl FetchStrategy for AugmentWebStrategy {
    fn id(&self) -> &str {
        "augment.web"
    }

    fn kind(&self) -> FetchKind {
        FetchKind::WebCookies
    }

    #[instrument(skip(self, _ctx))]
    async fn is_available(&self, _ctx: &FetchContext) -> bool {
        // Don't try to import cookies here - it may hit Chrome Safe Storage keychain!
        // Just check if any browser is installed (no keychain access).
        // Let fetch() handle the actual cookie import and return appropriate errors.
        use exactobar_fetch::host::browser::Browser;
        !Browser::default_priority()
            .iter()
            .filter(|b| b.is_installed())
            .collect::<Vec<_>>()
            .is_empty()
    }

    #[instrument(skip(self, ctx))]
    async fn fetch(&self, ctx: &FetchContext) -> Result<FetchResult, FetchError> {
        debug!("Fetching Augment usage via web cookies");

        let (_, cookies) = ctx
            .browser
            .import_cookies_auto(self.domain, Browser::default_priority())
            .await
            .map_err(FetchError::Browser)?;

        let cookie_header =
            exactobar_fetch::host::browser::BrowserCookieImporter::cookies_to_header(&cookies);

        // Validate we have a session cookie
        if !AugmentWebClient::has_session_cookie(&cookie_header) {
            return Err(FetchError::AuthenticationFailed(
                "No valid Augment session cookie found".to_string(),
            ));
        }

        // Send keepalive to maintain session before fetching usage
        // This is important as Augment sessions can timeout quickly
        debug!("Sending Augment session keepalive");
        let keepalive_result = ctx
            .http
            .inner()
            .post(AUGMENT_KEEPALIVE)
            .header(reqwest::header::COOKIE, &cookie_header)
            .send()
            .await;

        match keepalive_result {
            Ok(response) if response.status().is_success() => {
                info!("Augment keepalive successful");
            }
            Ok(response) => {
                warn!(status = %response.status(), "Augment keepalive returned non-success");
            }
            Err(e) => {
                warn!(error = %e, "Augment keepalive failed, continuing anyway");
            }
        }

        // Now fetch the actual usage data
        let response = ctx
            .http
            .get_with_cookies(AUGMENT_API, &cookie_header)
            .await
            .map_err(|e| FetchError::InvalidResponse(e.to_string()))?;

        if response.status() == reqwest::StatusCode::UNAUTHORIZED {
            return Err(FetchError::AuthenticationFailed(
                "Augment cookies rejected (session may have expired)".to_string(),
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

        let snapshot = parse_augment_response(&body)?;
        info!("Fetched Augment usage successfully");
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
    fn test_web_strategy() {
        let s = AugmentWebStrategy::new();
        assert_eq!(s.id(), "augment.web");
        assert_eq!(s.priority(), 100);
    }

    #[test]
    fn test_keepalive_endpoint_defined() {
        // Verify keepalive endpoint is correctly defined
        assert!(AUGMENT_KEEPALIVE.contains("keepalive"));
        assert!(AUGMENT_KEEPALIVE.starts_with("https://"));
    }

    #[test]
    fn test_has_session_cookie() {
        // Verify the session cookie check is accessible
        assert!(AugmentWebClient::has_session_cookie("__session=abc"));
        assert!(AugmentWebClient::has_session_cookie("connect.sid=xyz"));
        assert!(!AugmentWebClient::has_session_cookie("random=value"));
    }
}
