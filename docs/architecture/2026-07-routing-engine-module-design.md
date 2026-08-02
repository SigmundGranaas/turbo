# Routing Engine — Module Design

**Status:** proposal (rev. 2)
**Companion to:** `2026-07-routing-engine-modularization.md` (the *why* — the
nine couplings and the sequencing) and
`2026-07-routing-engine-design-rationale.md` (the *on what basis*).
This document is the *what*: every module, its purpose, its dependencies,
its boundary, and which of its APIs are external contracts versus internal
detail.

> **Rev. 2 corrects a real error in rev. 1.** Rev. 1 placed the composition
> root *inside* the engine — `Engine::open(config, pack_dir)`, a
> `SourceRegistry`, and an `EngineConfig` whose schema named file formats
> (`kind = "dem-tiles"`, `file = "dem.10m"`). That made the engine
> source-aware: it did I/O, it resolved configuration, and its own contract
> knew about storage. §2 states the corrected principle; §7 is the new
> composition layer that now owns all of it.

---

## 0. The two axes

Two independent classifications run through this design. Keeping them apart
is the point of the document.

**Axis 1 — layer.** How far a module is from the hardware and how close it
is to the request.

**Axis 2 — kind.** What sort of thing a module *is*:

| Kind | Meaning | Changes when |
|---|---|---|
| **Technical** | Computes. Numerics, geometry, algorithms. | The algorithm changes |
| **Contract** | Traits and data types. No behaviour. | A capability is added |
| **Orchestration** | Decides *what runs in what order*. Owns lifecycle, not computation. | Product behaviour changes |
| **Infrastructure** | Fetches, decodes, constructs. All I/O lives here. | A storage format or deployment changes |
| **Profile** | Data and configuration for one region/domain. | The country or dataset changes |
| **Host** | Adapts the engine to a delivery mechanism. | A new client platform appears |

The current architecture's central failure is that `Pathfinder` is
simultaneously Technical, Contract, Orchestration, Infrastructure and (via
`routing_setup.rs`) Profile. Twelve distinct responsibilities in one type
— enumerated in §6.1.

---

## 1. Layer stack

```
┌───────────────────────────────────────────────────────────────────────┐
│ L6  HOSTS                                                       Host  │
│     turbo-route-http · turbo-route-ffi · turbo-route-cli · route-lab   │
│     ── EXTERNAL API SURFACE ──────────────────────────────────────────│
├───────────────────────────────────────────────────────────────────────┤
│ L5  COMPOSITION                            Infrastructure + Profile   │
│     turbo-route-compose   config → open data → construct → Engine     │
│     turbo-profile-no      everything Norwegian (names, classes, TOML) │
│     turbo-geo-frame       geographic ⇄ planar                         │
│     ══ THE I/O AND CONFIGURATION CEILING ═════════════════════════════│
├───────────────────────────────────────────────────────────────────────┤
│ L4  ENGINE                                              Orchestration │
│     turbo-route-engine    PURE. No I/O, no config, no file formats.   │
│       Engine::new(Terrain, CostModel, SolverSet, Budget) -> Engine     │
│       planner: feasibility · repair · overlay · legs · dispatch ·      │
│                stitch      │      strategies · inspect                 │
├───────────────────────────────────────────────────────────────────────┤
│ L3  ADAPTERS                                          Infrastructure  │
│     turbo-geodata-artifacts · -pack · -memory                         │
│     (implement L1 shapes over concrete storage; constructed by L5)    │
├───────────────────────────────────────────────────────────────────────┤
│ L2  SERVICES                                              Technical   │
│     turbo-route-cost      contributors, composition, typed builder    │
│     turbo-route-solvers   Solver impls, CostField, corridor, extract  │
│     turbo-route-observe   Observer impls, recording formats           │
├───────────────────────────────────────────────────────────────────────┤
│ L1  MODEL                                                  Contract   │
│     turbo-route-model     the engine's own data shapes + domain types │
│     ── EXTENSION API SURFACE ─────────────────────────────────────────│
├───────────────────────────────────────────────────────────────────────┤
│ L0  KERNEL                                                Technical   │
│     turbo-geom   pure planar geometry predicates                      │
│     turbo-fmm    eikonal / elastica numerics, generic over Metric     │
└───────────────────────────────────────────────────────────────────────┘
```

### Dependency rules (CI-enforceable)

1. **Downward only.** No module depends on a higher layer. L0 depends on
   nothing but `std`.
2. **L0–L4 are region-agnostic.** A grep for `norway`, `n50`, `fkb`,
   `25833`, `dnt` across L0–L4 must return zero. Today
   `turbo-tiles-pathfind` alone has 44 such references.
3. **L0–L4 perform no I/O and read no configuration.** No `std::fs`, no
   `memmap2`, no `zstd`, no `reqwest`, no `tokio`, no `std::env`, no
   `serde` *deserialization of settings*. §2.
4. **Adapters depend on L1 only** — never on L2 or L4. An adapter that
   knows about cost contributors is misfactored.
5. **Only L4 orchestrates.** L2 solvers solve one leg on one corridor;
   they never split waypoints, retry, cache, or choose strategy.
6. **Only L5 constructs.** Nothing below L5 calls an adapter constructor,
   parses a config file, or resolves a path.
