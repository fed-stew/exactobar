//! Retry strategies for HTTP requests.

use std::time::Duration;

/// Strategy for retrying failed requests.
#[derive(Debug, Clone)]
pub struct RetryStrategy {
    /// Maximum number of retry attempts.
    pub max_attempts: u32,
    /// Base delay between retries in seconds.
    pub base_delay_secs: u64,
    /// Whether to use exponential backoff.
    pub exponential_backoff: bool,
    /// Maximum delay between retries.
    pub max_delay_secs: u64,
}

impl RetryStrategy {
    /// Creates a new retry strategy.
    pub fn new(max_attempts: u32) -> Self {
        Self {
            max_attempts,
            base_delay_secs: 1,
            exponential_backoff: true,
            max_delay_secs: 60,
        }
    }

    /// Disables retries.
    pub fn no_retry() -> Self {
        Self {
            max_attempts: 1,
            base_delay_secs: 0,
            exponential_backoff: false,
            max_delay_secs: 0,
        }
    }

    /// Sets the base delay.
    pub fn with_base_delay(mut self, secs: u64) -> Self {
        self.base_delay_secs = secs;
        self
    }

    /// Enables or disables exponential backoff.
    pub fn with_exponential_backoff(mut self, enabled: bool) -> Self {
        self.exponential_backoff = enabled;
        self
    }

    /// Calculates the delay for a given attempt number.
    pub fn delay_for_attempt(&self, attempt: u32) -> Duration {
        let delay = if self.exponential_backoff {
            self.base_delay_secs * 2u64.pow(attempt.saturating_sub(1))
        } else {
            self.base_delay_secs
        };

        Duration::from_secs(delay.min(self.max_delay_secs))
    }

    /// Determines if a request error should be retried.
    pub fn should_retry(&self, error: &reqwest::Error) -> bool {
        // Retry on connection errors and timeouts
        error.is_connect() || error.is_timeout()
    }
}

impl Default for RetryStrategy {
    fn default() -> Self {
        Self::new(3)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_exponential_backoff() {
        let strategy = RetryStrategy::default();

        assert_eq!(strategy.delay_for_attempt(1), Duration::from_secs(1));
        assert_eq!(strategy.delay_for_attempt(2), Duration::from_secs(2));
        assert_eq!(strategy.delay_for_attempt(3), Duration::from_secs(4));
        assert_eq!(strategy.delay_for_attempt(4), Duration::from_secs(8));
    }

    #[test]
    fn test_max_delay_cap() {
        let strategy = RetryStrategy::new(10).with_base_delay(10);

        // Should be capped at 60 seconds
        assert_eq!(strategy.delay_for_attempt(5), Duration::from_secs(60));
    }
}
