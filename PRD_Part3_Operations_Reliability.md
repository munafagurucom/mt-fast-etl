# Rust Data Pipeline PRD: Part 3 - Operations, Reliability, and Control

## 10. Data Quality, Lineage, Reliability, Checkpointing
- **Data Lineage:** Every record passed through the pipeline automatically receives injected lineage headers (e.g., `_pipeline_ingest_timestamp`, `_source_topic`, `_pipeline_id`).
- **Reliability & Checkpointing:** Checkpoints are flushed using the **Memento Pattern**. After a sink acknowledges a confirmed write (e.g., Postgres `COMMIT`), the engine writes the highest processed LSN/Offset to `s3://state/pipeline_id/cursor.json`. If the worker crashes, it resumes from the precise cursor, guaranteeing exactly-once or at-least-once delivery.
- **Observability:** Granular `tracing-subscriber` structured JSON logs are emitted for every micro-batch.
- **Data Catalogue:** Native integrations push schema definitions and schema drifts sequentially into data catalogue representations (e.g., Databricks Unity Catalog or generic Hive Metastore API pushes).

## 11. Alerting Integrations (API, PagerDuty, Custom)
- **Webhook Alerting:** Natively triggers JSON POST webhooks continuously when strict operational thresholds are breached (e.g., `lag_seconds > 300` or `dlq_rate > 5%`).
- **PagerDuty Integration:** Direct PagerDuty Events API v2 integration. The engine sends `trigger`, `acknowledge`, and `resolve` event severities natively over `reqwest` upon fatal Tokio task panics or database connection failures.
- **Custom:** Pluggable `AlertDispatcher` interfaces allow developers to add arbitrary dispatchers (e.g., Slack Webhooks, Microsoft Teams, or custom internal NOC dashboard APIs).

## 12. Feature Flags, Overrides, and Configs
The application is highly driven by dynamic configs:
- `engine.max_memory_buffer_mb`: Prevents OOM by hard-stopping source ingress polling.
- `pipeline.batch_flush_interval_ms`: Micro-batch frequency threshold for Parquet generation to S3.
- `pipeline.enable_dlq`: Boolean flag explicitly turning DLQ routing on/off.
- `security.require_tls`: Enforces strict TLS connections globally for all initialized sink and source connections.
- `engine.concurrency_limit`: Global cap on spawned `tokio` threads to prevent severe CPU thrashing.
- `overrides.schema_drift_mode`: `[Halt, Drop_Field, DLQ]` — Defines the behavioral override when a completely unknown schema mutation is detected mid-stream.

## 13. TPS / Rate Limiting on Source and Sink
The engine provides deterministic throughput controls to rigorously protect external systems:
- **Source Limiting:** Limits the velocity at which `tokio` polls the HTTP source API (e.g., `max_source_tps=500`). Uses the `governor` crate for extreme Token Bucket rate limiting calculations natively in memory.
- **Sink Limiting:** Controls egress connection concurrency. Even if the internal memory buffer is full, it restricts HTTP outputs or Database bulk inserts to `max_sink_tps=3000` or `max_concurrent_connections=10`, entirely mitigating the risk of executing an accidental Denial of Service (DoS) attack on production operational databases.

## 14. Extensibility and Future Proofing
The application architecture fundamentally supports horizontally-scaling stateless compute nodes via Kubernetes (HPA), hot-reloading JSON definitions from object storage instantaneously without restarting the master process, and exposing an SDK specifically built for executing custom CI/CD terraform deployment pipelines securely.
