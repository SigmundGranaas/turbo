# turbomap-golden

Headless golden-image + record/replay test harness for turbomap. This is
the Phase 0 foundation from the implementation & testing plan: it lets
*every render path* be exercised deterministically in CI without a window,
a network, or a real GPU.

## What's here

| Module | Role |
| --- | --- |
| `gpu` | Headless wgpu context (prefers a **software** adapter for determinism) + single-frame offscreen readback. |
| `sources` | Deterministic, in-process synthetic tile sources (no network). |
| `trace` | A serialisable `Trace` describing a scene + a `replay` runner: `Trace -> image`. |
| `golden` | Perceptual comparison against committed reference PNGs, regenerated with `UPDATE_GOLDEN=1`. |

## Adding a render test

No Rust required — add a trace and a reference:

1. Drop a `tests/traces/<name>.json` describing the scene (see existing
   traces). Layers reference named synthetic sources.
2. Generate the reference under a software adapter:
   ```sh
   UPDATE_GOLDEN=1 cargo test -p turbomap-golden --features gpu-tests
   ```
3. Reference the trace from a `#[test]` in `tests/golden.rs` with a
   tolerance, and commit `tests/golden/<name>.png`.

The trace format is intentionally open: a later phase can add a source
that replays **recorded real tiles** from a live session, turning
real-world traces into first-class fixtures.

## Running

GPU golden tests are behind the `gpu-tests` feature, so the default
`cargo test --workspace` lane (no GPU) compiles this crate but runs only
the non-GPU trace-format test.

```sh
# Fast lane (no GPU): trace-format contract only
cargo test -p turbomap-golden

# Golden lane: needs a wgpu adapter. A software one (Lavapipe) is enough:
#   sudo apt-get install -y mesa-vulkan-drivers
cargo test -p turbomap-golden --features gpu-tests
```

Set `REQUIRE_GPU=1` (as CI does) to turn "no adapter available" from a
skip into a hard failure. On a mismatch, the actual + diff images are
written to `apps/turbomap/target/golden-failures/`.

## Determinism note

References are captured on a software adapter (llvmpipe / Lavapipe), which
is deterministic for a fixed Mesa version but drifts slightly across
versions. Comparison is therefore **perceptual** (a per-channel tolerance
+ an outlier budget), not bit-exact. If a Mesa bump in CI moves pixels
within the noise, re-baseline with `UPDATE_GOLDEN=1` and review the diff.
