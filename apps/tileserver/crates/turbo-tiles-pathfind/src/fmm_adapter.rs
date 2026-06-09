//! Pathfinder ↔ FMM adapter.
//!
//! Bridges the project's `Dem` + `CostContributor` stack to the
//! generic `turbo-tiles-fmm` solver. Phase 3 surface:
//!
//!   - `DemElevation` — implements `fmm::Elevation` against a
//!     project `Arc<Dem>`, translating grid `(i, j)` cell indices
//!     into UTM33N `PointXY` for the DEM sampler.
//!   - `solve_fmm_corridor` — given `(from, to)` in UTM33N + a
//!     contributor list, sizes a corridor bbox around the from-to
//!     centerline, allocates the FMM grid, bakes the cost field
//!     (Tobler + per-cell vetoes from the contributors), and runs
//!     `solve_2d_with_metric`. Returns the arrival-time grid +
//!     statistics. Phase 4 will add path extraction on top.

use std::sync::Arc;

use turbo_tiles_elev::{Dem, PointXY};
use turbo_tiles_fmm::{
    bake_aniso_corridor, bake_metric_2d, chaikin_smooth_cost_aware, extract_path_discrete,
    solve_2d_anisotropic, solve_2d_isotropic, CellForm, Elevation, FmmGrid, GridShape, PathPoint,
    StopCondition, ToblerAnisotropic, ToblerIsotropic,
};

use crate::contributor::{CostContributor, EdgeContext, EdgeKind};

/// Implements `fmm::Elevation` against the project's `Arc<Dem>`.
/// Each cell sample goes through `Dem::sample(PointXY)`, which the
/// DEM crate's tile cache hot-paths efficiently. The adapter holds
/// the `Arc` so it can outlive the corridor solve.
pub struct DemElevation {
    pub dem: Arc<Dem>,
    /// Per-cell elevation memo (m), sized `nx*ny` lazily on first use.
    /// `+∞` = not yet sampled, `NaN` = sampled-but-nodata, else the
    /// value. The grade-limited lifted solver queries each cell's
    /// elevation ~16-30× (once per incident move across all headings);
    /// memoising collapses that to one `Dem::sample()` per cell — the
    /// dominant per-solve DEM cost. `Mutex` (not `RefCell`) because
    /// `Elevation: Send + Sync`; the solver is single-threaded so the
    /// lock is always uncontended and far cheaper than a DEM sample.
    memo: std::sync::Mutex<Vec<f32>>,
}

impl DemElevation {
    pub fn new(dem: Arc<Dem>) -> Self {
        Self {
            dem,
            memo: std::sync::Mutex::new(Vec::new()),
        }
    }
}

impl Elevation for DemElevation {
    fn at(&self, shape: &GridShape, i: u32, j: u32) -> Option<f32> {
        let nx = shape.nx as usize;
        let idx = (j as usize) * nx + (i as usize);
        {
            let mut m = self.memo.lock().unwrap();
            if m.is_empty() {
                m.resize(nx * (shape.ny as usize), f32::INFINITY);
            }
            let cached = m[idx];
            if cached != f32::INFINITY {
                return if cached.is_nan() { None } else { Some(cached) };
            }
        }
        let (x, y) = shape.cell_centre(i, j);
        let v = self.dem.sample(PointXY { x, y }).ok().flatten();
        self.memo.lock().unwrap()[idx] = v.unwrap_or(f32::NAN);
        v
    }
}

/// Lazy `CellOverlay`: evaluates the project's `CostContributor` stack
/// (water/glacier refusal + trail-proximity/slope pace) for a cell ONLY when
/// the lifted A* first asks about it, memoising the result. On a long route
/// the search touches a fraction of the corridor, so this avoids the eager
/// whole-corridor bake — speeding the solve and letting progress snapshots
/// stream from the first step instead of after a silent bake.
/// Append the off-trail roughness contributor (a multiplicative pace
/// factor on mesh edges) to a per-request contributor stack. Off-trail
/// roughness is per-request/profile (`inputs.off_trail_factor`), so it
/// is added here at solve time rather than baked into the static
/// Pathfinder stack — and it never touches graph (trail) edges. Used by
/// the grade-limited (default) and isotropic FMM paths; the anisotropic
/// path keeps `off_trail_factor` in its metric instead.
pub(crate) fn with_off_trail(
    contributors: &[Arc<dyn CostContributor>],
    off_trail_factor: f32,
) -> Vec<Arc<dyn CostContributor>> {
    let mut v: Vec<Arc<dyn CostContributor>> = contributors.to_vec();
    v.push(Arc::new(
        crate::native_contributors::OffTrailRoughnessContributor::new(off_trail_factor),
    ));
    v
}

pub(crate) struct LazyContributorOverlay<'a> {
    shape: GridShape,
    /// For the shared `EdgeElevProbe` handed to the contributor stack
    /// — one DEM pass per cell instead of one per slope contributor.
    dem: Arc<Dem>,
    base_pace: f32,
    profile: turbo_tiles_graph::Profile,
    contributors: &'a [Arc<dyn CostContributor>],
    // 0 = uncomputed, 1 = passable, 2 = refused.
    state: std::cell::RefCell<Vec<u8>>,
    mul: std::cell::RefCell<Vec<f32>>,
    refused_by: std::cell::RefCell<std::collections::BTreeSet<String>>,
}

impl<'a> LazyContributorOverlay<'a> {
    pub(crate) fn new(
        shape: GridShape,
        dem: Arc<Dem>,
        base_pace: f32,
        profile: turbo_tiles_graph::Profile,
        contributors: &'a [Arc<dyn CostContributor>],
    ) -> Self {
        let n = (shape.nx as usize) * (shape.ny as usize);
        Self {
            shape,
            dem,
            base_pace,
            profile,
            contributors,
            state: std::cell::RefCell::new(vec![0u8; n]),
            mul: std::cell::RefCell::new(vec![1.0f32; n]),
            refused_by: std::cell::RefCell::new(std::collections::BTreeSet::new()),
        }
    }

