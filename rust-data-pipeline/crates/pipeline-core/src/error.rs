//! Central error type for the entire pipeline engine.

use thiserror::Error;

/// Unified error enum for all pipeline operations.
/// Every crate in the workspace maps its internal errors into this type.
#[derive(Error, Debug)]
pub enum PipelineError {
    // ── Connection Errors ──────────────────────────────────────────
    #[error("Connection failed for connector '{connector}': {message}")]
    ConnectionFailed {
        connector: String,
        message: String,
    },

    #[error("Authentication failed for connector '{connector}': {message}")]
    AuthenticationFailed {
        connector: String,
        message: String,
    },

    // ── Data Errors ────────────────────────────────────────────────
    #[error("Schema validation failed: {message}")]
    SchemaValidation { message: String },

    #[error("Deserialization failed: {message}")]
    Deserialization { message: String },

    #[error("Transformation '{transform}' failed: {message}")]
    TransformFailed {
        transform: String,
        message: String,
    },

    // ── Sink Errors ────────────────────────────────────────────────
    #[error("Sink write failed for '{connector}' after {retries} retries: {message}")]
    SinkWriteFailed {
        connector: String,
        retries: u32,
        message: String,
    },

    // ── Infrastructure Errors ──────────────────────────────────────
    #[error("Checkpoint persistence failed: {message}")]
    CheckpointFailed { message: String },

    #[error("Secret retrieval failed for key '{key}': {message}")]
    SecretRetrievalFailed { key: String, message: String },

    #[error("Configuration error: {message}")]
    ConfigError { message: String },

    #[error("Rate limit exceeded: {message}")]
    RateLimitExceeded { message: String },

    // ── WASM Plugin Errors ─────────────────────────────────────────
    #[error("WASM plugin '{plugin}' error: {message}")]
    WasmPluginError { plugin: String, message: String },

    // ── Generic Errors ─────────────────────────────────────────────
    #[error("Internal error: {0}")]
    Internal(String),

    #[error(transparent)]
    Arrow(#[from] arrow::error::ArrowError),

    #[error(transparent)]
    Io(#[from] std::io::Error),

    #[error(transparent)]
    SerdeJson(#[from] serde_json::Error),
}
