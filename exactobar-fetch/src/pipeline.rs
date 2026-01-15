//! Fetch pipeline for executing strategies in order.
//!
//! The pipeline takes a list of fetch strategies and executes them in
//! priority order until one succeeds.

use std::time::{Duration, Instant};
use tracing::{debug, info, instrument, warn};

use crate::context::FetchContext;
use crate::error::FetchError;
use crate::strategy::{FetchKind, FetchResult, FetchStrategy};

// ============================================================================
// Fetch Attempt
// ============================================================================

/// Record of a single fetch attempt.
#[derive(Debug, Clone)]
pub struct FetchAttempt {
    /// The strategy ID that was attempted.
    pub strategy_id: String,
    /// The kind of fetch used.
    pub kind: FetchKind,
    /// Whether the attempt succeeded.
    pub success: bool,
    /// Error if the attempt failed.
    pub error: Option<String>,
    /// How long the attempt took.
    pub duration: Duration,
}

impl FetchAttempt {
    /// Creates a successful attempt record.
    pub fn success(strategy_id: impl Into<String>, kind: FetchKind, duration: Duration) -> Self {
        Self {
            strategy_id: strategy_id.into(),
            kind,
            success: true,
            error: None,
            duration,
        }
    }

    /// Creates a failed attempt record.
    pub fn failure(
        strategy_id: impl Into<String>,
        kind: FetchKind,
        error: impl Into<String>,
        duration: Duration,
    ) -> Self {
        Self {
            strategy_id: strategy_id.into(),
            kind,
            success: false,
            error: Some(error.into()),
            duration,
        }
    }
}

// ============================================================================
// Fetch Outcome
// ============================================================================

/// The outcome of a fetch pipeline execution.
#[derive(Debug)]
pub struct FetchOutcome {
    /// The result (success or final error).
    pub result: Result<FetchResult, FetchError>,
    /// All attempts made.
    pub attempts: Vec<FetchAttempt>,
    /// Total duration of all attempts.
    pub duration: Duration,
}

impl FetchOutcome {
    /// Returns true if the fetch succeeded.
    pub fn is_success(&self) -> bool {
        self.result.is_ok()
    }

    /// Returns the number of strategies that were tried.
    pub fn attempts_count(&self) -> usize {
        self.attempts.len()
    }

    /// Returns the successful strategy ID, if any.
    pub fn successful_strategy(&self) -> Option<&str> {
        self.result.as_ref().ok().map(|r| r.strategy_id.as_str())
    }

    /// Returns all errors that occurred.
    pub fn errors(&self) -> Vec<&str> {
        self.attempts
            .iter()
            .filter_map(|a| a.error.as_deref())
            .collect()
    }
}

// ============================================================================
// Fetch Pipeline
// ============================================================================

/// A pipeline of fetch strategies tried in order.
///
/// The pipeline executes strategies in priority order until one succeeds.
/// Strategies can opt out of fallback on certain errors.
pub struct FetchPipeline {
    strategies: Vec<Box<dyn FetchStrategy>>,
}

impl FetchPipeline {
    /// Creates an empty pipeline.
    pub fn new() -> Self {
        Self {
            strategies: Vec::new(),
        }
    }

    /// Creates a pipeline with the given strategies.
    pub fn with_strategies(strategies: Vec<Box<dyn FetchStrategy>>) -> Self {
        let mut pipeline = Self { strategies };
        pipeline.sort_by_priority();
        pipeline
    }

    /// Adds a strategy to the pipeline.
    pub fn add_strategy(&mut self, strategy: Box<dyn FetchStrategy>) {
        self.strategies.push(strategy);
        self.sort_by_priority();
    }

    /// Sorts strategies by priority (highest first).
    fn sort_by_priority(&mut self) {
        self.strategies.sort_by(|a, b| b.priority().cmp(&a.priority()));
    }

    /// Returns the number of strategies in the pipeline.
    pub fn len(&self) -> usize {
        self.strategies.len()
    }

    /// Returns true if the pipeline is empty.
    pub fn is_empty(&self) -> bool {
        self.strategies.is_empty()
    }

    /// Returns information about all strategies.
    pub async fn strategy_info(&self, ctx: &FetchContext) -> Vec<crate::strategy::StrategyInfo> {
        let mut info = Vec::with_capacity(self.strategies.len());
        for strategy in &self.strategies {
            info.push(crate::strategy::StrategyInfo::from_strategy(strategy.as_ref(), ctx).await);
        }
        info
    }

