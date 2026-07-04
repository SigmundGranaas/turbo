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
            // PMTiles archives are the one remote-source kind the engine
            // resolves to REAL in-process sources rather than host-fed stubs:
            // the reader owns the archive smarts (directories, Hilbert,
            // compression), and a host can't express "range-read this
            // archive" as a URL-template fetch. Bundled (file) opens are a
            // header+root-dir read — cheap at apply time. Remote (http) opens
            // do blocking range GETs on the apply thread — fine for desktop
            // v1; FFI hosts should prefer bundled archives until resolution
            // is made lazy. On wasm there is no std::fs / blocking reqwest:
            // resolve to Unsupported (the layer is skipped and reported).
            #[cfg(not(target_arch = "wasm32"))]
            SourceDef::PmtilesRaster { location } => match open_pmtiles(location) {
                Ok(src) => ResolvedSource::Raster(Arc::new(src)),
                Err(e) => {
                    log::warn!("pmtiles raster source {location:?} failed to open: {e}");
                    ResolvedSource::Unsupported
                }
            },
            #[cfg(not(target_arch = "wasm32"))]
            SourceDef::PmtilesVector { location } => match open_pmtiles(location) {
                Ok(src) => ResolvedSource::Vector(Arc::new(src)),
                Err(e) => {
                    log::warn!("pmtiles vector source {location:?} failed to open: {e}");
                    ResolvedSource::Unsupported
                }
            },
            #[cfg(not(target_arch = "wasm32"))]
            SourceDef::PmtilesDem { location, halo, .. } => match open_pmtiles(location) {
                Ok(src) => ResolvedSource::Dem(Arc::new(WithDemHalo {
                    inner: src,
                    halo: *halo,
                })),
                Err(e) => {
                    log::warn!("pmtiles dem source {location:?} failed to open: {e}");
                    ResolvedSource::Unsupported
                }
            },
            #[cfg(target_arch = "wasm32")]
            SourceDef::PmtilesRaster { .. }
            | SourceDef::PmtilesVector { .. }
            | SourceDef::PmtilesDem { .. } => ResolvedSource::Unsupported,
        }
    }
}

/// Open a PMTiles archive by location: http(s) URLs range-read remote static
/// storage; anything else is a local filesystem path (the bundled baseline).
#[cfg(not(target_arch = "wasm32"))]
fn open_pmtiles(
    location: &str,
) -> Result<turbomap_tiles_pmtiles::PMTilesSource, turbomap_tiles_pmtiles::PMTilesError> {
    if location.starts_with("http://") || location.starts_with("https://") {
        turbomap_tiles_pmtiles::PMTilesSource::open_http(location)
    } else {
        turbomap_tiles_pmtiles::PMTilesSource::open(location)
    }
}

#[cfg(all(test, not(target_arch = "wasm32")))]
mod tests {
    //! Value boundary: a Scene declaring a `pmtiles-*` source resolves to a
    //! REAL in-process source that serves tiles from the archive — the
    //! bundled-baseline path (B5.2). Uses the writer to build a genuine
    //! archive in a temp file; no fixtures, no network.
    use super::*;
    use turbomap_tiles_pmtiles::{writer::write_archive, TileType};

    fn write_temp_archive(tile_type: TileType, payload: &[u8]) -> tempfile::NamedTempFile {
        let tile = TileId::new(3, 1, 2);
        let bytes = write_archive(tile_type, &[(tile, payload.to_vec())]).unwrap();
        let f = tempfile::NamedTempFile::new().unwrap();
        std::fs::write(f.path(), bytes).unwrap();
        f
    }

    #[test]
    fn pmtiles_raster_resolves_to_a_real_source_that_serves_tiles() {
        let f = write_temp_archive(TileType::Png, b"png-ish");
        let def = SourceDef::PmtilesRaster {
            location: f.path().to_string_lossy().into_owned(),
        };
        match HostDrivenResolver.resolve("sat", &def) {
            ResolvedSource::Raster(src) => {
                let t = src.request(TileId::new(3, 1, 2)).expect("tile served");
                assert_eq!(t.bytes, b"png-ish");
            }
            _ => panic!("pmtiles-raster must resolve to a Raster source"),
        }
    }

    #[test]
    fn pmtiles_dem_reports_the_scene_declared_halo() {
        let f = write_temp_archive(TileType::Png, b"dem-ish");
        let def = SourceDef::PmtilesDem {
            location: f.path().to_string_lossy().into_owned(),
            encoding: turbomap_scene::DemEncoding::MapboxRgb,
            halo: 1,
        };
        match HostDrivenResolver.resolve("dem", &def) {
            ResolvedSource::Dem(src) => {
                assert_eq!(src.dem_halo_px(), 1, "halo comes from the Scene, not the archive");
                assert_eq!(src.request(TileId::new(3, 1, 2)).unwrap().bytes, b"dem-ish");
            }
            _ => panic!("pmtiles-dem must resolve to a Dem source"),
        }
    }

    #[test]
    fn a_missing_archive_degrades_to_unsupported_not_a_panic() {
        let def = SourceDef::PmtilesVector {
            location: "/definitely/not/here.pmtiles".to_string(),
        };
        assert!(matches!(
            HostDrivenResolver.resolve("ghost", &def),
            ResolvedSource::Unsupported
        ));
    }
}

/// Overrides the DEM halo a wrapped source reports — the archive header has
/// no halo field, so the Scene's declared value must reach the terrain mesh.
#[cfg(not(target_arch = "wasm32"))]
struct WithDemHalo {
    inner: turbomap_tiles_pmtiles::PMTilesSource,
    halo: u32,
}

#[cfg(not(target_arch = "wasm32"))]
impl TileSource for WithDemHalo {
    // PMTilesSource implements BOTH TileSource and VectorTileSource, so the
    // delegations disambiguate via fully-qualified syntax.
    fn request(&self, tile: TileId) -> Result<RasterTile, TileError> {
        TileSource::request(&self.inner, tile)
    }
    fn min_zoom(&self) -> u8 {
        TileSource::min_zoom(&self.inner)
    }
    fn max_zoom(&self) -> u8 {
        TileSource::max_zoom(&self.inner)
    }
    fn dem_halo_px(&self) -> u32 {
        self.halo
    }
}
