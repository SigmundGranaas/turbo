#!/usr/bin/env bash
# Autonomous E2E test runner. Boots a fresh tiles-db container,
# applies migrations, runs every unit + integration + HTTP-layer
# test against it, then prints a single-line pass/fail summary.
#
# Usage:
#   tools/run-e2e.sh            # full run, ~30 s
#   tools/run-e2e.sh --keep-db  # leave the container up after tests
#                                # so the curator can poke at the state
#   tools/run-e2e.sh --rebuild  # rebuild the postgis image before running
#
# Exit code: 0 if everything green, 1 otherwise.

set -euo pipefail

REPO_ROOT="$(cd "$(dirname "$0")/../../.." && pwd)"
TILESERVER_DIR="$(cd "$(dirname "$0")/.." && pwd)"
CONTAINER="turbo-tiles-db-test"
PORT="${TILES_DB_TEST_PORT:-55433}"
DB_URL="postgres://postgres:testpass@localhost:${PORT}/tiles"
KEEP_DB=0
REBUILD=0

for arg in "$@"; do
  case "$arg" in
    --keep-db) KEEP_DB=1 ;;
    --rebuild) REBUILD=1 ;;
    *) echo "unknown arg: $arg" >&2; exit 2 ;;
  esac
done

cleanup() {
  if [ "${KEEP_DB}" -eq 0 ]; then
    docker rm -f "${CONTAINER}" >/dev/null 2>&1 || true
  fi
}
trap cleanup EXIT

# psql is needed for ingest jobs to shell out to.
if ! command -v psql >/dev/null 2>&1; then
  if [ -x "/opt/homebrew/opt/libpq/bin/psql" ]; then
    export PATH="/opt/homebrew/opt/libpq/bin:$PATH"
  else
    echo "psql not found — install with: brew install libpq" >&2
    exit 2
  fi
fi

cd "${TILESERVER_DIR}"

echo "== Building test postgis image"
if [ "${REBUILD}" -eq 1 ] || ! docker image inspect turbo-tiles-db:test >/dev/null 2>&1; then
  DOCKER_DEFAULT_PLATFORM=linux/amd64 docker build --platform linux/amd64 \
    -t turbo-tiles-db:test \
    -f infra/compose/postgis-pgrouting/Dockerfile.test \
    infra/compose/postgis-pgrouting/ >/dev/null
fi

echo "== Starting ${CONTAINER} on port ${PORT}"
docker rm -f "${CONTAINER}" >/dev/null 2>&1 || true
docker run -d --name "${CONTAINER}" --platform linux/amd64 \
  -e POSTGRES_USER=postgres -e POSTGRES_PASSWORD=testpass -e POSTGRES_DB=tiles \
  -p "${PORT}:5432" turbo-tiles-db:test >/dev/null

echo "== Waiting for Postgres"
for _ in $(seq 1 60); do
  if docker exec "${CONTAINER}" pg_isready -U postgres >/dev/null 2>&1; then
    break
  fi
  sleep 1
done

# Install required extensions (the postgis image installs postgis +
# postgis_raster by default; pg_trgm and pgrouting need a one-line
# CREATE EXTENSION). Errors get swallowed because some are already
# present on first connect.
docker exec "${CONTAINER}" psql -U postgres -d tiles -c "
  CREATE EXTENSION IF NOT EXISTS postgis;
  CREATE EXTENSION IF NOT EXISTS pgrouting;
  CREATE EXTENSION IF NOT EXISTS pg_trgm;
  CREATE EXTENSION IF NOT EXISTS postgis_raster;
" >/dev/null

echo "== Applying migrations"
DATABASE_URL="${DB_URL}" cargo run --quiet --bin tileserver -- migrate >/dev/null

echo "== Running workspace tests (unit + integration)"
INGEST_TEST_DATABASE_URL="${DB_URL}" \
  cargo test --quiet --workspace --exclude turbo-tiles-admin -- --test-threads=1 2>&1 \
  | tee /tmp/turbo-e2e-out.log | grep -E "^(test result|running|test .* FAILED|---- )" \
  || true

# Parse the test outputs for a final tally.
total_passed=$(grep -E "^test result: ok\." /tmp/turbo-e2e-out.log | awk '{s+=$4} END {print s+0}')
total_failed=$(grep -E "^test result:" /tmp/turbo-e2e-out.log | awk '{s+=$6} END {print s+0}')

echo ""
echo "================================================================"
if [ "${total_failed}" -eq 0 ] && [ "${total_passed}" -gt 0 ]; then
  echo "  ✓ E2E SUITE: ${total_passed} passed, 0 failed"
  echo "================================================================"
  exit 0
else
  echo "  ✗ E2E SUITE: ${total_passed} passed, ${total_failed} failed"
  echo "  See full log at: /tmp/turbo-e2e-out.log"
  echo "================================================================"
  exit 1
fi