    #[inline]
    fn idx(&self, i: u32, j: u32) -> usize {
        (j as usize) * (self.shape.nx as usize) + (i as usize)
    }

    fn ensure(&self, i: u32, j: u32) {
        let idx = self.idx(i, j);
        if self.state.borrow()[idx] != 0 {
            return;
        }
        let cell_m = self.shape.cell_m;
        let (cx, cy) = self.shape.cell_centre(i, j);
        // ONE shared elevation probe for the whole contributor stack:
        // the slope-family contributors all sample the same points
        // along this synthetic cell edge.
        let probe = crate::contributor::EdgeElevProbe::new(
            &self.dem,
            cx - 0.5 * cell_m,
            cy,
            cx + 0.5 * cell_m,
            cy,
        );
        let ctx = EdgeContext {
            fx: cx - 0.5 * cell_m,
            fy: cy,
            tx: cx + 0.5 * cell_m,
            ty: cy,
            length_m: cell_m,
            profile: self.profile,
            kind: EdgeKind::Mesh,
            elev_probe: Some(&probe),
        };
        for c in self.contributors {
            if let Some(label) = c.veto(&ctx) {
                self.state.borrow_mut()[idx] = 2;
                self.refused_by.borrow_mut().insert(label.to_string());
                return;
            }
        }
        let mut extra_s: f64 = 0.0;
        let mut factor: f64 = 1.0;
        for c in self.contributors {
            let dv = c.contribute(&ctx);
            if dv.is_finite() {
                extra_s += dv;
            }
            let f = c.pace_factor(&ctx);
            if f.is_finite() && f > 0.0 {
                factor *= f;
            }
        }
        let extra_pace = (extra_s / cell_m) as f32;
        // Additive deltas scale the base pace; multiplicative pace
        // factors (off-trail roughness, …) scale the whole composed
        // pace — matching the solver's former `tobler × off × mul`.
        let mul = ((self.base_pace + extra_pace) / self.base_pace) as f64 * factor;
        self.mul.borrow_mut()[idx] = (mul as f32).clamp(0.1, 20.0);
        self.state.borrow_mut()[idx] = 1;
    }

    fn refused_count(&self) -> u32 {
        self.state.borrow().iter().filter(|&&s| s == 2).count() as u32
    }

    pub(crate) fn refused_labels(&self) -> Vec<String> {
        self.refused_by.borrow().iter().cloned().collect()
    }
}

impl<'a> turbo_tiles_fmm::CellOverlay for LazyContributorOverlay<'a> {
    fn refused(&self, i: u32, j: u32) -> bool {
        self.ensure(i, j);
        self.state.borrow()[self.idx(i, j)] == 2
    }
    fn pace_mul(&self, i: u32, j: u32) -> f32 {
        self.ensure(i, j);
        let idx = self.idx(i, j);
        if self.state.borrow()[idx] == 2 {
            1.0
        } else {
            self.mul.borrow()[idx]
        }
    }
}

/// Inputs to `solve_fmm_corridor`. Mirrors the off-trail-solve API
/// shape used by the existing `Pathfinder::build_off_trail_segment`
/// so the phase 6 dispatch swap is a near-drop-in.
#[derive(Debug, Clone)]
pub struct FmmSolveInputs {
    /// Start point in UTM33N (metres).
    pub from: PointXY,
    /// Goal point in UTM33N (metres).
    pub to: PointXY,
    /// Cell size for the FMM grid. 10 m matches the native DEM
    /// resolution; smaller values blow up memory + solve time
    /// faster than they improve accuracy.
    pub cell_m: f64,
    /// Pace floor used by `ToblerIsotropic::base_pace_s_per_m`.
    pub base_pace_s_per_m: f32,
    /// Slope threshold past which cells are refused. Matches the
    /// project-wide `slope_cell.refuse_above_deg` cost-config knob.
    pub refuse_above_deg: f32,
    /// Off-trail factor applied to every cell pace. The Pathfinder
    /// dispatch (phase 6) sets this from `prefs.off_trail_base`.
    pub off_trail_factor: f32,
    /// Switch to the phase-5 anisotropic Tobler-Finsler metric +
    /// AGSI stencil. When `false`, the legacy isotropic Tobler path
    /// runs (still useful for A/B screenshots and as a safety net).
    pub use_anisotropic: bool,
    /// Naismith vertical-gain weight (effective flat-metres per gain-
    /// metre) folded directionally into the anisotropic along-fall-line
    /// pace, matching on-graph `length + k·gain` pricing. Foot ≈ 8.
    pub gain_factor_k: f32,
    /// Use the state-augmented (x,y,heading) grade-limited solver, which
    /// switchbacks up steep ground instead of climbing the fall line.
    /// When `false`, the 2D anisotropic FMM runs (the gentle-terrain
    /// default).
    pub use_grade_limited: bool,
    /// Grade cap (deg) for the grade-limited solver: forward moves
    /// steeper than this are refused, forcing traverses/switchbacks.
    pub max_grade_deg: f32,
    /// Seconds charged per 45° heading change in the grade-limited
    /// solver. Tunes switchback spacing.
    pub turn_penalty_s: f32,
}

impl Default for FmmSolveInputs {
    fn default() -> Self {
        Self {
            from: PointXY { x: 0.0, y: 0.0 },
            to: PointXY { x: 0.0, y: 0.0 },
            cell_m: 10.0,
            base_pace_s_per_m: 0.714,
            refuse_above_deg: 45.0,
            off_trail_factor: 1.0,
            use_anisotropic: false,
            gain_factor_k: 0.0,
            use_grade_limited: false,
            max_grade_deg: 27.0,
            turn_penalty_s: 8.0,
        }
    }
}

