# Routing Engine — Module Design

**Status:** proposal
**Companion to:** `2026-07-routing-engine-modularization.md` (the *why* — the
nine couplings and the sequencing). This document is the *what*: every
module, its purpose, its dependencies, its boundary, and which of its APIs
are external contracts versus internal detail.

---

## 0. The two axes

Two independent classifications run through this design. Keeping them apart
is the point of the document.

**Axis 1 — layer.** How far a module is from the hardware and how close it
is to the request.

**Axis 2 — kind.** What sort of thing a module *is*:

| Kind | Meaning | Changes when |
|---|---|---|
| **Technical** | Computes or fetches. Numerics, geometry, storage, I/O. | The algorithm or storage format changes |
| **Contract** | Traits and data types. No behaviour. | A capability is added |
| **Orchestration** | Decides *what runs in what order*. Owns lifecycle, not computation. | Product behaviour changes |
| **Profile** | Data and configuration for one region/domain. | The country or dataset changes |
| **Host** | Adapts the engine to a delivery mechanism. | A new client platform appears |

The current architecture's central failure is that `Pathfinder` is
simultaneously Technical, Contract, Orchestration and (via
`routing_setup.rs`) Profile. Twelve distinct responsibilities in one type
— enumerated in §5.1.

---

## 1. Layer stack

```
┌───────────────────────────────────────────────────────────────────────┐
│ L6  HOSTS                                                       Host  │
│     turbo-route-http · turbo-route-ffi · turbo-route-cli · route-lab   │
│     ── EXTERNAL API SURFACE ──────────────────────────────────────────│
├───────────────────────────────────────────────────────────────────────┤
│ L5  PROFILES                                                  Profile │
│     turbo-profile-no  (everything Norwegian: names, classes, TOML)    │
├───────────────────────────────────────────────────────────────────────┤
│ L4  ORCHESTRATION                                       Orchestration │
│     turbo-route-engine                                                │
│       ├ assembly   config + registries → Engine        (boot-time)    │
│       └ planner    request → Route                     (per-request)  │
│           intake · feasibility · repair · overlay · legs ·            │
│           dispatch · stitch · strategies                              │
├───────────────────────────────────────────────────────────────────────┤
│ L3  ADAPTERS                                                Technical │
│     turbo-geodata-artifacts · -pack · -memory · turbo-proj            │
│     (implement L1 ports over concrete storage)                        │
├───────────────────────────────────────────────────────────────────────┤
│ L2  SERVICES                                                Technical │
│     turbo-route-cost      contributors, composition, registry         │
│     turbo-route-solvers   Solver impls, CostField, corridor, extract  │
│     turbo-route-observe   Observer impls, recording formats           │
├───────────────────────────────────────────────────────────────────────┤
│ L1  CONTRACTS                                                Contract │
│     turbo-route-core      ports · domain types · units · config spec  │
│     ── EXTENSION API SURFACE ─────────────────────────────────────────│
├───────────────────────────────────────────────────────────────────────┤
│ L0  KERNEL                                                  Technical │
│     turbo-geom   pure planar geometry predicates                      │
│     turbo-fmm    eikonal / elastica numerics, generic over Metric     │
└───────────────────────────────────────────────────────────────────────┘
```

### Dependency rules (CI-enforceable)

1. **Downward only.** No module depends on a higher layer. L0 depends on
   nothing but `std`.
2. **L0–L4 are region-agnostic.** A grep for `norway`, `n50`, `fkb`,
   `25833`, `dnt` across L0–L4 must return zero. Today `turbo-tiles-pathfind`
   alone has 44 such references.
3. **Adapters depend on L1 only** — never on L2. An adapter that needs to
   know about cost contributors is misfactored.
4. **L2 services never touch I/O.** They reach data exclusively through L1
   ports. This is what makes the CI corpus (`turbo-geodata-memory`) possible.
5. **Only L4 orchestrates.** L2 solvers solve one leg on one corridor; they
   never split waypoints, retry, cache, or decide strategy.
6. **Only L6 is external.** Everything below is a workspace-internal Rust
   API, free to change under corpus gating.

---

## 2. L0 — Kernel (Technical)

Pure computation. No I/O, no domain vocabulary, no coordinate-system
opinions. Testable with no fixtures.

### `turbo-geom`
*From `turbo-tiles-geom` (already 90% correct).*

| Module | Purpose |
|---|---|
| `predicates` | `segment_polygon_intersection_length`, `segment_linestring_crossings`, `point_in_polygon`, `segment_intersects_aabb` |
| `types` | `PlanarPoint` (`Pod`, casts from mmap), `Aabb`, `Polyline` |
| `resample` | Densify / decimate / arc-length parameterise a polyline |

**Boundary fix:** today its docs state "coordinates are always EPSG:25833".
Delete that assertion. The kernel operates on *planar metres*; which planar
frame is the `Projection` port's business (L1).

**API:** internal (workspace). Stable in practice — pure functions.

