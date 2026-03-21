# Rust Data Movement Application: Integration Landscape

This document provides a detailed, exhaustive analysis of every integration category required to build the Rust-based 1-to-1 data movement application. For each connector family, it specifies:
1. The **Rust crate ecosystem** that is available out-of-the-box.
2. The **effort tier** required to integrate (Tier 1 = fully native, Tier 2 = adapter needed, Tier 3 = must custom-build).
3. The **specific implementation approach** following the architectural patterns from `rust_pipeline_design_patterns.md`.

---

## Architecture Foundation: Universal Crates (Required for ALL Connectors)

Before building any specific connector, the following crates form the mandatory foundation of the entire Rust application and are used by every source and sink connector equally.

| Crate | Purpose |
|---|---|
| `tokio` | Async runtime: powers all non-blocking I/O and concurrent pipeline tasks |
| `async-trait` | Enables implementing async methods on trait objects (`Box<dyn SourceConnector>`) |
| `serde` + `serde_json` | Universal serialization/deserialization for JSON-based source payloads |
| `reqwest` | HTTP client for all REST API-based Source & Sink connectors  |
| `tower` | Middleware for rate limiting, circuit breakers, and retry logic on REST clients |
| `deadpool` | Generic connection pool for all database (Sink) connections |
| `tracing` + `tracing-subscriber` | Structured JSON logging across all 500+ async tasks |
| `dotenvy` | Securely loads environment variables (credentials, DSNs) at runtime |
| `object_store` | Unified S3/GCS/Azure Blob connector for state checkpointing and DLQ |
| `parquet` + `arrow` | Columnar serialization for bulk loading into data warehouses |
| `extism` | WebAssembly plugin host for Bring Your Own Connector (BYOC) integrations |
| `axum` | Internal HTTP control plane: pipeline orchestration REST API + `/metrics` |
| `jsonwebtoken` | JWT signing for OAuth2 token generation (Salesforce, Snowflake, Azure) |
| `uuid` | Generates deterministic idempotency keys for exactly-once sink writes |
| `chrono` | Timezone-aware timestamp handling across all date-typed source/sink fields |

---

## Part 1: Source Connectors

### 1.1. Message Broker / Streaming Sources ✅ TIER 1 (Native - Out of the Box)

These are the highest-value, highest-throughput sources. The Rust ecosystem has **first-class, production-grade support** for all of them.

| Source | Rust Crate | Integration Approach | Notes |
|---|---|---|---|
| **Apache Kafka** | `rdkafka` | Async `StreamConsumer` loop feeding bounded `mpsc` channel | Highest throughput. Requires librdkafka C library. Use `tokio::spawn` for isolation. |
| **Amazon Kinesis** | `aws-sdk-kinesis` | `GetShardIterator` + `GetRecords` polling loop via `aws-config` | Full AWS Rust SDK available. IAM auth works natively. |
| **Apache Pulsar** | `pulsar` | Native async Pulsar consumer crate built on Tokio | Official Rust client; supports JWT and TLS auth. |
| **Redpanda** | `rdkafka` | Identical to Kafka — Redpanda speaks the Kafka wire protocol | Zero code change from Kafka connector. |
| **NATS JetStream** | `async-nats` | `consumer.messages().await` async stream | Natively async; smallest footprint of all broker sources. |
| **MQTT** | `rumqttc` | Async MQTT event loop via `tokio` | Supports QoS 0/1/2 natively. |
| **Google Pub/Sub** | `google-cloud-pubsub` | Async pull subscription loop | Requires `google-cloud-auth` for ADC credentials. |
| **Amazon SQS** | `aws-sdk-sqs` | `receive_message` polling loop | AWS SDK based; long-polling approach minimizes costs. |

**Implementation Pattern:** Each wraps a long-lived `tokio::spawn` task that pumps messages into a bounded `mpsc::channel`. This naturally enforces **backpressure**: if the downstream Sink slows, the channel fills, pausing the Source without dropping data or crashing.

---

### 1.2. Database CDC (Change Data Capture) Sources ⚠️ TIER 2 (Partial Native + Custom Adapter)

These sources connect to live production databases and tap their internal write-ahead logs (WAL) or binary logs in real-time. The fundamental connectivity crates exist, but **the CDC replication logic must be custom-implemented** using low-level protocol primitives.

