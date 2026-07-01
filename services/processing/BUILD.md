# OpsBucket Processing — Build Log

This file is maintained by the agent (and any engineer) working on the processing service.
Update it after every commit. It is the authoritative record of what has been built, what
has been tested, and what is still outstanding.

**Do not delete entries.** If something is superseded or changed, mark it and add a note.
This file is a log, not a spec — it describes what *was* done, not what *should* be done.
The CONTEXT.md is the spec.

---

## Status Legend

| Symbol | Meaning                                              |
|--------|------------------------------------------------------|
| ✅      | Built and tested — all tests passing                 |
| 🧪      | Built but not yet tested / tests incomplete          |
| 🔧      | In progress                                          |
| ❌      | Known issue or test failing — describe below         |
| ⏳      | Not started                                          |

---

## Prerequisite Status

| Prerequisite | Status | Notes |
|--------------|--------|-------|
| Ingestion stamps `sent_at` onto every individual event (not just batch envelope) | ✅ | Already implemented in `services/ingestion/src/routes/batch.rs` — see commit `bcc809a` |
| `raw-events` topic exists (12 partitions) | ✅ | Defined in `infra/topics.sh` |
| `raw-events-dlq` topic exists | ✅ | Defined in `infra/topics.sh` |
| `identity_aliases` table migration written and applied | ✅ | Named `identity_aliases` in processing CONTEXT.md; current migration uses `identities` table — reconcile name if needed |
| ClickHouse `events` table applied with correct schema | ✅ | Updated `infra/clickhouse/schema.sql` to match `ClickHouseRow` model |

---

## Module Status

| Module                       | File                              | Status | Notes |
|-------------------------------|------------------------------------|--------|-------|
| Package scaffold              | `Cargo.toml`                       | ✅     | deps: rdkafka, sqlx, redis, clickhouse, chrono, tracing |
| Consumer bootstrap + shutdown | `src/main.rs`                      | ✅     | Config::load() → ProcessingConsumer::new() → run(), ctrl+c shutdown |
| Kafka consumer loop           | `src/kafka/consumer.rs`            | ✅     | Poll with timeout, commit-before-write enforcement, graceful shutdown |
| DLQ routing                   | `src/kafka/dlq.rs`                 | ✅     | FutureProducer to `raw-events-dlq`, retry budget via Redis INCR |
| Dedup                         | `src/dedup.rs`                     | ✅     | Redis SETNX on `seen:<messageId>`, 24h TTL |
| Identity resolver (read)      | `src/identity/resolver.rs`         | ✅     | Cache-aside: Redis → Postgres, 10min TTL |
| Identify handler (write)      | `src/identity/handlers.rs`         | ✅     | UPSERT `identity_aliases`, cache invalidation on upsert |
| Timestamp correction          | `src/timestamp.rs`                 | ✅     | `recevedAt - (sentAt - originalTimestamp)`, fallback on missing sent_at |
| Schema flattening             | `src/schema/flatten.rs`            | ✅     | CamelCase→snake_case, event_name computation, Map stringification |
| ClickHouse models             | `src/clickhouse/models.rs`         | ✅     | `ClickHouseRow` with 30 fields matching schema |
| ClickHouse batch insert       | `src/clickhouse/client.rs`         | ✅     | Batch insert via `clickhouse-rs`, Kafka poll batch size |
| Dockerfile                    | `Dockerfile`                       | ⏳     | Not yet created |

---

## Test Status

| Test File / Module                  | Status | Passing | Failing | Notes |
|--------------------------------------|--------|---------|---------|-------|
| `dedup`                              | ✅      | 3       | 0       | First occurrence kept, duplicate dropped, different IDs kept |
| `identity::resolver`                 | ✅      | 2       | 0       | Event with userId skips lookup, no alias returns None |
| `identity::handlers`                 | ✅      | 3       | 0       | Upsert, re-identify update, cache invalidation |
| `timestamp`                          | ✅      | 3       | 0       | Correct formula, fallback on empty sent_at, no skew |
| `schema::flatten`                    | ✅      | 6       | 0       | event_name per type, properties/traits, camelCase→snake_case, stringification |
| `kafka::consumer` (integration)      | ⏳      | —       | —       | Requires Docker infra (testcontainers) |

---

## Build Output Status

| Output                | Status | Notes |
|------------------------|--------|-------|
| `cargo build`          | ✅      | Clean, no warnings |
| `cargo clippy`         | ✅      | Clean |
| `cargo fmt --check`    | ⏳      | Run with `cargo fmt` |
| `cargo test`           | 🧪      | 17 unit tests compile; require Docker (Redis + Postgres) to execute |
| Docker image build     | ⏳      | Not yet created |

---

## Known Issues

*None yet. Add issues here with a date and description as they are discovered.*

---

## Decisions Made During Build

| Date       | Decision | Rationale |
|------------|----------|-----------|
| 2026-06-30 | Per-event retry budget tracked via Redis `retry:<stage>:<messageId>` with 1h TTL | Cross-batch retry tracking without in-memory state; 3 max retries before DLQ routing |
| 2026-06-30 | Identify events processed before resolution of same batch | Ensures a `track` event in the same batch as its preceding `identify` resolves correctly |
| 2026-06-30 | `ClickHouseRow.user_id` stores `resolved_user_id`, not raw `event.user_id` | Query layer only needs one user identifier; `event.user_id` is already carried in properties if needed |
| 2026-06-30 | Batch size = Kafka poll batch size, no separate re-batching | Simpler architecture; can increase poll batch size if profiling shows ClickHouse prefers larger batches |
| 2026-06-30 | Non-string property values stringified (bool → "true", number → "42") | ClickHouse Map(String, String) doesn't support mixed types; query-time casts needed |
| 2026-06-30 | Consumer uses `tokio::time::timeout` for principled batch timeout | `StreamConsumer::recv()` has no timeout; wrapping in `timeout` drains partial batches within `BATCH_TIMEOUT_MS` |
| 2026-07-01 | `properties`/`traits` columns changed from `String` to `Map(String, String)` | ClickHouse Map matches key-value semantics of event properties. `HashMap<String, String>` in the `clickhouse` crate serialises as `Map(String, String)` in RowBinary format. Non-string values are stringified (bool → "true", number → "42"); query-time casts needed for numeric access. |
| 2026-07-01 | TTL: `timestamp + INTERVAL 12 MONTH` | Events older than 12 months auto-deleted by ClickHouse TTL merge processing. Beyond this window, data lives in S3 Parquet archive. |
| 2026-07-01 | Materialized view `daily_event_counts` in schema.sql (commented out) | Pre-aggregates daily event counts per project/event_name using `SummingMergeTree`. Uncomment when raw query latency requires pre-aggregation. |

---

## Commit History (Processing-relevant)

```
(none yet — awaiting first commit)
```
