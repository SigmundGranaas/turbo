//! Anisotropic 2D FMM: AGSI stencil + Selling-reduced upwind.
//!
//! Built on the same narrow-band heap as the isotropic solver, but
//! each cell carries a `NormForm` — three lattice offsets with
//! per-direction weights. The Hopf-Lax update at a cell is:
//!
//!   `Σ_k w_k · (u_c − u_{n_k})² = h²`         (`u_c > u_{n_k}` ∀k)
//!
//! solved as a quadratic in `u_c`. When the candidate violates the
//! upwind constraint for some `k` (`u_c ≤ u_{n_k}`), drop that term
//! and re-solve. Worst case 3 sub-solves for 3-term forms.
//!
//! ## Why this is the "right" stencil
//!
//! On an axis-aligned anisotropy (Riemannian with eigenvectors
//! along x/y), the AGSI reduces to the per-axis Sethian update with
//! direction-dependent weights — wave propagates faster on the cheap
//! axis. On a 45°-tilted anisotropy, the third lattice direction
//! `v_3 = ±(v_1 + v_2)` activates and the diagonal contribution
//! re-creates Tobler's contour-following behaviour.
//!
//! For sane anisotropy ratios (≤ 50× along/perp pace) the offsets
//! stay in `{-2..2}` per axis after Lagrange-Gauss reduction, so the
//! stencil reads at most 6 distinct neighbour cells per update.

use crate::grid::{FmmGrid, GridShape};
use crate::heap::NarrowBandHeap;
use crate::metric::NormForm;
use crate::solve::{FmmResult, NodeState, StopCondition};

/// Per-cell baked AGSI form plus a per-cell "speed scale" `g²`. The
/// eikonal we discretise is `F(∇u)² · g² = 1`, so the right-hand
/// side of the cell update is `h²`. Both the form and the `g²`
/// (encoded as the form's overall scale) are baked once at corridor
/// build time by the metric layer.
#[derive(Debug, Clone, Copy)]
pub struct CellForm {
    pub norm: NormForm,
    /// `+∞` marks the cell as refused; the marching loop skips it.
    /// Otherwise this is the eikonal right-hand side scale (= h²
    /// for the basic Tobler-Finsler metric, since the metric is
    /// already pace² and the eikonal closes at 1).
    pub rhs: f32,
}

impl CellForm {
    pub fn refused() -> Self {
        Self {
            norm: NormForm {
                offsets: [[0; 3]; 3],
                weights: [0.0; 3],
                n_terms: 0,
            },
            rhs: f32::INFINITY,
        }
    }
    pub fn is_refused(&self) -> bool {
        !self.rhs.is_finite()
    }
}

impl Default for CellForm {
    fn default() -> Self {
        Self::refused()
    }
}

