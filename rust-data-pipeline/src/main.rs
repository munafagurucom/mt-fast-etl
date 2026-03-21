//! Rust Data Pipeline Engine — Binary Entry Point
//!
//! This is the main executable that:
//! 1. Parses CLI arguments
//! 2. Initializes structured JSON logging
//! 3. Loads pipeline configurations from YAML/JSON
//! 4. Spawns the orchestrator engine
//! 5. Starts the control plane HTTP server (optional)
//! 6. Blocks until CTRL+C for graceful shutdown

use clap::Parser;
use tracing::{info, error};
use std::sync::Arc;

use connectors_source::create_source;
use connectors_sink::create_sink;
use pipeline_orchestrator::engine::Engine;
use pipeline_orchestrator::control_plane::start_server;

#[derive(Parser)]
#[command(name = "rust-data-pipeline")]
#[command(about = "High-performance data pipeline engine built in Rust")]
#[command(version)]
struct Cli {
    /// Path to a pipeline config file (YAML or JSON).
    #[arg(short, long)]
    config: Option<String>,

    /// Run with the stdin→stdout demo pipeline.
    #[arg(long, default_value_t = false)]
    demo: bool,

    /// Port for the control plane HTTP API.
    #[arg(long, default_value_t = 8080)]
    api_port: u16,
}

#[tokio::main]
async fn main() -> anyhow::Result<()> {
    // Initialize structured JSON logging
    pipeline_observe::init_logging();

    let cli = Cli::parse();
    info!("Rust Data Pipeline Engine starting...");

    let engine = Arc::new(Engine::new());

    if cli.demo {
        // Run a simple stdin → stdout demo pipeline
        info!("Starting demo pipeline: stdin → stdout");

        let source_config = pipeline_core::types::pipeline_config::ConnectorConfig {
            connector_type: "stdin".to_string(),
            properties: Default::default(),
            secrets: Default::default(),
        };

        let sink_config = pipeline_core::types::pipeline_config::ConnectorConfig {
            connector_type: "stdout".to_string(),
            properties: Default::default(),
            secrets: Default::default(),
        };

        let pipeline_config = pipeline_core::types::pipeline_config::PipelineConfig {
            pipeline_id: "demo-stdin-to-stdout".to_string(),
            enabled: true,
            source: source_config.clone(),
            sink: sink_config.clone(),
            field_mappings: Vec::new(),
            transforms: Vec::new(),
            schema: None,
            rate_limits: Default::default(),
            flags: Default::default(),
        };

        let source = create_source(&source_config)?;
        let sink = create_sink(&sink_config)?;

        engine.spawn_pipeline(pipeline_config, source, sink, Vec::new()).await?;

        info!("Demo pipeline spawned. Type JSON objects (one per line) and press Enter.");
        info!("Press Ctrl+C to shut down.");

        // Block until CTRL+C for graceful shutdown
        tokio::signal::ctrl_c().await?;
        info!("Shutdown signal received. Stopping all pipelines...");
    } else if let Some(config_path) = &cli.config {
        // Load pipeline config from file
        let content = tokio::fs::read_to_string(config_path).await?;
        let pipeline_config = pipeline_config::load_from_json(&content)?;

        let source = create_source(&pipeline_config.source)?;
        let sink = create_sink(&pipeline_config.sink)?;

        engine.spawn_pipeline(pipeline_config, source, sink, Vec::new()).await?;

        info!("Pipeline spawned from config: {}", config_path);
        info!("Press Ctrl+C to shut down.");

        // Block until CTRL+C for graceful shutdown
        tokio::signal::ctrl_c().await?;
        info!("Shutdown signal received. Stopping all pipelines...");
    } else {
        info!("No config specified. Use --demo for a quick test or --config <path> to load a pipeline.");
        info!("Starting control plane API on port {}...", cli.api_port);
        
        // Start the control plane server
        let server_handle = tokio::spawn(async move {
            if let Err(e) = start_server(engine, cli.api_port).await {
                error!("Control plane server failed: {}", e);
            }
        });

        // Wait for shutdown signal
        tokio::signal::ctrl_c().await?;
        info!("Shutdown signal received. Stopping control plane server...");
        
        // Abort the server
        server_handle.abort();
    }

    // List and log all pipeline statuses
    for (id, status) in engine.list_pipelines().await {
        info!(pipeline_id = %id, status = ?status, "Pipeline final status");
    }

    info!("Rust Data Pipeline Engine shut down gracefully.");
    Ok(())
}
