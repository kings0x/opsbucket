#!/usr/bin/env bash
# ─── OpsBucket End-to-End Integration Test ─────────────────────────────────────
#
# This script runs the full OpsBucket pipeline:
#   SDK (curl-simulated) → Ingestion → Redpanda → Processing → ClickHouse
#
# It tests: happy path (track, identify, page), identity resolution, deduplication,
# timestamp correction, invalid write key rejection, and DLQ routing.
#
# Prerequisites:
#   - Docker + Docker Compose
#   - Rust toolchain (stable)
#   - clickhouse-client (or curl for HTTP queries)
#   - psql (Postgres client)
#
# Usage:
#   bash tests/e2e/run.sh              # full run, cleanup on exit
#   bash tests/e2e/run.sh --no-cleanup  # keep infra running after test
#   bash tests/e2e/run.sh --skip-build  # skip cargo build step
#
# ────────────────────────────────────────────────────────────────────────────────
set -euo pipefail

# ── Config ─────────────────────────────────────────────────────────────────────
ROOT_DIR="$(cd "$(dirname "$0")/../.." && pwd)"
INFRA_DIR="$ROOT_DIR/infra"
SERVICES_DIR="$ROOT_DIR/services"

INGEST_PORT=8080
CLICKHOUSE_HTTP_PORT=8123
REDPANDA_PORT=9092
POSTGRES_PORT=5432

WRITE_KEY="wk_test_valid_key_12345"
PROJECT_ID="proj_test"

PASS=0
FAIL=0
TIMEOUT=60  # max seconds to wait for processing to consume

NO_CLEANUP=false
SKIP_BUILD=false

# Parse flags
for arg in "$@"; do
  case "$arg" in
    --no-cleanup) NO_CLEANUP=true ;;
    --skip-build) SKIP_BUILD=true ;;
  esac
done

# ── Helpers ────────────────────────────────────────────────────────────────────
green()  { printf "\033[32m%s\033[0m\n" "$1"; }
red()    { printf "\033[31m%s\033[0m\n" "$1"; }
bold()   { printf "\033[1m%s\033[0m\n" "$1"; }
info()   { printf "  ▸ %s\n" "$1"; }

pass()   { PASS=$((PASS+1)); green "  ✓ PASS: $1"; }
fail()   { FAIL=$((FAIL+1)); red "  ✗ FAIL: $1"; }

# Wait for a TCP port to be open
wait_for_port() {
  local host=$1 port=$2 label=$3 timeout=${4:-60}
  info "waiting for $label ($host:$port) ..."
  local i=0
  while ! timeout 1 bash -c "echo >/dev/tcp/$host/$port" 2>/dev/null; do
    i=$((i+1))
    if [ $i -ge "$timeout" ]; then
      fail "$label did not start within ${timeout}s"
      return 1
    fi
    sleep 1
  done
  green "  $label is ready"
}

# Cleanup handler
cleanup() {
  if [ "$NO_CLEANUP" = true ]; then
    info "skipping cleanup (--no-cleanup)"
    return
  fi
  bold "── Cleaning Up ──"
  # Kill background processes
  info "stopping ingestion and processing..."
  kill "$INGESTION_PID" 2>/dev/null || true
  kill "$PROCESSING_PID" 2>/dev/null || true
  wait "$INGESTION_PID" 2>/dev/null || true
  wait "$PROCESSING_PID" 2>/dev/null || true
  # Stop Docker infra
  info "stopping Docker containers..."
  docker compose -f "$INFRA_DIR/docker-compose.yml" down -v 2>/dev/null || true
  green "cleanup complete"
}

trap cleanup EXIT INT TERM

# ── Phase 1: Start Infrastructure ─────────────────────────────────────────────
bold "── Phase 1: Infrastructure ──"

info "starting Docker containers (Redpanda, Postgres, Redis, ClickHouse)..."
docker compose -f "$INFRA_DIR/docker-compose.yml" up -d
wait_for_port "localhost" "$REDPANDA_PORT" "Redpanda" 60
wait_for_port "localhost" "$POSTGRES_PORT" "Postgres" 30
wait_for_port "localhost" "6379" "Redis" 30
wait_for_port "localhost" "$CLICKHOUSE_HTTP_PORT" "ClickHouse HTTP" 60

