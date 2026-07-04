# OpsBucket Audit Observations

Date: 2026-07-04

Scope: repo-wide review of architecture/docs, SDK, ingestion, processing, query, archiver, infra, CI, tests, and obvious security/data-loss risks.

## Executive Summary

The SDK and ingestion unit suites are mostly healthy, but the backend pipeline is not production-ready yet. The biggest risks are:

- Query SQL is built by string interpolation instead of parameter binding.
- Processing and query integration tests currently fail against local Docker infra.
- The archiver can commit Kafka offsets without flushing buffered events to S3, which can lose archived raw events.
- Processing can silently drop events when Redis dedup fails.
- CI/e2e wiring has drifted from the repo layout and likely cannot run as written.
- Several service docs/build logs say things are complete that are not actually passing today.

The current worktree was already dirty before this report: `.gitignore` is modified, and `infra/docker-compose.yml` also shows modified. I did not intentionally edit those files.

## Commands Run

### Passing

- `pnpm -C sdk test`
  - 80 tests passed.
  - Emits a jsdom navigation warning in `autocapture.test.ts`, but Vitest exits 0.
- `pnpm -C sdk typecheck`
  - Passed.
- `pnpm -C sdk build`
  - Passed; generated `dist/index.js`, `dist/index.cjs`, declarations and maps.
- `pnpm -C sdk lint`
  - Passed.
- `cargo test -p opsbucket-ingestion`
  - 34 tests passed.
- `cargo test -p opsbucket-archiver`
  - Passed, but there are 0 tests.
- `cargo test -p opsbucket-query` unit portion
  - 54 unit tests passed before the integration test binary ran.

### Failing

- `cargo fmt --check`
  - Failed on formatting in `services/archiver/src/kafka/consumer.rs`, `services/e2e/src/helpers.rs`, `services/e2e/src/scenarios.rs`, `services/e2e/src/setup.rs`, `services/processing/src/kafka/consumer.rs`, and `services/query/src/routes/events.rs`.
- `cargo test --workspace`
  - Failed in `opsbucket-processing`.
  - 13 passed, 5 failed in the first run.
  - Failures were in `identity::handlers` and `identity::resolver`, with Postgres/Redis connection reset, aborted, and pool timeout errors.
- `cargo test -p opsbucket-processing --lib -- --test-threads=1`
  - Failed: 12 passed, 6 failed.
  - Failed tests:
    - `dedup::tests::different_ids_are_both_kept`
    - `identity::handlers::tests::identify_invalidates_cache`
    - `identity::handlers::tests::identify_upserts_alias_row`
    - `identity::handlers::tests::re_identify_updates_existing_row`
    - `identity::resolver::tests::event_with_user_id_skips_lookup`
    - `identity::resolver::tests::no_alias_returns_none`
  - Root symptom: hard-coded live Redis/Postgres tests are timing out or getting connection resets.
- `cargo test -p opsbucket-query --test e2e -- --test-threads=1`
  - Failed: 0 passed, 32 failed.
  - Every failure happened during test setup at `services/query/tests/common/mod.rs:48`, connecting to Postgres.
  - Errors included `PoolTimedOut`, connection reset, and connection aborted.
- `cargo clippy --workspace --all-targets -- -D warnings`
  - Failed.
  - `services/e2e` has multiple `needless_borrows_for_generic_args`, `unnecessary_map_or`, and `type_complexity` errors.
  - `services/query/tests/common/mod.rs` has dead code and `vec_init_then_push` errors.

## Incomplete Or Not Actually Verified

1. Archiver has no tests.
   - `cargo test -p opsbucket-archiver` runs 0 tests.
   - `services/archiver/BUILD.md` says tests are not yet written, despite the module table reading as built.

2. Processing consumer integration tests are not implemented.
   - `services/processing/BUILD.md` still marks Kafka consumer integration as not started.
   - This is the most important correctness boundary: offset commit after ClickHouse insert.

3. Query integration tests exist but currently fail completely.
   - `services/query/tests/e2e.rs` has 32 tests.
   - All fail at Postgres setup in the current local run.

4. Root docs are stale.
   - Root `CONTEXT.md` still says several service CONTEXT files are pending and does not include the `archiver` or `e2e` service in the architecture.
   - `infra/CONTEXT.md` says the identity table is `identities`, but migrations and code use `identity_aliases`.