/// Outputs from one corridor solve.
#[derive(Debug)]
pub struct FmmSolveOutput {
    /// Arrival-time field. Refused / unreached cells are `+∞`.
    pub arrival: FmmGrid<f32>,
    /// Shape (= bbox) of the corridor grid.
    pub shape: GridShape,
    /// FMM grid cell containing the `to` (goal) point. `None`
    /// when `to` fell outside the corridor (shouldn't happen
    /// given our corridor formula but defended defensively).
    pub goal_cell: Option<(u32, u32)>,
    /// Wall time of the solve loop in milliseconds. For
    /// diagnostics; the corridor bake isn't included.
    pub solve_ms: u32,
    /// How many cells were ACCEPTED. Useful for tracking how much
    /// the causal heuristic will save (phase 6).
    pub cells_accepted: u32,
    /// How many cells were refused by the per-cell veto pass
    /// (water polygon hits, glacier mask, etc.). Diagnostic only.
    pub vetoed_cells: u32,
    /// Labels of the contributors that vetoed at least one cell.
    pub refused_by: Vec<String>,
    /// Baked anisotropic CellForms (the per-cell dual metric, Selling-
    /// decomposed). Present only for anisotropic solves; consumed by
    /// `extract_path_aniso` to follow the metric characteristic. `None`
    /// for isotropic solves (extraction uses the plain gradient).
    pub forms: Option<FmmGrid<CellForm>>,
}

#[derive(Debug, thiserror::Error)]
pub enum FmmAdapterError {
    #[error("corridor is degenerate (start ≈ goal): {0}")]
    DegenerateCorridor(String),
    #[error("start point fell outside the corridor grid")]
    StartOutsideGrid,
    #[error("goal point fell outside the corridor grid")]
    GoalOutsideGrid,
    #[error("solve returned no finite arrival at the goal cell")]
    GoalUnreachable,
}

/// Compute the corridor bbox for a from→to solve.
///
/// The corridor is an *axis-aligned* (UTM-grid-aligned) rectangle
/// that fully contains the oriented `from→to` centerline rectangle
/// of half-width `corridor_half_width_m + pad`. We do not rotate
/// the grid — keeping it axis-aligned lets neighbour indexing stay
/// trivial in the stencil hot loop. The bloat is at most
/// `sin(angle) × length` (worst case 45°), which for a 5 km solve
/// at 10 m cells is ~3500 extra cells in each dimension. Acceptable.
///
/// Margin formula (matches the legacy mesh builder):
///   `pad = max(4·cell_m, 0.30 · ‖to − from‖)`
///   `half_width = max(800 m, 0.20 · ‖to − from‖)`
/// Corridor extent in the *along-direction*: `‖to − from‖ + 2·pad`.
/// Corridor extent in the *cross-direction*: `2·(half_width + pad)`.
/// We project both bounding box corners onto UTM-aligned coords by
/// taking the AABB of the rotated rectangle's corners.
pub fn compute_corridor_shape(
    from: PointXY,
    to: PointXY,
    cell_m: f64,
) -> Result<GridShape, FmmAdapterError> {
    let dx = to.x - from.x;
    let dy = to.y - from.y;
    let d = (dx * dx + dy * dy).sqrt();
    if d < cell_m * 0.5 {
        return Err(FmmAdapterError::DegenerateCorridor(format!(
            "distance {:.2} m is below half a cell ({:.2} m)",
            d,
            cell_m * 0.5
        )));
    }
    // Corridor extents are clamped so the search area grows O(d), not O(d²).
    // Without the caps a long route's padding/width scale with length and the
    // cell count explodes (an 11.5 km route became a ~21×20 km, 4.4 M-cell
    // bbox). 3 km is generous enough to detour around large lakes while
    // bounding the corridor for long routes. A* keeps the wider span cheap by
    // exploring toward the goal rather than flooding it.
    let pad = (4.0 * cell_m).max(0.30 * d).min(3000.0);
    let half_width = 800.0_f64.max(0.20 * d).min(3000.0);
    // Compute AABB of the oriented rectangle. Easier in unit-vector
    // form: along-axis u = (dx/d, dy/d); perp-axis v = (-dy/d, dx/d).
    let u = (dx / d, dy / d);
    let v = (-u.1, u.0);
    let along = d * 0.5 + pad;
    let cross = half_width + pad;
    // Rectangle centre.
    let cx = (from.x + to.x) * 0.5;
    let cy = (from.y + to.y) * 0.5;
    // Four corners in world coordinates.
    let corners = [
        (
            cx + along * u.0 + cross * v.0,
            cy + along * u.1 + cross * v.1,
        ),
        (
            cx + along * u.0 - cross * v.0,
            cy + along * u.1 - cross * v.1,
        ),
        (
            cx - along * u.0 + cross * v.0,
            cy - along * u.1 + cross * v.1,
        ),
        (
            cx - along * u.0 - cross * v.0,
            cy - along * u.1 - cross * v.1,
        ),
    ];
    let mut min_x = f64::INFINITY;
    let mut max_x = f64::NEG_INFINITY;
    let mut min_y = f64::INFINITY;
    let mut max_y = f64::NEG_INFINITY;
    for (x, y) in corners {
        if x < min_x {
            min_x = x;
        }
        if x > max_x {
            max_x = x;
        }
        if y < min_y {
            min_y = y;
        }
        if y > max_y {
            max_y = y;
        }
    }
    // Snap origin to grid alignment so cell centres land at
    // consistent fractions of `cell_m`. Doesn't affect correctness
    // but makes per-cell DEM samples reproducible.
    let origin_x = (min_x / cell_m).floor() * cell_m;
    let origin_y = (min_y / cell_m).floor() * cell_m;
    let nx = ((max_x - origin_x) / cell_m).ceil() as u32 + 1;
    let ny = ((max_y - origin_y) / cell_m).ceil() as u32 + 1;
    Ok(GridShape::new_2d(nx, ny, origin_x, origin_y, cell_m))
}