# ── Phase 2: Configure Infrastructure ─────────────────────────────────────────
bold "── Phase 2: Configuration ──"

info "creating Kafka topics..."
docker compose -f "$INFRA_DIR/docker-compose.yml" exec -T redpanda \
  rpk topic create raw-events --partitions 1 -r 1 2>&1 || true
docker compose -f "$INFRA_DIR/docker-compose.yml" exec -T redpanda \
  rpk topic create raw-events-dlq --partitions 1 -r 1 2>&1 || true
info "topics created"

info "running Postgres migrations..."
cd "$SERVICES_DIR"
DATABASE_URL="postgres://opsbucket:opsbucket@localhost:5432/opsbucket" \
  cargo run -p opsbucket-migrator -- up 2>&1
cd "$ROOT_DIR"

info "seeding test data..."
PGPASSWORD=opsbucket psql -h localhost -U opsbucket -d opsbucket -f "$ROOT_DIR/tests/e2e/seed.sql"
info "seed data loaded"

info "applying ClickHouse schema..."
docker compose -f "$INFRA_DIR/docker-compose.yml" exec -T clickhouse \
  clickhouse-client --multiquery < "$INFRA_DIR/clickhouse/schema.sql"
info "ClickHouse schema applied"

# ── Phase 3: Build Services ────────────────────────────────────────────────────
bold "── Phase 3: Build ──"

if [ "$SKIP_BUILD" = false ]; then
  info "building ingestion..."
  cd "$SERVICES_DIR"
  cargo build -p opsbucket-ingestion 2>&1
  cd "$ROOT_DIR"
  info "ingestion built"

  info "building processing..."
  cd "$SERVICES_DIR"
  cargo build -p opsbucket-processing 2>&1
  cd "$ROOT_DIR"
  info "processing built"
else
  info "skipping build (--skip-build)"
fi

# ── Phase 4: Start Services ────────────────────────────────────────────────────
bold "── Phase 4: Start Services ──"

info "starting ingestion on port $INGEST_PORT..."
KAFKA_BROKERS="localhost:9092" \
DATABASE_URL="postgres://opsbucket:opsbucket@localhost:5432/opsbucket" \
REDIS_URL="redis://localhost:6379" \
RUST_LOG="info" \
PORT="$INGEST_PORT" \
  cargo run -p opsbucket-ingestion &
INGESTION_PID=$!
wait_for_port "localhost" "$INGEST_PORT" "Ingestion" 30

info "starting processing consumer..."
KAFKA_BROKERS="localhost:9092" \
KAFKA_CONSUMER_GROUP="opsbucket-processing-inttest" \
DATABASE_URL="postgres://opsbucket:opsbucket@localhost:5432/opsbucket" \
REDIS_URL="redis://localhost:6379" \
CLICKHOUSE_URL="http://localhost:$CLICKHOUSE_HTTP_PORT" \
CLICKHOUSE_USER="default" \
CLICKHOUSE_PASSWORD="opsbucket" \
BATCH_SIZE="100" \
BATCH_TIMEOUT_MS="3000" \
DEDUP_TTL_SECONDS="86400" \
ALIAS_CACHE_TTL_SECONDS="600" \
DLQ_MAX_RETRIES="3" \
RUST_LOG="info" \
  cargo run -p opsbucket-processing &
PROCESSING_PID=$!
info "processing PID: $PROCESSING_PID"
sleep 3  # give processing time to start polling

# ── Phase 5: Run Test Scenarios ────────────────────────────────────────────────
bold "── Phase 5: Test Scenarios ──"

INGEST_PORT="$INGEST_PORT" \
CLICKHOUSE_HTTP_PORT="$CLICKHOUSE_HTTP_PORT" \
WRITE_KEY="$WRITE_KEY" \
PROJECT_ID="$PROJECT_ID" \
  bash "$ROOT_DIR/tests/e2e/scenarios.sh"
# scenarios.sh handles its own pass/fail accounting and exit code
