//! Tobler-hiking-function metric for FMM.
//!
//! Phase 2: isotropic. The pace at each cell is computed from the
//! local slope *magnitude* only (not direction). A cell sitting on
//! a 30° slope returns the slow Tobler pace whether the wave is
//! traveling along the contour or up the fall line. That is
//! mathematically incomplete — true Tobler is direction-dependent —
//! and the contour-following behaviour we ultimately want requires
//! the anisotropic (Finsler) extension, which lands in phase 5 with
//! the state-augmented (x, y, θ) elastica metric. Phase 2's value
//! is correct *magnitude*: steep terrain becomes expensive enough
//! that the FMM wave reaches it later than the contour.
//!
//! ## Tobler's hiking function
//!
//! ```text
//!   v(s) = 1.6667 · exp(-3.5 · |tan(slope) + 0.05|)   [m/s]
//! ```
//!
//! where `slope` is the *signed* slope angle (positive uphill). The
//! minimum pace (max speed) is at `slope ≈ -0.05 rad ≈ -2.86°` — a
//! gentle descent is optimal. We use the unsigned slope magnitude
//! in phase 2.

use crate::grid::GridShape;
use crate::metric::{LocalCost, Metric};

/// Per-cell elevation sampler. The FMM crate doesn't depend on
/// `turbo-tiles-elev` directly — instead the adapter crate
/// (phase 3) implements this trait against `Dem` and hands it in.
/// This keeps the FMM core unit-testable with synthetic terrain.
pub trait Elevation: Send + Sync {
    /// Elevation at cell centre `(i, j)` in metres. `None` for
    /// nodata; the metric will refuse those cells.
    fn at(&self, shape: &GridShape, i: u32, j: u32) -> Option<f32>;
}

/// In-memory synthetic elevation grid — used by tests and the
/// disc-arrival example.
pub struct ArrayElevation {
    pub data: Vec<Option<f32>>,
    pub nx: u32,
    pub ny: u32,
}

impl Elevation for ArrayElevation {
    fn at(&self, _shape: &GridShape, i: u32, j: u32) -> Option<f32> {
        if i >= self.nx || j >= self.ny {
            return None;
        }
        self.data[(j as usize) * (self.nx as usize) + (i as usize)]
    }
}

/// Isotropic Tobler metric: per-cell pace as a function of the
/// slope magnitude (sampled via finite differences against the
/// `Elevation` source).
pub struct ToblerIsotropic<E: Elevation> {
    pub elev: E,
    /// Cells with slope above this threshold are refused.
    pub refuse_above_deg: f32,
    /// Pace floor — applied to flat terrain. Defaults to
    /// `1.0/1.4 = 0.714 s/m`, matching the Norway flat-trail
    /// baseline the rest of the cost model uses.
    pub base_pace_s_per_m: f32,
    /// Override the off-trail pace multiplier (defaults to 1.0; the
    /// pathfinder applies its `off_trail_base` factor on top of
    /// whatever this returns). Kept as a metric field so a future
    /// per-cell off-trail-base variation can plug in without
    /// changing the Metric trait shape.
    pub off_trail_factor: f32,
}

impl<E: Elevation> ToblerIsotropic<E> {
    /// Sample the slope magnitude at cell centre `(i, j)` by
    /// 2-point central differences against the 4 axis neighbours.
    /// Returns `None` when the cell or *any* of its 4 neighbours
    /// has nodata — we conservatively refuse cells we can't slope-
    /// classify. Edge cells get `None` for the same reason.
    fn slope_mag(&self, shape: &GridShape, i: u32, j: u32) -> Option<f32> {
        if i == 0 || j == 0 || i + 1 >= shape.nx || j + 1 >= shape.ny {
            return None;
        }
        let z_left = self.elev.at(shape, i - 1, j)?;
        let z_right = self.elev.at(shape, i + 1, j)?;
        let z_down = self.elev.at(shape, i, j - 1)?;
        let z_up = self.elev.at(shape, i, j + 1)?;
        let dz_dx = (z_right - z_left) / (2.0 * shape.cell_m as f32);
        let dz_dy = (z_up - z_down) / (2.0 * shape.cell_m as f32);
        Some((dz_dx * dz_dx + dz_dy * dz_dy).sqrt())
    }

    /// Tobler pace (s/m) given a slope tangent (rise/run).
    fn pace_from_grad(&self, grad: f32) -> f32 {
        // Treat as unsigned slope — phase 2 is isotropic. The
        // ascending and descending paces are conservatively equal
        // here, capped by the slowest Tobler value at that
        // magnitude.
        let v = 1.6667 * (-3.5 * (grad.abs() + 0.05)).exp();
        if v < 1e-4 {
            // Physically untraversable; flagged at the call site
            // via refuse_above_deg, but defend here too.
            return 1.0e6;
        }
        1.0 / v
    }
}

