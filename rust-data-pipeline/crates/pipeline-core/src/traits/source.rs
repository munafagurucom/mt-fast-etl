//! Source connector trait — the universal interface every data source must implement.

use arrow::record_batch::RecordBatch;
use async_trait::async_trait;

use crate::error::PipelineError;
use crate::types::checkpoint::Checkpoint;

/// Every source connector (Kafka, PostgreSQL CDC, REST API, WASM plugin, etc.)
/// implements this trait. The orchestrator interacts with sources exclusively
/// through this interface, enabling the Abstract Factory pattern.
#[async_trait]
pub trait SourceConnector: Send + Sync {
    /// Human-readable name of this connector (e.g., "kafka", "postgres_cdc").
    fn name(&self) -> &str;

    /// Establish the connection to the external source system.
    /// Called once when the pipeline is first spawned.
    async fn connect(&mut self) -> Result<(), PipelineError>;

    /// Poll a batch of records from the source.
    /// Returns an Arrow RecordBatch for zero-copy columnar processing downstream.
    /// Returns an empty batch if no new data is available (the orchestrator will
    /// back off via the configured rate limiter).
    async fn poll_batch(&mut self) -> Result<RecordBatch, PipelineError>;

    /// Acknowledge that records up to this checkpoint have been successfully
    /// written to the sink. The source can safely advance its internal offset
    /// (e.g., commit Kafka offsets, advance PostgreSQL replication slot LSN).
    async fn acknowledge(&mut self, checkpoint: &Checkpoint) -> Result<(), PipelineError>;

    /// Gracefully disconnect from the source system.
    /// Called during pipeline shutdown or pause.
    async fn disconnect(&mut self) -> Result<(), PipelineError>;
}
