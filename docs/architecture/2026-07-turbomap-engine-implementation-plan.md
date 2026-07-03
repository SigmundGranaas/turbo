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
2. **Parity-first refactors.** Any slice that restructures rendering or data
   flow must first prove pixel/behaviour equivalence (goldens compare-mode,
   conformance, sim gates) before adding new behaviour. This playbook already
   worked twice (single-pass refactor, wgpu 22→29).
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
- **Gate:** every golden passes compare-mode (references untouched); sim
  budgets hold; `stats_json` shows per-pass ms.

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
- **Gate:** goldens unchanged; one write site for every environmental uniform
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
| Golden churn from D3's R16F height switch | Perceptual-tolerance compare first; regenerate only reviewed, intentional diffs (established golden discipline). |

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
