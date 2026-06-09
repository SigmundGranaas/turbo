#!/usr/bin/env bash
# Build the cdylib, regenerate the Python binding, drop the lib next to it, and
# run the synthetic golden client. Verifies the UniFFI surface end-to-end.
set -euo pipefail
cd "$(dirname "$0")/.."

cargo build --features uniffi --quiet
cargo run --features uniffi --quiet --bin uniffi-bindgen -- \
    generate --library target/debug/libplace_core.so --language python --out-dir clients/python
cp target/debug/libplace_core.so clients/python/
python3 clients/python/golden_client.py
