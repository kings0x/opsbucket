#!/usr/bin/env bash
# ─── OpsBucket E2E Test Scenarios ──────────────────────────────────────────────
#
# Runs test scenarios against already-running infrastructure.
# Used by the CI workflow (where Docker services are provided via GHA `services`).
# Called by run.sh for local dev (after infra is set up).
#
# Usage:
#   bash tests/e2e/scenarios.sh
#
# Expects:
#   Ingestion on localhost:8080, ClickHouse on localhost:8123
#   Valid write key "wk_test_valid_key_12345" and project "proj_test" seeded
#   Kafka topics raw-events, raw-events-dlq created
#   ClickHouse schema applied
#   Processing consumer running
#
# ────────────────────────────────────────────────────────────────────────────────
set -euo pipefail

ROOT_DIR="$(cd "$(dirname "$0")/../.." && pwd)"
INFRA_DIR="$ROOT_DIR/infra"

INGEST_PORT=${INGEST_PORT:-8080}
CLICKHOUSE_HTTP_PORT=${CLICKHOUSE_HTTP_PORT:-8123}
WRITE_KEY=${WRITE_KEY:-wk_test_valid_key_12345}
PROJECT_ID=${PROJECT_ID:-proj_test}

PASS=0
FAIL=0
TIMEOUT=120

green()  { printf "\033[32m%s\033[0m\n" "$1"; }
red()    { printf "\033[31m%s\033[0m\n" "$1"; }
bold()   { printf "\033[1m%s\033[0m\n" "$1"; }
info()   { printf "  ▸ %s\n" "$1"; }
pass()   { PASS=$((PASS+1)); green "  ✓ PASS: $1"; }
fail()   { FAIL=$((FAIL+1)); red "  ✗ FAIL: $1"; }

send_event() {
  local batch_json=$1
  curl -s -X POST "http://localhost:$INGEST_PORT/v1/batch" \
    -H "Authorization: Bearer $WRITE_KEY" \
    -H "Content-Type: application/json" \
    -d "$batch_json"
}

send_event_raw() {
  local batch_json=$1 key=$2
  curl -s -X POST "http://localhost:$INGEST_PORT/v1/batch" \
    -H "Authorization: Bearer $key" \
    -H "Content-Type: application/json" \
    -d "$batch_json"
}

query_ch() {
  local sql=$1
  curl -s -u "default:opsbucket" \
    "http://localhost:$CLICKHOUSE_HTTP_PORT/?readonly=0" \
    --data "$sql" 2>/dev/null | tr -d '\n'
}

wait_for_events() {
  local expected=$1 label=$2 timeout=${3:-$TIMEOUT}
  info "waiting for $expected events in ClickHouse ($label) ..."
  local i=0
  while true; do
    local count
    count=$(query_ch "SELECT count() FROM events" 2>/dev/null)
    if [ -n "$count" ] && [ "$count" -ge "$expected" ] 2>/dev/null; then
      green "  $expected events present in ClickHouse"
      return 0
    fi
    i=$((i+1))
    if [ $i -ge "$timeout" ]; then
      fail "timeout waiting for $expected events ($label) — got count=$count"
      return 1
    fi
    sleep 1
  done
}

# ── Scenario 1: Happy Path (Track) ──
bold "Scenario 1: Happy Path — Track Events"
TRACK_BATCH='{
  "sentAt": "2026-06-30T10:05:30.000Z",
  "batch": [{
    "messageId": "e2e-track-001",
    "type": "track",
    "anonymousId": "anon_e2e_1",
    "userId": null,
    "originalTimestamp": "2026-06-30T10:05:28.000Z",
    "context": {
      "library": {"name": "@opsbucket/browser", "version": "0.1.0"},
      "page": {"url": "https://app.example.com/dashboard", "path": "/dashboard", "referrer": "https://google.com", "title": "Dashboard", "search": ""},
      "screen": {"width": 1440, "height": 900, "density": 2},
      "userAgent": "Mozilla/5.0", "locale": "en-US", "timezone": "America/New_York",
      "campaign": {"source": null, "medium": null, "name": null, "term": null, "content": null}
    },
    "event": "Button Clicked",
    "properties": {"button_text": "Sign Up", "page_section": "hero"}
  }]
}'
RESP=$(send_event "$TRACK_BATCH")
if echo "$RESP" | grep -q '"status":"ok"'; then
  pass "ingestion accepted track event"