/// Fold additive walk-seconds contributions from the contributor
/// stack into the per-cell pace field. Each cell's pace is updated:
///
///   pace_new = max(base_pace, tobler_pace + Σ contribute_per_metre)
///         * off_trail_factor
///
/// Contributors whose `contribute()` returns infinity (encoded
/// veto) are skipped here — `bake_vetoes` handles those. Negative
/// contributions (trail-proximity bonus, marking bonus on graph
/// edges, etc.) speed the wave; positive contributions slow it.
/// Pace floor at the configured `base_pace_s_per_m` so the wave
/// can't somehow become *faster* than the flat-trail baseline,
/// which would violate the rest of the cost model's calibration.
fn bake_contributor_pace(
    shape: GridShape,
    cost: &mut FmmGrid<f32>,
    contributors: &[Arc<dyn CostContributor>],
    profile: turbo_tiles_graph::Profile,
    base_pace_s_per_m: f32,
) {
    let cell_m = shape.cell_m;
    for j in 0..shape.ny {
        for i in 0..shape.nx {
            let idx = shape.idx(i, j, 0);
            // Skip cells already refused by slope or by `bake_vetoes`
            // (we run the latter after this fn, but be defensive).
            if !cost.flat()[idx].is_finite() {
                continue;
            }
            let (cx, cy) = shape.cell_centre(i, j);
            let ctx = EdgeContext {
                fx: cx - 0.5 * cell_m,
                fy: cy,
                tx: cx + 0.5 * cell_m,
                ty: cy,
                length_m: cell_m,
                profile,
                kind: EdgeKind::Mesh,
                elev_probe: None,
            };
            // Sum walk-seconds contributions; skip Inf (vetoes). Also
            // accumulate the multiplicative pace factors (off-trail
            // roughness, …) that scale the whole composed pace.
            let mut extra_s: f64 = 0.0;
            let mut factor: f64 = 1.0;
            for c in contributors {
                let dv = c.contribute(&ctx);
                if dv.is_finite() {
                    extra_s += dv;
                }
                let f = c.pace_factor(&ctx);
                if f.is_finite() && f > 0.0 {
                    factor *= f;
                }
            }
            let extra_pace = (extra_s / cell_m) as f32; // s/m delta
            let tobler_pace = cost.flat()[idx];
            let composed = (tobler_pace + extra_pace).max(base_pace_s_per_m);
            cost.flat_mut()[idx] = composed * factor as f32;
        }
    }
}

/// Walk the `CostContributor` list once per cell, marking any cell
/// that any contributor vetoes as refused in the per-cell cost grid.
/// Returns the refused-cell count plus a deduped list of contributor
/// names that vetoed at least one cell.
fn bake_vetoes(
    shape: GridShape,
    cost: &mut FmmGrid<f32>,
    contributors: &[Arc<dyn CostContributor>],
    profile: turbo_tiles_graph::Profile,
) -> (u32, Vec<String>) {
    use std::collections::BTreeSet;
    let mut refused = 0u32;
    let mut who: BTreeSet<String> = BTreeSet::new();
    let cell_m = shape.cell_m;
    for j in 0..shape.ny {
        for i in 0..shape.nx {
            let (cx, cy) = shape.cell_centre(i, j);
            // Build a tiny synthetic mesh edge at the cell centre.
            // Length is one cell so vector polygon-integral
            // contributors see a representative segment, not a
            // degenerate zero-length one.
            let ctx = EdgeContext {
                fx: cx - 0.5 * cell_m,
                fy: cy,
                tx: cx + 0.5 * cell_m,
                ty: cy,
                length_m: cell_m,
                profile,
                kind: EdgeKind::Mesh,
                elev_probe: None,
            };
            for c in contributors {
                if let Some(label) = c.veto(&ctx) {
                    let idx = shape.idx(i, j, 0);
                    if cost.flat()[idx].is_finite() {
                        cost.flat_mut()[idx] = f32::INFINITY;
                        refused += 1;
                    }
                    who.insert(label.to_string());
                    break;
                }
            }
        }
    }
    (refused, who.into_iter().collect())
}

/// Apply contributor pace deltas to an anisotropic CellForm grid.
/// We scale the AGSI weights by `1 / (1 + δ/τ_base)²` so the eikonal
/// arrival time grows by the multiplicative factor `(1 + δ/τ_base)`.
/// This treats the contributor delta as isotropic on top of the
/// anisotropic Tobler base — accurate for non-directional layers
/// (wetland, trail-proximity, cultivated landcover); a fully
/// direction-aware version would land in a phase-5b follow-up.
fn apply_contributor_factors_aniso(
    shape: GridShape,
    forms: &mut FmmGrid<CellForm>,
    contributors: &[Arc<dyn CostContributor>],
    profile: turbo_tiles_graph::Profile,
    base_pace_s_per_m: f32,
) {
    let cell_m = shape.cell_m;
    for j in 0..shape.ny {
        for i in 0..shape.nx {
            let idx = shape.idx(i, j, 0);
            let cell = forms.flat()[idx];
            if cell.is_refused() {
                continue;
            }
            let (cx, cy) = shape.cell_centre(i, j);
            let ctx = EdgeContext {
                fx: cx - 0.5 * cell_m,
                fy: cy,
                tx: cx + 0.5 * cell_m,
                ty: cy,
                length_m: cell_m,
                profile,
                kind: EdgeKind::Mesh,
                elev_probe: None,
            };
            let mut extra_s: f64 = 0.0;
            let mut factor: f64 = 1.0;
            for c in contributors {
                let dv = c.contribute(&ctx);
                if dv.is_finite() {
                    extra_s += dv;
                }
                let pf = c.pace_factor(&ctx);
                if pf.is_finite() && pf > 0.0 {
                    factor *= pf;
                }
            }
            if extra_s.abs() < 1e-6 && (factor - 1.0).abs() < 1e-6 {
                continue;
            }
            let extra_pace = (extra_s / cell_m) as f32;
            let f = (((base_pace_s_per_m + extra_pace) / base_pace_s_per_m) as f64 * factor)
                .max(0.1) as f32;
            let scale = 1.0 / (f * f);
            let mut new_form = cell;
            for k in 0..new_form.norm.n_terms as usize {
                new_form.norm.weights[k] *= scale;
            }
            forms.flat_mut()[idx] = new_form;
        }
    }
}