5. CI e2e workflow appears stale or broken.
   - `.github/workflows/e2e.yml` references `tests/e2e/**`, `tests/e2e/seed.sql`, and `tests/e2e/scenarios.sh`.
   - The repo has `services/e2e`, not `tests/e2e`.
   - The workflow creates `raw-events` with 1 partition, while `infra/topics.sh` creates 12.

6. Render config does not include the archiver.
   - `render.yaml` defines ingestion, processing, query, and migrator only.
   - `services/archiver` exists and has a Dockerfile but is not deployed by the blueprint.

## Security Issues

1. Query SQL injection risk is real.
   - Files: `services/query/src/queries/funnel.rs`, `retention.rs`, `segment.rs`, `raw_events.rs`.
   - These builders use `format!` plus `sql_escape()` rather than ClickHouse parameter binding.
   - Examples:
     - `services/query/src/queries/funnel.rs:7`, `:24`, `:44`
     - `services/query/src/queries/segment.rs:50`, `:69`, `:71`, `:107`
     - `services/query/src/queries/raw_events.rs:14`, `:40`, `:53`
   - Escaping only `'` with backslash is not a sufficient SQL injection defense.

2. Query identity patch is not tenant-scoped.
   - File: `services/query/src/identity.rs:23`
   - It queries `WHERE anonymous_id = ANY($1)` without `project_id`.
   - If two tenants have the same `anonymous_id`, raw event identity patching can assign a user from the wrong project.

3. Ingestion logs write keys at debug level.
   - File: `services/ingestion/src/routes/batch.rs:116`
   - `tracing::debug!(write_key = %write_key_str, ...)` can leak public write keys into logs. Write keys are browser-exposed, but they still authorize ingestion and should not be logged.

4. Query and ingestion use permissive CORS.
   - Ingestion: `services/ingestion/src/server.rs:62`
   - Query: `services/query/src/main.rs:49`
   - Query is secret-key protected, but permissive CORS increases blast radius if a secret key is ever used from a browser or leaked.

5. Query secret key is a single shared credential.
   - This is documented as a v1 limitation, but it is still a security risk: no per-project scope, no per-customer revocation, no role separation.

6. Local secrets/config hygiene needs attention.
   - `.env` exists in the workspace and is ignored by the current `.gitignore`.
   - `nul` exists as a zero-byte local file and is ignored.
   - `git ls-files .env nul` returned no tracked files, but confirm these were never committed historically before pushing.

7. Query-param write key for `sendBeacon` is architecturally required, but risky.
   - SDK: `sdk/src/transport.ts:102`
   - Ingestion accepts it: `services/ingestion/src/routes/mod.rs:205`
   - Query params often land in proxy logs. This may be acceptable for public write keys, but production logs should redact `write_key`.

## Data Loss / Correctness Risks

1. Archiver can commit offsets before flushing buffered events.
   - File: `services/archiver/src/kafka/consumer.rs`
   - `process_batch()` only calls `flush()` when buffer length reaches batch size or estimated size reaches max file size (`:148-150`).
   - It commits offsets afterward even if the buffer is below thresholds and was not flushed (`:153-162`).
   - On crash after that commit, those raw events are lost from the archive.

2. Processing dedup silently drops events on Redis errors.
   - File: `services/processing/src/dedup.rs:16`
   - `SETNX` errors are mapped through `.unwrap_or(false)`, making Redis failure look like “duplicate event”.
   - That drops new events instead of failing the batch and preserving Kafka offset for retry.

3. SDK batcher removes events before transport success.
   - File: `sdk/src/batcher.ts:56-58`
   - `flush()` splices the queue and saves the empty queue before the transport succeeds.
   - The catch block ignores transport errors.
   - This contradicts the SDK context requirement that failed flushes leave events queued.

4. SDK `reset()` flushes before identity reset, even though SDK context says reset should not flush the queue.
   - File: `sdk/src/index.ts:74`
   - Context says reset does not flush; implementation does.

5. Processing and archiver shutdown flags are disconnected.
   - Processing: `services/processing/src/main.rs:35-44`
   - Archiver: `services/archiver/src/main.rs:32-41`
   - `AtomicBool` is set on Ctrl+C, but the consumers do not read that atomic. `consumer.run()` loops on its own internal `running` flag, and `shutdown()` is never called.