7. **Only L6 is external.** Everything below is a workspace-internal Rust
   API, free to change under corpus gating.

---

## 2. The engine is a library, not a framework

This is the principle rev. 1 violated, and it is worth stating precisely
because several specific decisions follow from it mechanically.

> **The engine receives capabilities. It never acquires them.**
>
> It declares the *shapes* of data it can reason about. It does not know
> what a file is, what a format is, what a pack is, or where anything came
> from. Fetching, decoding, converting, and wiring are infrastructure
> concerns that live strictly above it.

**What this rules out, and what replaces it:**

| Rev. 1 (framework-shaped) | Rev. 2 (library-shaped) |
|---|---|
| `Engine::open(config_json, pack_dir)` | `Engine::new(terrain, cost, solvers, budget)` |
| `SourceRegistry` inside the engine | the caller holds the handles it built |
| `EngineConfig { SourceSpec { kind, file } }` | no config type in the engine at all |
| `ContributorRegistry` (string → factory) | `CostModel::builder()` — typed, no strings |
| `SolverRegistry` (string → factory) | `SolverSet` — a constructed list |
| `RouteRequest { points: Vec<GeoPoint> }` | `RouteRequest { points: Vec<Point> }` (planar) |
| `Projection` as an engine port | `turbo-geo-frame` at L5; the engine has no CRS |
| `Profile { Foot, Bicycle, Ski }` | `ModeId(u8)` — the profile names the modes |

**Why this matters beyond tidiness.** Three concrete consequences:

1. **Portability.** Anything that can answer "what is the height here" can
   drive the engine — a game engine's terrain chunk, a procedurally
   generated heightmap, a unit test's `Vec<f32>`. §3.4 makes this a
   conformance test, not an aspiration.
2. **The stringly-typed layer is quarantined.** Registries trade
   compile-time safety for runtime strings. That trade is *correct* at a
   text-config boundary — that is what a parser is — and *wrong* inside a
   domain library. Moving it to L5 gives the engine typed construction back
   while preserving the FFI benefit (§8.1).
3. **Testability.** `Engine::new` taking constructed values means every
   engine test is a unit test. No fixture files, no temp dirs, no artifact
   directory.

**The one intentional exception:** hosts *may* combine composition and
engine behind a single call — `RouteEngine::open(config, dir)` on the FFI
is fine and desirable (§8.1). The rule constrains *internal* APIs; a host
is allowed to be a convenience façade over two layers, because that is what
a host is for.

---

## 3. L1 — Model (`turbo-route-model`)

The engine's own data model: the shapes it reasons about and the domain
types it produces. Traits and data only. **Zero dependencies outside
`std` + `serde`(derive).** Renamed from rev. 1's `turbo-route-core` to say
what it is — this is the model, not a utility crate.

### 3.1 Shapes, not sources

The rename from rev. 1 is not cosmetic. "Source" implies acquisition;
"field" and "network" name a *queryable shape*. A game engine's heightmap
**is** a heightfield; it is not a source of one.

```rust
/// A continuous scalar field over the plane. Elevation today; the trait
/// says nothing about elevation, tiles, files, or resolution sources.
pub trait Heightfield: Send + Sync {
    fn height_at(&self, p: Point) -> Option<f32>;
    fn extent(&self) -> Extent;
    /// Intrinsic sample spacing (m). Corridor sizing uses it to avoid
    /// asking finer questions than the data can answer.
    fn resolution_m(&self) -> f32;
    /// Bulk path. Default loops over `height_at`; implementations that can
    /// serve a run from one decode override it.
    fn heights_at(&self, pts: &[Point], out: &mut [Option<f32>]) { … }
}

/// A categorical (and optionally scalar) field: landcover, avalanche
/// class, snow depth, a game's biome map.
pub trait ClassField: Send + Sync {
    fn label(&self) -> &str;              // display only, never dispatch
    fn class_at(&self, p: Point) -> u8;
    fn value_at(&self, p: Point) -> Option<f32> { None }
    fn extent(&self) -> Extent;
}

/// An indexed set of planar geometry: lakes, marsh, streams, buildings,
/// a game's no-build zones.
pub trait GeometrySet: Send + Sync {
    fn label(&self) -> &str;
    fn kind(&self) -> GeomKind;           // Point | Polyline | Polygon
    fn query(&self, area: Extent, f: &mut dyn FnMut(FeatureRef<'_>));
}

/// A traversable network of connected segments: trails, roads, a game's
/// nav-mesh edges.
pub trait TraversalNetwork: Send + Sync {
    fn snap(&self, p: Point, radius_m: f32) -> Option<NodeId>;
    fn node(&self, id: NodeId) -> Option<Point>;
    fn edges_in(&self, area: Extent, cap: usize, f: &mut dyn FnMut(EdgeRef<'_>));
    fn geometry(&self, id: EdgeId) -> &[Point];
    fn physical(&self, id: EdgeId) -> EdgePhysical;   // length, gain, loss, max_grade
    fn attrs(&self, id: EdgeId) -> AttrView<'_>;      // opaque semantic bag
    fn mode_cost(&self, id: EdgeId, mode: ModeId) -> f32;
}
```

