# OpsBucket Query — Session Replay API

## Overview

Session replay adds three read-only endpoints to the query service for listing and retrieving
recorded session replays. Replay data follows a separate storage path from analytical events:

- **Postgres** (`replay_sessions`, `replay_chunks`) — session metadata and chunk pointers
- **S3/MinIO** — raw rrweb event data, stored as gzipped JSON

The query service proxies chunk data from S3; it never stores replay data itself.

---

## Endpoints

### `GET /v1/query/replay/sessions`

List replay sessions for a project.

**Query params:**
- `projectId` (required) — project to list sessions for
- `status` (optional) — filter by session status (e.g. `"active"`, `"completed"`)
- `limit` (optional, default 50, max 200)
- `offset` (optional, default 0)

**Auth:** Bearer secret key with scope matching `projectId`

**Response:**
```json
[
  {
    "sessionId":    "ses_01j4km2n3p4q5r6s7t8u9v0w",
    "distinctId":   "usr_abc123",
    "startedAt":    "2026-06-29T10:00:00.000Z",
    "lastActivity": "2026-06-29T10:30:00.000Z",
    "chunkCount":   4,
    "status":       "completed"
  }
]
```

### `GET /v1/query/replay/sessions/:session_id/chunks`

List chunks for a specific session.

**Query params:**
- `projectId` (required)

**Auth:** Bearer secret key with scope matching `projectId`

**Response:**
```json
[
  {
    "windowId":   "win_001",
    "chunkSeq":   0,
    "s3Key":      "replay/proj_abc/ses_xyz/0.json.gz",
    "byteSize":   12842,
    "eventCount": 42,
    "isFinal":    false,
    "receivedAt": "2026-06-29T10:00:01.000Z"
  }
]
```

Chunks are ordered by `chunkSeq` ascending. `isFinal: true` means this was the last chunk
of the session (triggered by page unload or session timeout on the SDK side).

### `GET /v1/query/replay/sessions/:session_id/chunks/:chunk_seq`

Retrieve the raw rrweb event data for a single chunk.

**Query params:**
- `projectId` (required)

**Auth:** Bearer secret key with scope matching `projectId`

**Response:** `200 OK` with `Content-Type: application/json` and the raw rrweb event array
as the body. The data is proxied directly from S3 (MinIO in development).

**Errors:**
- `404` — chunk not found
- `502` — S3 fetch failed

---

## Data Flow

```
SDK  ──POST──>  Ingestion  ──Kafka──>  Retrace  ──S3──>  MinIO
                                                    │
                                                    └──Postgres──> Query (proxy)
```

1. SDK records rrweb events, batches them into chunks, sends to Ingestion
2. Ingestion validates and produces to `replay_events` Kafka topic
3. Retrace consumer reads topic, deduplicates, uploads to S3, writes metadata to Postgres
4. Query service reads Postgres for session/chunk lists, proxies chunk bodies from S3

---

## Configuration

| Variable | Required | Default | Description |
|----------|----------|---------|-------------|
| `S3_ENDPOINT` | Yes | — | S3/MinIO endpoint URL (e.g. `http://127.0.0.1:9000`) |

The per-chunk bucket is stored in the `replay_chunks` table rather than in config, allowing
chunks to span buckets in multi-region deployments.

---

## Storage Schema

### `replay_sessions`

| Column | Type | Description |
|--------|------|-------------|
| `project_id` | TEXT | Multi-tenant scope |
| `session_id` | TEXT | Unique session identifier |
| `distinct_id` | TEXT | Resolved user identity (nullable) |
| `started_at` | TIMESTAMPTZ | First event time |
| `last_activity` | TIMESTAMPTZ | Most recent event time |
| `chunk_count` | INTEGER | Total chunks in session |
| `status` | TEXT | `active` or `completed` |

### `replay_chunks`

| Column | Type | Description |
|--------|------|-------------|
| `project_id` | TEXT | Multi-tenant scope |
| `session_id` | TEXT | Parent session |
| `window_id` | TEXT | rrweb window identifier |
| `chunk_seq` | INTEGER | Monotonic chunk index within session |
| `s3_key` | TEXT | Object key in S3 |
| `s3_bucket` | TEXT | S3 bucket name |
| `byte_size` | INTEGER | Uncompressed byte size |
| `event_count` | INTEGER | Number of rrweb events in chunk |
| `is_final` | BOOLEAN | True if last chunk of session |
| `received_at` | TIMESTAMPTZ | Ingestion timestamp |
