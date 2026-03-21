# Rust Integration Guide: Cassandra / ScyllaDB

## Overview
Apache Cassandra (and strongly ScyllaDB, a C++ rewrite) are ultra-scalable wide-column NoSQL databases designed for massive high-availability throughput. Using Rust to act as a data sink into Cassandra clusters requires specific understanding of the CQL protocol.

## APIs & Methods
Rust benefits immensely from the `scylla-rust-driver`, officially developed by the ScyllaDB team, but 100% interoperable with standard Cassandra clusters.

### 1. `scylla` (Asynchronous Driver)
- **Crate:** `scylla`
- **Target:** Apache Cassandra 3.0+, 4.0+, and ScyllaDB.
- **Focus:** Implements the CQL binary protocol explicitly over Tokio, natively async and partition-aware.
- **Example Usage:**
  ```rust
  use scylla::{Session, SessionBuilder};

  async fn insert_telemetry(session: &Session, device_id: &str, temp: f32) -> Result<(), scylla::transport::errors::QueryError> {
      session
          .query(
              "INSERT INTO raw_data.telemetry (device_id, temp, timestamp) VALUES (?, ?, toTimestamp(now()))",
              (device_id, temp),
          )
          .await?;
      Ok(())
  }
  ```

### 2. `cdrs-tokio`
- **Method:** Alternative legacy driver native to pure Rust, but generally superseded by the official `scylla` crate in performance and active development.

## Configurations
- **Contact Points:** The builder (`SessionBuilder::new().known_node("127.0.0.1:9042")`) uses one node to gather the entire cluster topology, routing upcoming queries directly to the correct replica automatically (Token-Aware Routing).
- **Execution Profiles:** Define specific timeouts and retry configurations universally within the Session.
- **Consistency Levels:** Cassandra sinks require configuring the Consistency `scylla::statement::Consistency::Quorum` directly inside the Rust query logic to dictate write reliability.

## Features & Transformations
- **Prepared Statements:** The single most important feature. Rust compiles the statement using `session.prepare(...)` locally. It pushes a hashed ID over the wire instead of formatting CQL dynamically, boosting write speed 300%.
- **Batch Queries:** Cassandra natively supports `BEGIN BATCH ... APPLY BATCH`. Rust handles this seamlessly using the `scylla::batch::Batch` syntax perfectly handling unlogged updates or counters.
- **Tracing:** Enable Rust Native Tracing to inspect raw binary bytes over the wire.

## Pros & Cons
### Pros
- **Linearly Scalable Writes:** The driver natively hashes the partition keys dynamically inside Rust, maintaining minimal cluster hop routing logic.
- **Non-blocking Operations:** Sending 100,000 asynchronous writes does not bog down Tokio runtimes because `scylla` multiplexes thousands of binary stream requests over exactly one TCP multiplexed node connection.

### Cons
- **Schema Complexities:** Unlike Postgres mapping, there is no native structural ORM handling UDTs (User Defined Types) elegantly without manual struct implementations.
- **LWT Overhead:** Emitting Lightweight Transactions (IF NOT EXISTS) via Rust brutally fragments the cluster's Paxos algorithm. You must treat it explicitly as an append-only time-series sink.

## Issues & Workarounds
- **Issue: Cassandra Cluster Overload (Too many concurrent writes).**
  - *Workaround:* Due to Rust's insane concurrency inside `join_all!`, you can routinely send 2,000,000 active requests simultaneously, bringing a poorly configured JVM Cassandra node to its knees due to garbage collection starvation. Limit concurrent Futures strictly using a `tokio::sync::Semaphore` set to a safe amount (e.g., 5,000).
- **Issue: Prepared Statement Caching.**
  - *Workaround:* The `Session` caches prepared statements internally, but you must still execute `.prepare()` consistently across logical boundaries or store the `PreparedStatement` globally via `Arc`.

## Recommendations & Suggestions
1. **Always use Prepared Statements:** For writing massive amounts of dynamic row IDs inside an application, `.prepare` the insert string strictly once at the app startup phase.
2. **Token-Aware Driver Checks:** Utilize the `scylla` crate over unmaintained implementations. Ensure token-aware routing happens so the Rust node hits the primary replica natively.
3. **Paging:** If attempting to execute reads while syncing data for aggregations, utilize explicit paging `.fetch_size()` configurations within the statement payload.
