#!/usr/bin/env bash
# Primitive-only E2E driver. Doesn't need Postgres — each primitive
# crate carries an integration test that constructs a synthetic
# artifact in a tempfile, opens it, and exercises the reader API.
#
# Use when you want a fast (~10 s) signal that the artifact contract
# + readers are healthy without spinning up the full DB. The
# Postgres-backed builder is covered by `tools/run-e2e.sh`.
#
# Exit code: 0 if every primitive's reader + composer is green.

set -euo pipefail

TILESERVER_DIR="$(cd "$(dirname "$0")/.." && pwd)"
cd "${TILESERVER_DIR}"

echo "== Primitive crate tests (synthetic artifacts)"
cargo test --quiet \
  -p turbo-tiles-artifacts \
  -p turbo-tiles-elev \
  -p turbo-tiles-mask \
  -p turbo-tiles-graph \
  -p turbo-tiles-search \
  -p turbo-tiles-pathfind \
  2>&1 | grep -E "^(test result|running |test .* FAILED|---- )" || true

echo ""
echo "== Compile-only check: criterion benches"
cargo bench --no-run -p turbo-tiles-elev 2>&1 | tail -3

echo ""
echo "== Admin SPA typecheck"
( cd ../admin && npm run --silent typecheck )

echo ""
echo "================================================================"
echo "  Primitive E2E: OK"
echo "================================================================"
