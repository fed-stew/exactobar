//! Probe implementations for health checks.

use crate::client::HttpClient;
use futures::future::join_all;
use std::time::{Duration, Instant};
use tracing::debug;

/// Result of a probe check.
#[derive(Debug, Clone)]
pub struct ProbeResult {
    /// Whether the probe succeeded.
    pub success: bool,
    /// Response time in milliseconds.
    pub response_time_ms: u64,
    /// Optional status code.
    pub status_code: Option<u16>,
    /// Optional error message.
    pub error: Option<String>,
}

/// A probe for checking endpoint availability.
#[derive(Debug, Clone)]
pub struct Probe {
    /// The URL to probe.
    pub url: String,
    /// Request timeout.
    pub timeout: Duration,
}

impl Probe {
    /// Creates a new probe for the given URL.
    pub fn new(url: impl Into<String>) -> Self {
        Self {
            url: url.into(),
            timeout: Duration::from_secs(10),
        }
    }

    /// Sets the timeout for this probe.
    pub fn with_timeout(mut self, timeout: Duration) -> Self {
        self.timeout = timeout;
        self
    }

    /// Executes the probe and returns the result.
    pub async fn check(&self, client: &HttpClient) -> ProbeResult {
        let start = Instant::now();

        debug!(url = %self.url, "Running probe");

        match client.get(&self.url).await {
            Ok(response) => {
                let elapsed = start.elapsed();
                ProbeResult {
                    success: response.status().is_success(),
                    response_time_ms: elapsed.as_millis() as u64,
                    status_code: Some(response.status().as_u16()),
                    error: None,
                }
            }
            Err(e) => {
                let elapsed = start.elapsed();
                ProbeResult {
                    success: false,
                    response_time_ms: elapsed.as_millis() as u64,
                    status_code: None,
                    error: Some(e.to_string()),
                }
            }
        }
    }
}

/// Runs multiple probes concurrently.
pub async fn run_probes(probes: Vec<Probe>, client: &HttpClient) -> Vec<ProbeResult> {
    let futures: Vec<_> = probes.iter().map(|p| p.check(client)).collect();
    join_all(futures).await
}
