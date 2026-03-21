//! HTTP Control Plane API — REST endpoints for dynamic pipeline management.
//! Provides real-time pipeline orchestration via HTTP API using axum.

use std::collections::HashMap;
use std::sync::Arc;

use axum::{
    extract::{Path, Query, State},
    http::StatusCode,
    response::Json,
    routing::{get, post, delete},
    Router,
};
use serde::{Deserialize, Serialize};
use tokio::sync::RwLock;
use tracing::{info, warn, error, debug};

use crate::engine::{Engine, PipelineStatus};
use pipeline_core::types::pipeline_config::{PipelineConfig, ConnectorConfig};
use pipeline_core::error::PipelineError;

/// HTTP API state shared across all endpoints
#[derive(Clone)]
pub struct ApiState {
    engine: Arc<Engine>,
}

/// API response wrapper
#[derive(Debug, Serialize)]
pub struct ApiResponse<T> {
    pub success: bool,
    pub data: Option<T>,
    pub error: Option<String>,
}

impl<T> ApiResponse<T> {
    pub fn success(data: T) -> Self {
        Self {
            success: true,
            data: Some(data),
            error: None,
        }
    }

    pub fn error(message: String) -> Self {
        Self {
            success: false,
            data: None,
            error: Some(message),
        }
    }
}

/// Pipeline status response
#[derive(Debug, Serialize)]
pub struct PipelineStatusResponse {
    pub pipeline_id: String,
    pub status: String,
    pub uptime_seconds: Option<u64>,
    pub records_processed: Option<u64>,
    pub last_error: Option<String>,
}

/// Create pipeline request
#[derive(Debug, Deserialize)]
pub struct CreatePipelineRequest {
    pub pipeline_id: String,
    pub source: ConnectorConfig,
    pub sink: ConnectorConfig,
    pub transforms: Vec<pipeline_core::types::pipeline_config::TransformConfig>,
    pub rate_limits: Option<pipeline_core::types::pipeline_config::RateLimitConfig>,
    pub flags: Option<pipeline_core::types::pipeline_config::FeatureFlags>,
}

/// List pipelines query parameters
#[derive(Debug, Deserialize)]
pub struct ListPipelinesQuery {
    pub status: Option<String>,
    pub limit: Option<usize>,
    pub offset: Option<usize>,
}

/// Create the HTTP control plane router
pub fn create_router(engine: Arc<Engine>) -> Router {
    let state = ApiState { engine };

    Router::new()
        // Pipeline management endpoints
        .route("/api/v1/pipelines", get(list_pipelines))
        .route("/api/v1/pipelines", post(create_pipeline))
        .route("/api/v1/pipelines/:id", get(get_pipeline))
        .route("/api/v1/pipelines/:id", delete(stop_pipeline))
        
        // Health and metrics endpoints
        .route("/api/v1/health", get(health_check))
        .route("/api/v1/metrics", get(get_metrics))
        
        // Configuration endpoints
        .route("/api/v1/config/schemas", get(get_config_schemas))
        
        .with_state(state)
}

/// List all pipelines with optional filtering
async fn list_pipelines(
    State(state): State<ApiState>,
    Query(query): Query<ListPipelinesQuery>,
) -> Result<Json<ApiResponse<Vec<PipelineStatusResponse>>>, StatusCode> {
    debug!("Listing pipelines with query: {:?}", query);

    let pipelines = state.engine.list_pipelines().await;
    let mut filtered_pipelines: Vec<PipelineStatusResponse> = Vec::new();

    for (id, status) in pipelines {
        // Apply status filter if provided
        if let Some(filter_status) = &query.status {
            let status_str = match status {
                PipelineStatus::Starting => "starting",
                PipelineStatus::Running => "running",
                PipelineStatus::Paused => "paused",
                PipelineStatus::Stopped => "stopped",
                PipelineStatus::Failed(_) => "failed",
            };
            if status_str != filter_status {
                continue;
            }
        }

        let status_response = PipelineStatusResponse {
            pipeline_id: id.clone(),
            status: match status {
                PipelineStatus::Starting => "starting".to_string(),
                PipelineStatus::Running => "running".to_string(),
                PipelineStatus::Paused => "paused".to_string(),
                PipelineStatus::Stopped => "stopped".to_string(),
                PipelineStatus::Failed(msg) => format!("failed: {}", msg),
            },
            uptime_seconds: None, // TODO: Track uptime in engine
            records_processed: None, // TODO: Track records in engine
            last_error: match status {
                PipelineStatus::Failed(msg) => Some(msg),
                _ => None,
            },
        };

        filtered_pipelines.push(status_response);
    }

    // Apply pagination
    let limit = query.limit.unwrap_or(100);
    let offset = query.offset.unwrap_or(0);
    let paginated_pipelines: Vec<PipelineStatusResponse> = filtered_pipelines
        .into_iter()
        .skip(offset)
        .take(limit)
        .collect();

    Ok(Json(ApiResponse::success(paginated_pipelines)))
}

