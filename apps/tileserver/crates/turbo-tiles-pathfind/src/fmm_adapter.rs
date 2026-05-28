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
    bake_aniso_corridor, bake_metric_2d, chaikin_smooth_cost_aware, extract_path,
    solve_2d_anisotropic, solve_2d_isotropic, CellForm, Elevation, FmmGrid, GridShape,
    PathPoint, StopCondition, ToblerAnisotropic, ToblerIsotropic,
};

use crate::contributor::{CostContributor, EdgeContext, EdgeKind};

/// Implements `fmm::Elevation` against the project's `Arc<Dem>`.
/// Each cell sample goes through `Dem::sample(PointXY)`, which the
/// DEM crate's tile cache hot-paths efficiently. The adapter holds
/// the `Arc` so it can outlive the corridor solve.
pub struct DemElevation {
    pub dem: Arc<Dem>,
}

impl Elevation for DemElevation {
    fn at(&self, shape: &GridShape, i: u32, j: u32) -> Option<f32> {
        let (x, y) = shape.cell_centre(i, j);
        self.dem.sample(PointXY { x, y }).ok().flatten()
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
            d, cell_m * 0.5
        )));
    }
    let pad = (4.0 * cell_m).max(0.30 * d);
    let half_width = 800.0_f64.max(0.20 * d);
    // Compute AABB of the oriented rectangle. Easier in unit-vector
    // form: along-axis u = (dx/d, dy/d); perp-axis v = (-dy/d, dx/d).
    let u = (dx / d, dy / d);
    let v = (-u.1, u.0);
    let along = (d * 0.5 + pad) as f64;
    let cross = (half_width + pad) as f64;
    // Rectangle centre.
    let cx = (from.x + to.x) * 0.5;
    let cy = (from.y + to.y) * 0.5;
    // Four corners in world coordinates.
    let corners = [
        (cx + along * u.0 + cross * v.0, cy + along * u.1 + cross * v.1),
        (cx + along * u.0 - cross * v.0, cy + along * u.1 - cross * v.1),
        (cx - along * u.0 + cross * v.0, cy - along * u.1 + cross * v.1),
        (cx - along * u.0 - cross * v.0, cy - along * u.1 - cross * v.1),
    ];
    let mut min_x = f64::INFINITY;
    let mut max_x = f64::NEG_INFINITY;
    let mut min_y = f64::INFINITY;
    let mut max_y = f64::NEG_INFINITY;
    for (x, y) in corners {
        if x < min_x { min_x = x; }
        if x > max_x { max_x = x; }
        if y < min_y { min_y = y; }
        if y > max_y { max_y = y; }
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
    off_trail_factor: f32,
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
            };
            // Sum walk-seconds contributions; skip Inf (vetoes).
            let mut extra_s: f64 = 0.0;
            for c in contributors {
                let dv = c.contribute(&ctx);
                if dv.is_finite() {
                    extra_s += dv;
                }
            }
            let extra_pace = (extra_s / cell_m) as f32; // s/m delta
            let tobler_pace = cost.flat()[idx];
            let composed = (tobler_pace + extra_pace).max(base_pace_s_per_m);
            cost.flat_mut()[idx] = composed * off_trail_factor;
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
            if cell.is_refused() { continue; }
            let (cx, cy) = shape.cell_centre(i, j);
            let ctx = EdgeContext {
                fx: cx - 0.5 * cell_m,
                fy: cy,
                tx: cx + 0.5 * cell_m,
                ty: cy,
                length_m: cell_m,
                profile,
                kind: EdgeKind::Mesh,
            };
            let mut extra_s: f64 = 0.0;
            for c in contributors {
                let dv = c.contribute(&ctx);
                if dv.is_finite() {
                    extra_s += dv;
                }
            }
            if extra_s.abs() < 1e-6 { continue; }
            let extra_pace = (extra_s / cell_m) as f32;
            let f = ((base_pace_s_per_m + extra_pace) / base_pace_s_per_m).max(0.1);
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

    let elev = DemElevation { dem: dem.clone() };
    let metric = ToblerAnisotropic {
        elev,
        refuse_above_deg: inputs.refuse_above_deg,
        base_pace_s_per_m: inputs.base_pace_s_per_m,
        off_trail_factor: inputs.off_trail_factor,
    };
    let mut forms = bake_aniso_corridor(shape, &metric);
    apply_contributor_factors_aniso(
        shape, &mut forms, contributors, profile,
        inputs.base_pace_s_per_m * inputs.off_trail_factor,
    );
    let (vetoed_cells, refused_by) = bake_vetoes_aniso(shape, &mut forms, contributors, profile);

    let t0 = std::time::Instant::now();
    let result = solve_2d_anisotropic(
        shape,
        &forms,
        &[(start_cell.0, start_cell.1, 0.0)],
        StopCondition::GoalReached { gi: goal_cell.0, gj: goal_cell.1 },
    );
    let solve_ms = t0.elapsed().as_millis() as u32;

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
    let elev = DemElevation { dem: dem.clone() };
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
    bake_contributor_pace(shape, &mut cost, contributors, profile,
        inputs.base_pace_s_per_m, inputs.off_trail_factor);

    // 3) Bake hard refusals on top — water/ocean/glacier/building
    //    polygons. Sets `cost = +∞` for vetoed cells.
    let (vetoed_cells, refused_by) = bake_vetoes(shape, &mut cost, contributors, profile);

    // 3) Solve.
    let t0 = std::time::Instant::now();
    let result = solve_2d_isotropic(
        shape,
        &cost,
        &[(start_cell.0, start_cell.1, 0.0)],
        StopCondition::GoalReached { gi: goal_cell.0, gj: goal_cell.1 },
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
    let from = inputs.from;
    let to = inputs.to;
    let solve = solve_fmm_corridor(inputs, dem, contributors, profile)?;
    let start_pp = PathPoint { x: from.x, y: from.y };
    let goal_pp = PathPoint { x: to.x, y: to.y };
    let raw_path = extract_path(&solve.shape, &solve.arrival, start_pp, goal_pp, None, None)
        .map_err(|e| match e {
            turbo_tiles_fmm::ExtractError::GoalUnreachable => FmmAdapterError::GoalUnreachable,
            _ => FmmAdapterError::GoalUnreachable, // Bucket all extract errors
        })?;
    // Cost-aware Chaikin smooth. 2 iterations is the sweet spot —
    // visually smooth, doesn't drift across cost discontinuities.
    let smoothed = chaikin_smooth_cost_aware(&raw_path, &solve.arrival, &solve.shape, 2, 8192);
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
        assert!(shape.nx >= 160 && shape.nx <= 180,
            "nx out of expected range: {}", shape.nx);
        // y extent: 2 × (max(800, 200) + 300) = 2200 m → ny ≈ 220
        assert!(shape.ny >= 220 && shape.ny <= 240,
            "ny out of expected range: {}", shape.ny);
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
        let to = PointXY { x: d / 2.0_f64.sqrt(), y: d / 2.0_f64.sqrt() };
        let shape = compute_corridor_shape(from, to, 10.0).expect("should compute");
        // The AABB grows by ~sin(45°) factor in each dimension.
        // Both nx and ny should be comfortably > the axis-aligned case.
        assert!(shape.nx > 180, "rotated corridor should inflate nx; got {}", shape.nx);
        assert!(shape.ny > 180, "rotated corridor should inflate ny; got {}", shape.ny);
    }
}
