//! Capability ports — what the engine reasons *about*, rather than what it
//! reads *from*.
//!
//! The engine's cost model and solvers previously held `Arc<Dem>`: the
//! concrete mmap'd artifact type. That made "swap the DEM source or
//! resolution" mean editing `turbo-tiles-elev`, the crate every contributor
//! imports, and it is what blocks region packs, in-memory test fixtures and
//! any non-Norwegian data.
//!
//! `Heightfield` is the first of the four ports in the module design. It is
//! deliberately *not* named `ElevationSource`: "source" implies acquisition,
//! and the engine must never acquire anything. A game engine's terrain chunk
//! **is** a heightfield; it is not a source of one.
//!
//! ## Why `dyn` and not generics
//!
//! The design originally required "generics in the hot loop, `dyn` at the
//! edges", with monomorphisation as the mitigation for dispatch cost —
//! called out as the largest technical bet in the rationale. E2 measured it:
//! `Dem::sample` costs **135 ns** and `slope_aspect` **225 ns**, dominated
//! by the rstar tile lookup, while concrete / `dyn` / monomorphised-generic
//! land within half a nanosecond of each other and disagree in sign. Derived
//! solve-level penalty was −0.014% to −0.059% against a 2% budget.
//!
//! So the rule is dropped: `Arc<dyn Heightfield>` throughout, no generic
//! parameters, one fewer invariant to enforce forever.
//!
//! ## Two queries, not one
//!
//! `Dem::sample` returns `Result<Option<f32>>` and callers used the two
//! failure modes to mean different things — `is_ok()` for "inside the
//! extent, even if nodata" and `ok().flatten()` for "has a value here".
//! Conflating them is what made the coverage defect (D1/E6) easy to write,
//! so the port separates them explicitly: [`Heightfield::covers`] answers
//! authority, [`Heightfield::height_at`] answers value.

use turbo_tiles_elev::{Dem, DemCoverage, PointXY, SlopeAspect};

/// A continuous scalar field of terrain height over the plane.
///
/// Nothing here mentions files, tiles, formats, compression or resolution
/// *sources*. Anything that can answer "how high is it here" can drive the
/// engine.
pub trait Heightfield: Send + Sync {
    /// Height in metres, or `None` for no data at this point — whether
    /// because the point is outside the field or because the field has a
    /// hole there. Use [`Self::covers`] to distinguish.
    fn height_at(&self, p: PointXY) -> Option<f32>;

    /// Is this point inside the field's authoritative extent?
    ///
    /// `true` for a nodata hole *inside* coverage: the field is authoritative
    /// that it does not know. Only `false` outside the extent entirely. This
    /// is the distinction routing feasibility depends on.
    fn covers(&self, p: PointXY) -> bool;

    /// Local slope and aspect, or `None` where undefined.
    fn slope_aspect_at(&self, p: PointXY) -> Option<SlopeAspect>;

    /// The field's extent.
    fn extent(&self) -> DemCoverage;
}

impl Heightfield for Dem {
    #[inline]
    fn height_at(&self, p: PointXY) -> Option<f32> {
        self.sample(p).ok().flatten()
    }
    #[inline]
    fn covers(&self, p: PointXY) -> bool {
        self.sample(p).is_ok()
    }
    #[inline]
    fn slope_aspect_at(&self, p: PointXY) -> Option<SlopeAspect> {
        self.slope_aspect(p).ok().flatten()
    }
    #[inline]
    fn extent(&self) -> DemCoverage {
        self.coverage()
    }
}
