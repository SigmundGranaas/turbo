//! The implicit Web-Mercator quadtree — **instance #1** of the chunk tree.
//! Nodes are computed, never fetched: `(z, x, y)` packs into a [`NodeId`] and
//! back, bounds derive from Mercator math, and geometric error halves per
//! level from the equatorial texel size.
//!
//! Self-contained on purpose (mirrors `turbomap-core`'s `TileId` semantics
//! without depending on the GPU crate); core adopts these keys at the
//! plan-boundary slice (B3), where the conversion is field-for-field.

use serde::{Deserialize, Serialize};

use crate::chunk::{BoundingVolume, ChunkMeta, NodeId, Refine};

/// Earth's equatorial circumference in meters (WGS84) — the anchor for the
/// pyramid's ground-resolution / geometric-error table.
pub const EQUATOR_M: f64 = 40_075_016.685_578_49;

/// A node of the XYZ pyramid: at zoom `z` there are `2^z` tiles per axis and
/// `(0, 0, 0)` covers the world.
#[derive(
    Debug, Clone, Copy, PartialEq, Eq, Hash, PartialOrd, Ord, Serialize, Deserialize,
)]
pub struct QuadKey {
    pub z: u8,
    pub x: u32,
    pub y: u32,
}

impl QuadKey {
    pub const fn new(z: u8, x: u32, y: u32) -> Self {
        Self { z, x, y }
    }

    /// Pack into an opaque [`NodeId`]: `z` in the top byte, then 28 bits of
    /// `x` and 28 bits of `y`. 28 bits per axis covers zoom ≤ 28 — beyond any
    /// tile source we address (max_zoom 22 today) with headroom.
    pub fn node_id(self) -> NodeId {
        debug_assert!(self.z <= 28, "quadtree NodeId packs 28 bits per axis");
        NodeId(((self.z as u64) << 56) | ((self.x as u64 & 0x0FFF_FFFF) << 28) | (self.y as u64 & 0x0FFF_FFFF))
    }

    /// Inverse of [`Self::node_id`].
    pub fn from_node_id(node: NodeId) -> Self {
        Self {
            z: (node.0 >> 56) as u8,
            x: ((node.0 >> 28) & 0x0FFF_FFFF) as u32,
            y: (node.0 & 0x0FFF_FFFF) as u32,
        }
    }

    /// The ancestor `levels_up` shallower, or `None` above the root.
    pub fn ancestor(self, levels_up: u8) -> Option<QuadKey> {
        if levels_up > self.z {
            return None;
        }
        Some(QuadKey {
            z: self.z - levels_up,
            x: self.x >> levels_up,
            y: self.y >> levels_up,
        })
    }

    /// The four children one level deeper, or `None` at the zoom ceiling.
    pub fn children(self) -> Option<[QuadKey; 4]> {
        let z = self.z.checked_add(1)?;
        let (x, y) = (self.x * 2, self.y * 2);
        Some([
            QuadKey::new(z, x, y),
            QuadKey::new(z, x + 1, y),
            QuadKey::new(z, x, y + 1),
            QuadKey::new(z, x + 1, y + 1),
        ])
    }

    /// Geographic bounds as a 3D Tiles-shaped region (radians + meters).
    /// Latitude comes from the inverse Web-Mercator (Gudermannian); heights
    /// are zero — a 2D pyramid chunk is flat until a layer says otherwise.
    pub fn region(self) -> BoundingVolume {
        let n = (1u64 << self.z) as f64;
        let lng = |x: f64| (x / n) * 2.0 * std::f64::consts::PI - std::f64::consts::PI;
        let lat = |y: f64| {
            let t = std::f64::consts::PI * (1.0 - 2.0 * (y / n));
            t.sinh().atan()
        };
        BoundingVolume::Region {
            west: lng(self.x as f64),
            east: lng(self.x as f64 + 1.0),
            // Tile y grows southward; the region's south edge is y+1.
            south: lat(self.y as f64 + 1.0),
            north: lat(self.y as f64),
            min_height_m: 0.0,
            max_height_m: 0.0,
        }
    }

    /// Geometric error, in meters, of rendering this node instead of its
    /// children: one equatorial texel of this level,
    /// `EQUATOR_M / (tile_px · 2^z)` — the standard ground-resolution table
    /// (z0/256px ≈ 156 543 m), halving every level. The runtime SSE
    /// projection (S6) turns this into pixels; here only the *ratios* between
    /// levels and layers must be right.
    pub fn geometric_error_m(self, tile_px: u32) -> f64 {
        EQUATOR_M / (tile_px as f64 * (1u64 << self.z) as f64)
    }

    /// The full [`ChunkMeta`] for this node. Pyramids refine by replacement.
    pub fn meta(self, tile_px: u32) -> ChunkMeta {
        ChunkMeta {
            bounds: self.region(),
            geometric_error_m: self.geometric_error_m(tile_px),
            refine: Refine::Replace,
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn node_id_round_trips_across_the_pyramid() {
        for &(z, x, y) in &[
            (0u8, 0u32, 0u32),
            (1, 1, 0),
            (11, 1058, 588),
            (22, (1 << 22) - 1, (1 << 22) - 1),
            (28, (1 << 28) - 1, 0),
        ] {
            let k = QuadKey::new(z, x, y);
            assert_eq!(QuadKey::from_node_id(k.node_id()), k, "({z},{x},{y})");
        }
    }

    #[test]
    fn node_ids_are_unique_across_adjacent_levels() {
        // The parent and its four children must never collide.
        let parent = QuadKey::new(5, 9, 20);
        let mut seen = std::collections::HashSet::new();
        seen.insert(parent.node_id());
        for c in parent.children().unwrap() {
            assert!(seen.insert(c.node_id()), "collision at {c:?}");
        }
    }

    #[test]
    fn ancestor_and_children_mirror_the_pyramid() {
        let k = QuadKey::new(4, 5, 6);
        assert_eq!(k.ancestor(1), Some(QuadKey::new(3, 2, 3)));
        assert_eq!(k.ancestor(4), Some(QuadKey::new(0, 0, 0)));
        assert_eq!(k.ancestor(5), None);
        let kids = QuadKey::new(3, 2, 3).children().unwrap();
        assert!(kids.contains(&k), "a child of my parent includes me");
        assert!(kids.iter().all(|c| c.ancestor(1) == Some(QuadKey::new(3, 2, 3))));
    }

    #[test]
    fn root_region_spans_the_mercator_world() {
        let BoundingVolume::Region {
            west,
            south,
            east,
            north,
            ..
        } = QuadKey::new(0, 0, 0).region()
        else {
            panic!("quadtree bounds are regions");
        };
        assert!((west + std::f64::consts::PI).abs() < 1e-12);
        assert!((east - std::f64::consts::PI).abs() < 1e-12);
        // Web-Mercator clip latitude ≈ ±85.051129° in radians.
        let clip = 85.051_128_78_f64.to_radians();
        assert!((north - clip).abs() < 1e-6, "north {north}");
        assert!((south + clip).abs() < 1e-6, "south {south}");
    }

    #[test]
    fn geometric_error_matches_the_ground_resolution_table_and_halves() {
        let e0 = QuadKey::new(0, 0, 0).geometric_error_m(256);
        assert!((e0 - 156_543.03).abs() < 0.01, "z0 texel {e0}");
        for z in 0..22u8 {
            let e = QuadKey::new(z, 0, 0).geometric_error_m(256);
            let child = QuadKey::new(z + 1, 0, 0).geometric_error_m(256);
            assert!((e / child - 2.0).abs() < 1e-12, "halving broke at z{z}");
        }
    }
}