/// Solve the eikonal equation on a 2D grid using per-cell AGSI
/// forms. Companion to `solve_2d_isotropic`.
pub fn solve_2d_anisotropic(
    shape: GridShape,
    forms: &FmmGrid<CellForm>,
    seeds: &[(u32, u32, f32)],
    stop: StopCondition,
) -> FmmResult {
    debug_assert_eq!(shape.nz, 1, "2D anisotropic solver");
    let n = shape.len();
    let mut arrival: FmmGrid<f32> = FmmGrid::filled(shape, f32::INFINITY);
    let mut state: Vec<NodeState> = vec![NodeState::Far; n];
    let mut heap = NarrowBandHeap::with_cells(n);

    for &(si, sj, u0) in seeds {
        if si >= shape.nx || sj >= shape.ny {
            continue;
        }
        let flat = shape.idx(si, sj, 0);
        if forms.flat()[flat].is_refused() {
            continue;
        }
        if u0 < arrival.flat()[flat] {
            arrival.flat_mut()[flat] = u0;
            state[flat] = NodeState::Considered;
            heap.push(u0, flat as u32);
        }
    }

    let mut cells_accepted: u32 = 0;
    while let Some((u_a, cell_a)) = heap.pop_min() {
        let flat_a = cell_a as usize;
        if state[flat_a] == NodeState::Accepted {
            continue;
        }
        if arrival.flat()[flat_a] < u_a {
            continue;
        }
        state[flat_a] = NodeState::Accepted;
        cells_accepted += 1;
        let (ai, aj, _) = shape.unpack(flat_a);

        if let StopCondition::GoalReached { gi, gj } = stop {
            if ai == gi && aj == gj {
                return FmmResult {
                    arrival,
                    cells_accepted,
                };
            }
        }

        // Relax the symmetric pairs of neighbours given by the
        // form at `a`. AGSI's causality holds because the Selling-
        // reduced offsets are "short" and the per-direction weights
        // are non-negative.
        let form_a = forms.get(ai, aj, 0);
        if form_a.is_refused() {
            continue;
        }
        let nx_i = shape.nx as i32;
        let ny_i = shape.ny as i32;
        // Up to 3 directions × 2 signs = 6 candidate neighbours.
        // Stack-collected so we can release the read-borrow on
        // `forms`/`arrival`/`state` before mutating them.
        let mut candidates: [(usize, f32); 6] = [(0, f32::INFINITY); 6];
        let mut n_candidates = 0;
        for k in 0..form_a.norm.n_terms as usize {
            let off_i = form_a.norm.offsets[k][0] as i32;
            let off_j = form_a.norm.offsets[k][1] as i32;
            for &sign in &[-1i32, 1] {
                let bi = ai as i32 + sign * off_i;
                let bj = aj as i32 + sign * off_j;
                if bi < 0 || bj < 0 || bi >= nx_i || bj >= ny_i {
                    continue;
                }
                let bflat = shape.idx(bi as u32, bj as u32, 0);
                if state[bflat] == NodeState::Accepted {
                    continue;
                }
                let form_b = forms.get(bi as u32, bj as u32, 0);
                if form_b.is_refused() {
                    continue;
                }
                let candidate =
                    anisotropic_update_at(shape, forms, &arrival, &state, bi as u32, bj as u32);
                if candidate.is_finite() {
                    candidates[n_candidates] = (bflat, candidate);
                    n_candidates += 1;
                }
            }
        }
        for &(bflat, candidate) in &candidates[..n_candidates] {
            if candidate < arrival.flat()[bflat] {
                arrival.flat_mut()[bflat] = candidate;
                state[bflat] = NodeState::Considered;
                heap.decrease_key_or_insert(bflat as u32, candidate);
            }
        }
    }
    FmmResult {
        arrival,
        cells_accepted,
    }
}

/// Compute the anisotropic FMM update for cell `(bi, bj)` using
/// that cell's `CellForm` and its lattice-neighbour arrival times.
fn anisotropic_update_at(
    shape: GridShape,
    forms: &FmmGrid<CellForm>,
    arrival: &FmmGrid<f32>,
    state: &[NodeState],
    bi: u32,
    bj: u32,
) -> f32 {
    let form_b = forms.get(bi, bj, 0);
    if form_b.is_refused() {
        return f32::INFINITY;
    }
    let rhs = form_b.rhs as f64;

    // Gather upwind arrival values along each of the form's
    // lattice directions. For each direction k, the upwind value
    // is the min of `u(b + e_k)` and `u(b - e_k)` among ACCEPTED
    // neighbours. If neither is ACCEPTED that axis contributes ∞.
    let mut u_neigh = [f32::INFINITY; 3];
    let mut active = [false; 3];
    let nx = shape.nx as i32;
    let ny = shape.ny as i32;
    for k in 0..form_b.norm.n_terms as usize {
        let off_i = form_b.norm.offsets[k][0] as i32;
        let off_j = form_b.norm.offsets[k][1] as i32;
        let mut best = f32::INFINITY;
        for &sign in &[-1i32, 1] {
            let ni = bi as i32 + sign * off_i;
            let nj = bj as i32 + sign * off_j;
            if ni < 0 || nj < 0 || ni >= nx || nj >= ny {
                continue;
            }
            let nflat = shape.idx(ni as u32, nj as u32, 0);
            if state[nflat] == NodeState::Accepted {
                let u = arrival.flat()[nflat];
                if u < best {
                    best = u;
                }
            }
        }
        u_neigh[k] = best;
        active[k] = best.is_finite() && form_b.norm.weights[k] > 0.0;
    }
    if !active.iter().any(|&x| x) {
        return f32::INFINITY;
    }
    solve_subset(
        &form_b.norm.weights,
        &u_neigh,
        form_b.norm.n_terms as usize,
        rhs,
    )
}

