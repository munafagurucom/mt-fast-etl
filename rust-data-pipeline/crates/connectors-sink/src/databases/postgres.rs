//! PostgreSQL Sink connector — high-performance batch writes to PostgreSQL.
//! Uses sqlx with connection pooling and prepared statements for optimal performance.

use std::collections::HashMap;
use std::time::Instant;

use arrow::array::{Array, ArrayRef, StringArray, Int64Array, Float64Array, BooleanArray, TimestampMillisecondArray};
use arrow::datatypes::{DataType, Schema};
use arrow::record_batch::RecordBatch;
use async_trait::async_trait;
use sqlx::{PgPool, Row, postgres::PgRow};
use tracing::{info, warn, error};

use pipeline_core::error::PipelineError;
use pipeline_core::traits::sink::SinkConnector;
use pipeline_core::types::write_receipt::WriteReceipt;
use pipeline_core::types::pipeline_config::ConnectorConfig;

/// Configuration for PostgreSQL sink connector
#[derive(Debug, Clone)]
pub struct PostgresSinkConfig {
    pub connection_string: String,
    pub table_name: String,
    pub schema_name: Option<String>,
    pub batch_size: usize,
    pub max_connections: u32,
    pub create_table_if_not_exists: bool,
    pub upsert_mode: bool,
    pub upsert_keys: Vec<String>,
}

impl TryFrom<&ConnectorConfig> for PostgresSinkConfig {
    type Error = PipelineError;

    fn try_from(config: &ConnectorConfig) -> Result<Self, Self::Error> {
        let properties = &config.properties;
        
        let connection_string = config.secrets.get("connection_string")
            .or_else(|| properties.get("connection_string"))
            .ok_or_else(|| PipelineError::ConfigError {
                message: "Missing required property: connection_string".to_string(),
            })?
            .clone();

        Ok(PostgresSinkConfig {
            connection_string,
            table_name: properties.get("table_name")
                .ok_or_else(|| PipelineError::ConfigError {
                    message: "Missing required property: table_name".to_string(),
                })?
                .clone(),
            schema_name: properties.get("schema_name").cloned(),
            batch_size: properties.get("batch_size")
                .and_then(|v| v.parse().ok())
                .unwrap_or(1000),
            max_connections: properties.get("max_connections")
                .and_then(|v| v.parse().ok())
                .unwrap_or(10),
            create_table_if_not_exists: properties.get("create_table_if_not_exists")
                .and_then(|v| v.parse().ok())
                .unwrap_or(false),
            upsert_mode: properties.get("upsert_mode")
                .and_then(|v| v.parse().ok())
                .unwrap_or(false),
            upsert_keys: properties.get("upsert_keys")
                .map(|keys| keys.split(',').map(|s| s.trim().to_string()).collect())
                .unwrap_or_default(),
        })
    }
}

/// PostgreSQL sink connector that writes Arrow RecordBatches to PostgreSQL tables
pub struct PostgresSink {
    config: PostgresSinkConfig,
    pool: Option<PgPool>,
    total_rows_written: u64,
    prepared_statements: HashMap<String, sqlx::postgres::PgStatement<'static>>,
}

impl PostgresSink {
    pub fn from_config(config: &ConnectorConfig) -> Result<Self, PipelineError> {
        let pg_config = PostgresSinkConfig::try_from(config)?;
        Ok(Self {
            config: pg_config,
            pool: None,
            total_rows_written: 0,
            prepared_statements: HashMap::new(),
        })
    }

    fn get_full_table_name(&self) -> String {
        match &self.config.schema_name {
            Some(schema) => format!("{}.{}", schema, self.config.table_name),
            None => self.config.table_name.clone(),
        }
    }

    async fn create_table_if_needed(&self, pool: &PgPool, schema: &Schema) -> Result<(), PipelineError> {
        let table_name = self.get_full_table_name();
        let mut columns = Vec::new();

        for field in schema.fields() {
            let pg_type = match field.data_type() {
                DataType::Utf8 => "TEXT",
                DataType::Int64 => "BIGINT",
                DataType::Float64 => "DOUBLE PRECISION",
                DataType::Boolean => "BOOLEAN",
                DataType::Timestamp(_, _) => "TIMESTAMP WITH TIME ZONE",
                DataType::Date32 => "DATE",
                DataType::Decimal128(_, _) => "NUMERIC",
                _ => {
                    warn!(field_name = %field.name(), data_type = ?field.data_type(), 
                          "Using TEXT for unsupported Arrow type");
                    "TEXT"
                }
            };

            columns.push(format!("{} {}", field.name(), pg_type));
        }

        let create_sql = if self.config.upsert_mode && !self.config.upsert_keys.is_empty() {
            let primary_key = self.config.upsert_keys.join(", ");
            format!(
                "CREATE TABLE IF NOT EXISTS {} ({}, PRIMARY KEY ({}))",
                table_name,
                columns.join(", "),
                primary_key
            )
        } else {
            format!(
                "CREATE TABLE IF NOT EXISTS {} ({})",
                table_name,
                columns.join(", ")
            )
        };

        sqlx::query(&create_sql)
            .execute(pool)
            .await
            .map_err(|e| PipelineError::SinkWriteFailed {
                connector: "postgres".to_string(),
                retries: 0,
                message: format!("Failed to create table {}: {}", table_name, e),
            })?;

        info!(table_name = %table_name, "Created table if not exists");
        Ok(())
    }

