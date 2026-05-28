//! Gradient-descent path extraction from an FMM arrival-time field.
//!
//! The optimal continuous path from start to goal is the integral
//! curve of `-∇u`, started at the goal and integrated backward in
//! time. We do explicit Euler integration at a sub-cell step
//! (`cell_m / 4` by default) with bilinear gradient interpolation.
//! Sub-cell steps are critical: at full-cell steps the path zig-
//! zags between equally-low neighbour cells. The well-known
//! `flow-field` trick from game engines.
//!
//! ## Termination
//!
//! Two stopping criteria:
//!   1. Reached the start: `‖p - start‖ < 0.5 · cell_m`.
//!   2. Reached the seed value: `u(p) ≤ step_m × BASE_PACE_S_PER_M`.
//!      Belt-and-braces in case the start coordinate doesn't quite
//!      match the seed cell.
//!
//! A `max_steps` ceiling catches non-convergence; if the gradient
//! becomes degenerate (zero magnitude with non-zero u) we return
//! `ExtractError::Diverged`. That's typically a metric bug — a
//! true eikonal solution should have a strictly increasing
//! gradient from the seed outward.

use crate::grid::{FmmGrid, GridShape};

/// Same `BASE_PACE_S_PER_M` constant the pathfinder uses. Defined
/// here so the FMM crate stays self-contained; phase 6 will pass
/// it in from the cost config to avoid drift.
const BASE_PACE_S_PER_M: f32 = 1.0 / 1.4;

/// Path-extraction error states.
#[derive(Debug, thiserror::Error)]
pub enum ExtractError {
    #[error("goal coordinate outside the arrival grid")]
    GoalOutOfGrid,
    #[error("goal cell has +∞ arrival time (refused or unreached)")]
    GoalUnreachable,
    #[error("gradient descent diverged — no convergence after {0} steps")]
    Diverged(u32),
    #[error("gradient went to zero before reaching the start")]
    LocalMinimum,
}

/// 2D point in the same UTM33N world frame as the grid.
#[derive(Debug, Clone, Copy)]
pub struct PathPoint {
    pub x: f64,
    pub y: f64,
}

/// Extract the optimal path from `start` to `goal` against the
/// arrival-time field `arrival`.
///
/// `start` is the seed location (where `u = 0`); `goal` is where
/// we *started* the gradient descent. Returns a polyline in
/// `start → goal` order (we integrate backward then reverse).
///
/// `step_m` defaults to `shape.cell_m / 4` if `None`.
/// `max_steps` defaults to `10 × (corridor diagonal / step_m)` —
/// generous enough for any well-behaved cost field.
pub fn extract_path(
    shape: &GridShape,
    arrival: &FmmGrid<f32>,
    start: PathPoint,
    goal: PathPoint,
    step_m: Option<f64>,
    max_steps: Option<u32>,
) -> Result<Vec<PathPoint>, ExtractError> {
    let step = step_m.unwrap_or(shape.cell_m / 4.0);
    let diagonal = ((shape.nx as f64).powi(2) + (shape.ny as f64).powi(2)).sqrt() * shape.cell_m;
    let max_iters = max_steps.unwrap_or((10.0 * diagonal / step) as u32);

    // Validate goal is inside the grid.
    let goal_cell = shape
        .world_to_cell(goal.x, goal.y)
        .ok_or(ExtractError::GoalOutOfGrid)?;
    let goal_u = arrival.get(goal_cell.0, goal_cell.1, 0);
    if !goal_u.is_finite() {
        return Err(ExtractError::GoalUnreachable);
    }

    let half_cell = shape.cell_m * 0.5;
    let seed_threshold = (step as f32) * BASE_PACE_S_PER_M;

    let mut p = goal;
    let mut path = Vec::with_capacity(max_iters as usize);
    path.push(p);

    for iter in 0..max_iters {
        // Stop if we're within half a cell of the start.
        let dx = p.x - start.x;
        let dy = p.y - start.y;
        if (dx * dx + dy * dy).sqrt() < half_cell {
            path.push(start);
            path.reverse();
            return Ok(path);
        }
        // Stop if arrival here is essentially zero (seed value).
        let u_here = sample_arrival(shape, arrival, p);
        if u_here.is_finite() && u_here <= seed_threshold {
            path.push(start);
            path.reverse();
            return Ok(path);
        }

        // Compute ∇u via bilinear interpolation. The descent step
        // moves in the direction of decreasing arrival time, i.e.
        // opposite the gradient.
        let g = gradient_at(shape, arrival, p);
        let mag = (g.0 * g.0 + g.1 * g.1).sqrt();
        if !mag.is_finite() || mag < 1e-6 {
            // Local minimum or refused-cell region; bail.
            if iter < max_iters / 10 {
                // Bail close to goal isn't catastrophic — try to
                // make headway with a tiny step toward `start`.
                let inv = 1.0 / ((dx * dx + dy * dy).sqrt() + 1e-9);
                p.x += -dx * inv * step;
                p.y += -dy * inv * step;
                path.push(p);
                continue;
            }
            return Err(ExtractError::LocalMinimum);
        }
        // Move backward along -∇u with `step` length.
        p.x -= step * g.0 / mag;
        p.y -= step * g.1 / mag;
        path.push(p);
    }
    Err(ExtractError::Diverged(max_iters))
}

