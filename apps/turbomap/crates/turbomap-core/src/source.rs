//! The `TileSource` contract — a host plugs in HTTP, disk, PMTiles, etc.
//!
//! The renderer never calls `TileSource` itself; the host drains pending
//! requests via `Map::pending_tiles`, fetches via this trait, and pushes the
//! decoded RGBA back through `Map::ingest_tile`. That keeps the renderer
//! free of any async runtime or I/O dependency.

use crate::{error::TileError, tile::TileId};

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum RasterFormat {
    Png,
    Jpeg,
    Webp,
}

#[derive(Debug, Clone)]
pub struct RasterTile {
    pub bytes: Vec<u8>,
    pub format: RasterFormat,
}

pub trait TileSource: Send + Sync {
    /// Synchronous fetch — the host is expected to call this from a worker
    /// thread, not the render thread.
    fn request(&self, tile: TileId) -> Result<RasterTile, TileError>;

    fn min_zoom(&self) -> u8 {
        0
    }

    fn max_zoom(&self) -> u8 {
        22
    }

    /// The encoding of bytes this source produces. Cache adapters need this
    /// to re-emit a `RasterTile` on a hit without keeping the inner source
    /// alive. Defaults to PNG since that's the common case.
    fn raster_format(&self) -> RasterFormat {
        RasterFormat::Png
    }

    /// For DEM sources only: how the tile's RGB channels encode elevation.
    /// The ingest-side codec ([`crate::dem::decode_dem_rgba`]) uses this to
    /// turn fetched bytes into metres before they reach the renderer — the
    /// render path itself never sees an encoding (plan slice D3). Defaults
    /// to Mapbox Terrain-RGB, by far the common case.
    fn dem_encoding(&self) -> crate::dem::DemEncoding {
        crate::dem::DemEncoding::MapboxRgb
    }

    /// For DEM sources only: how many pixels of halo (overscan) the
    /// returned tile carries on every side beyond the canonical 256×256
    /// tile envelope. `0` (the default) is back-compat — the consumer
    /// computes gradients with `ClampToEdge` at the edge and you get
    /// visible seams. `> 0` means the tile's outer ring is sampled from
    /// the neighbouring tile's geographic area, so the consumer can
    /// crop to the interior 256² for display while using the halo
    /// pixels in its gradient kernel. The hillshade pipeline reads
    /// this on `add_hillshade_layer`.
    fn dem_halo_px(&self) -> u32 {
        0
    }
}
