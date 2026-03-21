# Rust Integration Guide: MySQL

## Overview
MySQL is a widely deployed open-source relational database. Writing data to MySQL from a Rust application is a common pattern for ETL (Extract, Transform, Load) operations and web backend systems.

## APIs & Methods
Rust provides excellent support for MySQL through multiple crates, catering to both asynchronous and synchronous workloads.

### 1. `sqlx` (Asynchronous pure Rust SQL crate)
- **Crate:** `sqlx`
- **Method:** Compile-time verified queries (`sqlx::query!`)
- **Example Usage:**
  ```rust
  use sqlx::mysql::MySqlPoolOptions;

  async fn insert_record(pool: &sqlx::MySqlPool, val1: &str, val2: i32) -> Result<(), sqlx::Error> {
      sqlx::query!(
          "INSERT INTO my_table (col1, col2) VALUES (?, ?)",
          val1, val2
      )
      .execute(pool)
      .await?;
      Ok(())
  }
  ```

### 2. `mysql_async` (Asynchronous driver)
- **Crate:** `mysql_async`
- **Focus:** Low-level asynchronous interaction natively built for Tokio.

### 3. `diesel` (Synchronous ORM)
- Provides a deeply-typed schema synchronization layer, ensuring no runtime errors regarding missing columns or tables.

## Configurations
- **Connection Pools:** `MySqlPoolOptions` manages concurrent connections efficiently without overloading MySQL's thread-per-connection model.
- **Time Zones:** Configure connection sessions to UTC reliably (`SET time_zone = '+00:00'`) to prevent timezone drift during inserts.
- **Collation:** Ensure character sets are strictly defined (`utf8mb4` for comprehensive Unicode/emoji support).

## Features & Transformations
- **Prepared Statements:** Enable `binary` protocol natively in `sqlx` to execute repeated parameters securely without parsing step overhead on the database side.
- **Local Infile / Bulk Insert:** MySQL supports `LOAD DATA INFILE`. While tricky to set up asynchronously over pure TCP, batch inserts (`INSERT INTO ... VALUES (), (), ()`) work effortlessly in Rust using `QueryBuilder`.

## Pros & Cons
### Pros
- **Widespread Usage:** The ecosystem mapping SQL data types (like `DATETIME` to `chrono::NaiveDateTime`) is extremely mature.
- **Safety:** Using `sqlx`, potential SQL injection is structurally eliminated by design.

### Cons
- **Write Performance:** Slower bulk unnesting compared to PostgreSQL's `COPY` command.
- **Blocking:** Heavy batched inserts lock tables (MyIsam) or rows (InnoDB) creating contention if not batched intelligently.

## Issues & Workarounds
- **Issue: 'Packet too large' errors.**
  - *Workaround:* If running batched inserts, the payload can easily exceed `max_allowed_packet`. Configure `max_allowed_packet` appropriately in MySQL, and strictly limit the vector length of batched inserts inside Rust to (e.g., 1,000 to 5,000 rows).
- **Issue: Dropped Connections (Wait Timeout).**
  - *Workaround:* Configure the `PgPool`/`MySqlPool`'s idle timeout to be significantly lower than MySQL's `wait_timeout` to ensure Rust closes and clears stale connections before the server drops them forcefully.

## Recommendations & Suggestions
1. **Batching:** Use `sqlx::QueryBuilder` to dynamically construct multi-row insert statements to reduce network I/O latency.
2. **Migrations:** Integrate `sqlx-cli` database migrations inside `build.rs` or standard CI checks.
3. **Data Mapping:** Carefully map MySQL unsigned types to standard Rust types (`u32`, `u64`), unlike PostgreSQL which does not support unsigned identifiers native.
