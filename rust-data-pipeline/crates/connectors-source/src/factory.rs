//! Source Connector Factory — Abstract Factory pattern for creating source connectors.

use pipeline_core::traits::source::SourceConnector;
use pipeline_core::types::pipeline_config::ConnectorConfig;
use pipeline_core::error::PipelineError;

use crate::brokers::stdin::StdinSource;
use crate::brokers::kafka::KafkaSource;

/// Create a source connector from a configuration block.
/// This is the Abstract Factory pattern: the caller doesn't know which
/// concrete type is returned — only that it implements SourceConnector.
pub fn create_source(config: &ConnectorConfig) -> Result<Box<dyn SourceConnector>, PipelineError> {
    match config.connector_type.as_str() {
        "stdin" => Ok(Box::new(StdinSource::new())),

        // Message Broker Sources (Tier 1)
        "kafka" => Ok(Box::new(KafkaSource::from_config(config)?)),
        // "kinesis" => Ok(Box::new(brokers::kinesis::KinesisSource::from_config(config)?)),
        // "pulsar" => Ok(Box::new(brokers::pulsar::PulsarSource::from_config(config)?)),
        // "nats" => Ok(Box::new(brokers::nats::NatsSource::from_config(config)?)),
        // "sqs" => Ok(Box::new(brokers::sqs::SqsSource::from_config(config)?)),

        // CDC Sources (Tier 2)
        // "postgres_cdc" => Ok(Box::new(cdc::postgres::PostgresCdcSource::from_config(config)?)),
        // "mysql_cdc" => Ok(Box::new(cdc::mysql::MysqlCdcSource::from_config(config)?)),
        // "mongodb_cdc" => Ok(Box::new(cdc::mongodb::MongoDbCdcSource::from_config(config)?)),

        // SaaS Sources (Tier 2-3)
        // "salesforce" => Ok(Box::new(saas::connectors::salesforce::SalesforceSource::from_config(config)?)),
        // "hubspot" => Ok(Box::new(saas::connectors::hubspot::HubspotSource::from_config(config)?)),

        unknown => Err(PipelineError::ConfigError {
            message: format!("Unknown source connector type: '{}'. Available: stdin, kafka, kinesis, pulsar, nats, sqs, postgres_cdc, mysql_cdc, mongodb_cdc, salesforce, hubspot", unknown),
        }),
    }
}
