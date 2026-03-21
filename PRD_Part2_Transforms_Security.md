# Rust Data Pipeline PRD: Part 2 - Transformations, Schemas, and Security

## 2. Default Transformations & Filtering
The application provides out-of-the-box transformations executed at sub-millisecond speeds using native Rust operations before serialization:
- **Field Mutation:** Map source fields to sink keys (e.g., `{"CustID": ...}` to `{"customer_id": ...}`).
- **Type Coercion:** Explicit type casting (e.g., Integer to String, Epoch Unix to ISO8601 Timestamp).
- **Filtering:** Declarative predicate filtering (e.g., `DROP ROW IF amount < 0`).
- **Data Masking/Hashing:** Native SHA-256 hashing or Redaction (`***`) for PII columns like SSN or Credit Card before writing to analytical sinks.

## 3. Custom Rust Code for Transformations & Encryption
For operations exceeding default capabilities, operators can inject custom Rust code natively:
- **UDFs (User Defined Functions):** The engine allows registering `Box<dyn Fn(Row) -> Row>` callbacks locally during compilation. 
- **Encryption Logic:** Customers can implement custom envelope encryption (e.g., AES-GCM-256) inside the pipeline, dynamically fetching DEKs (Data Encryption Keys) from their own KMS before persisting to the destination.

## 5. Passing Secrets and Keys
Security and secret management are handled inherently. No plaintext keys are ever passed in configuration files.
- **Secret References:** The YAML/JSON configuration uses Uniform Resource Names (URNs): `secret_ref: "arn:aws:secretsmanager:region:account:secret:my-db-pass"`.
- **Vault Integrations:** The Rust application leverages the AWS SDK (`aws-sdk-secretsmanager`), Azure Identity, or HashiCorp Vault APIs to fetch plaintext keys directly into protected memory at runtime using asynchronous clients.
- **Environment Fallbacks:** Seamless support for `.env` files via the `dotenvy` crate.

## 6. Custom Serialization and Deserialization
If standard `serde_json` or Apache Avro definitions are insufficient (e.g., proprietary main-frame formats, Protobufs, or FIX protocol):
- **Custom Deserializers:** Users implement the `PayloadDeserializer` trait, taking raw `&[u8]` byte payloads from the socket/stream and explicitly parsing them into the standard internal memory representation (Arrow RecordBatch).
- **Custom Serializers:** Corresponding traits allow converting the internal Arrow memory matrices into any proprietary egress format before triggering the Sink connector HTTP/TCP handoffs.

## 9. Schema Validation & Dead Letter Queue (DLQ)
Strict data contracts to enforce warehouse and pipeline integrity.
- **Schema Enforcement:** If a strict JSON Schema or Arrow Schema is provided at pipeline spawn, every incoming row is evaluated concurrently. Missing non-nullable fields or underlying type mismatches immediately trigger standard validation errors.
- **Dead Letter Queue (DLQ):** Bad records are instantly shunted off the main execution hot-path to avoid halting the system. They are serialized with an error payload wrapper containing the explicit validation failure and dumped into a partitioned S3 bucket (`s3://dlq/pipeline_id/type_mismatch/year/month/`). The DLQ acts as a secondary data lake, allowing operators to trace, modify, and replay the data once the initial schema definition is corrected.
