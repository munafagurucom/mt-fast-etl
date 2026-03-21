# Source Connectors by Platform

This document outlines the list of source connectors offered by RisingWave, Fivetran, Airbyte, Confluent, and Hevo Data, based on their capabilities and supported integrations.

---

## 1. RisingWave
RisingWave specializes in real-time, continuous data ingestion for streaming analytics.

**Message Queues:**
- Apache Kafka
- Redpanda
- Apache Pulsar
- Amazon Kinesis
- NATS JetStream
- MQTT
- Google Pub/Sub

**Change Data Capture (CDC) Databases:**
- PostgreSQL CDC (versions 10-14)
- MySQL CDC (versions 5.7, 8.0)
- SQL Server CDC (versions 2019, 2022)
- MongoDB CDC

**Cloud Storage & Data Lakes:**
- Amazon S3
- Google Cloud Storage
- Azure Blob
- Apache Iceberg

**Other Sources:**
- Webhook (Built-in for HTTP ingestion)
- Events API (HTTP, External service)
- Snowflake
- Load generator (Built-in for testing)

---

## 2. Fivetran
Fivetran offers automated data movement from over 700+ different sources, categorizing them broadly:

**SaaS Applications:**
- Major cloud-based CRMs, ERPs, Marketing platforms, etc. (e.g., Salesforce, Marketo, Zendesk).

**Databases:**
- Relational and NoSQL Databases (e.g., Oracle, SQL Server, PostgreSQL, MySQL) including cloud-native equivalents.

**Files & Events:**
- File storage and sharing platforms (e.g., CSVs via Amazon S3, Google Drive, Box).
- Event trackers like Webhooks, Snowplow, etc.

*Note: Fivetran manages standard connectors for widespread services and allows for Partner-built or SDK-based custom connectors for niche applications.*

---

## 3. Airbyte
Airbyte provides over 600 connectors, with a robust open-source library supporting varied data sources. 

**Popular Connectors Include:**
- ActiveCampaign
- Amazon Ads
- Airtable
- Asana
- Facebook Marketing
- Github
- Google Ads / Google Analytics
- Google Sheets
- Salesforce
- Shopify
- Slack
- Stripe
- Twitter / X
- Zendesk Support

*Note: Airbyte community contributions expand their marketplace connectors continuously, while "Airbyte" officially maintains core connectors.*

---

## 4. Confluent
Confluent specializes in Kafka Connect source connectors for ingesting data into Apache Kafka topics. They offer over 120 pre-built connectors.

**Database Connectors:**
- JDBC Source Connector (Relational DBs)
- Oracle CDC Source Connector
- Debezium Connectors (CDC for SQL Server, MongoDB, MySQL, PostgreSQL)
- Amazon DynamoDB CDC Source Connector

**Cloud & Messaging Systems:**
- Amazon S3, Kinesis, SQS
- Azure Blob Storage, Event Hubs
- ActiveMQ, JMS, AMPS
- Google Cloud Pub/Sub

**APIs & Applications:**
- HTTP Source Connector (polls JSON-based HTTP APIs)
- Zendesk Source Connector

---

## 5. Hevo Data
Hevo offers 150+ pre-built connectors specifically built for robust data integration pipelines.

**SaaS Sources (By Business Purpose):**
- **Marketing:** Google Ads, Facebook Ads, LinkedIn Ads, HubSpot, Marketo, Mailchimp.
- **Sales & Support:** Salesforce, Zendesk, Freshdesk.
- **Product & E-Commerce:** Amplitude, Mixpanel, Pendo, Shopify.
- **Finance/Accounting:** Stripe, Xero.

**Database & File System Sources:**
- **Databases:** MySQL, PostgreSQL, Oracle, SQL Server, Amazon DocumentDB, Elasticsearch, MongoDB.
- **Data Warehouses:** Amazon Redshift, Google BigQuery.
- **File Storage:** Amazon S3, Google Drive, Google Sheets.

**Custom Sources:**
- Webhooks & REST API support for extracting data not natively listed by pre-built connectors.