/// Mark refused cells in an anisotropic CellForm grid via the
/// contributor veto pass. Returns counts + a deduped contributor
/// label set, mirroring the isotropic `bake_vetoes`.
fn bake_vetoes_aniso(
    shape: GridShape,
    forms: &mut FmmGrid<CellForm>,
    contributors: &[Arc<dyn CostContributor>],
    profile: turbo_tiles_graph::Profile,
) -> (u32, Vec<String>) {
    use std::collections::BTreeSet;
    let mut refused = 0u32;
    let mut who: BTreeSet<String> = BTreeSet::new();
    let cell_m = shape.cell_m;
    for j in 0..shape.ny {
        for i in 0..shape.nx {
            let (cx, cy) = shape.cell_centre(i, j);
            let ctx = EdgeContext {
                fx: cx - 0.5 * cell_m,
                fy: cy,
                tx: cx + 0.5 * cell_m,
                ty: cy,
                length_m: cell_m,
                profile,
                kind: EdgeKind::Mesh,
                elev_probe: None,
            };
            for c in contributors {
                if let Some(label) = c.veto(&ctx) {
                    let idx = shape.idx(i, j, 0);
                    if !forms.flat()[idx].is_refused() {
                        forms.flat_mut()[idx] = CellForm::refused();
                        refused += 1;
                    }
                    who.insert(label.to_string());
                    break;
                }
            }
        }
    }
    (refused, who.into_iter().collect())
}

/// Anisotropic-Tobler corridor solve: bake CellForms via Selling
/// reduction of the Tobler-Finsler metric tensor, apply contributor
/// scalings + vetoes, then run the AGSI stencil. This is the phase-5
/// entry point — the architectural moat that produces contour-
/// following geodesics instead of "shortest distance over peaks".
fn solve_fmm_corridor_aniso(
    inputs: &FmmSolveInputs,
    dem: Arc<Dem>,
    contributors: &[Arc<dyn CostContributor>],
    profile: turbo_tiles_graph::Profile,
) -> Result<FmmSolveOutput, FmmAdapterError> {
    let shape = compute_corridor_shape(inputs.from, inputs.to, inputs.cell_m)?;
    let start_cell = shape
        .world_to_cell(inputs.from.x, inputs.from.y)
        .ok_or(FmmAdapterError::StartOutsideGrid)?;
    let goal_cell = shape
        .world_to_cell(inputs.to.x, inputs.to.y)
        .ok_or(FmmAdapterError::GoalOutsideGrid)?;

    let elev = DemElevation::new(dem.clone());
    let metric = ToblerAnisotropic {
        elev,
        refuse_above_deg: inputs.refuse_above_deg,
        base_pace_s_per_m: inputs.base_pace_s_per_m,
        off_trail_factor: inputs.off_trail_factor,
        gain_factor_k: inputs.gain_factor_k,
    };
    let mut forms = bake_aniso_corridor(shape, &metric);
    apply_contributor_factors_aniso(
        shape,
        &mut forms,
        contributors,
        profile,
        inputs.base_pace_s_per_m * inputs.off_trail_factor,
    );
    let (vetoed_cells, refused_by) = bake_vetoes_aniso(shape, &mut forms, contributors, profile);

    let t0 = std::time::Instant::now();
    let result = solve_2d_anisotropic(
        shape,
        &forms,
        &[(start_cell.0, start_cell.1, 0.0)],
        StopCondition::GoalReached {
            gi: goal_cell.0,
            gj: goal_cell.1,
        },
    );
    let solve_ms = t0.elapsed().as_millis() as u32;

    if !result.arrival.get(goal_cell.0, goal_cell.1, 0).is_finite() {
        // Expected when the off-trail corridor between the endpoints is
        // severed by a refused barrier (cliff > refuse_above_deg, water,
        // glacier) — common in alpine terrain where the marked trail
        // exists precisely to switchback/bridge around the barrier. The
        // caller falls back to the Theta* mesh. Verified on the Norway
        // corpus: every force-off-trail "goal unreachable" was a genuine
        // refused-cell disconnection (goal not in the start's connected
        // component), not a solver stall.
        let total = (shape.nx * shape.ny) as usize;
        let refused: usize = forms.flat().iter().filter(|c| c.is_refused()).count();
        tracing::warn!(
            nx = shape.nx, ny = shape.ny, total, refused, vetoed_cells,
            refused_by = ?refused_by,
            cells_accepted = result.cells_accepted,
            "FMM aniso: goal unreachable (corridor severed by refused cells)"
        );
        return Err(FmmAdapterError::GoalUnreachable);
    }

    Ok(FmmSolveOutput {
        arrival: result.arrival,
        shape,
        goal_cell: Some(goal_cell),
        solve_ms,
        cells_accepted: result.cells_accepted,
        vetoed_cells,
        refused_by,
        forms: Some(forms),
    })
}

