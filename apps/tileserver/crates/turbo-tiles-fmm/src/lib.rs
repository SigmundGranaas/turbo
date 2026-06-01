//! Fast Marching Method solver for the off-trail pathfinder.
//!
//! Phase 1 ships a 2D isotropic eikonal solver. Subsequent phases
//! add anisotropic Finsler (Tobler), state-augmented (x, y, θ)
//! curvature-penalised metrics (Euler-elastica), path extraction by
//! gradient descent, and cost-aware smoothing.
//!
//! The crate has zero dependency on `turbo-tiles-pathfind` —
//! coupling goes the other way: pathfind's adapter (added in
//! phase 3) constructs a `Metric` impl from its `CostContributor`
//! stack and feeds it to this crate's solver.

pub mod aniso;
pub mod elastica;
pub mod extract;
pub mod grid;
pub mod heap;
pub mod metric;
pub mod selling;
pub mod smooth;
pub mod solve;
pub mod stencil;
pub mod tobler;
pub mod tobler_aniso;

pub use extract::{
    extract_path, extract_path_aniso, extract_path_discrete, ExtractError, PathPoint,
};
pub use smooth::chaikin_smooth_cost_aware;

pub use aniso::{solve_2d_anisotropic, CellForm};
pub use elastica::{
    extract_path_lifted, solve_lifted_grade_limited, tobler_pace, ArrayOverlay, CellOverlay,
    GradeLimitedCost, LiftedProgress, LiftedResult, N_HEADINGS,
};
pub use grid::{FmmGrid, GridShape};
pub use metric::{LocalCost, Metric, NormForm, UniformMetric};
pub use selling::{selling_reduce, SymMat2};
pub use solve::{
    bake_metric_2d, solve_2d_isotropic, solve_2d_with_metric, FmmResult, NodeState, StopCondition,
};
pub use tobler::{ArrayElevation, Elevation, ToblerIsotropic};
pub use tobler_aniso::{bake_aniso_corridor, ToblerAnisotropic};