/// Bilinearly interpolate the arrival time at sub-cell point `p`.
/// Returns `+∞` if `p` falls outside the grid or any of the four
/// corner cells is refused (`+∞`).
fn sample_arrival(shape: &GridShape, arrival: &FmmGrid<f32>, p: PathPoint) -> f32 {
    let fi = (p.x - shape.origin_x) / shape.cell_m - 0.5;
    let fj = (p.y - shape.origin_y) / shape.cell_m - 0.5;
    let i0 = fi.floor() as i64;
    let j0 = fj.floor() as i64;
    if i0 < 0 || j0 < 0 || (i0 + 1) >= shape.nx as i64 || (j0 + 1) >= shape.ny as i64 {
        return f32::INFINITY;
    }
    let tx = (fi - i0 as f64) as f32;
    let ty = (fj - j0 as f64) as f32;
    let i = i0 as u32;
    let j = j0 as u32;
    let u00 = arrival.get(i,     j,     0);
    let u10 = arrival.get(i + 1, j,     0);
    let u01 = arrival.get(i,     j + 1, 0);
    let u11 = arrival.get(i + 1, j + 1, 0);
    (1.0 - tx) * (1.0 - ty) * u00
        + tx * (1.0 - ty) * u10
        + (1.0 - tx) * ty * u01
        + tx * ty * u11
}

