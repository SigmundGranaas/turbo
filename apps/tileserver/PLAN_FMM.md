# Implementation Plan: Anisotropic Curvature-Penalised FMM Off-Trail Solver

**Replaces:** `turbo-tiles-pathfind/src/core/off_trail.rs` (Theta\*) for off-trail and hybrid-edge solves, gated behind `CostMode::FastMarching`.

**Reference:** Mirebeau, *JMIV 2018, "Fast-Marching Methods for Curvature-Penalized Shortest Paths"* — `HamiltonFastMarching` (HFM) C++ library. We port the subset that matters for trail-following on a 2D Riemannian/Finsler manifold with an Euler-elastica state-augmented metric.

**Architecture target:** anisotropic FMM on a regular DEM-aligned grid; Finsler norm encoding Tobler signed-slope pace × heading; state-augmentation `(x, y, θ)` for curvature; cost-aware gradient descent from goal back to start in `(x, y)` after marginalising over θ; Chaikin smoothing snapped to cost minima.

---

## 1. Crate Layout

### Decision: new crate `turbo-tiles-fmm`, not extension of `turbo-tiles-pathfind`.

Why a new crate:
- The FMM solver has zero conceptual overlap with `pathfind/core/off_trail.rs`. The Theta\* code is graph-search-over-`Mesh`; FMM is upwind-PDE-on-grid. Sharing a module would mean one file with two unrelated algorithms.
- The narrow band heap + stencil iteration is performance-critical and benefits from being benched in isolation (criterion crate, no Pathfinder boot needed).
- Eventually the FMM crate becomes reusable for the *avalanche-runout* analysis (already on the roadmap per the `AvalancheTerrainLayer` comments) which is exactly the same machinery with a different metric.
- Keeps Theta\* code in `pathfind` for the duration of the rollout — it's the strategy-2 (hybrid) fallback's only off-trail leg implementation today, so we can't yank it until phase 9.

### Workspace shape

```text
crates/
  turbo-tiles-fmm/                 # new
    Cargo.toml
    src/
      lib.rs                       # public surface: Solver, Metric trait, FmmResult
      grid.rs                      # FmmGrid<T> — corridor-shaped 2D/3D array
      heap.rs                      # tiered narrow-band heap (paged bucket queue)
      stencil.rs                   # 2D + 3D upwind stencils, AGSI lattice basis reduction
      metric.rs                    # Metric / FinslerMetric / ElasticaMetric trait
      tobler.rs                    # Tobler-derived Finsler norm on slope vector
      elastica.rs                  # Euler-elastica 3-D (x, y, θ) augmented metric
      solve.rs                     # the Solver::run loop
      extract.rs                   # gradient descent / sub-cell tracer
      smooth.rs                    # cost-aware Chaikin
      diagnostics.rs               # arrival-time field dump for the SPA overlay
    tests/
      eikonal_isotropic.rs         # closed-form sanity (unit speed → t = dist)
      tobler_flat_vs_slope.rs      # disc-of-arrival test
      elastica_curvature.rs        # turning-radius bound test
  turbo-tiles-pathfind/
    src/
      fmm_adapter.rs               # NEW — feeds Pathfinder's CostContributor
                                   #       stack into a Metric impl and runs solver
      pathfinder.rs                # adds CostMode::FastMarching, dispatch
```

### Metric: trait-generic solver, hard-wired metric stack in the adapter

`turbo-tiles-fmm` is generic over `trait Metric`. The adapter crate is where Tobler-from-DEM, elastica state augmentation, and per-cell veto from the `CostContributor` stack get composed. This keeps the FMM core free of project-specific data sources (good for benchmarking and for the avalanche reuse later) while keeping the hot metric-evaluation tight (monomorphisation).

Public surface:

```rust
// turbo-tiles-fmm/src/lib.rs
pub trait Metric: Send + Sync {
    /// Dimensionality of the state space (2 for plain Finsler,
    /// 3 for (x, y, θ) elastica).
    const DIM: usize;

    /// Reduced quadratic-form coefficients on the **AGSI** lattice
    /// basis (Adaptive Geometric Stencil), in the local tangent
    /// plane at `state`. Two reduced offsets in 2D, three in 3D.
    /// Returning `None` marks the cell vetoed (`F = +∞`).
    fn local_norm(&self, state: State) -> Option<NormForm>;

    /// Fast cell-scale upper bound — used by the causality heuristic
    /// to pre-compute admissible front speed for the corridor mask.
    fn max_speed(&self) -> f64;
}

pub struct Solver<M: Metric> {
    grid: FmmGrid<f32>,        // arrival times u(x)
    state: FmmGrid<NodeState>, // FAR / CONSIDERED / ACCEPTED
    heap: NarrowBandHeap,
    metric: M,
}

pub struct FmmResult {
    pub arrival_times: FmmGrid<f32>,
    pub came_from: FmmGrid<u32>,   // packed upwind index
    pub solve_ms: u32,
    pub cells_accepted: u32,
}
```

---

## 2. Phased rollout (8 phases)

Every phase compiles, has tests, and leaves the existing Theta\* serving path untouched until phase 8.

| Phase | Title | Compiles | Default-on? | ~LoC |
|------:|:------|:--------:|:-----------:|:----:|
| 1 | Empty crate + isotropic eikonal | ✅ | n/a | 600 |
| 2 | Tobler Finsler metric on raw DEM | ✅ | n/a | 700 |
| 3 | Pathfinder adapter — corridor extraction + `CostContributor` veto | ✅ | n/a | 500 |
| 4 | Gradient-descent path extraction + Chaikin smooth | ✅ | n/a | 400 |
| 5 | State-augmented (x,y,θ) elastica metric | ✅ | n/a | 900 |
| 6 | Pathfinder dispatch — `CostMode::FastMarching` opt-in | ✅ | opt-in | 350 |
| 7 | Mimicry-corpus scenarios + new "Skolten" cases + SPA toggle | ✅ | opt-in | 250 |
| 8 | Production cut-over: default off-trail + hybrid legs use FMM | ✅ | **default** | 200 |

