# Architectural Blueprint: Universal Rust Data Pipeline Engine

## Executive Summary
This architectural document outlines the foundational blueprints for designing a hyper-scalable, fault-tolerant, universal data movement engine entirely in **Rust**. Operating as the intermediary pulling from 600+ heterogeneous sources to 100+ destinations.

**Cost-Optimization & Autonomy Directive:** To strictly minimize operational overhead, infrastructure bloat, and manual deployment cycles, this architecture dictates an **"Object Storage First"** and **"API-Driven Autonomy"** philosophy. The engine relies purely on cheap Object Storage (AWS S3, Google Cloud Storage, Cloudflare R2) for *everything*—from pipeline state and DLQ, down to the actual core applicational metadata and configurations. There is absolutely zero dependency on proprietary RDBMS databases.

---

## 1. Connectivity, Integration & Pluggable Connectors (BYOC)
**Objective:** Standardize extraction from hundreds of heterogeneous sources while specifically empowering users to upload pure custom code for bizarre, proprietary, or highly secured corporate APIs.

- **Source Traits (Native):** Baseline native sources implement a standardized async Rust trait (e.g., `#[async_trait] pub trait SourceReader`).
- **Bring Your Own Connector (BYOC) via WebAssembly Plugins:** 
  - Often, companies wrap third-party sources behind their own internal gateways, demanding proprietary HTTP headers, asymmetric cryptographic handshakes, or dynamic IP-routing before data can be fetched.
  - The Rust engine solves this by natively integrating **`Extism`** (a Universal WebAssembly framework) at the very edge. Users can write their entire extraction loop and proprietary connection wrapper in Go, Python, C++, or Rust, compile it to a `.wasm` file, and upload it to the engine.
  - The Rust application instantiates the user's Wasm plugin safely, hands it secure socket access, and allows the custom code to handle the bizarre connectivity layer before yielding rows back into the main Rust pipeline. Because it runs in WebAssembly, it remains infinitely extensible without risking the `tokio` runtime panicking due to bad user code.
- **Connection Pools:** `deadpool` manages connection bounds to prevent socket starvation across all native and WASM-driven plugins.

## 2. Dynamic Autonomous Onboarding (Zero-Touch Provisioning)
**Objective:** The engine must be highly autonomous, allowing operators to spin up new pipelines instantly without redeploying code.
- **REST Control Plane (Axum):** The Rust application spawns an internal HTTP server using `axum`. Users can onboard a new pipeline entirely in real-time by posting a JSON configuration payload to `/api/v1/pipelines/spawn`.
- **State-free Instant Spawning:** Upon receiving the POST, the Tokio runtime dynamically spins up a detached asynchronous worker (`tokio::spawn`) running the requested loop immediately in the background.

## 3. Application Metadata (Object-Storage Native)
**Objective:** Keep the application portable by eliminating database dependencies.
- **Pipeline Configurations on Blob:** The Rust engine serializes the configuration as JSON and stores it directly into a designated Object Storage prefix.
- **Disaster Recovery:** Upon startup, the Rust app simply calls `object_store::list("s3://.../active-configs/")`, streams all JSON definitions, and triggers the `tokio::spawn` loops for every pipeline found.

## 4. Serialization, Deserialization & Custom Parsing
**Objective:** Instantaneous conversion spanning network boundaries, while safely catching bizarre unstandardized ingress formats.
- **Native Ingress Deserialization:** Incoming standard data is mapped via `serde_json` or `apache-avro`.
- **Custom Deserialization Wrappers (Programmable Parsers):** 
  - If a vendor sends a deeply nested, XML-wrapped JSON payload or a highly obscure binary format (e.g., FIX protocol for finance), standard deserializers will fail.
  - The architecture allows users to supply a **Custom Parser UDF (JavaScript/Wasm)** dynamically. Before the payload touches the core memory bus, the engine hands the raw byte blob `Vec<u8>` to the user's embedded script (`deno_core` or `wasmtime`). The script explicitly unpacks, trims, or translates the proprietary format into a clean, flat JSON/Record object, which the Rust engine then safely accepts.
- **The Engine Core Layer:** Incoming successfully mapped data resides in memory using **Apache Arrow** arrays. Conversion to Memory-mapped Arrow yields 10x-50x processing speeds.
- **Egress Serialization:** Rust natively serializes Arrow vectors directly into highly compressed **Apache Parquet** files.

---

## 5. Transformation, Filtering, & Custom Code (UDFs)
**Objective:** Empower users to inject their own highly custom logic (filtering, proprietary encryption, complex data-masking) mid-flight cleanly.

- **Option A: WebAssembly (Wasm) Sandboxing (Highest Performance)**
  - Users compile their custom transformation logic into a lightweight `.wasm` binary. The engine executes this ultra-fast natively on Arrow memory buffers. Even if the user code hits an infinite loop, it traps securely inside the Wasm runtime, preventing the main application from crashing.
- **Option B: Embedded JavaScript & TypeScript (Best Usability)**
  - For rapid iteration, users submit JS/TS snippets. The Rust engine embeds `deno_core` to execute these functions against incoming JSON payloads blazingly fast while constraining their memory footprint.
- **Option C: Native Script Plugins (Rhai / Lua)**
  - For lightweight logic, `Rhai` abstracts dynamic filtering securely per-row inside a `rayon` pool.

---

## 6. High-Fidelity Metrics & Observability
**Objective:** Aggressive, transparent measurement without costly APM dependencies.
- **Captured Telemetry Metrics:** Every pipeline persistently tracks: `records_consumed`, `records_validated`, `records_transformed`, `records_dlq`, and `records_sunk`. 
- **Metric Archiving on Blob:** A background `tokio` task flushes these counters every 60 seconds into `s3://telemetry-metrics/job_x/.../metrics.parquet`.

## 7. Reliability & Data Quality
**Objective:** Zero dataloss and verifiable structural integrity.
- **Object Storage State Checkpointing:** The Rust workers flush atomic cursor files directly into an S3/R2 bucket, enforcing atomicity on "last_read_id".
- **DLQ (Dead Letter Queues) as Data Lakes:** Failed rows logically dump into explicitly partitioned folders (`s3://dlq/job_123/...`).

## 8. Data Governance, Tagging, & Metadata
**Objective:** Decentralized, dataset cataloging maintained persistently on Object Storage.
- **Dataset-Level Governance (Sidecar Manifests):** Alongside every dataset directory, the Rust engine updates a `.governance.json` sidecar holding SLAs and governance tiers. 

---
### Architect's Note
By extending the WebAssembly (`wasmtime` and `Extism`) fabric from merely mid-flight transformations straight to the very **Edge (Ingress Connectors and Deserialization Parsers)**, the Rust Engine becomes infinitely adaptable. Enterprises are no longer blocked waiting for the core engineering team to build a native integration for a proprietary API or obscure binary wrapper. The end-users simply write their own connector scripts, compile them, and hook them securely into the ultra-fast, database-free data planes.