else
  fail "ingestion rejected track event: $RESP"
fi

# ── Scenario 2: Identify + Identity Resolution ──
bold "Scenario 2: Identify + Identity Resolution"
IDENTIFY_BATCH='{
  "sentAt": "2026-06-30T10:06:00.000Z",
  "batch": [{
    "messageId": "e2e-identify-001",
    "type": "identify",
    "anonymousId": "anon_e2e_identify",
    "userId": "usr_e2e_jane",
    "originalTimestamp": "2026-06-30T10:05:58.000Z",
    "context": {
      "library": {"name": "@opsbucket/browser", "version": "0.1.0"},
      "page": {"url": "https://app.example.com/settings", "path": "/settings", "referrer": "", "title": "Settings", "search": ""},
      "screen": {"width": 1440, "height": 900, "density": 2},
      "userAgent": "Mozilla/5.0", "locale": "en-US", "timezone": "America/New_York",
      "campaign": {"source": "google", "medium": "cpc", "name": "spring_sale", "term": "analytics", "content": "banner1"}
    },
    "traits": {"email": "jane@example.com", "plan": "pro", "age": 28}
  }]
}'
RESP=$(send_event "$IDENTIFY_BATCH")
if echo "$RESP" | grep -q '"status":"ok"'; then
  pass "ingestion accepted identify event"
else
  fail "ingestion rejected identify event: $RESP"
fi

TRACK_AFTER_IDENTIFY='{
  "sentAt": "2026-06-30T10:06:05.000Z",
  "batch": [{
    "messageId": "e2e-track-resolved-001",
    "type": "track",
    "anonymousId": "anon_e2e_identify",
    "userId": null,
    "originalTimestamp": "2026-06-30T10:06:03.000Z",
    "context": {
      "library": {"name": "@opsbucket/browser", "version": "0.1.0"},
      "page": {"url": "https://app.example.com/dashboard", "path": "/dashboard", "referrer": "", "title": "Dashboard", "search": ""},
      "screen": {"width": 1440, "height": 900, "density": 2},
      "userAgent": "Mozilla/5.0", "locale": "en-US", "timezone": "America/New_York",
      "campaign": {"source": null, "medium": null, "name": null, "term": null, "content": null}
    },
    "event": "Plan Upgraded",
    "properties": {"from": "free", "to": "pro"}
  }]
}'
RESP=$(send_event "$TRACK_AFTER_IDENTIFY")
if echo "$RESP" | grep -q '"status":"ok"'; then
  pass "ingestion accepted track-after-identify event"
else
  fail "ingestion rejected track-after-identify: $RESP"
fi

# ── Scenario 3: Page Event ──
bold "Scenario 3: Page Event"
PAGE_BATCH='{
  "sentAt": "2026-06-30T10:07:00.000Z",
  "batch": [{
    "messageId": "e2e-page-001",
    "type": "page",
    "anonymousId": "anon_e2e_page_user",
    "userId": null,
    "originalTimestamp": "2026-06-30T10:06:58.000Z",
    "context": {
      "library": {"name": "@opsbucket/browser", "version": "0.1.0"},
      "page": {"url": "https://app.example.com/pricing", "path": "/pricing", "referrer": "https://google.com", "title": "Pricing", "search": "?plan=enterprise"},
      "screen": {"width": 1920, "height": 1080, "density": 1},
      "userAgent": "Mozilla/5.0", "locale": "fr-FR", "timezone": "Europe/Paris",
      "campaign": {"source": null, "medium": null, "name": null, "term": null, "content": null}
    },
    "name": "Pricing Page",
    "properties": {"section": "enterprise"}
  }]
}'
RESP=$(send_event "$PAGE_BATCH")
if echo "$RESP" | grep -q '"status":"ok"'; then
  pass "ingestion accepted page event"
