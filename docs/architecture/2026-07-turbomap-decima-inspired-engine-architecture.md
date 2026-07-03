# Turbomap Engine Architecture — a Decima-inspired, subsystem-formalized design

**Status:** design / direction-setting · **Date:** 2026-07-03
**Scope:** `apps/turbomap` — the wgpu renderer core, the Scene/engine layer, the
tile/streaming stack, and every host (desktop, FFI/Android, web).
**Builds on:** `2026-06-map-engine-architecture.md` (the MapEngine contract —
kept), `2026-06-turbomap-tile-pipeline-plan.md` (absorbed: its Slices 1–4 become
Phase 1 here), `2026-06-terrain-lod-horizon-shadows.md`,
`2026-06-tile-lod-retention-and-crossfade.md`, `2026-06-tile-data-architecture.md`.

**Thesis:** turbomap has strong *pieces* — a clean contract, a real Scene IR,
best-in-class headless testing — but no *architecture between the pieces*. The
frame is a hand-written procedure, subsystems grow as ad-hoc methods that bypass
the IR, streaming policy is re-implemented per host, and environmental state is
smeared through the raster uniforms. This document formalizes the engine the way
Guerrilla's **Decima** engine is formalized: a small number of named subsystems
with explicit contracts, one streaming system with priorities and budgets, one
environment model every system samples, and observability as a requirement of
being a subsystem — so each part can be evaluated, debugged, and extended alone.

---

## Part I — Where we actually are

This section is grounded in a full read of the ~45k-line workspace (all file
references verified 2026-07-03). It matters because the fix must preserve what
is genuinely good.

### I.1 What is already strong (keep, and build on)

| Asset | Where | Why it matters |
| --- | --- | --- |
| **`MapEngine` contract + Scene IR + conformance suite** | `turbomap-scene` (`engine.rs`, `scene.rs`, `diff.rs`, `conformance.rs`) | Renderer-agnostic control plane; pure LCS diff; 9 named guarantees run against both `ModelEngine` and `TurbomapEngine`. This is our "one object model" seed. |
| **Pull/push tile IO** | `turbomap-core/src/source.rs`, `Map::pending_tiles` (`map.rs:1870`) | Core never blocks on IO; host owns transport. Matches wgpu threading. Decima separates streaming policy from IO the same way. |
| **Global tile priority sort (tier, distance)** | `map.rs:1904`, `scene.rs:427` (`TileTier::Overview→Visible→Prefetch`) | The germ of a real streaming priority system — it exists in core but stops at the FFI boundary. |
| **SSE quadtree LOD + capacity governor** | `lod.rs::select`, `capacity.rs` (compile-time `desired ≤ cache` proof) | Screen-space-error-driven mixed-zoom selection with bounded working set — this *is* the Decima LOD-ring idea for a map; it's just not yet the engine-wide LOD vocabulary. |
| **Single MSAA pass + prepare/draw split** | `map.rs:2417` (Decision 1 of the wgpu audit, executed) | Mobile-correct frame structure to hang a pass formalization on. |
| **Tessellate-once, GPU-evaluated paint** | `tessellate.rs`, `vector.rs` (width/dash/color via per-tile dynamic-offset uniforms) | Zoom curves animate with zero re-tessellation. |
| **Headless verification culture** | `turbomap-golden`, `turbomap-sim` (blank-map gates, convergence, cache budgets, ANR-stall gate), `inspect` | Nobody else in the map space has this. Every proposal below is expressed as something this harness can gate. |
| **Bundled-data reader** | `turbomap-tiles-pmtiles` (v3, file/bytes/HTTP-range backends, byte-identical across backends) | The offline-baseline capability already exists as a library — it is just not wired to anything. |
| **Android host threading** | `turbomap-ffi/src/surface.rs` (render thread + wait-free `Cmd` queue + snapshot reads + time-budgeted ingest) | The most disciplined concurrency in the codebase; the model to generalize, not replace. |

### I.2 The structural problems (why it "has no clear architecture")

These are six faces of one root cause: **capabilities were added faster than
concepts were named.** Each new feature found a seam that worked and grew there,
so the system's behaviour is the sum of local decisions, not the consequence of
stated rules.

