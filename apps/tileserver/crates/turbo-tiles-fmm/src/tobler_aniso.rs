//! Anisotropic Tobler-Finsler metric (phase 5).
//!
//! Where the isotropic version returns a single scalar pace per
//! cell — punishing motion equally in every direction once a slope
//! is detected — the anisotropic version returns a *direction-
//! dependent* metric: motion along the contour is cheap (`F ≈ flat
//! pace`); motion up/down the fall line is Tobler-expensive.
//!
//! The metric tensor at cell `(i, j)` is
//!
//! ```text
//!   M(i, j) = λ_along²(s)  · n_along ⊗ n_along
//!           + λ_perp²      · n_perp  ⊗ n_perp
//! ```
//!
//! where `n_along` is the unit slope-gradient direction, `n_perp`
//! is perpendicular (along the contour), `λ_along(s)` is the Tobler
//! pace at slope `s`, and `λ_perp` is the flat-trail pace. Both
//! eigenvalues are *paces* (s/m); `M` therefore has units of (s/m)².
//!
//! The AGSI / Selling reduction translates `M` into the three short
//! lattice offsets + weights the anisotropic stencil consumes.
//!
//! ## Contour following — the architectural property the user asked
//! ## for
//!
//! With the isotropic metric, the FMM wave on a 30° slope marches
//! at ~5.4 s/m in *every* direction — including along the contour
//! where a real hiker maintains base pace. The anisotropic metric
//! says: along the contour, pay base pace; uphill, pay Tobler. The
//! wave reaches contour-following cells substantially earlier than
//! uphill cells of the same Euclidean distance, so the extracted
//! geodesic naturally curves around the mountain instead of cutting
//! straight over it.

use crate::aniso::CellForm;
use crate::grid::{FmmGrid, GridShape};
use crate::metric::NormForm;
use crate::selling::{selling_reduce, SymMat2};
use crate::tobler::Elevation;

/// Maximum allowed anisotropy ratio (along-pace / perp-pace). Beyond
/// this, the Selling reduction can need many iterations or produce
/// stencil offsets too large for our `i8` storage. We clamp the
/// along-pace at this multiple of the perp-pace — empirically a
/// 50× anisotropy already gives near-perfect contour-following.
const MAX_ANISO_RATIO: f32 = 50.0;

/// Anisotropic Tobler metric. Returns a `NormForm` per cell;
/// callers wrap it with a base RHS via `bake_aniso_corridor`.
pub struct ToblerAnisotropic<E: Elevation> {
    pub elev: E,
    pub refuse_above_deg: f32,
    pub base_pace_s_per_m: f32,
    pub off_trail_factor: f32,
    /// Naismith vertical-gain weight: effective extra metres of flat
    /// walking per metre of elevation gained (foot ≈ 8, ski ≈ 6,
    /// bicycle ≈ 20). Folded *directionally* into the along-fall-line
    /// pace — the only place that knows the climb direction — so the
    /// off-trail geodesic prices cumulative gain the same way on-graph
    /// routing does (`length + k·gain`), instead of seeing only Tobler.
    /// Without this the solver cuts straight over hills the marked
    /// trail switchbacks around. `0.0` reproduces pure Tobler.
    pub gain_factor_k: f32,
}

