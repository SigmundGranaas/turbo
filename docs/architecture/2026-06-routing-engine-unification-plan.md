# Routing Engine Unification & Performance Architecture

**Status:** proposal / finalized design
**Date:** 2026-06-09
**Scope:** `turbo-tiles-pathfind`, `turbo-tiles-fmm`, `turbo-tiles-elev`, `turbo-tiles-bin` (HTTP routing flow)
**Goal:** one composable solve pipeline that is fast and bounded on slow / memory-constrained hardware, reached by *better software design* rather than scattered micro-optimizations.

---

## 1. Why this exists

The live foot router is `unified::solve_unified` (`turbo-tiles-pathfind/src/unified.rs`): one weighted-A\* over a per-request corridor mesh ∪ trail graph. It works and quality is good. But a performance/memory profile found that **every top bottleneck is one architectural fault wearing different clothes** — the search loop reaches all the way down the stack on each edge relaxation:

```
A* relax → mesh_step → MeshOverlay::ensure → contributor stack
         → EdgeContext (alloc) → dem.sample → rstar query → cache mutex
```

Three concerns that must be independent are **fused**:

| Concern | Where it lives today | Problem |
|---|---|---|
| **Data access** (DEM, mask, graph polylines) | pulled point-by-point inside `mesh_step` (`unified.rs:551-552`) | random-access DEM, re-sampled ~16× per cell; `sample()` itself re-runs `find_tile` ~5× (`dem.rs:316`) |
| **Cost model** (Tobler, gain, surface, refusal) | `compose_edge_walk_seconds` called per edge via `EdgeContext` | per-edge allocation + RefCell churn; contributor stack cloned per request (`pathfinder.rs:1412`) |
| **Search** (A\*) | `solve_unified`, dense `g/prev/prev_seg` over `n_total` | no ownership boundary → can't pool buffers; `BinaryHeap` has no stale-pop guard |

Bottleneck → root-cause mapping:

- **DEM sampled ~16-80×/cell** → search does *random-access* data reads instead of the field being *materialized once*.
- **`edge_polyline().to_vec()` churn** (`unified.rs:427,754`, `graph/lib.rs:459`) → search re-derives *data* mid-loop.
- **dense `mul/state/g/prev/prev_seg` per request** → no boundary that could *pool/reuse* buffers.
- **no concurrency cap + dead 512 MB DEM cache knob** (`dem.rs:29`, `main.rs:191`) → no *service layer* owning bounded resources.

**The fix already exists in this repo.** `turbo-tiles-fmm` is built the right way: `GridShape` (discretization), `Metric` / `Elevation` / `CellOverlay` (data seams), and a clean `bake_* → solve_* → extract_*` pipeline — with **zero dependency on pathfind** (coupling goes one way). `unified.rs` regressed from this pattern: it reinvents `Corridor` (a duplicate `GridShape`), fuses lazy cost eval into A\*, and samples the DEM in the inner loop.

> **This plan is "rebuild `unified` on the seams the FMM crate already proves, then add a resource-owning service layer."** Not a rewrite — a strangler that re-converges the two solvers on one contract.

---

## 2. The unified model — one pipeline, four seams

```
                 ┌──────────────────────────────────────────────┐
                 │            RoutingExecutor (service)           │
                 │  owns: Graph(mmap) · Dem(cache) · Mask(mmap)   │
                 │        FieldPool · Semaphore(≈cores)           │
                 └───────────────┬──────────────────────────────┘
                                 │ checkout scratch, admit request
   build phase                  ▼                       solve phase            extract
 ┌───────────┐   ┌──────────────────────────┐   ┌──────────────────┐   ┌──────────────┐
 │  RouteGrid│──▶│  CostField  (data seam)   │──▶│  Solver           │──▶│ extract_path │
 │ (=GridShape)  │  EagerField | LazyField   │   │ A* | FMM (pluggable)  │  (shared)    │
 └───────────┘   │  + StepMetric (cost seam) │   │ over field+metric │   └──────────────┘
                 │  + TrailGraph (CSR)       │   │ + pooled scratch  │
                 └──────────────────────────┘   └──────────────────┘
```