**P1 — The frame is a procedure, not a composition.**
`Map::render` (`map.rs:2417–2884`) is a ~460-line straight-line function:
finite gate → `RenderFrame::build` → shadow heightfield → AO accumulate →
prepare loop → clear-color choice → one MSAA pass with a *hardcoded* draw
sequence (sky → floor → layers → route tubes → icons → text → markers) → 4 post
passes → a bespoke cloud pass that couldn't join the MSAA pass. Ordering
dependencies (shadows before AO before frame; text-prepare before icon-prepare)
exist only as call order. Pipelines have two ownership models (per-`LayerEntry`
vs shared `Renderer`), forcing split-borrow gymnastics. Adding any new visual
subsystem means editing this function and inventing its own state slot on `Map`.

**P2 — Subsystems bypass the Scene IR.**
The IR promises "one immutable value describes the map," but terrain sun/shadow
state, aerial haze, basemap gain, route tubes, and the **entire cloud/radar
subsystem** are imperative inherent methods on `TurbomapEngine`
(`engine.rs:516–613`: `enable_clouds`, `ingest_radar_frame`, `set_cloud_time`,
`set_sun_time`, `set_terrain_shadows`, …). They are not in the Scene, not
diffed, not conformance-tested, invisible to `ModelEngine` and any future
adapter. `Layer::Custom` — the declared extensibility hook — is a dead letter:
`ModelEngine` advertises it, the real engine drops it and reports
`custom_layers: false`. Meanwhile `Circle` layers silently escape the "index =
draw order" promise by becoming markers in a separate pass. **The precedent is
set: the next subsystem will also bypass the IR, because that is the path of
least resistance.**

**P3 — A tile has no lifecycle, and streaming policy is re-implemented per host.**
Tile state is smeared across ~6 collections in 3 layers (`Scene.ingested`,
FFI `queued`, host `inFlight`/`retryAt`, GPU cache residency, fade `first_seen`).
Core computes a good priority order (`pending_tiles`, tier+distance), but the
list crosses the boundary as an **unordered request dump** and each host
re-invents policy: desktop has rayon pumps + per-layer inflight caps + 8s
backoff (`map_host.rs`); Android has a Kotlin reconciler + adaptive ingest
budget; web has *nothing* (JS fetches whatever, whenever). There is **no
cancellation anywhere** — a fast pan leaves stale fetches decoding to
completion. FFI/web hosts decode PNG/MVT **on the render/main thread**
(mitigated by time-slicing, which is a symptom). Every flicker/race bug of the
last month lived in this hand-kept bookkeeping.

**P4 — The offline/bundled-baseline goal has no seam.**
`turbomap-tiles-pmtiles` is complete and tested, and used by nothing. There is
no `SourceDef` variant for a bundled archive, no resolver path, no provider
chain. Separately there are **two divergent disk caches** (`tiles-http::DiskCache`
bounded LRU — wired for vector only; `tiles-cache::DiskCachedSource` unbounded —
wired to nothing), and raster HTTP has **no cache at all**.

**P5 — Environmental state is entangled, not modeled.**
Sun, haze, shadow, AO, sky, and cloud parameters are folded into the raster
pipeline's `Globals`/`TerrainConfig` uniform blocks and patched from multiple
places (`frame.rs:195–209` documents its own fields being "patched in
Map::render"). There is no single environment value that subsystems sample —
which is exactly what blocks coherent environmental simulation (weather that
drives clouds *and* water *and* snow *and* lighting together).

**P6 — LOD is three local mechanisms, not one engine concept.**
SSE quadtree for pitched ground tiles, a separate coarser DEM scene
("proto-clipmap", `map.rs:1048–1075`), zoom bands in styles — each fine, but
no shared vocabulary of "detail level as a function of screen-space error and
budget" that a new subsystem (vegetation, water, 3D buildings) can plug into.

---

## Part II — Decima: the core architecture and its pillars

Guerrilla Games' **Decima** engine (Killzone: Shadow Fall → Horizon Zero Dawn →
Death Stranding → Horizon Forbidden West) is the reference because its central
problem is ours: **stream a huge, richly layered outdoor world through a small
memory window, with graceful degradation, on constrained hardware** — and stay
debuggable while dozens of subsystems composite into one coherent image. Its
publicly documented architecture (GDC/SIGGRAPH talks and publications on
guerrilla-games.com — *Streaming the World of Horizon Zero Dawn*, *GPU-Based
Procedural Placement in Horizon Zero Dawn* (GDC 2017), *Decima Engine:
Visibility in Horizon Zero Dawn* (2017), *The Real-Time Volumetric Cloudscapes
of Horizon Zero Dawn* (SIGGRAPH 2015) and the Nubis follow-ups, *Decima Engine:
Advances in Lighting and AA* (SIGGRAPH 2017), *The Technology of Horizon
Forbidden West* (GDC 2022)) rests on six pillars.

