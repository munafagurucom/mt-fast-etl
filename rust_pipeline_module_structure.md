# Rust Data Pipeline Engine: Package & Module Structure

This document defines the complete Rust workspace, crate, and module hierarchy for the data pipeline engine. The structure is derived directly from the PRD requirements (Parts 1-3), the dependency research (Parts 1-2), the design patterns document, and the full source/sink connector inventories.

The application is organized as a **Cargo Workspace** with multiple internal crates to enforce strict compilation boundaries, enable independent testing, and allow selective feature-gated compilation.

---

## Workspace Layout

```
rust-data-pipeline/
├── Cargo.toml                          # Workspace root manifest
├── Cargo.lock
├── rust-toolchain.toml                 # Pin stable Rust version (e.g., 1.82+)
├── .cargo/
│   └── config.toml                     # Cross-compilation targets, linker settings
├── build.rs                            # Top-level: Protobuf compilation for BigQuery gRPC stubs
│
├── crates/                             # Internal library crates
│   │
│   ├── pipeline-core/                  # ══════════════════════════════════════
│   │   ├── Cargo.toml                  # Core traits, types, errors — zero external deps
│   │   └── src/
│   │       ├── lib.rs
│   │       ├── traits/
│   │       │   ├── mod.rs
│   │       │   ├── source.rs           # `#[async_trait] pub trait SourceConnector`
│   │       │   │                       #   fn name() -> &str
│   │       │   │                       #   async fn connect(&mut self, config) -> Result<()>
│   │       │   │                       #   async fn poll_batch(&mut self) -> Result<RecordBatch>
│   │       │   │                       #   async fn acknowledge(&mut self, checkpoint) -> Result<()>
│   │       │   ├── sink.rs             # `#[async_trait] pub trait SinkConnector`
│   │       │   │                       #   async fn connect(&mut self, config) -> Result<()>
│   │       │   │                       #   async fn write_batch(&mut self, batch: RecordBatch) -> Result<WriteReceipt>
│   │       │   │                       #   async fn flush(&mut self) -> Result<()>
│   │       │   ├── transformer.rs      # `pub trait Transformer: Send + Sync`
│   │       │   │                       #   fn transform(&self, batch: RecordBatch) -> Result<RecordBatch>
│   │       │   ├── serializer.rs       # `pub trait PayloadSerializer` / `PayloadDeserializer`
│   │       │   └── alert_dispatcher.rs # `#[async_trait] pub trait AlertDispatcher`
│   │       │
│   │       ├── types/
│   │       │   ├── mod.rs
│   │       │   ├── pipeline_config.rs  # Structs: PipelineConfig, SourceConfig, SinkConfig, FieldMapping
│   │       │   ├── checkpoint.rs       # Struct: Checkpoint { pipeline_id, offset, timestamp }
│   │       │   ├── write_receipt.rs    # Struct: WriteReceipt { rows_written, bytes, duration }
│   │       │   ├── dlq_record.rs       # Struct: DlqRecord { original_payload, error, timestamp }
│   │       │   └── alert.rs            # Enum: AlertSeverity { Info, Warning, Critical }
│   │       │
│   │       └── error.rs                # `thiserror` based PipelineError enum
│   │
│   ├── pipeline-config/                # ══════════════════════════════════════
│   │   ├── Cargo.toml                  # Depends on: pipeline-core, serde, serde_json, serde_yaml, object_store
│   │   └── src/
│   │       ├── lib.rs
│   │       ├── loader.rs               # Loads pipeline YAML/JSON configs from S3/GCS/local
│   │       ├── validator.rs            # Validates config for required fields, known connector types
│   │       ├── feature_flags.rs        # All feature flags: enable_dlq, schema_drift_mode, require_tls, etc.
│   │       ├── rate_limit_config.rs    # max_source_tps, max_sink_tps, max_concurrent_connections
│   │       └── secrets/
│   │           ├── mod.rs
│   │           ├── resolver.rs         # Dispatches to AWS/Azure/GCP/Vault/Env based on URN prefix
│   │           ├── aws_secrets.rs      # aws-sdk-secretsmanager integration
│   │           ├── azure_keyvault.rs   # Azure Key Vault integration
│   │           ├── gcp_secrets.rs      # Google Secret Manager integration
│   │           ├── vault.rs            # HashiCorp Vault HTTP client
│   │           └── env.rs              # dotenvy / environment variable fallback
│   │
│   ├── pipeline-orchestrator/          # ══════════════════════════════════════
│   │   ├── Cargo.toml                  # Depends on: pipeline-core, pipeline-config, tokio, governor
│   │   └── src/
│   │       ├── lib.rs
│   │       ├── engine.rs              # Main loop: Watches S3 for configs → spawns workers
│   │       ├── worker.rs              # Per-pipeline tokio::spawn task: Source → Transform → Sink
│   │       ├── scheduler.rs           # Dynamic pipeline lifecycle (start, pause, resume, stop)
│   │       ├── backpressure.rs        # Bounded mpsc channel creation + memory budget enforcement
│   │       ├── rate_limiter.rs        # governor::RateLimiter wrappers for source/sink TPS control
│   │       └── checkpoint/
│   │           ├── mod.rs
│   │           ├── manager.rs         # Flush checkpoint to S3 after confirmed sink write
│   │           └── recovery.rs        # On startup: load checkpoints from S3, resume pipelines
│   │
│   ├── connectors-source/             # ══════════════════════════════════════
│   │   ├── Cargo.toml                 # Depends on: pipeline-core, rdkafka, aws-sdk-*, mongodb, etc.
│   │   └── src/
│   │       ├── lib.rs
│   │       ├── factory.rs             # SourceConnectorFactory: config → Box<dyn SourceConnector>
│   │       │
│   │       ├── brokers/               # === Message Broker Sources ===
│   │       │   ├── mod.rs
│   │       │   ├── kafka.rs           # rdkafka StreamConsumer + consumer group management
│   │       │   ├── kinesis.rs         # aws-sdk-kinesis GetShardIterator + GetRecords loop
│   │       │   ├── pulsar.rs          # pulsar crate async consumer
│   │       │   ├── nats.rs            # async-nats JetStream consumer
│   │       │   ├── sqs.rs             # aws-sdk-sqs long-polling receive_message
│   │       │   ├── pubsub.rs          # google-cloud-pubsub pull subscription
│   │       │   ├── mqtt.rs            # rumqttc event loop
│   │       │   └── rabbitmq.rs        # lapin AMQP consumer
│   │       │
│   │       ├── cdc/                   # === Change Data Capture Sources ===
│   │       │   ├── mod.rs
│   │       │   ├── postgres.rs        # tokio-postgres logical replication + pgoutput decoder
│   │       │   ├── mysql.rs           # mysql_async binlog replication + event parser
│   │       │   ├── mongodb.rs         # mongodb watch() Change Streams + resume token
│   │       │   ├── sqlserver.rs       # tiberius CDC polling + LSN tracking
│   │       │   └── common/
│   │       │       ├── mod.rs
│   │       │       ├── wal_decoder.rs     # Shared WAL/binlog binary parsing utilities
│   │       │       └── snapshot.rs        # Initial full-table snapshot before streaming
│   │       │
│   │       ├── object_storage/        # === File/Object Storage Sources ===
│   │       │   ├── mod.rs
│   │       │   ├── s3.rs              # aws-sdk-s3 ListObjectsV2 + GetObject polling
│   │       │   ├── gcs.rs             # google-cloud-storage object listing
│   │       │   ├── azure_blob.rs      # object_store Azure backend
│   │       │   └── file_tracker.rs    # Tracks processed file keys to avoid re-ingestion
│   │       │
│   │       ├── saas/                  # === REST API SaaS Sources (550+) ===
│   │       │   ├── mod.rs
│   │       │   ├── generic_client.rs  # Reusable SaaSConnector struct (reqwest + tower)
│   │       │   ├── auth/
│   │       │   │   ├── mod.rs
│   │       │   │   ├── api_key.rs     # Static API key header injection
│   │       │   │   ├── bearer.rs      # Static Bearer token
│   │       │   │   ├── oauth2.rs      # OAuth2 Client Credentials + token refresh loop
│   │       │   │   └── jwt.rs         # JWT signing (jsonwebtoken) for Salesforce, Snowflake
│   │       │   ├── pagination/
│   │       │   │   ├── mod.rs
│   │       │   │   ├── offset.rs      # ?page=N&per_page=100
│   │       │   │   ├── cursor.rs      # starting_after=<id>
│   │       │   │   ├── timestamp.rs   # ?updated_since=<ISO8601>
│   │       │   │   └── link_header.rs # RFC 5988 Link: <url>; rel="next"
│   │       │   │
│   │       │   ├── connectors/        # Per-SaaS connector configs (P1 priority)
│   │       │   │   ├── mod.rs
│   │       │   │   ├── salesforce.rs
│   │       │   │   ├── hubspot.rs
│   │       │   │   ├── stripe.rs
│   │       │   │   ├── shopify.rs
│   │       │   │   ├── zendesk.rs
│   │       │   │   ├── jira.rs
│   │       │   │   ├── github.rs
│   │       │   │   ├── google_ads.rs
│   │       │   │   └── ... (each ~50-100 lines of config)
│   │       │   └── registry.rs        # Maps connector name → endpoint/auth/pagination config
│   │       │
│   │       ├── webhook/               # === Inbound HTTP Webhook Source ===
│   │       │   ├── mod.rs
│   │       │   └── handler.rs         # axum route: POST /ingest/:pipeline_id
│   │       │
│   │       └── iceberg/               # === Apache Iceberg Source ===
│   │           ├── mod.rs
│   │           └── scanner.rs         # iceberg crate: scan manifests, read Parquet data files
│   │
│   ├── connectors-sink/              # ══════════════════════════════════════
│   │   ├── Cargo.toml                # Depends on: pipeline-core, sqlx, mongodb, redis, etc.
│   │   └── src/
│   │       ├── lib.rs
│   │       ├── factory.rs            # SinkConnectorFactory: config → Box<dyn SinkConnector>
│   │       │
│   │       ├── warehouses/           # === Analytical Warehouse Sinks ===
│   │       │   ├── mod.rs
│   │       │   ├── snowflake.rs      # Parquet → S3 stage → COPY INTO via REST API + JWT auth
│   │       │   ├── databricks.rs     # delta-rs direct write OR Databricks SQL API
│   │       │   ├── bigquery.rs       # insertAll REST OR Storage Write API gRPC (tonic)
│   │       │   ├── redshift.rs       # Parquet → S3 → COPY via Redshift Data API
│   │       │   ├── synapse.rs        # Parquet → ADLS → COPY INTO via Synapse REST
│   │       │   ├── clickhouse.rs     # clickhouse-rs native HTTP bulk INSERT
│   │       │   └── common/
│   │       │       ├── mod.rs
│   │       │       ├── parquet_stager.rs  # Arrow RecordBatch → compressed Parquet → upload to stage
│   │       │       └── copy_trigger.rs    # Issue COPY INTO / LOAD commands after staging
│   │       │
│   │       ├── databases/            # === Operational Database Sinks ===
│   │       │   ├── mod.rs
│   │       │   ├── postgres.rs       # sqlx PgPool + COPY FROM STDIN binary protocol
│   │       │   ├── mysql.rs          # sqlx / mysql_async multi-row INSERT + UPSERT
│   │       │   ├── mongodb.rs        # mongodb insert_many() ordered:false
│   │       │   ├── redis.rs          # redis::pipe() multiplexed pipelining
│   │       │   ├── elasticsearch.rs  # _bulk NDJSON endpoint
│   │       │   ├── cassandra.rs      # scylla BatchStatement + token-aware routing
│   │       │   ├── dynamodb.rs       # aws-sdk-dynamodb BatchWriteItem (25-item chunks)
│   │       │   ├── oracle.rs         # oracle crate (C OCI FFI binding)
│   │       │   ├── sqlserver.rs      # tiberius bulk insert
│   │       │   └── sqlite.rs         # sqlx sqlite feature
│   │       │
│   │       ├── lakehouse/            # === Lakehouse Format Sinks ===
│   │       │   ├── mod.rs
│   │       │   ├── delta_lake.rs     # deltalake crate: RecordBatchWriter → commit transaction
│   │       │   ├── iceberg.rs        # iceberg crate: append data files + update manifests
│   │       │   └── partitioner.rs    # Dynamic Hive-style partitioning by attribute values
│   │       │                         # Splits RecordBatch by partition keys → separate Parquet files
│   │       │                         # e.g., tenant_id=123/event_date=2024-01-01/data.parquet
│   │       │
│   │       ├── object_storage/       # === Object Storage Sinks ===
│   │       │   ├── mod.rs
│   │       │   ├── s3.rs             # aws-sdk-s3 multipart upload
│   │       │   ├── gcs.rs            # google-cloud-storage resumable upload
│   │       │   ├── azure_blob.rs     # object_store Azure backend
│   │       │   └── file_namer.rs     # Generates partitioned file paths with timestamps
│   │       │
│   │       ├── brokers/              # === Message Broker Sinks ===
│   │       │   ├── mod.rs
│   │       │   ├── kafka.rs          # rdkafka FutureProducer + delivery guarantees
│   │       │   ├── kinesis.rs        # aws-sdk-kinesis PutRecords (500 records/batch)
│   │       │   ├── pulsar.rs         # pulsar async producer
│   │       │   ├── sqs.rs            # aws-sdk-sqs SendMessageBatch
│   │       │   ├── pubsub.rs         # google-cloud-pubsub publish
│   │       │   ├── rabbitmq.rs       # lapin AMQP producer
│   │       │   └── mqtt.rs           # rumqttc publish
│   │       │
│   │       ├── vector_stores/        # === AI / Vector Database Sinks ===
│   │       │   ├── mod.rs
│   │       │   ├── qdrant.rs         # qdrant-client official gRPC
│   │       │   ├── pinecone.rs       # reqwest REST upsert
│   │       │   ├── weaviate.rs       # reqwest REST batch objects
│   │       │   └── elasticsearch_knn.rs  # dense_vector mapping via elasticsearch crate
│   │       │
│   │       └── routing/              # === Multi-Table Fanout / Content-Based Routing ===
│   │           ├── mod.rs
│   │           ├── router.rs         # Evaluates routing rules per record → destination table
│   │           ├── rules.rs          # Struct: RoutingRule { field, operator, value, target_table }
│   │           └── fanout_writer.rs  # Manages multiple concurrent SinkConnector instances
│   │
│   ├── pipeline-transform/          # ══════════════════════════════════════
│   │   ├── Cargo.toml               # Depends on: pipeline-core, arrow, serde_json, sha2
│   │   └── src/
│   │       ├── lib.rs
│   │       ├── field_mapper.rs       # Rename fields: source_key → sink_key
│   │       ├── type_coercer.rs       # Cast types: Int→String, Epoch→ISO8601, etc.
│   │       ├── filter.rs             # Predicate filtering: DROP ROW IF amount < 0
│   │       ├── masking.rs            # PII: SHA-256 hash, redaction (***), partial mask
│   │       ├── encryption.rs         # AES-GCM-256 envelope encryption with KMS DEK fetch
│   │       ├── enrichment.rs         # Inject lineage headers: _pipeline_id, _ingest_timestamp
│   │       └── chain.rs             # TransformChain: ordered Vec<Box<dyn Transformer>>
│   │
│   ├── pipeline-schema/             # ══════════════════════════════════════
│   │   ├── Cargo.toml               # Depends on: pipeline-core, arrow, serde_json
│   │   └── src/
│   │       ├── lib.rs
│   │       ├── validator.rs          # Validates RecordBatch against provided Arrow Schema
│   │       ├── inference.rs          # Auto-infer schema from first N records
│   │       ├── drift_detector.rs     # Detect new/dropped/changed columns mid-stream
│   │       ├── drift_policy.rs       # Enum: DriftAction { Halt, DropField, AlterTable, DLQ }
│   │       └── ddl_generator.rs      # Generate CREATE TABLE / ALTER TABLE DDL for sink
│   │
│   ├── pipeline-dlq/                # ══════════════════════════════════════
│   │   ├── Cargo.toml               # Depends on: pipeline-core, object_store, parquet
│   │   └── src/
│   │       ├── lib.rs
│   │       ├── writer.rs            # DLQ writer: wraps failed record + error → S3 partitioned path
│   │       ├── partitioner.rs       # s3://dlq/pipeline_id/error_type/year/month/day/
│   │       └── replay.rs            # CLI command to re-ingest DLQ records back into a pipeline
│   │
│   ├── pipeline-observe/            # ══════════════════════════════════════
│   │   ├── Cargo.toml               # Depends on: pipeline-core, tracing, metrics, metrics-exporter-prometheus
│   │   └── src/
│   │       ├── lib.rs
│   │       ├── tracker.rs           # Per-pipeline Arc<AtomicU64> counters
│   │       ├── metrics_server.rs    # axum /metrics endpoint for Prometheus
│   │       ├── logger.rs            # tracing-subscriber JSON structured logging setup
│   │       ├── lineage.rs           # Record-level lineage metadata injection
│   │       └── catalogue.rs         # Push schema definitions to data catalogue (Hive Metastore/Unity)
│   │
│   ├── pipeline-alert/              # ══════════════════════════════════════
│   │   ├── Cargo.toml               # Depends on: pipeline-core, reqwest
│   │   └── src/
│   │       ├── lib.rs
│   │       ├── dispatcher.rs        # Routes alerts to configured AlertDispatcher implementations
│   │       ├── webhook.rs           # Generic JSON POST webhook alerts
│   │       ├── pagerduty.rs         # PagerDuty Events API v2 (trigger/acknowledge/resolve)
│   │       ├── slack.rs             # Slack Incoming Webhook integration
│   │       └── custom.rs            # User-implemented AlertDispatcher trait
│   │
│   ├── pipeline-wasm/               # ══════════════════════════════════════
│   │   ├── Cargo.toml               # Depends on: pipeline-core, extism
│   │   └── src/
│   │       ├── lib.rs
│   │       ├── plugin_loader.rs     # Load .wasm files from S3 or local disk
│   │       ├── source_plugin.rs     # Wraps WASM guest as Box<dyn SourceConnector>
│   │       ├── sink_plugin.rs       # Wraps WASM guest as Box<dyn SinkConnector>
│   │       ├── transform_plugin.rs  # Wraps WASM guest as Box<dyn Transformer>
│   │       ├── host_functions.rs    # Functions exposed to WASM guest (HTTP fetch, log, etc.)
│   │       └── sandbox.rs           # Memory limits, fuel metering, timeout enforcement
│   │
│   └── pipeline-serde/              # ══════════════════════════════════════
│       ├── Cargo.toml               # Depends on: pipeline-core, serde_json, apache-avro, arrow, csv
│       └── src/
│           ├── lib.rs
│           ├── json.rs              # JSON deserializer → Arrow RecordBatch
│           ├── avro.rs              # Avro deserializer → Arrow RecordBatch
│           ├── csv.rs               # CSV deserializer → Arrow RecordBatch
│           ├── parquet_reader.rs    # Parquet file reader → Arrow RecordBatch
│           ├── custom.rs            # User-implemented PayloadDeserializer / PayloadSerializer
│           └── registry.rs          # Maps format name → deserializer instance
│
├── src/                              # ══════════════════════════════════════
│   └── main.rs                       # Binary entry point
│                                     #   1. Parse CLI args (clap)
│                                     #   2. Initialize tracing/logging
│                                     #   3. Load global config from S3
│                                     #   4. Start orchestrator engine
│                                     #   5. Start control plane (axum) HTTP server
│                                     #   6. Block on tokio::signal::ctrl_c() for graceful shutdown
│
├── tests/                            # ══════════════════════════════════════
│   ├── integration/
│   │   ├── kafka_to_postgres.rs      # Full e2e: Kafka source → Postgres sink
│   │   ├── s3_to_delta.rs            # S3 file source → Delta Lake partitioned write
│   │   ├── saas_to_snowflake.rs      # Mock SaaS API → Parquet staging → Snowflake COPY
│   │   ├── dlq_replay.rs             # Validate DLQ write + replay flow
│   │   └── wasm_plugin.rs            # WASM plugin load + sandbox memory isolation
│   └── unit/
│       ├── transform_chain.rs        # Unit tests for field mapping, type coercion, masking
│       ├── schema_validator.rs       # Schema validation + drift detection
│       ├── rate_limiter.rs           # Governor rate limiter accuracy tests
│       └── routing_rules.rs          # Multi-table fanout rule evaluation
│
├── benches/                          # ══════════════════════════════════════
│   ├── parquet_write.rs              # Benchmark: Arrow RecordBatch → Parquet throughput
│   ├── json_to_arrow.rs             # Benchmark: JSON deserialization → Arrow conversion
│   └── wasm_overhead.rs             # Benchmark: WASM host-guest data transfer overhead
│
├── deploy/                           # ══════════════════════════════════════
│   ├── Dockerfile                    # Multi-stage build: compile Rust → minimal runtime image
│   ├── docker-compose.yml            # Local dev: engine + Kafka + Postgres + Redis
│   ├── helm/                         # Kubernetes Helm chart for BYOC container deployment
│   │   ├── Chart.yaml
│   │   ├── values.yaml
│   │   └── templates/
│   │       ├── deployment.yaml
│   │       ├── service.yaml
│   │       └── configmap.yaml
│   ├── terraform/
│   │   ├── aws/                      # AMI builder + EC2 launch template
│   │   ├── azure/                    # Azure Managed Application package
│   │   └── gcp/                      # GCP Compute Engine image builder
│   └── packer/
│       └── ami.pkr.hcl              # HashiCorp Packer: bake Rust binary into hardened AMI
│
├── sdk/                              # ══════════════════════════════════════
│   └── wasm-connector-sdk/           # Published crate for users building BYOC WASM plugins
│       ├── Cargo.toml
│       └── src/
│           ├── lib.rs
│           ├── source_guest.rs       # Guest-side SourceConnector trait for WASM compilation
│           ├── sink_guest.rs         # Guest-side SinkConnector trait for WASM compilation
│           └── types.rs              # Shared types between host and guest
│
└── docs/                             # ══════════════════════════════════════
    ├── architecture.md
    ├── connector-guide.md
    ├── wasm-plugin-guide.md
    └── configuration-reference.md