**`EdgePhysical` vs `AttrView`.** Physical facts the engine can reason
about generically are typed. Semantic classification —
surface type, waymarking scheme, access restriction — is an **opaque
attribute view** that only the contributors configured for a given profile
interpret. This is the mechanism that keeps `fkb_type` and DNT marking out
of the engine, and the codebase already has `AttrView` in
`turbo-tiles-vector` for exactly this purpose.

**`ModeId(u8)`** replaces `Profile { Foot, Bicycle, Ski }`. The engine
needs an *index* into the network's precomputed cost table; it does not
need to know that index 0 means walking. The profile crate names them.
(Rev. 1 kept `Profile` in the domain; that is a leak of the hiking use case
into a general engine, and it fails the game-engine test.)

### 3.2 Geometry and extent

`Point { x, y }` — **planar metres, always.** `Extent` — a planar
bounding box. `Corridor` — an oriented rectangle plus a `GridShape`.

There is **no `GeoPoint` and no `Projection` in the engine.** The engine
works in one planar frame and never learns which one. Geographic
conversion is L5 (§7.3). This is what makes the game-engine case need no
projection at all rather than an identity stub.

### 3.3 Domain types

```rust
pub struct RouteRequest {
    pub points: Vec<Point>,        // planar; ≥ 2
    pub mode: ModeId,
    pub avoid: Vec<Vec<Point>>,    // planar polylines
    pub strategy: StrategyId,
    pub tuning: Tuning,            // resolved scalar knobs — NOT a config file
}

pub struct Route { geometry: Vec<Point>, legs, waypoint_legs, length_m,
                   duration_s, ascent_m, surface_breakdown, refused_by }

pub enum RouteError { OutsideExtent, EndpointBlocked, NoRoute,
                      SegmentFailed { leg, source }, BudgetExceeded }
```

`Tuning` is the crucial distinction from rev. 1's `cost_config_override`:
it is a **resolved struct of scalars**, produced by L5 from presets and
overrides. The engine never merges TOML patches, never resolves a preset
name, never reads a file. It receives numbers.

### 3.4 The portability conformance test

This belongs in the model crate's test suite, and it is the mechanical
check that §2 is being honoured. If it ever needs a file, a config string,
or a projection, the boundary has leaked.

```rust
// A "game engine" driving the router with nothing but memory.
struct ChunkHeights<'a> { cells: &'a [f32], w: u32, h: u32, cell: f32 }

impl Heightfield for ChunkHeights<'_> {
    fn height_at(&self, p: Point) -> Option<f32> { /* index the array */ }
    fn extent(&self) -> Extent { Extent::new(0.0, 0.0, self.w as f64 * self.cell, …) }
    fn resolution_m(&self) -> f32 { self.cell }
}

let height: Arc<dyn Heightfield> = Arc::new(ChunkHeights { … });

let engine = Engine::new(
    Terrain { height: height.clone(), network: None, extent: height.extent() },
    CostModel::builder()
        .add(ToblerSlope::new(height.clone(), CliffDeg(60.0)))
        .add(NaismithGain::new(height.clone(), GainWeight(7.92)))
        .build(),
    SolverSet::only(FmmGradeLimited::default()),
    Budget::interactive(),
)?;

let route = engine.plan(&RouteRequest { points: vec![a, b], mode: ModeId(0), .. })?;
```

No files. No TOML. No pack. No profile. No CRS. No network. That is the
target, and it is a compiling test, not a claim.

---

## 4. L0 — Kernel (Technical)

Pure computation. No I/O, no domain vocabulary, no coordinate-system
opinions. Testable with no fixtures.

### `turbo-geom` *(from `turbo-tiles-geom`, already 90% correct)*