/// Build the corridor bbox, bake a cost field, run the FMM solve.
/// Phase-3 entry point. Path extraction comes in phase 4.
pub fn solve_fmm_corridor(
    inputs: FmmSolveInputs,
    dem: Arc<Dem>,
    contributors: &[Arc<dyn CostContributor>],
    profile: turbo_tiles_graph::Profile,
) -> Result<FmmSolveOutput, FmmAdapterError> {
    if inputs.use_anisotropic {
        return solve_fmm_corridor_aniso(&inputs, dem, contributors, profile);
    }
    let shape = compute_corridor_shape(inputs.from, inputs.to, inputs.cell_m)?;
    let start_cell = shape
        .world_to_cell(inputs.from.x, inputs.from.y)
        .ok_or(FmmAdapterError::StartOutsideGrid)?;
    let goal_cell = shape
        .world_to_cell(inputs.to.x, inputs.to.y)
        .ok_or(FmmAdapterError::GoalOutsideGrid)?;

    // 1) Bake the per-cell pace from the Tobler metric. This is
    //    the slope-aware baseline; non-slope contributors are
    //    folded in below.
    let elev = DemElevation::new(dem.clone());
    let metric = ToblerIsotropic {
        elev,
        refuse_above_deg: inputs.refuse_above_deg,
        base_pace_s_per_m: inputs.base_pace_s_per_m,
        off_trail_factor: 1.0, // off_trail_factor folded in below to compose with contributor deltas
    };
    let mut cost = bake_metric_2d(&metric, shape);

    // 2) Bake the additive walk-seconds contributions from the
    //    full CostContributor stack into the same pace field. For
    //    each cell, run `compose_edge_walk_seconds` on a synthetic
    //    1 m mesh edge at the cell centre and divide by 1 m to get
    //    a per-cell pace delta. Trail-proximity bonus (negative)
    //    speeds the wave on cells near graph edges; wetland /
    //    cultivated landcover slow it. This is what makes the
    //    FMM path actually follow trails — without it the cost
    //    field is purely slope-driven and routes around graph
    //    edges instead of toward them.
    //    Off-trail roughness rides along as a multiplicative
    //    `pace_factor` contributor (added here, per-request) so the
    //    factor is applied uniformly by the cost stack rather than
    //    hard-coded into the solver.
    let augmented = with_off_trail(contributors, inputs.off_trail_factor);
    bake_contributor_pace(
        shape,
        &mut cost,
        &augmented,
        profile,
        inputs.base_pace_s_per_m,
    );

    // 3) Bake hard refusals on top — water/ocean/glacier/building
    //    polygons. Sets `cost = +∞` for vetoed cells.
    let (vetoed_cells, refused_by) = bake_vetoes(shape, &mut cost, &augmented, profile);

    // 3) Solve.
    let t0 = std::time::Instant::now();
    let result = solve_2d_isotropic(
        shape,
        &cost,
        &[(start_cell.0, start_cell.1, 0.0)],
        StopCondition::GoalReached {
            gi: goal_cell.0,
            gj: goal_cell.1,
        },
    );
    let solve_ms = t0.elapsed().as_millis() as u32;

    // Defensive: if the goal cell ended up unreachable (cost field
    // surrounded by refused cells), report it explicitly. The phase
    // 6 dispatch will fall back to Theta\* on this error.
    if !result.arrival.get(goal_cell.0, goal_cell.1, 0).is_finite() {
        return Err(FmmAdapterError::GoalUnreachable);
    }

    Ok(FmmSolveOutput {
        arrival: result.arrival,
        shape,
        goal_cell: Some(goal_cell),
        solve_ms,
        cells_accepted: result.cells_accepted,
        vetoed_cells,
        refused_by,
        forms: None,
    })
}

/// A complete off-trail FMM solve: corridor + bake + solve +
/// gradient-descent extract + cost-aware Chaikin smooth.
///
/// This is the function Phase 6 dispatch wires into the off-trail
/// strategy of `Pathfinder`. The returned `polyline` is in UTM33N
/// metres, ordered start → goal. `cost_seconds` is the arrival
/// time at the goal cell, which (because the Tobler metric returns
/// pace in s/m) is directly comparable to walk-seconds used by
/// the other Pathfinder strategies.
pub struct FmmPathOutput {
    /// Smoothed polyline, start → goal, in EPSG:25833 metres.
    pub polyline: Vec<PathPoint>,
    /// Total walk-time cost from FMM (= arrival at goal cell).
    pub cost_seconds: f64,
    /// Diagnostic stats from the underlying solve.
    pub solve_ms: u32,
    pub cells_accepted: u32,
    pub vetoed_cells: u32,
    pub refused_by: Vec<String>,
}