6. Query date-range limit ignores config.
   - `services/query/src/config.rs:33` loads `MAX_DATE_RANGE_DAYS`.
   - `services/query/src/queries/builder.rs` uses a hard-coded `MAX_DATE_RANGE_DAYS`.
   - Also, `num_days()` truncates partial days, allowing 366 days plus almost 24 hours.

7. Raw event query params are not validated like other query inputs.
   - File: `services/query/src/routes/events.rs`
   - It accepts `projectId`, optional `eventName`, optional `userId`, and cursor directly into SQL builder without the shared string length/non-empty guard.

8. Query cache responses do not set `cachedAt`.
   - Cache hits return the stored response unchanged.
   - Fresh responses store `cached_at: None`, so clients cannot tell cache hits despite the API spec.

## Test Problems

1. Processing tests are not isolated unit tests.
   - `services/processing/src/dedup.rs`, `identity/handlers.rs`, and `identity/resolver.rs` connect to hard-coded `localhost:6379` and `localhost:5432`.
   - They call `FLUSHDB` and delete shared rows, so parallel or repeated runs are brittle.

2. Query integration tests rely on a shared local Postgres/ClickHouse/Redis stack.
   - `services/query/tests/e2e.rs` documents `--test-threads=1`.
   - Even sequentially, all 32 failed here at Postgres connection setup.

3. Query e2e setup uses global tables and truncates shared ClickHouse state.
   - `services/query/tests/common/mod.rs` truncates the `events` table for each test.
   - This makes default parallel execution unsafe.

4. Archiver has no behavioral tests.
   - No test covers Parquet contents, S3 key partitioning, offset commit discipline, or malformed event handling.

5. CI quality gates miss or misroute important paths.
   - E2E workflow watches `tests/e2e/**`, but e2e code is in `services/e2e/**`.
   - Query workflow does not include `.github/workflows/query.yml` in its own path trigger.

## Infrastructure / Deployment Issues

1. Docker Compose images are not fully pinned.
   - Redpanda uses `latest` in `infra/docker-compose.yml`.
   - This can change local behavior without code changes.

2. Local Docker containers expose databases on all interfaces.
   - Postgres, Redis, ClickHouse, and Redpanda use `0.0.0.0` published ports.
   - Fine for local dev on a trusted machine, but risky on shared networks.

3. No healthchecks in `infra/docker-compose.yml`.
   - `infra/CONTEXT.md` documents healthchecks, but the compose file does not define them.

4. Render service ordering/migration safety is unclear.
   - `render.yaml` defines a migrator worker, but no explicit dependency/order guarantees before web/worker services start.

5. ClickHouse schema and docs drift.
   - `infra/CONTEXT.md` talks about ordering by `(project_id, event, timestamp)`.
   - Actual schema uses `ORDER BY (project_id, event_name, timestamp)`.

## What Looks Healthy

- SDK tests, typecheck, build, and lint pass.
- Ingestion tests pass.
- Ingestion stamps `sent_at` onto each raw event, satisfying processing’s prerequisite.
- Postgres migrations define `write_keys`, `projects`, and `identity_aliases`.
- ClickHouse schema includes flattened event fields, properties/traits maps, and TTL.

## Suggested Fix Order

1. Fix data-loss bugs first:
   - Archiver offset commit before flush.
   - Processing dedup swallowing Redis errors.
   - SDK batcher removing events before successful send.

2. Replace query SQL interpolation with ClickHouse parameter binding.

3. Fix query identity patch to include `project_id`.

4. Stabilize tests:
   - Make processing tests use testcontainers or isolated test DB/Redis namespaces.
   - Make query e2e setup reliable and explicitly serial in CI.
   - Add archiver tests.

5. Fix formatting and clippy so CI-style checks are clean.

6. Repair CI/e2e workflow paths and partition count drift.

7. Update root and infra docs to match the actual services and schemas.

## Second-Pass Deep Audit Additions

These findings came from a deeper implementation read after the first report was written.

### SDK Delivery Semantics