| Source | Rust Crate(s) | Integration Approach | Custom Dev Required |
|---|---|---|---|
| **PostgreSQL CDC** | `tokio-postgres` | `START_REPLICATION SLOT` via logical decoding (`pgoutput`) | Yes: Parse WAL `XLogData` byte stream manually into INSERT/UPDATE/DELETE row events |
| **MySQL CDC** | `mysql_async` | Connect in binlog replication mode (`BINLOG_DUMP` command) | Yes: Parse MySQL binary log events (WRITE_ROWS_EVENTv2, UPDATE_ROWS_EVENTv2) |
| **MongoDB CDC** | `mongodb` | `collection.watch()` Change Streams API | Minimal: Mostly native; however resume token persistence on S3 must be custom-built |
| **SQL Server CDC** | `tiberius` | Query SQL Server's CDC system tables (`cdc.fn_cdc_get_all_changes_*`) | Yes: Polling-based; no native log streaming. Must track LSN offsets manually |
| **Snowflake (Source)** | `odbc-api` | Query `CHANGES` clause on streams | Moderate: ODBC driver dependency; manage stream offset tokens explicitly |
| **Amazon S3 (Source)** | `aws-sdk-s3` | List new object keys via `ListObjectsV2` + event notifications | Low: Straightforward SDK usage; custom S3 key cursor tracking needed |
| **Azure Blob Storage** | `object_store` | Unified blob listing API | Low: Directly supported via `object_store` crate with Azure feature flag |
| **Google Cloud Storage** | `object_store` | Same as Azure above | Low: Same crate, GCS feature flag |

**Critical Implementation Detail for CDC:** The most complex component of the entire pipeline. Postgres CDC specifically requires:
1. Running a separate `CREATE REPLICATION SLOT` SQL command before starting.
2. Continuously calling `tokio-postgres::replication::LogicalReplicationStream` to receive WAL events.
3. Decoding the binary `pgoutput` protocol into row-level changes using a custom parser (no out-of-the-box crate decodes the full protocol).
4. **Persisting the WAL LSN (Log Sequence Number) checkpoint to S3** after every successful batch sink to enable exactly-once resumption.

---

### 1.3. SaaS API Sources (REST/GraphQL) ⚠️ TIER 2-3 (Generic Client + Per-Connector Adapter)

The Rust ecosystem has **no pre-built SaaS connectors**. The 550+ REST API sources (HubSpot, Salesforce, Stripe, Shopify, etc.) from the Airbyte list require custom implementation. However, since they all share the same underlying HTTP mechanics, the effort follows a pattern:

**Common Infrastructure (Build Once, Reuse 550x):**

```rust
// Generic SaaS Extractor — built once, configured per source
pub struct SaaSConnector {
    client: reqwest::Client,     // reqwest + tower retry/rate-limit stack
    auth: AuthProvider,          // OAuth2 | ApiKey | Basic | JWT 
    pagination: PaginationMode,  // Cursor | Offset | Timestamp | Keyset
    endpoint: String,
}
```

| Auth Type | Rust Implementation | Effort |
|---|---|---|
| **API Key** (e.g., Twilio, SendGrid) | `reqwest` header injection | Minimal |
| **Bearer Token (static)** (e.g., GitHub) | `reqwest` `Authorization` header | Minimal |
| **OAuth2 Client Credentials** (e.g., Salesforce) | Custom token refresh loop using `reqwest` + `jsonwebtoken` | Moderate |
| **OAuth2 PKCE (User Flow)** (e.g., Google, Twitter) | Requires embedded browser or redirect flow | High |

| Pagination Type | Rust Implementation | Standardizable? |
|---|---|---|
| **Offset/Limit** (e.g., `?page=2&per_page=100`) | Simple integer counter loop | ✅ Yes |
| **Cursor-based** (e.g., Stripe `starting_after`) | Persist last returned cursor key to S3 | ✅ Yes |
| **Timestamp-based** (e.g., `?updated_since=ISO8601`) | Track `last_sync_time` in S3 state | ✅ Yes |
| **Link Header** (RFC 5988) | Parse `Link: <url>; rel="next"` header per response | ✅ Yes |

**Key Insight:** While 550+ specific SaaS connectors need to be written, approximately **80% of the implementation code is shared infrastructure**. Only the URL endpoint, JSON field mappings, and auth type differ per connector. By building a reusable `SaaSConnector` struct, adding a new SaaS source becomes a ~50-line configuration file, not a full re-implementation.

