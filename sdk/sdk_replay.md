# OpsBucket Session Replay — SDK Layer (TypeScript/JavaScript)

## Scope
This document covers everything the OpsBucket JS/TS SDK needs to do to capture, batch, compress, and transmit rrweb session replay data. This sits alongside the existing `track()`, `page()`, `identify()` calls — it does not replace them. Replay is a parallel capture stream sharing the same `distinct_id`/`anonymous_id` identity resolution already in the SDK, but with its own transport path.

---

## 1. Identity: session_id and window_id

Two IDs matter here, and they are **not** the same as `distinct_id`.

- **`session_id`**: identifies one continuous user session. Generate as a UUIDv4 (or ULID, preferred — sortable, and lets you eyeball rough creation order in S3/Postgres without a join). Persist in `sessionStorage` (not `localStorage` — you want it to die when the tab closes, matching a "session").
- **`window_id`**: identifies a single browser tab/window within that session, since a user can have multiple tabs of your app open simultaneously, each running its own rrweb recorder. Generate a fresh UUIDv4 per tab load, held only in memory (module-level variable), not persisted.

**Session continuity rule**: if `sessionStorage` already has a `session_id` when the SDK initializes (e.g. user navigated to a new page within the same tab), reuse it. If the session has been idle longer than a configurable timeout (default 30 minutes — mirror your existing analytics session timeout if one exists), treat it as expired and mint a new `session_id`.

Store alongside the `session_id` in `sessionStorage`: a `last_activity_ts`. Update it on every rrweb event captured. On SDK init, if `now - last_activity_ts > timeout`, discard and start fresh.

---

## 2. rrweb recorder configuration

Use `rrweb.record()` with these settings as your defaults (expose overrides via SDK init options, but ship sane defaults so nobody has to think about this to get started):

```ts
rrweb.record({
  emit(event) {
    buffer.push(event);
  },
  maskAllInputs: true,          // default ON — privacy first, let orgs opt out per-field
  maskTextSelector: '[data-ob-mask]', // let customers tag sensitive DOM nodes explicitly
  blockSelector: '[data-ob-block]',   // fully exclude a node from recording (e.g. payment iframe)
  sampling: {
    mousemove: 50,       // throttle mousemove sampling to every 50ms, not every pixel event
    scroll: 150,         // throttle scroll events similarly
    input: 'last',       // only record final value on input, not every keystroke frame
  },
  recordCanvas: false,   // canvas recording is expensive and rarely needed — opt-in only
  collectFonts: false,
});
```

Also capture (via rrweb plugins or manual instrumentation, same emit callback):
- Console output (`getRecordConsolePlugin`) — capped at a reasonable max entries per session to avoid a noisy console.log loop blowing up your buffer.
- Uncaught JS errors (window `error` and `unhandledrejection` listeners) — tag these events distinctly so the metadata layer downstream can set a `has_error` flag without inspecting the full event stream.

---

## 3. Buffering and flush logic (client-side)

Do not send one HTTP request per rrweb event. Buffer in memory and flush on whichever threshold hits first:

| Trigger | Default value | Why |
|---|---|---|
| Time-based | every 10 seconds | keeps replay "close to live" for anyone building a live-view feature later, bounds data loss on crash |
| Size-based | ~64KB of buffered pre-compression JSON | keeps individual request payloads predictable regardless of how bursty the DOM mutation is |
| Page unload | `visibilitychange` → `hidden`, and `pagehide` | flush whatever's buffered immediately, don't wait for timers |
| Explicit stop | `sdk.stopReplay()` call | for SPA logout flows etc. |

**Do not rely solely on `beforeunload`** — it's unreliable across browsers/mobile. Use `visibilitychange` (fires when tab is backgrounded, which is the actual reliable signal on mobile Safari) as the primary "flush now" trigger, with `pagehide` as a backup.

Each flush produces one **batch**:

```ts
interface ReplayBatch {
  session_id: string;
  window_id: string;
  chunk_seq: number;        // monotonically increasing per window_id, starts at 0
  distinct_id: string;      // resolved same as your existing track() calls
  project_id: string;       // resolved from SDK init key, same as existing events
  sdk_version: string;
  events: RRWebEvent[];     // raw rrweb event objects, untouched
  is_final: boolean;        // true only on the flush triggered by unload/stop
}
```

`chunk_seq` is critical — it's how the processing service (Retrace, see doc 03) detects gaps, dedups retries, and orders chunks that may arrive out of order over the network. Increment it in memory per `window_id`, do not try to persist/recover it across reloads — a new `window_id` means a fresh `chunk_seq` starting at 0.

---

## 4. Compression

Compress client-side before sending — do not push that cost onto the ingestion service. rrweb payloads are highly repetitive DOM-diff JSON and compress extremely well (commonly 80-90% size reduction).

- Use `CompressionStream('gzip')` where available (all evergreen browsers as of 2026) via the Streams API. Fall back to a bundled `pako` gzip implementation for older targets if you need that support matrix.
- Set a request header `Content-Encoding: gzip` so the ingestion service knows not to re-decompress or double-compress.
- If compression fails for any reason (unsupported environment, error), send uncompressed and omit the header — ingestion must handle both paths (see doc 02).

---

## 5. Transport

- Primary: `navigator.sendBeacon(url, blob)` for unload-triggered flushes — this is the only transport that reliably completes after the page starts unloading.
- Regular interval/size-triggered flushes: `fetch()` with `keepalive: true`, standard JSON/gzip body.
- Endpoint: a **separate path** from your existing `/capture` endpoint used for `track`/`page`/`identify` — e.g. `/capture/replay`. Same auth (project API key in header, same as existing SDK calls), different route so the ingestion service can apply replay-specific body size limits and routing (see doc 02).
- **Retry policy**: on transport failure (network error, non-2xx), retry the batch up to 3 times with exponential backoff (500ms, 2s, 8s). After exhausting retries, drop the batch — do not accumulate a growing backlog in memory indefinitely, that risks an unbounded memory leak client-side on a bad network. Log a dropped-batch metric via your existing SDK internal telemetry if you have one.
- **Ordering caveat**: because of retries and `sendBeacon`'s fire-and-forget nature, batches can arrive at the ingestion service out of order. This is fine — `chunk_seq` handles reordering downstream. The SDK's only job is to make sure `chunk_seq` is correct and monotonic per `window_id`.

---

## 6. Privacy / masking controls exposed to the customer

Expose these as SDK init options, since this is customer-facing data capture and orgs will have different compliance needs:

```ts
interface ReplayOptions {
  enabled: boolean;              // default false — replay is opt-in per project
  maskAllInputs?: boolean;       // default true
  maskTextSelector?: string;
  blockSelector?: string;
  sampleRate?: number;           // 0.0–1.0, e.g. 0.1 = record 10% of sessions, for cost control
  recordCanvas?: boolean;
}
```

`sampleRate` should be evaluated once per new `session_id` (not per event) — decide at session start whether this session records at all, using `Math.random() < sampleRate`, and store that decision in the same `sessionStorage` entry as `session_id` so it's consistent across page loads within the session.

---

## 7. What NOT to build into the SDK

- No local persistence/retry-across-reload of unsent batches (e.g. via IndexedDB) for v1 — adds real complexity for a fairly rare edge case (tab crash mid-flush). Note it as a future improvement, don't build it now.
- No client-side session stitching across devices — that's an identity-resolution problem for `identify()`, out of scope for replay itself.