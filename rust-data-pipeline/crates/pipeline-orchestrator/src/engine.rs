//! Pipeline Engine — the central orchestrator that manages all running pipelines.

use std::collections::HashMap;
use std::sync::Arc;
use tokio::sync::RwLock;
use tracing::{info, error};

use pipeline_core::traits::source::SourceConnector;
use pipeline_core::traits::sink::SinkConnector;
use pipeline_core::traits::transformer::Transformer;
use pipeline_core::types::pipeline_config::PipelineConfig;

use crate::worker::PipelineWorker;

/// Tracks the runtime state of a spawned pipeline.
pub struct PipelineHandle {
    pub config: PipelineConfig,
    pub status: PipelineStatus,
    pub cancel_token: tokio::sync::watch::Sender<bool>,
}

/// Current status of a pipeline.
#[derive(Debug, Clone, PartialEq, Eq, serde::Serialize)]
pub enum PipelineStatus {
    Starting,
    Running,
    Paused,
    Stopped,
    Failed(String),
}

/// The main engine that manages pipeline lifecycle.
pub struct Engine {
    /// Active pipelines keyed by pipeline_id.
    pipelines: Arc<RwLock<HashMap<String, PipelineHandle>>>,
}

impl Engine {
    pub fn new() -> Self {
        Self {
            pipelines: Arc::new(RwLock::new(HashMap::new())),
        }
    }

    /// Spawn a new pipeline from the given config, source, sink, and transformer chain.
    pub async fn spawn_pipeline(
        &self,
        config: PipelineConfig,
        source: Box<dyn SourceConnector>,
        sink: Box<dyn SinkConnector>,
        transforms: Vec<Box<dyn Transformer>>,
    ) -> Result<(), pipeline_core::error::PipelineError> {
        let pipeline_id = config.pipeline_id.clone();
        info!(pipeline_id = %pipeline_id, "Spawning pipeline");

        // Create a cancellation channel for graceful shutdown
        let (cancel_tx, cancel_rx) = tokio::sync::watch::channel(false);

        let handle = PipelineHandle {
            config: config.clone(),
            status: PipelineStatus::Starting,
            cancel_token: cancel_tx,
        };

        // Register the pipeline
        {
            let mut pipelines = self.pipelines.write().await;
            pipelines.insert(pipeline_id.clone(), handle);
        }

        // Spawn the worker as a detached tokio task
        let pipelines_ref = self.pipelines.clone();
        let worker = PipelineWorker::new(
            config,
            source,
            sink,
            transforms,
            cancel_rx,
        );

        tokio::spawn(async move {
            // Mark as running
            {
                let mut pipelines = pipelines_ref.write().await;
                if let Some(h) = pipelines.get_mut(&pipeline_id) {
                    h.status = PipelineStatus::Running;
                }
            }

            // Run the main loop
            match worker.run().await {
                Ok(()) => {
                    info!(pipeline_id = %pipeline_id, "Pipeline completed successfully");
                    let mut pipelines = pipelines_ref.write().await;
                    if let Some(h) = pipelines.get_mut(&pipeline_id) {
                        h.status = PipelineStatus::Stopped;
                    }
                }
                Err(e) => {
                    error!(pipeline_id = %pipeline_id, error = %e, "Pipeline failed");
                    let mut pipelines = pipelines_ref.write().await;
                    if let Some(h) = pipelines.get_mut(&pipeline_id) {
                        h.status = PipelineStatus::Failed(e.to_string());
                    }
                }
            }
        });

        Ok(())
    }

    /// List all pipelines and their current status.
    pub async fn list_pipelines(&self) -> Vec<(String, PipelineStatus)> {
        let pipelines = self.pipelines.read().await;
        pipelines
            .iter()
            .map(|(id, handle)| (id.clone(), handle.status.clone()))
            .collect()
    }

    /// Stop a pipeline by its ID.
    pub async fn stop_pipeline(&self, pipeline_id: &str) -> Result<(), pipeline_core::error::PipelineError> {
        let pipelines = self.pipelines.read().await;
        if let Some(handle) = pipelines.get(pipeline_id) {
            let _ = handle.cancel_token.send(true);
            info!(pipeline_id = %pipeline_id, "Sent stop signal to pipeline");
            Ok(())
        } else {
            Err(pipeline_core::error::PipelineError::ConfigError {
                message: format!("Pipeline '{}' not found", pipeline_id),
            })
        }
    }
}

impl Default for Engine {
    fn default() -> Self {
        Self::new()
    }
}
