//! Secret resolver — dispatches to the appropriate vault based on URN prefix.

use pipeline_core::error::PipelineError;
use tracing::info;

/// Resolve a secret reference to its plaintext value.
///
/// Supported URN formats:
/// - `env:VARIABLE_NAME` → reads from environment
/// - `arn:aws:secretsmanager:...` → AWS Secrets Manager (TODO: implement)
/// - `https://*.vault.azure.net/...` → Azure Key Vault (TODO: implement)
/// - `gcp:projects/*/secrets/*/versions/*` → GCP Secret Manager (TODO: implement)
pub async fn resolve(secret_ref: &str) -> Result<String, PipelineError> {
    if secret_ref.starts_with("env:") {
        let var_name = &secret_ref[4..];
        std::env::var(var_name).map_err(|_| PipelineError::SecretRetrievalFailed {
            key: var_name.to_string(),
            message: format!("Environment variable '{}' not set", var_name),
        })
    } else if secret_ref.starts_with("arn:aws:secretsmanager:") {
        info!("Resolving AWS Secrets Manager secret: {}", secret_ref);
        // TODO: Implement aws-sdk-secretsmanager integration
        Err(PipelineError::SecretRetrievalFailed {
            key: secret_ref.to_string(),
            message: "AWS Secrets Manager not yet implemented".to_string(),
        })
    } else if secret_ref.contains("vault.azure.net") {
        info!("Resolving Azure Key Vault secret: {}", secret_ref);
        // TODO: Implement Azure Key Vault integration
        Err(PipelineError::SecretRetrievalFailed {
            key: secret_ref.to_string(),
            message: "Azure Key Vault not yet implemented".to_string(),
        })
    } else if secret_ref.starts_with("gcp:") {
        info!("Resolving GCP Secret Manager secret: {}", secret_ref);
        // TODO: Implement GCP Secret Manager integration
        Err(PipelineError::SecretRetrievalFailed {
            key: secret_ref.to_string(),
            message: "GCP Secret Manager not yet implemented".to_string(),
        })
    } else {
        // Treat as a literal value (not recommended for production)
        Ok(secret_ref.to_string())
    }
}