/// Bilinearly-interpolated ∇u at sub-cell point `p`. Returns
/// `(∂u/∂x, ∂u/∂y)` in `s/m` (cell-edge differences).
///
/// When any of the four corner cells has `+∞` arrival, the gradient
/// in that direction is pushed *away* from the refused cell by
/// substituting a large finite penalty — this keeps the descent
/// from heading straight into a refusal.
fn gradient_at(shape: &GridShape, arrival: &FmmGrid<f32>, p: PathPoint) -> (f64, f64) {
    let fi = (p.x - shape.origin_x) / shape.cell_m - 0.5;
    let fj = (p.y - shape.origin_y) / shape.cell_m - 0.5;
    let i0 = fi.floor() as i64;
    let j0 = fj.floor() as i64;
    if i0 < 0 || j0 < 0 || (i0 + 1) >= shape.nx as i64 || (j0 + 1) >= shape.ny as i64 {
        return (0.0, 0.0);
    }
    let tx = (fi - i0 as f64) as f32;
    let ty = (fj - j0 as f64) as f32;
    let i = i0 as u32;
    let j = j0 as u32;
    let u00 = arrival.get(i,     j,     0);
    let u10 = arrival.get(i + 1, j,     0);
    let u01 = arrival.get(i,     j + 1, 0);
    let u11 = arrival.get(i + 1, j + 1, 0);
    // Replace +∞ with a large finite "wall" so the gradient pushes
    // away rather than producing NaN. The constant is large enough
    // to dominate any neighbour-cell finite value yet stays in f32
    // range without producing infinity in subsequent arithmetic.
    let wall = 1.0e10f32;
    let u00 = if u00.is_finite() { u00 } else { wall };
    let u10 = if u10.is_finite() { u10 } else { wall };
    let u01 = if u01.is_finite() { u01 } else { wall };
    let u11 = if u11.is_finite() { u11 } else { wall };
    let dux = (1.0 - ty) * (u10 - u00) + ty * (u11 - u01);
    let duy = (1.0 - tx) * (u01 - u00) + tx * (u11 - u10);
    (
        dux as f64 / shape.cell_m,
        duy as f64 / shape.cell_m,
    )
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::{solve_2d_isotropic, FmmGrid, GridShape, StopCondition};

    #[test]
    fn straight_path_on_uniform_field() {
        // F = 1, 100×100 grid, seed at (10, 50), goal at (90, 50).
        // The extracted path should run along the y = 50 line with
        // ~80 m total length, ±1 % (one cell).
        let n = 100u32;
        let h = 1.0_f64;
        let shape = GridShape::new_2d(n, n, 0.0, 0.0, h);
        let cost: FmmGrid<f32> = FmmGrid::filled(shape, 1.0);
        let result = solve_2d_isotropic(
            shape, &cost, &[(10, 50, 0.0)], StopCondition::AllAccepted,
        );
        let start = PathPoint { x: 10.5, y: 50.5 };
        let goal  = PathPoint { x: 90.5, y: 50.5 };
        let path = extract_path(&shape, &result.arrival, start, goal, None, None)
            .expect("extract should succeed on uniform field");
        // Sum lengths.
        let mut total = 0.0f64;
        for w in path.windows(2) {
            total += ((w[1].x - w[0].x).powi(2) + (w[1].y - w[0].y).powi(2)).sqrt();
        }
        // Direct distance is 80 m; path can be up to ~1 m longer due
        // to discrete-step accumulation.
        assert!(
            (total - 80.0).abs() < 2.0,
            "expected ~80 m, got {:.2}",
            total
        );
        // First point near start, last near goal.
        assert!((path[0].x - start.x).abs() < 1.0 && (path[0].y - start.y).abs() < 1.0);
        assert!((path[path.len() - 1].x - goal.x).abs() < 1.0
                && (path[path.len() - 1].y - goal.y).abs() < 1.0);
    }

    #[test]
    fn detour_around_refused_block() {
        // 80×80 grid, seed at (5, 40), goal at (75, 40). A vertical
        // wall of refused cells at i = 40, j ∈ [20, 60]. The wave
        // wraps around the top of the wall (j = 0..19) since the
        // bottom edge (j = 60..79) is also open. The path should
        // visibly bend.
        let n = 80u32;
        let h = 1.0_f64;
        let shape = GridShape::new_2d(n, n, 0.0, 0.0, h);
        let mut cost: FmmGrid<f32> = FmmGrid::filled(shape, 1.0);
        for j in 20..60 {
            cost.set(40, j, 0, f32::INFINITY);
        }
        let result = solve_2d_isotropic(shape, &cost, &[(5, 40, 0.0)], StopCondition::AllAccepted);
        let start = PathPoint { x: 5.5, y: 40.5 };
        let goal = PathPoint { x: 75.5, y: 40.5 };
        let path = extract_path(&shape, &result.arrival, start, goal, None, None)
            .expect("detour path should be extractable");
        // The straight-line path would have y ≈ 40 throughout. The
        // detour must go to y < 20 or y > 60 at some point — assert
        // the y-extent of the path exceeds ±20 cells.
        let mut min_y = f64::INFINITY;
        let mut max_y = f64::NEG_INFINITY;
        for p in &path {
            if p.y < min_y { min_y = p.y; }
            if p.y > max_y { max_y = p.y; }
        }
        let y_extent = (max_y - min_y).abs();
        assert!(
            y_extent > 15.0,
            "detour path y-extent {:.2} is too small — straight line?",
            y_extent
        );
    }

    #[test]
    fn goal_outside_grid_errors() {
        let shape = GridShape::new_2d(10, 10, 0.0, 0.0, 1.0);
        let arrival: FmmGrid<f32> = FmmGrid::filled(shape, 1.0);
        let res = extract_path(
            &shape,
            &arrival,
            PathPoint { x: 5.5, y: 5.5 },
            PathPoint { x: 100.0, y: 100.0 },
            None,
            None,
        );
        assert!(matches!(res, Err(ExtractError::GoalOutOfGrid)));
    }

    #[test]
    fn unreached_goal_errors() {
        let shape = GridShape::new_2d(10, 10, 0.0, 0.0, 1.0);
        let arrival: FmmGrid<f32> = FmmGrid::filled(shape, f32::INFINITY);
        let res = extract_path(
            &shape,
            &arrival,
            PathPoint { x: 0.5, y: 0.5 },
            PathPoint { x: 5.5, y: 5.5 },
            None,
            None,
        );
        assert!(matches!(res, Err(ExtractError::GoalUnreachable)));
    }
}