**Priority Tier for SaaS Sources:**

| Priority | Connector Class | Examples | Custom Work | Strategy |
|---|---|---|---|---|
| **P1** | Major CRM/Marketing | Salesforce, HubSpot, Marketo | OAuth2 refresh loop + cursor pagination | Full native implementation |
| **P1** | E-Commerce | Shopify, Stripe, WooCommerce | API Key + offset/cursor pagination | Full native implementation |
| **P2** | Ad Platforms | Google Ads, Facebook Marketing, LinkedIn Ads | OAuth2 PKCE | Native but complex auth |
| **P3** | Long-Tail SaaS | 400+ niche SaaS APIs | REST API Key | **WASM Plugin via `extism`** |

---

### 1.4. Miscellaneous Sources

| Source | Crate | Effort |
|---|---|---|
| **Apache Iceberg** | `iceberg` (Apache official Rust impl) | Tier 2: Scan manifests, read Parquet files from S3 |
| **Webhook (HTTP Ingress)** | `axum` | Tier 1: Extend the control plane `axum` server with inbound webhook routes |
| **RSS/Atom Feeds** | `feed-rs` | Tier 1: Pure Rust RSS/Atom parser |
| **SFTP** | `openssh` + `russh` | Tier 2: SSH file listing and streaming |
| **Oracle DB** | `oracle` (via C driver) | Tier 3: Requires Oracle Instant Client C libraries; ODBC layer needed |
| **SAP S/4HANA** | Custom `reqwest` OData client | Tier 3: OData v4 protocol; no native Rust crate exists |

---

## Part 2: Sink Connectors

### 2.1. Data Warehouses & Lakehouses ✅ TIER 1-2 (Native with Staged Loading Pattern)

The recommended pattern for **all** analytical sinks (Snowflake, Databricks, BigQuery, Redshift) is identical:
1. Rust buffers incoming records in memory (using an Arrow `RecordBatch`).
2. Once the buffer reaches a configurable threshold (e.g., 50MB or 60 seconds), serialize to a compressed **Apache Parquet** file using the `parquet` crate.
3. Upload the Parquet file to an intermediate staging area (S3 / GCS / ADLS) via the relevant object storage SDK.
4. Issue a `COPY INTO table FROM @stage` (Snowflake) or trigger a Load Job (BigQuery / Redshift) to atomically ingest the staged file.

| Sink | Rust Crates | Native Driver? | Recommended Approach |
|---|---|---|---|
| **Snowflake** | `odbc-api`, `reqwest`, `parquet`, `aws-sdk-s3` | ❌ No pure Rust driver | Stage Parquet to S3 → `COPY INTO` via Snowflake SQL REST API with `jsonwebtoken` JWT auth |
| **Databricks / Delta Lake** | `deltalake`, `reqwest`, `parquet` | ✅ `deltalake-rs` Rust library | Write Delta Log + Parquet directly to S3/ADLS via `deltalake` crate, bypassing Databricks compute |
| **Google BigQuery** | `google-cloud-bigquery`, `tonic` | ✅ Community crate | Stream via `insertAll` REST or Storage Write API via gRPC (`tonic`) |
| **Amazon Redshift** | `aws-sdk-s3` + `reqwest` (Redshift Data API) | ❌ No native driver | Stage Parquet to S3 → `COPY` via Redshift Data API REST |
| **Azure Synapse** | `object_store` + `reqwest` | ❌ No native driver | Stage Parquet to ADLS Gen2 → Trigger Synapse `COPY INTO` via REST |
| **ClickHouse** | `clickhouse-rs` | ✅ Native Rust driver | Bulk `INSERT` via HTTP native interface; very fast |
| **Apache Iceberg** | `iceberg` | ✅ Apache official Rust lib | Write to Iceberg table directly; merges Parquet data files and updates manifest |
| **Delta Lake** | `deltalake` | ✅ Delta-RS library | Write Delta files independently of any Databricks cluster; pure Rust |
| **Firebolt** | `reqwest` (REST API) | ❌ No native driver | REST API based; simple HTTP POST queries |
| **AlloyDB / PostgreSQL** | `sqlx` | ✅ Native async | `COPY FROM STDIN` binary protocol for bulk loads |
| **SingleStore** | `mysql_async` | ✅ Via MySQL wire protocol | SingleStore speaks MySQL protocol; use `LOAD DATA` |
| **Teradata** | `odbc-api` | ❌ C ODBC driver only | Requires Teradata ODBC shared library on host |

