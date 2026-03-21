# Rust Data Pipeline PRD: Part 1 - Sources, Sinks, and Routing

## 1. Source Connectors Integration
The engine integrates with over 600 specific sources.
- **Message Brokers (Kafka, Kinesis, Pulsar):** Natively integrated using `rdkafka`, `aws-sdk-kinesis`, and `pulsar` crates. The engine spawns long-lived `tokio` asynchronous consumer tasks. Backpressure is natively managed via bounded `mpsc` channels.
- **Database CDC (PostgreSQL, MySQL, MongoDB):** Utilizes `tokio-postgres` for Logical Decoding (WAL), `mysql_async` for Binlog replication, and `mongodb` for Change Streams. Checkpointing is maintained using an S3 Memento.
- **REST APIs (Salesforce, HubSpot, etc.):** Integrates via a generic `reqwest` and `tower` based HTTP client supporting cursors, offset pagination, and OAuth2 refreshes.
- **WASM BYOC:** Custom proprietary sources can be integrated via WebAssembly (`extism`), allowing users to inject custom Python/Go extraction logic without recompiling the Rust host.

## 4. Custom Input Connectors
Users can develop custom input connectors through two mechanisms:
1. **Native Trait Implementation:** Implementing the `#[async_trait] pub trait SourceConnector`. Developers wrap their own TCP/HTTP extraction logic into a stream of byte arrays.
2. **WebAssembly (Extism):** Users compile their bespoke extraction logic into `.wasm`. The Rust engine dynamically loads the module at runtime, hands it the network execution context via capabilities, and streams the yielded bytes into the primary pipeline buffer.

## 7. Sink Connectors Integration
The engine writes to over 100 destinations.
- **Data Warehouses (Snowflake, BigQuery):** Utilizes micro-batching. Data is transformed into Apache Parquet in memory, flushed to a cloud staging bucket (S3/GCS), and loaded via native commands (`COPY INTO` for Snowflake, Storage Write API for BigQuery).
- **Delta Lake / Iceberg:** Handled via pure Rust implementations (`deltalake-rs` and `iceberg-rs`). These sinks execute ACID-compliant transactional appends directly to object storage without needing Spark/Databricks compute.
- **Operational Databases (PostgreSQL, Redis, Elasticsearch):** Leverages bulk/pipeline protocols. `sqlx` COPY for Postgres, Pipelined multiplexing for Redis, and NDJSON `_bulk` APIs for Elasticsearch. Connection pooling is strictly governed by `deadpool`.

## 8. Big Data Sink Routing & Segregation
When writing to Object Storage, Delta Lake, or Apache Iceberg, the pipeline supports **Dynamic Content-Based Routing**.
- **Attribute-Based Partitioning:** The user can configure partition keys based on payload attributes. E.g., `partition_by: ["tenant_id", "event_date"]`.
- **Hive-Style Layouts:** The Rust sink will dynamically segregate the incoming Arrow RecordBatch into distinct Parquet files targeting discrete pathways: `s3://bucket/table/tenant_id=123/event_date=2023-10-01/data.parquet`.
- **Multi-Table Fanout:** A single pipeline can evaluate a regex or direct field map to route specific event types to entirely separate Delta/Iceberg tables (`event_type == "login" -> users_table`, `event_type == "purchase" -> sales_table`).
