# Deep Research Analysis Part 1: Core Runtime, Async Foundation, and Build System

## 1. Async Runtime: Tokio

| Attribute | Detail |
|---|---|
| **Crate** | `tokio` |
| **Latest Version** | ~1.50.0 (1.x LTS series) |
| **Maturity** | Production-grade. Powers Cloudflare, Discord, AWS Lambda runtime for Rust. |
| **License** | MIT |
| **Key Features** | Multi-threaded work-stealing scheduler, async TCP/UDP/file I/O, timers, channels (`mpsc`, `broadcast`, `watch`), synchronization primitives (`Mutex`, `RwLock`, `Semaphore`). |
| **C Dependencies** | None; pure Rust. |
| **Risk** | Extremely low. Tokio is the de facto standard for async Rust. Inaugural TokioConf scheduled for April 2026. |

**Required Feature Flags for this Application:**
```toml
tokio = { version = "1", features = ["full"] }
# "full" enables: rt-multi-thread, io-util, net, time, sync, signal, macros, fs
```

**Pipeline Usage:** Every source/sink connector runs as a detached `tokio::spawn` task. Bounded `mpsc` channels connect pipeline stages, providing native backpressure. CPU-intensive work (Parquet compression, encryption) must be offloaded to `tokio::task::spawn_blocking` to prevent executor starvation.

---

## 2. HTTP Client: Reqwest + Tower

| Attribute | Detail |
|---|---|
| **Crate** | `reqwest` |
| **Latest Version** | 0.12.x |
| **Purpose** | All REST API integrations (550+ SaaS sources, Snowflake SQL REST API, Databricks Jobs API, PagerDuty Events API) |
| **TLS Backend** | `rustls` (pure Rust, recommended) or `native-tls` (OpenSSL C binding) |
| **Connection Pooling** | Built-in via `hyper` connection pool; configurable `pool_max_idle_per_host` |

| Attribute | Detail |
|---|---|
| **Crate** | `tower` |
| **Latest Version** | 0.5.x |
| **Purpose** | Middleware stack wrapping `reqwest` for retry, rate-limit, timeout, and circuit-breaker logic |
| **Key Layers** | `tower::retry::Retry`, `tower::limit::RateLimit`, `tower::timeout::Timeout` |

**Pipeline Usage:** Every SaaS source connector wraps `reqwest::Client` inside a `tower::ServiceBuilder` stack. Rate limiting uses the Token Bucket algorithm via `tower::limit::RateLimitLayer`, preventing HTTP 429 violations.

---

## 3. Serialization Foundation: Serde + JSON/Avro

| Crate | Version | Purpose |
|---|---|---|
| `serde` | 1.x | Universal `Serialize`/`Deserialize` derive macros |
| `serde_json` | 1.x | JSON parsing for all REST API payloads, pipeline configs, DLQ wrappers |
| `serde_yaml` | 0.9.x | YAML pipeline configuration file parsing |
| `apache-avro` | 0.17.x | Avro schema-driven deserialization for Kafka/Confluent Schema Registry payloads |

**All four are pure Rust with zero C dependencies.**

---

## 4. Columnar Data Engine: Apache Arrow + Parquet

| Attribute | Detail |
|---|---|
| **Crates** | `arrow` + `parquet` (from the `apache/arrow-rs` repository) |
| **Latest Version** | 58.0.0 (released February 2026) |
| **Maturity** | Official Apache project. Monthly release cadence. v57.0.0 introduced 4x faster Parquet metadata parsing and Parquet Variant type support. |
| **C Dependencies** | None; pure Rust implementation. |

**Pipeline Usage:** The columnar engine is the heart of the data warehouse sink strategy:
1. Source connector emits rows as `Vec<serde_json::Value>`.
2. The `transform/` module converts to a strongly-typed `arrow::record_batch::RecordBatch`.
3. The warehouse sink serializes the `RecordBatch` into a compressed Parquet file using `parquet::arrow::ArrowWriter`.
4. The Parquet chunk is uploaded to S3/GCS staging and loaded into Snowflake/BigQuery/Redshift via `COPY INTO`.

---

## 5. Object Storage (Unified): object_store

| Attribute | Detail |
|---|---|
| **Crate** | `object_store` |
| **Latest Version** | 0.11.x (part of `apache/arrow-rs` ecosystem) |
| **Maturity** | Official Apache crate; used internally by `deltalake-rs` and `iceberg-rust`. |
| **Supported Backends** | AWS S3, Google Cloud Storage, Azure Blob/ADLS Gen2, Local Filesystem, HTTP |
| **C Dependencies** | None; pure Rust. |

