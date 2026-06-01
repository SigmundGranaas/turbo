//! Cost-field abstraction the FMM solver consumes.
//!
//! Phase 2 surface: per-cell scalar speed `F(x)` (or equivalently
//! cost `f = 1/F`), with optional veto. The trait is shaped so the
//! later phases (anisotropic Tobler-Finsler via AGSI decomposition;
//! state-augmented Euler-elastica with curvature) can extend it
//! without breaking phase 2's calling convention.
//!
//! The trait is `Send + Sync` so the solver can be parallelised
//! later (e.g. for tile-blocked solves on huge corridors). Today
//! the solve loop is single-threaded; that's fine for the typical
//! 10 km × 1.5 km corridor.

use crate::grid::GridShape;

/// AGSI-reduced lattice basis used by the anisotropic 2D stencil
/// (phase 5). A norm form encodes the local Finsler/Riemannian
/// metric as a weighted sum of three rank-1 quadratics over Z²
/// lattice offsets:
///
///   `F(v)² = Σ_k w_k · (offset_k · v)²`
///
/// The `offset_k` are short integer vectors (almost always within
/// `{-2, -1, 0, 1, 2}` per axis for sane anisotropy); they double
/// as the stencil's *upwind direction* — the anisotropic FMM update
/// reads `u` at the cell + offset_k position and combines the
/// per-direction updates via a generalised Sethian quadratic.
///
/// Stored as fixed-size arrays to avoid allocations on the hot
/// per-cell bake path.
#[derive(Debug, Clone, Copy, Default)]
pub struct NormForm {
    /// Lattice offsets `[i, j, k]` in cell-index units. The third
    /// coordinate is the θ-band offset; 0 in phase 5's 2D case.
    pub offsets: [[i8; 3]; 3],
    /// Per-direction weights. Non-negative when the form is
    /// successfully Selling-reduced.
    pub weights: [f32; 3],
    /// How many of the (offset, weight) pairs are active. Always
    /// 3 in 2D; 6 in 3D (phase 5 follow-up).
    pub n_terms: u8,
}

/// A cell's local cost.
///
/// Phase 2 stores only an isotropic scalar `f = 1/F` (seconds per
/// metre); phase 5 will add direction-dependent terms via either
/// extending this enum or a parallel `AnisotropicCost` type.
#[derive(Debug, Clone, Copy, PartialEq)]
pub enum LocalCost {
    /// Walkable cell at the given pace (s/m). Positive finite.
    Walkable { pace_s_per_m: f32 },
    /// Refused. Stencil treats this as `f = +∞`; cell will never be
    /// accepted. Adjacent cells route around it.
    Refused,
}

impl LocalCost {
    pub fn pace(self) -> f32 {
        match self {
            LocalCost::Walkable { pace_s_per_m } => pace_s_per_m,
            LocalCost::Refused => f32::INFINITY,
        }
    }
}

/// Lookup of per-cell cost in seconds-per-metre. Implementations
/// hand back `LocalCost` for any `(i, j, k)` triple. `k` is the
/// θ-band index in phase 5; phase 2 implementations ignore it.
///
/// The solver evaluates the metric **once per cell**, during a
/// pre-bake pass at corridor construction. That keeps the hot
/// relaxation loop reading from a flat `Vec<f32>` instead of going
/// through dynamic dispatch on every neighbour visit.
pub trait Metric: Send + Sync {
    /// Number of state dimensions. 2 in phase 2; 3 in phase 5
    /// (the (x, y, θ) elastica extension).
    fn dim(&self) -> usize {
        2
    }

    /// Cell cost at the given grid index.
    fn local(&self, shape: &GridShape, i: u32, j: u32, k: u32) -> LocalCost;
}

/// Trivial metric used by the phase-1 sanity tests: constant `F`
/// everywhere. Useful as a baseline against which terrain-aware
/// metrics are compared.
pub struct UniformMetric {
    pub pace_s_per_m: f32,
}

impl Metric for UniformMetric {
    fn local(&self, _shape: &GridShape, _i: u32, _j: u32, _k: u32) -> LocalCost {
        LocalCost::Walkable {
            pace_s_per_m: self.pace_s_per_m,
        }
    }
}