1. `sendBeacon` is implemented but not wired into unload flushing.
   - `sdk/src/transport.ts:19-22` supports `send(batch, useBeacon = false)`.
   - `sdk/src/batcher.ts:102` calls plain `this.flush()` on `visibilitychange`.
   - `sdk/src/index.ts:28` wires the batcher as `(batch) => transport!.send(batch)`, always using the default `useBeacon = false`.
   - Result: the tested `sendBeacon` path is effectively dead in normal SDK usage.

2. `beforeunload` is not registered.
   - SDK context requires both `visibilitychange` and `beforeunload`.
   - `sdk/src/batcher.ts:105` only registers `visibilitychange`.
   - Browser/tab close cases can be missed or use cancellable `fetch`.

3. Failed normal flushes are permanently lost from memory and persisted queue.
   - `sdk/src/batcher.ts:56-58` drains the queue and persists the empty queue before the transport result is known.
   - `sdk/src/batcher.ts:62-64` catches transport errors but does not restore the events.
   - This makes retry behavior in `Transport` less useful because process/tab failure during retries loses the queue.

4. Transport’s `dropBatch()` only logs; it does not update persisted storage.
   - `sdk/src/transport.ts:123-125`
   - The SDK context says dropped batches should be removed from localStorage. In the current implementation the batcher has already cleared storage, so this works accidentally for normal flushes but not as an explicit transport contract.

5. Payload size check runs after adding the new event.
   - `sdk/src/batcher.ts:38-51`
   - Context says flush before adding an oversized event/batch. Current behavior appends, persists, then flushes. For a single huge event, this can still produce a 413/drop path.

6. `reset()` behavior contradicts the SDK context.
   - `sdk/src/index.ts:84-88` flushes before reset.
   - SDK context says reset does not flush the queue.

### Ingestion

1. Auth infrastructure errors are collapsed into invalid-key responses.
   - `services/ingestion/src/auth/mod.rs:24-27`
   - `validate_write_key()` returns `Result<Option<String>>`, but `RedisPgAuth::validate()` does `.unwrap_or(None)`.
   - Redis/Postgres outages become `None`, and `post_batch()` returns 401 instead of 5xx. That hides infrastructure outages from clients and monitoring.

2. `sentAt` is not validated.
   - `services/ingestion/src/validation.rs` validates each event’s `originalTimestamp`, but not the batch envelope `sentAt`.
   - Processing falls back to `received_at` if `sent_at` cannot parse, silently degrading timestamp correction.

3. Empty batches appear to be accepted.
   - `validate_batch()` rejects only `payload.batch.len() > 500`.
   - A request with `"batch": []` can authenticate, rate-limit, stamp zero events, and call Kafka with an empty slice.

4. IP extraction trusts forwarded headers from any caller.
   - `services/ingestion/src/routes/batch.rs:30-41`
   - This is usually acceptable only behind a trusted proxy that strips client-supplied forwarding headers. Otherwise clients can spoof `context.ip`.

5. Ingestion Kafka producer can partially enqueue a batch and then return 502.
   - `services/ingestion/src/kafka/producer.rs:37-47`
   - Events are sent one by one. If event N fails, earlier events may already be in Kafka while the SDK sees a 502 and retries the whole batch. Downstream dedup is expected to absorb this, but it increases duplicate pressure and makes ingestion response semantics non-atomic.

### Processing

1. Failed identify events can disappear before their DLQ threshold.
   - `services/processing/src/kafka/consumer.rs:202-224`
   - If `handle_identify()` fails and `should_dlq()` returns false, the identify event is neither inserted into ClickHouse nor sent to DLQ, but the batch can still proceed and commit offsets.
   - This can lose identity events for transient Postgres/Redis failures.

2. Deserialization DLQ path can commit malformed offsets immediately.
   - `services/processing/src/kafka/consumer.rs:135-145` sends malformed payloads to DLQ.
   - `services/processing/src/kafka/consumer.rs:158-170` commits offsets when the batch is DLQ-only.
   - That is acceptable only if DLQ send succeeds. `send_to_dlq()` logs DLQ failure but does not propagate it, so a DLQ send failure can still lead to offset commit and permanent loss of malformed events.

3. DLQ retry budget itself swallows Redis failures.
   - `services/processing/src/kafka/consumer.rs:241-253`
   - Redis `INCR` errors default to count `1`; `EXPIRE` errors default to success. This can prevent DLQ routing during Redis outages and keep failing events in an unclear state.

