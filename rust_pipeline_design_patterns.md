# Software Design Architecture: Rust 1-to-1 Data Pipeline Engine

This document details the exact architectural design patterns utilized to implement the features outlined in the `rust_1to1_pipeline_features.md` specification. By leveraging established software design patterns alongside Rust's strict safety guarantees and async paradigms, the engine achieves a codebase that is highly decoupled, fault-tolerant, and performant.

---

## 1. Abstract Factory & Plugin Pattern: The Connector Layer
**Problem:** The engine must interface with over 600 Sources and 100 Sinks dynamically without bloating a single massive monolithic `main.rs` file.
**Implementation:** 
* **Trait Objects (Interfaces):** The system defines strict Rust Trait boundaries: `#[async_trait] pub trait SourceConnector` and `#[async_trait] pub trait SinkConnector`.
* **Factory Registry:** An Application Factory reads the `pipeline_config.json` from S3. Depending on the `source_type` (e.g., `postgres_cdc`, `salesforce_api`, `wasm_plugin`), the Factory provisions the concrete struct that implements the `SourceConnector` trait.
* **Benefits:** Extreme decoupling. Adding a new `ShopifyAPI` connector or a `Snowflake` sink does not touch the core execution engine code; you simply register the new struct into the Factory.

## 2. Strategy Pattern: Execution Providers (Native vs. WASM)
**Problem:** The engine supports both native Rust integrations and Bring Your Own Connector (BYOC) WebAssembly (`Extism`) plugins securely.
**Implementation:**
* The engine uses the **Strategy Pattern** to decouple the generic pipeline loop from *how* extraction or loading occurs. 
* At runtime, the `PipelineWorker` is handed an `ExtractionStrategy`. If it's a native database connection, it executes the `NativeExtractionStrategy`. If the user provided a proprietary `.wasm` connector file, it executes the `WasmIsolationStrategy`.
* **Benefits:** The core pipeline mechanics (buffering, backpressure, state flushing) remain identical and perfectly tested regardless of whether the user wrote the connector logic natively in Rust or injected Python/Go via WebAssembly.

## 3. Pipes and Filters Pattern (with Tokio Streams)
**Problem:** High-volume data must move from Source to Sink concurrently without exhausting system memory or crashing the process (Out-Of-Memory/OOM).
**Implementation:**
* High-speed `tokio` bounded channels connect the pipeline phases: the Ingestion thread (**Filter 1**), the schema translating Adapter (**Filter 2**), and the Egress/Sink thread (**Filter 3**).
* **Native Backpressure:** Using strictly bounded Rust `mpsc` (Multi-Producer, Single-Consumer) channels entirely solves backpressure natively in memory. If the Sink pipeline slows down (e.g., a Database lock), the channel buffer fills up. This naturally causes the Source thread to hit an `.await` yielding to the executor, completely stopping the data pull from the Source API. Memory usage remains perfectly flat.

## 4. Adapter Pattern: 1-to-1 Extensible Schema Mapping
**Problem:** In 1-to-1 strict mapping, Source A outputs `{"CustID": 123}`, but Sink B strictly requires `{"customer_id": "123"}` for insertion.
**Implementation:**
* Sitting between the `SourceConnector` stream and the `SinkConnector` consumer is a dynamic **Adapter Component**. Using `serde_json::Value` manipulation, the Adapter takes the ingested payload and cleanly applies the declarative 1-to-1 field transformation map configuring by the user.
* **Benefits:** Sinks do not need to understand Source payloads. The Adapter enforces the semantic translation across boundaries explicitly.

## 5. Circuit Breaker & Retry Pattern
**Problem:** External SaaS APIs (like HubSpot or Zendesk) suffer from network instability and impose strict rate limits via HTTP 429 errors.
**Implementation:**
* Wrapping the `reqwest` HTTP clients with the `tower` middleware crate, the engine enforces exponential backoff, jitter, and the **Circuit Breaker Pattern**.
* If an API returns a `429 Too Many Requests`, the circuit specifically opens for that pipeline, parsing the `Retry-After` header. The `tokio` executor aggressively `sleeps` the pipeline, yielding CPU cycles completely back to the other 499 running pipelines on the node.
* **Benefits:** Eliminates cascading node failures and legally honors vendor API rate limits autonomously.

## 6. Object Pool Pattern (Connection Pooling)
**Problem:** Re-authenticating and negotiating SSL/TCP handshakes for every bulk database load into Databricks or Postgres consumes massive compute overhead and latency.
**Implementation:**
* **`deadpool` Integration:** The engine utilizes the **Object Pool Pattern** to maintain long-lived collections of database sockets to Operational sinks. When a Micro-batched Parquet chunk is ready to flush, the Sink thread checks out a connection from the pool, executes the `COPY INTO`, and returns the socket.
* **Benefits:** Slashes latency per batch. More importantly, it strictly bounds the maximum number of concurrent database connections hitting the destination, guaranteeing the Rust engine cannot accidentally execute a Denial of Service (DoS) attack on an operational target database.

## 7. Memento Pattern (Cursor State Checkpointing)
**Problem:** The pipeline relies exclusively on Object Storage (S3/GCS) for state orchestration. It must survive unexpected Kubernetes Pod terminations and resume "Exactly-Once" ingestion without skipping or heavily duplicating data.
**Implementation:**
* The runtime uses a checkpointing mechanism parallel to the **Memento Pattern**. After a micro-batch successfully flushes to a Sink and the transaction commits, the Pipeline creates a "Memento" object representing exactly what was completed (e.g., the last Kafka Offset ID, Postgres LSN, or API Timestamp).
* This JSON Memento is atomically overwritten to S3 (`s3://state/pipeline_id/cursor.json`). 
* If the Rust worker panics or the node terminates, a fresh clone spins up, fetches the exact Memento from S3, and resumes polling precisely where it left off.

## 8. Observer Pattern: Observability and Telemetry
**Problem:** Platform Engineers must monitor 500+ completely detached asynchronous tasks simultaneously in real-time.
**Implementation:**
* **Prometheus Registry:** The system uses the **Observer Pattern** where every pipeline task increments shared atomic counters (`Row Read`, `API Timeout`, `DLQ Flush`) encapsulated in `Arc<AtomicU64>`.
* A dedicated, lightweight `tokio` telemetry thread *observes* these memory-safe atomic references and exposes them via an internal `axum` HTTP REST endpoint (`/metrics`) built exactly for Prometheus scraping and Grafana rendering.
