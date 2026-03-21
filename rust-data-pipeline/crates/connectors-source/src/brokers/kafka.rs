//! Kafka Source connector — high-throughput consumption from Apache Kafka topics.
//! Uses rdkafka with tokio integration for async, zero-copy performance.

use std::collections::HashMap;
use std::sync::Arc;
use std::time::Duration;

use arrow::array::{StringArray, Int64Array, Float64Array, BooleanArray};
use arrow::datatypes::{Schema, Field, DataType};
use arrow::record_batch::RecordBatch;
use async_trait::async_trait;
use rdkafka::config::ClientConfig;
use rdkafka::consumer::{Consumer, StreamConsumer, CommitMode};
use rdkafka::message::BorrowedMessage;
use rdkafka::topic_partition_list::TopicPartitionList;
use serde_json::Value;
use tokio::time::timeout;
use tracing::{info, warn, error};

use pipeline_core::error::PipelineError;
use pipeline_core::traits::source::SourceConnector;
use pipeline_core::types::checkpoint::Checkpoint;
use pipeline_core::types::pipeline_config::ConnectorConfig;

/// Configuration for Kafka source connector
#[derive(Debug, Clone)]
pub struct KafkaSourceConfig {
    pub bootstrap_servers: String,
    pub topics: Vec<String>,
    pub group_id: String,
    pub client_id: String,
    pub auto_offset_reset: String, // "earliest" or "latest"
    pub enable_auto_commit: bool,
    pub batch_size: usize,
    pub poll_timeout_ms: u64,
    pub security_protocol: Option<String>, // "PLAINTEXT", "SSL", "SASL_SSL"
    pub sasl_mechanism: Option<String>,    // "PLAIN", "SCRAM-SHA-256", etc.
    pub sasl_username: Option<String>,
    pub sasl_password: Option<String>,
}

impl TryFrom<&ConnectorConfig> for KafkaSourceConfig {
    type Error = PipelineError;

    fn try_from(config: &ConnectorConfig) -> Result<Self, Self::Error> {
        let properties = &config.properties;
        
        Ok(KafkaSourceConfig {
            bootstrap_servers: properties.get("bootstrap_servers")
                .ok_or_else(|| PipelineError::ConfigError {
                    message: "Missing required property: bootstrap_servers".to_string(),
                })?
                .clone(),
            topics: properties.get("topics")
                .ok_or_else(|| PipelineError::ConfigError {
                    message: "Missing required property: topics".to_string(),
                })?
                .split(',')
                .map(|s| s.trim().to_string())
                .collect(),
            group_id: properties.get("group_id")
                .cloned()
                .unwrap_or_else(|| "rust-data-pipeline".to_string()),
            client_id: properties.get("client_id")
                .cloned()
                .unwrap_or_else(|| "rust-data-pipeline-source".to_string()),
            auto_offset_reset: properties.get("auto_offset_reset")
                .cloned()
                .unwrap_or_else(|| "latest".to_string()),
            enable_auto_commit: properties.get("enable_auto_commit")
                .and_then(|v| v.parse().ok())
                .unwrap_or(false),
            batch_size: properties.get("batch_size")
                .and_then(|v| v.parse().ok())
                .unwrap_or(100),
            poll_timeout_ms: properties.get("poll_timeout_ms")
                .and_then(|v| v.parse().ok())
                .unwrap_or(1000),
            security_protocol: properties.get("security_protocol").cloned(),
            sasl_mechanism: properties.get("sasl_mechanism").cloned(),
            sasl_username: config.secrets.get("sasl_username").cloned(),
            sasl_password: config.secrets.get("sasl_password").cloned(),
        })
    }
}

/// Kafka source connector that consumes messages and converts them to Arrow RecordBatches
pub struct KafkaSource {
    config: KafkaSourceConfig,
    consumer: Option<StreamConsumer>,
    current_partition_offsets: HashMap<i32, i64>, // partition -> offset
}

impl KafkaSource {
    pub fn from_config(config: &ConnectorConfig) -> Result<Self, PipelineError> {
        let kafka_config = KafkaSourceConfig::try_from(config)?;
        Ok(Self {
            config: kafka_config,
            consumer: None,
            current_partition_offsets: HashMap::new(),
        })
    }

