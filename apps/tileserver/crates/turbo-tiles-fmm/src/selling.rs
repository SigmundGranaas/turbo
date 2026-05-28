//! 2D Selling / Lagrange-Gauss lattice basis reduction.
//!
//! Given a 2×2 SPD matrix `M = [[a, b], [b, c]]`, find three lattice
//! offsets `(v_k ∈ Z²)` and three non-negative weights `(w_k)` such
//! that for every world vector `v`:
//!
//! ```text
//!   vᵀ M v = Σ_k w_k · (v_k · v)²
//! ```
//!
//! This is the Voronoi/Selling decomposition: every SPD form on a
//! 2D lattice admits such a decomposition over its three "obtuse"
//! superbase vectors. The anisotropic FMM stencil uses the `(v_k,
//! w_k)` pairs as its upwind directions and per-direction weights
//! — Mirebeau's "AGSI" (Adaptive Geometric Stencil) approach, the
//! workhorse of the 2014 SINUM paper.
//!
//! ## Algorithm
//!
//! Lagrange-Gauss reduction of the basis `(v1, v2)`:
//!   1. Compute `m_ij = v_i · M · v_j` in the current basis.
//!   2. If `|2·m12| > m11` (i.e. `v2` has a large projection on `v1`
//!      under the M-metric), subtract the rounded multiple of `v1`
//!      from `v2` and restart.
//!   3. If `m22 < m11`, swap `v1` and `v2` and restart.
//!   4. Otherwise the basis is "M-reduced": `m11 ≤ m22` and
//!      `|2·m12| ≤ m11`.
//!
//! Once reduced, set `v3 = ±(v1 + v2)` with the sign chosen so the
//! cross-term is captured with a non-negative weight, and read off
//! the weights from the closed-form decomposition.
//!
//! ## Refused / very anisotropic inputs
//!
//! The iteration must converge: at each "size-reduction" step the
//! quantity `m11 + m22` strictly decreases (or stays equal once
//! reduced). A safety counter caps the loop at 32 iterations —
//! generous for any non-pathological SPD with reasonable condition
//! number. Pathological inputs (extreme anisotropy, near-singular
//! M) return `None` rather than producing a buggy decomposition;
//! the metric layer falls back to isotropic at that cell.

use crate::metric::NormForm;

/// Maximum Lagrange-Gauss iterations. 32 is well above the
/// theoretical bound (~log₂(κ(M))) for sane condition numbers.
const MAX_ITERS: u32 = 32;

/// 2D SPD form. Triangular storage; off-diagonal stored once.
#[derive(Debug, Clone, Copy)]
pub struct SymMat2 {
    pub a: f64, // M[0][0]
    pub b: f64, // M[0][1] = M[1][0]
    pub c: f64, // M[1][1]
}

impl SymMat2 {
    pub fn new(a: f64, b: f64, c: f64) -> Self { Self { a, b, c } }

    /// `vᵀ M v` for any world-coord vector.
    #[inline]
    pub fn quad(&self, v: (i32, i32)) -> f64 {
        let vx = v.0 as f64;
        let vy = v.1 as f64;
        self.a * vx * vx + 2.0 * self.b * vx * vy + self.c * vy * vy
    }

    /// `v1ᵀ M v2` cross-form.
    #[inline]
    pub fn cross(&self, v1: (i32, i32), v2: (i32, i32)) -> f64 {
        let x1 = v1.0 as f64;
        let y1 = v1.1 as f64;
        let x2 = v2.0 as f64;
        let y2 = v2.1 as f64;
        self.a * x1 * x2 + self.b * (x1 * y2 + y1 * x2) + self.c * y1 * y2
    }
}