### D1 — One typed object model for everything

Every asset — mesh, texture, entity, placement rule, quest — is a typed,
reflected object serialized into object-graph files (the `.core` format) with
GUID references. Consequences: **one** loader, **one** dependency resolver,
**one** budget accountant for every content class; reflection gives free
tooling (inspection, diffing, serialization); references are explicit, so
dependency closure and prefetch are *computable*, not guessed. The editor and
the game share the same object model — there is no "export" gap.

### D2 — Streaming *is* the world model, not a feature

There is no "loaded level." The world is a grid of streamable sectors holding
LOD-graded content, and a **single streaming manager** serves every consumer:

- It tracks *interest points* (player, camera, velocity, scripted hints) and
  derives concentric **LOD rings** — full detail near, progressively coarser
  representations farther out.
- Loads are scheduled by **priority = f(distance, direction of travel, LOD
  delta, content class)** under **hard memory and IO budgets**.
- The invariant that makes it feel seamless: **coarse before fine, always.**
  A region is never blank; fine content arrives as *refinement* of something
  already visible. Eviction is the same priority function, inverted.
- Everything is async; nothing on the critical frame path ever waits for IO.

### D3 — Generate, don't store

Horizon's vegetation, rocks, and debris are not placed in world data. Artists
author compact 2D **density/ecotope layers**; a GPU compute pass expands them
*deterministically* into instance placements in rings around the camera as it
moves (GDC 2017). Because placement is a pure function of (world layers,
position, seed), it is never serialized, regenerates identically on revisit,
and costs milliseconds. The world description stays small; **detail is a
function of data, not data itself.**

### D4 — One environmental model that every system samples

Time-of-day, sun, sky, volumetric clouds (Nubis), wind, and weather form one
shared environment state. Terrain lighting, cloud shadows on the ground, water
response, vegetation sway all **sample the same state** rather than owning
private copies. This single-source-of-truth is why the world reads as coherent:
when a storm rolls in, everything responds together.

### D5 — Unified visibility and GPU-driven work

One system answers "what is potentially visible?" for all clients — renderer,
shadow cascades, streaming hints — across hundreds of thousands of objects
(the 2017 visibility talk). Work migrates to the GPU (culling, placement,
terrain) so the CPU *orchestrates* rather than enumerates.

### D6 — Budgets and observability are first-class

Every subsystem runs under explicit budgets (memory pools, IO bandwidth,
frame-time slices) and exposes profiling and debug views. Decima is debuggable
subsystem-by-subsystem **because subsystems have names, owners, budgets, and
inspection surfaces** — the property the user is asking for by name.

### What we deliberately do *not* copy

Decima is a game engine: entity/component gameplay object model, physics, AI,
baked-content pipelines with a bespoke editor. None of that maps here. We adopt
the *architectural discipline* — the six pillars — not the game-engine shape.
Our "world data" is live cartographic data (tiles, DEMs, weather feeds), which
actually makes D2/D3 *easier*: our world is already a pyramid-addressed,
streamable dataset by nature.

---

## Part III — The target architecture

Decima's pillars, translated onto turbomap. The design formalizes **seven named
subsystems** behind **four cross-cutting contracts**. Everything below is
expressed so it can land incrementally on the existing code — nothing is a
rewrite.

### III.0 The one-picture view

