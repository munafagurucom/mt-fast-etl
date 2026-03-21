//! Transform Chain — an ordered sequence of transformers applied to each batch.

use arrow::record_batch::RecordBatch;
use pipeline_core::error::PipelineError;
use pipeline_core::traits::transformer::Transformer;

/// Applies an ordered sequence of transformers to each RecordBatch.
/// This is the Pipes and Filters pattern.
pub struct TransformChain {
    transformers: Vec<Box<dyn Transformer>>,
}

impl TransformChain {
    pub fn new(transformers: Vec<Box<dyn Transformer>>) -> Self {
        Self { transformers }
    }

    pub fn empty() -> Self {
        Self {
            transformers: Vec::new(),
        }
    }

    /// Apply all transformers in order.
    pub fn apply(&self, batch: RecordBatch) -> Result<RecordBatch, PipelineError> {
        let mut current = batch;
        for transformer in &self.transformers {
            current = transformer.transform(current)?;
        }
        Ok(current)
    }
}
