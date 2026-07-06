# Turbomap Engine Architecture — a Decima-inspired, subsystem-formalized design

**Status:** design / direction-setting · **Date:** 2026-07-03
**Scope:** `apps/turbomap` — the wgpu renderer core, the Scene/engine layer, the
tile/streaming stack, and every host (desktop, FFI/Android, web).
**Builds on:** `2026-06-map-engine-architecture.md` (the MapEngine contract —
kept), `2026-06-turbomap-tile-pipeline-plan.md` (absorbed: its Slices 1–4 become
Phase 1 here), `2026-06-terrain-lod-horizon-shadows.md`,
`2026-06-tile-lod-retention-and-crossfade.md`, `2026-06-tile-data-architecture.md`.

**Revision 2026-07-03 (same day):** after the owner's representation-independence
challenge — *"are we modeling the engine around PNG/DEM/XYZ tiles just because
that's what we fetch today? What if we move solely to geometry?"* — Part III
was reworked around a generalized chunk-tree + codec model and a `Surface`
ground authority. The binding outcomes are in **Part 0 — Decision record**.

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

## Part 0 — Decision record (owner interview, 2026-07-03)

The representation-independence review asked whether the design was limited by
the current implementation: DEM and topo raster tiles were chosen for
*availability*, not as the model — the engine must not be shaped to think in
those formats. The forks were resolved in a structured interview; these
decisions bind the rest of the document.

| # | Decision | Choice |
| --- | --- | --- |
| D1 | World model | **2.5D single-valued surface + discrete 3D objects.** The ground is always `z = f(x,y)` — an invariant, not a storage format. Discrete 3D content (buildings, placed models) sits *on* it. Full-3D fused scenes (caves, overhangs, photogrammetry-as-ground) are out of scope. |
| D2 | Spatial address | **Generalized chunk tree.** Every streamable dataset is a tree of chunks: bounding volume + geometric error (meters) + refine mode (Replace/Add), payload opaque — the 3D Tiles model. The Web-Mercator XYZ pyramid is implicit instance #1, computed not stored. |
| D3 | 3D Tiles interop | **Consume-only, milestoned.** The internal tree must map a 3D Tiles tileset losslessly from day one; the actual `tileset.json` + glTF codec is a later, gated milestone validated against one real Norwegian dataset. The tileserver is *not* committed to producing 3D Tiles. |
| D4 | Coordinate frame | **Normalized Mercator `[0,1]` + RTC stays the internal world.** Codecs reproject (ECEF/UTM/anything) into chunk-local Mercator at decode time. Globe rendering, if it comes, is a camera/projection mode — never a world-frame change. |
| D5 | Format seam | **Hybrid: engine codec registry + raw-representation ingest.** Hosts deliver opaque bytes; engine codecs (own worker pool) decode into a closed representation set. A documented raw path lets hosts with native decoders — and test harnesses — hand ready representations directly. |
| D6 | Ground conformance | **Surface seam now, composited ground material later.** `Surface` becomes the ground authority; `HeightfieldSurface` keeps today's per-pipeline displacement as its private implementation. Material compositing arrives scoped to `MeshSurface`, if adopted. |
| D7 | Terrain roadmap | **Terrain-RGB ships; tileserver TIN experiment.** Once the Surface seam exists, the tileserver emits TIN mesh tiles from the Kartverket DEMs it already ingests, for a test region; `MeshSurface` renders behind a flag; goldens + sim decide on data, not vibes. |
| D8 | 3D content horizon | **All four classes plausible within ~2 years:** extruded footprints (have), terrain-as-mesh, municipal 3D buildings, small placed glTF models (huts, towers, 3D POIs). Mesh machinery is core, milestoned. |
| D9 | Sequencing | **Types first, seams staged** *(defaulted — owner: no preference)*: Phase 1 streaming is built on `ChunkKey`/error types from day one; the Surface seam lands with the frame-graph phase; TIN / 3D Tiles / glTF are gated milestones after. |
| D10 | Vector styling locus | **Client-styled, geometry-capable** *(defaulted)*: MVT + client tessellation stays primary (style flexibility, one offline bundle serves all themes); server-baked geometry basemap layers remain expressible as *just another codec* emitting mesh chunks, adoptable per-layer if profiling demands. |
| D11 | 3D building styling | **Stylized geometry** *(defaulted)*: consume geometry only; materialize with the map palette + the shared environment lighting (the Google-Maps look). glTF codec v1 skips materials/textures; photoreal is a later codec capability if a product case appears. |
| D12 | Shadow strategy | **Hybrid** *(defaulted)*: terrain self-shadowing/AO stay heightfield-analytic — any `Surface` can answer height-grid queries (the 2.5D invariant), so the tuned analytic look survives mesh terrain. A compact sun-space shadow-map pass covers discrete 3D objects; the single Environment sun keeps the two coherent. |

