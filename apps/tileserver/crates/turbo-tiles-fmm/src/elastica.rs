//! State-augmented (x, y, heading) grade-limited path solver.
//!
//! The 2D anisotropic-eikonal FMM produces smooth *time-optimal*
//! geodesics — and on steep ground that means climbing nearly straight
//! up the fall line. A convex Finsler metric provably cannot switchback
//! (it always prefers the single diagonal traverse). Real trails
//! switchback because they obey a **grade constraint**: you can't ascend
//! steeper than ~`max_grade_deg`, so to keep gaining elevation you
//! traverse at the cap and reverse.
//!
//! We model that by lifting the state to `(i, j, k)` where `k` is one of
//! `N_HEADINGS = 8` compass headings (the 8 grid-neighbour directions),
//! and solving a **directed shortest path** (Dijkstra) on that lattice:
//!   - a FORWARD move steps to the neighbour cell in heading `k`, at the
//!     Tobler pace for that move's grade — but **refused (∞) when the
//!     grade exceeds `max_grade_deg`**, which forbids going straight up;
//!   - a TURN rotates the heading by ±45° at a fixed `turn_penalty_s`.
//!
//! Switchbacks emerge: on a slope steeper than the cap the only finite
//! forward moves are the traverses that ascend at ≤ cap; to keep
//! climbing the path pays a turn penalty and reverses heading. Dijkstra
//! on this non-symmetric, direction-dependent cost is provably correct
//! and reuses the existing `NarrowBandHeap`.

use crate::grid::{FmmGrid, GridShape};
use crate::heap::NarrowBandHeap;
use crate::tobler::Elevation;

/// Number of discrete headings. 16 directions — the 8 compass moves
/// plus the 8 knight (2:1) moves — so the solver can traverse a steep
/// fall line at a SHALLOW grade: a 2:1 move on a 35° slope is only ~17°,
/// where the 45° diagonal would be 26°. Without the knight moves there
/// is no sub-25° ascending move on a uniform steep slope, so the grade
/// cap becomes unreachable and the route can't switchback.
pub const N_HEADINGS: u32 = 16;

/// `(di, dj, step_length_in_cells)` per heading `k`, ordered CCW from
/// East by bearing. Mix of 1-cell compass and 2-cell knight moves.
const SQRT2: f32 = std::f32::consts::SQRT_2;
const SQRT5: f32 = 2.236_068;
const DIRS: [(i32, i32, f32); 16] = [
    (1, 0, 1.0),     //   0°
    (2, 1, SQRT5),   //  26.6°
    (1, 1, SQRT2),   //  45°
    (1, 2, SQRT5),   //  63.4°
    (0, 1, 1.0),     //  90°
    (-1, 2, SQRT5),  // 116.6°
    (-1, 1, SQRT2),  // 135°
    (-2, 1, SQRT5),  // 153.4°
    (-1, 0, 1.0),    // 180°
    (-2, -1, SQRT5), // 206.6°
    (-1, -1, SQRT2), // 225°
    (-1, -2, SQRT5), // 243.4°
    (0, -1, 1.0),    // 270°
    (1, -2, SQRT5),  // 296.6°
    (1, -1, SQRT2),  // 315°
    (2, -1, SQRT5),  // 333.4°
];

/// Per-cell cost overlay the lifted solver consults so it sees more than
/// bare elevation — without it the solver walks straight across lakes
/// (flat ⇒ cheap). Implementors map the project's cost model onto the grid:
///   - `refused` ⇒ impassable (deep water / glacier / true cliff): any move
///     into the cell costs `∞`.
///   - `pace_mul` ⇒ multiplier on the move pace into the cell (< 1 near
///     trails, > 1 on expensive ground; 1.0 neutral).
///
/// Looked up on demand so the adapter can evaluate cells lazily (only those
/// the A* actually visits) instead of baking the whole corridor up front.
pub trait CellOverlay {
    fn refused(&self, i: u32, j: u32) -> bool;
    fn pace_mul(&self, i: u32, j: u32) -> f32;
}

/// Precomputed array overlay — used by the synthetic unit tests and any
/// caller that already has the grids. Empty vecs ⇒ neutral (no refusals,
/// all multipliers 1.0).
pub struct ArrayOverlay {
    pub nx: u32,
    pub refused: Vec<bool>,
    pub pace_mul: Vec<f32>,
}

impl CellOverlay for ArrayOverlay {
    #[inline]
    fn refused(&self, i: u32, j: u32) -> bool {
        if self.refused.is_empty() {
            return false;
        }
        self.refused
            .get((j * self.nx + i) as usize)
            .copied()
            .unwrap_or(false)
    }
    #[inline]
    fn pace_mul(&self, i: u32, j: u32) -> f32 {
        if self.pace_mul.is_empty() {
            return 1.0;
        }
        self.pace_mul
            .get((j * self.nx + i) as usize)
            .copied()
            .unwrap_or(1.0)
    }
}

