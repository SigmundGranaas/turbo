//! Vector-tile types and the source trait that hosts implement.
//!
//! This crate re-exports the decoded data types from `turbomap-mvt` so that
//! consumers only need to depend on `turbomap-core`. The raw protobuf and
//! geometry-command interpretation lives in the mvt crate; here we wrap it
//! in the trait the renderer talks to.

use crate::{error::TileError, tile::TileId};

pub use turbomap_mvt::{
    decode as decode_mvt, DecodeError, Feature, GeomType, Geometry, Layer, Value, VectorTile,
};

/// A source of vector tiles. Mirrors the raster `TileSource` contract:
/// synchronous, called from a host worker thread, never on the render
/// thread.
pub trait VectorTileSource: Send + Sync {
    fn request(&self, tile: TileId) -> Result<VectorTile, TileError>;

    fn min_zoom(&self) -> u8 {
        0
    }

    fn max_zoom(&self) -> u8 {
        14
    }
}

/// Project a tile-local coordinate (`0..extent`) into the renderer's
/// normalised world space (`[0, 1]`).
///
/// Tile `(z, x, y)` covers the world rectangle from
/// `(x/2^z, y/2^z)` to `((x+1)/2^z, (y+1)/2^z)`. A point at tile-local
/// `(lx, ly)` lies at the fraction `(lx/extent, ly/extent)` within that
/// rectangle.
pub fn tile_local_to_world(tile: TileId, extent: u32, local: (i32, i32)) -> (f64, f64) {
    let n = (1u64 << tile.z) as f64;
    // `extent` is the tile's coordinate grid size; a malformed tile claiming
    // 0 would divide to NaN/Inf and poison the vertex buffer (which the GPU
    // finite gate doesn't cover). Treat 0 as 1 — decode also normalizes it to
    // the 4096 default, so this is belt-and-braces.
    let ext = extent.max(1) as f64;
    let fx = local.0 as f64 / ext;
    let fy = local.1 as f64 / ext;
    ((tile.x as f64 + fx) / n, (tile.y as f64 + fy) / n)
}

#[cfg(test)]
mod tests {
    //! Value boundary: developers projecting vector geometry into the
    //! renderer's world coords need this to be (a) correct at tile corners,
    //! (b) consistent across zooms — a point in the same lat/lng projects to
    //! the same world coord whether we got it from z=10 or z=12 tiles.

    use super::*;

    #[test]
    fn zero_extent_does_not_divide_by_zero() {
        // A malformed tile claiming extent 0 must not poison the vertex buffer
        // with NaN/Inf. The projection stays finite (extent treated as 1).
        let (x, y) = tile_local_to_world(TileId::new(6, 3, 5), 0, (10, 20));
        assert!(x.is_finite() && y.is_finite());
    }

    #[test]
    fn tile_corners_project_to_tile_world_bounds() {
        // Tile (3, 5, 6) with extent 4096:
        //   top-left  (0, 0)   → world ((5)/8, (6)/8)
        //   bottom-right (4096, 4096) → world ((5+1)/8, (6+1)/8)
        let t = TileId::new(3, 5, 6);
        let nw = tile_local_to_world(t, 4096, (0, 0));
        let se = tile_local_to_world(t, 4096, (4096, 4096));
        assert!((nw.0 - 5.0 / 8.0).abs() < 1e-12);
        assert!((nw.1 - 6.0 / 8.0).abs() < 1e-12);
        assert!((se.0 - 6.0 / 8.0).abs() < 1e-12);
        assert!((se.1 - 7.0 / 8.0).abs() < 1e-12);
    }

    #[test]
    fn projection_is_invariant_across_zoom_levels() {
        // The same world point reached from a z=10 tile and a z=12 tile
        // (which sits *inside* the z=10 one) must round to the same world
        // coords. Use the centre of z=12 tile (2048, 3000, 1500): its world
        // is computed both ways.
        let deep = TileId::new(12, 2048, 1500);
        let world_from_deep = tile_local_to_world(deep, 4096, (0, 0));
        // The same point in the z=10 ancestor: (2048>>2, 1500>>2) = (512, 375).
        // Local offset within ancestor: ((2048 - 512*4)/4 * 4096,
        //                                (1500 - 375*4)/4 * 4096) = (0, 0).
        let ancestor = TileId::new(10, 512, 375);
        let world_from_ancestor = tile_local_to_world(ancestor, 4096, (0, 0));
        assert!((world_from_deep.0 - world_from_ancestor.0).abs() < 1e-12);
        assert!((world_from_deep.1 - world_from_ancestor.1).abs() < 1e-12);
    }
}