4. ClickHouse timestamp fields are `u32`.
   - `services/processing/src/clickhouse/models.rs:10-12`
   - Dates after February 2106 overflow Unix seconds in `u32`. Not urgent for product analytics, but a needless future landmine; `i64` or chrono row types would be cleaner.

5. `allow.auto.create.topics = true` is enabled for the processing consumer and DLQ producer.
   - `services/processing/src/kafka/consumer.rs:55`
   - `services/processing/src/kafka/dlq.rs:27`
   - Misconfigured topic names could silently create wrong topics instead of failing fast.

### Query

1. Route-level JSON extraction errors bypass the documented error shape.
   - Query handlers use `Json(spec): Json<...>` directly.
   - Axum extractor rejections will not go through `AppError`, so malformed/empty JSON may not return `{ "error": "...", "detail": "..." }` consistently.

2. Segment `contains` is likely wildcard-injection-prone.
   - `services/query/src/queries/segment.rs:18-20`
   - User values are wrapped in `%...%` for `LIKE`, but `%`, `_`, and backslash are not escaped. A value like `%` matches everything.

3. `withinDays` is not bounded in segment conditions.
   - `services/query/src/queries/segment.rs:45`
   - `within_days` comes from user input and can create huge scans. Shared validation currently checks only condition count and limit.

4. `retention.periods` is not bounded.
   - `services/query/src/queries/builder.rs` only checks `periods > 0`.
   - A very large period count can produce very large responses even for a small query result.

5. Raw events limit silently clamps instead of rejecting invalid values.
   - `services/query/src/routes/events.rs:24`
   - `limit` is `unwrap_or(50).min(200)`. A caller asking for `limit=1000000` gets 200 rather than a clear 400. This is not a security issue by itself, but it differs from stricter validation elsewhere.

6. Cache key does not include auth/tenant context beyond the request body.
   - The current secret key is global, so this is okay for v1.
   - If per-project/per-key auth is added, cache keys need to include the effective authorization scope, not only the JSON spec.

### Archiver

1. Archive partition date uses flush time, not event time.
   - `services/archiver/src/kafka/consumer.rs:112-116`
   - The context says files are partitioned by date, but all events in a flush go under `dt=<now>`. Late/backfilled events will be stored under ingestion/archive date rather than event date.

2. Parquet timestamp uses `sent_at`, not corrected event timestamp or `received_at`.
   - `services/archiver/src/storage.rs:25-28`
   - This may be acceptable for raw archive, but the context describes it as “Unix milliseconds from sentAt”; document that archive date and timestamp semantics differ from ClickHouse query timestamp.

3. Archiver skips malformed Kafka messages without DLQ.
   - This is documented as intentional, but combined with committing offsets it means malformed raw events are unrecoverable unless Kafka retention still has them.

### Docker / Deployment

1. Dockerfiles may fail depending on build context.
   - Every service Dockerfile uses `WORKDIR /app`, `COPY . .`, then `cargo build -p ...`.
   - If Render or local Docker builds use repo root as context with `-f services/<name>/Dockerfile`, `/app` will not contain `Cargo.toml`; it will be at `/app/services/Cargo.toml`.
   - These Dockerfiles likely require the build context to be `services/`, but `render.yaml` does not make that explicit.

2. Dockerfiles install only `ca-certificates`.
   - Runtime images for rdkafka/clickhouse/sqlx may also need native libs depending on static/dynamic linking outcomes.
   - This should be verified with actual Docker builds, not assumed from `cargo build`.

3. Rust toolchain drift.
   - Dockerfiles pin `rust:1.81-slim`.
   - Local clippy output showed Rust 1.92-era lints. CI uses `dtolnay/rust-toolchain@stable`, not `rust-toolchain.toml`.
   - This can make local/Docker/CI lint behavior diverge.

## Query E2E Fix Log - 2026-07-04

I continued the failing query e2e investigation against the local Docker infra and brought the full query e2e suite to green.

### Initial failures reproduced

- `cargo test -p opsbucket-query --test e2e test_health -- --test-threads=1 --nocapture`
  - Initially failed while connecting to Postgres through `localhost`.
