# Routing Engine Modularization — Ports, Algorithms, Packaging, Debugging

**Status:** proposal
**Supersedes in scope:** the open P4 item of
`2026-06-routing-engine-unification-plan.md` (delete legacy cost machinery)
**Prerequisite for:** on-device (Android/iOS) routing

---

## 0. Why this document exists

The obvious next step after "the solver works" is "ship the solver on the
phone". That step is a trap if taken first. An FFI boundary is a contract
you cannot cheaply change once Kotlin and Swift call sites exist, and the
current API surface — `Pathfinder` + `Prefs` — is not a contract you want
to freeze. It is a god object with a grab-bag options struct, both of which
encode assumptions (Norwegian artifacts, a single CRS, one hardcoded
algorithm branch, one DEM implementation) that the on-device use case
immediately violates.

This document maps what the architecture actually is, names the specific
couplings that would leak through an FFI boundary drawn today, and proposes
a port/adapter decomposition that makes four things cheap:

1. implementing and swapping **algorithms**
2. swapping **DEM sources and resolutions**
3. adding **data sources** (lakes, marshes, avalanche, snow, …)
4. **packaging a region** and running it offline, on-device

…plus a fifth that falls out of the same seams: a **debug frontend** that
works against the server *and* against a solve captured on a phone.

Everything below is grounded in the code as it stands at `36d2038`.

---

## 1. The architecture as it actually is

### 1.1 Crate graph

```
                 turbo-tiles-bin  (routing_setup.rs — the composition root)
                        │
                 turbo-tiles-api  (axum handlers, SSE, crash_dump)
                        │
              turbo-tiles-pathfind  (13.4k LOC with fmm; the engine)
                 ┌──────┴────────────────────────────┐
        turbo-tiles-fmm                    primitives:
        (eikonal solvers,           elev / mask / graph / vector / geom
         generic over traits)              (all mmap'd artifacts)
                                                  │
                                        turbo-tiles-artifacts (formats)
```

`turbo-tiles-pathfind` internals:

| Module | LOC | Role |
|---|---|---|
| `pathfinder.rs` | 1986 | God object: owns everything, dispatches, stitches, caches |
| `native_contributors.rs` | 1931 | The 18 production cost contributors |
| `unified.rs` | 778 | Unified mesh ∪ trail A\* (the default router) |
| `fmm_adapter.rs` | 1037 | Bridges contributors + DEM to `turbo-tiles-fmm` |
| `layers.rs` + `cost.rs` | 1153 | **Legacy** multiplicative `CostLayer` generation |
| `vector_layers.rs` | 567 | Legacy vector layers (paired with native contributors) |
| `config.rs` | 515 | `CostConfig` + presets, from embedded TOML |
| `solver_trace.rs` + `tracer.rs` | 723 | Recording + phase timing |
| `cost_field.rs` | 191 | `LazyCostField` — the one genuinely good seam |

### 1.2 What is already well-designed

Credit where due, because the target design should preserve these rather
than replace them:

- **`turbo-tiles-fmm` is the model to copy.** It has zero dependency on
  `pathfind` and is generic over traits — `Metric`, `Elevation`,
  `CellOverlay` — with monomorphized solvers (`solve_2d_with_metric<M:
  Metric>`). It is testable in isolation (`tests/eikonal_isotropic.rs`,
  `aniso_geodesic.rs`, `elastica_switchback.rs`) with no artifacts. This is
  exactly the shape the rest of the engine needs and does not have.

- **`CostContributor` is the right cost abstraction.** Physical units
  (walk-seconds), additive composition, explicit `veto()` separate from
  cost, a separate multiplicative `pace_factor()` channel for effects that
  scale rather than add. This solved the multiplicative-coupling calibration
  problem and should be kept verbatim.

- **`LazyCostField`** (`cost_field.rs`) is the one place both routers agree
  on "evaluate the stack at a cell, memoise, answer O(1)". It already
  implements `fmm::CellOverlay` *and* serves the unified A\* directly. It is
  the correct hot-path seam and the natural place to hang cost attribution.

- **`EdgeElevProbe`** — sharing one elevation sample set across the
  slope-family contributors removed ~93% of DEM work. The idea is right.