impl<E: Elevation> ToblerAnisotropic<E> {
    /// Compute the eikonal's dual metric tensor `G*` at the cell.
    /// `G* = (1/τ_along²) n_along n_alongᵀ + (1/τ_perp²) n_perp n_perpᵀ`
    /// — eigenvalues are *reciprocal* paces² (i.e. speed²).
    ///
    /// Then `F(∇u)² = ∇uᵀ G* ∇u = 1` is the eikonal we discretise.
    /// Returns `None` for refused cells (slope too steep / nodata).
    fn metric_at(&self, shape: &GridShape, i: u32, j: u32) -> Option<SymMat2> {
        // Slope gradient via finite differences. At the grid boundary the
        // central stencil would read out-of-range cells, so we clamp the
        // sample indices and divide by the *actual* sampled span. This
        // yields a proper one-sided gradient (and thus the correct
        // anisotropic metric) at edge cells. The previous behaviour —
        // collapsing edge cells to isotropic base pace — turned the whole
        // corridor border into an artificially cheap racetrack, which the
        // wave flooded along and the extracted geodesic ballooned out to
        // follow (the 2-3× perimeter-loop failures on the terrain corpus).
        let il = i.saturating_sub(1);
        let ir = (i + 1).min(shape.nx - 1);
        let jd = j.saturating_sub(1);
        let ju = (j + 1).min(shape.ny - 1);
        // DEM nodata must NOT sever the corridor. The Norway DTM is ~67%
        // absent (alpine areas worst), and returning `None` here refused
        // those cells → the corridor disconnected → blocky Theta*
        // fallback. Instead, treat a cell with any missing neighbour as
        // flat at a high (discouraging) isotropic pace: passable, so the
        // FMM always connects and stays smooth, but avoided when real
        // terrain is available. (DemCoveragePenaltyContributor adds the
        // matching walk-seconds penalty for routing realism.)
        let (z_l, z_r, z_d, z_u) = match (
            self.elev.at(shape, il, j),
            self.elev.at(shape, ir, j),
            self.elev.at(shape, i, jd),
            self.elev.at(shape, i, ju),
        ) {
            (Some(a), Some(b), Some(c), Some(d)) => (a, b, c, d),
            _ => {
                let p = (self.base_pace_s_per_m * self.off_trail_factor * 3.0) as f64;
                let inv = 1.0 / (p * p);
                return Some(SymMat2::new(inv, 0.0, inv));
            }
        };
        let h = shape.cell_m as f32;
        let span_x = (ir - il).max(1) as f32 * h;
        let span_y = (ju - jd).max(1) as f32 * h;
        let dz_dx = (z_r - z_l) / span_x;
        let dz_dy = (z_u - z_d) / span_y;
        let grad_mag = (dz_dx * dz_dx + dz_dy * dz_dy).sqrt();
        // No slope hard-refuse: steep ground is very-high-but-FINITE cost
        // (tau_along is Tobler pace, capped by MAX_ANISO_RATIO below), so
        // the corridor is always connected on slope and the FMM always
        // produces a smooth route — the geodesic curves around steepness
        // on the gentle line rather than the corridor being severed and
        // dropping to the blocky Theta* fallback. Only genuine impassable
        // classes (water interior, glacier, building, ocean) are refused,
        // by the contributor veto pass. `refuse_above_deg` is retained on
        // the struct for callers/tests but no longer gates the metric.
        let _ = self.refuse_above_deg;

        let base = self.base_pace_s_per_m;
        let off = self.off_trail_factor;
        let tau_perp = base * off;
        // Along the fall line, pay Tobler pace PLUS a Naismith gain
        // term: moving one path-metre up a slope of grade `grad_mag`
        // gains `sinθ = grad/√(1+grad²)` metres, and Naismith prices
        // that at `k` effective flat-metres → `k·sinθ·base` extra
        // seconds per metre. This matches the on-graph `length + k·gain`
        // pricing so off-trail routing avoids gratuitous climbing the
        // same way the marked trails do. Gain term is unscaled by the
        // off-trail factor (climbing cost is surface-independent), the
        // Tobler base pace carries the off-trail slowdown.
        let sin_theta = grad_mag / (1.0 + grad_mag * grad_mag).sqrt();
        let gain_pace = self.gain_factor_k * sin_theta * base;
        // Cap anisotropy ratio so the Selling reduction's integer
        // offsets stay within i8 range. The cap is multiplicative on
        // the along-pace relative to the perp-pace.
        let tau_along_raw = tobler_pace(grad_mag).max(base) * off + gain_pace;
        let tau_along = tau_along_raw.min(tau_perp * MAX_ANISO_RATIO);

        let inv_along2 = (1.0 / (tau_along * tau_along)) as f64;
        let inv_perp2 = (1.0 / (tau_perp * tau_perp)) as f64;

        if grad_mag < 1e-6 {
            // Effectively flat: isotropic.
            return Some(SymMat2::new(inv_perp2, 0.0, inv_perp2));
        }
        let nax = (dz_dx / grad_mag) as f64;
        let nay = (dz_dy / grad_mag) as f64;
        // G* = inv_along2 · n_along n_alongᵀ + inv_perp2 · n_perp n_perpᵀ
        // where n_along = (nax, nay), n_perp = (-nay, nax).
        let a = inv_along2 * nax * nax + inv_perp2 * nay * nay;
        let b = (inv_along2 - inv_perp2) * nax * nay;
        let c = inv_along2 * nay * nay + inv_perp2 * nax * nax;
        Some(SymMat2::new(a, b, c))
    }
}