    fn create_consumer(&self) -> Result<StreamConsumer, PipelineError> {
        let mut client_config = ClientConfig::new();

        // Basic connection settings
        client_config.set("bootstrap.servers", &self.config.bootstrap_servers);
        client_config.set("group.id", &self.config.group_id);
        client_config.set("client.id", &self.config.client_id);
        client_config.set("auto.offset.reset", &self.config.auto_offset_reset);
        client_config.set("enable.auto.commit", &self.config.enable_auto_commit.to_string());

        // Security settings
        if let Some(security_protocol) = &self.config.security_protocol {
            client_config.set("security.protocol", security_protocol);
        }

        if let Some(sasl_mechanism) = &self.config.sasl_mechanism {
            client_config.set("sasl.mechanism", sasl_mechanism);
        }

        if let (Some(username), Some(password)) = (&self.config.sasl_username, &self.config.sasl_password) {
            client_config.set("sasl.username", username);
            client_config.set("sasl.password", password);
        }

        // Performance tuning
        client_config.set("fetch.min.bytes", "1");
        client_config.set("fetch.max.wait.ms", "500");
        client_config.set("max.poll.records", &self.config.batch_size.to_string());

        let consumer: StreamConsumer = client_config
            .create()
            .map_err(|e| PipelineError::ConnectionFailed {
                connector: "kafka".to_string(),
                message: format!("Failed to create Kafka consumer: {}", e),
            })?;

        Ok(consumer)
    }

    fn parse_json_to_arrow(&self, messages: Vec<&BorrowedMessage>) -> Result<RecordBatch, PipelineError> {
        if messages.is_empty() {
            let schema = Arc::new(Schema::new(vec![
                Field::new("topic", DataType::Utf8, false),
                Field::new("partition", DataType::Int64, false),
                Field::new("offset", DataType::Int64, false),
                Field::new("timestamp", DataType::Int64, false),
                Field::new("key", DataType::Utf8, true),
                Field::new("value", DataType::Utf8, false),
                Field::new("headers", DataType::Utf8, true),
            ]));
            return Ok(RecordBatch::new_empty(schema));
        }

        let mut topics = Vec::with_capacity(messages.len());
        let mut partitions = Vec::with_capacity(messages.len());
        let mut offsets = Vec::with_capacity(messages.len());
        let mut timestamps = Vec::with_capacity(messages.len());
        let mut keys = Vec::with_capacity(messages.len());
        let mut values = Vec::with_capacity(messages.len());
        let mut headers = Vec::with_capacity(messages.len());

        for msg in &messages {
            topics.push(msg.topic().to_string());
            partitions.push(msg.partition() as i64);
            offsets.push(msg.offset());
            
            // Convert timestamp to milliseconds
            let timestamp = msg.timestamp()
                .map(|ts| ts.to_millis())
                .unwrap_or_else(|| chrono::Utc::now().timestamp_millis());
            timestamps.push(timestamp);

            // Handle key
            let key = msg.key()
                .map(|k| String::from_utf8_lossy(k).to_string())
                .unwrap_or_else(|| "".to_string());
            keys.push(key);

            // Handle value
            let value = msg.payload()
                .map(|v| String::from_utf8_lossy(v).to_string())
                .unwrap_or_else(|| "".to_string());
            values.push(value);

            // Handle headers
            let headers_str = if msg.headers().is_some() {
                let headers_map: HashMap<String, String> = msg.headers()
                    .unwrap()
                    .iter()
                    .map(|(key, value)| (key.to_string(), String::from_utf8_lossy(value).to_string()))
                    .collect();
                serde_json::to_string(&headers_map).unwrap_or_else(|_| "{}".to_string())
            } else {
                "{}".to_string()
            };
            headers.push(headers_str);

            // Track current offsets for checkpointing
            self.current_partition_offsets.insert(msg.partition(), msg.offset());
        }

        let schema = Arc::new(Schema::new(vec![
            Field::new("topic", DataType::Utf8, false),
            Field::new("partition", DataType::Int64, false),
            Field::new("offset", DataType::Int64, false),
            Field::new("timestamp", DataType::Int64, false),
            Field::new("key", DataType::Utf8, true),
            Field::new("value", DataType::Utf8, false),
            Field::new("headers", DataType::Utf8, true),
        ]));

        let topic_array = StringArray::from(topics);
        let partition_array = Int64Array::from(partitions);
        let offset_array = Int64Array::from(offsets);
        let timestamp_array = Int64Array::from(timestamps);
        let key_array = StringArray::from(keys);
        let value_array = StringArray::from(values);
        let headers_array = StringArray::from(headers);

        RecordBatch::try_new(schema, vec![
            Arc::new(topic_array),
            Arc::new(partition_array),
            Arc::new(offset_array),
            Arc::new(timestamp_array),
            Arc::new(key_array),
            Arc::new(value_array),
            Arc::new(headers_array),
        ]).map_err(|e| PipelineError::Internal(format!("Failed to create Arrow RecordBatch: {}", e)))
    }
}

