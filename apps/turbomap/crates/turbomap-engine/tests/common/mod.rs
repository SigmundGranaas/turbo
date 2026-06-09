//! Shared test helpers: a deterministic resolver that maps the IR's
//! declarative sources onto the golden harness's synthetic tile sources,
//! so engine tests are offline and reproducible.

use std::sync::Arc;

use turbomap_engine::{ResolvedSource, SourceResolver};
use turbomap_golden::sources::{GaussianTerrainSource, ParchmentBasemap};
use turbomap_scene::SourceDef;

/// Resolves raster sources to a flat parchment basemap and DEM sources to
/// the Gaussian-Bergen terrain — the same synthetic data the golden trace
/// path uses, so engine renders are comparable to the imperative golden.
pub struct SyntheticResolver;

impl SourceResolver for SyntheticResolver {
    fn resolve(&self, _id: &str, def: &SourceDef) -> ResolvedSource {
        match def {
            SourceDef::RasterXyz { .. } => ResolvedSource::Raster(Arc::new(ParchmentBasemap)),
            SourceDef::DemXyz { .. } => {
                ResolvedSource::Dem(Arc::new(GaussianTerrainSource::bergen()))
            }
            SourceDef::VectorXyz { .. } | SourceDef::GeoJson { .. } => ResolvedSource::Unsupported,
        }
    }
}
