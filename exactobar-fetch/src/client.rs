//! HTTP client abstractions.

use crate::error::FetchError;
use crate::retry::RetryStrategy;
use reqwest::{header, Client, Response};
use std::time::Duration;
use tracing::{debug, warn};

/// Default request timeout in seconds.
const DEFAULT_TIMEOUT_SECS: u64 = 30;

/// HTTP client with retry capabilities.
#[derive(Debug, Clone)]
pub struct HttpClient {
    inner: Client,
    retry_strategy: RetryStrategy,
}

impl HttpClient {
    /// Creates a new HTTP client with default settings.
    pub fn new() -> Result<Self, FetchError> {
        Self::with_timeout(Duration::from_secs(DEFAULT_TIMEOUT_SECS))
    }

    /// Creates a new HTTP client with a custom timeout.
    pub fn with_timeout(timeout: Duration) -> Result<Self, FetchError> {
        let client = Client::builder()
            .timeout(timeout)
            .user_agent(concat!("exactobar/", env!("CARGO_PKG_VERSION")))
            .build()?;

        Ok(Self {
            inner: client,
            retry_strategy: RetryStrategy::default(),
        })
    }

    /// Sets the retry strategy for this client.
    pub fn with_retry_strategy(mut self, strategy: RetryStrategy) -> Self {
        self.retry_strategy = strategy;
        self
    }

    /// Performs a GET request with authentication.
    pub async fn get_with_auth(
        &self,
        url: &str,
        auth_header: &str,
    ) -> Result<Response, FetchError> {
        let mut attempts = 0;
        let max_attempts = self.retry_strategy.max_attempts;

        loop {
            attempts += 1;
            debug!(url = %url, attempt = attempts, "Making GET request");

            let result = self
                .inner
                .get(url)
                .header(header::AUTHORIZATION, auth_header)
                .send()
                .await;

            match result {
                Ok(response) => {
                    if response.status().is_success() {
                        return Ok(response);
                    }

                    // Handle rate limiting
                    if response.status() == reqwest::StatusCode::TOO_MANY_REQUESTS {
                        let retry_after = response
                            .headers()
                            .get(header::RETRY_AFTER)
                            .and_then(|v| v.to_str().ok())
                            .and_then(|v| v.parse().ok());

                        if attempts < max_attempts {
                            let wait_time = retry_after
                                .unwrap_or(self.retry_strategy.base_delay_secs);
                            warn!(
                                "Rate limited, waiting {} seconds before retry",
                                wait_time
                            );
                            tokio::time::sleep(Duration::from_secs(wait_time)).await;
                            continue;
                        }

                        return Err(FetchError::RateLimited { retry_after });
                    }

                    // Handle auth errors
                    if response.status() == reqwest::StatusCode::UNAUTHORIZED {
                        return Err(FetchError::AuthenticationFailed(
                            "Invalid or expired credentials".to_string(),
                        ));
                    }

                    return Err(FetchError::InvalidResponse(format!(
                        "Unexpected status code: {}",
                        response.status()
                    )));
                }
                Err(e) => {
                    if attempts < max_attempts && self.retry_strategy.should_retry(&e) {
                        let delay = self.retry_strategy.delay_for_attempt(attempts);
                        warn!(
                            error = %e,
                            delay_secs = delay.as_secs(),
                            "Request failed, retrying"
                        );
                        tokio::time::sleep(delay).await;
                        continue;
                    }
                    return Err(e.into());
                }
            }
        }
    }

    /// Performs a simple GET request without authentication.
    pub async fn get(&self, url: &str) -> Result<Response, FetchError> {
        Ok(self.inner.get(url).send().await?)
    }
}

impl Default for HttpClient {
    /// Creates a default HTTP client.
    ///
    /// # Panics
    ///
    /// Panics if the HTTP client cannot be created. This should only happen
    /// if the system's TLS configuration is broken, which indicates a
    /// fundamentally broken environment where the application cannot function.
    fn default() -> Self {
        Self::new().unwrap_or_else(|e| {
            panic!(
                "Failed to create default HTTP client: {}. \
                This usually indicates a broken TLS/SSL configuration.",
                e
            )
        })
    }
}