```
                        HOST (winit / Android / iOS / web / Flutter)
                        owns: surface, transport IO, gestures, UI
   ───────────────────────── MapEngine contract (kept) ─────────────────────────
        apply(Scene) · camera · project/hit_test · StreamingPlan pull/push
   ┌────────────────────────────────────────────────────────────────────────┐
   │                            TURBOMAP ENGINE                             │
   │                                                                        │
   │  Scene IR (extended: environment, ordered compositing, custom slots)   │
   │       │ diff/reconcile                                                 │
   │  ┌────▼─────────┐   samples   ┌──────────────────────────────┐         │
   │  │  SUBSYSTEMS  │◄───────────►│  S4 ENVIRONMENT  (time, sun, │         │
   │  │  (registry)  │             │  weather fields, wind, season)│        │
   │  └────┬─────────┘             └──────────────▲───────────────┘         │
   │       │ declare needs                        │ ticked by               │
   │  ┌────▼──────────────────────┐   ┌───────────┴───────────────┐         │
   │  │ S1 WORLD DATA LAYERS      │   │ S5 SIMULATION SYSTEMS      │        │
   │  │ (typed catalog: basemap,  │   │ (fixed-tick: weather adv., │        │
   │  │  DEM, radar, landcover…)  │   │  water state, cloud anim)  │        │
   │  └────┬──────────────────────┘   └───────────────────────────┘         │
   │  ┌────▼──────────────────────────────────────────────────────┐         │
   │  │ S2 STREAMING SYSTEM (one): lifecycle, priority, budgets,  │         │
   │  │ cancellation, provider chain (mem→disk→bundled→remote)    │         │
   │  └────┬──────────────────────────────────────────────────────┘         │
   │  ┌────▼──────────────────────────────────────────────────────┐         │
   │  │ S3 FRAME GRAPH: named phases, declared pass reads/writes, │         │
   │  │ per-pass timestamps, debug views                          │         │
   │  └───────────────────────────────────────────────────────────┘         │
   │   cross-cutting: S6 LOD policy · S7 Observability (inspect/gates)      │
   └────────────────────────────────────────────────────────────────────────┘
```

### III.1 — S1: World Data Layers (Decima D1 applied to data)

**Concept.** Everything the engine consumes is a **`WorldLayer`**: a typed,
pyramid-addressed dataset. Basemap vectors (MVT), raster imagery, DEM, radar
frames, water masks, landcover — one vocabulary, regardless of transport.

```rust
/// The typed catalog entry — the map's ".core object".
struct WorldLayerDef {
    id: WorldLayerId,                    // stable, host-visible
    kind: LayerDataKind,                 // VectorMvt | RasterRgba | DemHeight | Field2D { … }
    pyramid: PyramidSpec,                // zoom range, tile size, halo, extent
    providers: Vec<ProviderRef>,         // ordered: first hit wins (III.1.b)
    residency: ResidencyClass,           // e.g. Ground | Detail | Simulation
}
```

**Why.** Today "sources" exist per Scene layer with the DEM special-cased as a
`Map`-level singleton, radar frames arriving through a bespoke ingest call, and
the same `(z,x,y)` addressing re-derived in four places. Making `WorldLayer`
the *only* way data enters the engine gives us Decima's D1 payoffs: one loader
path, one residency/budget accountant, one addressing scheme, computable
prefetch — and every future dataset (snow cover, avalanche zones, sea state)
arrives without new plumbing.

**b) The provider chain — remote streaming + bundled baseline, one mechanism.**
Each `WorldLayer` resolves tiles through an *ordered chain*:

```
MemoryCache → DiskCache (one impl, bounded LRU) → BundledArchive (PMTiles file)
            → Remote (HTTP XYZ | PMTiles range | host-provided)
```

- This is where the orphaned `turbomap-tiles-pmtiles` reader finally plugs in:
  a bundled Norway-baseline `.pmtiles` (basemap + coarse DEM, e.g. z0–z10)
  ships in the app and terminates the chain offline. Remote range/XYZ refines
  above it. **A cold start with no network shows a complete (coarse) map.**
- The two disk caches merge into one bounded implementation
  (`tiles-http::DiskCache` semantics win: byte-budget LRU, atomic writes);
  `tiles-cache::DiskCachedSource` (unbounded) is deleted. Raster gets cached.
- `SourceDef` gains the missing variants: `PmtilesBundle { path }`,
  `PmtilesRemote { url }` — so bundled data is *declarative in the Scene*,
  diffable and conformance-testable like everything else.
- Add brotli support to the PMTiles reader (real planet archives use it).

**c) Baseline datasets policy.** Bundle: coarse basemap pyramid, coarse DEM,
font faces, sprite sheet. Stream: everything at detail zooms, radar/weather
(inherently live), satellite. The chain makes this a packaging decision, not
an architecture decision — exactly the property we want.

