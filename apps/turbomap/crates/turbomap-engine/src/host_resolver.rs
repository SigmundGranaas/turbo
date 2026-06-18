//! The resolver for host-driven tile IO — the FFI / production-embedding
//! configuration.
//!
//! Remote sources (raster XYZ, DEM XYZ, vector MVT) resolve to *stubs*
//! that never fetch: they only carry zoom bounds so the layer installs
//! and `pending_tiles` enumerates its needs. The host owns the actual IO
//! (auth, caching, offline) — it fetches each pending tile itself (it
//! knows the URL templates from its own scene) and pushes bytes back via
//! the engine's `ingest_*` methods. Inline GeoJSON resolves to the real
//! in-process source, since it needs no IO at all.

use std::sync::Arc;

use turbomap_core::{RasterTile, TileError, TileId, TileSource, VectorTile, VectorTileSource};
use turbomap_scene::SourceDef;

use crate::geojson::GeoJsonVectorSource;
use crate::resolver::{ResolvedSource, SourceResolver};

/// A raster/DEM source the host feeds. `request` always fails — by design.
struct RemoteRasterStub {
    min_zoom: u8,
    max_zoom: u8,
    /// DEM halo (px) the host bakes into each tile; 0 for plain rasters. The
    /// terrain mesh reads this so it crops to the interior 256² and uses the
    /// halo ring to stitch crack-free edges across tile boundaries.
    dem_halo: u32,
}

impl TileSource for RemoteRasterStub {
    fn request(&self, _tile: TileId) -> Result<RasterTile, TileError> {
        Err(TileError::Network(
            "host-driven source: fetch this tile host-side and call ingest".into(),
        ))
    }
    fn min_zoom(&self) -> u8 {
        self.min_zoom
    }
    fn max_zoom(&self) -> u8 {
        self.max_zoom
    }
    fn dem_halo_px(&self) -> u32 {
        self.dem_halo
    }
}

/// A vector source the host feeds with raw MVT bytes.
struct RemoteVectorStub {
    min_zoom: u8,
    max_zoom: u8,
}

impl VectorTileSource for RemoteVectorStub {
    fn request(&self, _tile: TileId) -> Result<VectorTile, TileError> {
        Err(TileError::Network(
            "host-driven source: fetch this tile host-side and call ingest_mvt".into(),
        ))
    }
    fn min_zoom(&self) -> u8 {
        self.min_zoom
    }
    fn max_zoom(&self) -> u8 {
        self.max_zoom
    }
}

/// Resolver for hosts that drive tile IO themselves (the FFI default).
pub struct HostDrivenResolver;

impl SourceResolver for HostDrivenResolver {
    fn resolve(&self, _id: &str, def: &SourceDef) -> ResolvedSource {
        match def {
            SourceDef::RasterXyz {
                min_zoom, max_zoom, ..
            } => ResolvedSource::Raster(Arc::new(RemoteRasterStub {
                min_zoom: *min_zoom,
                max_zoom: *max_zoom,
                dem_halo: 0,
            })),
            SourceDef::DemXyz {
                min_zoom,
                max_zoom,
                halo,
                ..
            } => ResolvedSource::Dem(Arc::new(RemoteRasterStub {
                min_zoom: *min_zoom,
                max_zoom: *max_zoom,
                dem_halo: *halo,
            })),
            SourceDef::VectorXyz {
                min_zoom, max_zoom, ..
            } => ResolvedSource::Vector(Arc::new(RemoteVectorStub {
                min_zoom: *min_zoom,
                max_zoom: *max_zoom,
            })),
            SourceDef::GeoJson { data } => {
                ResolvedSource::Vector(Arc::new(GeoJsonVectorSource::new(data)))
            }
        }
    }
}
