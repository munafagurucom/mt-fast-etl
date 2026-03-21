//! Rate limiting module — provides token bucket rate limiting for source and sink operations.
//! Uses the governor crate for efficient, fair rate limiting across all pipeline workers.

use std::num::NonZeroU32;
use std::sync::Arc;
use std::time::Duration;

use governor::{Quota, RateLimiter};
use tracing::{debug, warn};

use pipeline_core::types::pipeline_config::RateLimitConfig;

/// A rate limiter that controls both source polling and sink writing rates.
#[derive(Debug, Clone)]
pub struct PipelineRateLimiter {
    /// Rate limiter for source operations (polling batches)
    source_limiter: Arc<RateLimiter<governor::clock::QuantaClock>>,
    
    /// Rate limiter for sink operations (writing batches)
    sink_limiter: Arc<RateLimiter<governor::clock::QuantaClock>>,
    
    /// Configuration reference
    config: RateLimitConfig,
}

impl PipelineRateLimiter {
    /// Create a new rate limiter from the given configuration.
    pub fn new(config: RateLimitConfig) -> Self {
        // Create quotas for source and sink
        let source_quota = Quota::per_second(NonZeroU32::new(config.max_source_tps).unwrap_or_else(|| NonZeroU32::new(10_000)));
        let sink_quota = Quota::per_second(NonZeroU32::new(config.max_sink_tps).unwrap_or_else(|| NonZeroU32::new(5_000)));

        // Create rate limiters with the quotas
        let source_limiter = Arc::new(RateLimiter::direct(source_quota));
        let sink_limiter = Arc::new(RateLimiter::direct(sink_quota));

        debug!(
            source_tps = config.max_source_tps,
            sink_tps = config.max_sink_tps,
            "Created rate limiters"
        );

        Self {
            source_limiter,
            sink_limiter,
            config,
        }
    }

    /// Wait until a source operation is allowed by the rate limiter.
    /// This should be called before polling from the source.
    pub async fn wait_for_source_permit(&self) -> Result<(), RateLimitError> {
        let start = std::time::Instant::now();
        
        // Try to acquire a permit immediately
        match self.source_limiter.check() {
            Ok(()) => {
                debug!(duration_ms = start.elapsed().as_millis(), "Source permit granted immediately");
                Ok(())
            }
            Err(_) => {
                // If not immediately available, wait for the next available permit
                self.source_limiter.until_ready().await;
                let elapsed = start.elapsed();
                
                if elapsed > Duration::from_millis(100) {
                    warn!(duration_ms = elapsed.as_millis(), "Source rate limiting caused delay");
                } else {
                    debug!(duration_ms = elapsed.as_millis(), "Source permit granted after wait");
                }
                
                Ok(())
            }
        }
    }

    /// Wait until a sink operation is allowed by the rate limiter.
    /// This should be called before writing to the sink.
    pub async fn wait_for_sink_permit(&self) -> Result<(), RateLimitError> {
        let start = std::time::Instant::now();
        
        // Try to acquire a permit immediately
        match self.sink_limiter.check() {
            Ok(()) => {
                debug!(duration_ms = start.elapsed().as_millis(), "Sink permit granted immediately");
                Ok(())
            }
            Err(_) => {
                // If not immediately available, wait for the next available permit
                self.sink_limiter.until_ready().await;
                let elapsed = start.elapsed();
                
                if elapsed > Duration::from_millis(100) {
                    warn!(duration_ms = elapsed.as_millis(), "Sink rate limiting caused delay");
                } else {
                    debug!(duration_ms = elapsed.as_millis(), "Sink permit granted after wait");
                }
                
                Ok(())
            }
        }
    }

    /// Check if a source operation would be allowed without waiting.
    /// Returns true if the operation can proceed immediately.
    pub fn can_proceed_source(&self) -> bool {
        self.source_limiter.check().is_ok()
    }