### III.2 — S2: The Streaming System (Decima D2 — the heart of this design)

One subsystem owns **all** movement of data toward the GPU. It absorbs the
tile-pipeline plan's Slices 2–4 and generalizes them beyond basemap tiles.

**a) The lifecycle is one type** (single source of truth, replacing ~6 sets):

```rust
enum ResourcePhase {
    Desired { tier: Tier, priority: Priority },
    Fetching { started: Instant, cancel: CancelToken },
    Decoding,
    Resident { last_used_frame: u64, bytes: u64 },
    Retained,          // resident but no longer desired → eviction candidate
}
```

Host `inFlight`/`retryAt` and FFI `queued` become *views* of this table, not
parallel truths. Illegal transitions are unrepresentable; property tests assert
determinism (same camera ⇒ same desired set), monotonic LOD (never regress a
cell to coarser while finer is resident), and boundedness (`desired ≤ cache`,
extending the `capacity.rs` compile-time proof).

**b) Priority is a computed, explainable score** — not a sort key smeared
across layers:

```rust
struct Priority(u32);  // ordered; decomposable for inspection
// f(tier          — Overview ≺ VisibleNear ≺ VisibleFar ≺ DemForVisible ≺ Prefetch,
//   sse_benefit   — screen-space-error reduction if this tile lands (S6),
//   motion        — dot(camera_velocity, dir_to_tile): prefetch WHERE WE'RE HEADING,
//   layer_class   — ground data before decoration)
```

The Decima invariant, stated as a rule the sim can gate: **coarse before fine,
visible before prefetch, nothing visible is ever blank if any ancestor is
resident.** (The retention/crossfade plan's "best-resident-per-region" model is
the render-side half of this rule.)

**c) Budgets are explicit and engine-owned:**

```rust
struct StreamingBudgets {
    max_inflight: PerClass<u32>,        // e.g. vector 16, raster 12, dem 6
    decode_ms_per_frame: f32,           // generalizes Android's adaptive 6/8ms
    upload_bytes_per_frame: u64,
    vram: PerPool<u64>,                 // ground textures, meshes, dem, fields
}
```

**d) The host boundary becomes a *plan*, not a dump.** `pending_tiles()`
(unordered `Vec`) is replaced by:

```rust
struct StreamingPlan {
    start:  Vec<FetchRequest>,   // priority-ordered, budget-truncated
    cancel: Vec<RequestId>,      // stale: camera moved on ← THE missing verb
}
fn streaming_plan(&mut self) -> StreamingPlan;
fn ingest(&mut self, id: RequestId, bytes: Bytes);   // decode is engine-side, off-thread
```

The host stays the transport (OkHttp/URLSession/fetch — with its auth,
certificates, and network stack), but **policy** (what, when, in what order,
what to abandon) lives in one place, identical on every platform. Web stops
being the wild west; Kotlin's `TileReconcilePlan` shrinks to a transport shim.

**e) Decode moves off the render thread everywhere.** Desktop already decodes
in rayon pumps; FFI/web decode PNG/MVT on the render/main thread today. The
streaming system gains an internal worker pool (plain threads; wasm: chunked
budget as today, workers when threading lands) so `ingest` only ever *enqueues
bytes*, and the render thread only uploads within `upload_bytes_per_frame`.
This keeps the engine "gracefully async" without an async runtime: **the
render thread never waits on anything**, which is the property that matters.

### III.3 — S3: The Frame Graph (fixing P1)

Formalize what `Map::render` does implicitly. Not a full Frostbite-style
transient-resource allocator — a **declared composition** sized to our frame:

```rust
struct PassDesc {
    name: &'static str,                  // "shadow-heightfield", "ao", "ground", …
    phase: FramePhase,                   // BeforeFrame | GroundMsaa | OverlayMsaa | Post | Composite
    reads: &'static [ResourceTag],       // DemField, ShadowField, AoField, HdrColor…
    writes: &'static [ResourceTag],
}
trait RenderContribution {
    fn prepare(&mut self, frame: &FrameCtx) -> Prepared;    // CPU/upload, budgeted
    fn draw<'p>(&'p self, pass: &mut wgpu::RenderPass<'p>, prepared: &Prepared);
}
```

- The scheduler orders passes by declared reads/writes — the shadows→AO→frame
  and text→icon orderings become *derived from data*, not call sequence.
