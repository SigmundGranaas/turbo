//! Inspect-surface geometry types for the off-trail cost field.
//!
//! Historical note: this module once held the Theta\* local-mesh
//! builder (grid `Mesh` + exit anchors + LOS blockers). That solver
//! was superseded by the FMM grade-limited path and deleted (plan P4);
//! what remains are the pure types the `/v1/debug/pathfind/inspect`
//! surface still speaks: the bbox, per-cell cost samples, and refused
//! polygons that the SPA renders as overlays.

/// 2D point in the mesh's projected CRS (EPSG:25833 metres).
#[derive(Debug, Clone, Copy, PartialEq, serde::Serialize)]
pub struct Point2 {
    pub x: f64,
    pub y: f64,
}

impl Point2 {
    pub fn dist(self, other: Self) -> f64 {
        let dx = self.x - other.x;
        let dy = self.y - other.y;
        (dx * dx + dy * dy).sqrt()
    }
}

/// Bbox in the SAME projected CRS as the mesh points (typically
/// EPSG:25833 metres). The pure builder doesn't know about lon/lat
/// — that's the caller's transform.
#[derive(Debug, Clone, Copy)]
pub struct MeshBbox {
    pub min_x: f64,
    pub min_y: f64,
    pub max_x: f64,
    pub max_y: f64,
}

impl MeshBbox {
    pub fn is_valid(&self) -> bool {
        self.max_x > self.min_x && self.max_y > self.min_y
    }

    /// Number of cell centres along x and y at the given cell size.
    /// Capped to keep mesh size sane — a 10 km × 10 km bbox at 50 m
    /// cells already yields 40 000 nodes.
    pub fn grid_dims(&self, cell_m: f64) -> (u32, u32) {
        if cell_m <= 0.0 || !self.is_valid() {
            return (0, 0);
        }
        let nx = (((self.max_x - self.min_x) / cell_m).ceil() as i64).clamp(2, 400);
        let ny = (((self.max_y - self.min_y) / cell_m).ceil() as i64).clamp(2, 400);
        (nx as u32, ny as u32)
    }
}

#[derive(Debug, Clone, Copy)]
pub struct CostSample {
    pub at: Point2,
    pub cost_mul: f64,
}

/// Refused-region polygon. The outer ring is required; we don't
/// model holes — water bodies with islands are rare at the scale
/// of off-trail queries and the false-positive (extra refusal) is
/// safer than a false-negative.
#[derive(Debug, Clone)]
pub struct RefusedPolygon {
    pub ring: Vec<Point2>,
}