else
  fail "ingestion rejected page event: $RESP"
fi

# ── Scenario 4: Duplicate Dedup ──
bold "Scenario 4: Deduplication"
DUP_BATCH='{
  "sentAt": "2026-06-30T10:08:00.000Z",
  "batch": [{
    "messageId": "e2e-duplicate-001",
    "type": "track",
    "anonymousId": "anon_dup_test",
    "userId": null,
    "originalTimestamp": "2026-06-30T10:07:58.000Z",
    "context": {
      "library": {"name": "@opsbucket/browser", "version": "0.1.0"},
      "page": {"url": "https://app.example.com/", "path": "/", "referrer": "", "title": "Home", "search": ""},
      "screen": {"width": 1440, "height": 900, "density": 2},
      "userAgent": "Mozilla/5.0", "locale": "en-US", "timezone": "UTC",
      "campaign": {"source": null, "medium": null, "name": null, "term": null, "content": null}
    },
    "event": "Page Viewed",
    "properties": {}
  }]
}'
RESP1=$(send_event "$DUP_BATCH")
RESP2=$(send_event "$DUP_BATCH")
if echo "$RESP1" | grep -q '"status":"ok"' && echo "$RESP2" | grep -q '"status":"ok"'; then
  pass "ingestion accepted both duplicates (dedup happens in processing)"
else
  fail "ingestion rejected duplicate batch: $RESP1 / $RESP2"
fi

# ── Scenario 5: Invalid Write Key ──
bold "Scenario 5: Invalid Write Key → 401"
INVALID_BATCH='{
  "sentAt": "2026-06-30T10:09:00.000Z",
  "batch": [{
    "messageId": "e2e-invalid-key-001",
    "type": "track",
    "anonymousId": "anon_bad_key",
    "userId": null,
    "originalTimestamp": "2026-06-30T10:08:58.000Z",
    "context": {
      "library": {"name": "@opsbucket/browser", "version": "0.1.0"},
      "page": {"url": "https://example.com", "path": "/", "referrer": "", "title": "", "search": ""},
      "screen": {"width": 1440, "height": 900, "density": 2},
      "userAgent": "test", "locale": "en-US", "timezone": "UTC",
      "campaign": {"source": null, "medium": null, "name": null, "term": null, "content": null}
    },
    "event": "Test",
    "properties": {}
  }]
}'
RESP=$(send_event_raw "$INVALID_BATCH" "wk_invalid_nonexistent")
if echo "$RESP" | grep -q '"error":"invalid_write_key"'; then
  pass "invalid write key correctly returns 401"
else
  fail "invalid write key did not return 401: $RESP"
fi

# ── Verification ──
bold "── Verification ──"
wait_for_events 5 "all non-duplicate events" 120

# Total count
COUNT=$(query_ch "SELECT count() FROM events")
if [ "$COUNT" = "5" ]; then
  pass "ClickHouse has exactly 5 events"
else
  fail "expected 5 events, got $COUNT"
fi

# Track fields
TRACK_NAME=$(query_ch "SELECT event_name FROM events WHERE event_id='e2e-track-001'")
[ "$TRACK_NAME" = "Button Clicked" ] && pass "track event_name" || fail "expected 'Button Clicked', got '$TRACK_NAME'"

TRACK_TYPE=$(query_ch "SELECT event_type FROM events WHERE event_id='e2e-track-001'")
[ "$TRACK_TYPE" = "track" ] && pass "track event_type" || fail "expected 'track', got '$TRACK_TYPE'"

TRACK_PID=$(query_ch "SELECT project_id FROM events WHERE event_id='e2e-track-001'")
[ "$TRACK_PID" = "$PROJECT_ID" ] && pass "track project_id" || fail "expected '$PROJECT_ID', got '$TRACK_PID'"

# Identify fields
IDENT_NAME=$(query_ch "SELECT event_name FROM events WHERE event_id='e2e-identify-001'")
[ "$IDENT_NAME" = "Identify" ] && pass "identify event_name" || fail "expected 'Identify', got '$IDENT_NAME'"