pub fn solve_fmm_path(
    inputs: FmmSolveInputs,
    dem: Arc<Dem>,
    contributors: &[Arc<dyn CostContributor>],
    profile: turbo_tiles_graph::Profile,
) -> Result<FmmPathOutput, FmmAdapterError> {
    if inputs.use_grade_limited {
        return solve_grade_limited_path(inputs, dem, contributors, profile);
    }
    let from = inputs.from;
    let to = inputs.to;
    let use_aniso = inputs.use_anisotropic;
    let solve = solve_fmm_corridor(inputs, dem, contributors, profile)?;
    let start_pp = PathPoint {
        x: from.x,
        y: from.y,
    };
    let goal_pp = PathPoint { x: to.x, y: to.y };
    // Anisotropic corridors must extract along the metric characteristic
    // (-G*∇u), not the raw gradient (-∇u): on an anisotropic field the
    // two diverge by a consistent angle that accumulates into ballooning
    // detours. `extract_path_aniso` reconstructs G* from the baked
    // CellForms. Isotropic corridors keep the plain extractor (G* ∝ I,
    // so the two coincide).
    // Discrete steepest-descent extraction: provably convergent on the
    // monotone arrival field (cannot loop/diverge), where the continuous
    // gradient descent oscillated near walls and reported `Diverged` on
    // long/steep corridors → blocky Theta* fallback. The coarse cell path
    // is smoothed into an organic curve below.
    let _ = use_aniso;
    let extract_res = extract_path_discrete(&solve.shape, &solve.arrival, start_pp, goal_pp);
    let raw_path = extract_res.map_err(|e| {
        tracing::warn!(error = ?e, use_aniso, "FMM extraction failed");
        match e {
            turbo_tiles_fmm::ExtractError::GoalUnreachable => FmmAdapterError::GoalUnreachable,
            _ => FmmAdapterError::GoalUnreachable, // Bucket all extract errors
        }
    })?;
    // Cost-aware Chaikin smooth. The discrete extractor yields a coarse
    // (cell-centre, 8-direction) staircase, so use 4 iterations to round
    // it into an organic curve; cost-awareness keeps it from drifting
    // across refused/expensive cells.
    let smoothed = chaikin_smooth_cost_aware(&raw_path, &solve.arrival, &solve.shape, 4, 16384);
    let goal_cell = solve.goal_cell.expect("solve guarantees goal_cell");
    let cost_seconds = solve.arrival.get(goal_cell.0, goal_cell.1, 0) as f64;
    Ok(FmmPathOutput {
        polyline: smoothed,
        cost_seconds,
        solve_ms: solve.solve_ms,
        cells_accepted: solve.cells_accepted,
        vetoed_cells: solve.vetoed_cells,
        refused_by: solve.refused_by,
    })
}

