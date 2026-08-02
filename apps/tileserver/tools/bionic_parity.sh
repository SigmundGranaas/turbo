#!/usr/bin/env bash
# Does a route computed on Android match one computed on the server?
#
# E1 answered half of this: the solver is bit-identical between x86_64
# and aarch64 **under glibc**, run with qemu-user. Its own caveat named
# what was left — "Android links bionic. This settles the ISA question,
# not the platform one" — and bionic's libm is a different
# implementation, one E0 had already caught diverging from glibc's on
# `f32::atan`.
#
# This closes it without a phone. The NDK ships bionic, and a statically
# linked binary needs no Android loader, so the real engine can be built
# against bionic for aarch64 and run under qemu-user next to the other
# two.
#
# # Why the libm fingerprint is printed
#
# Route hashes alone cannot tell "bionic agrees with glibc" from "Rust
# never asked either of them" — and a test that proves the second while
# claiming the first is worse than no test. So each run also fingerprints
# the transcendentals the cost model actually calls. The three
# fingerprints are EXPECTED TO DIFFER; that is what makes the route
# hashes matching mean something. If they ever start agreeing, this
# script has stopped measuring what it says it measures.
#
# Usage:  tools/bionic_parity.sh
# Needs:  qemu-aarch64-static, aarch64-linux-gnu-gcc, cargo-ndk,
#         $ANDROID_NDK_HOME (the session-start hook installs all four).
set -euo pipefail

cd "$(dirname "$0")/.."

PACK="tools/ci-pack"
NDK="${ANDROID_NDK_HOME:-$(ls -d /opt/android-sdk/ndk/* 2>/dev/null | sort | tail -1)}"
: "${NDK:?ANDROID_NDK_HOME is not set and no NDK found under /opt/android-sdk}"
export ANDROID_NDK_HOME="$NDK"

run() { # name, command...
  local name="$1"; shift
  echo "── $name"
  "$@" "$PACK" | sed 's/^/   /'
}

echo "Routing parity across libc and ISA — pack: $PACK"
echo

# 1. Host: x86_64, glibc. The baseline the server runs on.
cargo build -q -p turbo-route-ffi --release --example parity
run "x86_64  glibc  (native)" target/release/examples/parity

# 2. aarch64, glibc. E1's original comparison — ISA only.
cargo build -q -p turbo-route-ffi --release --example parity \
  --target aarch64-unknown-linux-gnu \
  --config 'target.aarch64-unknown-linux-gnu.linker="aarch64-linux-gnu-gcc"'
run "aarch64 glibc  (qemu)" \
  qemu-aarch64-static -L /usr/aarch64-linux-gnu \
  target/aarch64-unknown-linux-gnu/release/examples/parity

# 3. aarch64, bionic — what a phone actually runs.
#
# `--no-default-features` drops the logcat feature: `liblog` ships only
# as a shared library, so a static binary cannot have it. Nothing else
# about the engine changes.
#
# Static because an Android binary's ELF interpreter is
# `/system/bin/linker64`, which does not exist here — and the point is to
# exercise bionic's libm, which a static link brings along.
RUSTFLAGS="-C target-feature=+crt-static" \
  cargo ndk -t arm64-v8a build --release --example parity \
  -p turbo-route-ffi --no-default-features >/dev/null
run "aarch64 bionic (qemu)" \
  qemu-aarch64-static target/aarch64-linux-android/release/examples/parity

echo
echo "The four route hashes must match across all three."
echo "The libm fingerprints must NOT — see the header."