/// Grade-limited transition cost over the lifted lattice.
pub struct GradeLimitedCost<E: Elevation, O: CellOverlay> {
    pub elev: E,
    pub base_pace_s_per_m: f32,
    pub off_trail_factor: f32,
    /// Above this slope (deg, magnitude) a forward move is refused — the
    /// constraint that forces traverses/switchbacks instead of a direct
    /// ascent.
    pub max_grade_deg: f32,
    /// Seconds charged per 45° heading change (curvature/effort). Higher
    /// → fewer, longer traverses; lower → tighter switchbacks.
    pub turn_penalty_s: f32,
    /// Per-cell refusal + pace overlay (water/glacier/trail cost).
    pub overlay: O,
}

/// Strength of the super-linear steep-grade penalty (applied to moves
/// whose along-direction grade exceeds the comfort `max_grade_deg`). The
/// move cost is multiplied by `1 + K·over²` where `over` is the fractional
/// overshoot. High enough that a gentler detour, when one exists, beats
/// climbing the fall line — but finite, so the route can still climb out of
/// a basin with no other exit.
const STEEP_PENALTY_K: f32 = 10.0;

/// Above this slope (deg) the ground is a true cliff and impassable.
const CLIFF_DEG: f32 = 60.0;

/// Tobler pace (s/m) from gradient magnitude (tan of slope). Mirrors
/// `tobler_aniso::tobler_pace`. Public so the pathfind crate's unified
/// solver costs off-trail mesh edges with the identical slope pace as
/// the lifted solver.
#[inline]
pub fn tobler_pace(grad_mag: f32) -> f32 {
    let v = 1.6667 * (-3.5 * (grad_mag.abs() + 0.05)).exp();
    if v < 1e-4 {
        1.0e6
    } else {
        1.0 / v
    }
}

impl<E: Elevation, O: CellOverlay> GradeLimitedCost<E, O> {
    /// `true` if cell `(i, j)` is impassable per the overlay.
    #[inline]
    fn refused_at(&self, i: i32, j: i32) -> bool {
        if i < 0 || j < 0 {
            return false;
        }
        self.overlay.refused(i as u32, j as u32)
    }

    /// Cost of a forward move from `(i, j)` to its heading-`k` neighbour.
    /// `+∞` when out of grid, when the grade exceeds the cap, or when the
    /// target (or the mid-cell of a 2:1 knight move) is a refused cell —
    /// the latter stops a knight step from leaping a thin water strip.
    /// Nodata elevation is treated as flat at a high (passable) pace so it
    /// never severs. The target cell's pace multiplier shapes the cost so
    /// the route prefers trails / avoids expensive ground.
    fn forward_cost(&self, shape: &GridShape, i: u32, j: u32, k: u32) -> f32 {
        let (di, dj, len_cells) = DIRS[k as usize];
        let ni = i as i32 + di;
        let nj = j as i32 + dj;
        if ni < 0 || nj < 0 || ni >= shape.nx as i32 || nj >= shape.ny as i32 {
            return f32::INFINITY;
        }
        // Refuse the move if the straight segment from this cell to the
        // neighbour passes through ANY refused cell — not just the target.
        // The drawn route follows these chords, so a 2:1 knight (or even a
        // diagonal) must not clip a lake/glacier cell between its endpoints.
        {
            let span = di.abs().max(dj.abs());
            let steps = (span * 4).max(2);
            for s in 0..=steps {
                let t = s as f32 / steps as f32;
                let ci = (i as f32 + di as f32 * t).round() as i32;
                let cj = (j as f32 + dj as f32 * t).round() as i32;
                if self.refused_at(ci, cj) {
                    return f32::INFINITY;
                }
            }
        }
        let step_m = len_cells * shape.cell_m as f32;
        let base = self.base_pace_s_per_m;
        // Off-trail roughness is no longer multiplied in here: it is a
        // multiplicative `pace_factor` contributor folded into the
        // overlay's `mul` (so it composes with the trail-proximity
        // bonus exactly as before). `off_trail_factor` survives only as
        // the A* heuristic's pace floor (see `min_pace` below).
        let mul = self.overlay.pace_mul(ni as u32, nj as u32);
        match (
            self.elev.at(shape, i, j),
            self.elev.at(shape, ni as u32, nj as u32),
        ) {
            (Some(z0), Some(z1)) => {
                let grad = ((z1 - z0) / step_m).abs();
                let grade_deg = grad.atan().to_degrees();
                // True cliffs are impassable.
                if grade_deg > CLIFF_DEG {
                    return f32::INFINITY;
                }
                // Ascending/descending steep ground is increasingly costly
                // above `max_grade_deg` (the comfort grade) — super-linearly,
                // but NOT impossible. A move along the fall line has a large
                // `grad`; a cross-slope traverse has `grad ≈ 0` and pays no
                // steep penalty (traversing steep terrain is fine). So the
                // solver prefers to traverse / walk around, and only climbs
                // directly when no gentler detour exists.
                let steep = if grade_deg > self.max_grade_deg {
                    let over = (grade_deg - self.max_grade_deg) / self.max_grade_deg.max(1.0);
                    1.0 + STEEP_PENALTY_K * over * over
                } else {
                    1.0
                };
                step_m * tobler_pace(grad) * mul * steep
            }
            // Missing elevation: passable but discouraged (flat × 3).
            _ => step_m * base * 3.0 * mul,
        }
    }
}