Decisions marked *(defaulted)* take the recommended option after the owner
expressed no preference; each is cheap to revisit before its phase begins.

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

### III.1 — S1: World Data (Decima D1) — representation-agnostic by construction

**The audit that reshaped this section.** The first draft of this design named
wire formats in its core type (`LayerDataKind: VectorMvt | RasterRgba |
DemHeight`) — repeating the over-fitting already in the code (`TileId{z,x,y}`
in 269 places across 30 files; the terrain-RGB decode formula compiled into
`shader.wgsl:212`). DEM and topo tiles were chosen because they are what is
widely *available*, not because they are the *model*. If a better world
representation appears, we switch — so no knowledge of "tiles of PNG" may
survive past a decode boundary. S1 is therefore three planes.

**a) Acquisition plane — the chunk tree (format-blind).**
Everything the engine consumes is a **`WorldLayer`**: a typed dataset whose
content is a **tree of chunks** (the 3D Tiles model — D2):

```rust
/// The typed catalog entry — the map's ".core object".
struct WorldLayerDef {
    id: WorldLayerId,                 // stable, host-visible
    tree: TreeShape,                  // how chunks are addressed (below)
    providers: Vec<ProviderRef>,      // ordered chain: first hit wins (III.1.c)
    residency: ResidencyClass,        // budget pool: Ground | Detail | Simulation
}

struct ChunkKey { layer: WorldLayerId, node: NodeId }   // opaque — NOT (z,x,y)
struct ChunkMeta {
    bounds: BoundingVolume,     // region (2D pyramids) or box (3D content)
    geometric_error_m: f32,     // error if THIS chunk renders instead of its children
    refine: Refine,             // Replace | Add
}
enum TreeShape {
    ImplicitQuadtree(PyramidSpec),  // instance #1: today's XYZ — NodeId computed, never fetched
    Explicit,                       // instance #2 (milestoned): fetched tree pages (tileset.json)
}
```

Streaming (S2) sees only `ChunkKey + ChunkMeta + bytes`; its priority math
needs nothing but bounds and error. **Tree expansion is itself a streaming
operation** — explicit trees page their children in under the same priorities
and budgets as payloads. `TileId` survives only as the internal coordinate
math of `ImplicitQuadtree`, not as the engine's vocabulary. A 3D Tiles tileset
maps onto this losslessly (D3); so does an octree, a quadtree of TIN terrain,
or today's basemap pyramid.

**b) Interpretation plane — the codec registry (where formats go to die, D5).**
A codec turns a chunk's payload bytes into exactly one member of a **closed set
of internal representations** — the only things renderer subsystems may consume:

```
PNG / JPEG / WebP ──┐                          ┌─ Texture2D    (imagery)
terrain-RGB ────────┤                          ├─ HeightField  (elevation grids)
MVT ────────────────┤    codec registry       ├─ FeatureSet   (styled → meshes by engine)
quantized-mesh ─────┤──  (engine worker  ────►├─ MeshChunk    (terrain TIN, buildings)
glTF (milestoned) ──┤     pool)                ├─ Field2D      (radar, wind, sea state)
tileset.json ───────┤                          ├─ InstanceSet  (placed models — D8/D11)
(anything future) ──┘                          └─ TreePage     (explicit-tree expansion)
```

- Adding a format = registering a codec. Nothing in streaming, caching, or
  rendering moves.
- **Reprojection happens inside the codec** (D4): ECEF/UTM/whatever lands as
  chunk-local normalized Mercator; nothing downstream may assume a source CRS.
- **The terrain-RGB decode leaves WGSL**: heights arrive as height data
  (`R16`/`R32` texture or mesh); `DemEncoding` becomes a codec parameter, not
  a shader branch. Same for `RasterFormat` — pipelines see `Texture2D`, period.
- **Raw ingest escape hatch** (D5): `ingest_representation(key, Representation)`
  lets hosts with native decoders (iOS ImageIO, Android hardware) and test
  harnesses inject representations directly, bypassing codecs but nothing else.
