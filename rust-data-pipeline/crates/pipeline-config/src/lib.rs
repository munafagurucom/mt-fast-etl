pub mod loader;
pub mod secrets;

use pipeline_core::types::pipeline_config::PipelineConfig;
use pipeline_core::error::PipelineError;

/// Load pipeline configurations from a YAML file path.
pub fn load_from_yaml(yaml_content: &str) -> Result<Vec<PipelineConfig>, PipelineError> {
    let configs: Vec<PipelineConfig> = serde_yaml::from_str(yaml_content)
        .map_err(|e| PipelineError::ConfigError {
            message: format!("Failed to parse YAML config: {}", e),
        })?;
    Ok(configs)
}

/// Load a single pipeline config from JSON.
pub fn load_from_json(json_content: &str) -> Result<PipelineConfig, PipelineError> {
    let config: PipelineConfig = serde_json::from_str(json_content)
        .map_err(|e| PipelineError::ConfigError {
            message: format!("Failed to parse JSON config: {}", e),
        })?;
    Ok(config)
}
