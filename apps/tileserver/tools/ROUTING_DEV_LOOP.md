# Autonomous routing development loop

A closed-loop, **server-free, deterministic** harness for iterating on the
routing engine without a human eyeballing maps. Change routing code →
one command → objective PASS/REGRESS verdict across four axes.

## TL;DR

```bash
cd apps/tileserver

# Measure the current tree against the committed baseline → verdict.
python3 tools/routing_loop.py

# After an intended improvement, accept the new numbers as the baseline.
python3 tools/routing_loop.py --update-baseline

# Iterate fast on a subset while developing (skip rebuild if binary is current).
python3 tools/routing_loop.py --no-build --filter trondelag
```

Exit code: `0` = PASS, `1` = REGRESS, `2` = harness error. Artifacts default
to `$TILESERVER_ARTIFACT_DIR` or `~/turbo-artifacts`.

## Why it exists

The router previously could only be evaluated by booting the HTTP server
and running `terrain_metrics.py` against it — and the in-process path
skipped the production vector/landcover layer stack (that wiring lived
only inside `serve()`). So there was no fast, faithful, headless way to
measure a routing change. This harness fixes that, so an agent can
develop the engine end-to-end on its own.

## How it works (the pieces)

1. **`routing_setup.rs`** (`turbo-tiles-bin`) — `load_routing_artifacts()`
   + `build_pathfinder()` construct the *identical* production layer
   stack `serve()` uses. The one source of truth for "how the router is
   wired", shared by the server and the evaluator. (Also the
   `RoutingExecutor` seam from the architecture plan.)

2. **`tileserver eval-terrain`** — loads artifacts + builds the
   pathfinder in-process (no DB, no socket), solves every ground-truth
   hike in `terrain-corpus.toml` with force-off-trail prefs (the only
   mode that exercises terrain decisions — see `terrain_metrics.py`),
   and writes per-hike JSON `{geometry, elevation profile, latency,
   geometry_hash}` + `_summary.json`. Elevation is sampled directly from
   the loaded DEM, so scoring needs no server.
   - `--check-determinism` solves the corpus twice and fails if any
     route's geometry hash differs (catches accidental nondeterminism).

3. **`terrain_metrics.py --offline DIR`** — re-scores the eval-terrain
   JSON with the *exact* trusted composite formula (gain/slope/fall-line/
   length/Fréchet), no HTTP. The same `metrics_from_profile` +
   `score_hike` code the online path uses.

4. **`routing_loop.py`** — chains build → eval → score → diff vs
   `routing-baseline.json` and prints the verdict.

## Two lanes — both routers are covered

The corpus endpoints are sti graph nodes, so prefs decide which router
runs (`Pathfinder::solve_inner` dispatch):

- **off-trail lane** (`--mode off-trail`, force-off-trail FMM): the
  solver re-derives routes from terrain — **quality vs ground truth is
  meaningful here**. This is the lane the composite score gates.
- **unified lane** (`--mode unified`, production-default prefs): the
  unified A* (mesh ∪ trail) that users actually hit. Quality vs truth is
  trivially ~100 (routes retrace the trails), but geometry hash,
  latency, and DEM work are real regression gates for the production
  router. Without this lane, changes to `unified.rs` are invisible.

## The verdict axes

| Axis | Gate |
|---|---|
| **Determinism** | hard fail if the two passes disagree (per lane) |
| **Solves** | fail if more hikes fail than baseline (per lane) |
| **Quality** | off-trail lane only: fail if corpus avg drops ≥ 0.5; lists per-hike drops ≥ 3.0 |
| **Latency** | fail if p95 > baseline×1.25 + 50 ms (per lane) |
| **Memory** | fail if peak process RSS > baseline×1.20 + 64 MiB (`getrusage`; includes faulted DEM/mask mmap pages) |
| **DEM work** | `dem_cache_lookups` (deterministic!) — fail if lookups grow vs baseline (per lane). The noise-free perf signal. |
| **Geometry** | lists exactly which routes moved (hash diff) — the precise change-detector (per lane) |

Thresholds live at the top of `routing_loop.py`.

## Baseline

`tools/routing-baseline.json` is the committed reference snapshot
(quality per hike, latency percentiles, per-hike + corpus geometry
hashes). It is produced with `--update-baseline`. **It is solver-mode
specific**: the current baseline is `cost_mode=fast_marching` (the
production default), offline-scored — its own internal reference, not
directly comparable to HTTP-measured numbers or other solver modes.
Regenerate it (intentionally) whenever you change the cost model,
corpus, or artifacts.

## Notes / limits

- Solves are deterministic on one machine, so quality + geometry are
  noise-free; **latency is the noisy axis** (machine load, a few
  multi-second solves dominate p95) — treat its gate as a coarse guard.
- Needs the full `~/turbo-artifacts` (DEM/graph/mask/vectors). Not in git.
- Memory is peak process RSS (`getrusage`), which counts resident
  mmap pages — so it's sensitive to how many DEM/mask tiles got faulted
  in during the corpus, not just heap. Good signal for "memory the
  solver touches", coarse like latency.

## Per-allocation profiling (dhat)

For the *heap* side — total bytes allocated, block count, peak live
bytes, and which call sites churn — build with the `dhat-heap` feature:

```bash
cargo build --release -p turbo-tiles-bin --features dhat-heap
RUST_LOG=error ./target/release/tileserver eval-terrain \
    --artifacts-dir ~/turbo-artifacts --limit 5 --out /tmp/dhat
# prints "Total: … bytes in … blocks / At t-gmax: …" and writes
# dhat-heap.json — open it in dhat/dh_view.html for per-call-site
# attribution.
```

The global-allocator shim taxes every allocation, so solves run much
slower under it — use a small `--limit` and skip `--check-determinism`.
For symbolized backtraces use the `profiling` cargo profile (release
speed + line debuginfo; the plain release profile strips symbols):

```bash
cargo build --profile profiling -p turbo-tiles-bin --features dhat-heap
./target/profiling/tileserver eval-terrain ...
```

Findings from the 2026-06 profiling pass (3 hikes, post-EdgeElevProbe):
~3 GB of the ~4.5 GB total churn is ONE-TIME BOOT (rstar tree build in
`TrailProximityContributor::new`, `VectorStore` blob parse) — harmless
steady-state. Per-solve churn (~0.5 GB / 1.6 M blocks per off-trail
solve) is diffuse: rstar query internals, polygon intersection math,
small per-cell probe vectors. No single dominant site remains.