/// Tobler pace (s/m) given the gradient magnitude `|∇z|` (tangent
/// of the slope, dimensionless). Floor at base flat pace is the
/// caller's responsibility.
#[inline]
fn tobler_pace(grad_mag: f32) -> f32 {
    let v = 1.6667 * (-3.5 * (grad_mag.abs() + 0.05)).exp();
    if v < 1e-4 { 1.0e6 } else { 1.0 / v }
}

/// Bake an anisotropic corridor: walk every cell, build its metric
/// tensor, run Selling reduction, write a `CellForm` to the grid.
/// Refused cells get `CellForm::refused()` (rhs = +∞).
///
/// The eikonal RHS is `h²` where `h = shape.cell_m`: F(∇u) = 1 is
/// dimensionless, ∇u has units of s/m, so F · h ≈ Δu has units of
/// seconds; we square to fit the AGSI quadratic.
pub fn bake_aniso_corridor<E: Elevation>(
    shape: GridShape,
    metric: &ToblerAnisotropic<E>,
) -> FmmGrid<CellForm> {
    let h = shape.cell_m;
    let rhs = (h * h) as f32;
    let mut grid: FmmGrid<CellForm> = FmmGrid::filled(shape, CellForm::refused());
    for j in 0..shape.ny {
        for i in 0..shape.nx {
            let Some(m) = metric.metric_at(&shape, i, j) else { continue; };
            let Some(norm) = selling_reduce(m) else {
                // Fall back to axis-aligned isotropic at base pace.
                // Eigenvalues are 1/τ² (dual metric).
                let p = metric.base_pace_s_per_m * metric.off_trail_factor;
                let inv_p2 = 1.0 / (p * p);
                let fallback = NormForm {
                    offsets: [[1, 0, 0], [0, 1, 0], [0, 0, 0]],
                    weights: [inv_p2, inv_p2, 0.0],
                    n_terms: 2,
                };
                grid.set(i, j, 0, CellForm { norm: fallback, rhs });
                continue;
            };
            grid.set(i, j, 0, CellForm { norm, rhs });
        }
    }
    grid
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::aniso::solve_2d_anisotropic;
    use crate::solve::StopCondition;
    use crate::tobler::ArrayElevation;

    fn ramp_dem(n: u32, slope_deg: f32) -> ArrayElevation {
        // Constant-slope ramp: z increases linearly with the x index.
        // Gradient is purely +x, so the contour direction is +y.
        let tan_s = slope_deg.to_radians().tan();
        let rise_per_cell = tan_s * 10.0; // cell_m = 10 m, see caller
        let mut data = Vec::with_capacity((n * n) as usize);
        for j in 0..n {
            for i in 0..n {
                let _ = j;
                data.push(Some(i as f32 * rise_per_cell));
            }
        }
        ArrayElevation { data, nx: n, ny: n }
    }

    #[test]
    fn contour_following_cheaper_than_climbing_on_ramp() {
        // Constant-slope ramp at 30°. Anisotropic Tobler:
        //   τ_along (east, up the gradient) ≈ 5.4 s/m  (Tobler @ 30°)
        //   τ_perp  (north, along contour) ≈ 0.714 s/m (base flat)
        // ratio ≈ 7.5× — modest, well within the anisotropy cap.
        //
        // Sample two cells at equal Euclidean distance (10 cells) from
        // the seed:
        //   east  = climbing the ramp → expensive
        //   north = following the contour at constant elevation → cheap
        let n = 41u32;
        let cell_m = 10.0;
        let shape = GridShape::new_2d(n, n, 0.0, 0.0, cell_m);
        let elev = ramp_dem(n, 30.0);
        let metric = ToblerAnisotropic {
            elev,
            refuse_above_deg: 60.0,
            base_pace_s_per_m: 0.714,
            off_trail_factor: 1.0,
            gain_factor_k: 0.0,
        };
        let forms = bake_aniso_corridor(shape, &metric);
        let centre = n / 2;
        let r = solve_2d_anisotropic(
            shape, &forms,
            &[(centre, centre, 0.0)],
            StopCondition::AllAccepted,
        );
        let u_east = r.arrival.get(centre + 10, centre, 0);
        let u_north = r.arrival.get(centre, centre + 10, 0);
        eprintln!("u_east (climbing) = {u_east}, u_north (contour) = {u_north}, ratio = {}", u_east / u_north);
        assert!(
            u_north < u_east,
            "contour should be cheaper than climbing; \
             u_east={u_east}, u_north={u_north}"
        );
        // Sanity: north arrival ≈ 10 cells × base pace × cell_m
        //  ≈ 10 × 0.714 × 10 = 71.4 seconds (allow generous slack).
        assert!(u_north < 80.0 && u_north > 60.0,
            "u_north out of expected range: {u_north}");
        // East should be much slower because Tobler at 30° is ~7× base.
        let ratio = u_east / u_north;
        assert!(ratio > 3.0, "expected >3× contour preference; got {ratio}");
    }

    #[test]
    fn flat_terrain_reproduces_isotropic_arrival() {
        // On perfectly flat terrain, anisotropic Tobler must
        // degenerate to the isotropic base pace, so the wave from
        // a single seed must spread radially.
        let n = 31u32;
        let cell_m = 10.0;
        let shape = GridShape::new_2d(n, n, 0.0, 0.0, cell_m);
        let elev = ArrayElevation {
            data: vec![Some(100.0); (n * n) as usize],
            nx: n, ny: n,
        };
        let metric = ToblerAnisotropic {
            elev,
            refuse_above_deg: 45.0,
            base_pace_s_per_m: 0.714,
            off_trail_factor: 1.0,
            gain_factor_k: 0.0,
        };
        let forms = bake_aniso_corridor(shape, &metric);
        let centre = n / 2;
        let r = solve_2d_anisotropic(
            shape, &forms,
            &[(centre, centre, 0.0)],
            StopCondition::AllAccepted,
        );
        // Cells at equal Euclidean distance should have ≈ equal
        // arrival times within FMM discretisation error.
        let u_east = r.arrival.get(centre + 10, centre, 0);
        let u_north = r.arrival.get(centre, centre + 10, 0);
        let u_ne = r.arrival.get(centre + 7, centre + 7, 0);
        assert!((u_east - u_north).abs() < 1e-3 * u_east);
        // 7² + 7² = 98 vs 10² = 100, so u_ne should be close to
        // u_east (slightly less). Tolerance generous because Sethian
        // diagonal updates have ~10% error.
        let expected_ne = u_east * (98.0_f32.sqrt() / 10.0);
        assert!((u_ne - expected_ne).abs() < 0.2 * u_east,
            "u_east={u_east}, u_north={u_north}, u_ne={u_ne}, expected_ne={expected_ne}");
    }
}
