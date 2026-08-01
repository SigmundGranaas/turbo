#!/usr/bin/env bash
# Does the solver return the same route on the phone's ISA and libc?
#
# `cargo test` runs on x86_64 against glibc. The phone is aarch64
# against bionic. Tobler's term is exp(), which is not correctly-rounded
# and is a different implementation in each libc — so a cost that
# differs in the last place can pop a different edge off the priority
# queue and diverge from there. Nothing in the host suite can see that.
#
# This plans one route in both worlds and compares the raw bits.
#
# The aarch64 side is linked STATIC on purpose: that pulls bionic's own
# libm into the binary, so the comparison is against the implementation
# the phone runs rather than against the emulator host's. It is also why
# `--no-default-features` is needed — liblog ships shared-only, so the
# logcat feature cannot be statically linked.
#
# Requires: qemu-aarch64-static, cargo-ndk, ANDROID_NDK_HOME.
set -euo pipefail

cd "$(dirname "$0")/.."
PACK="${1:-tools/ci-pack}"

if ! command -v qemu-aarch64-static >/dev/null; then
    echo "SKIP: qemu-aarch64-static not installed" >&2
    exit 0
fi
if ! command -v cargo-ndk >/dev/null; then
    echo "SKIP: cargo-ndk not installed" >&2
    exit 0
fi

echo "building host probe..."
cargo build -q --release --bin route-probe

echo "building aarch64-android probe (static)..."
RUSTFLAGS="-C target-feature=+crt-static" \
    cargo ndk -t arm64-v8a build -q --release --no-default-features --bin route-probe

host=$(cargo run -q --release --bin route-probe -- "$PACK")
droid=$(qemu-aarch64-static target/aarch64-linux-android/release/route-probe "$PACK")

# `arch` and `os` differ by construction; everything below them must not.
strip_id() { grep -Ev '^(arch|os) '; }

if diff <(echo "$host" | strip_id) <(echo "$droid" | strip_id) > /tmp/isa-diff; then
    echo
    echo "$host" | sed 's/^/  x86_64  /'
    echo "$droid" | sed 's/^/  aarch64 /'
    echo
    echo "PARITY OK — identical bits on glibc/x86_64 and bionic/aarch64"
else
    echo
    echo "PARITY FAILED — the phone would draw a different line:"
    cat /tmp/isa-diff
    exit 1
fi
