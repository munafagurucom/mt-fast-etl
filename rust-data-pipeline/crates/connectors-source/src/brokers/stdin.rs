//! Stdin Source — a simple source connector for development and testing.
//! Reads JSON lines from stdin and converts them to Arrow RecordBatches.

use std::sync::Arc;
use arrow::array::StringArray;
use arrow::datatypes::{Schema, Field, DataType};
use arrow::record_batch::RecordBatch;
use async_trait::async_trait;
use tokio::io::{AsyncBufReadExt, BufReader};

use pipeline_core::error::PipelineError;
use pipeline_core::traits::source::SourceConnector;
use pipeline_core::types::checkpoint::Checkpoint;

/// Reads JSON lines from stdin — useful for local testing and piping.
pub struct StdinSource {
    reader: Option<BufReader<tokio::io::Stdin>>,
    batch_size: usize,
}

impl StdinSource {
    pub fn new() -> Self {
        Self {
            reader: None,
            batch_size: 100,
        }
    }
}

#[async_trait]
impl SourceConnector for StdinSource {
    fn name(&self) -> &str {
        "stdin"
    }

    async fn connect(&mut self) -> Result<(), PipelineError> {
        self.reader = Some(BufReader::new(tokio::io::stdin()));
        tracing::info!("StdinSource connected — reading JSON lines from stdin");
        Ok(())
    }

    async fn poll_batch(&mut self) -> Result<RecordBatch, PipelineError> {
        let reader = self.reader.as_mut().ok_or_else(|| PipelineError::ConnectionFailed {
            connector: "stdin".to_string(),
            message: "Not connected".to_string(),
        })?;

        let mut lines: Vec<String> = Vec::new();

        for _ in 0..self.batch_size {
            let mut line = String::new();
            match reader.read_line(&mut line).await {
                Ok(0) => break, // EOF
                Ok(_) => {
                    let trimmed = line.trim().to_string();
                    if !trimmed.is_empty() {
                        lines.push(trimmed);
                    }
                }
                Err(e) => {
                    return Err(PipelineError::Internal(format!("Stdin read error: {}", e)));
                }
            }
        }

        if lines.is_empty() {
            // Return empty batch — orchestrator will back off
            let schema = Arc::new(Schema::new(vec![
                Field::new("raw_json", DataType::Utf8, false),
            ]));
            return Ok(RecordBatch::new_empty(schema));
        }

        let schema = Arc::new(Schema::new(vec![
            Field::new("raw_json", DataType::Utf8, false),
        ]));

        let json_array = StringArray::from(lines);
        RecordBatch::try_new(schema, vec![Arc::new(json_array)])
            .map_err(|e| PipelineError::Internal(e.to_string()))
    }

    async fn acknowledge(&mut self, _checkpoint: &Checkpoint) -> Result<(), PipelineError> {
        // Stdin is not resumable — no-op
        Ok(())
    }

    async fn disconnect(&mut self) -> Result<(), PipelineError> {
        self.reader = None;
        tracing::info!("StdinSource disconnected");
        Ok(())
    }
}
