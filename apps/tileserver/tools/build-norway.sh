#!/usr/bin/env bash
# Build every primitive artifact from the real `tiles-db` Postgres
# container against the full Norway ingest. Cheapest first so the
# server can come up with partial functionality sooner.
#
# Output goes to /var/lib/tileserver/artifacts by default; override
# with TILESERVER_ARTIFACT_DIR=/path. Progress is line-buffered to
# stdout — pipe it through `tee` if you also want a log file.

set -euo pipefail
export PATH="/opt/homebrew/opt/libpq/bin:$PATH"

PROD_DB="${DATABASE_URL:-postgres://postgres:yourpassword@localhost:5446/tiles}"
ARTIFACTS="${TILESERVER_ARTIFACT_DIR:-/var/lib/tileserver/artifacts}"
TILESERVER_DIR="$(cd "$(dirname "$0")/.." && pwd)"
BIN="${TILESERVER_DIR}/target/release/tileserver"

cd "${TILESERVER_DIR}"

if [[ ! -x "${BIN}" ]]; then
    echo "▶ Compiling release tileserver"
    cargo build --release --bin tileserver
fi

mkdir -p "${ARTIFACTS}"
echo "▶ Output dir: ${ARTIFACTS}"
echo "▶ Source DB: ${PROD_DB}"

build_one() {
    local kind="$1"
    local outfile="$2"
    if [[ -f "${ARTIFACTS}/${outfile}" ]]; then
        local size
        size=$(stat -f%z "${ARTIFACTS}/${outfile}" 2>/dev/null || stat -c%s "${ARTIFACTS}/${outfile}")
        echo "  ✓ ${kind} already present (${outfile}, ${size} bytes) — skipping"
        return 0
    fi
    echo ""
    echo "▶ Building ${kind} → ${outfile}"
    local started=$(date +%s)
    DATABASE_URL="${PROD_DB}" "${BIN}" build-artifacts \
        --kind="${kind}" --out="${ARTIFACTS}"
    local elapsed=$(($(date +%s) - started))
    echo "  ✓ ${kind} done in ${elapsed}s"
}

# Cheapest first (so partial functionality unlocks early):
#   graph  → /v1/route works
#   search → /v1/search works
#   mask   → off-trail respects water/glacier
#   dem    → off-trail honours slope; /v1/elev works
build_one graph  norway.graph
build_one search norway.anchors
build_one mask   norway.mask
build_one dem    norway.dem
# Vector feature collections: water/wetland/streams/cultivated/
# building polygons + linestrings, mmap'd at runtime. Replaces the
# rasterised masks for these classes — preserves original geometry
# so a 5m tarn no longer creates a 100m halo.
build_one vectors norway.vectors

echo ""
echo "▶ Verifying artifacts"
DATABASE_URL="${PROD_DB}" "${BIN}" verify-artifacts --dir="${ARTIFACTS}" | jq

echo ""
echo "================================================================"
echo "  ALL ARTIFACTS BUILT under ${ARTIFACTS}"
echo "================================================================"
