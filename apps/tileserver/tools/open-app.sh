#!/usr/bin/env bash
# One-command "make the admin app openable on my laptop":
#
#   1. Seed the dev database if needed (synthetic Oslo-area data).
#   2. Build every primitive artifact (DEM/graph/search/mask).
#   3. Build the React SPA so the tileserver serves the freshest /primitives screen.
#   4. Boot the tileserver in the foreground with dev-auth enabled.
#   5. Open the sign-in URL in your default browser. The handler
#      mints a curator JWT, sets the cookie, redirects to /admin/app/.
#
# Ctrl+C stops the server. Run `--keep` to leave the seeded DB
# around after exit; default tears it down.

set -euo pipefail
export PATH="/opt/homebrew/opt/libpq/bin:$PATH"
export JWT_SECRET="${JWT_SECRET:-dev-only-secret-do-not-use-anywhere-else-hs256}"
export TURBO_DEV_AUTH=1

KEEP=0
for arg in "$@"; do
    case "$arg" in
        --keep) KEEP=1 ;;
        *) echo "unknown arg: $arg" >&2; exit 2 ;;
    esac
done

ADMIN_DB="postgres://postgres:testpass@localhost:55433/postgres"
DB="postgres://postgres:testpass@localhost:55433/tiles_e2e"
ARTIFACTS="${TILESERVER_ARTIFACT_DIR:-/tmp/turbo-dev-artifacts}"
TILESERVER_DIR="$(cd "$(dirname "$0")/.." && pwd)"
ADMIN_DIR="$(cd "$(dirname "$0")/../../admin" && pwd)"
PORT="${PORT:-8090}"

cd "${TILESERVER_DIR}"

cleanup() {
    if [[ "${KEEP}" -eq 0 ]]; then
        psql "${ADMIN_DB}" -c "DROP DATABASE IF EXISTS tiles_e2e WITH (FORCE)" >/dev/null 2>&1 || true
        rm -rf "${ARTIFACTS}"
    fi
}
trap cleanup EXIT

# ---- 1. DB ----------------------------------------------------------------
if ! docker exec turbo-tiles-db-test pg_isready -U postgres >/dev/null 2>&1; then
    echo "✗ Test DB container 'turbo-tiles-db-test' isn't running."
    echo "  Start it:  docker start turbo-tiles-db-test"
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
    echo "  ✓ already seeded"
fi

# ---- 2. Artifacts ---------------------------------------------------------
echo "▶ Building primitive artifacts → ${ARTIFACTS}"
mkdir -p "${ARTIFACTS}"
artifact_file() {
    case "$1" in
        dem)    echo "norway.dem" ;;
        graph)  echo "norway.graph" ;;
        search) echo "norway.anchors" ;;
        mask)   echo "norway.mask" ;;
    esac
}
for kind in dem graph search mask; do
    if [[ -f "${ARTIFACTS}/$(artifact_file "$kind")" ]]; then
        echo "  ✓ ${kind} already built"
        continue
    fi
    DATABASE_URL="${DB}" cargo run --quiet --bin tileserver -- \
        build-artifacts --kind="${kind}" --out="${ARTIFACTS}" >/dev/null
    echo "  ✓ built ${kind}"
done

# ---- 3. SPA ---------------------------------------------------------------
echo "▶ Building admin SPA"
if [[ ! -d "${ADMIN_DIR}/node_modules" ]]; then
    ( cd "${ADMIN_DIR}" && npm install --silent )
fi
( cd "${ADMIN_DIR}" && npm run --silent build )
echo "  ✓ SPA built at ${ADMIN_DIR}/dist"

# ---- 4. Boot tileserver ---------------------------------------------------
APP_URL="http://localhost:${PORT}/admin/dev-login"
echo ""
echo "================================================================"
echo "  Open this URL in your browser:"
echo ""
echo "      ${APP_URL}"
echo ""
echo "  It mints a curator JWT cookie and redirects to /admin/app/."
echo "  Then navigate to Primitives to drive every endpoint."
echo "================================================================"
echo ""

# Try to open the browser automatically on macOS — best-effort.
( sleep 2 && open "${APP_URL}" 2>/dev/null ) &

exec env DATABASE_URL="${DB}" cargo run --bin tileserver -- serve \
    --bind="127.0.0.1:${PORT}" \
    --artifacts-dir="${ARTIFACTS}" \
    --public-base-url="http://localhost:${PORT}"