### `turbo-fmm`
*From `turbo-tiles-fmm`, which is already the best-factored crate in the
stack: generic over `Metric`, zero dependency on the pathfinder, tested
without artifacts.*

| Module | Purpose |
|---|---|
| `grid` | `GridShape`, `FmmGrid<T>` — row-major (nx, ny, nz) with world placement |
| `heap` | Indexed priority queue for the narrow band |
| `stencil`, `selling` | Stencil construction; Selling reduction for anisotropic metrics |
| `metric` | `trait Metric`, `LocalCost`, `NormForm`, `UniformMetric` |
| `solve` | `solve_2d_isotropic`, `solve_2d_with_metric<M: Metric>`, `StopCondition` |
| `aniso` | Anisotropic 2D Finsler solve |
| `elastica` | Lifted (x, y, θ) grade-limited solve; `trait CellOverlay` |
| `extract` | Gradient-descent path extraction from an arrival field |
| `smooth` | Cost-aware Chaikin smoothing |

**Boundary fix — one real violation:** `tobler.rs` and `tobler_aniso.rs`
encode *hiking physics* (Tobler's pace curve, Naismith weighting) inside a
numeric kernel. Move them to `turbo-route-cost::terrain`; the kernel keeps
`trait Metric` and `trait Elevation` as the plug points. After the move,
`turbo-fmm` is a general eikonal library with no walking in it — reusable
for avalanche runout and viewshed, which the crate docs already anticipate.

**API:** extension API. `Metric`, `CellOverlay`, `Elevation` are how you add
a new solver family.

---

## 3. L1 — Contracts (Contract)

### `turbo-route-core`

The whole system's vocabulary. Traits and data only — **no behaviour beyond
trivial constructors**. Every other crate depends on this; nothing depends
on anything else to use it.

#### `core::units`
`WalkSeconds`, `Metres`, `PaceSPerM`. Newtypes, not bare `f64`. The
contributor model's entire correctness argument rests on "everything is
walk-seconds"; make the compiler enforce it.

#### `core::geo`
`GeoPoint { lon, lat }`, `PlanarPoint { x, y }`, `Bbox`, `Corridor`
(oriented rectangle + `GridShape`). The `GeoPoint`/`PlanarPoint` split is
load-bearing: today both are `[f64; 2]` and the projection boundary is
maintained by comment discipline.

#### `core::domain`
```rust
pub enum Profile { Foot, Bicycle, Ski }   // MOVED from turbo-tiles-graph
pub struct RouteRequest { points, profile, preset, avoid, strategy, ... }
pub struct Route { geometry, legs, waypoint_legs, length_m, duration_s,
                   ascent_m, surface_breakdown, refused_by }
pub struct RouteLeg { kind, range, length_m, ... }
pub enum RouteError { NoCoverage, EndpointRefused, NoRoute, SegmentFailed, ... }
```

`Profile` currently lives in `turbo-tiles-graph` — a domain enum inside a
storage crate, which forces every cost contributor to depend on the graph
artifact format. Moving it is small and unblocks rule 3.

#### `core::ports::data`
The four data capabilities. **These are the answer to "swap DEM sources"
and "add marshes".**

```rust
pub trait ElevationSource: Send + Sync {
    fn sample(&self, p: PlanarPoint) -> Option<f32>;
    fn coverage(&self) -> Bbox;
    fn resolution_m(&self) -> f32;
    fn sample_many(&self, pts: &[PlanarPoint], out: &mut [Option<f32>]);  // default loops
}

pub trait FieldSource: Send + Sync {          // categorical/scalar raster
    fn name(&self) -> &str;
    fn class_at(&self, p: PlanarPoint) -> u8;
    fn value_at(&self, p: PlanarPoint) -> Option<f32> { None }
    fn coverage(&self) -> Bbox;
}

pub trait FeatureSource: Send + Sync {        // vector: lakes, marsh, streams
    fn name(&self) -> &str;
    fn geom_kind(&self) -> GeomKind;
    fn query(&self, aabb: Bbox, f: &mut dyn FnMut(FeatureRef<'_>));
}

pub trait NetworkSource: Send + Sync {        // trails/roads
    fn snap(&self, p: PlanarPoint, radius_m: f32) -> Option<NodeId>;
    fn node(&self, id: NodeId) -> Option<PlanarPoint>;
    fn edges_in(&self, aabb: Bbox, cap: usize, f: &mut dyn FnMut(EdgeRef<'_>));
    fn edge_geometry(&self, id: EdgeId) -> &[PlanarPoint];
    fn attrs(&self, id: EdgeId) -> EdgeAttrs;   // length, gain, surface, marking
}
```

`EdgeAttrs` is the neutral replacement for `turbo_tiles_graph::EdgeRecord`
with its `fkb_type` field — a Norwegian classification currently visible to
every contributor. The profile maps national classes onto a neutral
`Surface` enum at adapter level.

#### `core::ports::projection`
```rust
pub trait Projection: Send + Sync {
    fn to_planar(&self, p: GeoPoint) -> PlanarPoint;
    fn to_geographic(&self, p: PlanarPoint) -> GeoPoint;
    fn epsg(&self) -> u32;
}
```
Lifts `wgs84_to_utm33n` out of `turbo-tiles-elev` (`dem.rs:412`), where an
elevation primitive currently owns the system's coordinate reference.

#### `core::ports::observe`
```rust
pub trait Observer: Send + Sync {
    fn enabled(&self, kind: EventKind) -> bool;   // cheap gate, ~ns
    fn emit(&self, ev: SolverEvent<'_>);
    fn phase(&self, name: &'static str, at_us: u64);
}
```
Solver-agnostic event vocabulary (§6). Replaces the thread-local
`Recorder`/`Tracer` install pattern with an explicit field on
`SolveContext` — the thread-local was chosen to avoid signature churn, but
with `SolveContext` already threaded there is nothing to churn.

#### `core::ports::budget`
```rust
pub struct Budget {
    pub max_cells: u32,           // coarsens cell_m rather than OOMing
    pub max_corridor_m: f64,
    pub max_network_edges: usize,
    pub deadline: Option<Duration>,
}
impl Budget { pub fn server() -> Self; pub fn handheld() -> Self; }
```
`max_cells` is the `RoutingBudget` knob specified but never implemented in
the unification plan — "the one lever that makes any hardware safe". It
belongs in the contract layer because both solvers and the corridor sizer
must honour it.

#### `core::spec`
The declarative configuration schema — serde types, no logic.
`EngineConfig`, `SourceSpec`, `CostSpec`, `ContributorSpec`, `SolverSpec`,
`BudgetSpec`, `PresetSet`. **This is what crosses FFI as a string**, and it
is why adding a data source needs no ABI change.

**API classification:** `core::ports::*` + `core::domain` are the
**extension API** (semver'd, documented, the thing a contributor author
reads). `core::spec` and `core::domain::{RouteRequest, Route, RouteError}`
are **external** — they are serialized into HTTP, FFI, and pack manifests.

---

## 4. L2 — Services (Technical)

### `turbo-route-cost`

The cost model. Depends on `core` + `geom`. **Knows nothing about files,
countries, or solvers.**

| Module | Purpose | Kind |
|---|---|---|
| `contributor` | `trait CostContributor`, `EdgeContext`, `EdgeKind`, `compose_edge_walk_seconds` — **moved verbatim** from `pathfind::contributor` | Contract |
| `probe` | `ElevProbe<E: ElevationSource>` — the shared per-edge sample memo, now generic instead of welded to `Dem` | Technical |
| `model` | `CostModel` (static stack) + `RequestOverlay` (per-request) + `EffectiveCost` resolution | Technical |
| `registry` | `ContributorRegistry`: `kind name → factory(params, &SourceRegistry) -> Arc<dyn CostContributor>` | Orchestration-adjacent |
| `terrain` | Tobler slope, Naismith gain, contour crossing, DEM-coverage penalty, roughness, avalanche — generic over `ElevationSource` | Technical |
| `raster` | `RasterClassContributor` — generic over `FieldSource` | Technical |
| `vector` | `PolygonIntegral`, `PolygonRefusal`, `LineCrossing`, `PointProximity` — generic over `FeatureSource` | Technical |
| `network` | Surface pace, marking bonus, preferred edge, graph slope, total gain, trail proximity — generic over `NetworkSource` | Technical |

#### The important new concept: static stack + request overlay

Today the per-request cost inputs are threaded four different ways: the
cost-config patch is resolved **separately inside each solver**
(`try_build_off_trail_segment_fmm` and `solve_unified_path` each call
`cost_config.with_patch`, and the module comments record that the FMM path
once silently ignored overrides as a result); `off_trail_factor` is appended
by `fmm_adapter::with_off_trail`; the avoid set is passed as a separate
argument; `layer_weights` only ever reached the legacy composer.

Make it one type, resolved once, in one place:

```rust
pub struct RequestOverlay {
    pub config_patch: CostConfigPatch,        // preset + explicit override, pre-merged
    pub layer_weights: HashMap<String, f32>,
    pub avoided_edges: HashSet<EdgeId>,       // from avoid projection
    pub extra: Vec<Arc<dyn CostContributor>>, // e.g. off-trail roughness
}

impl CostModel {
    pub fn resolve(&self, overlay: &RequestOverlay) -> EffectiveCost<'_>;
}
```

`EffectiveCost` is what a `SolveContext` carries. One resolution site, one
thing to test, one thing the debug endpoint can print.

**API:** `CostContributor` + `ContributorRegistry` are **extension API** —
this is the seam for "add lake/marsh/snow data". Everything else internal.

### `turbo-route-solvers`

Algorithms. Depends on `core`, `cost`, `geom`, `fmm`.

| Module | Purpose | Kind |
|---|---|---|
| `solver` | `trait Solver`, `SolveLeg`, `SolveContext<'_>`, `Candidate` | Contract |
| `corridor` | Endpoints + `Budget` → `GridShape`. **Single** implementation (today `corridor_shape` in `unified.rs` and the sizer in `fmm_adapter.rs` are separate) | Technical |
| `field` | `CostField<E>` — per-corridor lazy memo of refusal / pace / elevation / **attribution** | Technical |
| `unified` | `UnifiedAStar` — one A\* over mesh ∪ network in one walk-seconds field (from `unified.rs`) | Technical |
| `fmm` | `FmmGradeLimited` (default off-trail), `FmmAnisotropic` — wrap `turbo-fmm` | Technical |
| `network` | `NetworkDijkstra` — pure on-trail | Technical |
| `extract` | Path extraction, smoothing, resampling, surface breakdown — shared post-processing | Technical |

```rust
pub trait Solver: Send + Sync {
    fn name(&self) -> &'static str;
    fn capabilities(&self) -> SolverCaps;   // needs_network? needs_elevation? anytime?
    fn solve(&self, leg: &SolveLeg, ctx: &SolveContext<'_>) -> Result<Candidate, SolveError>;
}

pub struct SolveContext<'a> {
    pub cost: &'a EffectiveCost<'a>,
    pub elevation: &'a dyn ElevationSource,
    pub network: Option<&'a dyn NetworkSource>,
    pub projection: &'a dyn Projection,
    pub budget: &'a Budget,
    pub observer: &'a dyn Observer,
}
```

`SolverCaps` lets assembly reject an impossible configuration at boot
("`unified` requires a network source; none configured") instead of
returning `NoRoute` at request time — which is what happens today
(`solve_unified_path` returns `Err(NoRoute)` when the graph or DEM is
missing, indistinguishable from a genuine routing failure).

**Performance rule.** The registry holds `Arc<dyn Solver>` and
`Arc<dyn ElevationSource>`, but `CostField<E>` and the solver inner loops
are **generic and monomorphized**. `dyn` is paid once per cell at most —
`turbo-fmm` already demonstrates this with `solve_2d_with_metric<M: Metric>`.
Target overhead versus today: <2% on the corpus DEM-work axis.

**API:** `Solver` is **extension API** — the seam for "swap algorithms".
`CostField`, `corridor`, `extract` are internal.

### `turbo-route-observe`

`Observer` implementations and wire formats. Depends on `core` only.

| Module | Purpose |
|---|---|
| `noop` | Zero-cost default (the `enabled()` gate compiles out the call sites) |
| `memory` | Collect into a `Recording` — tests, A/B diffing |
| `stream` | Bounded channel + decimation → SSE (the current `Recorder` behaviour, generalised) |
| `ndjson` | Append to a file — **on-device capture and CI failure artifacts** |
| `format` | `Recording` serde schema — versioned, **external** |

The `ndjson` sink is what makes a phone-side solve debuggable in the same
frontend as a server solve. It exists only because `Observer` is a port.

---

## 5. L3 — Adapters (Technical)

Each implements L1 ports over one storage strategy. **Depend on `core`
only.** Swapping an adapter changes where bytes come from and nothing else.

| Crate | Implements | Backed by |
|---|---|---|
| `turbo-geodata-artifacts` | all four data ports | today's mmap'd `norway.{dem,mask,graph,vectors}` via `turbo-tiles-{elev,mask,graph,vector}` |
| `turbo-geodata-pack` | all four data ports | a region pack directory (§8) |
| `turbo-geodata-memory` | all four data ports | in-process arrays and vectors — **fixtures, CI corpus, property tests** |
| `turbo-proj` | `Projection` | `Utm33n`, `WebMercator`, `LocalTangent` (fixtures) |

### Composite adapters — composition inside a layer

Adapters compose with each other because they implement the same port. This
is where "swap DEM resolution" actually gets cheap:

```rust
// Picks the coarsest level that still resolves the corridor's cell size.
PyramidElevation::new(vec![dem_10m, dem_40m])

// Decorators, each `impl ElevationSource`
CachedElevation::new(inner, 32 * MB)     // device-sized tile cache
ClampedElevation::new(inner, bbox)       // pack-boundary halo enforcement
FallbackElevation::new(primary, coarse)  // fill national DEM gaps
```

None of these require touching a cost contributor, a solver, or a format.
`PyramidElevation` alone is the difference between a ~85 MB and a ~25 MB
region pack.

**API:** internal. An adapter's constructor signature is referenced only by
its profile's registration function.

---

## 6. L4 — Orchestration (`turbo-route-engine`)

The layer the rest of this document exists to protect. It contains **no
numerics and no I/O** — it decides what runs, in what order, with what
inputs, and what happens when a step fails.

### 5.1 What is being decomposed

`Pathfinder` (1986 LOC) currently holds twelve responsibilities:

| # | Responsibility | Current site | Moves to |
|---|---|---|---|
| 1 | Assembly of layers/contributors | `with_defaults_and_config`, `push_with_native` | `engine::assembly` |
| 2 | Projection at entry/exit | inline `wgs84_to_utm33n` calls | `planner::intake` |
| 3 | Coverage precheck | `point_covered`, `has_graph_anchor` | `planner::feasibility` |
| 4 | Endpoint refusal repair | `snap_endpoints_out_of_refusal` | `planner::repair` |
| 5 | Per-request config resolution | duplicated in 2 solvers | `planner::overlay` |
| 6 | Avoid projection | `avoid::project_avoided_edges` | `planner::overlay` |
| 7 | Waypoint leg splitting | `solve_route_once` | `planner::legs` |
| 8 | Leg caching | `leg_cache` + `leg_fingerprint` | `planner::legs` |
| 9 | Algorithm dispatch | `if prefs.force_off_trail` | `planner::dispatch` |
| 10 | Leg stitching + breakdown | `stitch_legs` | `planner::stitch` |
| 11 | Round-trip composition | `solve_round_trip` | `planner::strategies` |
| 12 | Observability install | thread-local recorder/tracer | `SolveContext.observer` |

### 6.2 `engine::assembly` — boot-time orchestration

```rust
pub struct SourceRegistry {
    elevation: HashMap<String, Arc<dyn ElevationSource>>,
    fields:    HashMap<String, Arc<dyn FieldSource>>,
    features:  HashMap<String, Arc<dyn FeatureSource>>,
    networks:  HashMap<String, Arc<dyn NetworkSource>>,
}

pub struct EngineBuilder {
    sources:      SourceRegistry,
    contributors: ContributorRegistry,
    solvers:      SolverRegistry,
    strategies:   StrategyRegistry,
}

impl EngineBuilder {
    pub fn with_profile(self, p: &dyn Profile) -> Self;   // profile registers factories
    pub fn build(self, cfg: &EngineConfig) -> Result<Engine, ConfigError>;
}
```

`build` **validates the whole configuration up front**:

- every `ContributorSpec.kind` resolves to a registered factory
- every port a contributor requires exists in the `SourceRegistry`
- the selected `Solver`'s `capabilities()` are satisfied
- all source coverages intersect; the engine's coverage is their intersection
- the `Budget` is self-consistent

A misconfiguration is a startup error naming the offending spec entry —
never a runtime `NoRoute`. This is a concrete behavioural improvement over
today, where a missing DEM surfaces as an opaque per-request failure.

`Engine` is immutable and `Send + Sync` after build. Everything mutable
(caches, observers) is per-request or interior-mutable and bounded.

### 6.3 `engine::planner` — per-request orchestration

A named pipeline. Each stage is a free function over explicit inputs, unit
testable without artifacts.

```
RouteRequest
     │
 ┌───▼────────┐  validate arity, project GeoPoint → PlanarPoint,
 │ intake     │  resolve preset name → patch
 └───┬────────┘
 ┌───▼────────┐  coverage ∩ endpoints; network anchor within radius?
 │ feasibility│  → RouteError::NoCoverage (honest failure, early)
 └───┬────────┘
 ┌───▼────────┐  endpoint in a refused cell → snap outward within
 │ repair     │  refusal_snap_m, else RouteError::EndpointRefused
 └───┬────────┘
 ┌───▼────────┐  ONE resolution: preset ∘ override → patch,
 │ overlay    │  layer_weights, avoid polylines → edge ids,
 └───┬────────┘  profile extras → RequestOverlay → EffectiveCost
 ┌───▼────────┐  split at waypoints; per-leg cache probe
 │ legs       │  (fingerprint = overlay hash + leg endpoints)
 └───┬────────┘
 ┌───▼────────┐  select Solver (config default ∨ request hint ∨ caps),
 │ dispatch   │  size corridor under Budget, build SolveContext, solve
 └───┬────────┘
 ┌───▼────────┐  join legs, surface breakdown, waypoint legs,
 │ stitch     │  ascent/descent, refused_by union
 └───┬────────┘
   Route
```

**Cache-key correctness note.** `leg_fingerprint` today hashes `Prefs`. In
the target it must hash the resolved `RequestOverlay` — otherwise two
requests that differ only in preset *name* but resolve to the same patch
miss the cache, and (worse) two that differ in a field the fingerprint
forgot would share a cached leg. Hashing the resolved overlay makes the key
provably complete.

### 6.4 `engine::strategies` — the composition seam

Round-trip is not a solver and not a flag; it is *orchestration composed
from the primitive plan operation plus an overlay mutation*. Today it is a
private method. Make it the extension point:

```rust
pub trait RouteStrategy: Send + Sync {
    fn name(&self) -> &'static str;
    fn plan(&self, req: &RouteRequest, p: &Planner<'_>) -> Result<Route, RouteError>;
}
```

`Planner` exposes exactly one primitive — `plan_legs(points, overlay)` —
and strategies compose it:

| Strategy | Composition |
|---|---|
| `PointToPoint` | `plan_legs(points, overlay)` |
| `RoundTrip` | `plan_legs(out)` → push its geometry into `overlay.avoided_edges` → `plan_legs(back)` → stitch |
| `LoopOfLength(d)` *(future)* | sample candidate far points at `d/2` → `plan_legs` each → score → best |
| `MultiDay(huts)` *(future)* | anchor legs on network POIs, per-leg budget |
| `Alternatives(k)` *(future)* | k plans with escalating avoid overlays → dedupe by geometry hash |

Every one of those is orchestration over an unchanged solver and an
unchanged cost model. That is the test of whether the boundary is in the
right place — and today none of them can be written without editing
`Pathfinder`.

### 6.5 `engine::inspect` — the debug service

The debug capabilities as *engine methods*, not HTTP handlers, so they exist
on-device and in the CLI:

```rust
impl Engine {
    pub fn plan(&self, req: &RouteRequest) -> Result<Route, RouteError>;
    pub fn plan_observed(&self, req: &RouteRequest, o: &dyn Observer) -> Result<Route, RouteError>;
    pub fn cost_breakdown(&self, edge: &EdgeQuery) -> EdgeWalkCost;
    pub fn inspect_corridor(&self, req: &RouteRequest, spec: &InspectSpec) -> CorridorInspection;
    pub fn coverage(&self) -> Bbox;
    pub fn describe(&self) -> EngineDescription;   // resolved config, sources, contributors, solver
}
```

`inspect_corridor` is the capability that does not exist today and is the
cheapest large debugging win: `CostField::ensure()` already computes full
per-cell contributor attribution and discards everything except the composed
multiplier. Retaining it under an `InspectSpec` turns "why did it go
*there*?" from an inference into a lookup.

**API:** `RouteStrategy` is **extension API**. `Engine`'s method set is the
**internal façade that L6 hosts wrap** — the external contract is the host's
serialization of it, not the Rust signatures.

---

## 7. L5 — Profiles (`turbo-profile-no`)

Everything national, in one crate, mostly data:

| Item | Today | In the profile |
|---|---|---|
| Artifact filenames (`norway.dem`, …) | `routing_setup.rs` | `sources.toml` |
| N50/FKB class → neutral `Surface` map | `EdgeRecord.fkb_type` read by contributors | adapter-level mapping table |
| Cost constants: `WATER_CROSS_PENALTY_PER_M = 400.0`, wetland ×1.5, cultivated ×3.0, streams `10 + 5×width`, the landcover multiplier array | **inline in wiring code** | `cost-config.toml` rows |
| Presets (`balanced`, `trail_purist`, …) | `tools/route-presets.toml` via `include_str!` reaching outside the crate | profile crate's own `presets.toml` |
| Projection choice (UTM33N) | ambient in `turbo-tiles-elev` | `projection = "utm33n"` |
| DNT marking semantics | `MarkingBonusContributor` | contributor params |

Fixing the `include_str!("../../../tools/cost-config.toml")` escape is a
hard prerequisite for FFI: a crate that reads three directories outside
itself cannot be vendored into a cdylib.

A profile's only code is a registration function:

```rust
impl Profile for NorwayProfile {
    fn register(&self, b: &mut EngineBuilder) { /* factories + class maps */ }
    fn default_config(&self) -> EngineConfig { /* embedded TOML */ }
}
```

---

## 8. L6 — Hosts: the external API surface

Four hosts wrap the same `Engine`. **These are the only external contracts
in the system**, and each is independently versioned.

| Host | External contract | Consumers | Versioning |
|---|---|---|---|
| `turbo-route-http` | REST + SSE — `/v1/route/plan`, `/plan/stream`, `/v1/debug/*` | web SPA, Android/iOS today, admin, route-lab | URL path (`/v1`) |
| `turbo-route-ffi` | uniffi façade — `open`/`plan`/`plan_observed`/`coverage`/`describe`/`close` | Android (JNA), iOS (Swift) | uniffi checksum + `engine_min` in pack |
| `turbo-route-cli` | subcommands — `plan`, `pack`, `eval`, `bench`, `replay`, `describe` | CI, corpus, ops, pack building | semver |
| `apps/route-lab` | *consumes* HTTP + the `Recording` file format | humans | n/a |

### 8.1 The FFI façade — and why it is deliberately thin

```rust
#[uniffi::export]
impl RouteEngine {
    #[uniffi::constructor]
    pub fn open(config_json: String, pack_dir: String) -> Result<Arc<Self>, FfiError>;
    pub fn plan(&self, request_json: String) -> Result<String, FfiError>;
    pub fn plan_observed(&self, request_json: String,
                         cb: Box<dyn ProgressCallback>) -> Result<String, FfiError>;
    pub fn coverage(&self) -> String;
    pub fn describe(&self) -> String;
}
```

Three properties:

1. **Call frequency is O(routes), not O(cells).** A solve touches ~50 k
   cells and, even after `ElevProbe` and `CostField` memoisation, ~50–250 k
   elevation lookups. Putting the boundary at a *port* — letting Kotlin
   supply the DEM — would cost 5–25 ms of pure marshalling per route and
   destroy the memo locality that cut DEM work by 93%. **Ports must never
   become FFI boundaries.**
2. **Configuration crosses as a string.** Adding a marsh source, switching
   to a 40 m DEM level, or selecting a different solver is an
   `EngineConfig` edit — no ABI change, no binding regeneration, no app
   release coupling.
3. **`catch_unwind` at the boundary.** The server wraps solves in
   `catch_unwind` precisely because the solver does panic in the field
   (`crash_dump.rs` exists for this). Over uniffi an unwind aborts the
   process, so the boundary converts panics into `FfiError` and records the
   request for replay.

The only per-frame callback is `ProgressCallback`, whose rate is already
bounded by the observer's decimation.

### 8.2 Internal vs external — the complete classification

| Tier | What | Stability | Examples |
|---|---|---|---|
| **External** | Crosses a process, language, or disk boundary | Versioned; breaking changes need migration | `RouteRequest`/`Route`/`RouteError` JSON · HTTP `/v1` · uniffi façade · `EngineConfig` schema · pack manifest format · `Recording` format |
| **Extension** | Public Rust, for adding capability | Semver; documented; changes ripple to profiles | `ElevationSource`, `FieldSource`, `FeatureSource`, `NetworkSource`, `Projection`, `Observer` · `CostContributor` · `Solver` · `RouteStrategy` · the registries |
| **Internal** | Workspace-only | Free to change under corpus gating | `CostField` · `corridor` sizing · `EdgeContext` internals · every solver's inner loop · every adapter's internals · `Candidate` · planner stage functions |

**The rule that keeps this honest:** the internal result type (`Candidate`,
today's `Path` with its `debug`/`recording` payloads) is *never* what a host
returns. Hosts return `Route`. `route_plan.rs` already documents exactly
this discipline — "deliberately narrow and decoupled from the internal
`Path` / debug surface" — and it is the one boundary in the current system
that is already right. Generalise it.

---

## 9. Region packs — the storage contract

The pack is an **external format** because it is written by one program and
read by another, on a different device, at a different version. It is
defined in terms of **ports**, not in terms of today's artifacts:

```toml
[pack]
format_version = 1
engine_min     = "0.4.0"
profile        = "no"
bbox           = [8.4, 60.1, 9.9, 61.0]
epsg           = 25833

[[source]] port="elevation" name="dem"    kind="dem-tiles" file="dem.10m" resolution_m=10
[[source]] port="elevation" name="dem_c"  kind="dem-tiles" file="dem.40m" resolution_m=40
[[source]] port="network"   name="trails" kind="csr-graph" file="network"
[[source]] port="features"  name="water"  kind="vectors"   file="vectors"
[[source]] port="features"  name="marsh"  kind="vectors"   file="vectors"
[[source]] port="field"     name="forest" kind="mask"      file="forest.mask"

[elevation]
compose = { kind = "pyramid", levels = ["dem", "dem_c"] }

[cost]   config = "cost-config.toml"
[presets] file  = "presets.toml"
```

Two deliberate properties:

- **The pack carries its own cost config and presets.** Same pack version
  server-side and device-side ⇒ identical geometry, which is a *testable
  assertion*: run the corpus in-process against the pack, run it on-device
  against the same pack, diff geometry hashes.
- **`engine_min`** lets an old app refuse a pack it cannot solve correctly,
  rather than producing a subtly different route.

---

## 10. Observability contract

Replace the Theta\*-shaped vocabulary (`LineOfSightCast`, `took_los`,
`NodePopped` — from a solver that has been replaced) with events every
solver family can emit, including FMM, whose object of interest is a *field*
that the current recorder structurally cannot express:

```rust
pub enum SolverEvent<'a> {
    GridSized      { shape: GridShape, budget_clamped: bool },
    CellSettled    { i: u32, j: u32, arrival_s: f32 },
    FrontierState  { cells: &'a [(u32, u32)] },
    FieldSnapshot  { bbox: Bbox, cell_m: f64, values: &'a [f32] },
    CellVetoed     { i: u32, j: u32, layer: &'static str },
    CellAttributed { i: u32, j: u32, parts: &'a [NamedContribution] },
    NetworkRelaxed { edge: EdgeId, new_g: f32 },
    CandidateImproved { geometry: &'a [PlanarPoint], cost_s: f64 },
    Phase          { name: &'static str, at_us: u64 },
}
```

`apps/route-lab` (extracted from the 2883-line `PlotRoute.tsx`, which is
currently welded into the admin SPA's auth and routing) consumes either a
live engine or a dropped `Recording` file, and adds the two views the
current screen cannot have:

- **Cost attribution heatmap** — colour the corridor by any single
  contributor's walk-seconds; click a cell for its full breakdown. Backed by
  `CellAttributed` / `inspect_corridor`.
- **A/B diff** — two `EngineConfig`s or two solvers on one request;
  geometry overlay plus per-contributor cost delta. This is what makes
  "swap algorithms" and "retune the marsh penalty" operationally real.

Because `Observer` is a port with an `ndjson` sink, a recording captured on
a phone loads into the same tool. That is the only practical way to debug a
device-only divergence.

---

## 11. Composition worked through

Six scenarios, all against the same `Engine` type. Note what changes in
each — and what does not.

**A. Server, Norway, national.**
`artifacts` adapter · full contributor stack · `UnifiedAStar` ·
`Budget::server()` · `stream` observer behind `?record=1`.

**B. Handheld, region pack.**
`pack` adapter · `PyramidElevation[10 m, 40 m]` · **identical `CostSpec`**
(it comes from the pack) · same solver · `Budget::handheld()` (`max_cells`
clamp, 32 MB tile cache) · `ndjson` observer on demand.
*Changed: one adapter and one budget. Not changed: cost model, solvers,
orchestration, the route contract.*

**C. CI corpus.**
`memory` adapter with a synthetic DEM · same stack · `memory` observer ·
asserts geometry hashes. **Runs in CI with no artifacts and no server** —
today's corpus POSTs to a live tileserver and is skip-on-unreachable, so a
clean checkout silently executes zero scenarios.

**D. Add marsh data.**
```toml
[[source]] port="features" name="marsh" kind="vectors" file="vectors"
[[contributor]] kind="polygon-integral" source="marsh" s_per_m=1.9
```
**Zero Rust.** The `polygon-integral` contributor already exists and is
generic over `FeatureSource`. Today this is: a new contributor, a paired
legacy layer, a hardcoded block in `routing_setup.rs`, and a magic constant
in wiring code.

**E. New algorithm.**
`impl Solver for ContractionHierarchy`, one registry line, select with
`solver = "ch"`. Compare against the incumbent with route-lab's A/B view on
the corpus. No changes to cost, data, orchestration, or any host.

**F. Experiment in one process.**
Two `Engine`s from two `EngineConfig`s over one shared `SourceRegistry` —
the `Arc<dyn …Source>`s are shared, so a 20 m-vs-10 m DEM comparison costs
one extra mmap, not two engines' worth of data.

---

## 12. Boundary invariants (CI-checkable)

1. `grep -riE 'norway|n50|fkb|dnt|25833'` over L0–L4 returns nothing.
2. No crate in L0–L2 depends on `memmap2`, `zstd`, `reqwest`, `tokio`, or
   `std::fs`.
3. No `pub` item in L2–L4 names a concrete adapter type.
4. `turbo-route-core` has zero dependencies outside `serde` + `std`.
5. Adapters do not depend on `turbo-route-cost` or `-solvers`.
6. Every `Solver` passes the same conformance suite (admissibility on a
   uniform field, determinism, budget compliance, observer contract).
7. Corpus geometry hashes are unchanged across any refactor step, or the
   change is explained in the commit.

---

## 13. Mapping from today

| Today | Becomes | Note |
|---|---|---|
| `turbo-tiles-geom` | `turbo-geom` | drop the EPSG assertion |
| `turbo-tiles-fmm` | `turbo-fmm` | move `tobler*` out to `cost::terrain` |
| `pathfind::contributor` | `cost::contributor` | **moved verbatim** — the design is right |
| `pathfind::native_contributors` | `cost::{terrain,raster,vector,network}` | generic over ports |
| `pathfind::{layers,cost,vector_layers}` | **deleted** | legacy multiplicative generation |
| `pathfind::cost_field` | `solvers::field` | + attribution retention |
| `pathfind::unified` | `solvers::unified` | corridor sizing extracted |
| `pathfind::fmm_adapter` | `solvers::fmm` | `DemElevation` → the port |
| `pathfind::{solver_trace,tracer}` | `observe` | new event vocabulary |
| `pathfind::config` | `core::spec` + profile TOML | fixes the `include_str!` escape |
| `pathfind::avoid` | `engine::planner::overlay` | orchestration, not cost |
| `pathfind::Pathfinder` | `engine::{assembly,planner,strategies}` | the twelve-way split (§5.1) |
| `bin::routing_setup` | `turbo-profile-no` | out of the binary crate |
| `turbo_tiles_graph::Profile` | `core::domain::Profile` | domain enum out of a storage crate |
| `elev::wgs84_to_utm33n` | `turbo-proj::Utm33n` | CRS out of the elevation primitive |
| `pathfind::Prefs` | `RouteRequest` + `EngineConfig` + `ObserveSpec` | four concerns, three types |
| `pathfind::Path` | `Candidate` (internal) + `Route` (external) | never return the internal one |
