//! Secrets resolution — fetches credentials from cloud vaults at runtime.

pub mod resolver;

use pipeline_core::error::PipelineError;

/// Resolve a secret reference (URN) to its plaintext value.
/// Supports: AWS Secrets Manager, Azure Key Vault, GCP Secret Manager, env vars.
pub async fn resolve_secret(secret_ref: &str) -> Result<String, PipelineError> {
    resolver::resolve(secret_ref).await
}
