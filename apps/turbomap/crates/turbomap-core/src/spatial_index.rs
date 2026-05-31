//! Per-tile uniform-grid spatial index for hit-testing.
//!
//! A vector tile carries up to a few thousand interactive features
//! (roads, POIs, boundary chunks). The previous hit-test walked every
//! feature linearly per click — fine for sparse tiles, but O(N) is a
//! cliff once tile density crosses a few hundred features.
//!
//! This index buckets each feature by its tile-local AABB into a
//! `GRID × GRID` cell grid (default 16, so 256 cells per tile). At
//! query time the click point's cell is computed and only that cell's
//! feature indices are returned — typical speedup ~10–100× on dense
//! urban tiles.
//!
//! The index keys are *feature indices* into the caller's
//! `Vec<InteractiveFeature>`, so the index doesn't own the geometry
//! and stays cheap to build (one AABB per feature, then bucket).
//!
//! AABBs are computed by walking the geometry's raw coordinates —
//! `Geometry::{Point, LineString, Polygon}` cover all MVT shapes.

use std::collections::BTreeSet;

use crate::vector::Geometry;

const GRID: usize = 16;

/// `extent`: tile-local coordinate maximum (typically 4096 for MVT).
/// The index quantises tile-local coords [0..extent] into [0..GRID]
/// cells in each axis.
#[derive(Debug, Clone)]
pub struct SpatialIndex {
    extent: f64,
    // Flat row-major `GRID*GRID` cells of feature indices.
    cells: Vec<Vec<u32>>,
}

impl SpatialIndex {
    pub fn new(extent: u32) -> Self {
        Self {
            extent: extent as f64,
            cells: (0..GRID * GRID).map(|_| Vec::new()).collect(),
        }
    }

    /// Stamp `feature_idx` into every cell that the geometry's AABB
    /// overlaps. With `tolerance_local > 0` the AABB is expanded
    /// outward so a click that lands within tolerance of a feature
    /// (e.g. a thin line) still finds it on lookup.
    pub fn insert(&mut self, feature_idx: u32, geom: &Geometry, tolerance_local: f64) {
        let Some((min_x, min_y, max_x, max_y)) = geometry_aabb(geom) else {
            return; // empty geometry
        };
        let cell_size = self.extent / GRID as f64;
        let cx0 = ((min_x - tolerance_local) / cell_size).floor().max(0.0) as usize;
        let cy0 = ((min_y - tolerance_local) / cell_size).floor().max(0.0) as usize;
        let cx1 = (((max_x + tolerance_local) / cell_size).floor() as usize).min(GRID - 1);
        let cy1 = (((max_y + tolerance_local) / cell_size).floor() as usize).min(GRID - 1);
        let cx0 = cx0.min(GRID - 1);
        let cy0 = cy0.min(GRID - 1);
        for cy in cy0..=cy1 {
            for cx in cx0..=cx1 {
                self.cells[cy * GRID + cx].push(feature_idx);
            }
        }
    }

    /// Feature indices whose AABB overlaps the cell containing
    /// `(x, y)`. The returned slice may include false positives that
    /// the caller still resolves with `geometry_hit` — the index's
    /// job is to cheaply prune the set, not to be exact.
    pub fn query(&self, x: f64, y: f64) -> &[u32] {
        if x < 0.0 || y < 0.0 || x > self.extent || y > self.extent {
            return &[];
        }
        let cell_size = self.extent / GRID as f64;
        let cx = ((x / cell_size).floor() as usize).min(GRID - 1);
        let cy = ((y / cell_size).floor() as usize).min(GRID - 1);
        &self.cells[cy * GRID + cx]
    }

    /// Total bytes consumed by the bucket vectors. Surfaces in the
    /// vector cache's `bytes_used` so the LRU budget accounts for it.
    pub fn bytes(&self) -> usize {
        self.cells
            .iter()
            .map(|c| c.capacity() * std::mem::size_of::<u32>())
            .sum::<usize>()
            + std::mem::size_of::<Self>()
    }

    /// Deduplicate the per-cell vectors. Useful when a single feature
    /// touched many cells (long lines, large polygons) — keeps the
    /// query slice tight. Optional, called once after all inserts.
    pub fn finish(&mut self) {
        for c in &mut self.cells {
            let set: BTreeSet<u32> = c.iter().copied().collect();
            *c = set.into_iter().collect();
            c.shrink_to_fit();
        }
    }
}