- The single-MSAA-pass discipline is preserved: `GroundMsaa`/`OverlayMsaa` are
  *phases inside the one pass* (pipeline switches), exactly as today; `Post`
  and `Composite` (clouds) are their own passes as today — but now a new
  subsystem *declares* "I draw in GroundMsaa after terrain" instead of editing
  a 460-line function.
- Per-pass GPU timestamps and a **debug view registry** (render any single
  pass/resource to screen — the clouds crate's `DebugView` pattern, made
  engine-wide) come for free at this seam.
- The two pipeline-ownership models collapse: all pipelines are owned by their
  subsystem; `Renderer` keeps only truly shared resources (targets, DEM share,
  atlases).

This is also what makes `Layer::Custom` real: a custom layer is exactly a
host-registered `RenderContribution` bound to a declared phase — write-once
Rust+WGSL, portable across platforms, as the original architecture doc promised.

### III.4 — S4: The Environment (Decima D4) + S5: Simulation Systems

**One value, sampled by everyone:**

```rust
struct Environment {
    time_utc: f64, sun: SunPosition, atmosphere: Atmosphere,   // exists (sun.rs)
    wind: Vec2, season: f32,
    fields: FieldSet,   // GPU-resident 2D fields addressed like WorldLayers:
                        // precipitation (radar), cloud coverage, snow line,
                        // sea state, temperature — each a Field2D WorldLayer
}
```

- **Scene-declared:** the IR gains an `environment` block (mode: fixed /
  time-tracked / host-driven, plus per-field source references). It is diffed,
  conformance-checked, and visible to `ModelEngine` — closing P2 for good. The
  imperative methods (`set_sun_time`, `enable_clouds`, `ingest_radar_frame`, …)
  become a thin compatibility shim over scene application, then deprecate.
- **Consumers sample, never own:** sky, clouds, terrain lighting, cast
  shadows, haze, hillshade, water, future snow/vegetation all read
  `Environment` through the frame context. The raster `Globals` patching
  scattered across `frame.rs`/`map.rs` collapses into "build `Environment`,
  then each pass derives its uniforms from it."
- **S5 Simulation Systems** are the writers: fixed-tick (`tick(dt)`,
  decoupled from render rate) systems that advance the fields — radar-frame
  interpolation/advection today; wind-driven cloud drift (fixes the
  "cloud animation isn't in `is_animating`" wart *by construction* — sim
  activity is animation); later water state and snow accumulation. Determinism
  rule (Decima D3): every simulation is a **pure function of (environment
  inputs, world layers, time, seed)** — replayable in the sim harness, golden-
  testable at a fixed time.

**Generate-don't-store, applied.** Environmental detail derives from data we
already stream, not from new data: snow cover = f(DEM elevation/slope, season,
weather); vegetation density = f(landcover class, elevation); sea state =
f(wind field, fetch). Compact inputs, deterministic GPU expansion — the D3
recipe, and the reason this foundation scales to "environmental simulation"
without inventing a content pipeline.

### III.5 — S6: One LOD vocabulary

Generalize the two selectors into a single policy every subsystem speaks:

```rust
trait LodPolicy {
    /// Screen-space error of representation `level` for `cell` under `camera`.
    fn sse(&self, cell: Cell, level: Lod, camera: &Camera) -> f32;
    fn target_sse(&self) -> f32;      // per-subsystem knob (basemap 256px, DEM 480px…)
}
```

- Basemap keeps `lod.rs::select`; DEM keeps its coarser target — but both are
  now *instances of one concept* with per-subsystem targets and caps, and the
  streaming priority's `sse_benefit` term (III.2.b) reads the same numbers.
- The retention rule (from the crossfade plan) is stated once, engine-wide:
  **render the best resident representation per cell; replace only when a
  strictly better one is resident and faded in; never regress.**
- New subsystems inherit LOD by implementing `sse()` — water gets wave-detail
  rings, vegetation gets density rings, exactly like Decima's LOD rings.

### III.6 — S7: The Subsystem contract (observability as an obligation)

The formalization the user asked for, stated as the one trait everything must
implement — being debuggable is not optional equipment:

```rust
trait Subsystem {
    fn name(&self) -> &'static str;
    /// Declarative config in, delta out — Scene is the only config channel.
    fn reconcile(&mut self, scene_slice: &SceneSlice) -> SubsystemDelta;
    fn data_needs(&self) -> &[WorldLayerId];              // feeds S2
    fn tick(&mut self, dt: Duration, env: &Environment);  // sim/animation; may be no-op
    fn passes(&self) -> &[PassDesc];                      // feeds S3
    fn budgets(&self) -> BudgetReport;                    // bytes, ms, counts vs caps
    fn inspect(&self) -> serde_json::Value;               // extends the inspect tool
    fn debug_views(&self) -> &[DebugViewDesc];            // isolate any stage on screen
}
```

The engine becomes: a **registry of subsystems** (Basemap, Terrain, Symbols,
Overlays, Atmosphere, Water, …) + the four shared services (WorldData,
Streaming, FrameGraph, Environment). `Map`'s god-struct fields migrate into
subsystems; `Map` retains camera, registry, and frame orchestration. Each
subsystem is then evaluable alone, by construction:

| Evaluation axis | Mechanism (all exist today, become *required*) |
| --- | --- |
| Correctness | golden images per subsystem (`turbomap-golden`) |
| Behaviour under latency/motion | `turbomap-sim` gates (blank-map, convergence, budgets) |
| Perf | per-pass GPU timestamps + `BudgetReport` in `stats_json` |
| State inspection | `inspect` JSON section per subsystem |
| Visual isolation | `debug_views` (the clouds `DebugView` pattern, universal) |
| Streaming health | per-layer lifecycle histograms from S2's one table |

---

## Part IV — What this buys us (pillar → payoff traceability)

| Decima pillar | Turbomap application | Concrete payoff |
| --- | --- | --- |
| D1 one object model | S1 WorldLayers + Scene IR as the *only* config/data channels | New datasets & features without new plumbing; adapters and `ModelEngine` stay truthful; P2/P4 closed |
| D2 streaming is the world | S2 lifecycle/priority/budgets/cancellation + provider chain | Same loading behaviour on all platforms; no stale fetches; offline baseline; P3/P4 closed |
| D3 generate, don't store | S5 deterministic sims over streamed layers | Environmental richness with near-zero data cost; replayable in CI |
| D4 one environment | S4 `Environment` + fields | Coherent world (storm dims light *and* animates clouds *and* roughens water); P5 closed |
| D5 unified visibility/GPU-driven | S6 one LOD/SSE vocabulary feeding S2 priorities | Bounded working sets at any pitch; every subsystem LODs the same way; P6 closed |
| D6 budgets & observability | S7 subsystem contract + S3 per-pass timing | "Debug and evaluate one by one" is structural, not aspirational; P1 closed |

---

## Part V — Migration: phased, gated, no big-bang

Ordering principle: **name the concepts first in the code that exists** (types
over behaviour), prove each phase with the harness, never break the
`MapEngine` contract (extend it; conformance suite grows with it).

### Phase 0 — Instrument (≈ tile-pipeline plan Slice 1; do first, no behaviour change)
Structured per-frame trace in `stats_json`: per-state tile histograms,
per-stage ms, evictions/backlog, frame gaps. Extend `scenario.rs` + device
capture to one schema. **Gate:** baselines recorded; every later phase must
move these numbers or not regress them.

### Phase 1 — Streaming System (S2 + the S1 provider chain) — *highest value/risk ratio*
1. `ResourcePhase` table replaces the scattered sets (Slice 2), property tests
   for the three invariants.
2. Priority tiers + explainable score (Slice 3); `StreamingPlan` with
   `cancel[]` replaces `pending_tiles` across FFI/web (keep a deprecation shim).
3. Budgets + capacity governor generalization (Slice 4); engine-side decode
   workers; upload budget on the render thread.
4. Provider chain: unify the disk caches (delete the unbounded one), cache
   raster, wire PMTiles (`SourceDef::PmtilesBundle/Remote`), brotli, ship a
   small bundled baseline extract behind a fixture first.
**Gates:** sim cold-load shows visible-before-prefetch strictly; time-to-first-
full-viewport improves vs Phase 0 baseline; offline-cold-start sim renders a
complete coarse map with zero network; zero illegal lifecycle transitions over
the journey sweep; stale-fetch waste (cancelled/started) visible in trace.

