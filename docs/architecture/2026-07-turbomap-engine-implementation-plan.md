# Turbomap Engine Implementation Plan — chunk-tree streaming, codecs, Surface

**Status:** plan · **Date:** 2026-07-03
**Companion to:** `2026-07-turbomap-decima-inspired-engine-architecture.md`
(the design + the binding **Decision Record D1–D12**). Absorbs and supersedes
the open slices of `2026-06-turbomap-tile-pipeline-plan.md`.

This plan turns the architecture's Phases 0–4 into commit-sized slices. Every
slice names the files it touches, the tests that gate it, and what "done"
means. Sizes: **S** ≈ one focused session, **M** ≈ a few sessions, **L** ≈ a
workstream of sessions.

---

## 0. Ground rules (the established discipline, restated as law)

1. **Measure → model → enforce.** No behaviour change ships before the
   instrumentation slice (A1) records its baseline, and every slice's gate is
   a number or a property test, not a vibe.
2. **Goldens are instruments, not gates** *(owner decision, 2026-07-04 —
   supersedes the original "parity-first" rule)*. The goal is a better,
   faster, more reliable engine — not an identical one. A golden diff is a
   tripwire that demands a LOOK, never a rule that different = wrong:
   inspect the diff, and if the new output is equal or better, re-baseline
   (`UPDATE_GOLDEN=1`) and move on. Never contort an improvement to
   preserve old pixels. The HARD gates are the ones about reliability, not
   sameness: the sim's behavioural invariants (never blank, no flicker,
   budgets, convergence), the property tests, and the conformance suite.
   Nuance kept: for a pure refactor whose *intent* is no visual change
   (e.g. the D1 frame-graph port), pixel-equivalence remains the cheapest
   possible evidence of correctness — use it as a free check there, but
   the moment the new structure enables something better, take the better
   thing and re-baseline deliberately.
3. **Commit and push every increment.** Container resets have rewound this
   clone twice; origin is the only durable state.
4. **Shims, not flag days.** The FFI surface (`pending_tiles`/`ingest_*`) and
   the Kotlin/JS hosts migrate behind deprecation shims; apps never break.
5. **New policy code is GPU-free and property-tested.** Lifecycle, priority,
   budgets, provider chains live in a crate with no wgpu dependency so the
   whole streaming brain runs in plain `cargo test`.
6. **Invariants 10–12** (no format past the codec; closed representation set;
   2.5D surface) get mechanical enforcement the moment their phase lands
   (grep-gates in CI, listed per slice).

---

## 1. Slice map

| # | Slice | Size | Depends on | Phase |
| --- | --- | --- | --- | --- |
| A1 | Structured streaming/frame trace + baselines | M | — | 0 |
| B1 | `turbomap-world` crate: chunk model + lifecycle | M | — | 1 |
| B2 | Priority score + tier ordering (parity, then fix) | M | B1 | 1 |
| B3 | `StreamingPlan` boundary + host shims | L | B1, B2 | 1 |
| B4 | Codec registry + off-render-thread decode + budgets | L | B1 | 1 |
| B5 | Provider chain: one disk cache, raster cached, PMTiles wired | M | B4 | 1 |
| B6 | Bundled baseline + offline cold-start gate | S | B5 | 1 |
| C1 | Scene IR: `environment` block + `Field2D` sources | M | — | 2 |
| C2 | Out-of-band APIs → scene-declared (clouds/sun/haze/routes) | M | C1 | 2 |
| C3 | Compositing honesty + `Capabilities` truth | S | — | 2 |
| D1 | Frame graph: `PassDesc` port, 1:1 parity | L | — | 3 |
| D2 | Subsystem registry: `Map` fields migrate | M | D1 | 3 |
| D3 | Surface seam extraction + DEM decode out of WGSL | L | D1, B4 | 3 |
| D4 | `Layer::Custom` real (phase-bound contributions) | M | D1, D2 | 3 |
| E1 | `Environment` value + consumers sample it | M | C2, D2 | 4 |
| E2 | Clouds as first `SimulationSystem` (deterministic tick) | M | E1 | 4 |
| M-TIN | Tileserver TIN experiment + `MeshSurface` behind flag | L | D3 | gated |
| M-MODELS | glTF-geometry codec + `InstanceSet` + object shadow map | L | D2, E1 | gated |
| M-3DTILES | `tileset.json` explicit-tree codec, real dataset | L | M-MODELS | gated |

Critical path: **A1 → B1 → B2 → B3** (the device-visible streaming fix).
B4/B5 can proceed in parallel with B3 after B1. C and D are independent of B
except where noted and can interleave.

---

## 2. Workstream A — instrument first (Phase 0)

### A1 — Structured trace + baselines (M)

**Goal:** make first-load ordering, lifecycle churn, and per-stage cost
visible with one schema everywhere, before anything changes.

**Changes**
- `turbomap-core/src/map.rs` (`FrameMetrics`) + `turbomap-core/src/scene.rs`:
  per-frame histogram over the existing `TilePhase`/`TileTier`
  (`{desired, pending, resident, retained}` × tier), per-stage ms
  (`select / prepare / ingest / render`), evictions, backlog, `frame_gap_ms`.
- `turbomap-ffi/src/surface.rs`: extend the published `stats_json` snapshot
  with the histogram; add a one-shot "cold-load trace" ring buffer (first N s
  after surface create) dumpable via a new FFI call.
- `turbomap-app/examples/scenario.rs`: emit the same schema as CSV per step.
- `turbomap-sim/src/perf.rs`: persist the histogram in `PerfSummary`.

**Gates / done**
- Cold-load and steady-state baselines committed to `target/sim-reports/`
  reference files (and recorded in this doc's progress log).
- No behaviour change: all goldens + sim gates untouched.

---

## 3. Workstream B — the Streaming System (Phase 1)

### B1 — `turbomap-world`: chunk model + lifecycle table (M)

**Goal:** the general types (D2/D9) plus the single source of truth for
resource state — GPU-free, property-tested.

**Changes**
- New crate `apps/turbomap/crates/turbomap-world` (deps: `serde`, nothing GPU):
  - `ChunkKey { layer: WorldLayerId, node: NodeId }`, `ChunkMeta { bounds,
    geometric_error_m, refine }`, `TreeShape::ImplicitQuadtree(PyramidSpec)`
    (+ `Explicit` stub), `Refine::{Replace, Add}`.
  - `ImplicitQuadtree` wraps today's `TileId` math (`turbomap-core/src/tile.rs`
    moves here or is re-exported; `ancestor/children/sub_uv_in` unchanged).
    `geometric_error_m` for a quadtree node = tile ground resolution (derived,
    not stored) — the number `lod.rs` SSE already effectively uses.
  - `Lifecycle`: the `ResourcePhase { Desired, Fetching, Decoding, Resident,
    Retained }` table with explicit, checked transitions and per-phase
    metadata (tier, priority, cancel token, bytes, last_used_frame).
- **Design-validation test (D3 gate):** a checked-in miniature 3D Tiles
  `tileset.json` fixture is mapped onto `ChunkKey/ChunkMeta/TreeShape` in a
  unit test — proving region/box bounds, error-in-meters, REPLACE/ADD, and
  lazy children all land losslessly *before the types freeze*. No rendering.

**Tests**
- Property tests: no illegal transition sequence representable; determinism
  (same inputs ⇒ same table); `desired ≤ capacity` (port `capacity.rs`'s
  compile-time proof to the general table).

