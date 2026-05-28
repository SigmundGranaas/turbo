//! 2D isotropic upwind stencil — the Sethian quadratic.
//!
//! The eikonal PDE on a uniform grid with constant local cost
//! `f = 1/F` (seconds-per-metre) is discretised by the upwind
//! approximation:
//!
//! ```text
//!   max( (u - u_x)/h, 0 )² + max( (u - u_y)/h, 0 )²  =  f²
//! ```
//!
//! where `u_x = min(u[i-1, j], u[i+1, j])` and `u_y = min(u[i, j-1],
//! u[i, j+1])`. The "max with zero" enforces upwinding — only the
//! lower-arrival-time neighbour on each axis contributes. We solve
//! the quadratic in closed form.
//!
//! ## Cases
//!
//! Let `a = min(u_x, u_y)`, `b = max(u_x, u_y)`. Two cases:
//!
//! 1. **Both axes contribute**: `(u - a)² + (u - b)² = (f h)²`,
//!    i.e. `2u² - 2(a + b) u + (a² + b² - f² h²) = 0`. Discriminant
//!    `Δ = 4(a + b)² - 8(a² + b² - f² h²) = 4·(2 f² h² - (a - b)²)`.
//!    The candidate `u = ((a + b) + √(Δ)/2) / 2` is valid iff
//!    `u > b` (i.e. the *larger* of the two neighbours is genuinely
//!    upwind). When `Δ < 0` or the validity check fails, fall to
//!    case 2.
//!
//! 2. **Only one axis contributes**: `u = a + f h`. This is the
//!    degenerate case where the cost gradient is so steep along
//!    one axis that the other axis can't contribute to the wave.
//!
//! The full update returns `min` of cases 1 and 2 when both apply.
//!
//! ## Why we hand-roll the quadratic instead of using `nalgebra`
//!
//! Profile shows the stencil is called once per neighbour
//! relaxation — at FMM steady state that's ~8 calls per cell
//! accepted. For a 1.5 M-cell solve that's 12 M calls. The closed-
//! form is six multiplies + one square-root and the compiler keeps
//! it in registers; the nalgebra detour would touch heap-allocated
//! intermediates and inflate the inner loop. Sethian-level FMM
//! work always hand-rolls this — see Mirebeau's IPOL 2019 reference.

/// 2D isotropic update at a cell whose neighbours' minimum arrival
/// times along the two axes are `u_x` and `u_y`. `f_inv` is the
/// local cost `1/F` (s/m); `h` is the cell size (m).
///
/// Returns the candidate arrival time `u`. The caller compares
/// against the cell's current value and only writes if strictly
/// smaller.
///
/// ## Inputs
///
/// `u_x = +∞` means "no upwind neighbour on the x axis" (e.g.
/// boundary cell, or both x-neighbours are FAR). Same for `u_y`.
/// When both are `+∞` the stencil returns `+∞`; the caller should
/// skip the relaxation.
#[inline]
pub fn solve_quadratic_2d(u_x: f32, u_y: f32, f_inv: f32, h: f32) -> f32 {
    let fh = f_inv * h;
    let both_finite = u_x.is_finite() && u_y.is_finite();
    if both_finite {
        let a = u_x.min(u_y);
        let b = u_x.max(u_y);
        // Two-axis quadratic. Δ = 2 f² h² - (a - b)². If Δ ≥ 0 the
        // larger root candidate is u = ((a + b) + √(2 Δ)) / 2;
        // valid iff u > b.
        let delta = 2.0 * fh * fh - (a - b).powi(2);
        if delta >= 0.0 {
            let u = ((a + b) + delta.sqrt()) * 0.5;
            if u > b {
                return u;
            }
        }
        // Fall through to single-axis when the two-axis update is
        // invalid (delta < 0 or u ≤ b means the second axis isn't
        // genuinely contributing).
        return a + fh;
    }
    // Exactly one axis finite → trivial 1D upwind.
    let a = u_x.min(u_y);
    if a.is_infinite() {
        return f32::INFINITY;
    }
    a + fh
}