Total budget: ~3,900 LoC of new Rust + small SPA + tooling deltas.

---

### Phase 1 — Crate skeleton + isotropic eikonal

**Goal:** A working FMM that solves `‖∇u‖ = 1/F` on a uniform grid with constant speed `F = 1`. End state: closed-form sanity tests pass; nothing else in the workspace knows this crate exists.

**Files to add**
- `crates/turbo-tiles-fmm/Cargo.toml` — deps: only `thiserror`, `tracing`. No `nalgebra` yet (we hand-roll 2×2 quadratics).
- `crates/turbo-tiles-fmm/src/lib.rs`
- `crates/turbo-tiles-fmm/src/grid.rs`
- `crates/turbo-tiles-fmm/src/heap.rs`
- `crates/turbo-tiles-fmm/src/stencil.rs`
- `crates/turbo-tiles-fmm/src/solve.rs`
- `crates/turbo-tiles-fmm/tests/eikonal_isotropic.rs`

**Files to modify**
- `Cargo.toml` (workspace): add `turbo-tiles-fmm` to members; add a `[workspace.dependencies]` entry.

**Major types / functions**

```rust
// grid.rs
pub struct FmmGrid<T: Copy + Default> {
    pub nx: u32, pub ny: u32, pub nz: u32,   // nz=1 in this phase
    data: Vec<T>,
    pub origin_x: f64, pub origin_y: f64,
    pub cell_m: f64,
}
impl<T: Copy + Default> FmmGrid<T> {
    pub fn new(nx: u32, ny: u32, nz: u32, origin_x: f64, origin_y: f64, cell_m: f64) -> Self;
    pub fn idx(&self, i: u32, j: u32, k: u32) -> usize;
    pub fn get(&self, i: u32, j: u32, k: u32) -> T;
    pub fn set(&mut self, i: u32, j: u32, k: u32, v: T);
}

// heap.rs — paged bucket queue (Yatziv & Mirebeau, "O(n) FMM").
// 1024-entry bucket size, 64 buckets per page; degrades to a binary
// heap when the front is multi-scale (large speed contrast).
pub struct NarrowBandHeap { /* … */ }
impl NarrowBandHeap {
    pub fn push(&mut self, key: f32, idx: u32);
    pub fn decrease_key(&mut self, key: f32, idx: u32);
    pub fn pop_min(&mut self) -> Option<(f32, u32)>;
}

// stencil.rs — 4-neighbour upwind in 2D isotropic.
pub fn solve_quadratic_2d(u_x: f32, u_y: f32, f_inv: f32, h: f32) -> f32;

// solve.rs
impl<M: Metric> Solver<M> {
    pub fn new(grid_shape: GridShape, metric: M) -> Self;
    pub fn seed(&mut self, seeds: &[(u32, u32, u32)]);
    pub fn run_until(&mut self, stop: StopCondition) -> FmmResult;
}
pub enum StopCondition {
    AllAccepted,
    GoalReached { gi: u32, gj: u32, gk: u32 },
    TimeBudget(std::time::Duration),
}
```

**Algorithmic detail**
- Heap: paged bucket queue at `bucket_width = h / (2 * F_max)` so neighbour relaxations stay in the current or next bucket. Fall back to `BinaryHeap` if `F_max / F_min > 32`.
- Stencil: classical Sethian 2-direction upwind. Given accepted `u_x, u_y` from `{i±1, j±1}`, solve `(u - u_x)²/h² + (u - u_y)²/h² = f_inv²`. Discriminant clamp picks `max(u_x, u_y) + h * f_inv` on degenerate roots.
- Marching loop: pop min → ACCEPT → for each neighbour in FAR/CONSIDERED, recompute candidate u via stencil, decrease-key if better.

**Unit tests**
- `eikonal_isotropic::point_source_disc`: single seed at centre of 200×200 grid, `F=1`, `h=1`; assert `u[i,j] ≈ sqrt((i-c)² + (j-c)²) ± 0.5*h` for 95 % of cells (FMM is `O(h)` accurate).
- `eikonal_isotropic::axis_aligned_seed`: row of seeds along x=0; assert `u[i,j] = j*h` exactly along the axis.
- `heap::bucket_invariant`: 100 K pushes, popped values are non-decreasing.
- `stencil::degenerate_root_falls_back`: when discriminant < 0, returns `max(u_x, u_y) + h*f_inv`.

**Integration test**
- Skip. No connection to Pathfinder yet.

**Visual / SPA verification**
- None. Phase 1 ships only `cargo test -p turbo-tiles-fmm`.

**LoC:** 600

---

### Phase 2 — Tobler Finsler metric on raw DEM

**Goal:** Anisotropic FMM that solves the right PDE on real Norwegian terrain — for a circular start seed, arrival times stretch downhill (faster) and compress uphill (slower) by exactly the Tobler factor. Still standalone; no Pathfinder integration.

**Files to add**
- `crates/turbo-tiles-fmm/src/metric.rs`
- `crates/turbo-tiles-fmm/src/tobler.rs`
- `crates/turbo-tiles-fmm/tests/tobler_flat_vs_slope.rs`
- `crates/turbo-tiles-fmm/benches/tobler_bench.rs` (criterion)
- `crates/turbo-tiles-fmm/examples/disc_arrival_dump.rs` — dumps arrival field as PPM for visual inspection.

**Files to modify**
- `crates/turbo-tiles-fmm/Cargo.toml` — add `turbo-tiles-elev = { workspace = true }` (we need `Dem`).
- `crates/turbo-tiles-fmm/src/stencil.rs` — extend with anisotropic 8-neighbour stencil with AGSI lattice basis reduction.

**Major types**

