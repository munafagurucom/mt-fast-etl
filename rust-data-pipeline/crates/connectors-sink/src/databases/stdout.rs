//! Stdout Sink — a simple sink connector for development and testing.
//! Writes each RecordBatch row as a JSON line to stdout.

use std::time::Instant;
use arrow::record_batch::RecordBatch;
use arrow::json::LineDelimitedWriter as LineWriter;
use async_trait::async_trait;

use pipeline_core::error::PipelineError;
use pipeline_core::traits::sink::SinkConnector;
use pipeline_core::types::write_receipt::WriteReceipt;

/// Writes Arrow RecordBatches as JSON to stdout — useful for local dev and debugging.
pub struct StdoutSink {
    total_rows: u64,
}

impl StdoutSink {
    pub fn new() -> Self {
        Self { total_rows: 0 }
    }
}

#[async_trait]
impl SinkConnector for StdoutSink {
    fn name(&self) -> &str {
        "stdout"
    }

    async fn connect(&mut self) -> Result<(), PipelineError> {
        tracing::info!("StdoutSink connected — writing JSON to stdout");
        Ok(())
    }

    async fn write_batch(&mut self, batch: &RecordBatch) -> Result<WriteReceipt, PipelineError> {
        let start = Instant::now();
        let num_rows = batch.num_rows() as u64;

        // Serialize RecordBatch to NDJSON (newline-delimited JSON)
        let mut buf = Vec::new();
        let mut writer = LineWriter::new(&mut buf);
        writer.write(batch).map_err(|e: arrow::error::ArrowError| PipelineError::SinkWriteFailed {
            connector: "stdout".to_string(),
            retries: 0,
            message: e.to_string(),
        })?;
        writer.finish().map_err(|e: arrow::error::ArrowError| PipelineError::SinkWriteFailed {
            connector: "stdout".to_string(),
            retries: 0,
            message: e.to_string(),
        })?;

        let output = String::from_utf8_lossy(&buf);
        let bytes_written = buf.len() as u64;
        print!("{}", output);

        self.total_rows += num_rows;

        Ok(WriteReceipt::new(num_rows, bytes_written, start.elapsed()))
    }

    async fn flush(&mut self) -> Result<(), PipelineError> {
        // stdout is unbuffered by default — no-op
        Ok(())
    }

    async fn disconnect(&mut self) -> Result<(), PipelineError> {
        tracing::info!(total_rows = self.total_rows, "StdoutSink disconnected");
        Ok(())
    }
}