- **The recording infrastructure** (thread-local `Recorder`, phase frames,
  capacity-bounded decimation, SSE streaming) is genuinely good plumbing.

### 1.3 The couplings that block everything

#### C1 — Two cost-model generations are both alive

Every production layer is registered **twice**, via `push_with_native`:
once as a legacy multiplicative `Arc<dyn CostLayer>` and once as an additive
`Arc<dyn CostContributor>`. `Pathfinder` carries both vectors
(`pathfinder.rs:598` and `:606`). The unification plan's P4 ("delete
legacy") is still **OPEN**.

Consequence: **adding a marsh layer costs double.** You write a
contributor, a legacy layer, and a paired registration, and you keep them
semantically in lockstep by hand. The legacy half survives only to serve
the inspect endpoint and a build-time refusal sampler — it is a debug
dependency that taxes every future data source.

This is the single highest-leverage cleanup in the codebase and it must
happen **before** ports, not after: otherwise every port is designed twice.

#### C2 — Contributors depend on concrete artifacts, not on capabilities

```rust
pub struct ToblerSlopeContributor   { pub dem: Arc<Dem>, … }
pub struct NaismithGainContributor  { pub dem: Arc<Dem>, … }
pub struct MaskRefusalContributor   { pub mask: Arc<Mask>, … }
pub struct PolygonIntegralContributor { pub collection: Arc<VectorCollection>, … }
pub struct TrailProximityContributor { /* built from &Graph */ }
```

There is no `trait ElevationSource`, no `trait FieldSource`, no
`trait FeatureSource`, no `trait NetworkSource`. `EdgeElevProbe` itself
holds `&turbo_tiles_elev::Dem` (`contributor.rs:85`).

Consequence: **"swap the DEM source or resolution" means editing
`turbo-tiles-elev`.** There is no way to plug in a 20 m pyramid, an
in-memory fixture, a different national DEM, or a tiled fetch-on-demand
source without touching the crate every contributor imports. Same for
rasters and vectors.

Note the irony: `turbo-tiles-fmm` already defines `trait Elevation` and
`ArrayElevation` for exactly this reason. `pathfind` doesn't use it as a
port — it defines `DemElevation` as a one-off adapter around the concrete
`Dem` (`fmm_adapter.rs:33`).

#### C3 — Algorithm selection is an `if`

```rust
// pathfinder.rs, solve_inner
if prefs.force_off_trail {
    return self.solve_off_trail(from_xy, to_xy, &prefs);
}
self.solve_unified_path(from_xy, to_xy, &prefs)
```

There is no `trait Solver`. `PathStrategy { OnGraph, OffTrail, Hybrid }` is
an *output label* on the result, not a strategy object. Adding a
contraction-hierarchy router, an anisotropic-FMM variant, or a
"fast/accurate" tier means adding a branch and a `Prefs` bool, and the
choice is not expressible in configuration.

Consequence: **"easily implement and swap algorithms" is currently false.**

#### C4 — The composition root lives in the binary and is Norway-shaped

`turbo-tiles-bin/src/routing_setup.rs` (412 LOC) hardcodes:

- filenames — `norway.dem`, `norway.vectors`, `norway.forest.mask`, …
- Norwegian domain semantics — `cultivated`/*innmark*, N50 class names,
  `fkb_type`, DNT `marking`
- **cost constants inline in wiring code**:
  `WATER_CROSS_PENALTY_PER_M = 400.0`, wetland `× 1.5`, cultivated `× 3.0`,
  streams `10.0 + 5.0 × width_m`, landcover multipliers in a `&[(…)]` literal

Those numbers are not in `cost-config.toml`. They are not overridable per
request, not visible to `/debug/cost-config`, and not portable. And because
this file lives in the **bin** crate, any other host — a device, a test, a
CLI — must either depend on the binary crate or duplicate the wiring.

Consequence: **the solver is not decoupled from Norwegian data; the
decoupling boundary simply doesn't exist yet.** It is one file, which is
good news — but it is the wrong file in the wrong crate.

#### C5 — Projection and CRS are ambient global assumptions

`wgs84_to_utm33n` is defined in **`turbo-tiles-elev`** (`dem.rs:412`) and
re-exported through `pathfind`. EPSG:25833 is baked into `geom::Point`'s
contract, `GridShape`'s docs, every artifact format, and 44 references
inside `pathfind` alone. There is no `Projection` port.

Consequence: a second region (or a test fixture in a synthetic local frame)
requires touching the elevation primitive.

#### C6 — `Prefs` is not a contract

`Prefs` mixes four unrelated concerns in one flat serde struct: routing
intent (`profile`, `avoid`, `round_trip`), algorithm knobs (`mesh_cell_m`,
`max_off_trail_km`, `mesh_pad_m`, `force_off_trail`), calibration overrides
(`layer_weights`, `cost_config_override`), and debug switches (`debug`,
`record`, `record_cap`).

**This is the type that would become the FFI contract if we ported today.**
It would freeze the algorithm knobs of one specific solver into a
cross-language ABI.

#### C7 — Config reaches outside its crate

```rust
pub const EMBEDDED_DEFAULTS: &str = include_str!("../../../tools/cost-config.toml");
```

`turbo-tiles-pathfind` reads three directories up, out of the crate, into
`apps/tileserver/tools/`. The crate cannot be built or vendored standalone —
a hard blocker for packaging it into an FFI cdylib or a device build.

#### C8 — The safety net can't run in CI

`tests/scenarios.rs` is the routing corpus, and it works by **POSTing to a
running tileserver** because the artifacts are "8+ GB across DEM, vectors,
mask, graph". It is skip-on-unreachable, so a clean checkout silently
passes with zero scenarios executed.

Consequence: the one harness that would make a large refactor safe requires
a developer workstation with the full national dataset. This is *also* the
harness needed to prove server/device parity. It must be fixable, and the
port design is what fixes it (an in-memory / small-fixture adapter).

#### C9 — The debug surface is HTTP-only and speaks a dead vocabulary

`SolverEvent` is Theta\*-shaped: `LineOfSightCast { blocked }`,
`EdgeRelaxed { took_los }`, `NodePopped`. Theta\* has been replaced by FMM
and the unified A\*. **The recorder cannot express what an FMM solve
actually does** — there is no way to emit the arrival-time field, which is
the entire object of interest for the default off-trail solver.

And every debug capability (`/debug/pathfind/inspect`, `/debug/cell`,
`/debug/cost-breakdown`, `/debug/data/*`, the SSE replay) is an axum route.
On-device, all of it vanishes — precisely where you would most want it.

---

## 2. Where the FFI boundary belongs

This is the question that motivated the analysis, so it gets a direct
answer before the design.

### 2.1 The trap: FFI at the port level

It is tempting to let the host supply data — "Kotlin owns the DEM, Rust
calls back for elevation". **Do not.** A typical solve touches ~50 k cells;
after the `EdgeElevProbe` and `LazyCostField` memoisation that is still
~50–250 k elevation lookups. At ~100 ns per JNA round trip that is 5–25 ms
of pure marshalling overhead in the best case, and it destroys the memo
locality that took the DEM work down by 93% in the first place.

**Ports are internal Rust traits. They must never become FFI boundaries.**

### 2.2 The right boundary: a coarse engine façade

```
┌─────────────────── Kotlin / Swift / JS ───────────────────┐
│  RouteEngine.open(configJson, packDir) -> Engine          │  1 call / app start
│  engine.plan(requestJson) -> RouteJson                    │  1 call / route
│  engine.planObserved(requestJson, observerCallback)       │  1 call + N frames
│  engine.coverage() -> BboxJson                            │  cheap
│  engine.close()                                           │
└───────────────────────────────────────────────────────────┘
                              │  uniffi
┌─────────────────────────────┴─────────────────────────────┐
│                     turbo-route-engine                    │
│    SourceRegistry · CostModel · SolverRegistry · Observer │
└───────────────────────────────────────────────────────────┘
```

Three properties make this the right line:

1. **Call frequency is O(routes), not O(cells).** Marshalling cost is
   irrelevant.
2. **Configuration crosses as an opaque string.** Adding a marsh source,
   swapping the DEM to 20 m, or selecting a different algorithm is an
   `EngineConfig` change — **no FFI signature change, no Kotlin/Swift
   regeneration, no app release coupling.** This is the whole payoff of
   doing the refactor first.
3. **The only per-frame callback is the observer**, whose event rate is
   already bounded and decimated by design.

### 2.3 The types that cross

Replace `Prefs` with three purpose-built types (C6):

| Type | Contents | Lifetime |
|---|---|---|
| `EngineConfig` | source wiring, algorithm selection, budgets, projection, cost-config path | per engine (rare) |
| `RouteRequest` | points, profile, preset, avoid geometry, round_trip | per route |
| `ObserveSpec` | what to record, event cap, whether to emit field snapshots | per route, optional |
| `Route` | geometry, legs, distance, ascent, surface breakdown, refusals | the result |

`Route` is the existing `/v1/route/plan` response shape, which is already
described as "deliberately narrow and decoupled from the internal `Path` /
debug surface" (`route_plan.rs:1`). **That contract is already correct —
adopt it verbatim as the FFI result type.** The internal `Path` with its
`debug`/`recording` fields stays internal.

---

## 3. Target architecture

### 3.1 Crate decomposition

```
turbo-route-core        ports + domain types + units. NO I/O, NO artifacts.
  ├ trait ElevationSource, FieldSource, FeatureSource, NetworkSource
  ├ trait Projection
  ├ Bbox, PlanarPoint, GeoPoint, WalkSeconds
  └ RouteRequest, Route, Leg, RouteError

turbo-route-cost        cost model, generic over core ports. NO Norway.
  ├ trait CostContributor  (moved verbatim from pathfind)
  ├ CostModel, CostSpec (declarative), ContributorRegistry
  └ generic contributors: slope, gain, contour, roughness, coverage,
    raster-class, polygon-integral, polygon-refusal, line-crossing,
    point-proximity, network-attribute, proximity-bonus

turbo-route-solvers     trait Solver + implementations.
  ├ UnifiedAStar     (from unified.rs)
  ├ FmmGradeLimited  (from fmm_adapter.rs, wraps turbo-tiles-fmm)
  ├ GraphDijkstra
  └ CostField (from cost_field.rs) — shared hot-path memo

turbo-route-observe     trait Observer + solver-agnostic event vocabulary.

turbo-route-engine      composition root + orchestration. THE FFI façade.
  ├ SourceRegistry, SolverRegistry
  ├ EngineConfig → Engine  (declarative assembly)
  └ leg splitting, round-trip, snapping, coverage checks, leg cache

adapters (one crate each, depend on core only):
  turbo-geodata-artifacts   current mmap'd DEM/mask/graph/vector
  turbo-geodata-pack        offline region packs
  turbo-geodata-memory      in-memory fixtures for tests + CI corpus

turbo-profile-no        EVERYTHING Norwegian: filenames, N50/FKB class
                        maps, cost TOML, presets, UTM33N choice.
                        Mostly data + a registration function.
```

**The dependency rule, enforced by a CI grep:** no crate at or below
`turbo-route-engine` may contain the strings `norway`, `n50`, `fkb`, or
`25833`.

### 3.2 The ports

Sketches, not final signatures. All planar coordinates; the `Projection`
port owns the geographic↔planar conversion.

```rust
/// C2 + "swap DEM sources and resolution"
pub trait ElevationSource: Send + Sync {
    fn sample(&self, p: PlanarPoint) -> Option<f32>;
    fn coverage(&self) -> Bbox;
    fn resolution_m(&self) -> f32;
    /// Optional bulk path: adapters that can serve a whole corridor
    /// from one tile decode override this. Default = loop over `sample`.
    fn sample_many(&self, pts: &[PlanarPoint], out: &mut [Option<f32>]) { … }
}

/// Categorical or scalar raster: landcover, avalanche class, snow depth.
pub trait FieldSource: Send + Sync {
    fn name(&self) -> &str;
    fn class_at(&self, p: PlanarPoint) -> u8;
    fn value_at(&self, p: PlanarPoint) -> Option<f32> { None }
    fn coverage(&self) -> Bbox;
}

/// Vector features: lakes, marshes, streams, buildings, cliffs, cabins.
pub trait FeatureSource: Send + Sync {
    fn name(&self) -> &str;
    fn geom_kind(&self) -> GeomKind;
    fn query(&self, aabb: Bbox, f: &mut dyn FnMut(FeatureRef<'_>));
}

/// The trail/road network.
pub trait NetworkSource: Send + Sync {
    fn snap(&self, p: PlanarPoint, radius_m: f32) -> Option<NodeId>;
    fn node(&self, id: NodeId) -> Option<PlanarPoint>;
    fn edges_in(&self, aabb: Bbox, f: &mut dyn FnMut(EdgeRef<'_>));
    fn edge_geometry(&self, id: EdgeId) -> &[PlanarPoint];
}

/// C5 — lifts UTM33N out of turbo-tiles-elev.
pub trait Projection: Send + Sync {
    fn to_planar(&self, lon: f64, lat: f64) -> PlanarPoint;
    fn to_geographic(&self, p: PlanarPoint) -> (f64, f64);
    fn epsg(&self) -> u32;
}

/// C3 — algorithms become swappable objects.
pub trait Solver: Send + Sync {
    fn name(&self) -> &'static str;
    fn solve(&self, req: &SolveLeg, ctx: &SolveContext<'_>) -> Result<Candidate, SolveError>;
}
```

`SolveContext` bundles what every solver needs: the `CostModel`, the
`ElevationSource`, the `NetworkSource`, the `Projection`, the budget, and
the `Observer`. It is the *one* struct that gets threaded, replacing the
current pattern of each solver reaching into `Pathfinder`'s fields.

### 3.3 Keeping it fast

A port design that costs 20% throughput is not worth having. Three rules:

1. **`dyn` at assembly, generics in the hot loop.** The registry stores
   `Arc<dyn ElevationSource>`; `CostField` and the solvers are generic
   (`struct CostField<E: ElevationSource>`) and monomorphize for the
   configured combination. `turbo-tiles-fmm` already proves this works
   (`solve_2d_with_metric<M: Metric>`).
2. **`CostField` stays the single memo.** Per-cell contributor evaluation,
   elevation memo, and refusal state remain in one place. The `dyn`
   indirection is paid ~once per cell, not once per sample.
3. **Gate every step on the corpus.** The routing dev loop already gates on
   geometry hash, quality, determinism, and DEM-work axes
   (`tools/ROUTING_DEV_LOOP.md`). No step lands with a geometry-hash
   change that isn't explained.

Expected cost: <2% on the DEM-work axis. If a step exceeds that, it is a
bug in the step, not in the design — and the loop will say so.

### 3.4 What this buys, concretely

| Goal | Today | After |
|---|---|---|
| Add a marsh source | New contributor + legacy layer + paired wiring in the bin crate + a new artifact filename | One `FeatureSource` adapter row in the pack manifest + one `CostSpec` entry in TOML. **Zero code** if it reuses `polygon-integral`. |
| DEM at 20 m | Rebuild `norway.dem` | `EngineConfig.elevation.resolution = 20`, or a `PyramidElevation` adapter picking level by `cell_m` |
| New algorithm | Branch in `solve_inner` + a `Prefs` bool | `impl Solver`, register, select by name in config or request |
| Second country | Not possible without editing `turbo-tiles-elev` | New `turbo-profile-xx` crate |
| Run corpus in CI | Needs 8 GB artifacts + live server | `turbo-geodata-memory` fixture, in-process |
| Ship on device | Depends on `turbo-tiles-bin` for wiring | `turbo-route-engine` + `turbo-geodata-pack`, no server crates |

---

## 4. Region packs — "pull a local copy"

Define the pack in terms of **ports**, not current artifacts. A pack is a
directory (or single container) with a manifest:

```toml
[pack]
format_version = 1
profile        = "no"
bbox           = [8.4, 60.1, 9.9, 61.0]
epsg           = 25833
created        = "2026-07-28"
engine_min     = "0.4.0"

[[source]]
port = "elevation"; kind = "dem-tiles"; file = "dem.10m"; resolution_m = 10
[[source]]
port = "elevation"; kind = "dem-tiles"; file = "dem.40m"; resolution_m = 40   # fringe level
[[source]]
port = "network";   kind = "csr-graph"; file = "network"
[[source]]
port = "features";  name = "water";   kind = "vectors"; file = "vectors"
[[source]]
port = "features";  name = "marsh";   kind = "vectors"; file = "vectors"
[[source]]
port = "field";     name = "forest";  kind = "mask";    file = "forest.mask"

[cost]
config  = "cost-config.toml"   # the pack carries its own calibration
presets = "presets.toml"
```

Three consequences worth stating explicitly:

- **The pack carries its cost config and presets.** Same pack version on
  server and device ⇒ identical geometry. That is a testable parity
  assertion, not a hope: run the corpus against the pack in-process, and
  against the same pack on-device, and diff geometry hashes.
- **Multi-resolution is a pack concern, not a format concern.** Shipping
  10 m near the route and 40 m in the corridor fringe is a `PyramidElevation`
  adapter over two sources — the single biggest lever on pack size
  (see the on-device sizing analysis: ~85 MB → ~25 MB for a 50×50 km region).
- **The CLI is one command.** `turbo-route pack --bbox … --profile no
  --dem-res 10,40 --out marka.pack`, implemented by slicing the national
  artifacts (DEM tile filter is a verbatim payload copy; graph needs CSR
  renumbering with a halo).

---

## 5. Debugging frontend

### 5.1 What exists

`apps/admin/src/screens/PlotRoute.tsx` — **2883 LOC**, backed by
`/v1/pathfind/{record,stream}`, `/v1/debug/pathfind/{layers,inspect,cell}`,
`/v1/debug/cost-breakdown`, `/v1/debug/data/*`. It replays a solve as an
animated exploration trail. This is a real asset and should not be thrown
away — but it has four structural problems.

### 5.2 The four problems and their fixes

**P1 — The event vocabulary describes a solver we deleted (C9).**
`LineOfSightCast`, `took_los`, `NodePopped` are Theta\* concepts. The
default off-trail solver is FMM, whose interesting state is a *field*, not a
node sequence. Replace with a solver-agnostic vocabulary:

```rust
enum SolverEvent {
    GridSized     { shape: GridShape },
    CellSettled   { i: u32, j: u32, arrival_s: f32 },   // FMM + Dijkstra + A*
    FrontierState { cells: Vec<(u32, u32)> },           // decimated snapshot
    FieldSnapshot { bbox: Bbox, cell_m: f64, values: Vec<f32> },  // FMM's whole point
    CellVetoed    { i: u32, j: u32, layer: &'static str },
    CellAttributed{ i: u32, j: u32, parts: Vec<NamedContribution> },
    CandidateImproved { geometry: Vec<PlanarPoint>, cost_s: f64 },
    Phase         { name: &'static str, at_us: u64 },
}
```

**P2 — Debug is HTTP-only, so it can't see the on-device build.**
Make `Observer` a port with three adapters: SSE (server), NDJSON-to-file
(device + CI), and in-memory (tests). A recording captured on a phone then
loads into the same frontend. This is the only way to debug a device-only
divergence, and it costs almost nothing once the port exists.

**P3 — The highest-value view is missing.** `/debug/cost-breakdown` explains
*one edge*. What you actually need when calibrating is: *render the corridor,
colour every cell by contributor X's walk-seconds, click a cell for the full
breakdown.* `LazyCostField::ensure()` already computes exactly this
per-cell attribution — and discards everything but the composed multiplier.
Emitting it under `ObserveSpec` is a ~50-line change and is, by a wide
margin, the cheapest large debugging win available.

**P4 — It's a screen inside the admin SPA.** Extract to a standalone
`apps/route-lab` that accepts either a live engine URL or a dropped
recording file. Then it works against the server, against a device capture,
and against a CI corpus failure artifact. Add two views the current screen
lacks:

- **A/B diff** — two configs or two algorithms on one request; geometry
  overlay + per-contributor cost delta. This is what makes "swap
  algorithms" and "retune the marsh penalty" actually usable.
- **Cost attribution heatmap** — per contributor, per cell, from P3.

---

## 6. Sequencing

Ordered by dependency. Every step is corpus-gated and independently
revertable. **FFI is last, deliberately.**

| # | Step | Why it's here | Effort |
|---|---|---|---|
| 1 | **Fix the safety net** (C8). `turbo-geodata-memory` + a ~100 MB fixture region; make `tests/scenarios.rs` run in-process in CI. | Nothing else is safe without this. Also the future device-parity harness. | 4–6 d |
| 2 | **Delete the legacy `CostLayer` generation** (C1). Port the inspect endpoint and the build-time refusal sampler onto contributors; drop `layers.rs`, `cost.rs`, `vector_layers.rs`, `LegacyLayerAdapter`. | Halves the surface every later step touches. Already planned as P4. | 1–1.5 wk |
| 3 | **Extract `turbo-route-core` ports** (C2, C5). Traits + `Projection`; `turbo-geodata-artifacts` implements them over today's primitives; contributors and `EdgeElevProbe` become generic. Move `wgs84_to_utm33n` out of `turbo-tiles-elev`. | The load-bearing step. | 2–3 wk |
| 4 | **`trait Solver` + registry** (C3). `UnifiedAStar`, `FmmGradeLimited`, `GraphDijkstra` as implementations; selection by name in config. | Delivers "swap algorithms". | 1 wk |
| 5 | **`turbo-route-engine` + `turbo-profile-no`** (C4, C6, C7). Move `routing_setup.rs` out of the bin crate, make the inline constants config, split `Prefs` into `EngineConfig`/`RouteRequest`/`ObserveSpec`, fix the `include_str!` escape. | Delivers the decoupling and the future FFI contract. | 1.5–2 wk |
| 6 | **`Observer` port + new event vocabulary** (C9, P1–P2). | Unblocks device debugging and the frontend rework. | 1 wk |
| 7 | **Region packs** — `turbo-geodata-pack` + `turbo-route pack` CLI + pyramid elevation. | "Pull a local copy and package it." | 2–3 wk |
| 8 | **`apps/route-lab`** — extract, cost attribution heatmap, A/B diff, recording-file loading. | Debug frontend. Can run in parallel from step 6. | 2 wk |
| 9 | **FFI + Android** — uniffi over `RouteEngine`, cargo-ndk (pattern already proven by `turbomap-ffi` / `core/turbomap-android`), `catch_unwind` at the boundary, `max_cells` budget. | Now the contract is worth freezing. | 1.5–2 wk |
| 10 | **iOS** — uniffi Swift + xcframework (greenfield build glue; crate unchanged). | | 1 wk |

**Steps 1–6: ~7–9 weeks** — the engine is modular, testable in CI, and
Norway-agnostic, with the server unchanged in behaviour.
**Steps 7–10: ~6–8 weeks** — packs, lab, device.

Compared with porting first, this is roughly 5–6 weeks of additional
up-front work that buys: an FFI contract that survives adding data sources
and algorithms, a corpus that runs in CI, a debug story that works on
device, and a pack format defined by capabilities rather than by whatever
`norway.*` files happen to exist.

### What to skip

- **Don't make the ports async.** Every source is mmap or in-memory. Async
  buys nothing and poisons the whole tree for FFI.
- **Don't build a dylib plugin system.** Compile-time registration in
  `turbo-profile-*` is sufficient and keeps monomorphization.
- **Don't build reprojection machinery.** `Projection` as a port with one
  implementation is the goal — the point is that nothing *hardcodes* the
  CRS, not that we support arbitrary ones today.
- **Don't preserve `PathStrategy`'s `OnGraph`/`Hybrid` variants** as
  strategy inputs. They are historical output labels from the pre-unified
  candidate race.

---

## 7. If only three things get done

1. **Delete the legacy cost generation (step 2).** Every other item is
   cheaper afterwards, and it is already an accepted open plan item.
2. **Extract the ports, especially `ElevationSource` (step 3).** This is
   what "swap DEM sources and resolution" and "add marsh data" both reduce
   to, and it is what makes the pack format definable.
3. **Move the composition root out of the bin crate into
   `turbo-route-engine` + `turbo-profile-no` (step 5).** This is the
   decoupling from Norwegian data, and it produces the FFI façade as a
   by-product — at which point the Android port is mechanical.
