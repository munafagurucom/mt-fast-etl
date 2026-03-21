//! WriteReceipt — acknowledgement returned by sink after a successful batch write.

use std::time::Duration;
use serde::{Deserialize, Serialize};

/// Returned by `SinkConnector::write_batch()` to confirm successful persistence.
/// Used by the orchestrator for observability tracking and checkpoint advancement.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct WriteReceipt {
    /// Number of rows successfully written in this batch.
    pub rows_written: u64,

    /// Bytes written (approximate, for bandwidth tracking).
    pub bytes_written: u64,

    /// Time taken for this write operation.
    #[serde(with = "humantime_serde_compat")]
    pub duration: Duration,
}

mod humantime_serde_compat {
    use serde::{self, Deserialize, Deserializer, Serializer};
    use std::time::Duration;

    pub fn serialize<S>(d: &Duration, s: S) -> Result<S::Ok, S::Error>
    where S: Serializer {
        s.serialize_u64(d.as_millis() as u64)
    }

    pub fn deserialize<'de, D>(d: D) -> Result<Duration, D::Error>
    where D: Deserializer<'de> {
        let ms = u64::deserialize(d)?;
        Ok(Duration::from_millis(ms))
    }
}

impl WriteReceipt {
    pub fn new(rows: u64, bytes: u64, duration: Duration) -> Self {
        Self {
            rows_written: rows,
            bytes_written: bytes,
            duration,
        }
    }
}