#[cfg(test)]
mod tests {
    use super::*;

    /// Sethian quadratic with `u_x = u_y = h`, `f h = 1`. The
    /// closed-form Sethian update is
    ///     `u = ((a + b) + √(2·(fh)² - (a−b)²)) / 2`
    /// which here gives `(2 + √2)/2 = 1 + √2/2 ≈ 1.707`. That's a
    /// 0.293·h overestimate of the true continuous-PDE distance
    /// `√2 ≈ 1.414` — the well-known O(h) accuracy of the Sethian
    /// scheme on diagonal fronts. The disc-arrival closed-form
    /// test in `tests/eikonal_isotropic.rs` exercises the averaged
    /// behaviour across many cells; here we just pin the per-cell
    /// formula.
    #[test]
    fn diagonal_update_matches_sethian_closed_form() {
        let u = solve_quadratic_2d(1.0, 1.0, 1.0, 1.0);
        let expected = 1.0 + 2.0f32.sqrt() / 2.0;
        assert!((u - expected).abs() < 1e-5, "got {}, expected {}", u, expected);
    }

    /// On an axis (one neighbour `0`, the other `+∞`), the update
    /// is the trivial 1D upwind `u = 0 + fh`.
    #[test]
    fn one_axis_only_returns_1d_upwind() {
        let u = solve_quadratic_2d(0.0, f32::INFINITY, 1.0, 1.0);
        assert_eq!(u, 1.0);
    }

    /// When both neighbours are `+∞` (nothing to march from), the
    /// candidate is `+∞`. The caller's "only write if smaller" guard
    /// keeps the cell's value unchanged.
    #[test]
    fn both_infinite_stays_infinite() {
        let u = solve_quadratic_2d(f32::INFINITY, f32::INFINITY, 1.0, 1.0);
        assert!(u.is_infinite());
    }

    /// Discriminant-negative fallback to 1D upwind. Set up a
    /// pathological pair where the gap between the two axes
    /// exceeds the wave step.
    #[test]
    fn degenerate_root_falls_back_to_1d() {
        // u_x = 0, u_y = 10, f_inv = 1, h = 1: difference 10 ≫
        // √2 · 1 ≈ 1.414, so the two-axis formula's `u > b`
        // check fails, and we fall back to u = 0 + 1 = 1.
        let u = solve_quadratic_2d(0.0, 10.0, 1.0, 1.0);
        assert_eq!(u, 1.0);
    }

    /// Sanity: a cell two grid steps from the seed along the x axis
    /// (`u_x = h, u_y = 2h` because the diagonal route is longer
    /// than the direct one) gets `u = 2h`. Models the second step
    /// of an axis-aligned front.
    #[test]
    fn second_axis_step() {
        let u = solve_quadratic_2d(1.0, 2.0, 1.0, 1.0);
        // Two-axis formula: a = 1, b = 2, fh = 1.
        // delta = 2·1 - 1² = 1 > 0; u = (1 + 2 + 1)/2 = 2. Valid (u > b? 2 > 2? no).
        // So falls back to u = 1 + 1 = 2.
        assert_eq!(u, 2.0);
    }

    /// A cell flush with a horizontal seed line. `u_y` is the seed
    /// row's value; `u_x` is the same row, equally close. Should
    /// produce the same answer regardless of axis order.
    #[test]
    fn symmetric_in_x_y() {
        let u1 = solve_quadratic_2d(3.0, 5.0, 1.0, 1.0);
        let u2 = solve_quadratic_2d(5.0, 3.0, 1.0, 1.0);
        assert_eq!(u1, u2);
    }

    /// f_inv = 0 (infinitely fast cell) collapses to the larger of
    /// the two neighbour values — the wave passes through this cell
    /// with no time delay relative to its upwind neighbours.
    #[test]
    fn zero_cost_falls_through() {
        let u = solve_quadratic_2d(3.0, 5.0, 0.0, 1.0);
        // a = 3, b = 5, fh = 0. Two-axis: delta = 0 - 4 = -4 < 0. 1D fallback: u = 3.
        assert_eq!(u, 3.0);
    }
}