**Pipeline Usage:** Provides a single `Box<dyn ObjectStore>` abstraction used for:
- State checkpointing (`cursor.json`)
- Dead Letter Queue writes (partitioned by pipeline/error-type/date)
- Parquet staging for warehouse sinks
- Pipeline configuration loading on startup

---

## 6. Control Plane Web Framework: Axum

| Attribute | Detail |
|---|---|
| **Crate** | `axum` |
| **Latest Version** | 0.8.x |
| **Maturity** | Maintained by the Tokio team. Production-grade. |
| **C Dependencies** | None. |

**Pipeline Usage:** Powers two internal HTTP servers:
1. **Port 8080 — Control API:** `POST /api/v1/pipelines` (spawn), `GET /api/v1/pipelines` (list), `PATCH` (pause), `DELETE` (stop).
2. **Port 9090 — Metrics:** Prometheus-compatible `/metrics` endpoint for Grafana scraping.

---

## 7. WebAssembly Plugin Runtime: Extism

| Attribute | Detail |
|---|---|
| **Crate** | `extism` |
| **Latest Version** | 1.13.0 (November 2025) |
| **Maturity** | Production-ready. Built atop `wasmtime`. Actively promoted for plugin architectures. |
| **Guest SDK Languages** | Rust, Go, Python, JavaScript/TypeScript, C, C++, Haskell, Zig |
| **C Dependencies** | None for the Rust host SDK. |

**Pipeline Usage:** Powers the Bring Your Own Connector (BYOC) capability:
- Users compile custom source/sink logic into `.wasm` binaries.
- The host Rust engine loads the plugin via `extism::Plugin::new()`.
- Data flows through the WASM guest via shared linear memory.
- If the plugin panics or infinite-loops, the WASM sandbox traps safely without affecting the host `tokio` runtime.

---

## 8. Rate Limiting: Governor

| Attribute | Detail |
|---|---|
| **Crate** | `governor` |
| **Latest Version** | 0.10.4 (December 2025) |
| **Algorithm** | Generic Cell Rate Algorithm (GCRA) — Token Bucket variant |
| **C Dependencies** | None. |

**Pipeline Usage:** Per PRD requirement #13 (TPS/Rate Limiting):
- Each `SourceConnector` wraps its poll loop inside a `governor::RateLimiter`.
- Config `max_source_tps = 500` translates to a GCRA quota of 500 cells/second.
- Each `SinkConnector` independently limits its write throughput to protect destination databases.

---

## 9. Secrets Management SDKs

| Cloud | Crate | Version | Usage |
|---|---|---|---|
| **AWS** | `aws-sdk-secretsmanager` | Latest AWS SDK for Rust | Fetch DB passwords, API keys at runtime via IAM Role |
| **Azure** | `azure_security_keyvault` | 0.20.x | Fetch secrets via Managed Identity |
| **GCP** | `google-cloud-secretmanager` | Community crate | Fetch secrets via Service Account |
| **HashiCorp Vault** | `hashicorp_vault` / `vaultrs` | 0.7.x | HTTP API client for self-hosted Vault instances |
| **Environment Vars** | `dotenvy` | 0.15.x | Load `.env` files at startup |

---

## 10. Observability Stack

| Crate | Purpose | Version |
|---|---|---|
| `tracing` | Structured event instrumentation (spans, events) | 0.1.x |
| `tracing-subscriber` | JSON/stdout log formatting, log-level filtering | 0.3.x |
| `metrics` | Prometheus-style counter/gauge/histogram registration | 0.24.x |
| `metrics-exporter-prometheus` | Expose `/metrics` HTTP endpoint for Grafana | 0.16.x |

---

## 11. Build System and C FFI Dependencies Summary

| Dependency | Requires System C Library? | Affected Crates | Docker Build Impact |
|---|---|---|---|
| **librdkafka** | ✅ Yes | `rdkafka` (Kafka source/sink) | Must install `librdkafka-dev` or use `rdkafka/cmake` feature to build from source |
| **OpenSSL** | ⚠️ Optional | `reqwest` (if using `native-tls`) | Avoided entirely by using `rustls` TLS backend |
| **Oracle OCI** | ✅ Yes | `oracle` (Oracle DB source/sink) | Must install Oracle Instant Client `.so` libraries |
| **unixODBC** | ✅ Yes | `odbc-api` (Snowflake, Teradata ODBC sinks) | Must install `unixodbc-dev` + vendor ODBC driver |
| **Everything else** | ❌ No | All other 40+ crates | Pure Rust; zero system deps |

**Recommendation:** Use `rustls` everywhere to eliminate OpenSSL. Use `rdkafka` with the `cmake` feature flag to compile `librdkafka` from source during `cargo build`, eliminating the system-level dependency for Docker images.