/// Get details for a specific pipeline
async fn get_pipeline(
    State(state): State<ApiState>,
    Path(pipeline_id): Path<String>,
) -> Result<Json<ApiResponse<PipelineStatusResponse>>, StatusCode> {
    debug!("Getting pipeline: {}", pipeline_id);

    let pipelines = state.engine.list_pipelines().await;
    
    for (id, status) in pipelines {
        if id == pipeline_id {
            let status_response = PipelineStatusResponse {
                pipeline_id: id,
                status: match status {
                    PipelineStatus::Starting => "starting".to_string(),
                    PipelineStatus::Running => "running".to_string(),
                    PipelineStatus::Paused => "paused".to_string(),
                    PipelineStatus::Stopped => "stopped".to_string(),
                    PipelineStatus::Failed(msg) => format!("failed: {}", msg),
                },
                uptime_seconds: None,
                records_processed: None,
                last_error: match status {
                    PipelineStatus::Failed(msg) => Some(msg),
                    _ => None,
                },
            };
            return Ok(Json(ApiResponse::success(status_response)));
        }
    }

    Ok(Json(ApiResponse::error(format!("Pipeline '{}' not found", pipeline_id))))
}

/// Create and start a new pipeline
async fn create_pipeline(
    State(state): State<ApiState>,
    Json(request): Json<CreatePipelineRequest>,
) -> Result<Json<ApiResponse<PipelineStatusResponse>>, StatusCode> {
    info!("Creating pipeline: {}", request.pipeline_id);

    // Validate pipeline ID uniqueness
    let existing_pipelines = state.engine.list_pipelines().await;
    if existing_pipelines.iter().any(|(id, _)| id == &request.pipeline_id) {
        return Ok(Json(ApiResponse::error(format!(
            "Pipeline with ID '{}' already exists", request.pipeline_id
        ))));
    }

    // Create pipeline configuration
    let pipeline_config = PipelineConfig {
        pipeline_id: request.pipeline_id.clone(),
        enabled: true,
        source: request.source,
        sink: request.sink,
        field_mappings: Vec::new(),
        transforms: request.transforms,
        schema: None,
        rate_limits: request.rate_limits.unwrap_or_default(),
        flags: request.flags.unwrap_or_default(),
    };

    // Create source and sink connectors
    let source = connectors_source::create_source(&pipeline_config.source)
        .map_err(|e| {
            error!("Failed to create source connector: {}", e);
            StatusCode::BAD_REQUEST
        })?;

    let sink = connectors_sink::create_sink(&pipeline_config.sink)
        .map_err(|e| {
            error!("Failed to create sink connector: {}", e);
            StatusCode::BAD_REQUEST
        })?;

    // Spawn the pipeline
    state.engine.spawn_pipeline(pipeline_config.clone(), source, sink, Vec::new())
        .await
        .map_err(|e| {
            error!("Failed to spawn pipeline: {}", e);
            StatusCode::INTERNAL_SERVER_ERROR
        })?;

    let status_response = PipelineStatusResponse {
        pipeline_id: request.pipeline_id,
        status: "starting".to_string(),
        uptime_seconds: None,
        records_processed: None,
        last_error: None,
    };

    info!("Pipeline '{}' created successfully", request.pipeline_id);
    Ok(Json(ApiResponse::success(status_response)))
}

/// Stop a running pipeline
async fn stop_pipeline(
    State(state): State<ApiState>,
    Path(pipeline_id): Path<String>,
) -> Result<Json<ApiResponse<String>>, StatusCode> {
    info!("Stopping pipeline: {}", pipeline_id);

    state.engine.stop_pipeline(&pipeline_id)
        .await
        .map_err(|e| {
            error!("Failed to stop pipeline: {}", e);
            StatusCode::NOT_FOUND
        })?;

    info!("Pipeline '{}' stopped successfully", pipeline_id);
    Ok(Json(ApiResponse::success(format!("Pipeline '{}' stopped", pipeline_id))))
}