Four seams, each a clean contract:

1. **`RouteGrid`** — the *only* cell↔world mapping. Re-use `turbo_tiles_fmm::GridShape`. Delete `unified::Corridor`'s bespoke arithmetic.
2. **`CostField`** — the data contract the solver reads (`elevation`, `pace_mul`, `is_refused` per cell). Two implementations behind one trait: `EagerField` (SoA, tile-streamed, the default) and `LazyField` (today's memoized `MeshOverlay`, the escape hatch for unbounded corridors).
3. **`StepMetric`** — the *pure* directional cost function `(field, from, to) → walk_seconds`. The contributor stack is **compiled once** into the field's per-cell scalars + a tiny `StepParams`. Tobler/steep/gain are the only arithmetic left in the inner loop.
4. **`Solver`** — pure over `(grid, field, metric, trail, &mut Scratch)`. Knows nothing about DEM tiles, rstar, contributors, or Arcs. A\* and FMM become interchangeable strategies over the same contract.

Owned above by a **`RoutingExecutor`**: shared read-only data + a `FieldPool` (reused buffers → steady-state per-request allocation ≈ 0) + a bounded `Semaphore` (concurrency = a property of the service, not an accident of the 512-thread blocking pool).

---

## 3. Concrete trait & struct surfaces

### 3.1 Grid (seam 1) — reuse, don't reinvent

`unified::Corridor` is deleted; its constructor becomes a free function returning the canonical shape:

```rust
// turbo-tiles-pathfind/src/unified/grid.rs
use turbo_tiles_fmm::GridShape;

/// Oriented, padded, O(d)-bounded corridor for a from→to route.
/// Returns the canonical GridShape (origin/nx/ny/cell_m) — the SAME
/// type the FMM solver uses, so cell↔world math has one definition.
pub fn corridor_shape(from: PointXY, to: PointXY, cell_m: f64) -> Option<GridShape>;
```

All `cell_centre` / `world_to_cell` calls route through `GridShape` (already implemented and tested, `grid.rs:91-113`). Net: −1 duplicate mapping, −1 source of off-by-half bugs.

### 3.2 CostField (seam 2) — the data contract

The FMM crate **already** owns `Elevation` and `CellOverlay` traits. Promote them to *the* canonical field contract; pathfind implements it. (Dependency direction stays pathfind → fmm.)

```rust
// turbo-tiles-fmm/src/field.rs  (extends the existing Elevation/CellOverlay)
/// Everything a solver needs to price one cell. Pure data — no I/O,
/// no contributor objects, no locks. Indexed by flat cell id.
pub trait CostField: Send + Sync {
    fn grid(&self) -> &GridShape;
    fn elevation(&self, cell: u32) -> Option<f32>;   // None = no DEM coverage
    fn pace_mul(&self, cell: u32) -> f32;            // composed contributor scalar
    fn is_refused(&self, cell: u32) -> bool;
}
```

**`EagerField` — the default, SoA, built in one tile-ordered sweep:**

```rust
// turbo-tiles-pathfind/src/unified/field.rs
pub struct EagerField {
    grid: GridShape,
    elevation: Vec<f32>,   // f32::NAN = no coverage
    pace_mul: Vec<f32>,    // clamped [0.1, 20.0]
    refused: bitvec::BitVec,  // 1 bit/cell instead of a u8
}

impl EagerField {
    /// BUILD PHASE. One cache-coherent pass:
    ///   1. batch-sample elevation via Dem::profile (already exists!)
    ///      — each DEM tile decompressed once, read sequentially.
    ///   2. compose the contributor stack per cell ONCE → pace_mul + refused.
    /// Buffers come from the FieldPool (reused across requests).
    pub fn build(
        grid: GridShape,
        dem: &Dem,
        contributors: &[Arc<dyn CostContributor>],
        profile: Profile,
        base_pace: f32,
        buffers: FieldBuffers,   // checked out from the pool
    ) -> Self;
}
impl CostField for EagerField { /* O(1) slice reads */ }
```

This single change dissolves the #1 CPU bottleneck: **`Dem::profile(&[PointXY])` already batches** (`dem.rs:245`); sweeping cells in row-major (= tile) order touches each DEM tile once instead of ~80 random rstar lookups per cell. The contributor stack runs once/cell (not once/edge × 16).

**`LazyField`** wraps today's `MeshOverlay` memo behind the same trait — kept for the rare unbounded-corridor case, chosen by policy in the executor (`cell_count > THRESHOLD`). The solver never knows which it got.

### 3.3 StepMetric (seam 3) — the pure cost function

The contributor stack is fixed per request, so **compile it once**, not per edge:

```rust
// turbo-tiles-pathfind/src/unified/metric.rs
/// Per-request parameters distilled from the contributor stack +
/// profile. Copy, no allocation, no Arc, no RefCell.
#[derive(Clone, Copy)]
pub struct StepParams {
    pub base_pace_s_per_m: f32,
    pub mesh_max_grade_deg: f32,
    pub mesh_gain_k: f32,
    pub cliff_deg: f32,
    pub steep_penalty_k: f32,
}

/// Directional cost of stepping from cell a→b, in walk-seconds.
/// PURE: reads only the field + params. SIMD-friendly, unit-testable
/// against a synthetic field with no DEM/DB. None = impassable.
#[inline]
pub fn step_cost(field: &dyn CostField, p: &StepParams, a: u32, b: u32) -> Option<f32>;
```

`tobler_pace`, the steep penalty, and Naismith gain (currently inlined in `mesh_step`, `unified.rs:523-579`) move here verbatim — but read `field.elevation(a)/elevation(b)` and `field.pace_mul(b)` instead of calling `dem.sample` and `overlay.pace_mul`. Same numbers, zero I/O in the hot loop. (This mirrors `turbo-tiles-fmm`'s existing `Metric`/`GradeLimitedCost`; the off-trail FMM path already does exactly this.)

### 3.4 TrailGraph — extract the splice, make it CSR

The trail subgraph build (`unified.rs:401-493`) becomes its own type, with **CSR adjacency instead of `Vec<Vec<_>>`** (kills one heap alloc per node) and `edge_polyline` returning a **slice, not `.to_vec()`** (`graph/lib.rs:459` — the largest allocation-*count* source):

```rust
// turbo-tiles-pathfind/src/unified/trail.rs
pub struct TrailGraph {
    pos: Vec<(f64, f64)>,
    adj_offsets: Vec<u32>,            // CSR
    adj_edges: Vec<TrailEdge>,        // (to_node, cost_s, eid, k_from, k_to)
    cell_trails: Vec<(u32, u32)>,     // sorted (cell, node) — binary search, no HashMap
}
impl TrailGraph {
    pub fn build(graph: &Graph, grid: &GridShape, contributors: &[Arc<dyn CostContributor>],
                 profile: Profile, from: PointXY, to: PointXY) -> Self;
    pub fn neighbors(&self, node: u32) -> &[TrailEdge];
    pub fn trails_in_cell(&self, cell: u32) -> &[u32];
}
```

```rust
// turbo-tiles-graph/src/lib.rs — return a borrow; only the fallback owns.
pub fn edge_polyline(&self, eid: u32) -> &[NodePos];  // was -> Vec<NodePos>
```

### 3.5 Solver (seam 4) — pure over field + metric + pooled scratch

```rust
// turbo-tiles-pathfind/src/unified/solver.rs
/// Reusable A* working memory. Checked out from the FieldPool,
/// cleared (not realloc'd) per solve. Sized to grid.len() + trail nodes.
pub struct Scratch {
    g: Vec<f32>,
    prev: Vec<u32>,
    prev_seg: Vec<(u32, u32, u32)>,   // → sparse map; see §5 bottleneck #3
    heap: BinaryHeap<HeapItem>,
}

/// PURE search. No DEM, no contributors, no Arc. Trivially testable.
pub fn solve_unified(
    grid: &GridShape,
    field: &dyn CostField,
    metric: &StepParams,
    trail: &TrailGraph,
    from: PointXY, to: PointXY,
    scratch: &mut Scratch,
    recorder: Option<&Recorder>,   // SSE live-progress, unchanged
) -> Option<UnifiedRoute>;
```

`UnifiedRoute` stays exactly as today (`geometry_utm`, `seg_on_trail`, `seg_fkb`, `cost_s`) — **the public output contract does not change**, so `Pathfinder::solve_route` and the API layer are untouched.

The A\* loop also gains a **stale-pop guard** (`if f > g[node] + h { continue }`) — bounds heap growth, one line.

### 3.6 RoutingExecutor + FieldPool — where resource limits belong

```rust
// turbo-tiles-pathfind/src/service.rs
pub struct RoutingExecutor {
    graph: Arc<Graph>,
    dem: Arc<Dem>,                 // built via open_with_cache (wire the knob)
    mask: Arc<Mask>,
    contributors: Arc<[Arc<dyn CostContributor>]>,  // built ONCE at boot
    pool: FieldPool,               // reusable FieldBuffers + Scratch
    permits: tokio::sync::Semaphore,  // size ≈ cores → bounded concurrency
}
impl RoutingExecutor {
    pub async fn route(&self, req: RouteRequest) -> Result<Path, PathfindError> {
        let _permit = self.permits.acquire().await?;   // excess requests QUEUE
        let bufs = self.pool.checkout(req.estimated_cells());
        // ... build field, solve, extract; bufs returned on drop ...
    }
}
```

- **DEM cache**: `Dem::open_with_cache(path, env("TILESERVER_DEM_CACHE_BYTES").unwrap_or(128 MB))` — the knob exists (`dem.rs:122`) but `main.rs:191` calls `open`. One-line wire-up + a real LRU (`lru` crate) to replace the O(N) eviction (`cache.rs:98`).
- **Concurrency**: a `Semaphore(cores)` is the structural fix for "N concurrent 20 km solves each allocating a corridor with nothing bounding them" — the realistic OOM path on the 1.5 Gi prod pod.
- **FieldPool**: scratch + field buffers are checked out and returned, so steady-state allocation per request trends to zero (vs today's `vec![…; n]` × 5 per request).

---

## 4. Module / crate layout (what moves where)

```
turbo-tiles-fmm/                 (unchanged role: pure solver, zero pathfind deps)
  src/field.rs        ← NEW: promote Elevation + CellOverlay → CostField trait
  src/grid.rs           GridShape (already canonical — now used by unified too)
  src/metric.rs         Metric/NormForm (already the pattern unified copies)

turbo-tiles-pathfind/
  src/unified/
    grid.rs           ← corridor_shape() (was Corridor in unified.rs)
    field.rs          ← EagerField (SoA, tile-streamed) + LazyField (wraps MeshOverlay)
    metric.rs         ← StepParams + step_cost() (was inline mesh_step closure)
    trail.rs          ← TrailGraph (CSR; was inline trail_pos/adj/junction/cell_trails)
    solver.rs         ← solve_unified() pure search (was the body of solve_unified)
    smooth.rs         ← chaikin + smooth_off_trail (moved verbatim)
  src/service.rs      ← NEW: RoutingExecutor + FieldPool + Semaphore
  src/unified.rs        → thin re-export shim during migration, deleted at the end

turbo-tiles-graph/src/lib.rs   ← edge_polyline returns &[NodePos]
turbo-tiles-elev/src/
  dem.rs              ← sample() resolves tile once (last-tile memo); cache knob wired
  cache.rs            ← real O(1) LRU
turbo-tiles-bin/src/main.rs    ← open_with_cache; construct RoutingExecutor
```

| Current (fused) | Becomes (composed) |
|---|---|
| `unified::Corridor` | `turbo_tiles_fmm::GridShape` via `corridor_shape()` |
| `unified::MeshOverlay` (lazy, in-loop) | `EagerField` (built once) + `LazyField` (same trait) |
| `mesh_step` closure (DEM in loop) | `step_cost()` pure over `CostField` |
| inline trail splice + `Vec<Vec>` | `TrailGraph` (CSR, slice polylines) |
| `solve_unified` (13-arg free fn) | `solver::solve_unified` pure over seams |
| per-request `vec![…; n]` ×5 | `FieldPool` checkout/return |
| no concurrency limit | `RoutingExecutor` `Semaphore` |
| dead `TILESERVER_DEM_CACHE_BYTES` | wired `open_with_cache` + real LRU |

---

## 5. How each bottleneck is *structurally* eliminated

| # | Bottleneck (from profile) | Structural elimination |
|---|---|---|
| 1 | DEM sampled ~16-80×/cell; `sample()` re-runs `find_tile` 5× | `EagerField::build` does **one tile-ordered `Dem::profile` sweep** — each tile decompressed once, read sequentially. Inner loop reads a `Vec<f32>` slice. Random access becomes *impossible* (solver only sees a materialized field). |
| 2 | No concurrency cap + 512-thread blocking pool + dead 512 MB cache | `RoutingExecutor` owns a `Semaphore(cores)` and the wired `open_with_cache` knob. Concurrency + memory ceiling become *service properties*, not accidents. |
| 3 | Dense `g/prev/prev_seg` @ 25 B/cell per request | `FieldPool` reuses buffers (alloc ≈ 0 steady-state); `prev_seg` (12 B/cell, meaningful only for trail nodes) → sparse `HashMap`/side-table; `pace_mul` quantized; `refused` → 1 bit. ~25 B/cell → ~9 B/cell. |
| 4 | `edge_polyline().to_vec()` churn (100k+ allocs) | `edge_polyline` returns `&[NodePos]`; `TrailGraph` CSR removes per-node `Vec::new()`. Search can no longer re-derive data mid-loop. |
| — | `BinaryHeap` no stale guard; contributor stack cloned/req | stale-pop guard (1 line); `contributors: Arc<[…]>` built once in the executor, borrowed — not `.to_vec()`'d per request. |

None of these can regress later, because the *category* is gone: the solver cannot touch the DEM, cannot allocate per-edge, cannot exceed the permit count.

---

## 6. Migration — strangler sequence (each step ships + is validated)

Ordered so value lands early and nothing is a big-bang. Validate each against the existing corpus (memory: `terrain_metrics.py --force-off-trail`, 760 Flutter tests untouched, the routing scenario tests in `tests/scenarios.rs`).

**Phase 0 — seam extraction, zero behavior change (the keystone).**
- Add `corridor_shape()` returning `GridShape`; make `unified.rs` use it internally. Pure mechanical.
- Define `CostField` in fmm; implement it as a thin wrapper over today's `MeshOverlay` (`LazyField`). `solve_unified` reads through the trait.
- Extract `StepParams`/`step_cost`, `TrailGraph`, `Scratch` as internal types — same logic, just moved behind signatures.
- **Gate:** `cargo test` + a fixed-corridor criterion bench established as the baseline. Byte-identical routes.

**Phase 1 — the CPU win: `EagerField` + batch DEM.**
- Implement `EagerField::build` (tile-ordered `Dem::profile` + one-pass contributor compose). Flip the default; keep `LazyField` for `cell_count > THRESHOLD`.
- `edge_polyline → &[NodePos]`; `TrailGraph` CSR.
- **Gate:** bench shows the predicted DEM-sampling collapse; routes within float tolerance of Phase 0; corpus score held (`98.4`).

**Phase 2 — the memory win: pooling + sparse arrays.**
- `FieldPool` checkout/return for field + scratch buffers; `prev_seg` sparse; `pace_mul` quantized; `refused` bitset.
- **Gate:** `dhat` shows steady-state per-request allocation ≈ 0; RSS flat under repeated solves.

**Phase 3 — the resilience win: the service layer.**
- `RoutingExecutor` with `Semaphore`; wire `open_with_cache` + real LRU; bounded tokio blocking pool. HTTP handlers submit to the executor.
- **Gate:** drive K concurrent 20 km solves; RSS bounded; no OOM on a 1.5 Gi cap; p99 latency under load measured.

**Phase 4 — converge the solvers, delete legacy.**
- Re-target the FMM off-trail path and the grade-limited lifted solver to consume `CostField` + `StepMetric` (they already consume fmm's `Metric`/`Elevation`/`CellOverlay`, so this is mostly adapter deletion).
- Delete `unified::Corridor`, `MeshOverlay`, the legacy Theta\* path, and `mesh_inputs_for_bbox` (the dead eager raster). Resolves the long-standing "unify + delete legacy" item.
- **Gate:** one solve entry point; all scenario tests green; legacy modules removed.

Each phase is independently revertable (the seam is the stable interface). Phase 0 alone is worth doing even if perf work pauses — it converts every later optimization from surgery-on-a-fused-loop into a localized change.

---

## 7. Testing strategy (enabled by the seams)

The decomposition is what *makes* the engine testable:

- **`step_cost` is pure** → golden tests on walk-seconds for known slopes/grades, no DEM/DB. (Today you can't test cost without the whole stack.)
- **Solver runs on a synthetic `CostField`** → A\* correctness (shortest path, refusal handling, trail stickiness) on hand-built fields with deterministic costs.
- **`EagerField::build` vs `LazyField`** → property test: for any corridor, eager and lazy must produce identical `pace_mul`/`refused`/`elevation` per cell (catches build/lazy drift).
- **`corridor_shape`** → reuse fmm's existing `GridShape` round-trip tests.
- **Criterion benches** at the `solve_unified(seams)` boundary → every phase is *measured*, not asserted by eye. The fixed-corridor bench is the regression gate.
- **End-to-end** → `tests/scenarios.rs` + `terrain_metrics.py` corpus unchanged; the public `UnifiedRoute`/`Path` contract is invariant across all phases.

---

## 8. Config consolidation (slow-hardware as a profile)

Today's tuning knobs are scattered constants (`HEURISTIC_MIN_PACE`, `PAD_CAP_M`, `MAX_TRAIL_EDGES`, `cell_m` clamp) and a dead env var. Consolidate into one `RoutingBudget` owned by the executor — so "run on weak hardware" is **one config object**, not a fork:

```rust
pub struct RoutingBudget {
    pub max_cells: u32,        // auto-coarsen cell_m if exceeded (NEW lever)
    pub cell_m_floor: f64,     // raise 10→15 on low-end HW
    pub max_trail_edges: usize,
    pub dem_cache_bytes: usize,
    pub concurrency: usize,    // Semaphore size
    pub heuristic_min_pace: f64,
}
impl RoutingBudget { pub fn low_end() -> Self; pub fn server() -> Self; }
```

`max_cells` is the single highest-leverage new knob: it bounds memory *and* time simultaneously by coarsening `cell_m` when `nx·ny` would blow the budget — the one lever that makes any hardware safe.

---

## 9. Risks & mitigations

- **Eager pays for unvisited cells.** True, but the adaptive `cell_m` clamp already holds typical corridors at ~50 k cells (~1 MB SoA); a tile-ordered rasterization of 50 k cells is far cheaper than 50 k random rstar lookups. `LazyField` behind the same trait is the escape hatch for pathological corridors — the abstraction is what lets us keep both without a fork.
- **Refactor churn.** Mitigated by Phase 0 being byte-identical and the `UnifiedRoute`/`Path` contract never changing. Each phase reverts independently.
- **`edge_polyline → &[NodePos]` lifetime fallout.** The slice already exists in `g.vertices` (`graph/lib.rs:459`); only the synthesized fallback needs ownership — return `Cow<[NodePos]>` if a borrow can't cover every caller.
- **Drift between eager and lazy field.** Caught by the property test in §7.

---

## 10. Summary

The router doesn't need new algorithms — it needs the **build → solve → extract** separation the FMM crate already demonstrates, lifted into the live path and wrapped in a resource-owning service. Four seams (`RouteGrid`, `CostField`, `StepMetric`, `Solver`) + one executor turn every profiled bottleneck from a patch into a structural impossibility, make the engine unit-testable for the first time, and reduce per-request memory ~3× while collapsing the dominant DEM-sampling CPU cost. Start with **Phase 0** — it is cheap, reversible, and unlocks everything after it.

---

## Status (2026-06-11)

Executed via the autonomous routing dev loop (`tools/ROUTING_DEV_LOOP.md`);
every step gated on the corpus (geometry hash / quality / determinism /
DEM-work axes).

| Phase | Status | Notes |
|---|---|---|
| P0 — seam extraction | **DONE** (`a02f862`) | One `cost_field::LazyCostField` (impl `fmm::CellOverlay`) replaced both per-router overlays; `unified::Corridor` deleted in favour of the canonical `GridShape` via `corridor_shape()`. Loop-proven pure: DEM lookups unchanged to the digit on both lanes, geometry identical. |
| P1 — `EagerField` | **CLOSED, WON'T DO** | Premise invalidated by measurement. The eager tile-ordered bake was justified by scattered random DEM access (~80 lookups/cell). The `EdgeElevProbe` + per-cell memos + single-tile-resolve work cut that to ~5 lookups/cell (~96% total reduction), and the lazy field only pays for *explored* cells — an eager bake would evaluate the full corridor (2–4× more cells) for no remaining locality benefit. |
| P2 — `FieldPool` | **CLOSED, WON'T DO** | dhat profiling: per-solve heap churn is diffuse (rstar query internals, polygon math, small probe vectors — no dominant site) at ~0.5 GB/1.6 M blocks per off-trail solve, with solves at ~250 ms mean. Pooling would save allocator traffic nobody can measure end-to-end. Revisit only if solve concurrency × corridor size grows an order of magnitude. |
| P3 — executor / resource bounds | **DONE in substance** (`7b736fb`, `4a66e0d`) | Routing-solve `Semaphore` (`TILESERVER_ROUTING_CONCURRENCY`) gates all five API solve sites; inline solves moved off async workers; `TILESERVER_DEM_CACHE_BYTES` wired (and set in the k8s manifest). The full `RoutingExecutor` abstraction was not needed once these landed — `routing_setup::build_pathfinder()` is the construction seam shared by serve + eval. |
| P4 — delete legacy | **OPEN** | Theta\* remnants, `mesh_inputs_for_bbox`, the legacy multiplicative `CostLayer`/`compose_*` machinery (every production layer has a native contributor; `contributor.rs`'s own migration plan step 5). Pure maintenance-surface reduction; loop-gated whenever taken. |

Related outcomes the plan didn't anticipate: O(1) LRU for the tile cache
measured as unnecessary (entry counts are small; zstd dominates misses);
a thread-local last-tile memo was rejected for moving geometry on tile
overlaps (perf changes must stay bit-exact).