/// State-augmented (x, y, heading) grade-limited solve: switchbacks up
/// steep terrain instead of climbing the fall line. Reuses the corridor
/// sizing; lifts to `N_HEADINGS` heading bands; Dijkstra over the lifted
/// lattice; backtracks the parent tree; Chaikin-smooths.
fn solve_grade_limited_path(
    inputs: FmmSolveInputs,
    dem: Arc<Dem>,
    contributors: &[Arc<dyn CostContributor>],
    profile: turbo_tiles_graph::Profile,
) -> Result<FmmPathOutput, FmmAdapterError> {
    use turbo_tiles_fmm::{
        extract_path_lifted, solve_lifted_grade_limited, GradeLimitedCost, N_HEADINGS,
    };
    let shape2d = compute_corridor_shape(inputs.from, inputs.to, inputs.cell_m)?;
    let start_cell = shape2d
        .world_to_cell(inputs.from.x, inputs.from.y)
        .ok_or(FmmAdapterError::StartOutsideGrid)?;
    let goal_cell = shape2d
        .world_to_cell(inputs.to.x, inputs.to.y)
        .ok_or(FmmAdapterError::GoalOutsideGrid)?;
    let shape3d = GridShape::new_3d(
        shape2d.nx,
        shape2d.ny,
        N_HEADINGS,
        shape2d.origin_x,
        shape2d.origin_y,
        shape2d.cell_m,
    );

    // Lazy per-cell overlay over the SAME CostContributor stack the 2-D FMM
    // uses (water/glacier refusal + trail-proximity/slope pace). Evaluated on
    // demand as the A* visits cells — no eager whole-corridor bake — so long
    // routes only pay for cells they actually explore, and progress streams
    // from the first step.
    // Off-trail roughness joins the stack as a multiplicative
    // `pace_factor` contributor (per-request, mesh-only), so the
    // overlay's `mul` carries it — the solver no longer multiplies by
    // `off_trail_factor` in `forward_cost`. It survives on
    // `GradeLimitedCost` purely as the A* heuristic's pace floor.
    let augmented = with_off_trail(contributors, inputs.off_trail_factor);
    let overlay = LazyContributorOverlay::new(
        shape2d,
        dem.clone(),
        inputs.base_pace_s_per_m,
        profile,
        &augmented,
    );

    let cost = GradeLimitedCost {
        elev: DemElevation::new(dem.clone()),
        base_pace_s_per_m: inputs.base_pace_s_per_m,
        off_trail_factor: inputs.off_trail_factor,
        max_grade_deg: inputs.max_grade_deg,
        turn_penalty_s: inputs.turn_penalty_s,
        overlay,
    };
    // Stream the route reaching toward the goal as the A* runs: each
    // progress snapshot becomes a BestPathSnapshot solver-trace event, which
    // fans to the SSE channel when a streaming recorder is installed (and is
    // a cheap no-op otherwise). This is what makes the live preview show a
    // trail being built for off-trail solves — previously only the graph
    // Dijkstra emitted events.
    let mut emit = |p: &turbo_tiles_fmm::LiftedProgress| {
        crate::solver_trace::record(|| crate::solver_trace::SolverEvent::BestPathSnapshot {
            coords: p
                .best_path
                .iter()
                .map(|&(x, y)| [x as f32, y as f32])
                .collect(),
        });
    };
    let t0 = std::time::Instant::now();
    let result = solve_lifted_grade_limited(shape3d, &cost, start_cell, goal_cell, Some(&mut emit));
    let solve_ms = t0.elapsed().as_millis() as u32;

    let goal_state = result.goal_state.ok_or(FmmAdapterError::GoalUnreachable)?;
    let raw_xy = extract_path_lifted(
        &shape3d,
        &result,
        (inputs.from.x, inputs.from.y),
        (inputs.to.x, inputs.to.y),
    )
    .ok_or(FmmAdapterError::GoalUnreachable)?;
    let raw_path: Vec<PathPoint> = raw_xy
        .into_iter()
        .map(|(x, y)| PathPoint { x, y })
        .collect();

    // Project the lifted arrival to a 2D min-over-heading field so the
    // cost-aware Chaikin smoother has a sensible scalar field.
    let mut arr2d: FmmGrid<f32> = FmmGrid::filled(shape2d, f32::INFINITY);
    for j in 0..shape2d.ny {
        for i in 0..shape2d.nx {
            let mut m = f32::INFINITY;
            for k in 0..N_HEADINGS {
                let v = result.arrival.get(i, j, k);
                if v < m {
                    m = v;
                }
            }
            arr2d.set(i, j, 0, m);
        }
    }
    // Refusal-repair on the smoothed output. The Chaikin smoother only
    // rejects bad *vertices*; a segment between two good vertices can still
    // clip a refused-cell corner (the residual 1-cell water/cliff clips the
    // gate caught). `forward_cost` guarantees the RAW lattice path is
    // segment-safe, so take the smoothest version that's fully clear,
    // dropping iterations and finally falling back to raw if a clip remains.
    let cell_m_f = shape2d.cell_m;
    let seg_clear = |poly: &[PathPoint]| -> bool {
        use turbo_tiles_fmm::CellOverlay;
        for w in poly.windows(2) {
            let d = ((w[1].x - w[0].x).powi(2) + (w[1].y - w[0].y).powi(2)).sqrt();
            let steps = ((d / (0.4 * cell_m_f)).ceil() as i32).max(1);
            for s in 0..=steps {
                let t = s as f64 / steps as f64;
                let x = w[0].x + (w[1].x - w[0].x) * t;
                let y = w[0].y + (w[1].y - w[0].y) * t;
                if let Some((ci, cj)) = shape2d.world_to_cell(x, y) {
                    // Cells the path visits are already memoised by the solve.
                    if cost.overlay.refused(ci, cj) {
                        return false;
                    }
                }
            }
        }
        true
    };
    let mut smoothed = chaikin_smooth_cost_aware(&raw_path, &arr2d, &shape2d, 4, 16384);
    if !seg_clear(&smoothed) {
        let mut repaired = None;
        for iters in [2u8, 1] {
            let cand = chaikin_smooth_cost_aware(&raw_path, &arr2d, &shape2d, iters, 16384);
            if seg_clear(&cand) {
                repaired = Some(cand);
                break;
            }
        }
        // Raw lattice path is segment-safe by construction (forward_cost
        // refuses any move whose chord clips a refused cell).
        smoothed = repaired.unwrap_or_else(|| raw_path.clone());
        tracing::debug!(
            "grade-limited: smoothed path clipped a refused cell; repaired to safe variant"
        );
    }
    let cost_seconds = result.arrival.flat()[goal_state] as f64;
    Ok(FmmPathOutput {
        polyline: smoothed,
        cost_seconds,
        solve_ms,
        cells_accepted: result.cells_accepted,
        vetoed_cells: cost.overlay.refused_count(),
        refused_by: cost.overlay.refused_labels(),
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    /// Degenerate corridor (from ≈ to) returns a sensible error
    /// rather than allocating a huge grid.
    #[test]
    fn degenerate_corridor_errors() {
        let from = PointXY { x: 100.0, y: 200.0 };
        let to = PointXY { x: 100.5, y: 200.5 };
        assert!(matches!(
            compute_corridor_shape(from, to, 10.0),
            Err(FmmAdapterError::DegenerateCorridor(_))
        ));
    }

    /// A 1 km horizontal corridor at 10 m cell size: along-axis
    /// extent ≥ 1 km + 2·pad = 1 km + 600 m = 1.6 km; cross-axis
    /// extent ≥ 2 × (800 + 300) = 2200 m. So the resulting nx is
    /// ~160-170, ny is ~220+ at 10 m cells.
    #[test]
    fn one_km_corridor_sized_sanely() {
        let from = PointXY { x: 0.0, y: 0.0 };
        let to = PointXY { x: 1000.0, y: 0.0 };
        let shape = compute_corridor_shape(from, to, 10.0).expect("should compute");
        // x extent: 1000 + 2·pad where pad = max(40, 300) = 300; → 1600 m
        assert!(
            shape.nx >= 160 && shape.nx <= 180,
            "nx out of expected range: {}",
            shape.nx
        );
        // y extent: 2 × (max(800, 200) + 300) = 2200 m → ny ≈ 220
        assert!(
            shape.ny >= 220 && shape.ny <= 240,
            "ny out of expected range: {}",
            shape.ny
        );
        // Origin snapped to a multiple of cell_m.
        assert!((shape.origin_x / shape.cell_m).fract().abs() < 1e-6);
        assert!((shape.origin_y / shape.cell_m).fract().abs() < 1e-6);
    }

    /// Same physical 1 km corridor but rotated 45° — the AABB-of-
    /// rotated-rectangle approach inflates both axes equally.
    #[test]
    fn rotated_corridor_inflates_aabb() {
        let from = PointXY { x: 0.0, y: 0.0 };
        let d = 1000.0_f64;
        let to = PointXY {
            x: d / 2.0_f64.sqrt(),
            y: d / 2.0_f64.sqrt(),
        };
        let shape = compute_corridor_shape(from, to, 10.0).expect("should compute");
        // The AABB grows by ~sin(45°) factor in each dimension.
        // Both nx and ny should be comfortably > the axis-aligned case.
        assert!(
            shape.nx > 180,
            "rotated corridor should inflate nx; got {}",
            shape.nx
        );
        assert!(
            shape.ny > 180,
            "rotated corridor should inflate ny; got {}",
            shape.ny
        );
    }
}
