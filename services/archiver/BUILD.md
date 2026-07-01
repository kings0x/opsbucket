# OpsBucket Archiver — Build Log

This file documents the build state and decisions for the archiver service.

---

## Status Legend

| Symbol | Meaning |
|--------|---------|
| ✅ | Built and tested — all tests passing |
| 🔧 | In progress |
| ⏳ | Not started |

---

## Module Status

| Module | File | Status | Notes |
|--------|------|--------|-------|
| Package scaffold | `Cargo.toml` | 🔧 | Created, needs build verification |
| Config | `src/config.rs` | 🔧 | Created, needs build verification |
| Kafka consumer | `src/kafka/consumer.rs` | 🔧 | Created, needs build verification |
| Storage (Parquet + S3) | `src/storage.rs` | 🔧 | Created, needs build verification |
| Entrypoint | `src/main.rs` | 🔧 | Created, needs build verification |
| Dockerfile | `Dockerfile` | ⏳ | Not yet created |

---

## Decisions Made During Build

| Date | Decision | Rationale |
|------|----------|-----------|
| 2026-07-01 | Own consumer group `opsbucket-archiver` | Independent offset tracking; processing failures don't stall archival |
| 2026-07-01 | Parquet with ZSTD compression | Columnar format efficient for both storage and query; ZSTD offers best compression/speed trade-off for text-heavy data |
| 2026-07-01 | Minimal Parquet schema (5 columns: event_id, project_id, event_type, timestamp, raw_event) | Raw JSON preserves complete event data; key columns enable partition pruning without JSON parsing |
| 2026-07-01 | Hive-style partitioning `dt=YYYY-MM-DD` | Standard pattern for S3 data lakes; works with Athena, DuckDB, Spark, Trino out of the box |
| 2026-07-01 | Buffer-then-commit offset discipline | Same as processing: S3 upload must confirm before Kafka offset is committed |
| 2026-07-01 | No dedup in archiver | Duplicate rows in archive are acceptable; downstream tools can deduplicate by event_id. Not worth the complexity for backup data. |
| 2026-07-01 | `object_store` crate for S3 | Unified API (S3, GCS, local); auth via standard AWS credential chain; supports S3-compatible stores (R2, MinIO) via endpoint override |

---

## Known Issues

- Build not yet verified — may need dependency version adjustments
- Tests not yet written
- No graceful handling of S3 credential expiry (relies on object_store refresh)