| Module | Purpose |
|---|---|
| `predicates` | `segment_polygon_intersection_length`, `segment_linestring_crossings`, `point_in_polygon`, `segment_intersects_aabb` |
| `types` | `Point` (`Pod`, casts from mmap'd bytes), `Extent`, `Polyline` |
| `resample` | densify / decimate / arc-length parameterise |

**Boundary fix:** its docs currently assert "coordinates are always
EPSG:25833". Delete that. The kernel operates on planar metres; which
planar frame is L5's business.

### `turbo-fmm` *(from `turbo-tiles-fmm` — already the best-factored crate in the stack)*

| Module | Purpose |
|---|---|
| `grid` | `GridShape`, `FmmGrid<T>` |
| `heap`, `stencil`, `selling` | narrow-band queue; stencils; Selling reduction |
| `metric` | `trait Metric`, `LocalCost`, `NormForm` |
| `solve`, `aniso`, `elastica` | isotropic / anisotropic / lifted (x, y, θ) solvers; `trait CellOverlay` |
| `extract`, `smooth` | gradient-descent extraction; cost-aware Chaikin |

**Boundary fix — one real violation:** `tobler.rs` / `tobler_aniso.rs`
encode *hiking physics* inside a numeric kernel. Move to
`turbo-route-cost::terrain`. After the move `turbo-fmm` is a general
eikonal library with no walking in it — reusable for avalanche runout and
viewshed, which its own docs already anticipate.

**API:** extension API. `Metric`, `CellOverlay`, `Elevation` are how a new
solver family plugs in.

---

## 5. L2 — Services (Technical)

### `turbo-route-cost`

Depends on `model` + `geom`. Knows nothing about files, countries, or
solvers.

| Module | Purpose |
|---|---|
| `contributor` | `trait CostContributor`, `EdgeContext`, `EdgeKind`, `compose_edge_walk_seconds` — **moved verbatim** from `pathfind::contributor` |
| `probe` | `ElevProbe<H: Heightfield>` — the shared per-edge sample memo, generic instead of welded to `Dem` |
| `model` | `CostModel` (static stack) + `Overlay` (per-request) + `EffectiveCost` |
| `builder` | **typed** `CostModel::builder()` — no strings, no factories |
| `terrain` | Tobler slope, Naismith gain, contour crossing, coverage penalty, roughness, avalanche — generic over `Heightfield` |
| `field` | `ClassCost` — generic over `ClassField` |
| `geometry` | `PolygonIntegral`, `PolygonRefusal`, `LineCrossing`, `PointProximity` — generic over `GeometrySet` |
| `network` | surface pace, preferred-edge, grade, total-gain, proximity — generic over `TraversalNetwork` + `AttrView` |

#### Typed construction, not a registry

```rust
CostModel::builder()
    .add(ToblerSlope::new(height.clone(), CliffDeg(60.0)))
    .add(NaismithGain::new(height.clone(), GainWeight(7.92)))
    .add(PolygonIntegral::new(water.clone(), SecondsPerMetre(400.0)))
    .add(LineCrossing::new(streams.clone(), |w| 10.0 + 5.0 * w))
    .add(ClassCost::new(forest.clone(), &[(FOREST, SecondsPerMetre(0.29))]))
    .build()
```

Every parameter is a newtype; a transposed argument is a compile error.
L5 does the `"polygon-integral"` → `PolygonIntegral::new` mapping, which
is the only place a string can be wrong and the only place that needs a
runtime error path.

#### Static stack + per-request overlay

Today the per-request cost inputs are threaded four ways: the config patch
resolves **separately inside each solver** (`try_build_off_trail_segment_fmm`
and `solve_unified_path` each call `with_patch` — and the module comments
record that the FMM path once silently ignored overrides as a result);
`off_trail_factor` is appended by `with_off_trail`; the avoid set is a
separate argument; `layer_weights` only ever reached the legacy composer.

One type, resolved once, in the planner:

```rust
pub struct Overlay {
    pub tuning: Tuning,                       // resolved scalars from L5
    pub weights: Weights,                     // per-contributor scale
    pub blocked_edges: HashSet<EdgeId>,       // from avoid projection
    pub extra: Vec<Arc<dyn CostContributor>>, // e.g. off-trail roughness
}
impl CostModel { pub fn resolve(&self, o: &Overlay) -> EffectiveCost<'_>; }
```

**API:** `CostContributor` is **extension API** — the seam for adding lake,
marsh, snow, or biome data.

### `turbo-route-solvers`

Depends on `model`, `cost`, `geom`, `fmm`.

| Module | Purpose |
|---|---|
| `solver` | `trait Solver`, `SolveLeg`, `SolveContext<'_>`, `Candidate`, `SolverCaps` |
| `corridor` | endpoints + `Budget` → `GridShape`. **One** implementation (today `corridor_shape` in `unified.rs` and the sizer in `fmm_adapter.rs` are separate) |
| `field` | `CostField<H>` — per-corridor lazy memo of refusal / pace / height / **attribution** |
| `unified` | `UnifiedAStar` — one A\* over mesh ∪ network in one walk-seconds field |
| `fmm` | `FmmGradeLimited` (default off-trail), `FmmAnisotropic` |
| `network` | `NetworkDijkstra` |
| `extract` | extraction, smoothing, resampling, surface breakdown |

```rust
pub struct SolveContext<'a> {
    pub cost: &'a EffectiveCost<'a>,
    pub height: &'a dyn Heightfield,
    pub network: Option<&'a dyn TraversalNetwork>,
    pub budget: &'a Budget,
    pub observer: &'a dyn Observer,
}
```

`SolverCaps` lets `Engine::new` reject an impossible combination at
construction ("`unified` requires a network; `Terrain.network` is `None`")
instead of returning `NoRoute` at request time — which is what happens
today, where a missing graph or DEM is indistinguishable from a genuine
routing failure.

**Performance rule.** `Arc<dyn Heightfield>` at the boundary;
`CostField<H>` and solver inner loops are **generic and monomorphized**.
`turbo-fmm` already demonstrates this with `solve_2d_with_metric<M: Metric>`.
Budget: <2% on the corpus DEM-work axis.

### `turbo-route-observe`

`Observer` implementations. Depends on `model` only. `noop` (zero-cost
default), `memory` (tests, A/B diff), `stream` (bounded decimation → SSE),
`ndjson` (**device capture and CI failure artifacts**), `format` (the
versioned `Recording` schema — external).

---

## 6. L4 — Engine (`turbo-route-engine`)

Pure orchestration. **No numerics, no I/O, no configuration, no
construction of adapters.** It decides what runs, in what order, with what
inputs, and what happens when a step fails.

```rust
pub struct Terrain {
    pub height: Arc<dyn Heightfield>,
    pub network: Option<Arc<dyn TraversalNetwork>>,
    pub extent: Extent,
}

impl Engine {
    /// Everything is already built. Validates *semantic* consistency:
    /// solver caps vs available capabilities, non-empty extent, sane
    /// budget. Never touches a filesystem.
    pub fn new(terrain: Terrain, cost: CostModel,
               solvers: SolverSet, budget: Budget) -> Result<Self, EngineError>;

    pub fn plan(&self, req: &RouteRequest) -> Result<Route, RouteError>;
    pub fn plan_observed(&self, req: &RouteRequest, o: &dyn Observer)
                         -> Result<Route, RouteError>;
    pub fn cost_breakdown(&self, e: &EdgeQuery) -> EdgeWalkCost;
    pub fn inspect_corridor(&self, req: &RouteRequest, s: &InspectSpec) -> CorridorInspection;
    pub fn extent(&self) -> Extent;
    pub fn describe(&self) -> EngineDescription;   // what it HAS, not where it came from
}
```

### 6.1 What is being decomposed

`Pathfinder` (1986 LOC) currently holds twelve responsibilities:

| # | Responsibility | Current site | Moves to |
|---|---|---|---|
| 1 | Assembly of layers/contributors | `with_defaults_and_config`, `push_with_native` | **L5 `compose`** |
| 2 | Projection at entry/exit | inline `wgs84_to_utm33n` | **L5 `frame`** |
| 3 | Coverage precheck | `point_covered`, `has_graph_anchor` | `planner::feasibility` |
| 4 | Endpoint refusal repair | `snap_endpoints_out_of_refusal` | `planner::repair` |
| 5 | Per-request config resolution | duplicated in 2 solvers | split: preset merge → **L5**; overlay build → `planner::overlay` |
| 6 | Avoid projection | `avoid::project_avoided_edges` | `planner::overlay` |
| 7 | Waypoint leg splitting | `solve_route_once` | `planner::legs` |
| 8 | Leg caching | `leg_cache` + `leg_fingerprint` | `planner::legs` |
| 9 | Algorithm dispatch | `if prefs.force_off_trail` | `planner::dispatch` |
| 10 | Leg stitching + breakdown | `stitch_legs` | `planner::stitch` |
| 11 | Round-trip composition | `solve_round_trip` | `planner::strategies` |
| 12 | Observability install | thread-local recorder/tracer | `SolveContext.observer` |

Note rows 1, 2 and half of 5 leave the engine entirely — that is the rev. 2
correction expressed as a diff.

### 6.2 `planner` — per-request orchestration

```
RouteRequest  (planar points, resolved Tuning)
     │
 ┌───▼────────┐  arity; endpoints within Terrain.extent; network anchor
 │ feasibility│  → RouteError::OutsideExtent (honest failure, early)
 └───┬────────┘
 ┌───▼────────┐  endpoint in a blocked cell → snap outward within
 │ repair     │  tuning.repair_radius_m, else EndpointBlocked
 └───┬────────┘
 ┌───▼────────┐  ONE construction: Tuning + weights + avoid→edge ids
 │ overlay    │  + mode extras → Overlay → EffectiveCost
 └───┬────────┘
 ┌───▼────────┐  split at waypoints; cache probe keyed on the RESOLVED
 │ legs       │  overlay hash + leg endpoints
 └───┬────────┘
 ┌───▼────────┐  select Solver by caps + request hint; size corridor
 │ dispatch   │  under Budget; build SolveContext; solve
 └───┬────────┘
 ┌───▼────────┐  join legs, surface breakdown, ascent, refused_by union
 │ stitch     │
 └───┬────────┘
   Route  (planar geometry)
```

**Parnas caveat (carried from the rationale doc).** This is a *flowchart*
decomposition, which Parnas argues against. The defensible reading is that
each stage hides a **policy** that changes independently. Rule that
follows: **if a stage hides no policy, inline it.** A stage that is only a
sequence position is not a module.

**Cache-key correctness.** `leg_fingerprint` today hashes `Prefs`. It must
hash the *resolved* `Overlay` — otherwise two requests differing only in
preset name miss the cache, and (worse) two differing in a field the
fingerprint forgot silently share a cached leg.

### 6.3 `strategies` — the composition seam

Round-trip is not a solver and not a flag; it is orchestration composed
from the primitive plan operation plus an overlay mutation.

```rust
pub trait RouteStrategy: Send + Sync {
    fn id(&self) -> StrategyId;
    fn plan(&self, req: &RouteRequest, p: &Planner<'_>) -> Result<Route, RouteError>;
}
```

`Planner` exposes exactly one primitive — `plan_legs(points, overlay)`:

| Strategy | Composition |
|---|---|
| `PointToPoint` | `plan_legs(points, overlay)` |
| `RoundTrip` | `plan_legs(out)` → push geometry into `overlay.blocked_edges` → `plan_legs(back)` → stitch |
| `LoopOfLength(d)` *(future)* | sample far points at `d/2` → `plan_legs` each → score |
| `MultiDay(stops)` *(future)* | anchor legs on network nodes, per-leg budget |
| `Alternatives(k)` *(future)* | k plans with escalating blocked sets → dedupe by geometry hash |

All four future strategies are orchestration over an **unchanged** solver
and cost model. That is the test of whether the seam is placed correctly —
and today none can be written without editing `Pathfinder`.

### 6.4 `inspect` — debug as engine methods

Debug capabilities are engine methods, not HTTP handlers, so they exist
on-device and in the CLI. `inspect_corridor` is the capability that does
not exist today and is the cheapest large debugging win: `CostField::ensure()`
already computes full per-cell contributor attribution and discards
everything except the composed multiplier. Retaining it under an
`InspectSpec` turns "why did it go *there*?" from an inference into a
lookup.

---

## 7. L5 — Composition (Infrastructure + Profile)

Everything rev. 1 wrongly put in the engine. **All I/O, all configuration,
all format knowledge, all construction, and the only coordinate-reference
system in the codebase.**

### 7.1 `turbo-route-compose`

```rust
pub struct Composed {
    pub engine: Engine,
    pub frame: Frame,          // for hosts that speak lon/lat
    pub presets: PresetSet,    // for hosts that resolve preset names
    pub description: SourceProvenance,   // what was opened, from where
}

pub fn compose(cfg: &ComposeConfig, root: &Path, profile: &dyn Profile)
    -> Result<Composed, ComposeError>;
```

What it owns:

| Concern | Detail |
|---|---|
| **Config schema** | `ComposeConfig`, `SourceSpec { kind, path, params }`, `CostSpec`, `SolverSpec`, `BudgetSpec` — the serde types. **The engine has no config type.** |
| **Source resolution** | `kind = "dem-tiles"` → `turbo_geodata_artifacts::DemHeightfield::open(path)`. One `match` on a string, one error path. |
| **Composite adapters** | `Pyramid::new(vec![dem10, dem40])`, `Cached::new(inner, 32 MB)`, `Clamped::new(inner, extent)`, `Fallback::new(fine, coarse)` — all `impl Heightfield`, composed here |
| **Cost translation** | `CostSpec` rows → typed `CostModel::builder()` calls |
| **Preset resolution** | preset name + override → a resolved `Tuning` struct |
| **Validation** | file exists, format version, `engine_min`, extents intersect |
| **Provenance** | what was opened and from where, for `describe()` and support |

**The whole stringly-typed surface of the system lives in this one crate.**
That is not a compromise — mapping text to typed objects is what a parser
is, and confining it to one layer is the point.

### 7.2 `turbo-profile-no`

Everything national, mostly data:

| Item | Today | In the profile |
|---|---|---|
| Artifact filenames (`norway.dem`, …) | `routing_setup.rs` | `sources.toml` |
| N50/FKB class → neutral attrs | `EdgeRecord.fkb_type` read by contributors | adapter-level mapping table |
| `WATER_CROSS_PENALTY_PER_M = 400.0`, wetland ×1.5, cultivated ×3.0, streams `10 + 5×width`, the landcover multiplier array | **inline in wiring code** | `cost-config.toml` rows |
| Presets | `tools/route-presets.toml` via `include_str!` reaching three dirs outside the crate | the profile's own `presets.toml` |
| CRS choice (UTM33N) | ambient in `turbo-tiles-elev` | `frame = "utm33n"` |
| `ModeId` names (foot/bicycle/ski) | `Profile` enum in `turbo-tiles-graph` | `modes.toml` |

Fixing the `include_str!("../../../tools/cost-config.toml")` escape is a
hard prerequisite for FFI: a crate reading outside itself cannot be
vendored into a cdylib.

### 7.3 `turbo-geo-frame`

`Frame { to_planar(GeoPoint) -> Point, to_geographic(Point) -> GeoPoint,
epsg() -> u32 }` with `Utm33n`, `WebMercator`, `LocalTangent`.

This lifts `wgs84_to_utm33n` out of `turbo-tiles-elev` (`dem.rs:412`),
where an *elevation primitive currently owns the system's coordinate
reference*. It lands at L5, not L1 — the engine never converts coordinates
because it never sees a geographic one.

### 7.4 L3 adapters, constructed here

| Crate | Implements | Backed by |
|---|---|---|
| `turbo-geodata-artifacts` | all four shapes | today's mmap'd `norway.{dem,mask,graph,vectors}` |
| `turbo-geodata-pack` | all four shapes | a region pack directory (§9) |
| `turbo-geodata-memory` | all four shapes | in-process arrays — fixtures, CI corpus, property tests |

Adapters depend on `turbo-route-model` only. They are *constructed* by
`compose`, never by the engine.

---

## 8. L6 — Hosts: the external API surface

| Host | External contract | Consumers |
|---|---|---|
| `turbo-route-http` | REST + SSE — `/v1/route/plan`, `/plan/stream`, `/v1/debug/*` | web SPA, mobile today, admin, lab |
| `turbo-route-ffi` | uniffi façade | Android (JNA), iOS (Swift) |
| `turbo-route-cli` | `plan`, `pack`, `eval`, `bench`, `replay`, `describe` | CI, corpus, ops |
| `apps/route-lab` | consumes HTTP + the `Recording` file format | humans |

### 8.1 The FFI façade — combining the two layers deliberately

```rust
#[uniffi::export]
impl RouteEngine {
    #[uniffi::constructor]
    pub fn open(config_json: String, data_dir: String) -> Result<Arc<Self>, FfiError> {
        let cfg: ComposeConfig = serde_json::from_str(&config_json)?;
        let c = turbo_route_compose::compose(&cfg, Path::new(&data_dir), &NorwayProfile)?;
        Ok(Arc::new(Self { engine: c.engine, frame: c.frame, presets: c.presets }))
    }

    /// Accepts lon/lat and a preset NAME; projects and resolves here,
    /// then calls the pure engine.
    pub fn plan(&self, request_json: String) -> Result<String, FfiError> { … }
    pub fn plan_observed(&self, request_json: String,
                         cb: Box<dyn ProgressCallback>) -> Result<String, FfiError>;
    pub fn coverage(&self) -> String;
    pub fn describe(&self) -> String;
}
```

The host is where composition and engine are joined, where geographic
coordinates are converted, and where preset names are resolved. **All
three are things the engine must not do, and all three are things a host
exists to do.**

Three properties that make this the right boundary:

1. **Call frequency is O(routes), not O(cells).** A solve touches ~50 k
   cells and, after memoisation, ~50–250 k height lookups. Putting FFI at a
   *shape* — letting Kotlin answer `height_at` — would cost 5–25 ms of
   marshalling per route and destroy the memo locality that cut DEM work by
   93%. **Shapes must never become FFI boundaries.**
2. **Configuration crosses as a string.** Adding a marsh source, switching
   to a 40 m level, or selecting a different solver is a `ComposeConfig`
   edit — no ABI change, no binding regeneration, no app-release coupling.
3. **`catch_unwind` at the boundary.** The server wraps solves in
   `catch_unwind` because the solver does panic in the field (`crash_dump.rs`
   exists for this). Over uniffi an unwind aborts the process.

### 8.2 Internal vs external — the complete classification

| Tier | What | Stability | Examples |
|---|---|---|---|
| **External** | Crosses a process, language, or disk boundary | Versioned; breaking changes need migration | the HTTP `/v1` JSON contract · uniffi façade · `ComposeConfig` schema · pack manifest · `Recording` format |
| **Extension** | Public Rust, for adding capability | Semver; documented | `Heightfield`, `ClassField`, `GeometrySet`, `TraversalNetwork` · `CostContributor` · `Solver` · `RouteStrategy` · `Observer` · `Profile` |
| **Internal** | Workspace-only | Free to change under corpus gating | `CostField` · `corridor` · `EdgeContext` internals · solver inner loops · adapter internals · `Candidate` · planner stage functions |

**The rule that keeps this honest:** the engine's `Route` is planar and
internal; hosts serialize a geographic DTO. `route_plan.rs` already
documents exactly this discipline — "deliberately narrow and decoupled from
the internal `Path` / debug surface" — and it is the one boundary in the
current system that is already right.

---

## 9. Region packs — an L5 format, not an engine format

The pack is written by `turbo-route-cli pack` and read by
`turbo-geodata-pack`. **The engine never hears of it.**

```toml
[pack]
format_version = 1
engine_min     = "0.4.0"
profile        = "no"
extent         = [8.4, 60.1, 9.9, 61.0]
frame          = "utm33n"

[[source]] shape="height"   name="dem"    kind="dem-tiles" path="dem.10m" resolution_m=10
[[source]] shape="height"   name="dem_c"  kind="dem-tiles" path="dem.40m" resolution_m=40
[[source]] shape="network"  name="trails" kind="csr-graph" path="network"
[[source]] shape="geometry" name="water"  kind="vectors"   path="vectors"
[[source]] shape="geometry" name="marsh"  kind="vectors"   path="vectors"
[[source]] shape="class"    name="forest" kind="mask"      path="forest.mask"

[height] compose = { kind = "pyramid", levels = ["dem", "dem_c"] }

[cost]    config = "cost-config.toml"
[presets] path   = "presets.toml"
```

Two deliberate properties:

- **The pack carries its own cost config and presets**, so the same pack
  version server-side and device-side yields identical geometry — a
  *testable* assertion against the existing geometry-hash harness, not a
  hope.
- **`engine_min`** lets an old app refuse a pack it cannot solve correctly
  rather than silently routing differently.

Note the schema now says `shape=` rather than `port=`: it declares which
engine shape the source will be presented as. That word choice is the
boundary made visible in the file format.

---

## 10. Observability contract

Replace the Theta\*-shaped vocabulary (`LineOfSightCast`, `took_los`,
`NodePopped` — from a solver that has been replaced) with events every
solver family can emit, including FMM, whose object of interest is a *field*
the current recorder structurally cannot express:

```rust
pub enum SolverEvent<'a> {
    GridSized      { shape: GridShape, budget_clamped: bool },
    CellSettled    { i: u32, j: u32, arrival_s: f32 },
    FrontierState  { cells: &'a [(u32, u32)] },
    FieldSnapshot  { extent: Extent, cell_m: f64, values: &'a [f32] },
    CellBlocked    { i: u32, j: u32, by: &'static str },
    CellAttributed { i: u32, j: u32, parts: &'a [NamedContribution] },
    NetworkRelaxed { edge: EdgeId, new_g: f32 },
    CandidateImproved { geometry: &'a [Point], cost_s: f64 },
    Phase          { name: &'static str, at_us: u64 },
}
```

Coordinates are planar — the host converts once at serialization, which is
what the current recorder already does.

`apps/route-lab` (extracted from the 2883-line `PlotRoute.tsx`, currently
welded into the admin SPA's auth and routing) consumes a live engine or a
dropped `Recording` file, and adds:

- **Cost attribution heatmap** — colour the corridor by any single
  contributor's walk-seconds; click a cell for its breakdown.
- **A/B diff** — two compositions or two solvers on one request; geometry
  overlay plus per-contributor cost delta.

Because `Observer` is a shape with an `ndjson` sink, a recording captured
on a phone loads into the same tool.

---

## 11. Composition worked through

Six scenarios. Note that **only L5 changes** in the first three.

**A. Server, Norway, national.** `compose` opens the artifacts adapter ·
full contributor stack · `UnifiedAStar` · `Budget::server()` · `stream`
observer behind `?record=1`.

**B. Handheld, region pack.** `compose` opens the pack adapter ·
`Pyramid[10 m, 40 m]` · **identical `CostSpec`** (it comes from the pack) ·
same solver · `Budget::handheld()` · `ndjson` observer on demand.
*Changed: one `compose` branch and one budget. Unchanged: model, cost,
solvers, engine, contracts.*

**C. CI corpus.** `compose` with the memory adapter and a synthetic
heightfield. **Runs in CI with no artifacts and no server** — today's
`tests/scenarios.rs` POSTs to a live tileserver and is skip-on-unreachable,
so a clean checkout silently executes zero scenarios.

**D. Add marsh data.**
```toml
[[source]] shape="geometry" name="marsh" kind="vectors" path="vectors"
[[cost]]   kind="polygon-integral" source="marsh" s_per_m=1.9
```
**Zero Rust.** `PolygonIntegral` already exists and is generic over
`GeometrySet`; `compose` already knows the `"polygon-integral"` string.

**E. New algorithm.** `impl Solver for ContractionHierarchy`, one line in
`compose`'s solver match, select with `solver = "ch"`. No changes to model,
cost, data, engine, or hosts.

**F. Game engine / simulation.** No `compose` at all — construct `Terrain`
from memory and call `Engine::new` directly (§3.4). This is the case that
proves the boundary, and it is the one rev. 1 could not serve.

---

## 12. Boundary invariants (CI-checkable)

1. `grep -riE 'norway|n50|fkb|dnt|25833'` over L0–L4 returns nothing.
2. No crate in L0–L4 depends on `memmap2`, `zstd`, `reqwest`, `tokio`,
   `std::fs`, or `std::env`.
3. **No crate in L0–L4 has a type that deserializes settings, and no
   function in L0–L4 takes a `&Path`, a filename, or a format name.**
4. No `pub` item in L2–L4 names a concrete adapter type.
5. Adapters depend on `turbo-route-model` only.
6. `turbo-route-model` has zero dependencies outside `serde` + `std`.
7. The portability conformance test (§3.4) compiles and passes with no
   fixture files.
8. Every `Solver` passes the same conformance suite (admissibility on a
   uniform field, determinism, budget compliance, observer contract).
9. Corpus geometry hashes are unchanged across any refactor step, or the
   change is explained in the commit.

Invariants 3 and 7 are the mechanical guards on §2. Invariant 3 is worth
stating as bluntly as possible: **if a signature in L0–L4 mentions a path,
the boundary has been breached.**

---

## 13. Mapping from today

| Today | Becomes | Note |
|---|---|---|
| `turbo-tiles-geom` | `turbo-geom` | drop the EPSG assertion |
| `turbo-tiles-fmm` | `turbo-fmm` | move `tobler*` to `cost::terrain` |
| `pathfind::contributor` | `cost::contributor` | **moved verbatim** — the design is right |
| `pathfind::native_contributors` | `cost::{terrain,field,geometry,network}` | generic over shapes |
| `pathfind::{layers,cost,vector_layers}` | **deleted** | legacy multiplicative generation |
| `pathfind::cost_field` | `solvers::field` | + attribution retention |
| `pathfind::unified` | `solvers::unified` | corridor sizing extracted |
| `pathfind::fmm_adapter` | `solvers::fmm` | `DemElevation` → `Heightfield` |
| `pathfind::{solver_trace,tracer}` | `observe` | new event vocabulary |
| `pathfind::config` | **`compose` + profile TOML** | fixes the `include_str!` escape |
| `pathfind::avoid` | `engine::planner::overlay` | orchestration, not cost |
| `pathfind::Pathfinder` | `engine::{planner,strategies}` **+ `compose`** | the twelve-way split (§6.1) |
| `bin::routing_setup` | `turbo-route-compose` + `turbo-profile-no` | out of the binary crate |
| `turbo_tiles_graph::Profile` | `model::ModeId` + profile-supplied names | domain enum out of a storage crate, use case out of the engine |
| `elev::wgs84_to_utm33n` | `turbo-geo-frame::Utm33n` (**L5**) | CRS out of the elevation primitive *and* out of the engine |
| `pathfind::Prefs` | `RouteRequest` + `ComposeConfig` + `Tuning` + `ObserveSpec` | four concerns, four types, two layers |
| `pathfind::Path` | `Candidate` (internal) + `Route` (planar) + host DTO (geographic) | never return the internal one |
