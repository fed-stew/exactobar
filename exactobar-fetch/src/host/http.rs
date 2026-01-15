//! HTTP client with tracing, retries, and domain allowlist.
//!
//! This module provides a wrapped HTTP client that adds:
//! - Request/response tracing
//! - Domain allowlist for security
//! - Cookie support for web scraping
//! - Convenience methods for common operations

use reqwest::{header, header::HeaderMap, Client, Response};
use std::time::Duration;
use tracing::{debug, instrument};
use url::Url;

use crate::error::HttpError;

/// Default request timeout.
const DEFAULT_TIMEOUT_SECS: u64 = 30;

/// User agent string for ExactoBar.
const USER_AGENT: &str = concat!("ExactoBar/", env!("CARGO_PKG_VERSION"));

// ============================================================================
// HTTP Client
// ============================================================================

/// HTTP client wrapper with tracing, retries, and domain allowlist.
#[derive(Debug, Clone)]
pub struct HttpClient {
    inner: Client,
    allowed_domains: Option<Vec<String>>,
}

impl HttpClient {
    /// Creates a new HTTP client with default settings.
    pub fn new() -> Self {
        Self::with_timeout(Duration::from_secs(DEFAULT_TIMEOUT_SECS))
    }

    /// Creates a new HTTP client with a custom timeout.
    ///
    /// # Panics
    ///
    /// Panics if the HTTP client cannot be built. This should only occur
    /// if the system's TLS/SSL configuration is fundamentally broken,
    /// making network operations impossible. This is considered
    /// unrecoverable at runtime.
    pub fn with_timeout(timeout: Duration) -> Self {
        let client = Client::builder()
            .timeout(timeout)
            .user_agent(USER_AGENT)
            .build()
            .unwrap_or_else(|e| {
                panic!(
                    "Failed to create HTTP client: {}. \
                    This usually indicates a broken TLS/SSL configuration.",
                    e
                )
            });

        Self {
            inner: client,
            allowed_domains: None,
        }
    }

    /// Creates a new HTTP client with domain allowlist.
    ///
    /// Only requests to domains in the allowlist will be permitted.
    pub fn with_allowed_domains(domains: Vec<String>) -> Self {
        let mut client = Self::new();
        client.allowed_domains = Some(domains);
        client
    }

    /// Checks if a URL's domain is allowed.
    fn is_domain_allowed(&self, url: &str) -> Result<(), HttpError> {
        let Some(ref allowed) = self.allowed_domains else {
            return Ok(()); // No restrictions
        };

        let parsed = Url::parse(url).map_err(|e| HttpError::InvalidUrl(e.to_string()))?;

        let host = parsed
            .host_str()
            .ok_or_else(|| HttpError::InvalidUrl("No host in URL".to_string()))?;

        // Check if host matches any allowed domain
        let allowed = allowed.iter().any(|domain| {
            host == domain || host.ends_with(&format!(".{}", domain))
        });

        if allowed {
            Ok(())
        } else {
            Err(HttpError::DomainNotAllowed(host.to_string()))
        }
    }

    /// Performs a GET request.
    #[instrument(skip(self), fields(url = %url))]
    pub async fn get(&self, url: &str) -> Result<Response, HttpError> {
        self.is_domain_allowed(url)?;
        debug!("GET request");

        let response = self.inner.get(url).send().await?;
        debug!(status = %response.status(), "Response received");
        Ok(response)
    }

    /// Performs a GET request with custom headers.
    #[instrument(skip(self, headers), fields(url = %url))]
    pub async fn get_with_headers(
        &self,
        url: &str,
        headers: HeaderMap,
    ) -> Result<Response, HttpError> {
        self.is_domain_allowed(url)?;
        debug!("GET request with headers");

        let response = self.inner.get(url).headers(headers).send().await?;
        debug!(status = %response.status(), "Response received");
        Ok(response)
    }