/// Run the Lagrange-Gauss reduction + Selling decomposition.
///
/// Selling's theorem (2D version): for an SPD form `M`, there exists
/// a "superbase" `(b_0, b_1, b_2)` with `b_0 + b_1 + b_2 = 0` and
/// `M(b_i, b_j) ≤ 0` ∀ `i ≠ j` (obtuse condition). Then
///
/// ```text
///   M = Σ_{i<j} (-M(b_i, b_j)) · b_k^⊥ b_k^{⊥T}
/// ```
///
/// where `b_k` is the *third* superbase vector and `b_k^⊥` is its
/// 90°-rotation (still an integer lattice vector). The non-negative
/// weights `-M(b_i, b_j)` and the integer offsets `b_k^⊥` are exactly
/// what the AGSI stencil consumes.
///
/// We obtain the obtuse superbase from a Lagrange-Gauss-reduced
/// basis `(v_1, v_2)`: when `m12 ≤ 0` set `(b_0, b_1, b_2) = (v_1,
/// v_2, -v_1 - v_2)`; otherwise flip a sign first.
///
/// Returns `None` if the iteration fails to converge.
pub fn selling_reduce(m: SymMat2) -> Option<NormForm> {
    if m.a <= 0.0 || m.c <= 0.0 {
        return None;
    }
    let det = m.a * m.c - m.b * m.b;
    if det <= 0.0 {
        return None;
    }

    // Lagrange-Gauss reduction: find (v1, v2) ∈ Z² s.t.
    //   M(v1, v1) ≤ M(v2, v2)  and  |2 M(v1, v2)| ≤ M(v1, v1).
    let mut v1: (i32, i32) = (1, 0);
    let mut v2: (i32, i32) = (0, 1);
    let mut converged = false;
    for _ in 0..MAX_ITERS {
        let m11 = m.quad(v1);
        let m22 = m.quad(v2);
        let m12 = m.cross(v1, v2);
        // Size-reduction step: only when the off-diagonal *strictly*
        // exceeds m11/2. The strict check matters when |m12| = m11/2
        // exactly — rounding away from zero would oscillate v2 between
        // two equivalent reduced bases.
        if m11 > 0.0 && 2.0 * m12.abs() > m11 {
            let k = (m12 / m11).round() as i32;
            if k != 0 {
                v2 = (v2.0 - k * v1.0, v2.1 - k * v1.1);
                continue;
            }
        }
        if m22 < m11 {
            std::mem::swap(&mut v1, &mut v2);
            continue;
        }
        converged = true;
        break;
    }
    if !converged {
        return None;
    }

    // Build the obtuse superbase from the reduced basis. When the
    // off-diagonal `m12 > 0`, flip v2's sign so the new off-diagonal
    // is non-positive — both reduction-bound inequalities still hold.
    let m12 = m.cross(v1, v2);
    let (b0, b1, b2) = if m12 <= 0.0 {
        let b2 = (-v1.0 - v2.0, -v1.1 - v2.1);
        (v1, v2, b2)
    } else {
        let v2n = (-v2.0, -v2.1);
        let b2 = (-v1.0 - v2n.0, -v1.1 - v2n.1);
        (v1, v2n, b2)
    };

    // Weights = -M(b_i, b_j); each pair contributes one term, with
    // the integer offset given by the perpendicular of the THIRD
    // superbase vector. Perp((x, y)) = (-y, x) — still integer.
    let w01 = -m.cross(b0, b1);
    let w02 = -m.cross(b0, b2);
    let w12 = -m.cross(b1, b2);
    let perp = |v: (i32, i32)| (-v.1, v.0);
    let e2 = perp(b2); // partners with the (0,1) pair → weight w01
    let e1 = perp(b1); // partners with (0,2) → weight w02
    let e0 = perp(b0); // partners with (1,2) → weight w12

    // Clamp tiny negative weights (f64 rounding at the obtuse-edge).
    let w01 = w01.max(0.0);
    let w02 = w02.max(0.0);
    let w12 = w12.max(0.0);

    // Bail if any offset exceeds i8 range (shouldn't happen for
    // sane anisotropy; the test corpus stays well within ±12).
    let in_range = |v: (i32, i32)| v.0.abs() <= i8::MAX as i32 && v.1.abs() <= i8::MAX as i32;
    if !in_range(e0) || !in_range(e1) || !in_range(e2) {
        return None;
    }

    Some(NormForm {
        offsets: [
            [e2.0 as i8, e2.1 as i8, 0],
            [e1.0 as i8, e1.1 as i8, 0],
            [e0.0 as i8, e0.1 as i8, 0],
        ],
        weights: [w01 as f32, w02 as f32, w12 as f32],
        n_terms: 3,
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    /// Sanity: the decomposition reconstructs M to within float
    /// rounding error on a random adversarial sample.
    fn assert_decomposition_reconstructs(m: SymMat2) {
        let nf = selling_reduce(m).expect("should reduce");
        // Reconstruct vᵀ M v from the decomposition for several
        // test vectors and compare against direct evaluation.
        for &v in &[(1, 0), (0, 1), (1, 1), (2, -3), (-5, 7), (3, 5)] {
            let direct = m.quad(v);
            let mut reconstructed = 0.0_f64;
            for k in 0..nf.n_terms as usize {
                let offset = (nf.offsets[k][0] as i32, nf.offsets[k][1] as i32);
                let dot = (offset.0 * v.0 + offset.1 * v.1) as f64;
                reconstructed += nf.weights[k] as f64 * dot * dot;
            }
            assert!(
                (direct - reconstructed).abs() < 1e-5 * direct.abs().max(1.0),
                "M = (a={}, b={}, c={}); v = {:?}; direct = {}, reconstructed = {}",
                m.a, m.b, m.c, v, direct, reconstructed
            );
        }
    }

    #[test]
    fn identity_matrix_axis_aligned() {
        // M = I → decomp into the three lattice basis vectors with
        // weights (1, 1, 0) — the third vector is unused, weight 0.
        assert_decomposition_reconstructs(SymMat2::new(1.0, 0.0, 1.0));
    }

    #[test]
    fn modest_anisotropy_along_axis() {
        // Along-x slow, along-y fast. Standard basis works directly.
        assert_decomposition_reconstructs(SymMat2::new(4.0, 0.0, 1.0));
        assert_decomposition_reconstructs(SymMat2::new(1.0, 0.0, 4.0));
    }

    #[test]
    fn off_diagonal_positive() {
        // Slope direction at +45°: off-diagonal positive.
        assert_decomposition_reconstructs(SymMat2::new(2.0, 1.0, 2.0));
    }

    #[test]
    fn off_diagonal_negative() {
        // Slope direction at -45°: off-diagonal negative.
        assert_decomposition_reconstructs(SymMat2::new(2.0, -1.0, 2.0));
    }

    #[test]
    fn high_anisotropy_22_5_deg() {
        // The challenging case I worked out by hand: at 22.5° slope
        // direction with strong anisotropy, the standard basis
        // FAILS the secondary inequality and Lagrange-Gauss has to
        // iterate. Verify the algorithm handles it.
        let lambda_along: f64 = 24.3;
        let lambda_perp: f64 = 0.714;
        let theta: f64 = 22.5_f64.to_radians();
        let (s, c) = theta.sin_cos();
        let a = lambda_along.powi(2) * c.powi(2) + lambda_perp.powi(2) * s.powi(2);
        let b = (lambda_along.powi(2) - lambda_perp.powi(2)) * s * c;
        let cc = lambda_along.powi(2) * s.powi(2) + lambda_perp.powi(2) * c.powi(2);
        assert_decomposition_reconstructs(SymMat2::new(a, b, cc));
    }

    #[test]
    fn weights_are_nonnegative() {
        // Over a range of slope angles + anisotropy ratios, every
        // weight should be non-negative.
        for ratio_db in [1, 3, 10, 30, 100] {
            let lambda_along: f64 = (ratio_db as f64).sqrt();
            let lambda_perp: f64 = 1.0;
            for deg in (0..180).step_by(7) {
                let theta = (deg as f64).to_radians();
                let (s, c) = theta.sin_cos();
                let a = lambda_along.powi(2) * c.powi(2) + lambda_perp.powi(2) * s.powi(2);
                let b = (lambda_along.powi(2) - lambda_perp.powi(2)) * s * c;
                let cc = lambda_along.powi(2) * s.powi(2) + lambda_perp.powi(2) * c.powi(2);
                let m = SymMat2::new(a, b, cc);
                let nf = selling_reduce(m).expect("reduce should not fail");
                for k in 0..nf.n_terms as usize {
                    assert!(
                        nf.weights[k] >= -1e-6,
                        "negative weight {} at angle {}, ratio {}",
                        nf.weights[k], deg, ratio_db
                    );
                }
            }
        }
    }

    #[test]
    fn singular_matrix_returns_none() {
        // Rank-deficient (b² = a·c) → not SPD → None.
        assert!(selling_reduce(SymMat2::new(1.0, 1.0, 1.0)).is_none());
        // Negative diagonal — invalid input.
        assert!(selling_reduce(SymMat2::new(-1.0, 0.0, 1.0)).is_none());
    }
}
