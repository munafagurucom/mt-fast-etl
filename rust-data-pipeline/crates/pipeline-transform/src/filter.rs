//! Filter — drops rows from a RecordBatch based on predicate conditions.

use arrow::record_batch::RecordBatch;
use arrow::compute::filter_record_batch;
use arrow::array::BooleanArray;
use pipeline_core::error::PipelineError;
use pipeline_core::traits::transformer::Transformer;

/// Filters rows using a boolean predicate column or expression.
pub struct RowFilter {
    /// Name of a boolean column to use as a filter mask.
    /// Rows where this column is `false` are dropped.
    filter_column: String,
}

impl RowFilter {
    pub fn new(filter_column: impl Into<String>) -> Self {
        Self {
            filter_column: filter_column.into(),
        }
    }
}

impl Transformer for RowFilter {
    fn name(&self) -> &str {
        "row_filter"
    }

    fn transform(&self, batch: RecordBatch) -> Result<RecordBatch, PipelineError> {
        let schema = batch.schema();
        let idx = schema.index_of(&self.filter_column).map_err(|_| {
            PipelineError::TransformFailed {
                transform: "row_filter".to_string(),
                message: format!("Filter column '{}' not found in schema", self.filter_column),
            }
        })?;

        let mask = batch
            .column(idx)
            .as_any()
            .downcast_ref::<BooleanArray>()
            .ok_or_else(|| PipelineError::TransformFailed {
                transform: "row_filter".to_string(),
                message: format!("Column '{}' is not a boolean type", self.filter_column),
            })?;

        filter_record_batch(&batch, mask).map_err(|e| PipelineError::TransformFailed {
            transform: "row_filter".to_string(),
            message: e.to_string(),
        })
    }
}
