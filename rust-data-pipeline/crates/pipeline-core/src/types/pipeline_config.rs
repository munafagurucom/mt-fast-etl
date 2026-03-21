//! Pipeline configuration types — deserialized from YAML/JSON pipeline definitions.

use serde::{Deserialize, Serialize};
use std::collections::HashMap;

/// Root configuration for a single data pipeline.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct PipelineConfig {
    /// Unique identifier for the pipeline (e.g., "postgres-to-snowflake-orders").
    pub pipeline_id: String,

    /// Whether this pipeline is enabled.
    #[serde(default = "default_true")]
    pub enabled: bool,

    /// Source connector configuration.
    pub source: ConnectorConfig,

    /// Sink connector configuration.
    pub sink: ConnectorConfig,

    /// 1-to-1 field mappings from source schema to sink schema.
    #[serde(default)]
    pub field_mappings: Vec<FieldMapping>,

    /// Transformation chain configuration.
    #[serde(default)]
    pub transforms: Vec<TransformConfig>,

    /// Schema validation settings.
    #[serde(default)]
    pub schema: Option<SchemaConfig>,

    /// Rate limiting settings.
    #[serde(default)]
    pub rate_limits: RateLimitConfig,

    /// Feature flags and overrides.
    #[serde(default)]
    pub flags: FeatureFlags,
}

/// Configuration for a source or sink connector.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ConnectorConfig {
    /// Connector type identifier (e.g., "kafka", "postgres_cdc", "snowflake").
    #[serde(rename = "type")]
    pub connector_type: String,

    /// Arbitrary key-value properties specific to this connector.
    /// Examples: host, port, database, topic, bucket, etc.
    #[serde(default)]
    pub properties: HashMap<String, serde_json::Value>,

    /// Secret references for credentials (URN format).
    /// e.g., "arn:aws:secretsmanager:...", "env:DB_PASSWORD"
    #[serde(default)]
    pub secrets: HashMap<String, String>,
}

/// A single field mapping in the 1-to-1 schema translation.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct FieldMapping {
    /// Field name in the source payload.
    pub source_field: String,

    /// Field name in the destination schema.
    pub sink_field: String,

    /// Optional type cast (e.g., "string", "integer", "float", "timestamp").
    pub cast: Option<String>,
}

/// Configuration for a single transformation step.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct TransformConfig {
    /// Transform type (e.g., "field_mapper", "filter", "pii_mask", "encrypt").
    #[serde(rename = "type")]
    pub transform_type: String,

    /// Transform-specific parameters.
    #[serde(default)]
    pub params: HashMap<String, serde_json::Value>,
}

/// Schema validation settings.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SchemaConfig {
    /// Path to the Arrow/JSON Schema definition.
    pub schema_path: Option<String>,

    /// What to do when schema drift is detected.
    #[serde(default)]
    pub drift_policy: DriftPolicy,

    /// Whether to auto-infer schema from the first batch.
    #[serde(default)]
    pub auto_infer: bool,
}

/// Policy when unexpected schema changes are detected.
#[derive(Debug, Clone, Serialize, Deserialize, Default)]
pub enum DriftPolicy {
    /// Halt the pipeline and raise an alert.
    Halt,
    /// Drop unknown fields silently.
    DropField,
    /// Send malformed records to the DLQ and continue.
    #[default]
    Dlq,
    /// Auto-alter the destination table to accommodate new fields.
    AlterTable,
}

/// Rate limiting configuration (PRD Requirement #13).
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct RateLimitConfig {
    /// Maximum transactions per second from the source.
    #[serde(default = "default_source_tps")]
    pub max_source_tps: u32,

    /// Maximum transactions per second to the sink.
    #[serde(default = "default_sink_tps")]
    pub max_sink_tps: u32,

    /// Maximum concurrent connections to the sink database.
    #[serde(default = "default_max_connections")]
    pub max_concurrent_connections: u32,
}

impl Default for RateLimitConfig {
    fn default() -> Self {
        Self {
            max_source_tps: default_source_tps(),
            max_sink_tps: default_sink_tps(),
            max_concurrent_connections: default_max_connections(),
        }
    }
}

/// Feature flags and runtime overrides (PRD Requirement #12).
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct FeatureFlags {
    /// Enable Dead Letter Queue for failed records.
    #[serde(default = "default_true")]
    pub enable_dlq: bool,

    /// Enable schema validation on incoming records.
    #[serde(default)]
    pub enable_schema_validation: bool,

    /// Require TLS for all connections.
    #[serde(default = "default_true")]
    pub require_tls: bool,

    /// Maximum memory buffer in MB before spilling to disk.
    #[serde(default = "default_buffer_mb")]
    pub max_memory_buffer_mb: u64,

    /// Micro-batch flush interval in milliseconds.
    #[serde(default = "default_flush_interval")]
    pub batch_flush_interval_ms: u64,

    /// Enable data lineage header injection.
    #[serde(default = "default_true")]
    pub enable_lineage: bool,

    /// Global concurrency limit for tokio worker tasks.
    #[serde(default = "default_concurrency")]
    pub concurrency_limit: usize,
}

impl Default for FeatureFlags {
    fn default() -> Self {
        Self {
            enable_dlq: true,
            enable_schema_validation: false,
            require_tls: true,
            max_memory_buffer_mb: 512,
            batch_flush_interval_ms: 5000,
            enable_lineage: true,
            concurrency_limit: 64,
        }
    }
}

fn default_true() -> bool { true }
fn default_source_tps() -> u32 { 10_000 }
fn default_sink_tps() -> u32 { 5_000 }
fn default_max_connections() -> u32 { 10 }
fn default_buffer_mb() -> u64 { 512 }
fn default_flush_interval() -> u64 { 5000 }
fn default_concurrency() -> usize { 64 }