    /// Execute the pipeline, trying strategies in order until one succeeds.
    #[instrument(skip(self, ctx), fields(strategies = self.strategies.len()))]
    pub async fn execute(&self, ctx: &FetchContext) -> FetchOutcome {
        let start = Instant::now();
        let mut attempts = Vec::new();

        if self.strategies.is_empty() {
            return FetchOutcome {
                result: Err(FetchError::StrategyNotAvailable(
                    "No strategies configured".to_string(),
                )),
                attempts,
                duration: start.elapsed(),
            };
        }

        info!(count = self.strategies.len(), "Executing fetch pipeline");

        for strategy in &self.strategies {
            let strategy_id = strategy.id();
            let kind = strategy.kind();

            debug!(strategy = %strategy_id, kind = %kind, "Checking strategy availability");

            // Check if strategy is available
            if !strategy.is_available(ctx).await {
                debug!(strategy = %strategy_id, "Strategy not available, skipping");
                attempts.push(FetchAttempt::failure(
                    strategy_id,
                    kind,
                    "Not available",
                    Duration::ZERO,
                ));
                continue;
            }

            // Try the strategy
            let attempt_start = Instant::now();
            debug!(strategy = %strategy_id, "Executing strategy");

            match strategy.fetch(ctx).await {
                Ok(result) => {
                    let duration = attempt_start.elapsed();
                    info!(
                        strategy = %strategy_id,
                        duration = ?duration,
                        "Strategy succeeded"
                    );

                    attempts.push(FetchAttempt::success(strategy_id, kind, duration));

                    return FetchOutcome {
                        result: Ok(result),
                        attempts,
                        duration: start.elapsed(),
                    };
                }
                Err(error) => {
                    let duration = attempt_start.elapsed();
                    warn!(
                        strategy = %strategy_id,
                        error = %error,
                        duration = ?duration,
                        "Strategy failed"
                    );

                    attempts.push(FetchAttempt::failure(
                        strategy_id,
                        kind,
                        error.to_string(),
                        duration,
                    ));

                    // Check if we should try the next strategy
                    if !strategy.should_fallback(&error) {
                        debug!(
                            strategy = %strategy_id,
                            "Strategy indicates no fallback"
                        );
                        return FetchOutcome {
                            result: Err(error),
                            attempts,
                            duration: start.elapsed(),
                        };
                    }
                }
            }
        }

        // All strategies failed
        warn!("All strategies failed");
        FetchOutcome {
            result: Err(FetchError::AllStrategiesFailed),
            attempts,
            duration: start.elapsed(),
        }
    }

    /// Execute only available strategies.
    pub async fn execute_available(&self, ctx: &FetchContext) -> FetchOutcome {
        let start = Instant::now();
        let mut attempts = Vec::new();

        // Filter to available strategies
        let mut available = Vec::new();
        for strategy in &self.strategies {
            if strategy.is_available(ctx).await {
                available.push(strategy);
            }
        }

        if available.is_empty() {
            return FetchOutcome {
                result: Err(FetchError::StrategyNotAvailable(
                    "No available strategies".to_string(),
                )),
                attempts,
                duration: start.elapsed(),
            };
        }

        // Execute available strategies
        for strategy in available {
            let strategy_id = strategy.id();
            let kind = strategy.kind();
            let attempt_start = Instant::now();

            match strategy.fetch(ctx).await {
                Ok(result) => {
                    let duration = attempt_start.elapsed();
                    attempts.push(FetchAttempt::success(strategy_id, kind, duration));
                    return FetchOutcome {
                        result: Ok(result),
                        attempts,
                        duration: start.elapsed(),
                    };
                }
                Err(error) => {
                    let duration = attempt_start.elapsed();
                    attempts.push(FetchAttempt::failure(
                        strategy_id,
                        kind,
                        error.to_string(),
                        duration,
                    ));

                    if !strategy.should_fallback(&error) {
                        return FetchOutcome {
                            result: Err(error),
                            attempts,
                            duration: start.elapsed(),
                        };
                    }
                }
            }
        }

        FetchOutcome {
            result: Err(FetchError::AllStrategiesFailed),
            attempts,
            duration: start.elapsed(),
        }
    }
}

impl Default for FetchPipeline {
    fn default() -> Self {
        Self::new()
    }
}

// ============================================================================
// Tests
// ============================================================================

#[cfg(test)]
mod tests {
    use super::*;
    use async_trait::async_trait;
    use exactobar_core::UsageSnapshot;

    struct MockSuccessStrategy {
        id: String,
        available: bool,
        priority: u32,
    }

    impl MockSuccessStrategy {
        fn new(id: &str, available: bool) -> Self {
            Self {
                id: id.to_string(),
                available,
                priority: 50, // Default low priority
            }
        }