/// Health check endpoint
async fn health_check() -> Json<ApiResponse<HashMap<String, String>>> {
    let mut health_data = HashMap::new();
    health_data.insert("status".to_string(), "healthy".to_string());
    health_data.insert("timestamp".to_string(), chrono::Utc::now().to_rfc3339());
    health_data.insert("version".to_string(), "0.1.0".to_string());

    Json(ApiResponse::success(health_data))
}

/// Get basic metrics
async fn get_metrics(State(state): State<ApiState>) -> Json<ApiResponse<HashMap<String, serde_json::Value>>> {
    let pipelines = state.engine.list_pipelines().await;
    let mut metrics = HashMap::new();

    // Count pipelines by status
    let mut status_counts = HashMap::new();
    for (_, status) in pipelines {
        let status_str = match status {
            PipelineStatus::Starting => "starting",
            PipelineStatus::Running => "running",
            PipelineStatus::Paused => "paused",
            PipelineStatus::Stopped => "stopped",
            PipelineStatus::Failed(_) => "failed",
        };
        *status_counts.entry(status_str.to_string()).or_insert(0) += 1;
    }

    metrics.insert("total_pipelines".to_string(), serde_json::Value::Number(pipelines.len().into()));
    metrics.insert("status_counts".to_string(), serde_json::to_value(status_counts).unwrap());
    metrics.insert("timestamp".to_string(), serde_json::Value::String(chrono::Utc::now().to_rfc3339()));

    Json(ApiResponse::success(metrics))
}

/// Get configuration schemas for validation
async fn get_config_schemas() -> Json<ApiResponse<HashMap<String, serde_json::Value>>> {
    let mut schemas = HashMap::new();

    // Source connector schemas
    let mut source_schema = serde_json::json!({
        "type": "object",
        "properties": {
            "type": {
                "type": "string",
                "enum": ["stdin", "kafka", "kinesis", "pulsar", "nats", "sqs", "postgres_cdc", "mysql_cdc", "mongodb_cdc", "salesforce", "hubspot"]
            },
            "properties": {
                "type": "object",
                "additionalProperties": true
            },
            "secrets": {
                "type": "object",
                "additionalProperties": true
            }
        },
        "required": ["type"]
    });

    // Sink connector schemas
    let mut sink_schema = serde_json::json!({
        "type": "object",
        "properties": {
            "type": {
                "type": "string",
                "enum": ["stdout", "postgres", "mysql", "mongodb", "redis", "elasticsearch", "snowflake", "bigquery", "databricks", "delta_lake", "iceberg", "s3", "gcs"]
            },
            "properties": {
                "type": "object",
                "additionalProperties": true
            },
            "secrets": {
                "type": "object",
                "additionalProperties": true
            }
        },
        "required": ["type"]
    });

    schemas.insert("source".to_string(), source_schema);
    schemas.insert("sink".to_string(), sink_schema);

    Json(ApiResponse::success(schemas))
}

/// Start the HTTP control plane server
pub async fn start_server(engine: Arc<Engine>, port: u16) -> Result<(), Box<dyn std::error::Error + Send + Sync>> {
    let app = create_router(engine);

    let addr = format!("0.0.0.0:{}", port);
    info!("Starting control plane API on {}", addr);

    let listener = tokio::net::TcpListener::bind(addr).await?;
    axum::serve(listener, app).await?;

    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
    use axum::http::StatusCode;
    use tower::ServiceExt;

    #[tokio::test]
    async fn test_health_check() {
        let response = health_check().await;
        let api_response = response.0;
        
        assert!(api_response.success);
        assert!(api_response.data.is_some());
        
        let health_data = api_response.data.unwrap();
        assert_eq!(health_data.get("status").unwrap(), "healthy");
    }

    #[tokio::test]
    async fn test_api_response_creation() {
        let success_response = ApiResponse::success("test data");
        assert!(success_response.success);
        assert!(success_response.data.is_some());
        assert!(success_response.error.is_none());

        let error_response = ApiResponse::error("test error".to_string());
        assert!(!error_response.success);
        assert!(error_response.data.is_none());
        assert!(error_response.error.is_some());
    }
}