    /// Performs a GET request with an authorization header.
    #[instrument(skip(self, auth_header), fields(url = %url))]
    pub async fn get_with_auth(
        &self,
        url: &str,
        auth_header: &str,
    ) -> Result<Response, HttpError> {
        self.is_domain_allowed(url)?;
        debug!("GET request with auth");

        let response = self
            .inner
            .get(url)
            .header(header::AUTHORIZATION, auth_header)
            .send()
            .await?;
        debug!(status = %response.status(), "Response received");
        Ok(response)
    }

    /// Performs a GET request with cookies.
    ///
    /// Used for web scraping strategies that need browser session cookies.
    #[instrument(skip(self, cookies), fields(url = %url))]
    pub async fn get_with_cookies(
        &self,
        url: &str,
        cookies: &str,
    ) -> Result<Response, HttpError> {
        self.is_domain_allowed(url)?;
        debug!("GET request with cookies");

        let response = self
            .inner
            .get(url)
            .header(header::COOKIE, cookies)
            .send()
            .await?;
        debug!(status = %response.status(), "Response received");
        Ok(response)
    }

    /// Performs a POST request with JSON body.
    #[instrument(skip(self, body), fields(url = %url))]
    pub async fn post_json<T: serde::Serialize + ?Sized>(
        &self,
        url: &str,
        body: &T,
    ) -> Result<Response, HttpError> {
        self.is_domain_allowed(url)?;
        debug!("POST request with JSON");

        let response = self.inner.post(url).json(body).send().await?;
        debug!(status = %response.status(), "Response received");
        Ok(response)
    }

    /// Performs a POST request with form data.
    #[instrument(skip(self, form), fields(url = %url))]
    pub async fn post_form<T: serde::Serialize + ?Sized>(
        &self,
        url: &str,
        form: &T,
    ) -> Result<Response, HttpError> {
        self.is_domain_allowed(url)?;
        debug!("POST request with form data");

        let response = self.inner.post(url).form(form).send().await?;
        debug!(status = %response.status(), "Response received");
        Ok(response)
    }

    /// Returns the inner reqwest client for advanced operations.
    pub fn inner(&self) -> &Client {
        &self.inner
    }
}

impl Default for HttpClient {
    fn default() -> Self {
        Self::new()
    }
}

// ============================================================================
// Response Extensions
// ============================================================================

/// Extension trait for Response handling.
pub trait ResponseExt {
    /// Check if the response indicates rate limiting.
    fn is_rate_limited(&self) -> bool;

    /// Get the Retry-After header value in seconds.
    fn retry_after_secs(&self) -> Option<u64>;
}

impl ResponseExt for Response {
    fn is_rate_limited(&self) -> bool {
        self.status() == reqwest::StatusCode::TOO_MANY_REQUESTS
    }

    fn retry_after_secs(&self) -> Option<u64> {
        self.headers()
            .get(header::RETRY_AFTER)
            .and_then(|v| v.to_str().ok())
            .and_then(|v| v.parse().ok())
    }
}

// ============================================================================
// Tests
// ============================================================================

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_domain_allowlist() {
        let client = HttpClient::with_allowed_domains(vec![
            "api.anthropic.com".to_string(),
            "openai.com".to_string(),
        ]);

        // Allowed domains
        assert!(client.is_domain_allowed("https://api.anthropic.com/v1/usage").is_ok());
        assert!(client.is_domain_allowed("https://api.openai.com/v1/usage").is_ok());

        // Subdomain matching
        assert!(client.is_domain_allowed("https://status.openai.com").is_ok());

        // Not allowed
        assert!(client.is_domain_allowed("https://evil.com/steal").is_err());
    }

    #[test]
    fn test_no_domain_restrictions() {
        let client = HttpClient::new();

        // All domains allowed when no restrictions
        assert!(client.is_domain_allowed("https://any.domain.com").is_ok());
    }

    #[test]
    fn test_invalid_url() {
        let client = HttpClient::with_allowed_domains(vec!["example.com".to_string()]);

        // Completely invalid URL
        assert!(client.is_domain_allowed("not-a-valid-url").is_err());

        // Valid URL but domain not in allowlist
        assert!(client.is_domain_allowed("https://evil.com/path").is_err());
    }
}
