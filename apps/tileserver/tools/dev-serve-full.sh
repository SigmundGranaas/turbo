#!/usr/bin/env bash
# Boot tileserver against the FULL Norway dataset for local dev.
#
# Unlike tools/open-app.sh and tools/dev-up.sh (which seed a synthetic
# Oslo-area `tiles_e2e` DB and build throwaway artifacts in /tmp), this
# serves the real, pre-built Norway artifacts in ~/turbo-artifacts and
# the full `tiles` DB — the setup used to validate real routes
# (Langvatnet, Heggmotinden, the terrain corpus).
#
# It does NOT reseed or drop any database and does NOT delete artifacts.
# Pairs with the Vite dev server:  cd apps/admin && npm run dev
# (Vite on :5173 proxies /v1/* and /admin/api/* to localhost:8090).
#
# All values are overridable via env, e.g.  PORT=8091 ./tools/dev-serve-full.sh
set -euo pipefail
export PATH="/opt/homebrew/opt/libpq/bin:$PATH"

# Dev-only auth: mint a curator cookie via /admin/dev-login, no .NET stack.
export TURBO_DEV_AUTH="${TURBO_DEV_AUTH:-1}"
export JWT_SECRET="${JWT_SECRET:-dev-only-secret-do-not-use-anywhere-else-hs256}"
export DATABASE_URL="${DATABASE_URL:-postgres://postgres:yourpassword@localhost:5446/tiles}"

ARTIFACTS="${TILESERVER_ARTIFACT_DIR:-${HOME}/turbo-artifacts}"
PORT="${PORT:-8090}"
TILESERVER_DIR="$(cd "$(dirname "$0")/.." && pwd)"
BIN="${TILESERVER_DIR}/target/release/tileserver"

cd "${TILESERVER_DIR}"

if [[ ! -f "${ARTIFACTS}/norway.graph" ]]; then
    echo "✗ Full Norway artifacts not found in ${ARTIFACTS} (no norway.graph)." >&2
    echo "  Set TILESERVER_ARTIFACT_DIR or build them with tools/build-norway.sh." >&2
    exit 1
fi
if [[ ! -x "${BIN}" ]]; then
    echo "▶ Release binary missing — building (cargo build --release --bin tileserver)"
    cargo build --release --bin tileserver
fi

echo "▶ Booting tileserver on :${PORT}"
echo "  Artifacts:  ${ARTIFACTS}"
echo "  Database:   ${DATABASE_URL%%\?*}"
echo "  Sign in:    http://localhost:${PORT}/admin/dev-login   (mints curator cookie)"
echo "  SPA (Vite): http://localhost:5173/   (run: cd apps/admin && npm run dev)"
echo "  Ctrl+C to stop."
echo ""

exec "${BIN}" serve \
    --bind="127.0.0.1:${PORT}" \
    --artifacts-dir="${ARTIFACTS}" \
    --public-base-url="http://localhost:${PORT}"