---

### 2.2. Operational Databases ✅ TIER 1 (Fully Native)

| Sink | Rust Crate | Implementation | Bulk Load Strategy |
|---|---|---|---|
| **PostgreSQL** | `sqlx`, `tokio-postgres` | `PgPool` + `COPY FROM STDIN` binary protocol | ✅ COPY protocol: row structures serialized directly to binary wire format |
| **MySQL / MariaDB** | `sqlx` (MySQL feature), `mysql_async` | Connection pool → `INSERT ... ON DUPLICATE KEY UPDATE` | Multi-row `INSERT VALUES (…),(…)` batching |
| **MongoDB** | `mongodb` | `Collection::insert_many()` async bulk writes | `ordered: false` for maximum insert throughput |
| **Redis** | `redis` (tokio feature) | Multiplexed async connection; `redis::pipe()` | Pipeline 5000+ `SET`/`HSET` commands per RTT |
| **Elasticsearch** | `elasticsearch` (official crate) | Async `_bulk` NDJSON endpoint | 500-10,000 docs per bulk request |
| **Cassandra / Scylla** | `cdrs-tokio` or `scylla` | Async CQL driver with `BatchStatement` | Token-aware load balancing across cluster nodes |
| **DynamoDB** | `aws-sdk-dynamodb` | `BatchWriteItem` API (25 items/request max) | Must chunk writes into 25-item batches; parallelise with `join_all` |
| **Oracle** | `oracle` (via C OCI binding) | C FFI binding to Oracle OCI libraries | Requires Oracle Instant Client on host machine |

---

### 2.3. Object Storage Sinks ✅ TIER 1 (Native)

| Sink | Rust Crate | Implementation |
|---|---|---|
| **Amazon S3** | `aws-sdk-s3` or `object_store` | Multipart upload of Parquet/NDJSON files; S3 native partition keys (year/month/day) |
| **Google Cloud Storage** | `google-cloud-storage` or `object_store` | Resumable upload for large batches |
| **Azure Blob Storage** | `azure_storage_blobs` or `object_store` | Block blob upload with SAS token or Managed Identity auth |

**Key Advantage:** Rust's `object_store` crate abstracts all three cloud object storage providers behind a single unified API. A `Box<dyn ObjectStore>` can seamlessly switch from S3 to GCS to Azure Blob without any connector code changes.

---

### 2.4. Message Broker Sinks ✅ TIER 1 (Native)

| Sink | Rust Crate | Notes |
|---|---|---|
| **Apache Kafka** | `rdkafka` (`FutureProducer`) | Async produce with `DeliveryFuture`; supports transactions |
| **Amazon Kinesis** | `aws-sdk-kinesis` | `PutRecords` API; 500 records/request max |
| **Apache Pulsar** | `pulsar` | Native async producer with schema |
| **Redpanda** | `rdkafka` | Identical to Kafka |
| **NATS JetStream** | `async-nats` | Publish to subjects; JetStream persistence |
| **Amazon SQS** | `aws-sdk-sqs` | `SendMessageBatch` (10 messages max/batch) |
| **RabbitMQ** | `lapin` | AMQP 0-9-1 async producer |
| **Google Pub/Sub** | `google-cloud-pubsub` | Async publish with ordering keys |
| **MQTT** | `rumqttc` | QoS-level publish; retain flag support |

---

### 2.5. Vector / AI Database Sinks ⚠️ TIER 2-3 (REST API Required)

AI/Vector stores are modern destinations where the Rust native ecosystem is **nascent**. These require custom `reqwest`-based REST clients.

| Sink | Rust Integration | Custom Work Required |
|---|---|---|
| **Pinecone** | `reqwest` + Pinecone REST API | Medium: Build upsert batch client; manage index namespaces |
| **Weaviate** | `reqwest` + Weaviate REST/GraphQL | Medium: Batch object upload via `/v1/batch/objects` |
| **Qdrant** | `qdrant-client` (official Rust crate) | ✅ Low: Official async Rust gRPC client exists |
| **Milvus** | `milvus` (community) | Medium: Community Rust SDK available but incomplete |
| **Chroma** | `reqwest` + HTTP API | High: No Rust SDK; build REST wrapper |
| **Elasticsearch (vector)** | `elasticsearch` crate | Low: Use `knn` query with `dense_vector` mappings |

