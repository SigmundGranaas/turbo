//! **L3 — artifact adapter.** Today's mmap'd Norwegian primitives,
//! presented as the engine's model shapes.
//!
//! See `docs/architecture/2026-07-routing-engine-module-design.md` §7.4.
//!
//! This crate is the *only* place where the concrete artifact types meet
//! the engine's ports. It depends on `turbo-route-model` and the
//! artifact readers; it must never depend on the engine. The engine
//! receives `Arc<dyn Heightfield>` and cannot tell an mmap'd DEM tile
//! pyramid from an array in a test.
//!
//! Why a newtype rather than `impl Heightfield for Dem`: the orphan
//! rules would force that impl to live in either the model crate (which
//! would then have to depend on the artifact reader — destroying its
//! zero-dependency invariant) or in the artifact crate (which would make
//! every consumer of `Dem` carry the routing vocabulary). Neither is
//! acceptable, and the wrapper is the standard answer: adaptation lives
//! in the adapter.

#![forbid(unsafe_code)]

use std::sync::Arc;

use turbo_route_model::{Heightfield, Point, SlopeAspect};
use turbo_tiles_elev::{Dem, PointXY};

/// The `norway.dem` artifact as a [`Heightfield`].
///
/// Holds an `Arc<Dem>` rather than a `Dem` so the same mmap and
/// decompressed-tile cache is shared by every contributor in a stack —
/// the DEM is the single hottest resource in the solver (135 ns a
/// sample, dominated by the tile lookup) and duplicating it would
/// duplicate the cache along with it.
pub struct DemHeightfield {
    dem: Arc<Dem>,
}

impl DemHeightfield {
    pub fn new(dem: Arc<Dem>) -> Self {
        Self { dem }
    }

    /// The wrapped artifact, for callers that legitimately need the
    /// concrete type — the `/v1/elev` and `/v1/slope` endpoints serve
    /// the DEM directly and are not routing.
    pub fn dem(&self) -> &Arc<Dem> {
        &self.dem
    }
}

impl From<Arc<Dem>> for DemHeightfield {
    fn from(dem: Arc<Dem>) -> Self {
        Self::new(dem)
    }
}

/// The one coordinate translation in the adapter: the engine's planar
/// `Point` is the artifact's `PointXY`, both metres in the same frame.
/// This is a rename, not a projection — the engine never learns that the
/// frame is EPSG:25833, and this function does not tell it.
///
/// Public because the DEM-serving HTTP endpoints (`/v1/elev`,
/// `/v1/slope`) legitimately hold both vocabularies: they project a
/// request with `turbo-geo-frame` and then query the artifact directly,
/// without going through the engine at all.
#[inline]
pub fn xy(p: Point) -> PointXY {
    PointXY { x: p.x, y: p.y }
}

/// The inverse rename.
#[inline]
pub fn point(p: PointXY) -> Point {
    Point { x: p.x, y: p.y }
}

impl Heightfield for DemHeightfield {
    #[inline]
    fn height_at(&self, p: Point) -> Option<f32> {
        self.dem.sample(xy(p)).ok().flatten()
    }

    /// `Dem::sample` returns `Err(OutOfCoverage)` outside the tile set
    /// and `Ok(None)` for a nodata cell inside it — so `is_ok()` is
    /// exactly the authority question, and the `Ok(None)` case correctly
    /// reports "covered, but I do not know".
    #[inline]
    fn covers(&self, p: Point) -> bool {
        self.dem.sample(xy(p)).is_ok()
    }

    #[inline]
    fn slope_aspect_at(&self, p: Point) -> Option<SlopeAspect> {
        self.dem
            .slope_aspect(xy(p))
            .ok()
            .flatten()
            .map(|sa| SlopeAspect {
                slope_deg: sa.slope_deg,
                aspect_deg: sa.aspect_deg,
            })
    }
}

/// Erase an artifact DEM to the engine's port. The idiomatic call at a
/// composition root.
pub fn heightfield(dem: Arc<Dem>) -> Arc<dyn Heightfield> {
    Arc::new(DemHeightfield::new(dem))
}
