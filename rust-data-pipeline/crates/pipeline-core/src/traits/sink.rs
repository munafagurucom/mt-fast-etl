//! Sink connector trait — the universal interface every data destination must implement.

use arrow::record_batch::RecordBatch;
use async_trait::async_trait;

use crate::error::PipelineError;
use crate::types::write_receipt::WriteReceipt;

/// Every sink connector (PostgreSQL, Snowflake, Delta Lake, S3, Redis, etc.)
/// implements this trait. The orchestrator pushes transformed RecordBatches
/// through this interface exclusively.
#[async_trait]
pub trait SinkConnector: Send + Sync {
    /// Human-readable name of this connector (e.g., "snowflake", "postgres").
    fn name(&self) -> &str;

    /// Establish the connection (or connection pool) to the destination.
    async fn connect(&mut self) -> Result<(), PipelineError>;

    /// Write a batch of records to the destination.
    /// Returns a WriteReceipt acknowledging how many rows were persisted
    /// and the time taken for observability tracking.
    async fn write_batch(&mut self, batch: &RecordBatch) -> Result<WriteReceipt, PipelineError>;

    /// Flush any internal buffers to the destination.
    /// For warehouse sinks, this triggers the Parquet staging → COPY INTO flow.
    /// For operational databases, this triggers a transaction commit.
    async fn flush(&mut self) -> Result<(), PipelineError>;

    /// Gracefully close the connection pool and release resources.
    async fn disconnect(&mut self) -> Result<(), PipelineError>;
}