- After fixing connectivity and fixture encoding, the full e2e suite reached `26 passed, 6 failed`.
  - Failing tests were `test_events_identity_join`, `test_events_pagination`, `test_funnel_multi_step`, `test_retention_weekly`, `test_segment_multi_condition_and`, and `test_segment_truncated`.

### Fixes made

1. Query test infra connections now use explicit loopback addresses and bounded Postgres pools.
   - `services/query/tests/common/mod.rs`
   - Replaced `localhost` URLs with `127.0.0.1` for Postgres, Redis, and ClickHouse.
   - Replaced unbounded/default Postgres pool creation with `PgPoolOptions::new().max_connections(5)`.

2. Query ClickHouse fixtures now match the production row encoding.
   - `services/query/tests/common/mod.rs`
   - Changed fixture `DateTime` columns from chrono values to Unix seconds as `u32`, matching `services/processing/src/clickhouse/models.rs`.

3. Query handlers now authenticate before parsing JSON bodies and return the app error shape for malformed or empty bodies.
   - `services/query/src/routes/funnel.rs`
   - `services/query/src/routes/retention.rs`
   - `services/query/src/routes/segment.rs`
   - Handlers now accept raw `Bytes`, run auth first, then parse with `serde_json::from_slice()`.

4. Funnel fixture data now matches the stated funnel window.
   - `services/query/tests/common/mod.rs`
   - `anon_b` second funnel event now falls within the 3600 second window, so the expected second-step count is reachable.

5. Segment multi-condition fixture data now contains both required events.
   - `services/query/tests/common/mod.rs`
   - Added the missing `Feature Used` event for `anon_g`/`usr_g`, so it can satisfy both the event-count and trait conditions.

6. Retention weekly bucketing now uses Monday weeks.
   - `services/query/src/queries/retention.rs`
   - Replaced default `toStartOfWeek(...)` with `toStartOfWeek(..., 1)` for weekly cohort and event buckets.

7. Segment truncation detection now fetches one extra row per condition.
   - `services/query/src/queries/segment.rs`
   - Subqueries use `limit + 1` internally so the route can correctly set `truncated = true`.

8. Raw events identity lookup now supports late identity resolution and tenant scoping.
   - `services/query/src/routes/events.rs`
   - `services/query/src/queries/raw_events.rs`
   - `services/query/src/identity.rs`
   - `services/query/src/lib.rs`
   - When querying raw events by `userId`, the route now looks up that user's anonymous IDs for the same `project_id` and includes them in the ClickHouse filter.
   - `EventRow` now carries an internal, non-serialized `project_id`.
   - `identity::patch_identity()` now filters aliases by `project_id`, fixing the cross-tenant alias leak noted earlier in the audit.
   - Raw events now select `project_id` from ClickHouse so identity patching can be scoped correctly.

9. Raw events pagination response now matches the e2e contract.
   - `services/query/src/lib.rs`
   - `EventsResponse.next_cursor` serializes as `next_cursor`, matching the existing tests.

### Verification

- `cargo test -p opsbucket-query --lib`
  - Result: `54 passed, 0 failed`.
- `cargo test -p opsbucket-query --test e2e -- --test-threads=1 --nocapture`
  - Result: `32 passed, 0 failed`.
  - This was run again after formatting the touched query files.
- `cargo fmt -p opsbucket-query -- --check`
  - Result: passed.
- `cargo fmt --check`
  - Result: still fails because of pre-existing formatting drift in unrelated workspace files under `services/archiver`, `services/e2e`, and `services/processing`.
  - I did not reformat those unrelated files during this query e2e fix.

### Remaining notes

- Query e2e emits one warning: `redis_manager` in `services/query/tests/common/mod.rs` is currently unused.
- Cargo also warns that `sqlx-postgres v0.7.4` contains code that will be rejected by a future Rust version.

## Fix Pass - 2026-07-04

This pass fixed and verified the highest-risk data-loss/test-stability items from the audit.

### Fixed

1. SDK delivery semantics
   - `sdk/src/batcher.ts`
   - `sdk/src/transport.ts`
   - `sdk/src/index.ts`
   - Failed transient flushes now restore drained events to memory and persisted queue.
   - `visibilitychange` hidden and `beforeunload` now flush with `sendBeacon`.
   - `reset()` no longer flushes before rotating identity.
   - Transport now rejects after exhausted transient failures so the batcher can preserve retryable events.

