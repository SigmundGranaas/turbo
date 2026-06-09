#!/usr/bin/env bash
# One entrypoint to verify place-core end to end:
#   1. pure Rust logic + golden fixtures (`cargo test`)
#   2. the UniFFI Python binding (golden replay + round-trip + error path)
#   3. the UniFFI Kotlin binding (when a Kotlin toolchain + JNA are available)
#
# Kotlin step needs `kotlinc` on PATH (override with $KOTLINC) and $JNA_JAR
# pointing at a JNA 5.x jar; it is skipped (not failed) when those are absent.
set -euo pipefail
cd "$(dirname "$0")/.."

echo "== [1/3] cargo test (pure logic + golden) =="
cargo test --quiet

echo "== [2/3] UniFFI Python client =="
cargo build --features uniffi --quiet
cargo run --features uniffi --quiet --bin uniffi-bindgen -- \
    generate --library target/debug/libplace_core.so --language python --out-dir clients/python
cp target/debug/libplace_core.so clients/python/
python3 clients/python/golden_client.py

echo "== [3/3] UniFFI Kotlin client =="
KOTLINC="${KOTLINC:-kotlinc}"
if command -v "$KOTLINC" >/dev/null 2>&1 && [ -n "${JNA_JAR:-}" ] && [ -f "${JNA_JAR:-}" ]; then
    cargo run --features uniffi --quiet --bin uniffi-bindgen -- \
        generate --library target/debug/libplace_core.so --language kotlin --out-dir clients/kotlin
    jar="$(mktemp -d)/gc.jar"
    "$KOTLINC" clients/kotlin/uniffi/place_core/place_core.kt clients/kotlin/GoldenClient.kt \
        -cp "$JNA_JAR" -include-runtime -d "$jar" 2>/dev/null
    java -cp "$jar:$JNA_JAR" -Djna.library.path="$(pwd)/target/debug" GoldenClientKt
else
    echo "   skipped — set \$KOTLINC (or PATH) and \$JNA_JAR to run the Kotlin client"
fi

echo "All place-core checks passed."
