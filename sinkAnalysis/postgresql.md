# Rust Integration Guide: PostgreSQL

## Overview
PostgreSQL is one of the most widely used relational database sinks. This document details how a Rust-based application can write data efficiently to PostgreSQL, covering APIs, configurations, and common features.

## APIs & Methods
Rust features an incredibly robust database ecosystem. The two dominant paradigms are asynchronous ORMs/Query Builders and pure driver connections.

### 1. `sqlx` (Asynchronous pure Rust SQL crate)
- **Crate:** `sqlx`
- **Method:** `sqlx::query!`, `sqlx::query_builder`
- **Focus:** Compile-time verified SQL queries and asynchronous execution.
- **Example Usage:**
  ```rust
  use sqlx::postgres::PgPoolOptions;

  async fn insert_user(pool: &sqlx::PgPool, name: &str, email: &str) -> Result<(), sqlx::Error> {
      sqlx::query!(
          "INSERT INTO users (name, email) VALUES ($1, $2)",
          name, email
      )
      .execute(pool)
      .await?;
      Ok(())
  }
  ```

### 2. `diesel` (Synchronous ORM)
- **Crate:** `diesel`, `diesel-async` (for Tokio support)
- **Focus:** Strictly typed, compiler-enforced ORM mappings using code generation.

### 3. `tokio-postgres`
- Lower-level async driver, useful if you need absolute control over pipelining or COPY commands.

## Configurations
- **Connection Pools:** Use `sqlx::postgres::PgPoolOptions` to define `max_connections`, `idle_timeout`, and `acquire_timeout`. Never create a new connection per request.
- **SSL/TLS:** Enable the `tls-rustls` or `tls-native-tls` features in `sqlx` to encrypt data in transit over the network.
- **Timezone & Encoding:** Standardize the database config to `UTF-8` and handle dates using the `chrono` or `time` crate features natively supported by the drivers.

## Features & Transformations
- **Bulk Inserts (COPY):** For loading massive datasets, `tokio-postgres` supports the `COPY IN` binary protocol, which drastically outperforms standard `INSERT` statements.
- **Transactions:** Rust’s `sqlx` models transactions wonderfully. `let mut tx = pool.begin().await?` ensuring rolling back if the transaction object is dropped before `.commit()` is called.
- **JSONB Support:** Rust structs serialize natively into `jsonB` Postgres columns using `serde_json`.

## Pros & Cons
### Pros
- **ACID Compliant:** Strict safety guarantees for mission-critical sinks.
- **Compile-time Checks:** `sqlx` prevents runtime syntax errors and type mismatches natively.
- **Rich Types:** Native JSONB, arrays, and UUIDs are fully mapped to Rust types.

### Cons
- **Write Limits:** Cannot scale horizontal writes natively as easily as NoSQL data stores.
- **Connection Overhead:** Maxing out connections can crash the Postgres server (PgBouncer is often needed).

## Issues & Workarounds
- **Issue: High connection usage crashing the database context.**
  - *Workaround:* Use a connection pooler like `PgBouncer` or `Supavisor`. Inside Rust, ensure the `PgPool::max_connections` is tuned relative to server size.
- **Issue: Slow bulk inserts using ORMs.**
  - *Workaround:* Avoid looping `INSERT INTO`. Use `UNNEST` arrays passed as arguments in `sqlx` to execute bulk inserts in a single roundtrip, or use `COPY FROM STDIN`.

## Recommendations & Suggestions
1. **Migrations:** Use `sqlx-cli` to handle database migrations cleanly as part of your CI/CD flow.
2. **Prepared Statements:** Ensure prepared statements are enabled (default in `sqlx`) to save Postgres parsing time on repeated schema writes.
3. **Data Types:** Enable `uuid` and `chrono` feature flags in your Postgres drivers to natively map cleanly to Rust without manual casting.
