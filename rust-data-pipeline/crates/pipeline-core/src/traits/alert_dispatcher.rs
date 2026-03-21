//! Alert dispatcher trait — the interface for all alerting integrations.

use async_trait::async_trait;

use crate::error::PipelineError;
use crate::types::alert::Alert;

/// Every alerting integration (Webhook, PagerDuty, Slack, Custom)
/// implements this trait. The Observer Pattern dispatches alerts
/// to all registered dispatchers when thresholds are breached.
#[async_trait]
pub trait AlertDispatcher: Send + Sync {
    /// Name of this dispatcher (e.g., "pagerduty", "slack_webhook").
    fn name(&self) -> &str;

    /// Send an alert notification to the external system.
    async fn dispatch(&self, alert: &Alert) -> Result<(), PipelineError>;
}
