//! Transformer trait — the interface for all data transformation steps.

use arrow::record_batch::RecordBatch;

use crate::error::PipelineError;

/// Transformers operate on Arrow RecordBatches in-place.
/// Multiple transformers are chained in a TransformChain (Pipes and Filters pattern).
pub trait Transformer: Send + Sync {
    /// Human-readable name of this transformation (e.g., "field_mapper", "pii_masker").
    fn name(&self) -> &str;

    /// Apply the transformation to the incoming RecordBatch and return
    /// a new (potentially reshaped) RecordBatch.
    fn transform(&self, batch: RecordBatch) -> Result<RecordBatch, PipelineError>;
}
