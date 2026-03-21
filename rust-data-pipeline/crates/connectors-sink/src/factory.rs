//! Sink Connector Factory — Abstract Factory pattern for creating sink connectors.

use pipeline_core::traits::sink::SinkConnector;
use pipeline_core::types::pipeline_config::ConnectorConfig;
use pipeline_core::error::PipelineError;

use crate::databases::stdout::StdoutSink;
use crate::databases::postgres::PostgresSink;

/// Create a sink connector from a configuration block.
pub fn create_sink(config: &ConnectorConfig) -> Result<Box<dyn SinkConnector>, PipelineError> {
    match config.connector_type.as_str() {
        "stdout" => Ok(Box::new(StdoutSink::new())),

        // Warehouse Sinks
        // "snowflake" => Ok(Box::new(warehouses::snowflake::SnowflakeSink::from_config(config)?)),
        // "bigquery" => Ok(Box::new(warehouses::bigquery::BigQuerySink::from_config(config)?)),
        // "databricks" => Ok(Box::new(warehouses::databricks::DatabricksSink::from_config(config)?)),
        // "clickhouse" => Ok(Box::new(warehouses::clickhouse::ClickHouseSink::from_config(config)?)),

        // Database Sinks
        "postgres" => Ok(Box::new(PostgresSink::from_config(config)?)),
        // "mysql" => Ok(Box::new(databases::mysql::MysqlSink::from_config(config)?)),
        // "mongodb" => Ok(Box::new(databases::mongodb::MongoDbSink::from_config(config)?)),
        // "redis" => Ok(Box::new(databases::redis::RedisSink::from_config(config)?)),
        // "elasticsearch" => Ok(Box::new(databases::elasticsearch::ElasticsearchSink::from_config(config)?)),

        // Lakehouse Sinks
        // "delta_lake" => Ok(Box::new(lakehouse::delta_lake::DeltaLakeSink::from_config(config)?)),
        // "iceberg" => Ok(Box::new(lakehouse::iceberg::IcebergSink::from_config(config)?)),

        // Object Storage Sinks
        // "s3" => Ok(Box::new(object_storage::s3::S3Sink::from_config(config)?)),
        // "gcs" => Ok(Box::new(object_storage::gcs::GcsSink::from_config(config)?)),

        unknown => Err(PipelineError::ConfigError {
            message: format!("Unknown sink connector type: '{}'. Available: stdout, postgres, mysql, mongodb, redis, elasticsearch, snowflake, bigquery, databricks, delta_lake, iceberg, s3, gcs", unknown),
        }),
    }
}