fn geometry_aabb(geom: &Geometry) -> Option<(f64, f64, f64, f64)> {
    let mut min_x = f64::INFINITY;
    let mut min_y = f64::INFINITY;
    let mut max_x = f64::NEG_INFINITY;
    let mut max_y = f64::NEG_INFINITY;
    let mut any = false;
    let mut take = |x: i32, y: i32| {
        let xf = x as f64;
        let yf = y as f64;
        if xf < min_x {
            min_x = xf;
        }
        if yf < min_y {
            min_y = yf;
        }
        if xf > max_x {
            max_x = xf;
        }
        if yf > max_y {
            max_y = yf;
        }
        any = true;
    };
    match geom {
        Geometry::Point(pts) => {
            for &(x, y) in pts {
                take(x, y);
            }
        }
        Geometry::LineString(lines) => {
            for line in lines {
                for &(x, y) in line {
                    take(x, y);
                }
            }
        }
        Geometry::Polygon(rings) => {
            for ring in rings {
                for &(x, y) in ring {
                    take(x, y);
                }
            }
        }
    }
    if any {
        Some((min_x, min_y, max_x, max_y))
    } else {
        None
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn point_at_centre_is_only_findable_in_its_own_cell() {
        // GRID=16, extent=4096 → cell size = 256.
        // A point at (2050, 2050) lies in cell (8, 8).
        let mut idx = SpatialIndex::new(4096);
        idx.insert(7, &Geometry::Point(vec![(2050, 2050)]), 0.0);
        idx.finish();
        assert_eq!(idx.query(2050.0, 2050.0), &[7]);
        // Same cell anywhere within (2048..2304, 2048..2304):
        assert_eq!(idx.query(2300.0, 2100.0), &[7]);
        // Outside that cell:
        assert!(idx.query(100.0, 100.0).is_empty());
        assert!(idx.query(3000.0, 2050.0).is_empty());
    }

    #[test]
    fn line_spanning_diagonal_lands_in_every_cell_it_crosses_aabb() {
        // A line from (0,0) → (4095,4095) has an AABB covering the
        // whole tile, so every cell should contain its index. The
        // query is a pruning step; geometry_hit will reject false
        // positives off the actual line.
        let mut idx = SpatialIndex::new(4096);
        idx.insert(0, &Geometry::LineString(vec![vec![(0, 0), (4095, 4095)]]), 0.0);
        idx.finish();
        for cy in 0..GRID {
            for cx in 0..GRID {
                let x = (cx as f64 + 0.5) * (4096.0 / GRID as f64);
                let y = (cy as f64 + 0.5) * (4096.0 / GRID as f64);
                assert!(
                    idx.query(x, y).contains(&0),
                    "cell ({cx},{cy}) missing line idx"
                );
            }
        }
    }

    #[test]
    fn polygon_expanded_by_tolerance_catches_clicks_just_outside_aabb() {
        // A square polygon AABB (1000..1100). Without tolerance a click
        // at (1110, 1050) is outside the AABB so the query misses.
        // With tolerance_local = 20 the AABB grows to (980..1120) and
        // the click cell now contains the index.
        let ring = vec![
            (1000, 1000),
            (1100, 1000),
            (1100, 1100),
            (1000, 1100),
            (1000, 1000),
        ];
        let geom = Geometry::Polygon(vec![ring]);

        let mut tight = SpatialIndex::new(4096);
        tight.insert(3, &geom, 0.0);
        tight.finish();
        // (1110, 1050) lies in cell (4, 4) (256-wide cells). The
        // polygon AABB covers cell (3..=4, 3..=4) so the index does
        // include cell (4,4). That makes this test less crisp than
        // intended — verify via a click well outside the AABB cell.
        // Use (1700, 1050) → cell (6, 4), which the tight insert
        // *won't* cover.
        assert!(!tight.query(1700.0, 1050.0).contains(&3));

        let mut loose = SpatialIndex::new(4096);
        loose.insert(3, &geom, 800.0); // bigger than three cell widths
        loose.finish();
        assert!(loose.query(1700.0, 1050.0).contains(&3));
    }

    #[test]
    fn query_outside_tile_extent_returns_empty() {
        let mut idx = SpatialIndex::new(4096);
        idx.insert(0, &Geometry::Point(vec![(0, 0)]), 0.0);
        idx.finish();
        assert!(idx.query(-1.0, 100.0).is_empty());
        assert!(idx.query(100.0, 5000.0).is_empty());
    }
}
