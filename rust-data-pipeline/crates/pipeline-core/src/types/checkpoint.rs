//! Checkpoint — the Memento pattern for exactly-once resumption.

use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};

/// Represents the exact position a pipeline has processed up to.
/// Persisted atomically to Object Storage (S3/GCS) after every successful
/// sink write, enabling crash-safe resumption.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Checkpoint {
    /// The pipeline this checkpoint belongs to.
    pub pipeline_id: String,

    /// Opaque offset string — meaning depends on the source type:
    /// - Kafka: partition:offset (e.g., "0:12345")
    /// - PostgreSQL CDC: WAL LSN (e.g., "0/16B3748")
    /// - SaaS API: cursor token or timestamp
    /// - S3: last processed object key
    pub offset: String,

    /// When this checkpoint was created.
    pub timestamp: DateTime<Utc>,

    /// Number of records processed since last checkpoint.
    pub records_since_last: u64,
}

impl Checkpoint {
    pub fn new(pipeline_id: impl Into<String>, offset: impl Into<String>) -> Self {
        Self {
            pipeline_id: pipeline_id.into(),
            offset: offset.into(),
            timestamp: Utc::now(),
            records_since_last: 0,
        }
    }
}
