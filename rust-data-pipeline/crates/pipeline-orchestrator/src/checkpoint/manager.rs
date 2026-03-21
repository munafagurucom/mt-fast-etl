//! Checkpoint Manager — persists and loads pipeline checkpoints from object storage.

use pipeline_core::types::checkpoint::Checkpoint;
use pipeline_core::error::PipelineError;
use tracing::info;

/// Manages checkpoint persistence to object storage (S3/GCS/Azure Blob).
pub struct CheckpointManager {
    /// Object storage path prefix for checkpoints.
    prefix: String,
}

impl CheckpointManager {
    pub fn new(prefix: impl Into<String>) -> Self {
        Self {
            prefix: prefix.into(),
        }
    }

    /// Persist a checkpoint to object storage.
    pub async fn save(&self, checkpoint: &Checkpoint) -> Result<(), PipelineError> {
        let json = serde_json::to_string_pretty(checkpoint)?;
        let path = format!("{}/{}/cursor.json", self.prefix, checkpoint.pipeline_id);
        info!(pipeline_id = %checkpoint.pipeline_id, path = %path, "Saving checkpoint");

        // TODO: Write to object_store backend
        // For now, log the checkpoint
        info!(
            pipeline_id = %checkpoint.pipeline_id,
            offset = %checkpoint.offset,
            "Checkpoint saved (stub)"
        );
        let _ = json;
        Ok(())
    }

    /// Load a checkpoint from object storage for pipeline resumption.
    pub async fn load(&self, pipeline_id: &str) -> Result<Option<Checkpoint>, PipelineError> {
        let _path = format!("{}/{}/cursor.json", self.prefix, pipeline_id);
        info!(pipeline_id = %pipeline_id, "Loading checkpoint (stub)");

        // TODO: Read from object_store backend
        // Return None if no checkpoint exists (fresh start)
        Ok(None)
    }
}