```rust
// metric.rs
pub struct NormForm {
    /// 2×2 (or 3×3 in phase 5) symmetric positive-definite matrix
    /// in the local tangent plane — `F(v)² = vᵀ M v`. Stored as the
    /// AGSI-reduced basis: two (or three) offsets b_k and weights
    /// w_k such that `F(v)² = Σ w_k (b_k · v)²`.
    pub offsets: [[i8; 3]; 3],
    pub weights: [f32; 3],
    pub n_terms: u8,
}

// tobler.rs
pub struct ToblerFinsler {
    pub dem: Arc<turbo_tiles_elev::Dem>,
    pub refuse_slope_deg: f32,
    pub off_trail_base_s_per_m: f64,  // baseline pace
}

impl Metric for ToblerFinsler {
    const DIM: usize = 2;
    fn local_norm(&self, state: State) -> Option<NormForm> {
        let grad = self.dem_gradient(state.x, state.y)?;       // (dz/dx, dz/dy) in m/m
        let slope_mag = grad.norm();
        if slope_mag.atan().to_degrees() > self.refuse_slope_deg { return None; }
        // Asymmetric Tobler: pace(s) = 1/[1.6667·exp(-3.5·|s+0.05|)].
        // Build a Finsler norm whose unit ball is the indicatrix of
        // {v : pace(grad · v̂) · ‖v‖ = 1}. Project onto AGSI basis
        // via Selling/Voronoi reduction on the equivalent quadratic
        // (linearised at the cell scale).
        let m = build_anisotropic_form(grad, self.off_trail_base_s_per_m);
        Some(agsi_reduce_2x2(m))
    }
    fn max_speed(&self) -> f64 { 1.6667 }  // m/s on optimum descent
}
```

**Algorithmic detail**
- AGSI basis reduction: at each cell, take the 2×2 SPD metric tensor `M`, compute the Selling reduction (Voronoi's first reduction) yielding three lattice offsets `b_k ∈ {±e_1, ±e_2, ±(e_1+e_2), ±(e_1-e_2)}` and nonneg weights `w_k`. The stencil reads `u` at neighbours in those three offsets only — exact, monotone, causal. This is the core "anisotropic FMM" trick from Mirebeau.
- Linearised Tobler: Tobler is a non-quadratic Finsler norm. Mirebeau (Sec. 5 of the 2018 paper) approximates per-cell by the *osculating quadratic* — second-order expansion at the cell's dominant descent direction. This is what `build_anisotropic_form` does.
- Refusal: `local_norm → None`. Cells stay in FAR forever; their `u` reads as `INFINITY`. They are **not** masked out of the heap entirely — keeping them lets adjacent cells correctly detect "no valid upwind on this side".

**Unit tests**
- `tobler_flat_vs_slope::flat_ground_matches_isotropic`: zero-gradient synthetic DEM → arrival time = `dist / 1.4` everywhere ± 1 %.
- `tobler_flat_vs_slope::uphill_ramp_30deg`: synthetic ramp with `dz/dx = tan(30°)`, single seed at base, point 100 m along ramp → arrival time matches Tobler closed-form for 30° within 5 %.
- `tobler_flat_vs_slope::contour_following`: synthetic conical mountain, seed at one point on the slope, midline of the arrival-time isocontour should hug the contour, not climb to the peak. Assert: peak position of the `u = 200 s` isocontour stays within ±10 m of true elevation.
- `tobler_flat_vs_slope::refusal_inland_lake`: synthetic flat with a 100 m square of slope > 45° in the middle; arrival time field on the far side ≥ time on the near side via the longer way around (assert the lake is *avoided*, not crossed).
- `agsi_reduce::self_consistency`: random SPD matrices; reconstructed `F(v)²` from offsets+weights matches `vᵀ M v` within 1 e-6.

**Integration test**
- None yet — no Pathfinder wiring.

**Visual / SPA verification**
- Run `cargo run -p turbo-tiles-fmm --example disc_arrival_dump -- --dem-tile <tile> --seed <lon,lat>`. Open the emitted PPM. Verify isocontours visually bend downhill on Skolten DEM.

**LoC:** 700

---

### Phase 3 — Pathfinder adapter (corridor extraction + CostContributor veto)

**Goal:** A function in `pathfind` that takes the same inputs as today's `build_off_trail_segment` and produces an arrival-time field over a tight corridor bbox. Still returns nothing to the caller — invocable only from a unit test for now.

**Files to add**
- `crates/turbo-tiles-pathfind/src/fmm_adapter.rs`
- `crates/turbo-tiles-pathfind/tests/fmm_adapter_corridor.rs`

**Files to modify**
- `crates/turbo-tiles-pathfind/Cargo.toml` — add `turbo-tiles-fmm = { workspace = true }`.
- `crates/turbo-tiles-pathfind/src/lib.rs` — `pub mod fmm_adapter;` behind a feature-gate is **not** what we want; the module is unconditional from day one. Just `pub(crate) mod fmm_adapter;`.

**Major types**

```rust
// fmm_adapter.rs
pub(crate) struct PathfinderMetric<'a> {
    tobler: turbo_tiles_fmm::tobler::ToblerFinsler,
    /// CostContributor stack — re-used to detect *additional* vetoes
    /// (water, glacier, avalanche zone) that aren't slope. Iterated
    /// once per cell at corridor-build time; results are baked into
    /// a `refused: BitVec` rather than re-queried during the solve.
    contributors: &'a [Arc<dyn CostContributor>],
    /// Per-cell extra walk-seconds-per-metre delta from non-slope
    /// contributors (marking, proximity, total_gain). Sampled once
    /// at corridor-build time into an `FmmGrid<f32>`.
    extra_pace_s_per_m: turbo_tiles_fmm::FmmGrid<f32>,
}
impl<'a> turbo_tiles_fmm::Metric for PathfinderMetric<'a> { /* delegates */ }

pub(crate) struct FmmSolveInputs {
    pub from: PointXY,
    pub to: PointXY,
    pub profile: Profile,
    pub cell_m: f64,                          // 10.0 by default — see §3
    pub contributors: Vec<Arc<dyn CostContributor>>,
    pub off_trail_base: f64,
    pub refuse_slope_deg: f32,
}

pub(crate) struct FmmSolveOutput {
    pub arrival: turbo_tiles_fmm::FmmGrid<f32>,
    pub came_from: turbo_tiles_fmm::FmmGrid<u32>,
    pub corridor_bbox: MeshBbox,
    pub solve_ms: u32,
    pub vetoed_cells: u32,
    pub refused_by: Vec<String>,
}

pub(crate) fn solve_fmm_corridor(
    inputs: FmmSolveInputs,
    dem: &Arc<Dem>,
) -> Result<FmmSolveOutput, FmmAdapterError>;
```

**Algorithmic detail: corridor extraction**
The corridor is a tight bbox aligned to the DEM's UTM grid:

```text
let d = ‖to - from‖
let pad = max(4 * cell_m, 0.30 * d)            // same rule the mesh uses today
let centerline_buffer = max(800.0, 0.20 * d)   // wider corridor for FMM
let bbox = oriented_bbox(from, to, half_width = centerline_buffer + pad)
```

We do **not** rotate the grid — axis-aligned UTM corridor is enough and keeps neighbour indexing trivial. The "oriented" bbox is just the axis-aligned bbox of the rotated rectangle around the from-to centerline. This bloats the bbox by `sin(angle) * length` in the worst case (45°), which on a 5 km query adds ~1.4 km extra cells — acceptable on 10 m resolution (corridor stays under 5 × 10⁵ cells).

**Cell size: 10 m, not 25 m.**
- DEM native resolution is 10 m. At 25 m we are pre-averaging the very terrain features the FMM is supposed to follow — undoing the whole point of switching off Theta\*.
- At 10 m a 10 km × 1.5 km corridor is 1.5 × 10⁶ cells. With paged-bucket heap that's ~0.4 s solve (Mirebeau reports ~10⁷ cells/s; we'll budget 3 × 10⁶ cells/s for a Rust port). See §6.
- For mid-range queries (3–5 km) the cell count is ~4 × 10⁵; well under 200 ms.

