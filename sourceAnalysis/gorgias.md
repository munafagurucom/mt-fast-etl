# Source Extraction Guide: Gorgias

## 1. Extraction Strategy
Extracting data continuously from **Gorgias** inside a Rust application requires resilient architecture. 
**Strategy Strategy:** Periodic paginated polling via cursors or direct HTTP Webhook listener server.

## 2. APIs, SDKs, and Methods
- **Rust Crates/SDKs:** `reqwest`, `serde_json`, `tokio`
- **Primary API:** HTTP REST API / Webhooks
- **Methodology:** HTTP GET with pagination (Cursor/Offset) or Axum HTTP POST webhook handlers.

## 3. API & Method Signatures
A common, robust architectural signature for continuous extraction looks like this:
```rust
// Expected Method Signature
`async fn fetch_payload(client: &Client, url: &str, cursor: Option<&str>) -> Result<Vec<Value>, Error>`
```
*Note: This relies entirely on `tokio` asynchronous runtimes to ensure non-blocking network streams.*

## 4. Authentication & Authorization
- **Auth Method:** Depends on specific implementation (e.g., Bearer Tokens, OAuth2 Client Credentials, X509 Certs, or IAM/SASL).
- **Security:** Ensure credentials map dynamically via environment variables (`aws-config` / `dotenvy`) rather than hardcoded strings. If using a SaaS API, OAuth2 refresh token flows must be resiliently handled in the REST wrapper caching layers.

## 5. Output Formats
Data arrives from Gorgias usually in:
- **Format:** `JSON`, `Avro`, `Parquet`, or Binary Packets.
- **Deserialization:** You must utilize `serde_json` or `apache-avro` to map the incoming unstructured payload strongly to Rust `structs` immediately at the network boundary.

## 6. Restrictions & Limitations
- **Limitations:** Data pagination limits, active connections limits per process.
- **Issues:**
  - Strict rate limits (429 Too Many Requests), complex OAuth2 rotation, or missing webhook event types.
  - Network timeouts breaking the standing TCP connections dynamically.

## 7. Recommendations & Suggestions
### For Building a Continuous Pull/Stream Application:
1. **Never Panic:** If the network drops or Gorgias denies authorization, implement an exponential backoff retry loop (`reqwest-retry` or native loop). A panic brings down the whole continuous streamer pipeline.
2. **Backpressure:** If your Rust application processes the Gorgias data slower than it arrives, you will face OOM (Out Of Memory) exceptions if you dump it into uncontrolled `mpsc` Tokio channels. Always use bounded channels (e.g., `mpsc::channel(1000)`).
3. **Offset / Cursor State Management:** Never rely on keeping track of the "last read ID" entirely in RAM. You must externalize state (to a Postgres DB, Redis, or local SQLite) so if the Rust application restarts, it picks up precisely where it left off, avoiding duplicate data ingestions.