/// Output of the lifted solve: arrival field over `(i, j, k)`, the
/// predecessor flat index per state (for backtracking; `u32::MAX` =
/// none/seed), and the cheapest accepted goal state `(i, j, k)` if
/// reached.
pub struct LiftedResult {
    pub arrival: FmmGrid<f32>,
    pub parent: Vec<u32>,
    pub goal_state: Option<usize>,
    pub cells_accepted: u32,
}

const NO_PARENT: u32 = u32::MAX;

/// Dijkstra over the `(i, j, heading)` lattice. Seeds every heading at
/// `start_cell` (free initial facing); terminates when any heading at
/// `goal_cell` is finalised.
/// Periodic progress snapshot emitted by the lifted solve so callers can
/// stream "the route reaching toward the goal" live. `best_path` is the
/// current best partial route (world UTM, start → frontier tip nearest the
/// goal); `remaining_m` is that tip's straight-line distance to the goal.
pub struct LiftedProgress {
    pub best_path: Vec<(f64, f64)>,
    pub cells_accepted: u32,
    pub remaining_m: f64,
}

/// Emit a progress snapshot roughly this often (in accepted states).
const PROGRESS_INTERVAL: u32 = 40_000;

pub fn solve_lifted_grade_limited<E: Elevation, O: CellOverlay>(
    shape: GridShape,
    cost: &GradeLimitedCost<E, O>,
    start_cell: (u32, u32),
    goal_cell: (u32, u32),
    mut on_progress: Option<&mut dyn FnMut(&LiftedProgress)>,
) -> LiftedResult {
    debug_assert_eq!(
        shape.nz, N_HEADINGS,
        "lifted solver expects nz == N_HEADINGS"
    );
    let n = shape.len();
    let mut arrival: FmmGrid<f32> = FmmGrid::filled(shape, f32::INFINITY);
    let mut parent: Vec<u32> = vec![NO_PARENT; n];
    let mut accepted: Vec<bool> = vec![false; n];
    let mut heap = NarrowBandHeap::with_cells(n);

    // A* heuristic. `arrival` holds the true cost-to-reach `g`; the heap is
    // keyed by `f = g + h`, where `h` is an admissible lower bound on the
    // remaining cost: straight-line metres to the goal cell × the cheapest
    // possible pace of any move. Cheapest pace = best Tobler speed (≈0.6 s/m,
    // i.e. 1/1.6667 at the optimal slight downhill) × the off-trail factor ×
    // the per-cell pace-multiplier floor (0.1, the adapter's clamp). This
    // never overestimates, so A* stays optimal while exploring toward the
    // goal instead of flooding the whole corridor.
    let gi = goal_cell.0 as f32;
    let gj = goal_cell.1 as f32;
    let cell_m_f = shape.cell_m as f32;
    let min_pace = 0.6 * cost.off_trail_factor * 0.1;
    let h = |i: u32, j: u32| -> f32 {
        let dx = i as f32 - gi;
        let dy = j as f32 - gj;
        (dx * dx + dy * dy).sqrt() * cell_m_f * min_pace
    };

    for k in 0..N_HEADINGS {
        let flat = shape.idx(start_cell.0, start_cell.1, k);
        arrival.flat_mut()[flat] = 0.0;
        heap.push(h(start_cell.0, start_cell.1), flat as u32);
    }

    let mut cells_accepted = 0u32;
    let mut goal_state: Option<usize> = None;
    // Track the accepted state closest to the goal (min heuristic) so the
    // progress snapshot shows the route's leading edge, not a random pop.
    let mut best_state = shape.idx(start_cell.0, start_cell.1, 0);
    let mut best_h = f32::INFINITY;
    let mut since_emit = 0u32;

    while let Some((_f_a, cell_a)) = heap.pop_min() {
        let flat_a = cell_a as usize;
        if accepted[flat_a] {
            continue;
        }
        accepted[flat_a] = true;
        cells_accepted += 1;
        let g_a = arrival.flat()[flat_a];
        let (ai, aj, ak) = shape.unpack(flat_a);

        if ai == goal_cell.0 && aj == goal_cell.1 {
            goal_state = Some(flat_a);
            break;
        }

        // Progress: backtrack the best-toward-goal state into a partial
        // route and hand it to the caller. Done here (before `relax` borrows
        // `parent` mutably) so we can read `parent` freely.
        let h_a = h(ai, aj);
        if h_a < best_h {
            best_h = h_a;
            best_state = flat_a;
        }
        since_emit += 1;
        if let Some(cb) = on_progress.as_deref_mut() {
            if since_emit >= PROGRESS_INTERVAL {
                since_emit = 0;
                let mut pts: Vec<(f64, f64)> = Vec::new();
                let mut cur = best_state;
                let mut guard = 0usize;
                loop {
                    let (i, j, _) = shape.unpack(cur);
                    let (x, y) = shape.cell_centre(i, j);
                    if pts.last().is_none_or(|&(px, py): &(f64, f64)| {
                        (px - x).abs() > 1e-6 || (py - y).abs() > 1e-6
                    }) {
                        pts.push((x, y));
                    }
                    let p = parent[cur];
                    if p == NO_PARENT {
                        break;
                    }
                    cur = p as usize;
                    guard += 1;
                    if guard > shape.len() {
                        break;
                    }
                }
                pts.reverse();
                let prog = LiftedProgress {
                    best_path: pts,
                    cells_accepted,
                    remaining_m: (best_h / min_pace.max(1e-6)) as f64,
                };
                cb(&prog);
            }
        }

        // Relax candidates: one forward move + two ±45° turns. The heap key
        // is `g + h(neighbour)`; `arrival` stores the true `g`.
        let relax = |ni: u32,
                     nj: u32,
                     nk: u32,
                     edge: f32,
                     arrival: &mut FmmGrid<f32>,
                     parent: &mut [u32],
                     heap: &mut NarrowBandHeap| {
            if !edge.is_finite() {
                return;
            }
            let nflat = shape.idx(ni, nj, nk);
            if accepted[nflat] {
                return;
            }
            let cand = g_a + edge;
            if cand < arrival.flat()[nflat] {
                arrival.flat_mut()[nflat] = cand;
                parent[nflat] = flat_a as u32;
                heap.decrease_key_or_insert(nflat as u32, cand + h(ni, nj));
            }
        };

        // Forward.
        let (di, dj, _) = DIRS[ak as usize];
        let fi = ai as i32 + di;
        let fj = aj as i32 + dj;
        if fi >= 0 && fj >= 0 && fi < shape.nx as i32 && fj < shape.ny as i32 {
            let c = cost.forward_cost(&shape, ai, aj, ak);
            relax(
                fi as u32,
                fj as u32,
                ak,
                c,
                &mut arrival,
                &mut parent,
                &mut heap,
            );
        }
        // Turns (stay in place, rotate ±45°).
        for &dk in &[1i32, N_HEADINGS as i32 - 1] {
            let nk = ((ak as i32 + dk) % N_HEADINGS as i32) as u32;
            relax(
                ai,
                aj,
                nk,
                cost.turn_penalty_s,
                &mut arrival,
                &mut parent,
                &mut heap,
            );
        }
    }

    LiftedResult {
        arrival,
        parent,
        goal_state,
        cells_accepted,
    }
}