/// Solve `Σ_{k ∈ S} w_k (u − u_neigh[k])² = rhs` over the upwind subset:
/// start with every active axis (finite neighbour, positive weight),
/// drop the worst axis that violates the upwind constraint
/// (`u_neigh[k] ≥ candidate`), and repeat until all remaining axes are
/// causal. Returns `+∞` when no causal subset yields a root. Shared by
/// the narrow-band update (Accepted-gated neighbours) and the fast-sweep
/// update (finite-arrival neighbours).
fn solve_subset(weights: &[f32], u_neigh: &[f32], n_terms: usize, rhs: f64) -> f32 {
    let mut subset: u8 = 0;
    for k in 0..n_terms {
        if u_neigh[k].is_finite() && weights[k] > 0.0 {
            subset |= 1 << k;
        }
    }
    while subset != 0 {
        let candidate =
            solve_anisotropic_quadratic(&weights[..n_terms], &u_neigh[..n_terms], subset, rhs);
        if !candidate.is_finite() {
            return f32::INFINITY;
        }
        let candidate_f32 = candidate as f32;
        let mut worst: Option<usize> = None;
        let mut worst_u = f32::NEG_INFINITY;
        for (k, &u) in u_neigh.iter().enumerate().take(n_terms) {
            if subset & (1 << k) != 0 && u >= candidate_f32 && u > worst_u {
                worst_u = u;
                worst = Some(k);
            }
        }
        match worst {
            None => return candidate_f32,
            Some(k) => subset &= !(1u8 << k),
        }
    }
    f32::INFINITY
}

