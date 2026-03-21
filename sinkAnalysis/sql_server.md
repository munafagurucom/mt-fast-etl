# Rust Integration Guide: SQL Server (MSSQL)

## Overview
Microsoft SQL Server (T-SQL) remains an enterprise backbone for data lakes and operational storage. Integrating Rust with MSSQL requires specific network libraries utilizing the Tabular Data Stream (TDS) protocol.

## APIs & Methods
Rust connects natively to SQL Server using a pure-Rust asynchronous driver.

### 1. `tiberius` (Primary asynchronous driver)
- **Crate:** `tiberius`
- **Method:** A pure Rust implementation of the Microsoft TDS protocol.
- **Focus:** No C-bindings or external ODBC drivers required. Compatible natively with Tokio or `async-std`.
- **Example Usage:**
  ```rust
  use tiberius::{Client, Config};
  use tokio::net::TcpStream;
  use tokio_util::compat::TokioAsyncWriteCompatExt;

  async fn insert_data(config: Config) -> tiberius::Result<()> {
      let tcp = TcpStream::connect(config.get_addr()).await?;
      let mut client = Client::connect(config, tcp.compat_write()).await?;
      
      client.execute(
          "INSERT INTO dbo.Metrics (Device, Temp) VALUES (@P1, @P2)",
          &[&"Sensor_A", &25.5_f32]
      ).await?;
      Ok(())
  }
  ```

### 2. `sqlx` (via MSSQL feature)
- **Crate:** `sqlx::mssql`
- **Focus:** Extends `sqlx` macro queries. It utilizes `tiberius` under the hood dynamically. You benefit from compiler-enforced query verification against the live server schema.

## Configurations
- **Encryption:** Modern SQL Server installations force TLS/SSL connections. Make sure to specify `config.encryption(tiberius::EncryptionLevel::Required)` locally against your target node and configure the `TrustServerCertificate` parameter appropriately if running locally on Docker.
- **Connection Pools:** `tiberius` does not implicitly have connection pooling. Either utilize `deadpool-tiberius` to manage cross-thread pooling or exclusively rely on `sqlx`'s `MssqlPoolOptions`.
- **Authentication:** Supports native SQL Server Authentication (username/password) and Windows Authentication natively if integrated correctly with NTLM/Kerberos.

## Features & Transformations
- **Bulk Insert Pipelines:** Unlike pure Postgres array binds or `COPY`, moving millions of rows to SQL Server uses Table-Valued Parameters (TVPs) or native `.bulk_insert()` APIs built into Tiberius to inject rows without transactional statement parsing.
- **Transactions:** Begin, commit, and rollback logic are handled seamlessly. Tiberius binds logic explicitly across multiple calls to `.execute()`.

## Pros & Cons
### Pros
- **Pure Rust TDS:** The `tiberius` crate operates completely dependency-free of heavy MS binaries (`msodbcsql17`), making Alpine Linux Docker deployments extremely simple.
- **Type Compatibility:** Maps `Uuid`, `chrono::DateTime`, and other generic crates natively without messy implicit casting via T-SQL queries.

### Cons
- **Limited ORM Support:** `Diesel` does not officially support SQL Server asynchronously as fluently as `sqlx`, forcing you into query-builders or raw SQL.
- **Windows Auth Nuance:** Configuring Windows/Active Directory authentication out-of-the-box from a Linux Docker Rust container to MSSQL can be complex, often requiring explicit NTLM settings.

## Issues & Workarounds
- **Issue: Connection dropouts with Azure SQL.**
  - *Workaround:* Azure SQL will forcibly terminate idle SQL Server connections violently. Configure your pooler (e.g. `deadpool` or `sqlx`) natively to test connection health (`SELECT 1`) upon acquisition and set strict min/max lifetimes properly preventing broken pipe exceptions.
- **Issue: "Multiple Active Result Sets" (MARS) errors.**
  - *Workaround:* Tiberius doesn't execute queries strictly parallel on the same literal connection socket. If utilizing active streams traversing a result logic, you cannot fire a new `.execute()` unless the previous stream is gracefully dropped or consumed sequentially.

## Recommendations & Suggestions
1. **Prefer `sqlx`:** While `tiberius` is robust, `sqlx` shields developers from boilerplate setup (`TcpStream::connect`) and natively orchestrates massive pool behaviors optimally.
2. **Batch Sinking:** Construct massive insert values structurally (`INSERT INTO my_table (c1) VALUES (@P1), (@P2), (@P3)`) dynamic string generation if bulk methods fail on certain restricted Active Directory implementations, saving 100x sequential API calls over the Internet.
3. **Data Types:** Be notoriously exact using `NVARCHAR` (`&str` or `String`) correctly against legacy `VARCHAR` tables. Implicit conversions (CONVERT/CAST) natively handled by T-SQL will shatter indexing efficiency natively if the parameter binding is slightly off natively inside the Rust pipeline.