**Causality / admissible heuristic**
Don't introduce it in phase 3. Plain global FMM. The admissible-heuristic version (causal-restriction-to-Δ-ellipse around the from-to line) lands in **phase 6** with the dispatch wiring; until then we want a deterministic baseline.

**CostContributor vetoes**
Iterate the contributor list once per cell with a synthetic 1 m mesh edge `EdgeContext` at the cell centre. Any `veto()` → mark cell as refused (`local_norm` returns `None`). This avoids per-relaxation contributor calls (3 contributor evaluations × 5 × 10⁵ cells × 8 neighbours ≈ would dominate solve time).

**Unit tests**
- `fmm_adapter_corridor::corridor_around_oslo_pair`: real DEM coverage; corridor cell count, solve under 300 ms, no panic.
- `fmm_adapter_corridor::veto_water_polygon_creates_island`: synthetic mask refusing a square in the middle; assert that square's cells all have `arrival = +∞` and the surrounding cells route around.
- `fmm_adapter_corridor::degenerate_short_query`: from=to+(10m, 0); single-cell corridor produces a trivial arrival field.

**Integration test against `trail-mimicry.toml`**
- Skip — no path extraction yet. Phase 4.

**Visual / SPA verification**
- A new admin endpoint `/v1/debug/pathfind/fmm-field?from=…&to=…` returns the arrival-time grid as a flat array + bbox metadata. The SPA's existing inspect overlay can render it as a heatmap. Behind admin-auth, off-by-default in production.

**LoC:** 500 (300 adapter, 100 endpoint, 100 tests)

---

### Phase 4 — Gradient-descent path extraction + Chaikin smoothing

**Goal:** Turn the arrival-time field into a polyline. End state: `solve_fmm_corridor` followed by `extract_path` returns a `Vec<Point2>` that compares well to today's Theta\* output on isotropic terrain.

**Files to add**
- `crates/turbo-tiles-fmm/src/extract.rs`
- `crates/turbo-tiles-fmm/src/smooth.rs`
- `crates/turbo-tiles-pathfind/tests/fmm_path_vs_theta.rs`

**Files to modify**
- `crates/turbo-tiles-fmm/src/lib.rs` — expose `extract_path`, `chaikin_smooth_cost_aware`.

**Major functions**

```rust
// extract.rs
pub fn extract_path(
    arrival: &FmmGrid<f32>,
    goal: (f64, f64),         // in world coords (UTM m), sub-cell
    start: (f64, f64),
    cell_m: f64,
    step_m: f64,              // sub-cell, default cell_m / 4 = 2.5 m
    max_steps: u32,
) -> Result<Vec<Point2>, ExtractError>;

// smooth.rs
pub fn chaikin_smooth_cost_aware(
    path: &[Point2],
    arrival: &FmmGrid<f32>,  // re-used as cost field
    iterations: u8,           // 2 by default
    snap_radius_m: f64,       // 2 cells
) -> Vec<Point2>;
```

**Algorithmic detail: gradient descent**
- Start at `goal` (sub-cell). At each step compute `∇u` by central differences over the 4-cell neighbourhood — bilinearly interpolated for sub-cell positions.
- Step `p ← p - step_m * ∇u / ‖∇u‖`.
- Termination: `‖p - start‖ < 0.5 * cell_m` OR `u(p) ≤ step_m * BASE_PACE_S_PER_M`.
- Bail-out: `max_steps = 10 * (corridor_diagonal / step_m)`; if exceeded → `ExtractError::Diverged` (means the field has a local-minimum trap, which would be a metric bug).
- Sub-cell step `cell_m / 4` is critical: at full-cell steps the path zig-zags between two equally-low neighbours.

