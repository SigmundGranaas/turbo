//! Core wgpu map renderer — no I/O, no winit, no HTTP.
//!
//! The crate boundary is intentionally narrow: a host provides a `wgpu::Device`,
//! constructs a [`Map`], adds raster and/or vector layers (each backed by a
//! [`TileSource`] / [`VectorTileSource`]), drives input via `Map` methods,
//! and is responsible for fetching tiles outside the renderer (pull-push
//! pattern via [`Map::pending_tiles`] + ingest methods).

pub mod camera;
pub mod capacity;
pub mod dem;
pub mod error;
pub mod fb_probe;
pub mod geo;
pub mod hit;
pub mod lighting;
pub mod lod;
pub mod map;
pub mod markers;
pub mod projection;
pub mod render;
pub mod scene;
pub mod source;
pub mod spatial_index;
pub mod sprite;
pub mod style;
pub mod sun;
pub mod tessellate;
pub mod text;
pub mod tile;
pub mod vector;

pub use camera::{Camera, CameraAnimation, FiniteF64, ZoomBounds, ZoomLock, TILE_SIZE_PX};
pub use dem::{decode_elevation, DemEncoding};
pub use error::{MapError, TileError};
pub use geo::{LatLng, WorldPoint, MAX_LATITUDE_DEG};
pub use lighting::{Lighting, LightingMode};
pub use map::{
    CloudParams, FrameMetrics, HitFeature, HitMarker, HitResult, LayerKind, LayerMetrics, Map,
    MapOptions, Marker, MarkerId, PendingTile, PhaseTimings,
    PublicTerrainOptions as TerrainOptions, RadarFrame,
};
pub use projection::{reproject, Crs};
pub use render::graph::{FrameGraphReport, FramePhase, PassTiming};
pub use scene::Scene;
pub use source::{RasterFormat, RasterTile, TileSource};
pub use style::{Color, Filter, HillshadeStyle, IconSpec, Paint, Rule, VectorStyle};
pub use sun::{atmosphere, solar_position, Atmosphere, SunPosition};
pub use tessellate::{
    tessellate, IconRequest, InteractiveFeature, LabelRequest, Mesh, VectorVertex,
};
pub use tile::{SubUv, TileId};
// Streaming-plan attempt identity — re-exported so hosts don't need a direct
// turbomap-world dependency to speak the plan boundary.
pub use turbomap_world::RequestId;
pub use vector::{
    tile_local_to_world, Feature, GeomType, Geometry, Layer as VectorTileLayer,
    Value as VectorValue, VectorTile, VectorTileSource,
};