2. Ingestion validation/auth hygiene
   - `services/ingestion/src/auth/mod.rs`
   - `services/ingestion/src/routes/batch.rs`
   - `services/ingestion/src/validation.rs`
   - Auth infrastructure errors now return 503 instead of being collapsed into invalid-key 401s.
   - Write keys are no longer logged at debug level.
   - Empty batches and invalid/missing `sentAt` are rejected.
   - Forwarded IP headers are ignored unless `TRUST_PROXY_HEADERS=true`.

3. Processing data-loss risks
   - `services/processing/src/dedup.rs`
   - `services/processing/src/kafka/consumer.rs`
   - `services/processing/src/kafka/dlq.rs`
   - Dedup Redis errors now propagate.
   - Events are marked as seen only after ClickHouse write or DLQ handoff succeeds.
   - Identify failures below the DLQ threshold now fail the batch instead of disappearing before offset commit.
   - DLQ send failures and Redis retry-budget failures now prevent offset commit.
   - `allow.auto.create.topics` is now disabled for the processing consumer and DLQ producer.
   - Processing shutdown flag is now connected to the consumer loop.

4. Processing test stability
   - `services/processing/src/identity/handlers.rs`
   - `services/processing/src/identity/resolver.rs`
   - Identity tests now use `127.0.0.1` and bounded `PgPoolOptions`, matching the query/e2e connection fix.

5. Archiver offset and partition correctness
   - `services/archiver/src/kafka/consumer.rs`
   - The archiver now flushes non-empty batches before committing offsets.
   - Archive keys now partition by event `original_timestamp` date, falling back to `received_at` and then current time.
   - Archiver shutdown flag is now connected to the consumer loop.

6. Query hardening
   - `services/query/src/queries/builder.rs`
   - `services/query/src/routes/events.rs`
   - `services/query/src/queries/segment.rs`
   - `services/query/src/routes/funnel.rs`
   - `services/query/src/routes/retention.rs`
   - `services/query/src/routes/segment.rs`
   - Runtime validation now honors `MAX_DATE_RANGE_DAYS`.
   - `retention.periods` and segment `withinDays` are bounded.
   - Raw-events params are validated; invalid `limit` returns 400 instead of silently clamping.
   - Segment `contains` escapes `%`, `_`, and backslash.
   - Cached query responses now set `cachedAt` on cache hits.

7. E2E and lint cleanup
   - `services/e2e/src/*`
   - `services/query/tests/common/mod.rs`
   - Standalone e2e host endpoints use `127.0.0.1`.
   - Clippy failures in e2e and query test helpers were fixed.

### Verified

- `pnpm -C sdk test` -> 82 passed.
- `pnpm -C sdk typecheck` -> passed.
- `pnpm -C sdk lint` -> passed.
- `pnpm -C sdk build` -> passed.
- `cargo fmt --check` from `services/` -> passed.
- `cargo clippy --workspace --all-targets -- -D warnings` -> passed.
- `cargo test --workspace -- --test-threads=1` -> passed, including query e2e.
- `cargo run -p opsbucket-e2e -- --no-cleanup` -> 77 passed, 0 failed.

### Still Open / Needs Another Pass

1. Query SQL builders still use string interpolation. Some validation/escaping was tightened, but this is not a full ClickHouse parameter-binding refactor.
2. Query still uses a single global secret key. Per-project scoped query credentials need a product/API design.
3. Query and ingestion CORS policy still needs an environment-specific production allowlist decision.
4. Archiver still has no behavioral tests for Parquet contents, R2/S3 keying, malformed events, or offset discipline.
5. Archiver malformed-message policy still intentionally skips malformed Kafka messages rather than DLQing them; decide whether raw archive needs its own DLQ policy.
6. Processing ClickHouse timestamp fields are still `u32`; changing this needs schema/client/test coordination.
7. Render deployment ordering and archiver inclusion still need deployment design changes.
8. Dockerfile build-context/runtime-library concerns still need actual Docker build verification.
9. CI e2e workflow path drift still needs workflow repair.
10. Root/infra docs still need updates, but `CONTEXT.md` and `BUILD.md` remain ignored by project policy.
