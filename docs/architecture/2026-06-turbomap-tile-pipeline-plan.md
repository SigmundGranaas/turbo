# Turbomap tile/streaming pipeline — measure, model, enforce

Status: plan (2026-06-22). Goal: make tile loading + rendering follow **clear,
testable rules** — deterministic ordering, the most-important data first, and no
flicker — replacing the current emergent, heuristic behaviour. Each slice is
gated by a **measurement** so we change numbers, not vibes.

---

## 1. Measured baseline (do not skip — this is the "before")

Tool: `crates/turbomap-app/examples/scenario.rs` — real Bodø/Sjunkhatten DEM +
Kartverket tiles over HTTP, scripted 51-step camera (pitch 0→80°, orbit, pan,
zoom, time-of-day), per-step timing. Run:

```
TURBO_API_URL=https://kart-api.sandring.no \
  cargo run --release -p turbomap-app --example scenario -- --center 67.23,15.30
```

Headless Metal on dev Mac, z13, 2026-06-22:

| State | working-set tiles | prepare CPU (resolver+LOD) | GPU pass | draw calls |
|---|---|---|---|---|
| Flat (0°) | **56** | 0.8 ms | 0.06 ms | 6 |
| Tilted (5–80°) | **440** | 1.7–2.6 ms | 0.1–0.2 ms | 7 |
| Pan / zoom @ 78° | 440 | **3.3–4.7 ms** | 0.2 ms | 7 |

