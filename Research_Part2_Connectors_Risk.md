# Deep Research Analysis Part 2: Source/Sink Crate Deep-Dive and Risk Assessment

## 1. Source Connector Crate Ecosystem

### 1.1 Message Broker Sources (Tier 1 — Production Ready)

| Source | Crate | Version | Pure Rust? | Async? | Risk |
|---|---|---|---|---|---|
| **Apache Kafka** | `rdkafka` | 0.39.0 | ❌ (wraps C `librdkafka`) | ✅ Tokio native | Very Low — Battle-tested; 1M msgs/sec benchmarked. EOS (exactly-once) supported. |
| **Amazon Kinesis** | `aws-sdk-kinesis` | Latest AWS SDK | ✅ | ✅ | Very Low — Official AWS SDK for Rust. IAM auth native. |
| **Apache Pulsar** | `pulsar` | 6.x | ✅ | ✅ | Low — Official Pulsar Rust client. JWT + TLS auth. |
| **Redpanda** | `rdkafka` | 0.39.0 | ❌ | ✅ | Very Low — Uses Kafka wire protocol; zero code change. |
| **NATS JetStream** | `async-nats` | 0.38.x | ✅ | ✅ | Very Low — Maintained by Synadia (NATS creators). |
| **MQTT** | `rumqttc` | 0.24.x | ✅ | ✅ | Low — QoS 0/1/2. Lightweight IoT ingestion. |
| **Google Pub/Sub** | `google-cloud-pubsub` | 0.25.x | ✅ | ✅ | Low — Community-maintained; uses `tonic` gRPC. |
| **Amazon SQS** | `aws-sdk-sqs` | Latest AWS SDK | ✅ | ✅ | Very Low — Official AWS SDK. Long-polling support. |

### 1.2 Database CDC Sources (Tier 2 — Connectivity Native, Protocol Parsing Custom)

| Source | Connectivity Crate | Version | CDC Protocol | Custom Work Required |
|---|---|---|---|---|
| **PostgreSQL CDC** | `tokio-postgres` | 0.7.x | Logical Replication (`pgoutput`) | **High:** Must implement WAL `XLogData` binary stream parser. No existing crate decodes `pgoutput` into typed row events. Must handle replication slot lifecycle (create/drop/advance). |
| **MySQL CDC** | `mysql_async` | 0.34.x | Binlog Replication (`BINLOG_DUMP`) | **High:** Must parse `WRITE_ROWS_EVENTv2`, `UPDATE_ROWS_EVENTv2` binary structures. Table map events must be cached to resolve column metadata. |
| **MongoDB CDC** | `mongodb` | 3.5.2 | Change Streams (`collection.watch()`) | **Low:** Official driver supports Change Streams natively with `ResumeToken` for resumability. Just need S3 checkpoint persistence for the resume token. |
| **SQL Server CDC** | `tiberius` | 0.12.x | Polling-based (`cdc.fn_cdc_get_all_changes`) | **Medium:** No native log streaming. Must implement periodic SQL queries against CDC tables and track LSN offsets manually. |

**Critical Risk — PostgreSQL & MySQL CDC:** These are the two most expensive custom development items in the entire project. A robust CDC implementation for each requires approximately **4-8 weeks of focused development** including edge cases (DDL changes mid-stream, transaction interleaving, schema evolution).

### 1.3 SaaS REST API Sources (Tier 2-3 — Generic Framework + Per-Connector Config)

**No vendor-specific Rust crates exist for SaaS integrations.** All 550+ SaaS sources (Salesforce, HubSpot, Stripe, Shopify, Zendesk, Jira, etc.) are integrated using:

| Component | Crate | Role |
|---|---|---|
| HTTP Client | `reqwest` | Execute REST API calls with connection pooling |
| Middleware | `tower` | Retry, rate-limit, timeout, circuit-breaker |
| Auth | `jsonwebtoken` + `reqwest` | OAuth2 token refresh, JWT signing, API key injection |
| Response parsing | `serde_json` | Dynamic JSON deserialization into `serde_json::Value` |

**Per-connector effort:** ~50-100 lines of configuration code (endpoint URL, pagination type, auth type, field mappings). The generic `SaaSConnector` struct handles the execution loop reusably.

---

## 2. Sink Connector Crate Ecosystem

### 2.1 Data Warehouse Sinks

| Sink | Crate(s) | Version | Strategy | Native Driver? | Risk |
|---|---|---|---|---|---|
| **Snowflake** | `reqwest` + `parquet` + `aws-sdk-s3` + `jsonwebtoken` | — | Stage Parquet to S3/GCS → `COPY INTO` via Snowflake SQL REST API | ❌ No pure Rust driver | Medium — Requires JWT key-pair auth implementation; REST API is well-documented |
| **Databricks** | `deltalake` + `reqwest` | 0.31.1 | Write Delta Log + Parquet directly to S3/ADLS OR trigger Databricks SQL API | ✅ `delta-rs` | Low — `delta-rs` is mature (approaching 1.0); supports MERGE, DELETE, VACUUM |
| **Google BigQuery** | `google-cloud-bigquery` + `tonic` | Community | Streaming `insertAll` REST or Storage Write API via gRPC | ✅ Community | Medium — gRPC StubGeneration via `tonic-build` adds build complexity |
| **Amazon Redshift** | `aws-sdk-s3` + `aws-sdk-redshiftdata` | AWS SDK | Stage Parquet to S3 → `COPY` via Redshift Data API | ❌ | Low — Standard AWS SDK pattern |
| **ClickHouse** | `clickhouse-rs` | 1.x | HTTP bulk `INSERT` with native binary protocol | ✅ | Low — Actively maintained; high-performance native protocol |

### 2.2 Operational Database Sinks