#[async_trait]
impl SourceConnector for KafkaSource {
    fn name(&self) -> &str {
        "kafka"
    }

    async fn connect(&mut self) -> Result<(), PipelineError> {
        let consumer = self.create_consumer()?;
        
        // Subscribe to topics
        consumer.subscribe(&self.config.topics.iter().map(|s| s.as_str()).collect::<Vec<_>>())
            .map_err(|e| PipelineError::ConnectionFailed {
                connector: "kafka".to_string(),
                message: format!("Failed to subscribe to topics: {}", e),
            })?;

        self.consumer = Some(consumer);
        
        info!(
            topics = ?self.config.topics,
            group_id = %self.config.group_id,
            "KafkaSource connected successfully"
        );
        
        Ok(())
    }

    async fn poll_batch(&mut self) -> Result<RecordBatch, PipelineError> {
        let consumer = self.consumer.as_mut().ok_or_else(|| PipelineError::ConnectionFailed {
            connector: "kafka".to_string(),
            message: "Not connected".to_string(),
        })?;

        let mut messages = Vec::with_capacity(self.config.batch_size);
        let poll_duration = Duration::from_millis(self.config.poll_timeout_ms);

        // Poll for messages with timeout
        match timeout(poll_duration, async {
            for _ in 0..self.config.batch_size {
                match consumer.poll().await {
                    Some(Ok(msg)) => messages.push(msg),
                    Some(Err(e)) => {
                        warn!(error = %e, "Kafka poll error, continuing");
                        continue;
                    }
                    None => break, // No more messages available
                }
            }
        }).await {
            Ok(_) => {}, // Got messages or timeout
            Err(_) => {
                // Timeout occurred - return any messages we have or empty batch
                warn!("Kafka poll timeout");
            }
        }

        if messages.is_empty() {
            // Return empty batch - orchestrator will back off
            let schema = Arc::new(Schema::new(vec![
                Field::new("topic", DataType::Utf8, false),
                Field::new("partition", DataType::Int64, false),
                Field::new("offset", DataType::Int64, false),
                Field::new("timestamp", DataType::Int64, false),
                Field::new("key", DataType::Utf8, true),
                Field::new("value", DataType::Utf8, false),
                Field::new("headers", DataType::Utf8, true),
            ]));
            return Ok(RecordBatch::new_empty(schema));
        }

        // Convert messages to Arrow RecordBatch
        let borrowed_messages: Vec<&BorrowedMessage> = messages.iter().collect();
        let batch = self.parse_json_to_arrow(borrowed_messages)?;

        // Store message references for potential acknowledgment
        // Note: In a real implementation, you'd want to store the actual messages
        // for proper offset management. This is a simplified version.

        Ok(batch)
    }

    async fn acknowledge(&mut self, checkpoint: &Checkpoint) -> Result<(), PipelineError> {
        let consumer = self.consumer.as_mut().ok_or_else(|| PipelineError::ConnectionFailed {
            connector: "kafka".to_string(),
            message: "Not connected".to_string(),
        })?;

        // Parse checkpoint to get offset information
        // Checkpoint format: "partition1:offset1,partition2:offset2"
        if let Some(offset_data) = checkpoint.offset.get("kafka_offsets") {
            let mut tpl = TopicPartitionList::new();
            
            for pair in offset_data.split(',') {
                let parts: Vec<&str> = pair.split(':').collect();
                if parts.len() == 2 {
                    if let (Ok(partition), Ok(offset)) = (parts[0].parse::<i32>(), parts[1].parse::<i64>()) {
                        // For each topic we're subscribed to, add the partition+offset
                        for topic in &self.config.topics {
                            tpl.add_partition_offset(topic, partition, offset + 1); // +1 for next message
                        }
                    }
                }
            }

            if !tpl.count() == 0 {
                consumer.commit(&tpl, CommitMode::Sync)
                    .map_err(|e| PipelineError::Internal(format!("Failed to commit Kafka offsets: {}", e)))?;
                
                info!(offsets = ?offset_data, "Committed Kafka offsets");
            }
        }

        Ok(())
    }

    async fn disconnect(&mut self) -> Result<(), PipelineError> {
        if let Some(consumer) = self.consumer.take() {
            // Consumer will be dropped automatically, which triggers graceful shutdown
            drop(consumer);
        }
        
        self.current_partition_offsets.clear();
        info!("KafkaSource disconnected");
        Ok(())
    }
}