IDENT_TYPE=$(query_ch "SELECT event_type FROM events WHERE event_id='e2e-identify-001'")
[ "$IDENT_TYPE" = "identify" ] && pass "identify event_type" || fail "expected 'identify', got '$IDENT_TYPE'"

IDENT_UID=$(query_ch "SELECT user_id FROM events WHERE event_id='e2e-identify-001'")
[ "$IDENT_UID" = "usr_e2e_jane" ] && pass "identify user_id" || fail "expected 'usr_e2e_jane', got '$IDENT_UID'"

# Identity resolution
RESOLVED_UID=$(query_ch "SELECT user_id FROM events WHERE event_id='e2e-track-resolved-001'")
[ "$RESOLVED_UID" = "usr_e2e_jane" ] && pass "identity resolution: user_id resolved" || fail "expected 'usr_e2e_jane', got '$RESOLVED_UID'"

# Page event
PAGE_NAME=$(query_ch "SELECT event_name FROM events WHERE event_id='e2e-page-001'")
[ "$PAGE_NAME" = "Page Viewed" ] && pass "page event_name" || fail "expected 'Page Viewed', got '$PAGE_NAME'"

PAGE_LOCALE=$(query_ch "SELECT locale FROM events WHERE event_id='e2e-page-001'")
[ "$PAGE_LOCALE" = "fr-FR" ] && pass "page locale" || fail "expected 'fr-FR', got '$PAGE_LOCALE'"

# Dedup
DUP_COUNT=$(query_ch "SELECT count() FROM events WHERE event_id='e2e-duplicate-001'")
[ "$DUP_COUNT" = "1" ] && pass "duplicate messageId deduped" || fail "expected 1 row, got $DUP_COUNT"

# Campaign params
CAMP=$(query_ch "SELECT campaign_source FROM events WHERE event_id='e2e-identify-001'")
[ "$CAMP" = "google" ] && pass "campaign_source" || fail "expected 'google', got '$CAMP'"

# Timestamp correction
TS_CHECK=$(query_ch "SELECT count() FROM events WHERE timestamp < received_at")
[ "$TS_CHECK" = "5" ] && pass "timestamp correction applied to all events" || fail "expected 5, got $TS_CHECK"

# Page context
PAGE_URL=$(query_ch "SELECT page_url FROM events WHERE event_id='e2e-page-001'")
[ "$PAGE_URL" = "https://app.example.com/pricing" ] && pass "page_url" || fail "expected URL, got '$PAGE_URL'"

PAGE_REF=$(query_ch "SELECT page_referrer FROM events WHERE event_id='e2e-page-001'")
[ "$PAGE_REF" = "https://google.com" ] && pass "page_referrer" || fail "expected referrer, got '$PAGE_REF'"

SCREEN_W=$(query_ch "SELECT screen_width FROM events WHERE event_id='e2e-page-001'")
[ "$SCREEN_W" = "1920" ] && pass "screen_width" || fail "expected 1920, got '$SCREEN_W'"

# Project isolation
ALL_PROJ=$(query_ch "SELECT count() FROM events WHERE project_id='$PROJECT_ID'")
[ "$ALL_PROJ" = "5" ] && pass "all events have correct project_id" || fail "expected 5, got $ALL_PROJ"

# ── Postgres Verification ──
bold "── Postgres Verification ──"
ALIAS_COUNT=$(docker exec infra-postgres-1 psql -U opsbucket -d opsbucket -t -A \
  -c "SELECT count(*) FROM identity_aliases WHERE project_id='$PROJECT_ID' AND anonymous_id='anon_e2e_identify' AND user_id='usr_e2e_jane'" 2>/dev/null || echo "0")
[ "$ALIAS_COUNT" = "1" ] && pass "identity_aliases has correct mapping" || fail "expected 1 alias row, got '$ALIAS_COUNT'"

# ── Summary ──
bold "════════════════════════════════════════════════════════"
bold "  Results: $PASS passed, $FAIL failed"
bold "════════════════════════════════════════════════════════"
[ "$FAIL" -gt 0 ] && exit 1 || exit 0
