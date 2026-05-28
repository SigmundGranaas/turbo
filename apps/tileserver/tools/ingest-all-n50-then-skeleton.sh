#!/usr/bin/env bash
# After n50-restore completes, run every n50 upsert + skeleton-build
# in order. Each upsert is seconds; skeleton-build needs anchors
# snapped, which n50-stedsnavn-upsert provides.
#
# Run order:
#   n50-vann-upsert        — water polygons
#   n50-isogbre-upsert     — glaciers
#   n50-landcover-upsert   — forest/wetland/open polygons (bumps attr_version)
#   n50-stedsnavn-upsert   — anchors, snaps to graph
#   n50-vegnett-upsert     — roads (rebuilds topology)
#   skeleton-build         — Delaunay over snapped anchors

set -euo pipefail
export PATH="/opt/homebrew/opt/libpq/bin:$PATH"
export DATABASE_URL="${DATABASE_URL:-postgres://postgres:yourpassword@localhost:5446/tiles}"

cd "$(dirname "$0")/.."

run() {
  local job="$1"
  echo "=========================================================="
  echo "  $job"
  echo "=========================================================="
  cargo run --quiet --bin tileserver -- ingest --job "$job" 2>&1 \
    | grep -E "job finished|outcome|error|panic|WARN" \
    | tail -5
  echo ""
}

run n50-vann-upsert
run n50-isogbre-upsert
run n50-landcover-upsert
run n50-stedsnavn-upsert
run n50-vegnett-upsert
run skeleton-build
echo "✓ N50 + skeleton complete"
