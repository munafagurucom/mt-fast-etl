# Deployment Plan: Bring Your Own Cloud (BYOC) via Virtual Machine

This document provides a step-by-step guide for deploying the Rust Data Pipeline Engine inside a customer's own cloud account using a pre-packaged Virtual Machine image listed on AWS, Azure, or Google Cloud Marketplace.

The customer's data **never leaves their cloud account**. You (the vendor) never touch their infrastructure after the initial image is published to the marketplace.

---

## Architecture Overview

```
Customer's VPC / Virtual Network
┌─────────────────────────────────────────────────────────┐
│                                                         │
│   ┌──────────────┐    ┌───────────────────────────┐    │
│   │  Source DB   │───▶│  Rust Pipeline Engine VM  │    │
│   │ (Postgres,   │    │  ┌─────────────────────┐  │    │
│   │  Kafka, etc) │    │  │  SourceConnector     │  │    │
│   └──────────────┘    │  │  Transform/Adapter   │  │    │
│                        │  │  SinkConnector       │  │    │
│   ┌──────────────┐    │  └─────────────────────┘  │    │
│   │  Destination │◀───│  Port 8080: Control API   │    │
│   │ (Snowflake,  │    │  Port 9090: /metrics      │    │
│   │  BigQuery)   │    └───────────────────────────┘    │
│   └──────────────┘                    │                 │
│                                       ▼                 │
│                           ┌────────────────────┐        │
│                           │  Customer S3/Blob  │        │
│                           │  (State + DLQ)     │        │
│                           └────────────────────┘        │
└─────────────────────────────────────────────────────────┘
```

---

## Step 1: Customer Subscribes via Cloud Marketplace

### AWS
1. Customer navigates to the **AWS Marketplace** listing for the Rust Data Pipeline Engine.
2. Clicks **"Continue to Subscribe"** → Accepts the software license terms.
3. Clicks **"Continue to Configuration"** → Selects region and software version.
4. Clicks **"Continue to Launch"** → Chooses to launch via **EC2** or **CloudFormation template**.

### Azure
1. Customer navigates to the **Azure Marketplace** listing.
2. Clicks **"Get It Now"** → Selects their Azure Subscription and Resource Group.
3. Proceeds to deploy as an **Azure Managed Application** or a standalone **Virtual Machine** from the provided VHD image.

### Google Cloud
1. Customer navigates to the **GCP Marketplace** listing.
2. Clicks **"Launch"** → Selects the GCP Project and target Region/Zone.
3. Deploys as a **Compute Engine VM** using the pre-built Machine Image.

---

## Step 2: Customer Provisions the Virtual Machine

### Minimum Recommended Specifications

| Workload Scale | AWS Instance | Azure VM | GCP Machine | vCPU | RAM |
|---|---|---|---|---|---|
| Small (≤10 pipelines) | `c6i.large` | `Standard_F2s_v2` | `c2-standard-4` | 2 | 4 GB |
| Medium (≤100 pipelines) | `c6i.xlarge` | `Standard_F8s_v2` | `c2-standard-8` | 4-8 | 16 GB |
| Large (500+ pipelines) | `c6i.4xlarge` | `Standard_F32s_v2` | `c2-standard-32` | 16+ | 64 GB |

**OS:** Ubuntu 22.04 LTS (baked into the VM image)

### Networking Requirements
- The VM must reside within the **same VPC/VNet** as the source databases, or have VPC peering enabled.
- Outbound HTTPS (port 443) to destination SaaS sinks (Snowflake, Databricks, BigQuery).
- Inbound TCP 8080 (Control API) and 9090 (Metrics) — **restrict to admin CIDRs only**.
- Attach an **IAM Role (AWS) / Managed Identity (Azure) / Service Account (GCP)** with write access to an S3/Blob/GCS bucket for pipeline state storage.

---

## Step 3: Customer Configures the Engine