**Done when** the crate builds in the workspace, the 3D Tiles mapping test
passes, and nothing else references it yet (pure addition).

### B2 — Priority as an explainable score (M)

**Goal:** replace "sort by (tier, distance)" spread across
`scene.rs::pending_prioritized` (`turbomap-core/src/scene.rs:427`) and the
desktop re-sort (`turbomap-app/src/map_host.rs:205`) with one function.

**Changes**
- `turbomap-world::priority`: `Priority = f(tier, sse_benefit, motion,
  layer_class)` with a decomposed debug form (each term inspectable).
  `sse_benefit` computed from `ChunkMeta` (error × projected size), `motion`
  from camera velocity via `dot(velocity, dir_to_chunk)`.
- `turbomap-core`: `Map::pending_tiles()` (`map.rs:1870`) orders by the new
  score. Tier enum gains `SurfaceForVisible` (renames `DemForVisible` intent).

**Tests / gates**
- **Parity first:** with zero camera velocity and equal errors, the new score
  reproduces the current (tier, distance²) order — locked by a unit test over
  recorded pending sets from the A1 trace.
- **Then the fix:** sim invariant test — no `Prefetch` chunk enters `Fetching`
  while a `Visible` chunk is `Desired` (the tile-pipeline plan's Slice-3 gate).
- Cold-load time-to-first-full-viewport improves vs A1 baseline in
  `turbomap-sim` (3-frame latency journey).

### B3 — `StreamingPlan` boundary + host shims (L)

**Goal:** policy lives in the engine; hosts become transports. Cancellation
exists.

**Changes**
- `turbomap-core` / `turbomap-engine`:
  `fn streaming_plan(&mut self) -> StreamingPlan { start: Vec<FetchRequest>,
  cancel: Vec<RequestId> }`, budget-truncated, priority-ordered, derived from
  the B1 lifecycle table. `ingest(id, bytes)` keyed by `RequestId`.
  `pending_tiles()` (`engine.rs:397`) reimplemented as a shim over the plan
  (start-only, no cancel) and marked deprecated.
- `turbomap-ffi/src/lib.rs`: expose `streaming_plan_json()` + `ingest(id,
  bytes)`; keep the old trio as shims. `surface.rs`: the `Ingest` channel
  carries `(RequestId, bytes)`.
- Android host: `TileReconcilePlan.kt` shrinks — cancel-stale/backoff/inflight
  caps move engine-side; Kotlin keeps only OkHttp dispatch + cancellation of
  its own calls (honoring `plan.cancel`).
- `turbomap-web/src/lib.rs`: `streaming_plan()` → JS `fetch` with
  `AbortController` per request — the web host gets policy for free.
- `turbomap-app/src/map_host.rs`: `dispatch_fetches` consumes the plan;
  `MAX_INFLIGHT_PER_LAYER`, `recently_failed`/backoff move into engine
  budgets/policy (retry classification stays in `turbomap-tiles-http::retry`).

**Tests / gates**
- Sim: fast-pan scenario measures **cancelled-before-decode ratio** — stale
  work abandoned, wasted-decode ms drops vs A1 baseline.
- FFI roundtrip test (`turbomap-ffi/tests/`): plan → ingest by id → resident.
- Conformance suite gains a `check_streaming_plan_determinism` clause (same
  camera+scene ⇒ same plan).
- Existing sim gates (blank-map, convergence, cache budgets) stay green
  through the shim swap.

### B4 — Codec registry + decode off the render thread + budgets (L)

**Goal:** formats die at one boundary (D5, invariant 10); FFI/web stop
decoding on the render/main thread.

**Changes**
- `turbomap-engine/src/codec.rs` (new): `trait Codec { fn decode(&self, meta,
  bytes) -> Representation }` with `Representation::{Texture2D, HeightField,
  FeatureSet, MeshChunk, Field2D, TreePage, InstanceSet}` (CPU-side forms;
  `MeshChunk`/`InstanceSet`/`TreePage` are typed stubs until M-slices).
  Registered codecs v1: `png/jpeg/webp → Texture2D`, `terrain-rgb/terrarium →
  HeightField` (decode uses `dem.rs::decode_elevation` — the formula's new
  single home), `mvt → FeatureSet`.
- Worker pool inside the engine (native: threads + crossbeam, mirroring
  `turbomap-app/src/runtime.rs`'s pumps; wasm: keep chunked main-thread decode
  under the frame budget — same interface). `ingest(id, bytes)` enqueues;
  decoded `Representation`s apply on the render thread under
  `StreamingBudgets { decode_ms_per_frame, upload_bytes_per_frame }`
  (generalizing the Android adaptive 6/8 ms budget from
  `turbomap-ffi/src/surface.rs:702` into the engine).
- **Raw ingest escape hatch (D5):** `ingest_representation(key,
  Representation)` — used immediately by `turbomap-sim` and goldens (faster,
  and it exercises the API).
- `HeightField` upload switches terrain tiles to `R16Float`/`R32Float`
  textures (prep for D3; raster path unchanged).

**Tests / gates**
- Sim gate: a 100-tile cold burst on the FFI path never exceeds the per-frame
  ingest budget (extends the existing render-thread-stall gate pattern).
- Codec unit tests per format incl. failure→`Retained`-not-poisoned behaviour.
- Grep-gate (CI): `image::load_from_memory` and `turbomap_mvt::decode` appear
  only under `codec.rs`/the worker pool.

### B5 — Provider chain: one cache, raster cached, PMTiles wired (M)

**Goal:** close P4 — the bundled-baseline seam exists.

**Changes**
- `turbomap-tiles-cache` is **rewritten as the one disk cache**: move the
  bounded-LRU implementation from `turbomap-tiles-http/src/cache.rs` here;
  delete the unbounded `DiskCachedSource`; `turbomap-tiles-http` depends on it
  and `HttpRasterSource` gains `with_cache_dir` (raster finally cached).
- `turbomap-scene`: `SourceDef::{PmtilesBundle { path }, PmtilesRemote { url }}`.
- `turbomap-engine/src/host_resolver.rs`: resolve the new variants to
  `turbomap-tiles-pmtiles` sources (file / HTTP-range readers — already
  byte-identical-tested); in-process fetch for bundle, host-transport for
  remote range requests.
- `turbomap-tiles-pmtiles`: add brotli decompression (`lib.rs:270`
  `UnknownCompression` branch).
- Provider chain formalized in `turbomap-world`: ordered `Vec<ProviderRef>`
  per layer; first-hit-wins semantics unit-tested with fake providers.

**Tests / gates**
- Conformance: scenes with pmtiles sources diff/apply like any other.
- `omt_pmtiles.rs` engine test extends to the resolver path (not just direct
  construction) — the reader is no longer orphaned.
- Cache unit tests: budget eviction, atomicity, raster+vector sharing one root.

### B6 — Bundled baseline + offline cold-start gate (S)

**Changes:** ship a small committed baseline archive (extend the existing
1.2 MiB Bergen fixture toward a coarse z≤8 Norway extract; CI size budget on
the artifact); desktop/app wiring behind `TURBO_BASELINE_BUNDLE=path`.

**Gate:** new sim test — **network disabled, cold start**: full viewport
coverage from the bundle alone (blank < 1%), zero fetch requests issued below
the bundle's max zoom; the A1 trace proves the provider chain order.

---

## 4. Workstream C — Scene IR absorbs the side doors (Phase 2)

### C1 — `environment` block + `Field2D` sources (M)
- `turbomap-scene/src/scene.rs`: `Scene.environment: EnvironmentDef`
  (lighting mode fixed/time-tracked/host, haze, shadow toggles) +
  `SourceDef::Field2D` (radar/wind grids as world layers).
  `diff.rs` gains `EnvironmentChange`; `ModelEngine` stores it; conformance
  adds `check_environment_diffing` + `check_field_source_update`.
- Serde round-trip tests (`scene_serde.rs`).

### C2 — Out-of-band APIs become scene-declared (M)
- `turbomap-engine`: `set_sun_time`, `set_terrain_shadows/lit`,
  `set_aerial_haze`, `set_basemap_gain`, `enable_clouds` + cloud params, route
  tubes → all applied via `reconcile` from the Scene; the inherent methods
  become shims that mutate a scene overlay and are marked deprecated.
  `ingest_radar_frame` reroutes as a `Field2D` chunk ingest through B3/B4.
- **Gate:** `inspect` reproduces the entire engine state from the Scene alone;
  goldens unchanged (config-flow refactor only); FFI shims keep Android green.

### C3 — Compositing honesty (S)
- Either circles rejoin the ordered layer stack (marker pass draws per-slot)
  or the contract documents the overlay track explicitly; `TurbomapEngine::
  capabilities()` reports `data_driven_paint`/`custom_layers` truthfully;
  conformance adds a cross-track ordering check. (Small, but it removes the
  "the IR lies" precedent before D4 builds on it.)

---

## 5. Workstream D — Frame graph + Surface (Phase 3)

### D1 — `PassDesc` port, 1:1 (L)
- `turbomap-core/src/render/graph.rs` (new): `PassDesc { name, phase, reads,
  writes }`, `FramePhase::{BeforeFrame, GroundMsaa, OverlayMsaa, Post,
  Composite}`, scheduler ordering by declared reads/writes.
- `Map::render` (`map.rs:2417`) becomes: build `FrameCtx` → schedule → run.
  Existing passes register exactly today's order/attachments: shadow
  heightfield + AO in `BeforeFrame`; sky/floor/layers in `GroundMsaa`; route/
  icons/text/markers in `OverlayMsaa`; bloom/tonemap in `Post`; clouds in
  `Composite`. Per-pass GPU timestamps ride the existing
  `gpu_timestamps.rs` scopes.
- **Gate:** sim gates + conformance green; perf not worse (per-pass ms in
  `stats_json` vs the A1 baseline). Goldens are checked as an instrument:
  diffs are reviewed, and re-baselined when equal-or-better (see ground
  rule 2) — pixel identity is NOT a merge condition.

### D2 — Subsystem registry (M)
- `trait Subsystem` (architecture §III.6) in `turbomap-core`; migrate `Map`'s
  god-fields into registered subsystems (Basemap, Terrain, Symbols, Overlays,
  Atmosphere); pipeline ownership collapses to subsystems (ends the
  split-borrow dance at `map.rs:2548`).
- **Gate:** registry meta-test — every registered subsystem returns budgets,
  inspect JSON, ≥1 debug view; goldens unchanged.

### D3 — Surface seam + DEM decode out of WGSL (L)
- `turbomap-core/src/surface.rs` (new): the `Surface` trait (architecture
  §III.1.e). `HeightfieldSurface` wraps today's `TerrainShared`/`TerrainCache`
  (`render/terrain.rs`) — per-pipeline displacement stays its private detail.
- Consumers move to Surface queries: shadow heightfield assembly
  (`update_terrain_shadows`, `map.rs:2133`), AO, `elevation_at_world`
  (markers/route drape), camera pitch clamp (`sync_scenes`), hit-testing.
- With B4's `HeightField` codec output, DEM textures upload as real heights
  (`R16Float`); `decode_elevation` is deleted from `shader.wgsl:212` and
  `DemEncoding` disappears from `frame.rs`/`style.rs`/`terrain.rs` signatures.
- **Gates:** goldens compare-mode (terrain scenes bit-stable within perceptual
  tolerance); grep-gate: `DemEncoding|RasterFormat` only in codecs;
  sim terrain-stall gate stays green.

### D4 — `Layer::Custom` real (M)
- `Custom { id, kind }` binds to a host/engine-registered `RenderContribution`
  in a declared phase; `capabilities().custom_layers = true` honestly.
- **Gate:** a demo custom layer (e.g. animated flow-field) renders identically
  on desktop + web from one Rust impl; conformance keeps `ModelEngine` honest.

---

## 6. Workstream E — Environment + first simulation (Phase 4)

### E1 — `Environment` value (M)
- `turbomap-core`: `Environment { time, sun, atmosphere, wind, season,
  fields }` built once per frame in `RenderFrame::build` (`render/frame.rs`);
  sky/haze/shadows/hillshade/lighting derive uniforms from it — the
  "patched in Map::render" fields (`frame.rs:195–209`) collapse.
- **Gate:** golden diffs reviewed per ground rule 2; one write site for every environmental uniform
  (grep-gate on `haze_|sun_dir` writes).

### E2 — Clouds tick as `SimulationSystem` (M)
- `turbomap-clouds` drives from `tick(dt, env)` (wind-driven drift, radar
  frame advection); sim activity registers as animation — deletes the manual
  `request_redraw` wart (`turbomap-app/src/app.rs:729`).
- **Gates:** deterministic replay — same `(fields, time, seed)` ⇒ identical
  frame (golden at fixed time); storm scenario in sim shows coherent
  cloud+lighting response; frame budget holds with sim on.

---

## 7. Gated milestones (after D3 / E1 — scheduled when their gate opens)

- **M-TIN:** tileserver crate emits TIN mesh tiles (quantized-mesh or own
  payload) from ingested Kartverket DEMs for one region (server work in
  `apps/tileserver`); `MeshSurface` implements the Surface trait behind
  `TURBO_MESH_TERRAIN=1`; goldens/sim/scenario compare fidelity, memory,
  bandwidth vs `HeightfieldSurface`. **Adoption is a data decision.**
- **M-MODELS:** geometry-only glTF codec → `MeshChunk`/`InstanceSet`;
  placed-model scene layer; stylized materials from the map palette lit by
  `Environment` (D11); compact sun-space shadow-map pass for objects (D12)
  registered as a `BeforeFrame`+`GroundMsaa` contribution pair.
- **M-3DTILES:** `TreeShape::Explicit` + `tileset.json` codec (`TreePage`
  streaming through B3's plan); validated against one real Norwegian 3D-bygg
  extract, styled per D11.

---

## 8. Sequencing, parallelism, and what ships when

- **Now:** A1 → B1 → B2 → B3 in order (each independently shippable; B3 is
  the first device-visible payoff: no stale fetches, one policy everywhere).
- **Parallel lane 1:** B4 → B5 → B6 after B1 (codec/provider work doesn't
  block the plan boundary).
- **Parallel lane 2:** C1–C3 any time (scene crate is decoupled); C2 before
  E1.
- **Then:** D1 → D2 → D3/D4 → E1 → E2 → milestones.
- Device validation remains the standing Phase-0 gate from the global-map
  roadmap: first on-device session after B3 and after D1 re-baselines budgets.

## 9. Risks specific to execution

| Risk | Mitigation |
| --- | --- |
| B3 touches core scene bookkeeping the app ships on | Lifecycle table lands *behind* the existing sets first (dual-write, A1 trace asserts agreement for a full sim sweep), then the old sets delete. |
| Concurrent agents/sessions editing `apps/turbomap` | Same rule as the tile-pipeline plan: land on this branch, rebase small, push every increment. |
| wasm decode can't thread | Interface identical; wasm keeps budgeted main-thread decode. Revisit with wasm threads only if sim-on-wasm shows budget misses. |
| Kotlin reconciler shrink regresses Android | Shims keep the old path callable; the Kotlin reconciler is deleted only after a device session on the plan path matches its A1-baselined numbers. |
| Golden churn from D3's R16F height switch | Ground rule 2: review the diffs, re-baseline when equal-or-better — pixel identity is not a merge condition. |

## 10. Progress log

- _2026-07-03_: Plan authored against the architecture doc + Decision Record.
- _2026-07-03_: **A1 (delta) landed.** The FFI `FrameTrace`/`stats_json` +
  scenario CSV already implemented the old Slice-1 schema; what was missing
  was the lifecycle histogram. Added `TileHistogram`
  (desired/pending/resident/retained + pending-per-tier) computed per
  `Scene` (`scene.rs::phase_histogram`), summed across layers + terrain into
  `FrameMetrics::tiles` (`map.rs::tile_histogram`), published in the device
  `stats_json` (`desired`/`retained`/`pend_overview`/`pend_visible`/
  `pend_prefetch` keys — schema-gate test extended), in the scenario CSV
  columns, and in the sim's `FrameStats`/`PerfSummary`
  (`desired_max`/`retained_max`).
  **Recorded baseline** (llvmpipe, `frame_cost_stays_within_budget`,
  z12 pan session, 3-frame latency): frames 46, cpu p50/p95/max
  0.30/0.40/0.54 ms, worst_blank 0.019 %, tiles 228, **desired_max 126,
  retained_max 252** — the eviction-candidate pressure is now a number.
  Execution note (the standing rule): the sim gates SKIP silently without a
  wgpu adapter — always run them with `REQUIRE_GPU=1` (this session: installed
  `mesa-vulkan-drivers` + `protobuf-compiler` in the dev container first, and
  the first "green" run was a vacuous skip until the adapter existed).
- _2026-07-03_: **Harness forensics + repair.** Running the repaired gates for
  real exposed that CI's rust lane had been dead since 2026-06-28 (clippy
  `-D warnings` errors in `render/post.rs` + `capacity.rs`), so NO behavioural
  gate had run for a week of heavy rendering work. Everything the dead lane
  masked, found by running + inspecting (the `coldload_dump` tool: frame PNG,
  colour census, per-layer cache bytes, per-gate measurements):
  1. **Leftover HDR bloom + ACES tonemap** silently regrading the whole map
     since the water-feature revert (goldens never re-baselined; every
     screen-space sim assertion broken; the blank-map gates *defanged* —
     nothing on screen matched the authored clear, so `blank_frac` was
     0 forever and the gates could not fail). Owner decision: **complete the
     revert** — post pipeline removed, frame resolves straight to the surface;
     `golden_raster_parchment` passes against its untouched reference again.
  2. **Vector water fills deleted engine-wide** by a hardcoded source-layer
     skip in the tessellator (`86511549` — a raster-hybrid product decision
     lodged in the wrong layer). Replaced by
     `VectorStyle::without_water_fills()`: the app's raster-hybrid styles opt
     out at STYLE build time; every pure-vector basemap (sim, N50, Bergen
     fixtures) renders declared water again. `cold_load_paints_every_
     subsystem` fully green, lakes included.
  3. **Desktop demo defaulted to the water-only debug style** (leftover);
     full style is the default again, `TURBO_WATER_ONLY=1` keeps the debug
     mode.
  4. **Camera round-trip contract violation**: `Camera::sanitized`'s
     unconditional lng wrap cost ULPs (5.32 → 5.319999999999993), failing
     `check_camera_roundtrips`. In-range longitudes now pass through
     bit-exactly (unit-tested); only out-of-range values wrap.
  5. **Two stale goldens re-baselined deliberately** (`hillshade-bergen`,
     `omt-bergen-3d`): both predate the intentional June-25→July-1 aerial-
     perspective/lighting series; the old `omt-bergen-3d` reference also
     contains the water-smear artifacts of the buggy era it was captured in —
     the new render is visibly cleaner (proper water bodies, lit/shadow walls).
  Sim `ONSCREEN_*` constants now equal the authored palette (the seam and the
  re-baselining tool stay — this failure mode can't be reintroduced silently).
- _2026-07-03_: **B1 landed** — the `turbomap-world` crate (GPU-free,
  IO-free, clock-free). `ChunkKey`/`NodeId`/`ChunkMeta` (region/box/sphere
  bounds, geometric error in meters, Replace/Add refine);
  `TreeShape::ImplicitQuadtree(PyramidSpec)` as instance #1 with `QuadKey`
  node packing, Mercator regions, and the standard ground-resolution error
  table (z0/256px ≈ 156 543 m, halving per level — unit-pinned); the
  `Lifecycle` table (Desired/Fetching/Decoding/Resident/Retained) with
  transitions as fallible methods (`WrongPhase`/`StaleRequest`/
  `DesiredSetFull`), `RequestId`-scoped attempts, the eviction-re-pends
  coherence law as a transition, and the plan views
  (`pending`/`cancelable`/`eviction_candidates`/`histogram`). Property
  gates: deterministic LCG fuzz holds `wanted-missing ≤ capacity`,
  histogram-total, and request-carrying invariants over any op sequence,
  and replays identically. **D3 design gate passed:** a real-shaped
  `tileset.json` fixture (mixed region/box/sphere volumes, refine
  inheritance incl. an ADD override, non-quadtree branching,
  content-less interior nodes, per-node error) maps losslessly onto the
  types (`tests/threedtiles_mapping.rs` is the future codec's executable
  spec). Pure addition: nothing references the crate yet; core adopts
  the keys in B2/B3. Deviation from the plan sketch: `tile.rs` was NOT
  moved out of core (it leans on core's `geo`); `QuadKey` mirrors its
  semantics self-contained, and the field-for-field bridge happens at B3
  where that churn is already budgeted.
- _2026-07-03_: **B2 landed** — priority as one explainable score.
  `turbomap_world::priority`: `Priority(u64)` packs tier (the law, 2 bits)
  over IEEE-bit-ordered effective distance², with 30 bits reserved for the
  S6 SSE-benefit term; `Tier` gains the reserved `SurfaceForVisible`
  variant (activates in a later measured slice — today DEM maps to
  `Visible` to preserve the shipped interleave). The motion term is live:
  `Map::pending_tiles` derives the camera's travel direction (finite
  difference of the eye between calls, `Cell`-memoized) and modulates each
  chunk's effective distance by `dot(travel, dir_to_chunk)` up to ±30 % —
  stream where the user is heading, never enough to cross tiers.
  **Parity pinned twice:** a world-crate LCG fuzz orders arbitrary
  (tier, distance) sets identically to the historical lexicographic
  oracle, and `scene::tests::pending_priority_matches_the_historical_
  order_when_stationary` sweeps real cameras through the live selection.
  **Desktop host fixed:** `map_host::dispatch_fetches` consumed the
  engine's order until now only to RE-SORT it by raw centre distance,
  discarding tiers (a near prefetch tile could fetch before a farther
  missing visible tile); it now spawns in engine order. All 7 sim gates
  green post-change. Note: the "time-to-first-full-viewport improves"
  gate needs motion + a live host — measured on-device with B3's plan
  boundary; the sim's stationary cold load is the parity case by design.
- _2026-07-03_: **B3.1 landed — dual-write.** The `Lifecycle` table now
  shadows the legacy per-scene bookkeeping inside `Map`: the want-set
  syncs against every layer's + terrain's desired set on each
  `pending_tiles()` (visibility-independent, matching the A1 histogram's
  universe), deliveries mirror through `delivered_unrequested` (the
  documented legacy-shim transition B3.4 deletes), cache evictions
  through `evicted()`, and layer/terrain teardown through
  `forget_layer`. `WorldLayerId`s are minted per layer (`0` reserved for
  terrain); `ChunkKey`s pack tiles via `QuadKey`. **The agreement gate**
  (`Map::lifecycle_agreement`, surfaced on the engine) compares the
  table's histogram against the scenes' phase histogram —
  `Sim::step` asserts it on EVERY frame, so all 7 behavioural gates now
  sweep it continuously (856 s run green). Capacity is effectively
  unbounded during dual-write; the governor activates when the table
  becomes the source of truth. Next: B3.2 — `StreamingPlan { start,
  cancel }` derived from the table, `pending_tiles` becomes its shim.
- _2026-07-03_: **B3.2 landed — the StreamingPlan.**
  `Map::streaming_plan(max_start)` returns `{ start: Vec<FetchRequest
  { RequestId, PendingTile }>, cancel: Vec<RequestId> }`: starts are
  priority-ordered, budget-truncated, and minted through the table's
  `fetch_started` (a live attempt is never handed out twice); cancels
  are the stale in-flight list — the verb the pull-only contract never
  had. Completion is implicit through the existing `ingest_*` calls;
  `fetch_failed`/`fetch_cancelled` report the other outcomes (world
  gains `cancelled` + `key_of_request`). `pending_tiles()` is now the
  documented start-only shim over the same `plan_selection`. The
  agreement gate was generalized for live plans (scenes' `pending` ==
  table's Desired+Fetching+Decoding minus stale). Gate:
  `tests/streaming_plan.rs` walks start → deliver → fail-repend →
  move-away-cancel → acknowledge with agreement asserted at every step;
  engine + sim suites green. Next: B3.3 — FFI/web hosts consume the
  plan (AbortController on web); Kotlin reconciler shrinks.
- _2026-07-03_: **B3.3 landed — the web host is the first full plan
  consumer.** One serializer (`engine::streaming_plan_to_json`,
  unit-tested) feeds both bindings: wasm `streaming_plan(max_start)` +
  `report_fetch_failed/cancelled`, and uniffi `streaming_plan_json` +
  the same reports (u64 ids; session-scoped counters, exact in a JS
  number). `apps/web`'s `TileLoader` now consumes the plan: sizes
  `max_start` to its free lane capacity, aborts + acknowledges every
  `cancel` (the client-side pending-list diffing that used to re-derive
  this decision is deleted — the engine's table states it), declines
  lane-mismatched starts by reporting them cancelled so they re-issue,
  and reports failures so chunks re-pend. `pending_tiles` remains the
  documented legacy shim on every binding — desktop and the Kotlin
  reconciler migrate in B3.3b, then B3.4 deletes the legacy sets and
  arms the real capacity governor. Verified: wasm target builds,
  wasm-pack pkg regenerated, apps/web typecheck + 16 vitest green,
  clippy clean, engine plan-loop gate green. (Housekeeping: the dev
  container's 22 GB stale debug-profile artifacts were pruned to make
  room for the wasm toolchain.)
- _2026-07-03_: **B3.3b landed — the desktop host adopts the plan.**
  `map_host::dispatch_fetches` takes one `streaming_plan` sized to its
  free lane capacity; declined starts (lane mismatch, retry backoff,
  unsupported kind) report `fetch_cancelled` so the engine re-issues
  them; worker failures report `fetch_failed` via a `(layer, tile) →
  RequestId` attempts map; deliveries complete implicitly. Plan
  `cancel`s are acknowledged immediately — blocking `reqwest` can't
  abort mid-flight, but the inflight set prevents duplicate spawns and
  a late delivery completes whatever attempt is current. Core
  re-exports `RequestId` so hosts don't need a direct world dep.
  **Remaining on the shim: the Kotlin/Android reconciler** — the uniffi
  plan API is ready (`streaming_plan_json` + reports), but the
  reconciler swap needs an Android build environment to verify, so it
  is deferred to a session with one rather than landed blind (the
  execution rule: nothing ships unrun). Then B3.4: legacy sets +
  `delivered_unrequested` delete, capacity governor arms.
- _2026-07-04_: **B5.1 landed — one disk cache; raster finally cached.**
  The bounded LRU `DiskCache` (byte budget, mtime recency, atomic
  temp-then-rename writes, sweep-every-64) moved from `turbomap-tiles-http`
  into `turbomap-tiles-cache` as the crate's sole content — the ONE disk
  cache of the provider chain, deliberately format-blind (keys are relative
  paths, values are bytes; no `turbomap-core` dependency). The unbounded
  `DiskCachedSource<S>` adapter it replaces (wired to nothing, a latent
  disk-fill hazard) is deleted. `turbomap-tiles-http` re-exports `DiskCache`
  for back-compat and `HttpRasterSource` gains `with_cache_dir`/
  `with_cache` — raster/DEM tiles were re-fetched on every cold start until
  now. Raster and vector stores share one `<z>/<x>/<y>` layout (asserted by
  test). Gates: cache-hit-without-network tests for both source kinds
  (blackhole-URL technique), persistence-across-instances, budget/LRU suite
  moved intact; 52 workspace suites green, clippy clean. The sim/golden
  behavioural gates are out of this diff's dependency graph (neither crate
  is a dependency of sim/golden/engine — only the desktop host consumes
  them), so the unit lane is the full gate here. Remaining in B5: hosts
  opting the raster sources
  into the cache dir, `SourceDef::{PmtilesBundle,PmtilesRemote}` + resolver
  wiring, brotli in the PMTiles reader (B5.2).
- _2026-07-04_: **B5.2 landed — PMTiles declarative in the Scene; brotli.**
  `SourceDef::{PmtilesRaster, PmtilesVector, PmtilesDem}` with a single
  `location` field — a filesystem path (bundled baseline) or http(s) URL
  (serverless range requests); bundled-vs-remote is packaging, not a schema
  fork (D2/D7). Zoom bounds come from the archive header at resolve time;
  the DEM variant carries `encoding` + `halo` like `DemXyz` (halo reaches
  the mesh via a `WithDemHalo` wrapper — the archive header has no halo
  field). `HostDrivenResolver` resolves these to REAL in-process
  `PMTilesSource`s — the one remote kind the engine doesn't stub, because
  "range-read this archive" can't be expressed as a URL-template host fetch;
  a failed open degrades to `Unsupported` + a warning, never a panic. The
  dep is target-gated: on wasm (no std::fs / blocking reqwest) the variants
  resolve to `Unsupported` and `turbomap-web` still builds (verified on
  `wasm32-unknown-unknown`). The reader now decodes **brotli** tile/directory
  compression (`brotli-decompressor`, pure Rust; encoder round-trip test) —
  real planet archives ship brotli, and without it the bundled-baseline goal
  only worked for archives we repack ourselves. Zstd still errors clearly.
  Gates: serde round-trips (kebab-case tags, halo default), resolver
  serves a tile from a writer-built temp archive, missing-archive
  degradation, `is_supportable` accepts the new source kinds per layer;
  52 workspace suites green, clippy clean, wasm build green. (Sim gates:
  no sim scene declares a pmtiles source, so this diff is invisible to
  them; the `REQUIRE_GPU=1` re-run was kicked off at commit time and its
  result is recorded in the next entry.) Remaining in B5/B6: hosts opting
  raster sources
  into the disk cache, engine-level pmtiles read-through caching, the
  committed baseline extract + offline cold-start sim gate.
- _2026-07-04_: **B6 (core gate) landed — offline cold start proven at the
  engine seam.** New gpu-gated test `bundled_pmtiles_scene_is_fully_offline_
  via_the_production_resolver` (`turbomap-engine/tests/omt_pmtiles.rs`): the
  analytic OMT world packed into a real `.pmtiles` file, declared in the
  Scene as a `pmtiles-vector` source, resolved by the PRODUCTION
  `HostDrivenResolver` (no custom resolver, no URL hack), drained fully
  in-process by `pump_tiles`, then the offline invariant asserted —
  **`pending_tiles()` is empty** (nothing left for a host to fetch) — plus a
  pixel census proving water + roads actually rendered. Run green on
  Lavapipe (`REQUIRE_GPU=1`), alongside the untouched `omt-pmtiles-bergen`
  golden. Remaining in B6: a committed coarse Norway extract behind a CI
  size budget, host wiring (`TURBO_BASELINE_BUNDLE`), and the sim-level
  cold-start scenario — deferred until the provider-chain composition
  (bundled-under-remote layering) exists to wire them through.
- _2026-07-04_: **B5.2 sim confirmation recorded** (the pending result from
  the entry above): all 7 behavioural gates green under `REQUIRE_GPU=1` on
  Lavapipe, release profile, 622 s run — post-B5.2 tree, as expected for a
  diff no sim scene exercises.
- _2026-07-04_: **B6.2 landed — the provider chain, bundled-under-remote in
  the IR.** `SourceDef::Chain { providers }`: one source id, an ordered
  provider list; `validate()` rejects empty/nested/mixed-kind chains
  (`SceneError::InvalidChain`). `HostDrivenResolver` resolves every provider
  through itself and composes first-hit-wins (`ChainedTileSource` /
  `ChainedVectorSource`): a provider is consulted only inside its zoom
  range, the first `Ok` wins, and if nothing serves the tile the LAST error
  propagates — so a chain ending in a host stub yields the stub's
  "fetch me host-side" signal and the tile flows through the normal pending
  path with zero special cases downstream. Zoom coverage is the union of
  the providers'. On wasm the bundled provider resolves `Unsupported` and
  the chain composes what remains — same scene, graceful platform
  degradation. Gates: serde round-trip + the three validation rejections;
  resolver unit test (bundle serves its zoom, stub error propagates past
  it, union bounds); gpu gate `a_chained_source_renders_offline_and_
  surfaces_detail_to_the_host` — at the bundle's zoom every VISIBLE-zoom
  tile is served offline (the coarse overview floor below the fixture's
  single level correctly pends to the remote — per-tier invariant, not
  "pending empty"), and past the bundle the same source surfaces exactly
  the detail tiles for the host; 52 workspace suites green, clippy clean,
  wasm green, sim gates re-run `REQUIRE_GPU=1` (result in the next entry).
  Remaining in B6: the committed coarse Norway extract behind a CI size
  budget + host wiring (`TURBO_BASELINE_BUNDLE`) — now unblocked, since a
  host can declare `chain [bundle, remote]` without any engine change.
- _2026-07-04_: **B6.2 sim confirmation recorded:** all 7 behavioural gates
  green under `REQUIRE_GPU=1` on Lavapipe, release profile, 690 s run —
  post-chain tree (no sim scene declares a chain yet; the gate guards
  against regression in the shared resolver/engine paths).
- _2026-07-04_: **B4.1 landed — raster/DEM decode off the render thread.**
  New `turbomap-engine/src/codec.rs`: a `DecodeQueue` with two named worker
  threads on native (crossbeam job/result channels) and a budgeted inline
  drain on wasm (no threads) — one interface, per-platform mechanics.
  `ingest_raster_encoded`/`ingest_terrain_encoded` now ACCEPT bytes instead
  of decoding them on the calling thread (the render thread on Android, the
  main thread on web); decoded tiles apply at the top of `render()` under a
  6 ms `APPLY_BUDGET`, so a cold-load burst spreads across frames instead
  of pinning one. Three contract points, each load-bearing: (1) an
  in-flight **dedup set** — a tile stays in `pending_tiles` until its
  decode applies, so without dedup a host reconcile loop would re-enqueue
  every in-flight tile per pass; (2) **backlog counts as animating** —
  decoded tiles apply inside `render()`, so a sleeping render-on-demand
  host would strand them; (3) **decode failures clear the key and drop** —
  the tile re-pends and the host's retry/backoff owns the policy (the bool
  return now means "accepted", documented). `pump_tiles`' in-process path
  (golden/inspect/pmtiles bundles) is untouched — it never used the encoded
  entry points. MVT decode+tessellation stays synchronous (B4.2). Gates:
  codec unit tests (off-thread decode round-trip within budget, dedup,
  failure-clears-key-for-retry); 52 workspace suites, clippy, wasm build
  green; full engine gpu suite green incl. goldens/parity/incremental
  (test-only resolvers gained wildcard arms — gpu-gated code the default
  lane never compiled); sim gates re-run `REQUIRE_GPU=1` release (the real
  referee for the async-apply timing change) — result recorded in the next
  entry.
- _2026-07-04_: **B4.1 sim run caught a real defect — fixed forward.** The
  first `REQUIRE_GPU=1` release run came back **5/7**: `cold_load_paints_
  every_subsystem` and `pan_session_stays_covered_and_settles_without_
  flicker` failed. Diagnosis: with async apply, a tile stays in
  `pending_tiles` until its decode lands, so a host (and the sim) schedules
  it AGAIN; the duplicate delivery arrives after the tile is resident and
  re-ingests it — re-upload + fade restart = steady-state diff churn, and
  a cold load that struggles to settle. Exactly the class of timing bug
  the sim exists to catch. Fix: residency guards at the encoded-ingest
  boundary — `Scene::is_ingested` + `Map::{is_raster_ingested,
  is_terrain_ingested}`; `ingest_*_encoded` now accepts-and-drops
  deliveries for already-resident tiles (eviction still re-pends via
  `un_ingest`, so refresh semantics are intact). Verification re-run in
  flight; result in the next entry.
- _2026-07-04_: **B4.1 verified:** with the residency guard, all 7
  behavioural sim gates green under `REQUIRE_GPU=1` on Lavapipe, release
  profile, 679 s run — including the two that caught the defect
  (`cold_load_paints_every_subsystem`, `pan_session_stays_covered_and_
  settles_without_flicker`). Raster/DEM decode now runs off the render
  thread on every engine host with steady-state behaviour equivalent to
  the synchronous path. Remaining in B4: MVT decode+tessellation
  off-thread (B4.2) and retiring the FFI host's now-redundant ingest
  time-slicing in favour of the engine budget (B4.3).
- _2026-07-04_: **B4.2 landed — MVT decode + tessellation off the render
  thread.** The `DecodeQueue` gains `QueueKey::Vector`: `ingest_mvt` now
  accepts bytes, captures the layer's CURRENT `VectorStyle` (new
  `Map::vector_layer_style`) plus a **style epoch**, and the worker runs
  `decode_mvt` + lyon `tessellate` — the render thread only applies the
  finished mesh via `ingest_vector_mesh` under the same 6 ms budget.
  The epoch is the correctness keystone: every vector-layer (re)install
  bumps `vector_style_epochs[layer_id]` (monotonic, entries never removed
  so a re-added layer can't collide with a stale queued result), and apply
  drops any result whose epoch mismatches — a repaint/rebuild that races a
  decode re-pends the tile instead of painting stale style. Same residency
  guard as B4.1 (`Map::is_vector_ingested`) kills duplicate-delivery
  churn. With this, NOTHING decodes or tessellates on the render thread
  on any engine host — the desktop's rayon pumps are now the pattern
  everywhere, closing P3's worst half. Gates: codec unit tests incl.
  off-thread tessellate round-trip + epoch passthrough; 52 workspace
  suites, clippy, wasm green; all 13 engine gpu suites green; sim gates
  re-run `REQUIRE_GPU=1` release (result in the next entry).
- _2026-07-04_: **B4.2 sim run caught the next timing defect — the delivery
  echo.** First `REQUIRE_GPU=1` release run: **5/7**, failing
  `heavy_roaming_under_a_tight_cache_budget_keeps_reloading_tiles` and
  `terrain_cast_shadows_do_not_stall_the_render_thread_while_panning`, with
  total runtime blown from ~680 s to ~2690 s. Diagnosis: a delivered tile
  stays in `pending_tiles` until its decode APPLIES, so every pull-driven
  host refetches each tile once per decode latency — a delivery echo that
  multiplies settle frames (the heavy-roaming per-region 200-frame caps
  blew) and keeps tile churn running through the shadow gate's measurement
  windows. Fix: the engine now SUBTRACTS the decode queue's accept→apply
  window from `pending_tiles()` (`DecodeQueue::contains` + a
  `decode_key_of(PendingTile)` mapping; Hillshade pending shares the
  Terrain key since its ingest forwards to the shared terrain cache).
  Plan-driven hosts never had the echo — an issued request stays `Fetching`
  in the lifecycle table until its apply lands — so `streaming_plan` needs
  no filter. Fast lanes green (52 suites, clippy, wasm, 13 engine gpu
  suites); sim verification re-run in flight, result in the next entry.
- _2026-07-04_: **B4.2, third iteration — apply-budget starvation found and
  fixed.** The echo fix brought heavy-roaming back (6/7, runtime normal),
  but the shadow-stall gate kept failing — and instrumentation showed why:
  `prepare` grew 5 ms → 6300 ms across the pans with `backlog=true` on
  EVERY frame. The flat 6 ms apply budget starved the ~600-tile cold-load
  working set (~10 applies/frame), the sim's settle loop broke out on its
  step cap because its `animating` (camera-only `tick_now`) couldn't see
  the backlog, and the gate then measured pans mid-cold-load over a
  half-resident set — where vector prepare's fallback walks + label layout
  cost seconds. Two boundary-honest fixes: (1) the apply budget is now
  TIERED on `Map::is_camera_animating` (new; visual motion only — fades
  deliberately excluded, a fading tile IS an apply arriving): 6 ms while
  the camera moves, 32 ms settled, so cold loads catch up during settles
  exactly like the pre-B4 synchronous path; (2) the sim's per-frame
  `animating` now ORs in `engine.is_animating()` (fades + backlog), so
  settle loops actually settle before gates measure. Fast lanes green
  (52 suites, clippy); full sim suite verification in flight — result in
  the next entry.
- _2026-07-04_: **B4.2 verified: 7/7 sim gates green** (`REQUIRE_GPU=1`,
  Lavapipe, release, 515 s) with the tiered apply budget + backlog-aware
  sim settling. B4.2 took three iterations, each caught by a different
  gate: the delivery echo (heavy-roaming), then apply-budget starvation
  (shadow-stall) — both timing classes that only a device-equivalent
  behavioural harness surfaces. With B4.1+B4.2, nothing decodes or
  tessellates on the render thread on any engine host. Remaining in B4:
  B4.3 — retire the FFI host's now-redundant ingest time-slicing.
- _2026-07-04_: **B4.3 landed — the FFI ingest time-slice is retired.** The
  Android render loop's adaptive 8/6 ms drain budget existed to bound
  decode-on-the-render-thread; with B4.1/B4.2 that decode lives in the
  engine's worker pool, so `render_frame` now drains the ingest channel
  FULLY (each item is an O(µs) hand-off) and pacing has exactly one owner:
  the engine's tiered apply budget, identical on every host. The published
  trace's `backlog` becomes the engine's decode queue depth (new
  `TurbomapEngine::decode_backlog`) — the channel is always empty now.
  Verified here: 52 workspace suites, clippy, wasm, sim gates re-run
  (result in the next entry). `surface.rs` is Android-gated and this
  container has no NDK — the `android_build` CI lane compiles it on push,
  and the plan's standing on-device session gate applies before the
  Kotlin-side reconciler shrink leans on it.
- _2026-07-04_: **B4.3 sim confirmation: 7/7 green** (`REQUIRE_GPU=1`,
  Lavapipe, release, 573 s). **Workstream B4 is complete**: decode and
  tessellation run off the render thread on every engine host, pacing has
  one owner (the engine's motion-tiered apply budget), and the streaming
  trace reports the real backlog. Open on this branch: B3.4 (retire the
  legacy lifecycle sets after the dual-write soak), B6 packaging (the
  coarse Norway `.pmtiles` + `TURBO_BASELINE_BUNDLE` host wiring — needs
  real tile data near the tileserver), and the standing on-device
  validation session before the Kotlin reconciler shrink.
- _2026-07-04_: **B3.4 (first step) landed — residency truth has one owner.**
  The dual-write soak is formally concluded: `lifecycle_agreement` held on
  every sim frame across the entire B4 campaign (5 full release sweeps).
  `Map::is_{raster,vector,terrain}_ingested` — the decode queue's
  re-ingest guards — now answer from the lifecycle table
  (`phase_of ∈ {Resident, Retained}`) instead of the per-scene `ingested`
  sets, which the agreement gate proved equivalent. Verified: 52 workspace
  suites, clippy, 13 engine gpu suites, 7/7 sim gates (457 s release).
  **Continuation (next session):** delete the per-scene `ingested` sets —
  `Scene::{ingest, un_ingest, is_ingested, ingested_len}` and the
  `desired − ingested` filtering move to table-backed views threaded
  through `Map` (scenes keep only the camera-derived desired/LOD walk);
  `tile_histogram` derives from the table; `lifecycle_agreement` retires
  with the sets it compares. Keep the property tests; the capacity proof
  moves to the table's resident universe.
- _2026-07-04_: **B3.4 completed — the legacy per-scene residency sets are
  deleted.** `Scene` no longer tracks residency at all: `ingested` and its
  mutators (`ingest`/`un_ingest`/`is_ingested`/`ingested_len`) are gone;
  `pending_tiles`/`pending_prioritized`/`tile_phases`/`phase_histogram`
  classify against an INJECTED residency predicate — in production the
  lifecycle table via `Map` (`chunk_is_resident`), in tests a plain set.
  `TileHistogram.retained` comes from the table alone (resident-but-
  unwanted is the table's knowledge; scenes only know what they want).
  All ingest sites write residency solely through `lifecycle_delivered`;
  `lifecycle_agreement` is retired everywhere (core, engine passthrough,
  the sim's per-frame assertion, the plan test's checkpoints) together
  with the sets it existed to compare. The scenes are now purely
  camera-derived want-generators — the S6 LOD policy role the
  architecture assigns them. Scene property tests (determinism, tier
  ordering, monotonic convergence, bounded working set) survive with
  set-backed predicates; the evicted-tile-re-pends contract is covered at
  the table level (turbomap-world property tests) and behaviourally (the
  sim's heavy-roaming gate). Fast lanes green (52 suites, clippy, wasm);
  engine gpu + full sim verification in flight — verdict in the next
  entry.
- _2026-07-04_: **B3.4 verified: 7/7 sim gates green** (`REQUIRE_GPU=1`,
  Lavapipe, release, 460 s) on the sets-deleted tree; two stale
  `lifecycle_agreement` call sites in the gpu-gated plan test cleaned up
  (13/13 engine gpu suites green after). **Workstream B is complete.**
- _2026-07-04_: **C1 landed — the environment enters the Scene IR**
  (Phase 2 begins). `EnvironmentDef` (lighting mode Default/TimeTracked/
  Fixed, terrain-shadow strength, sun-lit shading, aerial haze, basemap
  gain — defaults engine-neutral so every pre-C1 document stays valid) +
  `SourceDef::Field2D { bounds }` (geo-anchored data grids; chains reject
  them). `SceneDelta.environment: Option<EnvironmentDef>` — an environment
  edit is an environment-only delta; `TurbomapEngine::apply` drives the
  same core setters the imperative side-doors call. Conformance grows two
  clauses every engine must satisfy: `check_environment_diffing` (edit →
  env-only delta, reapply → no-op) and `check_field_source_update`.
  Fast lanes green (52 suites, clippy, wasm); engine gpu suites 13/13;
  sim verification in flight — verdict next. Remaining in C: C2 (the
  imperative setters become shims; clouds/radar ride Field2D ingest),
  C3 (compositing honesty + truthful `Capabilities`).
- _2026-07-04_: **C1 verified: 7/7 sim gates green** (`REQUIRE_GPU=1`,
  Lavapipe, release, 458 s) on the environment-in-the-IR tree.
- _2026-07-04_: **C2b landed — the weather-cloud overlay is scene-declared.**
  `EnvironmentDef.clouds: Option<CloudsDef { source, grid, visible }>`: the
  overlay names its `Field2D` source (whose `bounds` anchor it
  geographically), its radar grid resolution, and visibility; `validate()`
  rejects a clouds block pointing at a missing or non-field source. The
  engine applies the block from the environment delta — enable + geo
  bounds + visibility on declare, teardown on removal — and
  `ingest_field(source, …)` is the source-addressed data push (the field
  twin of tile ingest): frames for a source the scene doesn't consume are
  dropped with a warning, because data is transport and the SCENE decides
  what renders. Playback (`set_cloud_time`) stays a control-plane verb,
  like the camera. `ingest_radar_frame`/`enable_clouds` remain as
  documented transitional side-doors for pre-C2 hosts. **C2a (demoting the
  sun/shadow/haze/gain setters to scene-syncing shims) is deliberately
  deferred:** it changes host-visible semantics — a scene reapply would
  revert imperatively-set sun mode — so it must ride with the Kotlin/web
  hosts moving to scene-declared environment authoring, behind the
  standing on-device gate. Fast lanes green (52 suites, clippy, wasm);
  gpu ladder result in the next entry.
- _2026-07-04_: **C3 landed — the contract stops lying.**
  `TurbomapEngine.capabilities().data_driven_paint` is now `true` (`Match`
  paints have compiled to per-feature rules since the expression work —
  the flag simply lied); `ModelEngine.custom_layers` drops to `false`
  (no engine renders custom layers until plan D4, and hosts read these
  flags to degrade — honesty over aspiration). The circle/marker
  compositing exception is now DOCUMENTED on `Layer::Circle` and the
  `Capabilities` fields carry their precise meaning: circles render in the
  overlay track above positional layers; full interleaving arrives with
  the frame graph (D1/D2). With C1 + C2b + C3, Phase 2's IR-honesty goals
  are met except C2a (setter demotion), which waits on host
  scene-authoring migration behind the device gate. **Next: D1 (frame
  graph) in a fresh session.**
- _2026-07-04_: **C2b verified: 13 engine gpu suites + 7/7 sim gates green**
  (`REQUIRE_GPU=1`, Lavapipe, release, 569 s). Phase 2 closes at its
  in-container scope: C1 + C2b + C3 done; C2a parked behind host
  scene-authoring migration and the standing device gate. Phase 3 (D1
  frame graph) opens next, in a fresh session.
- _2026-07-04_: **D1 landed — the frame is a graph.**
  `turbomap-core/src/render/graph.rs` (new): `PassDesc { name, phase,
  reads, writes }` over a coarse resource set (`HeightField`, `AoField`,
  `ShadowUniforms`, `ColorMsaa`, `Depth`, `FrameTarget`),
  `FramePhase::{BeforeFrame, GroundMsaa, OverlayMsaa, Composite}` (no
  `Post` — the HDR post stage was already removed on purpose), a
  unit-tested scheduler (phase-ordered, stable painter's order within a
  phase) and data-flow validation (every non-persistent read needs a
  producer this frame; the heightfield/AO fields are persistent —
  streaming semantics, validated in debug on every rendered frame).
  `Map::render` registers passes instead of hand-sequencing: shadow
  heightfield + AO in `BeforeFrame`; sky/floor/one node **per tile
  layer** (`layer:<id>`)/route tubes in `GroundMsaa`; icons/text/markers
  in `OverlayMsaa`; clouds in `Composite`. The single-MSAA-pass rule is
  now structural: `FrameGraph::run_msaa` is the only place the frame
  pass begins, and contributions get a `&mut RenderPass`, not the
  encoder. Where the graph is BETTER than the old sequence (ground rule
  2): per-pass CPU timings always-on in `FrameMetrics::passes`
  (+ `passes.csv`/`passes_json` surfaces), per-scope GPU timestamps
  (`ao`/`frame-pass`/`clouds`, `FrameMetrics::gpu_passes`), runtime pass
  isolation (`Map::set_pass_enabled`, harness `TURBO_DISABLE_PASSES` +
  `TURBO_PASS_ISOLATE` A/B image dumps), and `draw_calls` counting the
  floor/route nodes the old hand count missed.
  **Verification (visual, real data):** the Sjunkhatten scenario harness
  (real Kartverket topo + real kart-api DEM, 51 scripted steps) rendered
  before/after: frames byte-identical (max pixel diff 0 across the
  session), per-frame CPU no worse. Two pre-existing baseline findings
  recorded (not D1 regressions, now diagnosable with the isolation
  tool): the cloud overlay washes out tilted frames, and the harness's
  built-in cast-shadow proof fails at the default pose on unmodified
  HEAD. Full ladder (unit + gpu suites + sim) in the next entry.