```

---

## Crate Dependency Graph

```
                    ┌─────────────────┐
                    │  pipeline-core  │  (traits, types, errors)
                    └────────┬────────┘
          ┌──────────────────┼──────────────────────────┐
          │                  │                           │
          ▼                  ▼                           ▼
┌──────────────────┐ ┌────────────────┐  ┌──────────────────────┐
│ pipeline-config  │ │pipeline-schema │  │  pipeline-transform  │
│ (YAML, secrets)  │ │(validate, DDL) │  │ (map, filter, mask)  │
└───────┬──────────┘ └───────┬────────┘  └──────────┬───────────┘
        │                    │                       │
        ▼                    ▼                       ▼
┌───────────────────────────────────────────────────────────────┐
│                   pipeline-orchestrator                       │
│  (engine, worker, scheduler, checkpoint, rate_limiter)        │
└───────────┬───────────────┬───────────────────┬───────────────┘
            │               │                   │
            ▼               ▼                   ▼
┌───────────────┐  ┌────────────────┐  ┌────────────────┐
│connectors-src │  │ connectors-sink│  │ pipeline-wasm  │
│ (kafka, cdc,  │  │ (pg, snowflake,│  │ (extism BYOC)  │
│  saas, s3...) │  │  delta, s3...) │  └────────────────┘
└───────────────┘  └────────────────┘
                          │
              ┌───────────┼───────────┐
              ▼           ▼           ▼
      ┌────────────┐┌──────────┐┌───────────┐
      │pipeline-dlq││pipeline- ││pipeline-  │
      │(S3 failed  ││observe   ││alert      │
      │ records)   ││(metrics) ││(PagerDuty)│
      └────────────┘└──────────┘└───────────┘
```

---

## Workspace Cargo.toml

```toml
[workspace]
resolver = "2"
members = [
    "crates/pipeline-core",
    "crates/pipeline-config",
    "crates/pipeline-orchestrator",
    "crates/connectors-source",
    "crates/connectors-sink",
    "crates/pipeline-transform",
    "crates/pipeline-schema",
    "crates/pipeline-dlq",
    "crates/pipeline-observe",
    "crates/pipeline-alert",
    "crates/pipeline-wasm",
    "crates/pipeline-serde",
    "sdk/wasm-connector-sdk",
]

[workspace.dependencies]
tokio = { version = "1", features = ["full"] }
async-trait = "0.1"
serde = { version = "1", features = ["derive"] }
serde_json = "1"
arrow = "58"
parquet = { version = "58", features = ["async"] }
object_store = { version = "0.11", features = ["aws", "gcp", "azure"] }
thiserror = "2"
anyhow = "1"
tracing = "0.1"
chrono = { version = "0.4", features = ["serde"] }
uuid = { version = "1", features = ["v4"] }
```
