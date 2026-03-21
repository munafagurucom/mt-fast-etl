//! Alert types for the alerting subsystem (PRD Requirement #11).

use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};

/// An alert event dispatched to all registered AlertDispatchers.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Alert {
    /// Pipeline that triggered the alert.
    pub pipeline_id: String,

    /// Severity level.
    pub severity: AlertSeverity,

    /// Short summary of the alert condition.
    pub title: String,

    /// Detailed description.
    pub description: String,

    /// When the condition was first detected.
    pub timestamp: DateTime<Utc>,

    /// Key-value labels for routing and filtering.
    pub labels: std::collections::HashMap<String, String>,
}

/// Alert severity levels mapped to PagerDuty severity values.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub enum AlertSeverity {
    /// Informational — no action required.
    Info,
    /// Warning — attention recommended.
    Warning,
    /// Critical — immediate action required. Triggers PagerDuty incident.
    Critical,
}
