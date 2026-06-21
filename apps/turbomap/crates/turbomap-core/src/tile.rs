//! Tile addressing in the XYZ tile scheme.

use crate::geo::WorldPoint;

/// A tile in the XYZ pyramid. `z` is the zoom level; at zoom `z` there are
/// `2^z` tiles per axis. `(0, 0, 0)` covers the whole world.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, PartialOrd, Ord)]
pub struct TileId {
    pub z: u8,
    pub x: u32,
    pub y: u32,
}

/// A child's location inside an ancestor tile, used for sampling a sub-region
/// of the ancestor's texture during parent-fallback rendering.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct SubUv {
    /// Top-left of the child's region inside the ancestor (`[0, 1]`).
    pub origin: WorldPoint,
    /// Width and height of the child's region inside the ancestor (`[0, 1]`).
    pub size: f64,
}

impl TileId {
    pub const fn new(z: u8, x: u32, y: u32) -> Self {
        Self { z, x, y }
    }

    /// World-space bounds of this tile: `(north-west, south-east)`.
    pub fn world_bounds(self) -> (WorldPoint, WorldPoint) {
        let n = (1u64 << self.z) as f64;
        let nw = WorldPoint::new(self.x as f64 / n, self.y as f64 / n);
        let se = WorldPoint::new((self.x as f64 + 1.0) / n, (self.y as f64 + 1.0) / n);
        (nw, se)
    }

    /// The ancestor at `levels_up` zoom levels shallower, or `None` if that
    /// would go above zoom 0.
    pub fn ancestor(self, levels_up: u8) -> Option<TileId> {
        if levels_up > self.z {
            return None;
        }
        Some(TileId {
            z: self.z - levels_up,
            x: self.x >> levels_up,
            y: self.y >> levels_up,
        })
    }

    /// The four child tiles one zoom level deeper, or `None` at the `u8` zoom
    /// ceiling. Used by the best-available coverage resolver to retain finer
    /// resident detail when the ideal tile isn't loaded.
    pub fn children(self) -> Option<[TileId; 4]> {
        let z = self.z.checked_add(1)?;
        let (x, y) = (self.x * 2, self.y * 2);
        Some([
            TileId::new(z, x, y),
            TileId::new(z, x + 1, y),
            TileId::new(z, x, y + 1),
            TileId::new(z, x + 1, y + 1),
        ])
    }

    /// This tile's location *inside* `ancestor`. `None` if `ancestor` is not
    /// actually an ancestor of `self`.
    pub fn sub_uv_in(self, ancestor: TileId) -> Option<SubUv> {
        if ancestor.z > self.z {
            return None;
        }
        let levels = self.z - ancestor.z;
        let expected = self.ancestor(levels)?;
        if expected != ancestor {
            return None;
        }
        let factor = 1u32 << levels;
        let size = 1.0 / factor as f64;
        let origin = WorldPoint::new(
            (self.x % factor) as f64 * size,
            (self.y % factor) as f64 * size,
        );
        Some(SubUv { origin, size })
    }
}

#[cfg(test)]
mod tests {
    //! Value boundary: developers/host code address tiles via `TileId`. They
    //! need `world_bounds` to draw quads, `ancestor` for parent-fallback, and
    //! `sub_uv_in` to compute texture coordinates when sampling an ancestor.

    use super::*;

    const EPS: f64 = 1e-12;

    #[test]
    fn root_tile_covers_the_world() {
        let (nw, se) = TileId::new(0, 0, 0).world_bounds();
        assert_eq!(nw, WorldPoint::new(0.0, 0.0));
        assert_eq!(se, WorldPoint::new(1.0, 1.0));
    }

    #[test]
    fn world_bounds_partition_at_zoom_one() {
        // Four tiles, each a unit-square quarter of the world, no overlap.
        let tl = TileId::new(1, 0, 0).world_bounds();
        let tr = TileId::new(1, 1, 0).world_bounds();
        let bl = TileId::new(1, 0, 1).world_bounds();
        let br = TileId::new(1, 1, 1).world_bounds();
        assert_eq!(tl.0, WorldPoint::new(0.0, 0.0));
        assert_eq!(tl.1, WorldPoint::new(0.5, 0.5));
        assert_eq!(tr.0, WorldPoint::new(0.5, 0.0));
        assert_eq!(br.1, WorldPoint::new(1.0, 1.0));
        assert_eq!(bl.0, WorldPoint::new(0.0, 0.5));
    }

    #[test]
    fn ancestor_walk_returns_correct_pyramid_parent() {
        let child = TileId::new(4, 5, 6);
        assert_eq!(child.ancestor(1), Some(TileId::new(3, 2, 3)));
        assert_eq!(child.ancestor(2), Some(TileId::new(2, 1, 1)));
        assert_eq!(child.ancestor(4), Some(TileId::new(0, 0, 0)));
        assert_eq!(child.ancestor(5), None);
    }

    #[test]
    fn sub_uv_of_self_is_identity() {
        let t = TileId::new(3, 4, 5);
        let uv = t.sub_uv_in(t).unwrap();
        assert!(uv.origin.x.abs() < EPS && uv.origin.y.abs() < EPS);
        assert!((uv.size - 1.0).abs() < EPS);
    }

    #[test]
    fn sub_uv_in_grandparent_places_child_at_expected_quadrant() {
        // Child (2, 3, 1) lies inside (0, 0, 0). The 4×4 grid of zoom-2 tiles
        // means tile (3, 1) sits at uv origin (0.75, 0.25), each 0.25 wide.
        let child = TileId::new(2, 3, 1);
        let uv = child.sub_uv_in(TileId::new(0, 0, 0)).unwrap();
        assert!((uv.origin.x - 0.75).abs() < EPS);
        assert!((uv.origin.y - 0.25).abs() < EPS);
        assert!((uv.size - 0.25).abs() < EPS);
    }

    #[test]
    fn sub_uv_refuses_a_non_ancestor() {
        // (2, 0, 0) is *not* an ancestor of (2, 3, 1).
        let child = TileId::new(2, 3, 1);
        assert!(child.sub_uv_in(TileId::new(2, 0, 0)).is_none());
        // A deeper tile can't be an ancestor either.
        assert!(child.sub_uv_in(TileId::new(3, 0, 0)).is_none());
    }
}