/// Backtrack the Dijkstra parent tree from the goal state to the seed,
/// projecting each `(i, j, k)` state to its `(x, y)` cell centre (the
/// heading is dropped). Consecutive turn states at the same cell collapse
/// to one point. Returns the polyline start → goal, or `None` if the goal
/// was unreachable. Callers smooth the (coarse) result.
pub fn extract_path_lifted(
    shape: &GridShape,
    result: &LiftedResult,
    start: (f64, f64),
    goal: (f64, f64),
) -> Option<Vec<(f64, f64)>> {
    let goal_state = result.goal_state?;
    let mut pts: Vec<(f64, f64)> = Vec::new();
    pts.push(goal);
    let mut cur = goal_state;
    let mut guard = 0usize;
    let cap = shape.len() + 8;
    loop {
        let (i, j, _) = shape.unpack(cur);
        let (cx, cy) = shape.cell_centre(i, j);
        // Skip duplicate cell (turns produce same (i,j)).
        if pts
            .last()
            .is_none_or(|&(px, py)| (px - cx).abs() > 1e-6 || (py - cy).abs() > 1e-6)
        {
            pts.push((cx, cy));
        }
        let p = result.parent[cur];
        if p == NO_PARENT {
            break;
        }
        cur = p as usize;
        guard += 1;
        if guard > cap {
            break;
        }
    }
    pts.push(start);
    pts.reverse();
    Some(pts)
}