    fn build_insert_statement(&self, schema: &Schema) -> String {
        let table_name = self.get_full_table_name();
        let columns: Vec<String> = schema.fields().iter().map(|f| f.name().to_string()).collect();
        let placeholders: Vec<String> = (1..=columns.len()).map(|i| format!("${}", i)).collect();

        if self.config.upsert_mode && !self.config.upsert_keys.is_empty() {
            let update_columns: Vec<String> = columns.iter()
                .filter(|col| !self.config.upsert_keys.contains(col))
                .map(|col| format!("{} = EXCLUDED.{}", col, col))
                .collect();
            
            format!(
                "INSERT INTO {} ({}) VALUES ({}) ON CONFLICT ({}) DO UPDATE SET {}",
                table_name,
                columns.join(", "),
                placeholders.join(", "),
                self.config.upsert_keys.join(", "),
                update_columns.join(", ")
            )
        } else {
            format!(
                "INSERT INTO {} ({}) VALUES ({})",
                table_name,
                columns.join(", "),
                placeholders.join(", ")
            )
        }
    }

    fn convert_arrow_value_to_sql(&array: &ArrayRef, index: usize, data_type: &DataType) -> Result<Box<dyn sqlx::Encode<'sqlx::Postgres> + Send + Sync>, PipelineError> {
        match data_type {
            DataType::Utf8 => {
                let string_array = array.as_any().downcast_ref::<StringArray>()
                    .ok_or_else(|| PipelineError::Internal("Failed to downcast to StringArray".to_string()))?;
                let value = string_array.value(index);
                Ok(Box::new(value.to_string()))
            }
            DataType::Int64 => {
                let int_array = array.as_any().downcast_ref::<Int64Array>()
                    .ok_or_else(|| PipelineError::Internal("Failed to downcast to Int64Array".to_string()))?;
                let value = int_array.value(index);
                Ok(Box::new(value))
            }
            DataType::Float64 => {
                let float_array = array.as_any().downcast_ref::<Float64Array>()
                    .ok_or_else(|| PipelineError::Internal("Failed to downcast to Float64Array".to_string()))?;
                let value = float_array.value(index);
                Ok(Box::new(value))
            }
            DataType::Boolean => {
                let bool_array = array.as_any().downcast_ref::<BooleanArray>()
                    .ok_or_else(|| PipelineError::Internal("Failed to downcast to BooleanArray".to_string()))?;
                let value = bool_array.value(index);
                Ok(Box::new(value))
            }
            DataType::Timestamp(_, _) => {
                let ts_array = array.as_any().downcast_ref::<TimestampMillisecondArray>()
                    .ok_or_else(|| PipelineError::Internal("Failed to downcast to TimestampMillisecondArray".to_string()))?;
                let value = ts_array.value(index);
                // Convert milliseconds timestamp to chrono::DateTime
                let datetime = chrono::DateTime::from_timestamp_millis(value)
                    .ok_or_else(|| PipelineError::Internal("Invalid timestamp value".to_string()))?;
                Ok(Box::new(datetime))
            }
            _ => {
                // Fallback to string representation for unsupported types
                let string_value = format!("{:?}", array);
                Ok(Box::new(string_value))
            }
        }
    }
}

#[async_trait]
impl SinkConnector for PostgresSink {
    fn name(&self) -> &str {
        "postgres"
    }

    async fn connect(&mut self) -> Result<(), PipelineError> {
        let pool = PgPool::connect(&self.config.connection_string)
            .await
            .map_err(|e| PipelineError::ConnectionFailed {
                connector: "postgres".to_string(),
                message: format!("Failed to connect to PostgreSQL: {}", e),
            })?;

        info!(
            table_name = %self.config.table_name,
            max_connections = self.config.max_connections,
            "PostgresSink connected successfully"
        );

        self.pool = Some(pool);
        Ok(())
    }