        fn with_priority(mut self, priority: u32) -> Self {
            self.priority = priority;
            self
        }
    }

    #[async_trait]
    impl FetchStrategy for MockSuccessStrategy {
        fn id(&self) -> &str {
            &self.id
        }

        fn kind(&self) -> FetchKind {
            FetchKind::CLI
        }

        async fn is_available(&self, _ctx: &FetchContext) -> bool {
            self.available
        }

        async fn fetch(&self, _ctx: &FetchContext) -> Result<FetchResult, FetchError> {
            Ok(FetchResult::new(
                UsageSnapshot::new(),
                self.id.clone(),
                FetchKind::CLI,
            ))
        }

        fn priority(&self) -> u32 {
            self.priority
        }
    }

    struct MockFailStrategy {
        id: String,
        should_fallback: bool,
        priority: u32,
    }

    impl MockFailStrategy {
        fn new(id: &str, should_fallback: bool) -> Self {
            Self {
                id: id.to_string(),
                should_fallback,
                priority: 100, // Default high priority (tried first)
            }
        }

        fn with_priority(mut self, priority: u32) -> Self {
            self.priority = priority;
            self
        }
    }

    #[async_trait]
    impl FetchStrategy for MockFailStrategy {
        fn id(&self) -> &str {
            &self.id
        }

        fn kind(&self) -> FetchKind {
            FetchKind::WebDashboard
        }

        async fn is_available(&self, _ctx: &FetchContext) -> bool {
            true
        }

        async fn fetch(&self, _ctx: &FetchContext) -> Result<FetchResult, FetchError> {
            Err(FetchError::InvalidResponse("Mock error".to_string()))
        }

        fn should_fallback(&self, _error: &FetchError) -> bool {
            self.should_fallback
        }

        fn priority(&self) -> u32 {
            self.priority
        }
    }

    #[tokio::test]
    async fn test_empty_pipeline() {
        let pipeline = FetchPipeline::new();
        let ctx = FetchContext::new();
        let outcome = pipeline.execute(&ctx).await;

        assert!(!outcome.is_success());
        assert!(matches!(
            outcome.result,
            Err(FetchError::StrategyNotAvailable(_))
        ));
    }

    #[tokio::test]
    async fn test_single_success() {
        let pipeline = FetchPipeline::with_strategies(vec![
            Box::new(MockSuccessStrategy::new("test.success", true)),
        ]);

        let ctx = FetchContext::new();
        let outcome = pipeline.execute(&ctx).await;

        assert!(outcome.is_success());
        assert_eq!(outcome.attempts_count(), 1);
        assert_eq!(outcome.successful_strategy(), Some("test.success"));
    }

    #[tokio::test]
    async fn test_fallback_on_failure() {
        // MockFailStrategy with priority 100 (tried first)
        // MockSuccessStrategy with priority 50 (tried second)
        let pipeline = FetchPipeline::with_strategies(vec![
            Box::new(MockFailStrategy::new("test.fail", true).with_priority(100)),
            Box::new(MockSuccessStrategy::new("test.success", true).with_priority(50)),
        ]);

        let ctx = FetchContext::new();
        let outcome = pipeline.execute(&ctx).await;

        assert!(outcome.is_success());
        assert_eq!(outcome.attempts_count(), 2);
        assert_eq!(outcome.successful_strategy(), Some("test.success"));
    }

    #[tokio::test]
    async fn test_no_fallback_stops_pipeline() {
        // MockFailStrategy with priority 100 (tried first), no fallback
        // MockSuccessStrategy with priority 50 (should never be tried)
        let pipeline = FetchPipeline::with_strategies(vec![
            Box::new(MockFailStrategy::new("test.fail", false).with_priority(100)),
            Box::new(MockSuccessStrategy::new("test.success", true).with_priority(50)),
        ]);

        let ctx = FetchContext::new();
        let outcome = pipeline.execute(&ctx).await;

        // Should not succeed because first strategy says no fallback
        assert!(!outcome.is_success());
        assert_eq!(outcome.attempts_count(), 1);
    }

    #[tokio::test]
    async fn test_skip_unavailable() {
        let pipeline = FetchPipeline::with_strategies(vec![
            Box::new(MockSuccessStrategy::new("test.unavailable", false).with_priority(100)),
            Box::new(MockSuccessStrategy::new("test.available", true).with_priority(50)),
        ]);

        let ctx = FetchContext::new();
        let outcome = pipeline.execute(&ctx).await;

        assert!(outcome.is_success());
        assert_eq!(outcome.successful_strategy(), Some("test.available"));
    }
}
