# Rust Integration Guide: Databricks

## Overview
Databricks orchestrates massive Apache Spark workloads natively atop Delta Lake architectures. Sinking data to Databricks from a Rust application utilizes the standard REST API for data loading or bridging direct file interactions locally to cloud storage (ADLS/S3) which Spark consumes immediately.

## APIs & Methods
Unlike regular databases, you do not write "inserts" to Databricks actively per row. You integrate via APIs mapping massive files or JDBC connectivity.

### 1. `reqwest` (Databricks Jobs / SQL API)
- **Crate:** `reqwest`
- **Focus:** Instructs Databricks clusters to read externally staged data (e.g., S3/ADLS) using REST commands.
- **Example Usage (SQL API / Warehouse):**
  ```rust
  use reqwest::header::{AUTHORIZATION, HeaderMap, HeaderValue};
  use serde_json::json;

  async fn query_databricks_sql(host: &str, token: &str, warehouse_id: &str, sql: &str) -> reqwest::Result<()> {
      let url = format!("https://{}/api/2.0/sql/statements", host);
      let mut headers = HeaderMap::new();
      headers.insert(AUTHORIZATION, HeaderValue::from_str(&format!("Bearer {}", token)).unwrap());
      
      let client = reqwest::Client::new();
      let _res = client.post(&url)
          .headers(headers)
          .json(&json!({"warehouse_id": warehouse_id, "statement": sql}))
          .send()
          .await?;
      Ok(())
  }
  ```

### 2. ODBC/JDBC (Via Arrow/ODBC)
- Use standard `odbc-api` crates bound to the proprietary Simba Databricks C++ driver, enabling `INSERT INTO` semantics.

### 3. S3/ADLS Staging + Delta integration
- Ensure the Rust app writes massive Parquet files using the `parquet` API straight to the target Cloud Storage directory mapping to an external Databricks Delta table natively.

## Configurations
- **Databricks SQL Warehouse/Endpoint:** You require the specific `warehouse_id` and the `host` DNS routing URL provided by your Databricks workspace.
- **Tokens:** Always authenticate standard requests with Databricks Personal Access Tokens (`PAT`) or OAuth M2M tokens generated within your Active Directory (Azure) or AWS IAM scope.

## Features & Transformations
- **Delta Lake Sinking:** If you embed the `deltalake-rs` (Delta-rs library) within the Rust app, Rust can natively apply transformations locally, append raw Delta Logs and Parquet files inside Databricks tables on S3/ADLS, entirely bypassing Databricks compute billing.
- **Autoloader Integration:** Rust places JSON/Parquet files sequentially in S3. Databricks Autoloader inherently streams the results instantaneously into raw Delta Tables continuously.

## Pros & Cons
### Pros
- **Delta Native Integration:** Native `delta-rs` bindings map directly into Databricks file formats without Spark cluster uptime.
- **Highly Separated Compute/Storage:** By sinking to target ADLS/S3 URLs natively managed by Databricks, the Rust microservice doesn't block awaiting heavy Analytics endpoints to process ingestion.

### Cons
- **ODBC Fragility:** Compiling, distributing, and relying on C Databricks ODBC wrappers for straightforward application interactions in Rust is highly unreliable outside standard Linux configurations.
- **Latency Overheads:** SQL endpoint latency makes singleton row insertions extremely unresponsive.

## Issues & Workarounds
- **Issue: Inability to execute real-time single `INSERT` writes efficiently.**
  - *Workaround:* Never do this. If you must use direct APIs, generate batches locally, use the Databricks SQL API, and submit one massive stringified insert string, or switch entirely to local `delta-rs` bindings which write Parquet logs natively in millisecond scales natively from Rust.
- **Issue: Databricks Jobs failing mysteriously from Rust inputs.**
  - *Workaround:* Due to eventual consistency (S3 instances) vs. Job queueing, ensure your Rust app implements proper delay (or guarantees consistency) between Object Storage `put_object` successes and Databricks SQL API API `COPY` commands.

## Recommendations & Suggestions
1. **Delta-RS:** Thoroughly investigate utilizing the [delta-rs](https://github.com/delta-io/delta-rs) crate instead of JDBC. It lets you write to Databricks-supported tables from your Rust app asynchronously at unmatched speeds.
2. **Databricks REST API vs. Webhooks:** Trigger jobs dynamically via reqwest whenever your microservice completes staging a batch of data upstream in Cloud Storage.
3. **Partitioning Native Types:** Expose `Date` and `UUID` strings cleanly mapped to strings in NDJSON or Parquet logic from Rust so Databricks Infers schema types perfectly (eliminating `StringType` mapping bottlenecks inside Spark).
