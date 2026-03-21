//! DLQ Record — wraps a failed record with error context for the Dead Letter Queue.

use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};

/// A record that failed validation, transformation, or sink write.
/// Serialized to Parquet and dumped into partitioned S3 paths.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct DlqRecord {
    /// The pipeline that produced this failed record.
    pub pipeline_id: String,

    /// The original payload that failed (as raw JSON).
    pub original_payload: serde_json::Value,

    /// Classification of the error.
    pub error_type: DlqErrorType,

    /// Human-readable error message.
    pub error_message: String,

    /// When this failure occurred.
    pub timestamp: DateTime<Utc>,

    /// Number of retry attempts made before DLQ routing.
    pub retry_count: u32,
}

/// Categories of DLQ errors for partitioned storage.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub enum DlqErrorType {
    /// Schema validation failure (missing field, wrong type).
    SchemaValidation,
    /// Transformation failure (UDF error, encryption failure).
    TransformError,
    /// Sink write failure after all retries exhausted.
    SinkWriteError,
    /// Deserialization failure (malformed JSON, corrupt Avro).
    DeserializationError,
}

impl std::fmt::Display for DlqErrorType {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::SchemaValidation => write!(f, "schema_validation"),
            Self::TransformError => write!(f, "transform_error"),
            Self::SinkWriteError => write!(f, "sink_write_error"),
            Self::DeserializationError => write!(f, "deserialization_error"),
        }
    }
}
