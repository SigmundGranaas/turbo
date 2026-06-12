//! Shared test helpers: a deterministic resolver that maps the IR's
//! declarative sources onto the golden harness's synthetic tile sources,
//! so engine tests are offline and reproducible.

use std::sync::Arc;

use turbomap_engine::{GeoJsonVectorSource, ResolvedSource, SourceResolver};
use turbomap_golden::sources::{GaussianTerrainSource, ParchmentBasemap};
use turbomap_scene::SourceDef;

/// Resolves raster→parchment, DEM→Gaussian-Bergen terrain, and GeoJSON to a
/// real `GeoJsonVectorSource` over the inline data — the same data path
/// production uses, just with synthetic raster/DEM.
pub struct SyntheticResolver;

impl SourceResolver for SyntheticResolver {
    fn resolve(&self, _id: &str, def: &SourceDef) -> ResolvedSource {
        match def {
            SourceDef::RasterXyz { .. } => ResolvedSource::Raster(Arc::new(ParchmentBasemap)),
            SourceDef::DemXyz { .. } => {
                ResolvedSource::Dem(Arc::new(GaussianTerrainSource::bergen()))
            }
            SourceDef::GeoJson { data } => {
                ResolvedSource::Vector(Arc::new(GeoJsonVectorSource::new(data)))
            }
            SourceDef::VectorXyz { .. } => ResolvedSource::Unsupported,
        }
    }
}