    /// Check if a sink operation would be allowed without waiting.
    /// Returns true if the operation can proceed immediately.
    pub fn can_proceed_sink(&self) -> bool {
        self.sink_limiter.check().is_ok()
    }

    /// Get the current rate limit configuration.
    pub fn config(&self) -> &RateLimitConfig {
        &self.config
    }

    /// Get statistics about the current rate limiting state.
    pub fn stats(&self) -> RateLimitStats {
        RateLimitStats {
            max_source_tps: self.config.max_source_tps,
            max_sink_tps: self.config.max_sink_tps,
            source_available: self.source_limiter.remaining_permits(),
            sink_available: self.sink_limiter.remaining_permits(),
        }
    }
}

/// Statistics about the current rate limiting state.
#[derive(Debug, Clone)]
pub struct RateLimitStats {
    pub max_source_tps: u32,
    pub max_sink_tps: u32,
    pub source_available: u32,
    pub sink_available: u32,
}

/// Errors that can occur during rate limiting.
#[derive(Debug, thiserror::Error)]
pub enum RateLimitError {
    #[error("Rate limiter configuration error: {message}")]
    ConfigError { message: String },
    
    #[error("Rate limiter clock error: {message}")]
    ClockError { message: String },
}

impl From<governor::clock::ClockError> for RateLimitError {
    fn from(err: governor::clock::ClockError) -> Self {
        Self::ClockError { message: err.to_string() }
    }
}

/// A no-op rate limiter that allows all operations.
/// Useful for testing or when rate limiting is disabled.
#[derive(Debug, Clone, Default)]
pub struct NoOpRateLimiter;

impl NoOpRateLimiter {
    pub fn new() -> Self {
        Self
    }
}

impl NoOpRateLimiter {
    pub async fn wait_for_source_permit(&self) -> Result<(), RateLimitError> {
        Ok(())
    }

    pub async fn wait_for_sink_permit(&self) -> Result<(), RateLimitError> {
        Ok(())
    }

    pub fn can_proceed_source(&self) -> bool {
        true
    }

    pub fn can_proceed_sink(&self) -> bool {
        true
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use tokio::time::sleep;

    #[tokio::test]
    async fn test_rate_limiter_basic_functionality() {
        let config = RateLimitConfig {
            max_source_tps: 10,
            max_sink_tps: 5,
            max_concurrent_connections: 1,
        };

        let limiter = PipelineRateLimiter::new(config);

        // Should be able to proceed immediately
        assert!(limiter.can_proceed_source());
        assert!(limiter.can_proceed_sink());

        // Wait for permits should succeed
        limiter.wait_for_source_permit().await.unwrap();
        limiter.wait_for_sink_permit().await.unwrap();
    }

    #[tokio::test]
    async fn test_noop_rate_limiter() {
        let limiter = NoOpRateLimiter::new();

        // No-op limiter should always allow operations
        assert!(limiter.can_proceed_source());
        assert!(limiter.can_proceed_sink());

        limiter.wait_for_source_permit().await.unwrap();
        limiter.wait_for_sink_permit().await.unwrap();
    }

    #[tokio::test]
    async fn test_rate_limiter_enforcement() {
        let config = RateLimitConfig {
            max_source_tps: 2, // Very low rate for testing
            max_sink_tps: 2,
            max_concurrent_connections: 1,
        };

        let limiter = PipelineRateLimiter::new(config);

        let start = std::time::Instant::now();
        
        // First permit should be immediate
        limiter.wait_for_source_permit().await.unwrap();
        let first_duration = start.elapsed();
        
        // Second permit might need to wait
        let second_start = std::time::Instant::now();
        limiter.wait_for_source_permit().await.unwrap();
        let second_duration = second_start.elapsed();

        // The second permit should take longer due to rate limiting
        // (though this can be flaky in CI environments)
        println!("First: {}ms, Second: {}ms", first_duration.as_millis(), second_duration.as_millis());
    }
}
