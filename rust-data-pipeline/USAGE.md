# Rust Data Pipeline Engine — Usage Guide

The Rust Data Pipeline Engine is a high-performance data processing tool for reliable extraction, transformation, and loading (ETL) of data streams. Built in Rust, it emphasizes speed, type safety, and low resource utilization.

## Getting Started

### 1. Running the Demo
To quickly verify that the application is built correctly and working, run the built-in `stdin` → `stdout` demo pipeline:

```bash
cargo run -- --demo
```
Once started, type raw JSON objects (one per line) into your terminal and press Enter. The pipeline will immediately process and output them. The demo pipeline simply passes data through without modification.

### 2. Running with a Configuration File
To run a custom pipeline, provide a pipeline configuration file in JSON or YAML format:

```bash
cargo run -- --config examples/my_pipeline.json
```
or 
```bash
cargo run -- -c examples/my_pipeline.json
```

### 3. Running as a Control Plane API
If no arguments are provided, the engine will start an HTTP control plane API on port `8080` (default) which can be overridden via `--api-port`.

```bash
cargo run
# Or on a custom port
cargo run -- --api-port 9090
```

---

## Pipeline Configuration

A pipeline configuration file dictates how data is routed and transformed. Below is the structure of a `PipelineConfig` file.

### Basic Structure

```json
{
  "pipeline_id": "kafka-to-postgres-events",
  "enabled": true,
  "source": { ... },
  "sink": { ... },
  "field_mappings": [ ... ],
  "transforms": [ ... ],
  "schema": { ... },
  "rate_limits": { ... },
  "flags": { ... }
}
```

### 1. Sources (`source`)
Defines where the pipeline reads data from.

**Available Connectors:**
*   `stdin`: Reads from standard input (useful for testing).
*   `kafka`: Reads messages from an Apache Kafka topic.

**Kafka Source Example:**
```json
"source": {
  "type": "kafka",
  "properties": {
    "bootstrap_servers": "localhost:9092",
    "topic": "events.incoming",
    "group_id": "data_pipeline_consumer"
  },
  "secrets": {}
}
```

### 2. Sinks (`sink`)
Defines where the pipeline writes data to.

**Available Connectors:**
*   `stdout`: Writes to standard output.
*   `postgres`: Writes rows to a PostgreSQL database table.

**PostgreSQL Sink Example:**
```json
"sink": {
  "type": "postgres",
  "properties": {
    "table": "events_archived"
  },
  "secrets": {
    "database_url": "env:DATABASE_URL"
  }
}
```
*(Note: Secrets can use prefixes like `env:` to fetch from environment variables.)*

### 3. Field Mappings (`field_mappings`)
Provide 1-to-1 mappings to rename or cast fields from the source to the sink.

```json
"field_mappings": [
  {
    "source_field": "user_id",
    "sink_field": "account_id",
    "cast": "string"
  }
]
```

### 4. Transformations (`transforms`)
Pipelines support inline data transformations before reaching the sink.

**Available Transforms:**
*   `field_mapper`: Applies the defined `field_mappings`.
*   `row_filter`: Drops rows based on a boolean condition/column.

**Filter Transform Example:**
```json
"transforms": [
  {
    "type": "row_filter",
    "params": {
      "filter_column": "is_valid_event" 
    }
  }
]
```
*(Rows where `is_valid_event` is `false` will be dropped.)*

### 5. Schema Validation & Drift (`schema`)
Handle unexpected data format changes securely.

```json
"schema": {
  "schema_path": "schemas/events.json",
  "drift_policy": "Dlq",
  "auto_infer": false
}
```
*Drift Policies*: `Halt`, `DropField`, `Dlq` (Dead Letter Queue - Default), `AlterTable`.

### 6. Rate Limiting (`rate_limits`)
Control throughput to prevent overwhelming downstream services.

```json
"rate_limits": {
  "max_source_tps": 10000,
  "max_sink_tps": 5000,
  "max_concurrent_connections": 10
}
```

### 7. Feature Flags (`flags`)
Tune internal pipeline mechanisms.

```json
"flags": {
  "enable_dlq": true,
  "enable_schema_validation": false,
  "require_tls": true,
  "max_memory_buffer_mb": 512,
  "batch_flush_interval_ms": 5000,
  "enable_lineage": true,
  "concurrency_limit": 64
}
```
