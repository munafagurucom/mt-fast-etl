//! Configuration loader — loads pipeline definitions from object storage or local disk.

use object_store::ObjectStore;
use object_store::path::Path;
use pipeline_core::types::pipeline_config::PipelineConfig;
use pipeline_core::error::PipelineError;
use tracing::{info, warn};

/// Scans an object storage prefix for YAML/JSON pipeline definitions.
/// Called on engine startup and periodically (every 60s) to detect new pipelines.
pub async fn load_configs_from_object_store(
    store: &dyn ObjectStore,
    prefix: &str,
) -> Result<Vec<PipelineConfig>, PipelineError> {
    let prefix_path = Path::from(prefix);
    let mut configs = Vec::new();

    let list_result = store
        .list(Some(&prefix_path))
        .try_collect::<Vec<_>>()
        .await
        .map_err(|e| PipelineError::ConfigError {
            message: format!("Failed to list config objects at '{}': {}", prefix, e),
        })?;

    for meta in list_result {
        let path = meta.location.clone();
        let path_str = path.to_string();

        if !path_str.ends_with(".yaml") && !path_str.ends_with(".json") {
            continue;
        }

        match store.get(&path).await {
            Ok(get_result) => {
                let bytes = get_result.bytes().await.map_err(|e| PipelineError::ConfigError {
                    message: format!("Failed to read config '{}': {}", path_str, e),
                })?;
                let content = String::from_utf8_lossy(&bytes);

                if path_str.ends_with(".yaml") {
                    match serde_yaml::from_str::<PipelineConfig>(&content) {
                        Ok(config) => {
                            info!(pipeline_id = %config.pipeline_id, "Loaded pipeline config from {}", path_str);
                            configs.push(config);
                        }
                        Err(e) => warn!("Failed to parse YAML config '{}': {}", path_str, e),
                    }
                } else {
                    match serde_json::from_str::<PipelineConfig>(&content) {
                        Ok(config) => {
                            info!(pipeline_id = %config.pipeline_id, "Loaded pipeline config from {}", path_str);
                            configs.push(config);
                        }
                        Err(e) => warn!("Failed to parse JSON config '{}': {}", path_str, e),
                    }
                }
            }
            Err(e) => warn!("Failed to fetch config '{}': {}", path_str, e),
        }
    }

    info!("Loaded {} pipeline configurations from '{}'", configs.len(), prefix);
    Ok(configs)
}

use futures::TryStreamExt;