impl<E: Elevation> Metric for ToblerIsotropic<E> {
    fn local(&self, shape: &GridShape, i: u32, j: u32, _k: u32) -> LocalCost {
        let Some(grad) = self.slope_mag(shape, i, j) else {
            // Edge cell or nodata in any neighbour → return a safe
            // walkable cost so the wave can still propagate. The
            // path extractor in phase 4 will avoid these cells
            // implicitly via the slightly-higher arrival times.
            return LocalCost::Walkable {
                pace_s_per_m: self.base_pace_s_per_m * self.off_trail_factor,
            };
        };
        let slope_deg = grad.atan().to_degrees();
        if slope_deg > self.refuse_above_deg {
            return LocalCost::Refused;
        }
        let pace = self.pace_from_grad(grad);
        // Apply the larger of "Tobler at local slope" and the
        // configured base pace. Flat ground shouldn't be cheaper
        // than the base — that's the project-wide flat-trail
        // baseline the rest of the cost model is calibrated to.
        let final_pace = pace.max(self.base_pace_s_per_m) * self.off_trail_factor;
        LocalCost::Walkable {
            pace_s_per_m: final_pace,
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn flat(nx: u32, ny: u32) -> ArrayElevation {
        ArrayElevation {
            data: vec![Some(100.0); (nx * ny) as usize],
            nx,
            ny,
        }
    }

    #[test]
    fn flat_terrain_returns_base_pace() {
        let shape = GridShape::new_2d(10, 10, 0.0, 0.0, 10.0);
        let metric = ToblerIsotropic {
            elev: flat(10, 10),
            refuse_above_deg: 45.0,
            base_pace_s_per_m: 0.714,
            off_trail_factor: 1.0,
        };
        // Interior cell, all neighbours at 100 m → grad = 0 → Tobler
        // pace at 0 slope = 0.629 s/m; we floor at base_pace_s_per_m
        // = 0.714 → cell returns 0.714.
        let LocalCost::Walkable { pace_s_per_m } = metric.local(&shape, 5, 5, 0) else {
            panic!("flat cell shouldn't be refused");
        };
        assert!(
            (pace_s_per_m - 0.714).abs() < 1e-3,
            "expected ≈0.714, got {}",
            pace_s_per_m
        );
    }

    #[test]
    fn steep_slope_returns_higher_pace() {
        // 30° ramp: dz/dx = tan(30°) ≈ 0.577 over 10 m → rise of 5.77 m per cell
        let nx = 7u32;
        let ny = 7u32;
        let mut data = Vec::with_capacity((nx * ny) as usize);
        let rise = 5.77_f32;
        for j in 0..ny {
            for i in 0..nx {
                let _ = j;
                data.push(Some(i as f32 * rise));
            }
        }
        let elev = ArrayElevation { data, nx, ny };
        let shape = GridShape::new_2d(nx, ny, 0.0, 0.0, 10.0);
        let metric = ToblerIsotropic {
            elev,
            refuse_above_deg: 45.0,
            base_pace_s_per_m: 0.714,
            off_trail_factor: 1.0,
        };
        let LocalCost::Walkable { pace_s_per_m } = metric.local(&shape, 3, 3, 0) else {
            panic!("30° slope should be walkable");
        };
        // Tobler at 30° slope: tan(30°) = 0.577, exp(-3.5·0.627) ≈
        // 0.111, v ≈ 0.185 m/s, pace ≈ 5.39 s/m. Definitely much
        // higher than the 0.714 base pace.
        assert!(
            pace_s_per_m > 4.0,
            "30° slope should be slow; got {}",
            pace_s_per_m
        );
        assert!(
            pace_s_per_m < 7.0,
            "30° slope pace looks too extreme; got {}",
            pace_s_per_m
        );
    }

    #[test]
    fn slope_above_threshold_refused() {
        // 50° ramp; refuse_above_deg = 45° → refused.
        let nx = 5u32;
        let ny = 5u32;
        let rise = (50.0_f32.to_radians().tan()) * 10.0;
        let mut data = Vec::new();
        for j in 0..ny {
            for i in 0..nx {
                let _ = j;
                data.push(Some(i as f32 * rise));
            }
        }
        let elev = ArrayElevation { data, nx, ny };
        let shape = GridShape::new_2d(nx, ny, 0.0, 0.0, 10.0);
        let metric = ToblerIsotropic {
            elev,
            refuse_above_deg: 45.0,
            base_pace_s_per_m: 0.714,
            off_trail_factor: 1.0,
        };
        assert!(matches!(metric.local(&shape, 2, 2, 0), LocalCost::Refused));
    }
}