/// Quadratic solve for the "all-active-axes" candidate.
/// `Σ_{k ∈ subset} w_k (u − u_neigh[k])² = rhs`.
fn solve_anisotropic_quadratic(weights: &[f32], u_neigh: &[f32], subset: u8, rhs: f64) -> f64 {
    let mut w = 0.0_f64;
    let mut s = 0.0_f64;
    let mut q = 0.0_f64;
    for k in 0..weights.len() {
        if subset & (1 << k) == 0 {
            continue;
        }
        let wk = weights[k] as f64;
        let un = u_neigh[k] as f64;
        w += wk;
        s += wk * un;
        q += wk * un * un;
    }
    if w <= 0.0 {
        return f64::INFINITY;
    }
    let discriminant = s * s - w * (q - rhs);
    if discriminant < 0.0 {
        // No valid root — fall back to the 1D upwind along the
        // smallest u_neigh in the active set.
        let mut best = f64::INFINITY;
        for k in 0..weights.len() {
            if subset & (1 << k) == 0 {
                continue;
            }
            let wk = weights[k] as f64;
            if wk <= 0.0 {
                continue;
            }
            let candidate = u_neigh[k] as f64 + (rhs / wk).sqrt();
            if candidate < best {
                best = candidate;
            }
        }
        return best;
    }
    (s + discriminant.sqrt()) / w
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::grid::GridShape;
    use crate::metric::NormForm;

    /// Build a uniform isotropic AGSI form (weights 1, 1, 0 on the
    /// standard basis — third term unused). The anisotropic solver
    /// should reproduce the isotropic Sethian behaviour for this
    /// degenerate input, matching the phase-1 solver's output up to
    /// the AGSI stencil's discretisation differences.
    fn uniform_isotropic_form(n: u32) -> FmmGrid<CellForm> {
        let shape = GridShape::new_2d(n, n, 0.0, 0.0, 1.0);
        // Build M = I in standard basis → weights (1, 1, 0).
        let norm = NormForm {
            offsets: [[1, 0, 0], [0, 1, 0], [0, 0, 0]],
            weights: [1.0, 1.0, 0.0],
            n_terms: 2,
        };
        let cell = CellForm { norm, rhs: 1.0 };
        FmmGrid::filled(shape, cell)
    }

    #[test]
    fn isotropic_baseline_matches_axes_exactly() {
        // F=1 → Σ w_k (u - u_n)² = h² = 1, weights = (1, 1, 0).
        // Plane-wave from x=0 edge propagates as u(i,j) = i exactly
        // (no diagonal mixing on the axes).
        let n = 20u32;
        let forms = uniform_isotropic_form(n);
        let shape = forms.shape;
        let seeds: Vec<(u32, u32, f32)> = (0..n).map(|j| (0, j, 0.0)).collect();
        let r = solve_2d_anisotropic(shape, &forms, &seeds, StopCondition::AllAccepted);
        for j in 0..n {
            for i in 0..n {
                let u = r.arrival.get(i, j, 0);
                let expected = i as f32;
                assert!(
                    (u - expected).abs() < 1e-4,
                    "cell ({i},{j}): u={} expected={}",
                    u,
                    expected
                );
            }
        }
    }

    #[test]
    fn anisotropic_axis_aligned_faster_along_cheap_direction() {
        // Dual-metric form with G*_x = 4 (so τ_x = 0.5, FAST) and
        // G*_y = 1 (τ_y = 1, slower). The eikonal Σ w_k Δu² = 1
        // gives along-x step Δu = √(1/4) = 0.5 per cell vs along-y
        // Δu = 1. After 10 cells: u_x = 5 < u_y = 10. The x cell
        // arrives sooner.
        let n = 41u32;
        let shape = GridShape::new_2d(n, n, 0.0, 0.0, 1.0);
        let norm = NormForm {
            offsets: [[1, 0, 0], [0, 1, 0], [0, 0, 0]],
            weights: [4.0, 1.0, 0.0],
            n_terms: 2,
        };
        let cell = CellForm { norm, rhs: 1.0 };
        let forms: FmmGrid<CellForm> = FmmGrid::filled(shape, cell);
        let centre = n / 2;
        let r = solve_2d_anisotropic(
            shape,
            &forms,
            &[(centre, centre, 0.0)],
            StopCondition::AllAccepted,
        );
        let u_x = r.arrival.get(centre + 10, centre, 0);
        let u_y = r.arrival.get(centre, centre + 10, 0);
        assert!(
            u_x < u_y,
            "along-x should arrive sooner; got u_x={u_x}, u_y={u_y}"
        );
        assert!((u_x - 5.0).abs() < 1e-3, "u_x expected 5.0, got {u_x}");
        assert!((u_y - 10.0).abs() < 1e-3, "u_y expected 10.0, got {u_y}");
    }

    #[test]
    fn refused_cell_stays_infinite() {
        let n = 10u32;
        let shape = GridShape::new_2d(n, n, 0.0, 0.0, 1.0);
        let norm = NormForm {
            offsets: [[1, 0, 0], [0, 1, 0], [0, 0, 0]],
            weights: [1.0, 1.0, 0.0],
            n_terms: 2,
        };
        let mut forms: FmmGrid<CellForm> = FmmGrid::filled(shape, CellForm { norm, rhs: 1.0 });
        forms.set(5, 5, 0, CellForm::refused());
        let r = solve_2d_anisotropic(shape, &forms, &[(0, 0, 0.0)], StopCondition::AllAccepted);
        assert!(r.arrival.get(5, 5, 0).is_infinite());
    }
}