    async fn write_batch(&mut self, batch: &RecordBatch) -> Result<WriteReceipt, PipelineError> {
        let pool = self.pool.as_ref().ok_or_else(|| PipelineError::ConnectionFailed {
            connector: "postgres".to_string(),
            message: "Not connected".to_string(),
        })?;

        let start = Instant::now();
        let num_rows = batch.num_rows() as u64;
        let schema = batch.schema();

        // Create table if needed (only on first batch)
        if self.total_rows_written == 0 && self.config.create_table_if_not_exists {
            self.create_table_if_needed(pool, schema.as_ref()).await?;
        }

        // Build or get prepared statement
        let statement_key = format!("insert_{}", schema.fields().len());
        let insert_sql = if !self.prepared_statements.contains_key(&statement_key) {
            let sql = self.build_insert_statement(schema.as_ref());
            self.prepared_statements.insert(statement_key.clone(), sqlx::query(&sql).prepare(pool).await.map_err(|e| {
                PipelineError::SinkWriteFailed {
                    connector: "postgres".to_string(),
                    retries: 0,
                    message: format!("Failed to prepare statement: {}", e),
                }
            })?);
            sql
        } else {
            self.build_insert_statement(schema.as_ref())
        };

        // Process rows in smaller chunks to avoid parameter limits
        let chunk_size = std::cmp::min(self.config.batch_size, 100); // PostgreSQL has parameter limits
        let mut total_written = 0u64;

        for chunk_start in (0..batch.num_rows()).step_by(chunk_size) {
            let chunk_end = std::cmp::min(chunk_start + chunk_size, batch.num_rows());
            
            // Start transaction for this chunk
            let mut tx = pool.begin().await.map_err(|e| PipelineError::SinkWriteFailed {
                connector: "postgres".to_string(),
                retries: 0,
                message: format!("Failed to begin transaction: {}", e),
            })?;

            for row_idx in chunk_start..chunk_end {
                let mut query = sqlx::query(&insert_sql);

                // Add each column value as a parameter
                for (col_idx, field) in schema.fields().iter().enumerate() {
                    let array = batch.column(col_idx);
                    let value = self.convert_arrow_value_to_sql(array, row_idx, field.data_type())?;
                    
                    // This is a simplified approach - in practice, you'd need to handle
                    // different value types more carefully with sqlx's query builder
                    match field.data_type() {
                        DataType::Utf8 => {
                            let string_array = array.as_any().downcast_ref::<StringArray>().unwrap();
                            let val = string_array.value(row_idx);
                            query = query.bind(val);
                        }
                        DataType::Int64 => {
                            let int_array = array.as_any().downcast_ref::<Int64Array>().unwrap();
                            let val = int_array.value(row_idx);
                            query = query.bind(val);
                        }
                        DataType::Float64 => {
                            let float_array = array.as_any().downcast_ref::<Float64Array>().unwrap();
                            let val = float_array.value(row_idx);
                            query = query.bind(val);
                        }
                        DataType::Boolean => {
                            let bool_array = array.as_any().downcast_ref::<BooleanArray>().unwrap();
                            let val = bool_array.value(row_idx);
                            query = query.bind(val);
                        }
                        DataType::Timestamp(_, _) => {
                            let ts_array = array.as_any().downcast_ref::<TimestampMillisecondArray>().unwrap();
                            let val = ts_array.value(row_idx);
                            let datetime = chrono::DateTime::from_timestamp_millis(val)
                                .unwrap_or_else(|| chrono::DateTime::from_timestamp(0, 0).unwrap());
                            query = query.bind(datetime);
                        }
                        _ => {
                            // Fallback to string
                            let string_value = format!("{:?}", array);
                            query = query.bind(string_value);
                        }
                    }
                }

                query.execute(&mut *tx).await.map_err(|e| PipelineError::SinkWriteFailed {
                    connector: "postgres".to_string(),
                    retries: 0,
                    message: format!("Failed to insert row: {}", e),
                })?;
            }

            // Commit the transaction
            tx.commit().await.map_err(|e| PipelineError::SinkWriteFailed {
                connector: "postgres".to_string(),
                retries: 0,
                message: format!("Failed to commit transaction: {}", e),
            })?;

            total_written += (chunk_end - chunk_start) as u64;
        }

        self.total_rows_written += total_written;

        Ok(WriteReceipt::new(total_written, 0, start.elapsed()))
    }

    async fn flush(&mut self) -> Result<(), PipelineError> {
        // PostgreSQL writes are immediately committed due to transaction handling
        // No additional flushing needed
        Ok(())
    }

    async fn disconnect(&mut self) -> Result<(), PipelineError> {
        if let Some(pool) = self.pool.take() {
            pool.close().await;
        }
        
        self.prepared_statements.clear();
        info!(total_rows = self.total_rows_written, "PostgresSink disconnected");
        Ok(())
    }
}
