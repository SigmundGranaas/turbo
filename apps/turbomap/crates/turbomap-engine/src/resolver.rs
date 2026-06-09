//! Translating the IR's declarative [`SourceDef`] into a concrete
//! `turbomap_core::TileSource`.
//!
//! The IR describes sources by URL templates / inline data; the renderer
//! needs an `Arc<dyn TileSource>`. That translation is the one piece of
//! I/O policy the engine must not hard-code — a production host resolves
//! to HTTP-backed sources (with auth, caching, offline), while tests and
//! the inspect tool resolve to deterministic synthetic sources. So it is
//! injected as a `SourceResolver`.

use std::sync::Arc;

use turbomap_core::TileSource;
use turbomap_scene::SourceDef;

/// The concrete tile source a [`SourceResolver`] produces for a layer,
/// or a signal that this backend can't serve it.
pub enum ResolvedSource {
    /// A raster basemap/overlay source.
    Raster(Arc<dyn TileSource>),
    /// A DEM source feeding terrain/hillshade.
    Dem(Arc<dyn TileSource>),
    /// The resolver does not provide this source (the layer is skipped
    /// and recorded as unsupported).
    Unsupported,
}

/// Turns a declarative [`SourceDef`] into a tile source. Injected at
/// engine construction so I/O policy lives with the host, not the engine.
pub trait SourceResolver {
    fn resolve(&self, id: &str, def: &SourceDef) -> ResolvedSource;
}
