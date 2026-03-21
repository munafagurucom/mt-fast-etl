//! Pipeline DLQ — Dead Letter Queue writer and replay using object storage.
//! Failed records are serialized and stored in partitioned object storage paths
//! for later analysis and replay.

use std::collections::HashMap;
use std::sync::Arc;

use arrow::json::LineWriter;
use arrow::record_batch::RecordBatch;
use chrono::{DateTime, Utc};
use object_store::{ObjectStore, path::Path, PutPayload};
use serde_json::{json, Value};
use tokio::sync::RwLock;
use tracing::{info, warn, error, debug};
use uuid::Uuid;

use pipeline_core::error::PipelineError;

/// Configuration for Dead Letter Queue storage
#[derive(Debug, Clone)]
pub struct DlqConfig {
    /// Object storage URI (e.g., "s3://bucket-name/dlq/")
    pub storage_uri: String,
    /// Optional prefix for DLQ objects
    pub prefix: Option<String>,
    /// Maximum size of each DLQ batch file in bytes
    pub max_file_size_bytes: usize,
    /// Maximum number of records per DLQ file
    pub max_records_per_file: usize,
    /// Compression format for DLQ files
    pub compression: DlqCompression,
}

/// Compression options for DLQ files
#[derive(Debug, Clone)]
pub enum DlqCompression {
    None,
    Gzip,
    Snappy,
}

impl Default for DlqConfig {
    fn default() -> Self {
        Self {
            storage_uri: "s3://default-dlq-bucket/".to_string(),
            prefix: Some("dlq".to_string()),
            max_file_size_bytes: 100 * 1024 * 1024, // 100MB
            max_records_per_file: 10000,
            compression: DlqCompression::None,
        }
    }
}

/// A Dead Letter Queue record containing the failed data and error context
#[derive(Debug, Clone)]
pub struct DlqRecord {
    /// Unique identifier for this DLQ record
    pub id: String,
    /// Pipeline ID that generated this record
    pub pipeline_id: String,
    /// Timestamp when the record was written to DLQ
    pub timestamp: DateTime<Utc>,
    /// Error category that caused this record to be written to DLQ
    pub error_category: DlqErrorCategory,
    /// Human-readable error message
    pub error_message: String,
    /// Original source data (as JSON string)
    pub original_data: Value,
    /// Additional metadata about the failure
    pub metadata: HashMap<String, String>,
}

/// Categories of errors that can cause records to be written to DLQ
#[derive(Debug, Clone)]
pub enum DlqErrorCategory {
    /// Schema validation failed
    SchemaValidation,
    /// Data transformation failed
    TransformationError,
    /// Sink write failed
    SinkError,
    /// Source format parsing failed
    SourceParsingError,
    /// Network connectivity issues
    NetworkError,
    /// Authentication/authorization errors
    AuthError,
    /// Rate limiting or throttling
    RateLimitError,
    /// Unknown or uncategorized error
    Other,
}

impl DlqErrorCategory {
    fn as_str(&self) -> &'static str {
        match self {
            DlqErrorCategory::SchemaValidation => "schema_validation",
            DlqErrorCategory::TransformationError => "transformation_error",
            DlqErrorCategory::SinkError => "sink_error",
            DlqErrorCategory::SourceParsingError => "source_parsing_error",
            DlqErrorCategory::NetworkError => "network_error",
            DlqErrorCategory::AuthError => "auth_error",
            DlqErrorCategory::RateLimitError => "rate_limit_error",
            DlqErrorCategory::Other => "other",
        }
    }
}

/// Dead Letter Queue writer that stores failed records in object storage
pub struct DlqWriter {
    config: DlqConfig,
    store: Arc<dyn ObjectStore>,
    current_batch: Arc<RwLock<DlqBatch>>,
}

/// A batch of DLQ records waiting to be written
struct DlqBatch {
    records: Vec<DlqRecord>,
    current_size_bytes: usize,
    last_write_time: DateTime<Utc>,
}

