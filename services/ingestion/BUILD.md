# OpsBucket Ingestion — Build Log

This file is maintained by the agent (and any engineer) working on the ingestion service.
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

## Module Status

| Module                    | File                        | Status | Notes |
|---------------------------|-----------------------------|--------|-------|
| Package scaffold          | `Cargo.toml`                | ✅     | deps: deadpool-redis, tower-http, tracing-subscriber with env-filter |
| App bootstrap + graceful shutdown | `src/main.rs`        | ✅     | Thin: Config::load() → Server::new() → server.run() |
| Server struct             | `src/server.rs`             | ✅     | Server::new() + run(): init PG/Redis/Kafka with health checks, CORS, TraceLayer, graceful shutdown |
| Config loader             | `src/config.rs`             | ✅     | Config::load() — validates all env vars with expect(), fails fast at startup |
| DB — Postgres             | `src/db/postgres.rs`        | ✅     | init_pool() (max 20 connections), check_health() (SELECT 1), MockPgHealth |
| DB — Redis                | `src/db/redis.rs`           | ✅     | init_pool() (deadpool-redis), check_health() (PING), MockRedisHealth |
| DB — Health traits        | `src/db/mod.rs`             | ✅     | PgHealth + RedisHealth traits, re-exports |
| POST /v1/batch handler    | `src/routes/batch.rs`       | ✅     | Full flow: auth, rate limit, validate, stamp, kafka; Retry-After header |
| GET /health handler       | `src/routes/health.rs`      | ✅     | Checks PG + Redis + Kafka health, returns status object; 200 if all ok, 503 if degraded |
| Write-key auth            | `src/auth/write_key.rs`     | ✅     | Redis pool cache + Postgres fallback, AuthValidator trait |
| Payload validation        | `src/validation.rs`         | ✅     | 11 validation rules, ValidationError enum |
| Kafka producer wrapper    | `src/kafka/producer.rs`     | ✅     | rdkafka FutureProducer, EventProducer trait, MockProducer |
| Rate limiter              | `src/rate_limiter.rs`       | ✅     | Token bucket, per-key, stale eviction, RateLimitResult with retry-after |
| Dockerfile                | `Dockerfile`                | ✅     | Multi-stage, rust:1.81-slim |

---

## Test Status

| Test File / Module            | Status | Passing | Failing | Notes |
|-------------------------------|--------|---------|---------|-------|
| `auth::write_key`             | ✅      | 6       | 0       | In-memory TestStore |
| `validation`                  | ✅      | 11      | 0       | All validation rules |
| `kafka::producer`             | ✅      | 4       | 0       | MockProducer tests |
| `rate_limiter`                | ✅      | 5       | 0       | Token bucket tests, retry-after |
| `routes::batch` (integration) | ✅      | 8       | 0       | Full HTTP flow with mocks, no Docker needed |

---

## Build Output Status

| Output                     | Status | Notes |
|----------------------------|--------|-------|
| `cargo build`              | ✅      | Compiles with all deps |
| `cargo clippy`             | ✅      | Clean |
| `cargo fmt --check`        | ✅      | Clean |
| `cargo test`               | ✅      | 34 unit + integration tests |
| Docker image build         | ✅      | Multi-stage, rust:1.81-slim, ~200MB |

---

## Known Issues

*None yet.*

---

## Decisions Made During Build

| Date       | Decision | Rationale |
|------------|----------|-----------|
| 2026-06-29 | Rate limiter is in-memory, not Redis | Zero network hop, sub-ms check; resets on restart which is acceptable for v1 |
| 2026-06-30 | Separated config.rs, server.rs, db/ | Clean separation: Config::load() validates env vars at startup, Server::new()+run() wraps Axum setup, db/ has init + healthcheck traits |
| 2026-06-30 | Used deadpool-redis for Redis connection pool | User requested pool instead of single connection; health traits on db modules for testability |
| 2026-06-30 | PG/Redis health check via traits | Allows mocks in unit tests — all 34 tests run without Docker |
| 2026-06-30 | RateLimitResult returns retry_after_seconds | Token bucket computes exact retry-after duration; handler sends Retry-After header + JSON body |
| 2026-06-30 | CORS via tower-http CorsLayer::permissive() | Development-friendly; can be tightened in production |
| 2026-06-30 | TraceLayer::new_for_http() for distributed tracing | Auto-generates spans for each request; debug!() logs write_key and project_id |
| 2026-06-30 | Kafka health check via Producer::client().fetch_metadata() | rdkafka Producer trait lacks fetch_metadata — it lives on Client; call client().fetch_metadata() through Producer::client() |
| 2026-06-30 | Config.rs env-var validation | All required vars use .expect() — fails immediately on startup with a clear message about which var is missing |

---

## Commit History (Ingestion-relevant)

```
<latest> feat(ingestion): refactor with config, server, db health modules, Redis pool, Kafka health, Retry-After header
7146337 build(ingestion): add Dockerfile and first Postgres migration
28c1efa test(ingestion): add route integration tests
5b90510 feat(ingestion): implement POST /v1/batch and GET /health routes
a24632d feat(ingestion): implement in-memory rate limiter
a785935 feat(ingestion): implement Kafka producer wrapper with EventProducer trait
da50671 feat(ingestion): implement batch validation
e2ea67f feat(ingestion): implement write-key auth with Redis cache
8c1e90b chore(ingestion): scaffold module structure with empty stubs
```