- `FeatureSet` is a *resident* representation: a style change re-tessellates
  from memory without refetching bytes (today's style-epoch mesh cache, made
  explicit). Client styling stays primary (D10) — and a server that ships
  pre-baked styled geometry is *just another codec* emitting `MeshChunk`, so
  "moving solely to geometry" for any layer is a data decision, not an engine
  change. **That sentence is the test this section had to pass.**

**c) The provider chain — remote streaming + bundled baseline, one mechanism.**
Each `WorldLayer` resolves chunk bytes through an *ordered chain*:

```
MemoryCache → DiskCache (one impl, bounded LRU) → BundledArchive (PMTiles file)
            → Remote (HTTP XYZ | PMTiles range | explicit-tree | host-provided)
```

(PMTiles is the *archive format for pyramid-shaped layers* — a packaging
choice the chunk tree deliberately does not care about.)

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

**d) Baseline datasets policy.** Bundle: coarse basemap pyramid, coarse DEM,
font faces, sprite sheet. Stream: everything at detail zooms, radar/weather
(inherently live), satellite. The chain makes this a packaging decision, not
an architecture decision — exactly the property we want.

**e) The Surface — the ground authority (D1/D6).**
One abstraction owns "what is the ground," because the current answer — every
ground pipeline samples the DEM texture and displaces itself in its own vertex
shader — is the deepest representation leak in the engine (it works *only*
because terrain happens to be a texture):

```rust
trait Surface {
    // Queries — the 2.5D invariant: ground is single-valued, z = f(x,y).
    fn elevation_at(&self, w: WorldPoint) -> f32;
    fn normal_at(&self, w: WorldPoint) -> Vec3;
    fn height_grid(&self, region: Bounds, dim: u32) -> HeightGrid; // feeds analytic shadows/AO
    // Rendering — how ground geometry is provided / how content conforms.
    fn ground_binding(&self) -> GroundBinding;  // Heightfield{tex} | Mesh{chunk set}
}
```

- **`HeightfieldSurface`** (today) keeps per-pipeline vertex displacement as
  its *private implementation detail* — introducing the seam changes zero
  pixels; the seam itself is what gets tested.
- **`MeshSurface`** (the D7 TIN experiment) provides real ground geometry;
  ground-conforming content then composites as material on that geometry
  rather than displacing itself — the composited-ground-material rework is
  scoped to *this implementation*, not smeared across every pipeline.
- The analytic lighting suite (cast-shadow horizon march, AO bake),
  hit-testing, route/marker draping, and camera-ground logic (pitch clamp,
  horizon math) consume **Surface queries only**. Because `height_grid()`
  exists on *any* Surface, the tuned analytic terrain look survives a move to
  mesh terrain (D12) — and discrete 3D objects (buildings, placed models) get
  their own compact sun-space shadow-map pass, kept coherent with the analytic
  terrain shadows by the one Environment sun (S4).
- Answering the owner's question directly — *"what if we move solely to
  geometry, how does it affect our pipelines?"* — with this seam: swap the
  Surface implementation and the terrain/DEM codec; basemap texturing, vector
  conformance, shadows, AO, hit-testing, and camera logic follow automatically
  because none of them ever knew the ground was a texture.

### III.2 — S2: The Streaming System (Decima D2 — the heart of this design)

One subsystem owns **all** movement of data toward the GPU. It absorbs the
tile-pipeline plan's Slices 2–4 and generalizes them beyond basemap tiles.
Per D2/D9 it is keyed on `ChunkKey + ChunkMeta` from day one — it never sees
formats or `(z,x,y)`; where "tile" appears below, read "chunk of any tree
shape," and note that expanding an explicit tree's pages is scheduled under
the same lifecycle, priorities, and budgets as payload fetches.

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
// f(tier          — Overview ≺ VisibleNear ≺ VisibleFar ≺ SurfaceForVisible ≺ Prefetch,
//   sse_benefit   — screen-space-error reduction if this chunk lands, computed
//                   from ChunkMeta.geometric_error_m + bounds (S6) — the SAME
//                   number for a raster tile, a TIN chunk, or a building tileset,
//   motion        — dot(camera_velocity, dir_to_chunk): prefetch WHERE WE'RE HEADING,
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

- The stored per-chunk property is **geometric error in meters** (the 3D Tiles
  currency, D2/D3); the runtime metric is **screen-space error in pixels** —
  which is exactly what `lod.rs` already computes, so this is a renaming plus
  a per-chunk field, not a rewrite.
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
Built on the general types from day one (D9): `ChunkKey`/`ChunkMeta` +
geometric-error priorities + the codec registry, with `ImplicitQuadtree` +
PNG/terrain-RGB/MVT codecs as instance #1 of each — nothing in the lifecycle,
plan protocol, or caches names a format or `(z,x,y)`. Design-validate the
chunk tree against the 3D Tiles spec on paper (can a real tileset map in
losslessly?) before freezing the types.
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