impl DlqWriter {
    /// Create a new DLQ writer with the given configuration
    pub async fn new(config: DlqConfig) -> Result<Self, PipelineError> {
        let store = create_object_store(&config.storage_uri).await?;
        
        info!(
            storage_uri = %config.storage_uri,
            "DLQ writer initialized"
        );

        Ok(Self {
            config,
            store: Arc::new(store),
            current_batch: Arc::new(RwLock::new(DlqBatch {
                records: Vec::new(),
                current_size_bytes: 0,
                last_write_time: Utc::now(),
            })),
        })
    }

    /// Write a single failed record to the DLQ
    pub async fn write_record(&self, record: DlqRecord) -> Result<(), PipelineError> {
        let mut batch = self.current_batch.write().await;
        
        batch.records.push(record.clone());
        batch.current_size_bytes += estimate_record_size(&record);

        // Check if we should flush the batch
        let should_flush = batch.records.len() >= self.config.max_records_per_file
            || batch.current_size_bytes >= self.config.max_file_size_bytes;

        if should_flush {
            debug!(
                records_count = batch.records.len(),
                size_bytes = batch.current_size_bytes,
                "Flushing DLQ batch"
            );
            
            // Take the records to flush
            let records_to_flush = std::mem::take(&mut batch.records);
            batch.current_size_bytes = 0;
            batch.last_write_time = Utc::now();
            
            // Release the lock before writing to storage
            drop(batch);
            
            // Write the batch to storage
            self.write_batch_to_storage(records_to_flush).await?;
        }

        Ok(())
    }

    /// Write multiple failed records to the DLQ
    pub async fn write_records(&self, records: Vec<DlqRecord>) -> Result<(), PipelineError> {
        for record in records {
            self.write_record(record).await?;
        }
        Ok(())
    }

    /// Force flush any pending records to storage
    pub async fn flush(&self) -> Result<(), PipelineError> {
        let mut batch = self.current_batch.write().await;
        
        if !batch.records.is_empty() {
            let records_to_flush = std::mem::take(&mut batch.records);
            batch.current_size_bytes = 0;
            batch.last_write_time = Utc::now();
            
            drop(batch);
            
            self.write_batch_to_storage(records_to_flush).await?;
        }

        Ok(())
    }

    /// Write a batch of records to object storage
    async fn write_batch_to_storage(&self, records: Vec<DlqRecord>) -> Result<(), PipelineError> {
        if records.is_empty() {
            return Ok(());
        }

        // Generate file path with date partitioning
        let first_record = &records[0];
        let date = first_record.timestamp.format("%Y/%m/%d").to_string();
        let pipeline_id = &first_record.pipeline_id;
        let error_category = first_record.error_category.as_str();
        let file_id = Uuid::new_v4().to_string();

        let object_path = match &self.config.prefix {
            Some(prefix) => Path::from(format!(
                "{}/{}/{}/{}/{}.ndjson",
                prefix, date, pipeline_id, error_category, file_id
            )),
            None => Path::from(format!(
                "{}/{}/{}/{}.ndjson",
                date, pipeline_id, error_category, file_id
            )),
        };

        // Convert records to NDJSON format
        let mut json_lines = Vec::new();
        for record in records {
            let json_value = json!({
                "id": record.id,
                "pipeline_id": record.pipeline_id,
                "timestamp": record.timestamp.to_rfc3339(),
                "error_category": record.error_category.as_str(),
                "error_message": record.error_message,
                "original_data": record.original_data,
                "metadata": record.metadata,
            });
            json_lines.push(json_value.to_string());
        }

        let ndjson_content = json_lines.join("\n") + "\n";
        let payload = PutPayload::from_bytes(ndjson_content.as_bytes());

        // Write to object storage
        self.store.put(&object_path, payload).await
            .map_err(|e| PipelineError::SinkWriteFailed {
                connector: "dlq".to_string(),
                retries: 0,
                message: format!("Failed to write DLQ batch to {}: {}", object_path, e),
            })?;

        info!(
            object_path = %object_path,
            records_count = json_lines.len(),
            size_bytes = ndjson_content.len(),
            "DLQ batch written to storage"
        );

        Ok(())
    }

