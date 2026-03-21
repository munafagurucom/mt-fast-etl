//! Field Mapper — renames source fields to sink fields based on the FieldMapping config.

use std::sync::Arc;
use arrow::record_batch::RecordBatch;
use arrow::datatypes::{Schema, Field};
use pipeline_core::error::PipelineError;
use pipeline_core::traits::transformer::Transformer;
use pipeline_core::types::pipeline_config::FieldMapping;

/// Renames columns in a RecordBatch according to the configured field mappings.
pub struct FieldMapper {
    mappings: Vec<FieldMapping>,
}

impl FieldMapper {
    pub fn new(mappings: Vec<FieldMapping>) -> Self {
        Self { mappings }
    }
}

impl Transformer for FieldMapper {
    fn name(&self) -> &str {
        "field_mapper"
    }

    fn transform(&self, batch: RecordBatch) -> Result<RecordBatch, PipelineError> {
        if self.mappings.is_empty() {
            return Ok(batch);
        }

        let schema = batch.schema();
        let mut new_fields: Vec<Field> = Vec::new();
        let mut columns = Vec::new();

        for mapping in &self.mappings {
            if let Ok(idx) = schema.index_of(&mapping.source_field) {
                let original_field = schema.field(idx);
                let new_field = Field::new(
                    &mapping.sink_field,
                    original_field.data_type().clone(),
                    original_field.is_nullable(),
                );
                new_fields.push(new_field);
                columns.push(batch.column(idx).clone());
            }
        }

        let new_schema = Arc::new(Schema::new(new_fields));
        RecordBatch::try_new(new_schema, columns).map_err(|e| PipelineError::TransformFailed {
            transform: "field_mapper".to_string(),
            message: e.to_string(),
        })
    }
}
