//! Pipeline Worker — the per-pipeline async task that runs the Source → Transform → Sink loop.

use std::time::Instant;
use tokio::sync::watch::Receiver;
use tracing::{info, warn, error, debug};

use pipeline_core::error::PipelineError;
use pipeline_core::traits::source::SourceConnector;
use pipeline_core::traits::sink::SinkConnector;
use pipeline_core::traits::transformer::Transformer;
use pipeline_core::types::checkpoint::Checkpoint;
use pipeline_core::types::pipeline_config::PipelineConfig;

use crate::rate_limit::{PipelineRateLimiter, NoOpRateLimiter};

/// A worker runs a single pipeline's data flow loop.
pub struct PipelineWorker {
    config: PipelineConfig,
    source: Box<dyn SourceConnector>,
    sink: Box<dyn SinkConnector>,
    transforms: Vec<Box<dyn Transformer>>,
    cancel_rx: Receiver<bool>,
    rate_limiter: Box<dyn RateLimiter>,
}

/// Trait for rate limiters to allow different implementations.
trait RateLimiter: Send + Sync {
    async fn wait_for_source_permit(&self) -> Result<(), Box<dyn std::error::Error + Send + Sync>>;
    async fn wait_for_sink_permit(&self) -> Result<(), Box<dyn std::error::Error + Send + Sync>>;
}

impl RateLimiter for PipelineRateLimiter {
    async fn wait_for_source_permit(&self) -> Result<(), Box<dyn std::error::Error + Send + Sync>> {
        self.wait_for_source_permit().await.map_err(|e| Box::new(e) as Box<dyn std::error::Error + Send + Sync>)
    }

    async fn wait_for_sink_permit(&self) -> Result<(), Box<dyn std::error::Error + Send + Sync>> {
        self.wait_for_sink_permit().await.map_err(|e| Box::new(e) as Box<dyn std::error::Error + Send + Sync>)
    }
}

impl RateLimiter for NoOpRateLimiter {
    async fn wait_for_source_permit(&self) -> Result<(), Box<dyn std::error::Error + Send + Sync>> {
        self.wait_for_source_permit().await.map_err(|e| Box::new(e) as Box<dyn std::error::Error + Send + Sync>)
    }

    async fn wait_for_sink_permit(&self) -> Result<(), Box<dyn std::error::Error + Send + Sync>> {
        self.wait_for_sink_permit().await.map_err(|e| Box::new(e) as Box<dyn std::error::Error + Send + Sync>)
    }
}

impl PipelineWorker {
    pub fn new(
        config: PipelineConfig,
        source: Box<dyn SourceConnector>,
        sink: Box<dyn SinkConnector>,
        transforms: Vec<Box<dyn Transformer>>,
        cancel_rx: Receiver<bool>,
    ) -> Self {
        // Create rate limiter based on configuration
        let rate_limiter: Box<dyn RateLimiter> = if config.rate_limits.max_source_tps > 0 || config.rate_limits.max_sink_tps > 0 {
            Box::new(PipelineRateLimiter::new(config.rate_limits.clone()))
        } else {
            Box::new(NoOpRateLimiter::new())
        };

        Self {
            config,
            source,
            sink,
            transforms,
            cancel_rx,
            rate_limiter,
        }
    }

    /// Main execution loop: poll source → apply transforms → write to sink → checkpoint.
    pub async fn run(mut self) -> Result<(), PipelineError> {
        let pipeline_id = &self.config.pipeline_id;
        info!(pipeline_id = %pipeline_id, source = %self.source.name(), sink = %self.sink.name(), "Starting pipeline worker");

        // Step 1: Connect source and sink
        self.source.connect().await?;
        info!(pipeline_id = %pipeline_id, "Source connected: {}", self.source.name());

        self.sink.connect().await?;
        info!(pipeline_id = %pipeline_id, "Sink connected: {}", self.sink.name());

        let mut total_records: u64 = 0;
        let mut batch_count: u64 = 0;

        // Step 2: Main data flow loop
        loop {
            // Check for cancellation
            if *self.cancel_rx.borrow() {
                info!(pipeline_id = %pipeline_id, "Received stop signal, shutting down gracefully");
                break;
            }

            // Apply source rate limiting before polling
            if let Err(e) = self.rate_limiter.wait_for_source_permit().await {
                warn!(pipeline_id = %pipeline_id, error = %e, "Source rate limiting error, proceeding anyway");
            }

            // Poll a batch from the source
            let batch = match self.source.poll_batch().await {
                Ok(batch) => batch,
                Err(e) => {
                    warn!(pipeline_id = %pipeline_id, error = %e, "Source poll error, retrying...");
                    tokio::time::sleep(std::time::Duration::from_secs(1)).await;
                    continue;
                }
            };

            // Skip empty batches
            if batch.num_rows() == 0 {
                debug!(pipeline_id = %pipeline_id, "Empty batch, backing off");
                tokio::time::sleep(std::time::Duration::from_millis(
                    self.config.flags.batch_flush_interval_ms,
                )).await;
                continue;
            }

            let num_rows = batch.num_rows();
            let start = Instant::now();

            // Step 3: Apply transformation chain (Pipes and Filters pattern)
            let mut transformed_batch = batch;
            for transformer in &self.transforms {
                transformed_batch = transformer.transform(transformed_batch)?;
            }

            // Step 4: Apply sink rate limiting before writing
            if let Err(e) = self.rate_limiter.wait_for_sink_permit().await {
                warn!(pipeline_id = %pipeline_id, error = %e, "Sink rate limiting error, proceeding anyway");
            }

            // Write to sink
            let receipt = self.sink.write_batch(&transformed_batch).await?;

            // Step 5: Acknowledge (advance source offset)
            let checkpoint = Checkpoint::new(
                pipeline_id.clone(),
                format!("batch_{}", batch_count),
            );
            self.source.acknowledge(&checkpoint).await?;

            total_records += receipt.rows_written;
            batch_count += 1;
            let elapsed = start.elapsed();

            info!(
                pipeline_id = %pipeline_id,
                batch = batch_count,
                rows = num_rows,
                written = receipt.rows_written,
                elapsed_ms = elapsed.as_millis(),
                total_records = total_records,
                "Batch processed"
            );
        }

        // Step 6: Graceful shutdown
        info!(pipeline_id = %pipeline_id, "Flushing sink buffers");
        self.sink.flush().await?;
        self.sink.disconnect().await?;
        self.source.disconnect().await?;

        info!(
            pipeline_id = %pipeline_id,
            total_records = total_records,
            total_batches = batch_count,
            "Pipeline worker shutdown complete"
        );

        Ok(())
    }
}