### Phase 3 — Frame Graph (S3) + subsystem registry (S7) + Surface seam (D6)
Introduce `PassDesc`/`RenderContribution`; port existing passes 1:1 (goldens
prove pixel-equivalence, exactly like the single-pass refactor); collapse
pipeline ownership; per-pass timestamps + debug-view registry; migrate `Map`
fields into registered subsystems. Make `Layer::Custom` real via phase-bound
contributions. **Extract the Surface seam**: `HeightfieldSurface` wraps
today's displacement unchanged; the analytic shadow/AO suite, hit-testing,
draping, and camera-ground logic move to Surface queries; the DEM decode
leaves WGSL (heights upload as height data via the codec plane).
**Gates:** all goldens byte/perceptually stable; a demo custom layer runs on
desktop+web from one Rust impl; pass-level timings appear in `stats_json`;
grep-gate: no `DemEncoding`/`RasterFormat` reference outside the codec plane.

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

### Milestones behind gates (D3/D7/D8/D11/D12 — schedule when their gate opens)
- **M-TIN (after Phase 3):** tileserver emits TIN mesh tiles from its
  Kartverket DEMs for one test region; `MeshSurface` renders them behind a
  flag; goldens + sim compare fidelity/perf/bandwidth vs `HeightfieldSurface`.
  Adoption is a data-driven decision.
- **M-MODELS (after Phase 3/4):** geometry-only glTF codec + `InstanceSet`
  placement for small placed models (huts, towers, 3D POIs), stylized with the
  map palette (D11); the compact object shadow-map pass lands here (D12).
- **M-3DTILES (after M-MODELS):** `tileset.json` explicit-tree codec +
  streamed mesh tilesets, validated against one real Norwegian 3D-bygg
  dataset, styled per D11. This is the consume-only interop milestone (D3).

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
10. **No format past the codec:** nothing downstream of the interpretation
    plane (pipelines, WGSL, caches, streaming, hit-testing) may reference a
    wire format or a source CRS. (Grep-enforceable.)
11. **Closed representation set:** renderer subsystems consume only
    `{Texture2D, HeightField, MeshChunk, FeatureSet, Field2D, InstanceSet}`;
    adding a member is an architecture decision, not a convenience.
12. **2.5D surface invariant:** the ground is single-valued; every `Surface`
    implementation answers elevation/normal/height-grid queries, whatever its
    internal representation.

## Part VII — Risks

| Risk | Mitigation |
| --- | --- |
| Formalization becomes ceremony (traits nobody needed) | Each contract is extracted *from* working code (lifecycle from the 6 sets, passes from `Map::render`), never invented ahead of a second consumer. Phase gates are measurements, not checklists. |
| Frame-graph refactor destabilizes rendering | Same playbook as the single-pass refactor: port 1:1, goldens prove pixel-equivalence before any new pass lands. |
| `StreamingPlan` churn across FFI while apps ship | Keep `pending_tiles` as a shim over the plan for one release; Android reconciler shrinks incrementally. |
| Bundled baseline bloats app size | Size budget in CI for the artifact; coarse-zoom-only baseline (z≤10 Norway ≈ tens of MB); detail always streams. |
| Sim determinism vs floating point across platforms | Fields computed in f32 with defined order; goldens per-backend with perceptual tolerance (existing golden discipline). |
| Scope: six subsystems at once | Strict phase ordering; every phase independently shippable and valuable (Phase 1 alone fixes the worst UX pain). |
| Chunk tree over-generalizes before a second tree shape exists | Phase 1 design-validates the types against the 3D Tiles spec on paper; the first explicit-tree codec ships only behind M-3DTILES with a real dataset. |
| Surface seam stays single-implementation (untested abstraction) | M-TIN is scheduled specifically to give the seam its second implementation early; until then the seam's contract is exercised by the analytic suite + hit-testing moving onto its queries. |

## Part VIII — Recommended first artifact

Phase 0 + Phase 1, steps 1–2: **the lifecycle type, the priority score, and the
`StreamingPlan` boundary** — the smallest change that converts streaming from
emergent behaviour into stated rules on every platform, and the foundation the
other five subsystems' data needs will ride on.