On first boot, the VM starts the Rust engine in setup mode. The customer configures it via a provided CLI tool or by editing a YAML file.

### 3a. Configure Object Storage (State Backend)

```yaml
# /etc/pipeline-engine/config.yaml

state_backend:
  provider: "s3"                         # s3 | azure_blob | gcs
  bucket: "my-company-pipeline-state"
  prefix: "rust-pipeline/"
  region: "us-east-1"
  # Credentials automatically sourced from attached IAM Role
```

### 3b. Register a License Key

The engine calls your hosted licensing API once on startup to validate the customer's license key. This is the **only** external call made back to your infrastructure.

```bash
pipeline-engine license activate --key XXXX-XXXX-XXXX-XXXX
```

### 3c. Define a Pipeline (1-to-1 Mapping)

The customer POSTs a pipeline definition to the local Control API running on port 8080:

```bash
curl -X POST http://localhost:8080/api/v1/pipelines/spawn \
  -H "Content-Type: application/json" \
  -d '{
    "pipeline_id": "postgres-to-snowflake-001",
    "source": {
      "type": "postgres_cdc",
      "host": "prod-db.internal",
      "port": 5432,
      "database": "orders",
      "slot_name": "pipeline_slot_001",
      "secret_ref": "arn:aws:secretsmanager:us-east-1:123:secret/pg-creds"
    },
    "field_mappings": [
      { "source_field": "order_id",    "sink_field": "ORDER_ID",   "cast": "string" },
      { "source_field": "customer_id", "sink_field": "CUST_ID",    "cast": "string" },
      { "source_field": "amount",      "sink_field": "AMOUNT_USD", "cast": "float"  }
    ],
    "sink": {
      "type": "snowflake",
      "account": "xy12345.snowflakecomputing.com",
      "warehouse": "LOAD_WH",
      "database": "ANALYTICS",
      "schema": "PUBLIC",
      "table": "ORDERS",
      "auth": { "type": "keypair", "secret_ref": "arn:aws:secretsmanager:us-east-1:123:secret/sf-key" }
    }
  }'
```

The engine immediately spawns an isolated `tokio` async worker for this pipeline. It begins extracting data from the source and sinking to the destination in real time.

---

## Step 4: Monitor the Running Pipelines

### Option A: Prometheus + Grafana (Recommended)
The engine exposes a `/metrics` endpoint (port 9090) in Prometheus format. The customer points their existing Grafana instance at it:
```
http://<vm-private-ip>:9090/metrics
```

### Option B: Control API Status Check
```bash
# List all active pipelines and their live metrics
curl http://localhost:8080/api/v1/pipelines

# Response
[
  {
    "pipeline_id": "postgres-to-snowflake-001",
    "status": "running",
    "records_read": 1482930,
    "records_written": 1482907,
    "dlq_count": 23,
    "lag_seconds": 4
  }
]
```

---

## Step 5: Updates & Patching

As the vendor, you publish updated VM images to the Marketplace when new connector versions or security patches are available.

**Customer Update Flow:**
1. AWS Marketplace sends the customer a notification about the new version.
2. Customer launches a **new VM** from the updated image (zero downtime — old VM continues running).
3. Because all pipeline state is stored on S3/Blob/GCS (not on the VM disk), the new VM automatically loads and resumes all existing pipelines on startup.
4. Customer terminates the old VM.

This **stateless VM design** is the critical feature that allows seamless blue-green upgrades with no data loss and no downtime.

---

## Security Responsibilities

| Responsibility | Customer | Vendor (You) |
|---|---|---|
| VPC / Firewall rules | ✅ | |
| Data at rest (encryption) | ✅ | |
| IAM role permissions | ✅ | |
| OS patching of VM image | | ✅ |
| Connector bug fixes | | ✅ |
| License key validation | ✅ | ✅ |
| Secrets management | ✅ (uses their own Secrets Manager) | |