---

### 2.6. Specialty & Miscellaneous Sinks

| Sink | Crate / Strategy | Effort |
|---|---|---|
| **SQLite** | `sqlx` (SQLite feature) | Tier 1: Pure Rust SQLite support via `sqlx` |
| **DuckDB** | `duckdb-rs` | Tier 1: Embedded analytics; direct Parquet append |
| **SFTP/JSON** | `russh` + `openssh` | Tier 2: SSH channel streaming |
| **CSV/Local Files** | Rust `std::fs` + `csv` crate | Tier 1: Trivial buffered file writes |
| **Firestore** | `firestore-rs` or `reqwest` | Tier 2: Batch document writes via REST |
| **HubSpot (Sink)** | `reqwest` + HubSpot REST API | Tier 3: Rate-limited; complex batch upsert semantics |
| **Google Sheets** | `google-cloud-*` APIs | Tier 3: `batchUpdate` REST via `reqwest`; low throughput |

---

## Part 3: Integration Development Effort Summary

### What Is Available Out of the Box (Tier 1 — Zero Custom Dev)

The Rust ecosystem natively provides production-grade support for:

- **All message broker sources and sinks:** Kafka, Kinesis, Pulsar, NATS, SQS, Pub/Sub, RabbitMQ, Redpanda
- **All major operational DB sinks:** PostgreSQL (`sqlx`), MySQL (`sqlx`/`mysql_async`), MongoDB (`mongodb`), Redis (`redis`), Elasticsearch (`elasticsearch`)
- **All major object storage:** S3, GCS, Azure Blob (via `object_store` unified API)
- **Delta Lake and Apache Iceberg:** Pure `delta-rs` and `iceberg` Rust libraries
- **BigQuery:** Community `google-cloud-bigquery` crate for streaming inserts
- **ClickHouse Sink:** Native `clickhouse-rs` driver

### What Requires Custom Adapter Development (Tier 2)

- **Database CDC WAL parsing** for Postgres, MySQL, SQL Server (connectivity crate exists; protocol parsing must be implemented)
- **Columnar staging pipeline** for Snowflake, Databricks, Redshift (Parquet write → S3 stage → COPY trigger)
- **500+ SaaS REST sources** — a shared generic SaaS extractor framework covers 80% of the code; per-connector adapters specify URL, pagination, and auth config
- **MongoDB CDC** resume token checkpointing to S3

### What Must Be Fully Custom-Built (Tier 3)

- **Oracle DB** source/sink (requires embedded C OCI library; no pure Rust driver)
- **Teradata** source/sink (ODBC C driver only)
- **SAP S/4HANA** (complex OData v4 protocol; no Rust SDK)
- **Vector stores** without official Rust clients (Chroma, Milvus, etc.)
- **Google Sheets Sink** (very low throughput; API extremely rate-limited)
- **Long-tail SaaS connectors (400+)** — recommended strategy is to implement these as user-uploadable **WebAssembly (WASM) plugins via `extism`** instead of native Rust, offloading maintenance burden to users/partners

---

## Part 4: Recommended Development Phasing

| Phase | Focus | Connectors Delivered |
|---|---|---|
| **Phase 1 (Core)** | All Tier 1 sources and sinks | Kafka, Kinesis, Pulsar, SQS, NATS + Postgres, MySQL, MongoDB, Redis, Elasticsearch, S3, GCS, Azure Blob, ClickHouse |
| **Phase 2 (Warehouse)** | Staged analytics sinks | Snowflake, Databricks (Delta-RS), BigQuery, Redshift, Synapse  |
| **Phase 3 (CDC)** | Database CDC sources | PostgreSQL WAL, MySQL Binlog, MongoDB Change Streams, SQL Server CDC |
| **Phase 4 (SaaS Top-20)** | Highest demand SaaS sources | Salesforce, HubSpot, Stripe, Shopify, Google Analytics, Zendesk, Jira, GitHub |
| **Phase 5 (WASM Platform)** | BYOC plugin framework | Expose `extism` WASM plugin host; publish SDK for community connector development for 400+ long-tail SaaS |
