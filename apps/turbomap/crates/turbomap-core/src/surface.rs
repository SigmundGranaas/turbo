//! The Surface — the ground authority (plan slice D3, architecture
//! §III.1.e).
//!
//! One abstraction owns "what is the ground". Content queries — marker
//! anchoring, route draping, camera pitch clamping, hit-testing, and the
//! analytic shadow/AO heightfield — consume this trait ONLY, so none of them
//! know the ground is a texture. `HeightfieldSurface` (the impl on
//! [`Terrain`]) keeps per-pipeline vertex displacement as its private
//! implementation detail; a future `MeshSurface` (plan M-TIN) swaps the
//! implementation and the terrain codec, and every consumer of these queries
//! follows automatically.

use crate::geo::WorldPoint;
use crate::render::terrain::Terrain;

/// How ground geometry is provided to the render path. Consumers that only
/// *query* the ground never need this; the render path uses it to pick its
/// displacement strategy (and `inspect` reports it).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) enum GroundBinding {
    /// A shared DEM heightfield texture set — ground pipelines displace
    /// their own vertices from it (this representation's private detail).
    /// `halo_px` is the per-tile overscan the displacement shaders inset by.
    Heightfield { halo_px: u32 },
}

impl GroundBinding {
    pub(crate) fn kind(&self) -> &'static str {
        match self {
            GroundBinding::Heightfield { .. } => "heightfield",
        }
    }
}

/// A dense grid of ground elevations over a world-space square, row-major,
/// `dim × dim`, cell `(i, j)` sampled at `origin + (i, j)·cell`. Missing
/// coverage reads 0 m (sea level) — the same convention the shadow field
/// uses. Feeds analytic lighting so the tuned terrain look survives a move
/// to mesh ground (any Surface can produce one).
// Part of the Surface contract ahead of its first external consumer
// (M-TIN's mesh-ground validation); the shadow field uses the row API.
#[allow(dead_code)]
pub(crate) struct HeightGrid {
    pub dim: usize,
    /// Elevations in metres.
    pub heights: Vec<f32>,
}

/// The 2.5D ground authority: single-valued elevation, `z = f(x, y)`.
///
/// Elevations are METRES; conversion into world-z (× `meters_to_world` ×
/// exaggeration) stays with the caller, since it depends on the camera
/// latitude of the frame being rendered.
pub(crate) trait Surface {
    /// Ground elevation at a world point, stabilised for content anchoring:
    /// implementations must not let streaming churn snap the answer coarser
    /// once a fine sample was returned (markers flicker otherwise). `None`
    /// when nothing covers the point yet — callers treat it as flat.
    fn elevation_at(&self, w: WorldPoint) -> Option<f32>;

    /// Outward unit ground normal at a world point (world axes: x=E, y=S,
    /// z=up), from central differences of the raw elevation field. `[0,0,1]`
    /// where coverage is missing (flat).
    // Contract-complete ahead of its first consumer (architecture §III.1.e
    // names it; object placement / slope-aware styling arrive later).
    #[allow(dead_code)]
    fn normal_at(&self, w: WorldPoint) -> [f32; 3];

    /// Bulk raw sampler for the analytic shadow/AO heightfield: rows
    /// `[row0, row1)` of a `dim × dim` grid anchored at `origin` with world
    /// spacing `cell`. `f` receives `(index, elevation_m)` with the
    /// full-grid index `j·dim + i`, `None` where nothing is resident — the
    /// row split lets the caller amortise a big field across frames.
    fn sample_height_rows(
        &self,
        origin: (f64, f64),
        cell: f64,
        dim: usize,
        row0: usize,
        row1: usize,
        f: &mut dyn FnMut(usize, Option<f32>),
    );

    /// Dense elevation grid over a world square (missing coverage → 0 m).
    /// Provided in terms of [`Surface::sample_height_rows`].
    #[allow(dead_code)]
    fn height_grid(&self, origin: (f64, f64), cell: f64, dim: usize) -> HeightGrid {
        let mut heights = vec![0.0f32; dim * dim];
        self.sample_height_rows(origin, cell, dim, 0, dim, &mut |idx, e| {
            heights[idx] = e.unwrap_or(0.0);
        });
        HeightGrid { dim, heights }
    }

    /// How this surface provides ground geometry to the render path.
    fn ground_binding(&self) -> GroundBinding;
}

/// `HeightfieldSurface`: today's ground — the shared DEM tile cache. The
/// queries answer from the CPU-side height grids the cache keeps in
/// lock-step with the GPU textures; displacement itself stays inside the
/// ground pipelines (this impl's private detail).
impl Surface for Terrain {
    fn elevation_at(&self, w: WorldPoint) -> Option<f32> {
        self.cache.elevation_at_world_stable((w.x, w.y))
    }

    fn normal_at(&self, w: WorldPoint) -> [f32; 3] {
        // Central differences over ±2 CPU-grid cells' worth of world space at
        // the finest resident zoom (the grid is 128² per 256²-interior tile).
        let z = self.cache.finest_resident_zoom();
        let eps = 2.0 / (128.0 * (1u64 << z) as f64);
        let raw = |x: f64, y: f64| self.cache.elevation_at_world((x, y));
        let (Some(hx0), Some(hx1), Some(hy0), Some(hy1)) = (
            raw(w.x - eps, w.y),
            raw(w.x + eps, w.y),
            raw(w.x, w.y - eps),
            raw(w.x, w.y + eps),
        ) else {
            return [0.0, 0.0, 1.0];
        };
        // Slopes in metres per metre: world-xy → metres via the local
        // Mercator scale (cos(lat)·circumference), so the normal is unit in
        // physical space regardless of latitude.
        let lat = (std::f64::consts::PI * (1.0 - 2.0 * w.y)).sinh().atan();
        let metres_per_world = (lat.cos().abs() * 40_075_017.0).max(1.0);
        let dx_m = (2.0 * eps * metres_per_world) as f32;
        let n = glam::Vec3::new(-(hx1 - hx0) / dx_m, -(hy1 - hy0) / dx_m, 1.0).normalize_or_zero();
        if n == glam::Vec3::ZERO {
            [0.0, 0.0, 1.0]
        } else {
            [n.x, n.y, n.z]
        }
    }

    fn sample_height_rows(
        &self,
        origin: (f64, f64),
        cell: f64,
        dim: usize,
        row0: usize,
        row1: usize,
        f: &mut dyn FnMut(usize, Option<f32>),
    ) {
        self.cache
            .sample_grid_rows(origin, cell, dim, row0, row1, f);
    }

    fn ground_binding(&self) -> GroundBinding {
        GroundBinding::Heightfield {
            halo_px: self.cache.halo_px(),
        }
    }
}