| Sink | Crate | Version | Bulk Strategy | Risk |
|---|---|---|---|---|
| **PostgreSQL** | `sqlx` | 0.8.6 | `COPY FROM STDIN` binary, compile-time checked queries | Very Low |
| **MySQL** | `sqlx` (mysql feature) or `mysql_async` | 0.8.6 / 0.34.x | Multi-row `INSERT VALUES (…),(…)` | Very Low |
| **MongoDB** | `mongodb` | 3.5.2 | `insert_many()` with `ordered: false` | Very Low |
| **Redis** | `redis` | 0.27.x | `redis::pipe()` + multiplexed async connection | Very Low |
| **Elasticsearch** | `elasticsearch` | 8.x | `_bulk` NDJSON endpoint | Low |
| **Cassandra** | `scylla` or `cdrs-tokio` | 0.14.x / community | `BatchStatement` with token-aware routing | Low |
| **DynamoDB** | `aws-sdk-dynamodb` | AWS SDK | `BatchWriteItem` (25 items/batch max) | Low |

### 2.3 Lakehouse Format Sinks

| Sink | Crate | Version | Maturity | Risk |
|---|---|---|---|---|
| **Delta Lake** | `deltalake` | 0.31.1 | Approaching 1.0 stable. ACID transactions, MERGE, schema evolution. Supports S3, ADLS, GCS, local. | Very Low |
| **Apache Iceberg** | `iceberg` | 0.8.0 (Jan 2026) | Active Apache project. 144 PRs merged in 0.8.0 from 37 contributors. Supports REST Catalog, AWS Glue, time travel. | Low-Medium — Pre-1.0; API may evolve |
| **Apache Hudi** | None | — | **No official Rust implementation exists.** | **High — Must use WASM or shell-out to JVM** |

---

## 3. Complete Cargo.toml Dependency Manifest

```toml
[package]
name = "rust-data-pipeline"
version = "0.1.0"
edition = "2024"

[dependencies]
# === Async Runtime ===
tokio = { version = "1", features = ["full"] }
async-trait = "0.1"

# === HTTP & Middleware ===
reqwest = { version = "0.12", features = ["json", "rustls-tls"], default-features = false }
tower = { version = "0.5", features = ["retry", "limit", "timeout"] }

# === Serialization ===
serde = { version = "1", features = ["derive"] }
serde_json = "1"
serde_yaml = "0.9"
apache-avro = "0.17"

# === Columnar Data ===
arrow = "58"
parquet = { version = "58", features = ["async"] }
object_store = { version = "0.11", features = ["aws", "gcp", "azure"] }

# === Lakehouse ===
deltalake = { version = "0.31", features = ["s3", "azure", "gcs"] }
iceberg = "0.8"

# === Message Brokers ===
rdkafka = { version = "0.39", features = ["cmake-build", "tokio"] }
pulsar = "6"
async-nats = "0.38"
rumqttc = "0.24"
lapin = "2"   # RabbitMQ

# === Databases ===
sqlx = { version = "0.8", features = ["runtime-tokio", "tls-rustls", "postgres", "mysql", "sqlite"] }
mongodb = { version = "3.5", features = ["tokio-runtime"] }
redis = { version = "0.27", features = ["tokio-comp", "connection-manager"] }
elasticsearch = "8.5.0-alpha.1"
scylla = "0.14"
tiberius = "0.12"                          # SQL Server
clickhouse-rs = "1"

# === Cloud SDKs ===
aws-config = "1"
aws-sdk-s3 = "1"
aws-sdk-kinesis = "1"
aws-sdk-sqs = "1"
aws-sdk-dynamodb = "1"
aws-sdk-secretsmanager = "1"
google-cloud-bigquery = "0.10"
google-cloud-pubsub = "0.25"
google-cloud-storage = "0.20"

# === WebAssembly Plugin System ===
extism = "1.13"

# === Web Framework (Control Plane) ===
axum = "0.8"

# === Security & Auth ===
jsonwebtoken = "9"
uuid = { version = "1", features = ["v4"] }
sha2 = "0.10"                             # SHA-256 hashing for PII masking

# === Rate Limiting ===
governor = "0.10"

# === Observability ===
tracing = "0.1"
tracing-subscriber = { version = "0.3", features = ["json", "env-filter"] }
metrics = "0.24"
metrics-exporter-prometheus = "0.16"

# === Utilities ===
chrono = { version = "0.4", features = ["serde"] }
dotenvy = "0.15"
deadpool = "0.12"
thiserror = "2"
anyhow = "1"
```

---

## 4. Risk Matrix Summary

| Risk | Severity | Affected Area | Mitigation |
|---|---|---|---|
| Postgres/MySQL CDC WAL parsing | 🔴 High | Source Connectors | Budget 4-8 weeks; consider `pg_logical` as reference |
| Apache Hudi integration | 🔴 High | Sink Connectors | No Rust crate. Defer to WASM plugin or JVM shelling |
| Snowflake — no native driver | 🟡 Medium | Sink Connectors | Use SQL REST API + JWT auth; avoid ODBC |
| BigQuery gRPC build complexity | 🟡 Medium | Sink Connectors | `tonic-build` + Protobuf generation in `build.rs` |
| `iceberg` crate API instability (pre-1.0) | 🟡 Medium | Sink Connectors | Pin version; wrapper trait isolates API changes |
| `rdkafka` C dependency (`librdkafka`) | 🟢 Low | Build System | Use `cmake-build` feature to compile from source |
| SaaS API deprecations (550+ APIs) | 🟡 Medium | Source Connectors | WASM BYOC offloads maintenance to users |
| Tokio executor starvation | 🟢 Low | Runtime | `spawn_blocking` for CPU work; bounded channels |
