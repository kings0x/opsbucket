# OpsBucket Session Replay — Ingestion Service Layer (Rust)

## Scope
The ingestion service already exists and handles `track`/`page`/`identify` events: accept payload, auth/validate, produce to Kafka. This document covers the **additions** needed to also accept rrweb replay batches. This is the same service, a new route — not a new binary.

---

## 1. New route

Add `POST /capture/replay`, separate from the existing `/capture` route used for standard events.

Reasons to split the route rather than branch on a field inside the same handler:
- Different body size limits apply (see §2).
- Different Kafka topic as the produce target.
- Different rate-limit bucket (replay traffic volume/shape is unrelated to standard event traffic; you don't want a burst of replay chunks to eat into an org's standard-event rate limit, or vice versa).
- Keeps the handler for the hot, high-QPS standard events path simple and unaffected by replay-specific logic.

Both routes share: the same auth middleware (project API key resolution), same project_id/org_id resolution, same general request logging.

---

## 2. Body size limits

Standard events are small (single-digit KB). Replay batches from the SDK are buffered up to ~64KB pre-compression (per doc 01 §3), which post-gzip is typically much smaller, but you should size the limit against the **uncompressed** worst case, not the typical case — a page with dense, non-repetitive DOM mutations compresses worse.

- Set `/capture/replay` max body size to **1MB** (generous headroom over the ~64KB target, protects against a misbehaving/outdated SDK build sending oversized batches).
- Leave `/capture`'s existing limit untouched.
- Reject with `413` anything over the limit — this is a client bug or abuse signal, not something to try to salvage.

---

## 3. Handling compression

The SDK sends `Content-Encoding: gzip` when it successfully compressed client-side (doc 01 §4), and omits it when it fell back to uncompressed.

**Do not decompress at the ingestion layer.** Pass the payload through to Kafka exactly as received (compressed or not), and carry the `Content-Encoding` state forward as a field on the Kafka message (see §5). Decompression happens once, downstream, in the Retrace processing service (doc 03) — decompressing at ingestion just to re-serialize into a Kafka message wastes CPU on your highest-throughput, most latency-sensitive service for no benefit. Ingestion's job is auth + validate + route, nothing more.

---

## 4. Validation (at ingestion, before producing to Kafka)

Reject with `400` if any of these are missing or malformed — cheap checks, do them before touching Kafka:
- `session_id` — must be present, non-empty.
- `window_id` — must be present, non-empty.
- `chunk_seq` — must be present, a non-negative integer.
- `distinct_id` — must be present (resolved the same way as standard events already do).
- `events` — must be present and a non-empty array.

Do **not** validate the internal shape of individual rrweb events at this layer (e.g. don't try to parse/understand rrweb's internal event type enum). That's wasted work on the hot path — if an individual event is malformed in a way that matters, that's Retrace's job to catch and route to its DLQ (doc 03 §6). Ingestion only checks the envelope.

---

## 5. Kafka message shape and topic

New topic: **`replay_events`**

Partition key: **`session_id`** (not `window_id`, not `distinct_id`). This guarantees all chunks for a session — across however many browser tabs/windows contributed to it — land on the same partition, which gives you the ordering guarantee Retrace needs to buffer and reconstruct per-session (doc 03 §2). This was the ordering discussion from earlier: partition-by-session is the load-bearing decision here.

Kafka message value (this is what ingestion constructs and produces — essentially the validated envelope plus a bit of ingestion-added metadata):

```json
{
  "session_id": "...",
  "window_id": "...",
  "chunk_seq": 4,
  "distinct_id": "...",
  "project_id": "...",
  "sdk_version": "...",
  "is_final": false,
  "content_encoding": "gzip",     // or "none" — tells Retrace whether to decompress
  "payload": "<raw bytes of events array, compressed or not, as received>",
  "ingested_at": "2026-07-08T12:34:56Z"
}
```

Don't unpack/re-encode the `events` array at ingestion — pass the raw payload bytes through as-is inside this envelope. Re-serializing JSON you don't need to touch just burns CPU on the ingestion hot path.

Topic config:
- Partitions: size for your expected concurrent session volume — a good starting heuristic is 2-4x your ingestion service's instance count, so Retrace consumers (which will run as a matching consumer group) can scale out without being partition-starved.
- Retention: short — replay events only need to live on the topic long enough for Retrace to consume and flush them (a few hours is plenty; this is not your source of truth, S3 + Postgres is). Don't reuse your standard-events topic's longer retention config here.

---

## 6. Rate limiting

Apply a **separate rate-limit bucket** per project/org for `/capture/replay`, independent from the standard `/capture` bucket. Reasoning: a project might have `sampleRate` dialed up and legitimately generate a lot of replay traffic without a correspondingly high `track()` call volume, and you don't want the two to cannibalize each other's limits.

---

## 7. What ingestion explicitly does NOT do for replay

- No decompression (§3).
- No deep event validation (§4).
- No buffering/batching of chunks (that's Retrace, doc 03) — ingestion produces one Kafka message per incoming HTTP batch, 1:1, no aggregation.
- No S3 or Postgres interaction at all — ingestion's replay responsibility ends at "produced to `replay_events` topic."