**What it proves:**
- Tilting **8×'s the working set** (56 → 440 tiles). That set is what floods
  fetch → decode → cross-fade on first load (the "tiles loading over each
  other / chaos") and what drives cache pressure.
- `prepare` (the per-frame **resolver + LOD walk**, `render/raster.rs::prepare`
  + `lod.rs::select`) is the CPU hotspot and scales with the set size + pan;
  the **GPU pass is trivial** (batched to 7 draw calls). So perf work belongs
  in the *prepare/CPU + streaming* path, not the shaders.

**What it does NOT measure (the gap this plan closes):** the harness drains
tiles *synchronously* to steady state, so it can't see **first-load ordering /
jitter**. That signal lives only in the live engine and is currently
un-instrumented (see §3). Headless Metal also ≠ the Pixel's mobile GPU.

---

## 2. Architecture today: directed, with soft edges

Dataflow (real, not spaghetti):

```
camera → Scene.visible_tiles() (2D rect | 3D best-first SSE quadtree, lod.rs)
       → Scene.desired_tiles()  (+ overview backdrop / prefetch ring)
       → Scene.pending_tiles()  (desired − ingested, sorted nearest-first)
host   → planReconcile()         (TileReconcilePlan.kt — PURE, tested; lanes raster 32 / DEM 8)
       → fetch (OkHttp) → TurbomapTileCache (disk, read-through)
       → nativeIngest (enqueue bytes) → ingest channel
render → decode+upload, time-budgeted/frame → TextureCache (LRU by bytes; evict→un_ingest)
       → RasterPipeline.prepare resolver (Whole | AncestorPatch) + crossfade
       → draw → publish Snapshot (wait-free read model)
```

**Genuinely well-modelled seams (keep):**
- `lod.rs::select` — best-first `BinaryHeap` by screen-space-error span, distance
  metric, `MAX_TILES` budget, horizon fan. Deterministic per camera.
- `TileReconcilePlan.kt::planReconcile` — pure, unit-tested policy (cancel-stale
  frees slots same pass; start nearest up to cap; skip backoff; self-healing).
- Threading: actor/command-queue + wait-free `Snapshot` + GPU-cache↔`ingested`
  coherence (`un_ingest`).

**The soft edges (the actual problem):**

1. **A tile has no lifecycle type.** Its state is smeared across **6 collections
   in 3 layers**: `Scene.ingested` (core), `Surface.queued` (FFI), host
   `inFlight` + `retryAt`, `TextureCache` residency, overlay `first_seen`.
   "Is this tile loading / decoding / resident / shown?" is an AND/OR over all
   six, held consistent by hand-written bookkeeping. Every flicker/race bug this
   month lived in that bookkeeping.

2. **Priority is a single scalar — distance.** `pending_tiles()` sorts purely by
   tile-centre distance (`scene.rs`). There is **no tier** for
   *visible > prefetch > overview*, and raster/DEM split only by lane count. So a
   near *prefetch-ring* tile can fetch+decode before a farther *visible* tile.
   "Stream the most important first" is approximated, not stated.

3. **Desirable properties are emergent, not invariants.** Nothing asserts
   "no LOD regression," "visible before prefetch," "desired ≤ cache," or
   "same camera ⇒ same desired set." So they break and get spot-patched. Every
   recent bug was an emergent race, not a violated rule:
   flicker = projection elevated/flat lock race; crossfade flash = `first_seen`
   pruned on off-screen; coarse↔fine = desired-set > cache; load chaos = 440-tile
   burst with no tiering.

4. **Magic-number heuristics with no model:** ingest budget (8/6 ms), prefetch
   margin (256 px), "idle" = `cmds==0 && !animating`, fade 0.3 s.

5. **Telemetry is summary-only.** `FramePerf` (surface.rs) accumulates
   render_ms/ingest_ms/ingested/max_gap and logs periodically; `TurbomapTiles`
   logs pending/inflight/backoff. No **structured per-stage trace** and no
   **invariant checks** — so "is anything random/flickering?" can't be answered
   with data today.

**Verdict:** *directed with soft edges.* Flows + a few seams are solid; the
central noun (tile lifecycle) and verb (priority ordering) are not modelled, and
the properties we want aren't encoded — which is exactly why it feels random
under load.

---

## 3. Plan — slices, each gated by a measurement

### Slice 1 — Instrument (measure the truth first; low risk, no behaviour change)
- **Structured per-frame trace** in the published `stats_json` (already plumbed
  to the host): per-state tile histogram `{desired, fetching, decoding,
  resident, visible, evicted}`, per-stage ms `{select, prepare, ingest,
  render}`, `{evictions, backlog, pending, draw_calls}`, and `frame_gap_ms`.
- **Device capture**: a tiny `adb logcat` filter + a one-shot "cold-load trace"
  (first N seconds after surface create) dumped as CSV, so first-load ordering
  is finally visible.
- **Harness**: extend `scenario.rs` to emit the same per-stage CSV (it already
  has prepare/pass/tiles) so harness and device speak the same schema.
- **Gate:** baseline numbers recorded for cold-load (tiles-over-time, decode
  order, evictions) + steady-state (the table in §1). *Nothing else changes
  until we can see it.*

### Slice 2 — Model the tile lifecycle as ONE type
- Introduce `enum TilePhase { Desired, Fetching, Decoding, Resident, Evicted }`
  as the single source of truth in the core (replacing the 6 scattered sets),
  with explicit transitions. The host `inFlight`/`retryAt` and FFI `queued`
  become *views* derived from it, not parallel truths.
- **Invariants as property tests** (the spec, enforced):
  - *determinism*: same camera ⇒ identical `desired_tiles()`;
  - *monotonic LOD*: a visible cell's drawn LOD never regresses to coarser while
    a finer tile is resident (the anti-flicker law);
  - *bounded*: `desired ≤ cache capacity`.
- **Gate:** property tests green; the Slice-1 trace shows zero illegal
  transitions over the harness sweep.

### Slice 3 — Explicit priority tiers ("most important first" as a rule)
- Tag each desired tile `Visible{near,far} | Prefetch | Overview | DemForVisible`
  in `scene.rs`; order `pending` by **(tier, distance)**, not distance alone.
  The decode budget drains visible-tier before prefetch/overview.
- **Invariant test**: no prefetch/overview tile is fetched or decoded while a
  visible tile is still pending.
- **Gate:** cold-load trace shows visible tiles reaching `Resident` strictly
  before prefetch; time-to-first-full-viewport drops vs the Slice-1 baseline.

### Slice 4 — Capacity governor + retire the magic knobs
- Size the desired set to the cache by construction (the 440-tile set must fit
  with headroom) so coarse↔fine thrash is impossible, not just unlikely.
- Replace the ad-hoc ingest budget / prefetch margin / "idle" heuristic with
  values derived from the tier model + measured decode cost.
- **Gate:** thrash rate (evictions of still-desired tiles) → ~0 in the trace;
  first-load "chaos" (simultaneous cross-fades) bounded; steady-state prepare
  not worse than §1.

The flicker fixes already shipped (resident-keyed fade, always-elevated
projection, larger cache) become *consequences* of Slices 2–4's invariants
instead of standalone patches — and the projection-blocking choppiness gets a
principled fix (a wait-free elevation read off the snapshot, not a lock).

---

## 4. Measurement methodology (how we'll prove each slice)
- **Steady-state CPU/GPU**: `scenario.rs` per-step table (§1) — re-run per slice,
  compare prepare/pass/tiles. Deterministic, autonomous, no device.
- **First-load ordering + jitter**: the Slice-1 structured trace, captured on the
  Pixel during a cold load (and in the harness once it streams asynchronously).
- **Correctness/no-flicker**: the invariant property tests (Slice 2/3) — these
  are the regression gate, runnable headless in CI.
- **Honest scope**: headless Metal ≠ mobile GPU; the harness is the *CPU + set-
  size + ordering* oracle, the device is the *GPU + real-network* oracle. We use
  both and say which number came from which.

## 5. Risks / notes
- **Concurrent edits**: the `turbo` checkout is being modified by another agent
  (water rendering, currently non-compiling). This plan's core changes touch
  `scene.rs` / `raster.rs` / the FFI — coordinate to avoid colliding, or land on
  a branch.
- Slices 2–4 are real engine work (Rust core + host reconciler). Slice 1 is
  cheap and unlocks the rest; recommend doing it first and re-baselining before
  committing to the model change.