### Phase 2 — Scene IR absorbs the out-of-band subsystems (S1/S4 surface)
`environment` block + `Field2D` sources in the IR; clouds/sun/shadows/haze/
route-tubes become scene-declared (imperative methods → shims); fix compositing
honesty (circles rejoin the ordered stack or the contract documents tracks);
`Capabilities` reports truth. Conformance suite grows checks: environment
diffing, field-source updates, cross-track ordering.
**Gates:** conformance green on both engines; `inspect` shows the whole engine
state from the Scene alone; goldens unchanged (pure refactor of config flow).

### Phase 3 — Frame Graph (S3) + subsystem registry (S7)
Introduce `PassDesc`/`RenderContribution`; port existing passes 1:1 (goldens
prove pixel-equivalence, exactly like the single-pass refactor); collapse
pipeline ownership; per-pass timestamps + debug-view registry; migrate `Map`
fields into registered subsystems. Make `Layer::Custom` real via phase-bound
contributions.
**Gates:** all goldens byte/perceptually stable; a demo custom layer runs on
desktop+web from one Rust impl; pass-level timings appear in `stats_json`.

### Phase 4 — Environment + first simulation consumers (S4/S5)
`Environment` value + `FieldSet`; sky/haze/shadows/hillshade sample it;
clouds tick as the first `SimulationSystem` (radar advection; drift =
animation, fixing the redraw wart); deterministic-replay test pattern
established.
**Gates:** golden at fixed (time, seed, fields) for each sim; storm scenario
in sim shows coherent multi-system response; frame budgets hold with sim on.

### Phase 5 — Environmental expansion (the payoff round)
Water (the AAA-water doc's spectral ocean plugs in as a subsystem: an FFT sim
writing a field, a ground contribution sampling `Environment`), snow line,
vegetation density — each a new `Subsystem` + `WorldLayer`s, no core edits.
This phase existing *without touching Parts I–III* is the success criterion of
the whole architecture.

---

## Part VI — Invariants (the spec, to be enforced by tests)

1. **Never blank:** if any ancestor of a visible cell is resident, something
   draws there. (sim gate, exists — keeps its teeth through every phase)
2. **Coarse before fine; visible before prefetch.** (Phase-1 property test + sim)
3. **No LOD regression** while a finer representation is resident. (property test)
4. **Bounded:** desired working set ≤ cache capacity, per pool, by construction.
5. **Deterministic selection:** same camera + scene ⇒ same desired set & plan.
6. **Render thread never waits** on IO, decode, or lock acquisition beyond a
   bounded upload/ingest budget per frame.
7. **One config channel:** if it changes what's on screen, it is in the Scene
   (or camera), diffable, and conformance-tested. No side doors.
8. **Deterministic simulation:** same (inputs, time, seed) ⇒ same fields ⇒
   same pixels, on every platform.
9. **Every subsystem reports:** budgets, inspect JSON, ≥1 debug view, ≥1
   golden, ≥1 sim gate — enforced by a registry-driven meta-test.

## Part VII — Risks

| Risk | Mitigation |
| --- | --- |
| Formalization becomes ceremony (traits nobody needed) | Each contract is extracted *from* working code (lifecycle from the 6 sets, passes from `Map::render`), never invented ahead of a second consumer. Phase gates are measurements, not checklists. |
| Frame-graph refactor destabilizes rendering | Same playbook as the single-pass refactor: port 1:1, goldens prove pixel-equivalence before any new pass lands. |
| `StreamingPlan` churn across FFI while apps ship | Keep `pending_tiles` as a shim over the plan for one release; Android reconciler shrinks incrementally. |
| Bundled baseline bloats app size | Size budget in CI for the artifact; coarse-zoom-only baseline (z≤10 Norway ≈ tens of MB); detail always streams. |
| Sim determinism vs floating point across platforms | Fields computed in f32 with defined order; goldens per-backend with perceptual tolerance (existing golden discipline). |
| Scope: six subsystems at once | Strict phase ordering; every phase independently shippable and valuable (Phase 1 alone fixes the worst UX pain). |

## Part VIII — Recommended first artifact

Phase 0 + Phase 1, steps 1–2: **the lifecycle type, the priority score, and the
`StreamingPlan` boundary** — the smallest change that converts streaming from
emergent behaviour into stated rules on every platform, and the foundation the
other five subsystems' data needs will ride on.