**Algorithmic detail: cost-aware Chaikin**
- Standard Chaikin smoothing: each segment `[a, b]` becomes `[a + 0.25(b-a), a + 0.75(b-a)]`.
- "Cost-aware" twist: after each Chaikin pass, snap each new vertex to the local min of `u` within `snap_radius_m`. Prevents the smoother from cutting corners into high-cost cells (which standard Chaikin happily does — that's exactly what makes Theta\* output look unnatural).
- 2 iterations gives a path that no longer reads as "made of grid cells" but doesn't drift across cost discontinuities.

**Unit tests**
- `extract::straight_path_isotropic`: F=1 grid, seed at (0,0), goal at (100,100); extracted path has length 141.42 ± 1 m.
- `extract::diverging_field_errors`: hand-construct a field with a saddle point in the middle; `extract_path` returns `ExtractError::Diverged`.
- `smooth::cost_snap_keeps_off_refused`: synthetic field with a 100 m wide low-cost stripe and high-cost shoulders; raw extracted path inside the stripe; Chaikin output stays inside the stripe after smoothing.
- `fmm_path_vs_theta::oslo_pair_isotropic`: with a constant-1 cost field, FMM path length is within 2 % of Theta* output on the same corridor.

**Integration test against `trail-mimicry.toml`**
- Run scenarios 1, 2, 3 (`valnesfjord-trail-end-to-end-3km`, `valnesfjord-short-1km`, `oslo-marka-5km`) through a new `mimicry.py --solver fmm` mode that calls a debug endpoint `/v1/debug/pathfind/fmm-path` (admin-only). Compute mean/max deviation and length ratio.
- **Acceptance bar for this phase:** numbers don't have to be *better* than Theta\* yet — they just need to land inside the corpus thresholds for the simple scenarios (1, 2). The elastica metric in phase 5 is what closes the gap on Skolten-style cases.

**Visual / SPA verification**
- The same `/fmm-field` endpoint from phase 3 now also returns the extracted polyline. SPA inspect panel adds a "FMM path (debug)" toggle that overlays it in orange next to the current Theta* path in blue.

**LoC:** 400

---

### Phase 5 — State-augmented (x, y, θ) elastica metric

**Goal:** Add curvature penalty. End state: FMM paths on Skolten-area terrain follow contour curves rather than zig-zagging across them — the headline correctness win.

**Files to add**
- `crates/turbo-tiles-fmm/src/elastica.rs`
- `crates/turbo-tiles-fmm/tests/elastica_curvature.rs`
- `crates/turbo-tiles-fmm/examples/elastica_isocontour_dump.rs`

**Files to modify**
- `crates/turbo-tiles-fmm/src/stencil.rs` — extend AGSI reduction to 3D (3-offset Selling reduction; up to 6 terms).
- `crates/turbo-tiles-fmm/src/grid.rs` — already supports `nz`; phase 1 used `nz = 1`. Phase 5 sets `nz = N_θ`.

**State space**
- `(x, y)` on the corridor grid, plus `θ ∈ [0, 2π)` discretised into **16 bins** (every 22.5°).
  - 8 bins is too coarse — minimum turning radius collapses to a hexagon.
  - 32 bins is what HFM uses; 4× memory and 2× solve time over 16. Defer to phase 8 as a "if Skolten still looks wrong, bump to 32" knob.
- Memory cost at 16 bins, 10 m, 10 km × 1.5 km corridor: `1.5 × 10⁶ cells × 16 bins × 8 bytes (u32 came_from + f32 arrival) = 192 MB`. **At the ceiling.** §6 discusses tiling.

**Metric**

```rust
pub struct ElasticaMetric<'a> {
    base: PathfinderMetric<'a>,           // the 2D Finsler from phase 3
    pub n_theta: u8,                       // 16
    pub xi: f64,                            // curvature relaxation length (m). 50 m = "hiker"
}

impl<'a> Metric for ElasticaMetric<'a> {
    const DIM: usize = 3;
    fn local_norm(&self, state: State3D) -> Option<NormForm> {
        let theta = (state.k as f64 + 0.5) / self.n_theta as f64 * std::f64::consts::TAU;
        let dir = (theta.cos(), theta.sin());
        // Base 2D Finsler at (x, y) projected onto the *forward*
        // direction θ. The augmented quadratic form is
        //   F(v, ω)² = ‖v · dir‖² · F_base² + ξ² · ω²
        //   subject to v ⟂ dir⁺ → +∞  (no sideways motion).
        // Reduce in 3D via Selling on the 3×3 SPD form built from
        // the 2D base + ξ² for the θ component.
        Some(elastica_3d_form(&self.base.local_norm(state.xy())?, dir, self.xi))
    }
}
```

**Algorithmic detail**
- The Reeds–Shepp / Euler-elastica metric (Mirebeau 2018 §3.2) makes turning expensive proportional to `1/ξ`. `ξ` is the *characteristic turning length*: a hiker can comfortably turn 90° over ~50 m, so `ξ = 50` m is a starting point. Calibrate against scenario 5 (Jotunheimen) in phase 7.
- Stencil: the 3D AGSI lattice basis reduction yields up to 6 offsets, each a tuple `(di, dj, dk)` with `dk ∈ {-1, 0, +1}` (θ neighbour). The standard 8-neighbour 2D stencil becomes a ~12-neighbour 3D stencil.
- The "no sideways motion" constraint is enforced by setting the off-axis weight to a *very large* (not infinite) number — infinite would break Selling reduction. Mirebeau uses `1e6 × base`.

**Seeds**
- The start cell is seeded with **all** 16 θ-values at `u = 0` — initial heading is unknown.
- The goal is *any* θ at the goal `(i, j)` cell — we marginalise during extraction (next).

**Path extraction in 3D**
- Walk gradient descent in 3D `(x, y, θ)` from the `(i, j, k*)` with minimum arrival time at the goal `(i, j)`.
- After extraction, project to `(x, y)` by dropping θ.
- Smoothing is unchanged from phase 4.

**Unit tests**
- `elastica_curvature::turning_radius_lower_bound`: synthetic flat (F=1), seed at origin facing +x, goal at (-100, 0) ("hairpin"). Assert path length ≥ π · 50 m (= 157 m) — i.e. the path bends rather than reversing instantly.
- `elastica_curvature::large_xi_collapses_to_isotropic`: with `ξ = 10000` (i.e. no curvature penalty), path matches phase-4 isotropic output within 2 %.
- `elastica_curvature::contour_following_on_synthetic_ridge`: synthetic ridge running east-west, seed at one foot, goal at the other foot; assert the path crosses the ridge at most twice (i.e. doesn't oscillate). With elastica off, Theta\* output crosses it 6–10 times — that's the failure mode we're fixing.

**Integration test against `trail-mimicry.toml`**
- All existing scenarios + 3 new ones (see phase 7) through the debug `--solver fmm` mode. Phase 5 gate: scenarios 4 and 5 (Jotunheimen, Trøndelag coastal) now pass at the existing thresholds.

**Visual / SPA verification**
- The arrival-field overlay now needs a θ-slider (pick a heading; show the 2D slice). Phase 7 adds that to the SPA. For now, the example dumps PPMs.

**LoC:** 900 (500 elastica + 3D stencil, 200 tests, 200 example + scaffolding)

---

### Phase 6 — Pathfinder dispatch: `CostMode::FastMarching` opt-in

**Goal:** First request-time path through the new solver. End state: an SPA toggle or `Prefs::cost_mode = "fast_marching"` runs FMM end-to-end; default behaviour is unchanged.

**Files to modify**
- `crates/turbo-tiles-pathfind/src/pathfinder.rs`:
  - Extend the `CostMode` enum (line 426–444) with a `FastMarching` variant.
  - In `build_off_trail_segment` (line 1310), branch on `prefs.cost_mode == CostMode::FastMarching` *before* the mesh-build code; on the FMM branch, call `fmm_adapter::solve_fmm_corridor` + `extract_path` + `chaikin_smooth_cost_aware` and construct an `OffTrailSegment` identical-shaped to the Theta\* one.
  - Add `prefs.fmm_xi: Option<f64>` and `prefs.fmm_n_theta: Option<u8>` knobs (default None → use cost-config).
- `crates/turbo-tiles-pathfind/src/config.rs`: add `[fmm]` section with `xi_m`, `n_theta`, `cell_m`, `corridor_half_width_m`. Embedded defaults: `xi=50, n_theta=16, cell_m=10.0, corridor_half_width_m=800.0`.
- `crates/turbo-tiles-pathfind/src/fmm_adapter.rs`: implement the public `solve_off_trail_fmm(self, from, to, prefs) -> OffTrailSegment` that the dispatch calls.
- `crates/turbo-tiles-api/`: ensure `cost_mode = "fast_marching"` deserialises through to `Prefs`.

**Algorithmic decision: admissible-heuristic causal restriction**
Land it here, not before. With elastica wired up we know the metric's `max_speed`; restrict the FMM front to cells whose admissible heuristic to the goal is `≤ u_min_seen + slack`. Slack = `1.30 × straight_line_time` so we don't prematurely prune Skolten-style detours. This shaves ~40 % of cells in the typical corridor.

**Strategy contract — what does the FMM off-trail leg return?**
Same `OffTrailSegment` struct as today:
```rust
struct OffTrailSegment {
    geometry: Vec<Point2>,
    length_m: f64,
    cost: f64,           // ← in walk-seconds (already the WalkSeconds unit)
    refused_by: Vec<String>,
}
```
The `cost` field is just `u[goal] * off_trail_base`. That is **directly comparable** to the existing `WalkSeconds` graph leg cost — no rescaling needed, because Tobler-Finsler metric integrates exactly to walk-seconds along the path. This is the key reason the architectural change is clean: the integrated cost on the arrival surface *is* the path's walk-seconds.

`fkb_breakdown` continues to bucket the whole leg under `off_trail`.

**Cost comparison vs on-graph / hybrid**
- The existing candidate-list-then-min-by-cost logic in `solve_inner` (line 1110) **needs no change**. Each strategy returns `Path.cost` in walk-seconds; lowest wins.

**Tests**
- `pathfinder::fmm_path_strategy_chosen_when_cheaper`: synthesise a query where a long road detour costs 2000 s and a direct off-trail FMM path costs 1200 s; assert chosen strategy = `OffTrail`, cost = 1200.
- `pathfinder::fmm_falls_back_to_theta_on_error`: simulate `solve_fmm_corridor` returning a `Diverged` error (forced via a test-only metric); assert the pathfinder logs a warning and falls back to Theta\* in `Multiplicative`/`WalkSeconds` mode. **No silent failure.**

**Integration test against `trail-mimicry.toml`**
- `mimicry.py --solver fmm` runs the full corpus through the dispatch in `cost_mode = "fast_marching"`. Phase 6 gate: ≥ 4 of 6 existing scenarios pass.

**Visual / SPA verification**
- SPA Prefs panel: new radio "Cost mode" with `multiplicative / walk_seconds / fast_marching`. The endpoint already accepts `cost_mode`; just expose the new variant. Operator picks FMM, fires a query, sees the resulting path with `strategy = off_trail` and a curve through the right contour.

**LoC:** 350 (200 dispatch + adapter glue, 100 config, 50 tests)

---

### Phase 7 — Mimicry corpus expansion, "Skolten" cases, SPA polish

**Goal:** Lock in correctness on the cases that motivated the rewrite.

**Files to modify**
- `apps/tileserver/tools/trail-mimicry.toml`: add 3 new scenarios — Skolten-area cases where Theta\* visibly fails today.
- `apps/tileserver/tools/mimicry.py`: add `--solver {auto, theta, fmm}` flag (`auto` = whatever `cost_mode` resolves to); add `--compare` to print Theta\* vs FMM side-by-side for the corpus.
- SPA: add the θ-slice slider to the FMM debug overlay.

**Files to add**
- `apps/tileserver/tools/fmm-baseline.json` — committed baseline of arrival-time field hashes per scenario so regressions are detected by CI.

**New scenarios** (concrete; coordinates approximate — phase author to refine):

```toml
[[scenario]]
name = "skolten-ridge-traverse"
from = [13.20, 68.69]   # foot of NE ridge
to   = [13.27, 68.72]   # foot of SW ridge
profile = "foot"
# Theta* cuts across the ridgeline producing a 38° crossing; the
# hiker's choice is a longer contour-following route over the col.
mean_deviation_max_m = 220.0
max_deviation_max_m  = 600.0
length_ratio_max     = 1.7

[[scenario]]
name = "narvik-rombakstottta-contour"
from = [17.50, 68.45]
to   = [17.54, 68.46]
profile = "foot"
# Contour-following test along the south face. Theta* prefers
# straight, FMM should follow the contour band.
mean_deviation_max_m = 180.0
max_deviation_max_m  = 450.0
length_ratio_max     = 1.5

[[scenario]]
name = "lofoten-coastal-fjord-detour"
from = [13.95, 68.10]
to   = [14.05, 68.13]
profile = "foot"
# Small fjord forces a detour. Both algorithms should refuse the
# water; FMM should produce a curve hugging the shoreline rather
# than an angular polyline.
mean_deviation_max_m = 250.0
max_deviation_max_m  = 700.0
length_ratio_max     = 1.7
```

**Phase 7 gate:** all 9 scenarios (6 existing + 3 new) pass under `--solver fmm`; the 3 new ones pass for FMM but **fail** for Theta\*. That asymmetry is the proof.

**LoC:** 250 (mimicry.py changes + SPA polish + 3 scenarios + baseline JSON)

---

### Phase 8 — Production cut-over

**Goal:** Default `cost_mode` becomes `FastMarching` for off-trail and hybrid off-trail prefix/suffix legs.

**Files to modify**
- `crates/turbo-tiles-pathfind/src/pathfinder.rs`: change `CostMode::default()` to `FastMarching`. Document the migration in a comment.
- `apps/tileserver/tools/cost-config.toml`: bump version, set `fmm.cell_m = 10.0` in default profile, keep escape valves.
- Keep the `WalkSeconds` and `Multiplicative` variants and the Theta\* mesh code intact — escape valves for at least one full release cycle.

**Tests**
- Run the full mimicry corpus under default settings; baseline JSON locked.
- Run the existing route-scenarios.toml (the broader correctness corpus) — assert no regression on graph-only queries (FMM doesn't fire) and improvements on hybrid + off-trail.

**LoC:** 200 (mostly comment / doc / config changes)

---

## 3. Per-phase summary table

| # | Files to add (key) | Algorithmic crux | Mimicry gate |
|---|---|---|---|
| 1 | `grid.rs`, `heap.rs`, `stencil.rs`, `solve.rs` | Paged bucket queue + 2D Sethian upwind | n/a |
| 2 | `tobler.rs`, `metric.rs` | AGSI/Selling lattice basis reduction; Finsler from Tobler | n/a |
| 3 | `pathfind/fmm_adapter.rs` | Corridor extraction; per-cell veto bake-in | n/a |
| 4 | `extract.rs`, `smooth.rs` | Sub-cell gradient descent; cost-snapped Chaikin | scen 1–3 pass |
| 5 | `elastica.rs`, 3D stencil | `(x,y,θ)`-augmented Selling reduction; Reeds–Shepp ball | scen 4–5 pass |
| 6 | `CostMode::FastMarching` dispatch | Causal heuristic restriction; OffTrailSegment shape | corpus ≥ 4/6 pass |
| 7 | Skolten scenarios + SPA toggle | (calibration only) | 9/9 pass, Theta\* fails new 3 |
| 8 | (config only) | (default cut-over) | regression-free |

---

## 4. Specific algorithmic decisions (consolidated)

| Question | Decision | Reason |
|---|---|---|
| Cell size | **10 m** | Native DEM res; 25 m pre-averages the features FMM exists to follow. |
| Corridor bbox | Axis-aligned bbox of an oriented rectangle of half-width `max(800 m, 0.20·d)` plus `max(4·h, 0.30·d)` pad | Wider than current mesh because elastica needs detour room. |
| Causal heuristic | Slack-A\* style — front cells whose `u + h_admissible > u_min_goal × 1.30` are deferred | Saves ~40 % cells; 1.30 chosen so Skolten detours stay in. |
| Curvature state | `(x, y, θ)`, **16 bins** | 8 is too coarse, 32 is 2× cost. Phase 8 escape valve to 32 if needed. |
| ξ (turning length) | **50 m**, in cost-config | Calibrated in phase 7 against scenario 5 (Jotunheimen ridge). |
| Refused cells | `local_norm → None` → never accepted. Stay in FAR with `u = +∞`. | Keeps stencil monotonicity; cheaper than rebuilding the corridor. |
| Path extraction step | `cell_m / 4` = 2.5 m | Sub-cell to avoid zig-zag between equally-low neighbours. |
| Goal termination | `‖p - start‖ < 0.5·cell_m` OR `u(p) ≤ step_m × BASE_PACE` | Distance OR cost — whichever triggers first. |
| Seed: `start_node` (mesh) | Single sub-cell point source. In elastica, seed all 16 θ at `u = 0`. | Initial heading unknown. |
| Seed: `exit_nodes` (hybrid bridge) | When FMM is the *prefix* of a hybrid route, the *goal* is the graph-bridge node; we extract from there back to `from`. When it's the *suffix*, reversed. | One solve per leg; cells aren't shared because corridors differ. |

---

## 5. Integration with Pathfinder dispatch

```rust
// in solve_inner (≈ line 1083 onward):
if !prefs.force_off_trail { /* try_on_graph + try_hybrid as today */ }
if prefs.allow_off_trail && extent_km <= prefs.max_off_trail_km {
    let segment = match prefs.cost_mode {
        CostMode::FastMarching => self.solve_off_trail_fmm(from_xy, to_xy, &prefs)?,
        _ => self.build_off_trail_segment(from_xy, to_xy, &prefs)?,  // Theta*
    };
    candidates.push(off_trail_path_from_segment(segment));
}
candidates.into_iter().min_by(/* cost */).ok_or(NoRoute)
```

Hybrid: in `try_hybrid` (line 1152) the two `build_off_trail_segment` calls (line 1186, 1198) become `build_off_trail_segment_with_mode` that dispatches the same way. The graph middle leg is untouched.

`Path.cost` semantics: walk-seconds end-to-end, identical to the `WalkSeconds` mode contract. Min-by-cost comparison logic in `solve_inner` requires zero change.

---

## 6. Performance budget

Targets for the 10 km / 10 m / elastica-16θ worst case:

- **Cells:** `(10000/10) × (1500/10) × 16 = 2.4 × 10⁷`. At a budgeted 3 × 10⁶ cells/s pure-FMM throughput (Rust, paged-bucket heap, monomorphised metric), this is **8 s**. Too slow.
- **Mitigation:** for queries over 4 km, drop to 16 m cells **for the elastica solve only**, fall back to 10 m for a second-pass non-elastica refinement along the extracted band. 16 m × 16 bins × 10 km × 1.5 km = 6 × 10⁶ cells → ~2 s. 10 m × 1-θ refinement in a 100 m band around the path ≈ 10⁵ cells → 30 ms.
- **Memory:** 2.4 × 10⁷ × (4 + 4) bytes = 192 MB at worst case. At 16 m: 48 MB. Acceptable.
- **Tile when:** query distance > 8 km. Split into two corridor solves with overlap; stitch at the midpoint cell with minimum arrival cost on the shared boundary.
- **Common case (3 km query):** 300 × 100 × 16 = 5 × 10⁵ cells → 150 ms target. Realistic.

---

## 7. Verification criteria — mimicry harness changes

- New CLI flag `--solver {auto, theta, fmm}` on `mimicry.py`.
- New `--compare` mode prints a 3-column table per scenario: scenario / Theta\* metrics / FMM metrics, with PASS/FAIL per the thresholds.
- 3 new scenarios (§Phase 7) targeting cases Theta\* visibly fails on: contour traverses, ridge crossings, fjord-side curves. These scenarios **must fail under Theta\*** at land time — that's how we prove the rewrite added something.
- Commit a `tools/fmm-baseline.json` keyed by scenario name, containing the SHA-256 of the densified output polyline. CI runs the corpus under FMM and diffs against the baseline; intentional changes require committing a new hash.

---

## 8. Risk register

| Risk | Likelihood | Mitigation |
|---|---|---|
| **Selling reduction edge cases** — degenerate quadratic forms on flat / pathological DEM cells produce malformed offsets, breaking causality. | High. Mirebeau's C++ has 25 years of patches we don't. | Always sanity-check `weights ≥ 0` post-reduction; on failure fall back to the 2D Sethian isotropic stencil with `F = pace_at_local_slope`. Don't crash; just lose anisotropy on that cell. |
| **Tobler is non-quadratic** — the osculating-quadratic approximation in `build_anisotropic_form` is exact only at the cell's local descent direction. Strongly cross-slope motion gets the wrong cost. | Medium. | Validate against synthetic ridge in phase 2 unit test. If error > 10 %, bake a *direction-dependent* pace table (16 entries per cell) instead of a single quadratic — bigger memory but tractable. |
| **Elastica state-augmentation memory blow-up** at 10 m × 32 θ. | Medium. | Stay at 16 θ. Tile at > 8 km. Document the 32-bin escape valve as a per-request override only. |
| **Path extraction local-minimum traps** when the metric is nearly degenerate (long flat plateau). The arrival field has saddles; gradient descent walks parallel to true downhill. | Medium. | Detect via `‖∇u‖ < ε` for > 3 steps; on hit, switch to A\* over the discrete `came_from` field for the next 20 cells. Robust fallback. |
| **Hybrid leg start/end placement** — the bridge node sits *on* a graph edge, not at a cell centre. Sub-cell seed placement matters more here than in pure off-trail. | Low. | Mirebeau's approach: spread the seed mass to the 4 surrounding cells weighted by inverse distance — gives the correct arrival-time gradient near the bridge. Implement from day one in phase 4. |
| **Causal-heuristic mis-prunes the goal**, returning `+∞` for legitimate detours. | Medium. | Slack of 1.30 is generous. Add a fallback: if `u[goal] = +∞` after the heuristic-restricted run, rerun without the heuristic. One-shot retry, logged. |
| **Solver throughput below 3 × 10⁶ cells/s.** Mirebeau reports ~10⁷ cells/s in tuned C++. We don't get there in a v1 Rust port. | High. | Budget 1 × 10⁶ cells/s and tile aggressively. Performance pass after correctness. |
| **CostContributor + Metric duplication.** Slope is computed once for `ToblerSlopeContributor` (cell veto) and again for `ToblerFinsler::local_norm`. | Low (just wasteful). | Phase 3 adapter pre-bakes the slope from `ToblerSlopeContributor::sample_elevations` into the metric's input; one DEM sample per cell, not three. |
| **Floating-point causality breaks at zero-slope plateau.** FMM monotonicity assumes `u` strictly increases along the front. | Low. | Use `f32` arrival, `f64` arithmetic inside the stencil; tie-break by lexicographic cell index. Standard FMM hygiene. |
| **The literature is 25 years deep.** We're porting an algorithm whose corner cases are documented in papers we haven't read. | Certain. | Keep the `Multiplicative`/`WalkSeconds` modes wired forever. The escape valve is institutional. |

---

## 9. Explicitly out of scope

1. **IRL / learned costs** — no neural-network-derived cost surfaces. Tobler + per-contributor walk-seconds only.
2. **OSM-derived trail ingest** — the graph leg keeps reading from `paths.edge` exclusively. No mid-flight OSM joins.
3. **Mesh-resolution increases above 10 m** — Norwegian DEM doesn't support finer; we don't fake it.
4. **Building-mask polygon ingest** — no new vector layer in `MaskRefusalLayer` or alongside. Refusal stays at water + glacier + slope.
5. **Multi-objective routing** (e.g. minimise time *and* exposure) — single scalar `u(x)` only. Pareto fronts are a separate paper.

---

## Critical files for implementation

- `/Users/sigmundsandring/StudioProjects/turbo/apps/tileserver/crates/turbo-tiles-pathfind/src/pathfinder.rs`
- `/Users/sigmundsandring/StudioProjects/turbo/apps/tileserver/crates/turbo-tiles-pathfind/src/contributor.rs`
- `/Users/sigmundsandring/StudioProjects/turbo/apps/tileserver/crates/turbo-tiles-pathfind/src/native_contributors.rs`
- `/Users/sigmundsandring/StudioProjects/turbo/apps/tileserver/crates/turbo-tiles-pathfind/src/core/off_trail.rs`
- `/Users/sigmundsandring/StudioProjects/turbo/apps/tileserver/tools/trail-mimicry.toml`
