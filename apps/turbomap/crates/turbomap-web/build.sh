#!/usr/bin/env bash
# Build turbomap-web to an npm-importable WASM module (pkg/) for the web app.
#
#   ./build.sh           # dev build (fast, unoptimised)
#   ./build.sh --release # release build (wasm-opt, smaller/faster)
#
# Output: crates/turbomap-web/pkg/  (turbomap_web.js + _bg.wasm + .d.ts +
# package.json) — import it from apps/web (or the smoke page) as an ES module.
set -euo pipefail
cd "$(dirname "$0")/../.."   # repo: apps/turbomap

PROFILE="--dev"
[[ "${1:-}" == "--release" ]] && PROFILE="--release"

wasm-pack build crates/turbomap-web --target web "$PROFILE" --out-dir pkg

echo
echo "Built crates/turbomap-web/pkg. Smoke-test in a WebGPU browser:"
echo "  (cd crates/turbomap-web && python3 -m http.server 8000)"
echo "  open http://localhost:8000/smoke/"