    /// Get statistics about the current DLQ state
    pub async fn stats(&self) -> DlqStats {
        let batch = self.current_batch.read().await;
        DlqStats {
            pending_records: batch.records.len(),
            pending_size_bytes: batch.current_size_bytes,
            last_write_time: batch.last_write_time,
        }
    }
}

/// Statistics about the current DLQ state
#[derive(Debug, Clone)]
pub struct DlqStats {
    pub pending_records: usize,
    pub pending_size_bytes: usize,
    pub last_write_time: DateTime<Utc>,
}

/// Create an object store client from the given URI
async fn create_object_store(uri: &str) -> Result<Box<dyn ObjectStore>, PipelineError> {
    if uri.starts_with("s3://") {
        // Parse S3 URI
        let path = Path::from(uri.strip_prefix("s3://").unwrap());
        let bucket_name = path.parts().first()
            .ok_or_else(|| PipelineError::ConfigError {
                message: "Invalid S3 URI: missing bucket name".to_string(),
            })?;

        // Create S3 client
        let s3_config = object_store::aws::AmazonS3Builder::from_env()
            .with_bucket_name(bucket_name)
            .build()
            .map_err(|e| PipelineError::ConnectionFailed {
                connector: "s3".to_string(),
                message: format!("Failed to create S3 client: {}", e),
            })?;

        Ok(Box::new(s3_config))
    } else if uri.starts_with("gs://") {
        // Parse GCS URI
        let path = Path::from(uri.strip_prefix("gs://").unwrap());
        let bucket_name = path.parts().first()
            .ok_or_else(|| PipelineError::ConfigError {
                message: "Invalid GCS URI: missing bucket name".to_string(),
            })?;

        // Create GCS client
        let gcs_config = object_store::gcp::GoogleCloudStorageBuilder::from_env()
            .with_bucket_name(bucket_name)
            .build()
            .map_err(|e| PipelineError::ConnectionFailed {
                connector: "gcs".to_string(),
                message: format!("Failed to create GCS client: {}", e),
            })?;

        Ok(Box::new(gcs_config))
    } else {
        Err(PipelineError::ConfigError {
            message: format!("Unsupported storage URI scheme: {}", uri),
        })
    }
}

/// Estimate the size of a DLQ record in bytes
fn estimate_record_size(record: &DlqRecord) -> usize {
    let json_value = json!({
        "id": record.id,
        "pipeline_id": record.pipeline_id,
        "timestamp": record.timestamp.to_rfc3339(),
        "error_category": record.error_category.as_str(),
        "error_message": record.error_message,
        "original_data": record.original_data,
        "metadata": record.metadata,
    });
    json_value.to_string().len()
}

/// Initialize the global DLQ writer with default configuration
pub fn init() -> Result<DlqWriter, PipelineError> {
    // This is a simplified initialization - in practice, this would be async
    // and would read configuration from environment or config files
    let config = DlqConfig::default();
    
    // Note: This is a placeholder - actual initialization should be async
    // and would typically be called from the main application
    error!("DLQ init() called synchronously - use DlqWriter::new() for async initialization");
    
    Err(PipelineError::Internal("DLQ initialization must be async".to_string()))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_dlq_record_size_estimation() {
        let record = DlqRecord {
            id: "test-id".to_string(),
            pipeline_id: "test-pipeline".to_string(),
            timestamp: Utc::now(),
            error_category: DlqErrorCategory::SchemaValidation,
            error_message: "Test error".to_string(),
            original_data: json!({"field": "value"}),
            metadata: HashMap::new(),
        };

        let size = estimate_record_size(&record);
        assert!(size > 0);
    }

    #[test]
    fn test_dlq_error_category() {
        assert_eq!(DlqErrorCategory::SchemaValidation.as_str(), "schema_validation");
        assert_eq!(DlqErrorCategory::Other.as_str(), "other");
    }
}
