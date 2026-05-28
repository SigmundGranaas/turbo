#!/usr/bin/env bash
# Local-only dev driver. Stands up a seeded `tiles_e2e` database,
# builds every primitive artifact, then boots tileserver on :8090
# with the artifacts loaded so the admin SPA's Primitives tab is
# fully live. Ctrl+C to stop.
#
# Pairs with `cd apps/admin && npm run dev` — Vite proxies /v1/*
# and /admin/api/* to localhost:8090 (see apps/admin/vite.config.ts).
#
# Auth note: this script does NOT stand up the .NET auth service.
# The SPA's `AuthGate` will probe /admin/api/resources and gate the
# UI behind a sign-in. Either:
#   1. Run alongside your normal dev stack (you already have the
#      access_token cookie set), OR
#   2. Hit /v1/* endpoints directly with curl (the Primitives tab
#      is just a UI over them — they require no auth).

set -euo pipefail
export PATH="/opt/homebrew/opt/libpq/bin:$PATH"
export JWT_SECRET="${JWT_SECRET:-dev-not-a-real-secret-do-not-use-in-prod}"

ADMIN_DB="postgres://postgres:testpass@localhost:55433/postgres"
DB="postgres://postgres:testpass@localhost:55433/tiles_e2e"
ARTIFACTS="${TILESERVER_ARTIFACT_DIR:-/tmp/turbo-dev-artifacts}"
TILESERVER_DIR="$(cd "$(dirname "$0")/.." && pwd)"
PORT="${PORT:-8090}"

cd "${TILESERVER_DIR}"

if ! docker exec turbo-tiles-db-test pg_isready -U postgres >/dev/null 2>&1; then
    echo "Test DB container 'turbo-tiles-db-test' isn't responding."
    echo "Start it first: docker start turbo-tiles-db-test"
    exit 1
fi

echo "▶ Preparing tiles_e2e database"
if ! psql "${DB}" -c "SELECT 1 FROM paths.dem LIMIT 1" >/dev/null 2>&1; then
    psql "${ADMIN_DB}" -c "DROP DATABASE IF EXISTS tiles_e2e WITH (FORCE)" >/dev/null 2>&1 || true
    psql "${ADMIN_DB}" -c "CREATE DATABASE tiles_e2e" >/dev/null
    psql "${DB}" -c "
        CREATE EXTENSION IF NOT EXISTS postgis;
        CREATE EXTENSION IF NOT EXISTS postgis_raster;
        CREATE EXTENSION IF NOT EXISTS pgrouting;
        CREATE EXTENSION IF NOT EXISTS pg_trgm;
    " >/dev/null
    DATABASE_URL="${DB}" cargo run --quiet --bin tileserver -- migrate
    psql "${DB}" -v ON_ERROR_STOP=1 -f "${TILESERVER_DIR}/tools/e2e/seed.sql" >/dev/null
    echo "  ✓ seeded fresh"
else
    echo "  ✓ already seeded (reusing)"
fi

echo "▶ Building artifacts → ${ARTIFACTS}"
mkdir -p "${ARTIFACTS}"
for kind in dem graph search mask; do
    if [[ -f "${ARTIFACTS}/norway.${kind/anchors/anchors}" ]] || \
       ([[ "$kind" == "search" ]] && [[ -f "${ARTIFACTS}/norway.anchors" ]]); then
        echo "  ✓ ${kind} already present"
        continue
    fi
    DATABASE_URL="${DB}" cargo run --quiet --bin tileserver -- \
        build-artifacts --kind="${kind}" --out="${ARTIFACTS}" >/dev/null
    echo "  ✓ built ${kind}"
done

echo "▶ Booting tileserver on :${PORT}"
echo "  Primitives UI:    cd apps/admin && npm run dev  →  http://localhost:5173/admin/app/primitives"
echo "  Direct API smoke: curl http://localhost:${PORT}/v1/debug/elev/coverage | jq"
echo "  Ctrl+C to stop."
echo ""

exec env DATABASE_URL="${DB}" cargo run --bin tileserver -- serve \
    --bind="127.0.0.1:${PORT}" \
    --artifacts-dir="${ARTIFACTS}" \
    --public-base-url="http://localhost:${PORT}"
