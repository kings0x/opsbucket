# OpsBucket Archiver — Context

**Before reading this file**, make sure you have read:
- [`/AGENTS.md`](/AGENTS.md) — who you are and what tools you have
- [`/CONTEXT.md`](/CONTEXT.md) — the full pipeline, payload schema, and Git conventions

---

## What the Archiver Service Is

`opsbucket-archiver` is a **dedicated Kafka consumer** that reads all events from the
`raw-events` topic in its own consumer group (`opsbucket-archiver`) and writes them to
S3-compatible storage as Parquet files. It runs entirely independently of the processing
service — it does not share a consumer group, and its failures do not affect the main
processing pipeline.

**Purpose:** Long-term raw event archive. ClickHouse retains data for 12 months (TTL policy).
The archiver provides a queryable backup of every raw event, stored in open Parquet format
on S3, that can be queried with Athena, DuckDB, Spark, or any Parquet-compatible tool.

---

## How It Fits in the Pipeline

```
SDK → Ingestion → Redpanda (raw-events)
                      ├── Processing consumer group → dedup → flatten → ClickHouse
                      └── Archiver consumer group   → buffer → Parquet → S3
```

Both consumers read the same `raw-events` topic independently. The archiver cannot affect
processing because:
1. It uses a **different consumer group** (`opsbucket-archiver` vs `opsbucket-processing`)
2. It **only reads** from Kafka; it never produces or modifies anything
3. Its offset commits are independent

---

## What It Does

### Kafka Consumption
- Subscribes to `raw-events` in group `opsbucket-archiver`
- Polls with configurable batch size (default: 10,000) and timeout (default: 60s)
- Buffers events in memory
- Flushes to S3 when buffer reaches batch size, file size limit, or timeout expires

### Parquet Output
- Each flush writes one Parquet file per date partition
- S3 key structure: `<prefix>/dt=YYYY-MM-DD/part-<timestamp>.parquet`
- Parquet schema (5 columns):

| Column       | Type     | Description                                |
|-------------|----------|--------------------------------------------|
| `event_id`   | String   | `messageId` from the event                 |
| `project_id` | String   | Tenant/project identifier                  |
| `event_type` | String   | `track`, `identify`, or `page`             |
| `timestamp`  | Int64    | Unix milliseconds from `sentAt`            |
| `raw_event`  | String   | Complete RawEvent as JSON                  |

- Compression: ZSTD
- Hive-style partitioning by date for efficient range filtering

### Offset Commit Discipline
- Offsets are committed **only after** the S3 upload confirms
- On crash between write and commit: events are re-read and deduped by the archiver
  (duplicate Parquet rows are acceptable for archival; downstream consumers can
  deduplicate by `event_id`)

---

## Configuration

| Variable | Required | Default | Description |
|----------|----------|---------|-------------|
| `KAFKA_BROKERS` | Yes | — | Comma-separated Redpanda broker addresses |
| `KAFKA_CONSUMER_GROUP` | No | `opsbucket-archiver` | Consumer group; change per-environment |
| `ARCHIVE_S3_BUCKET` | Yes | — | S3 bucket name |
| `ARCHIVE_S3_PREFIX` | No | `raw-events` | Key prefix for all archive files |
| `ARCHIVE_S3_REGION` | No | `us-east-1` | AWS region |
| `ARCHIVE_S3_ENDPOINT` | No | — | Optional S3-compatible endpoint (for R2, MinIO) |
| `BATCH_SIZE` | No | `10000` | Max events per Kafka poll / Parquet file |
| `BATCH_TIMEOUT_MS` | No | `60000` | Max wait before flushing a partial batch |
| `ARCHIVE_MAX_FILE_SIZE` | No | `67108864` | Approximate max bytes before flushing (64MB) |
| `RUST_LOG` | No | `info` | Tracing/log level |

S3 authentication uses the standard AWS credential chain:
- `AWS_ACCESS_KEY_ID` / `AWS_SECRET_ACCESS_KEY` environment variables
- `~/.aws/credentials` file
- IAM instance profile (when deployed on EC2/Render)

---

## What It Deliberately Does NOT Do

- **No deduplication** — duplicates in the archive are acceptable. Downstream consumers
  (Athena, DuckDB) can deduplicate by `event_id`.
- **No schema flattening** — the raw event JSON preserves all original fields. The archiver
  is not a second processing pipeline; it is a backup.
- **No DLQ** — if an event fails to deserialize, it is skipped (logged at WARN). The
  archiver does not maintain a separate DLQ topic.
- **No retention management** — S3 lifecycle policies are configured externally.
- **No query interface** — the archive is queryable via external tools (Athena, DuckDB,
  Spark), not via OpsBucket's query API.

---

## Dependencies

```toml
rdkafka = "0.36"         # Kafka consumer
object_store = "0.11"    # S3 client (with "aws" feature)
parquet = "53"           # Parquet file format (with "arrow" feature)
arrow = "53"             # Arrow record batch construction
chrono = "0.4"           # Date/time handling
serde_json = "1"         # RawEvent serialization
tokio = "1"              # Async runtime
tracing = "0.1"          # Structured logging
```

---

## Building and Running

```bash
# Build
cargo build -p opsbucket-archiver

# Run (with local S3-compatible store, e.g. MinIO)
KAFKA_BROKERS="localhost:9092" \
ARCHIVE_S3_BUCKET="opsbucket-archive" \
ARCHIVE_S3_ENDPOINT="http://localhost:9000" \
ARCHIVE_S3_REGION="us-east-1" \
AWS_ACCESS_KEY_ID="minioadmin" \
AWS_SECRET_ACCESS_KEY="minioadmin" \
  cargo run -p opsbucket-archiver
```
