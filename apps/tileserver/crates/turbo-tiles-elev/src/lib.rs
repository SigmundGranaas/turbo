//! Elevation primitive.
//!
//! Tiled, zstd-compressed DEM on disk (`norway.dem`) with a bounded
//! in-process LRU cache of decompressed tiles. Sub-microsecond samples
//! once tiles are warm; targeted for the 1–4 GB RAM deploy box via
//! the 128 MB default cache cap.

pub mod cache;
pub mod dem;
pub mod format;

pub use cache::{CacheStats, TileCache, TileId};
pub use dem::{
    wgs84_to_utm33n, Dem, DemCoverage, DemError, PointXY, SlopeAspect, DEFAULT_CACHE_BYTES,
};
pub use format::{
    write_meta, write_tile_entry, DemMeta, TileEntry, COMPRESSION_ZSTD, DEFAULT_TILE_CELLS,
    DEM_FORMAT_VERSION, DEM_META_BYTES, NODATA_SENTINEL, TILE_ENTRY_BYTES,
};
