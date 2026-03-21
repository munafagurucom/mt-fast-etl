# Rust Integration Guide: Elasticsearch

## Overview
Elasticsearch (and OpenSearch) are leading search and analytics engines built on inverted indices. Writing to Elasticsearch from Rust is a common pattern for capturing highly concurrent logs, telemetry, and dense document structures.

## APIs & Methods
Elasticsearch support in Rust revolves around the official `elasticsearch` crate (Elastic NV) or `opensearch` crate (AWS).

### 1. `elasticsearch` (Official Crate)
- **Crate:** `elasticsearch`, `serde_json`
- **Method:** Uses a structured synchronous or asynchronous HTTP client wrapper.
- **Example Usage:**
  ```rust
  use elasticsearch::{Elasticsearch, IndexParts};
  use serde_json::json;

  async fn sink_document(client: &Elasticsearch, index: &str, id: &str) -> Result<(), elasticsearch::Error> {
      let body = json!({
          "event": "login",
          "user_id": 12345,
          "timestamp": "2023-11-20T14:00:00Z"
      });

      let response = client
          .index(IndexParts::IndexId(index, id))
          .body(body)
          .send()
          .await?;
          
      Ok(())
  }
  ```

### 2. Bulk/Streaming API
- **Focus:** The most critical API for high-volume sinks. Instead of indexing individual objects, the Bulk API takes NDJSON arrays.

## Configurations
- **Connection Pools / Transport:** Uses the `TransportBuilder` internally mapped to `reqwest` connection pools. You can enforce single-node connections or Multi-Node Sniffing pools natively.
- **Authentication:** Standard Basic Auth `Credentials::Basic("elastic", "pass")` or API and Bearer Tokens configured at initialization.

## Features & Transformations
- **Bulk API Endpoint:** For logs and telemetry, Rust constructs local NDJSON multiline strings manually or using `serde_json` formatters, and posts the chunk (`client.bulk()`), efficiently writing tens of thousands of documents inside milliseconds.
- **Data Streams & ILM:** Directly integrate Elasticsearch Data Streams (e.g., `logs-my_app-default`).

## Pros & Cons
### Pros
- **Highly Concurrent:** Built heavily on `reqwest` and `hyper`, the Rust ES client scales perfectly within Tokio, firing massive HTTP streams.
- **Serde Support:** Any complex nested Rust `struct` derives directly into Elasticsearch dynamic JSON mapping objects immediately.

### Cons
- **Verbose Bulk Code:** Constructing the NDJSON header/body syntax cleanly in Rust for the `_bulk` endpoint can be tedious, risking syntax parsing HTTP 400s if poorly constructed.
- **Memory Overhead:** Holding 10,000 deep nested structs in memory to `serde_json::to_string` them concurrently will balloon RAM usage inside the Rust process suddenly without capacity restrictions.

## Issues & Workarounds
- **Issue: High Connection Rejections (429s).**
  - *Workaround:* Elasticsearch will reject requests actively if the indexing queue is full. You must implement exponential backoff explicitly in Rust (or rely on `reqwest-retry`) locally using `tokio::time::sleep`. Do not panic out of the app.
- **Issue: NDJSON Syntax errors in Bulk endpoints.**
  - *Workaround:* Do not try to append `\n` manually without extreme testing. Ensure strict adherence: `{"index": { "_index": "test", "_id": "1" }}\n{"field": "value"}\n`.

## Recommendations & Suggestions
1. **Never Singly Index Data:** If you are sinking data for analytics or logs, ALWAYS aggregate into groups of 500-5000 and send via `client.bulk()`.
2. **Sniffing Node Pools:** If connecting to an Elasticsearch cluster (not local/managed cloud), utilize the client `Sniff` features to dynamically discover the topology of the cluster nodes inside Rust to distribute load balancing, averting coordinator-node bottlenecks.
3. **Elasticsearch vs. OpenSearch Crate:** Verify the destination cluster version. If it is AWS OpenSearch >= 1.0, use the `opensearch` crate instead of the `elasticsearch` crate due to arbitrary compatibility version locking intentionally enforced by Elastic.
