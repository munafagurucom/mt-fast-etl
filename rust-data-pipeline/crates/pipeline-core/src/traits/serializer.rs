//! Custom serialization/deserialization traits for proprietary data formats.

use arrow::record_batch::RecordBatch;

use crate::error::PipelineError;

/// Deserializes raw bytes from a source into an Arrow RecordBatch.
/// Used for JSON, Avro, CSV, Parquet, or user-defined proprietary formats.
pub trait PayloadDeserializer: Send + Sync {
    /// Format identifier (e.g., "json", "avro", "custom_fix").
    fn format_name(&self) -> &str;

    /// Convert raw bytes into a structured RecordBatch.
    fn deserialize(&self, raw: &[u8]) -> Result<RecordBatch, PipelineError>;
}

/// Serializes an Arrow RecordBatch into bytes for the sink connector.
/// Used for NDJSON output, Parquet file generation, or custom egress formats.
pub trait PayloadSerializer: Send + Sync {
    /// Format identifier.
    fn format_name(&self) -> &str;

    /// Convert a RecordBatch into serialized bytes.
    fn serialize(&self, batch: &RecordBatch) -> Result<Vec<u8>, PipelineError>;
}
