# Source Extraction Guide: Google Cloud Storage

## 1. Extraction Strategy
Extracting data continuously from **Google Cloud Storage** inside a Rust application requires resilient architecture. 
**Strategy Strategy:** Monitor the storage prefix periodically for new files (or integrate an Event Notification via SQS/PubSub). Download the file into `parquet` or byte buffers, and process line-by-line continuously.

## 2. APIs, SDKs, and Methods
- **Rust Crates/SDKs:** `google-cloud-storage`
- **Primary API:** GCS JSON API
- **Methodology:** Batch object retrieval (`GetObject`) followed by streaming decode (`parquet::StreamReader`).

## 3. API & Method Signatures
A common, robust architectural signature for continuous extraction looks like this:
```rust
// Expected Method Signature
`async fn stream_new_objects(bucket: &str, prefix: &str) -> Vec<ByteStream>`
```
*Note: This relies entirely on `tokio` asynchronous runtimes to ensure non-blocking network streams.*

## 4. Authentication & Authorization
- **Auth Method:** Depends on specific implementation (e.g., Bearer Tokens, OAuth2 Client Credentials, X509 Certs, or IAM/SASL).
- **Security:** Ensure credentials map dynamically via environment variables (`aws-config` / `dotenvy`) rather than hardcoded strings. If using a SaaS API, OAuth2 refresh token flows must be resiliently handled in the REST wrapper caching layers.

## 5. Output Formats
Data arrives from Google Cloud Storage usually in:
- **Format:** `JSON`, `Avro`, `Parquet`, or Binary Packets.
- **Deserialization:** You must utilize `serde_json` or `apache-avro` to map the incoming unstructured payload strongly to Rust `structs` immediately at the network boundary.

## 6. Restrictions & Limitations
- **Limitations:** Data pagination limits, active connections limits per process.
- **Issues:**
  - Fetching objects that are partially uploaded, or missing events in heavily congested prefixes (S3 ListObjects limits).
  - Network timeouts breaking the standing TCP connections dynamically.

## 7. Recommendations & Suggestions
### For Building a Continuous Pull/Stream Application:
1. **Never Panic:** If the network drops or Google Cloud Storage denies authorization, implement an exponential backoff retry loop (`reqwest-retry` or native loop). A panic brings down the whole continuous streamer pipeline.
2. **Backpressure:** If your Rust application processes the Google Cloud Storage data slower than it arrives, you will face OOM (Out Of Memory) exceptions if you dump it into uncontrolled `mpsc` Tokio channels. Always use bounded channels (e.g., `mpsc::channel(1000)`).
3. **Offset / Cursor State Management:** Never rely on keeping track of the "last read ID" entirely in RAM. You must externalize state (to a Postgres DB, Redis, or local SQLite) so if the Rust application restarts, it picks up precisely where it left off, avoiding duplicate data ingestions.
