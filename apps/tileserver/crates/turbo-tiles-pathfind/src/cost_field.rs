//! The lazy per-cell cost field — the CostField seam from the
//! routing-engine unification plan
//! (`docs/architecture/2026-06-routing-engine-unification-plan.md`).
//!
//! ONE implementation of "evaluate the contributor stack at a cell
//! centre, memoise the result, and answer `refused` / `pace_mul` /
//! `elevation` in O(1) thereafter", shared by BOTH routers:
//!
//!   - the FMM grade-limited off-trail solver (via the
//!     `turbo_tiles_fmm::CellOverlay` impl), and
//!   - the unified mesh ∪ trail A\* (direct method calls).
//!
//! It previously existed twice (`fmm_adapter::LazyContributorOverlay`
//! and `unified::MeshOverlay`) with byte-identical semantics — exactly
//! the duplication the plan's CostField seam removes. The grid is the
//! canonical [`GridShape`]; per-cell evaluation hands the contributor
//! stack ONE shared [`EdgeElevProbe`] so the slope-family contributors
//! sample the DEM once per point, not once per contributor.

use std::sync::Arc;

use turbo_tiles_elev::{Dem, PointXY};
use turbo_tiles_fmm::GridShape;

use crate::contributor::{CostContributor, EdgeContext, EdgeElevProbe, EdgeKind};

/// Lazily-evaluated per-cell cost field over a corridor grid.
///
/// Cell state: `0` = uncomputed, `1` = passable (with `mul`), `2` =
/// refused. Elevation memo: `+∞` = unsampled, `NaN` = nodata.
pub(crate) struct LazyCostField<'a> {
    shape: GridShape,
    dem: Arc<Dem>,
    base_pace: f32,
    profile: turbo_tiles_graph::Profile,
    contributors: &'a [Arc<dyn CostContributor>],
    state: std::cell::RefCell<Vec<u8>>,
    mul: std::cell::RefCell<Vec<f32>>,
    /// Per-cell cell-centre elevation memo. The mesh solvers query each
    /// cell's elevation up to ~16× (once per incident move); memoising
    /// collapses that to one `Dem::sample()` per cell. Sized lazily on
    /// first use: the grade-limited path never calls `elevation()` (its
    /// solver reads elevation through `fmm_adapter::DemElevation`, which
    /// carries its own per-cell memo), so eager allocation would zero
    /// ~0.5 MB per GL solve for nothing. Full unification of the two
    /// memos was considered and rejected: `fmm::Elevation` requires
    /// `Send + Sync` (this type is `!Sync` via `RefCell`), and since
    /// each path touches exactly one of the memos, no double-sampling
    /// actually occurs — the only waste was this allocation.
    elev: std::cell::RefCell<Vec<f32>>,
    refused_by: std::cell::RefCell<std::collections::BTreeSet<String>>,
}

impl<'a> LazyCostField<'a> {
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
            elev: std::cell::RefCell::new(Vec::new()),
            refused_by: std::cell::RefCell::new(std::collections::BTreeSet::new()),
        }
    }

    #[inline]
    fn idx(&self, i: u32, j: u32) -> usize {
        (j as usize) * (self.shape.nx as usize) + (i as usize)
    }

    /// Evaluate the contributor stack for cell `(i, j)` on first touch.
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
        let probe = EdgeElevProbe::new(&self.dem, cx - 0.5 * cell_m, cy, cx + 0.5 * cell_m, cy);
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

    /// Is this cell refused (water/glacier/building/…)?
    pub(crate) fn refused(&self, i: u32, j: u32) -> bool {
        self.ensure(i, j);
        self.state.borrow()[self.idx(i, j)] == 2
    }

    /// Composed pace multiplier for this cell (1.0 for refused cells —
    /// callers must check `refused` first; the solvers never price a
    /// refused cell).
    pub(crate) fn pace_mul(&self, i: u32, j: u32) -> f32 {
        self.ensure(i, j);
        let idx = self.idx(i, j);
        if self.state.borrow()[idx] == 2 {
            1.0
        } else {
            self.mul.borrow()[idx]
        }
    }

    /// Cell-centre elevation (m), sampled once per cell and memoised.
    /// `None` = no DEM coverage. Does NOT run the contributor stack.
    pub(crate) fn elevation(&self, i: u32, j: u32) -> Option<f32> {
        let idx = self.idx(i, j);
        {
            let mut m = self.elev.borrow_mut();
            if m.is_empty() {
                m.resize(
                    (self.shape.nx as usize) * (self.shape.ny as usize),
                    f32::INFINITY,
                );
            }
        }
        let cached = self.elev.borrow()[idx];
        if cached != f32::INFINITY {
            return if cached.is_nan() { None } else { Some(cached) };
        }
        let (cx, cy) = self.shape.cell_centre(i, j);
        let v = self.dem.sample(PointXY { x: cx, y: cy }).ok().flatten();
        self.elev.borrow_mut()[idx] = v.unwrap_or(f32::NAN);
        v
    }

    /// Count of cells evaluated as refused so far (diagnostics).
    pub(crate) fn refused_count(&self) -> u32 {
        self.state.borrow().iter().filter(|&&s| s == 2).count() as u32
    }

    /// Labels of the layers that refused at least one cell.
    pub(crate) fn refused_labels(&self) -> Vec<String> {
        self.refused_by.borrow().iter().cloned().collect()
    }
}

impl<'a> turbo_tiles_fmm::CellOverlay for LazyCostField<'a> {
    fn refused(&self, i: u32, j: u32) -> bool {
        LazyCostField::refused(self, i, j)
    }
    fn pace_mul(&self, i: u32, j: u32) -> f32 {
        LazyCostField::pace_mul(self, i, j)
    }
}
