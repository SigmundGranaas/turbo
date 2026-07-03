//! The chunk vocabulary — the only things streaming, caching, and priority
//! math are allowed to know about a piece of world data.

use serde::{Deserialize, Serialize};

/// A world layer — one streamable dataset (basemap vectors, imagery, DEM,
/// radar field, …). Stable, host-visible identity; the human-readable name
/// and provider chain live in the layer catalog, not on the hot-path key.
#[derive(
    Debug, Clone, Copy, PartialEq, Eq, Hash, PartialOrd, Ord, Serialize, Deserialize,
)]
pub struct WorldLayerId(pub u16);

/// A node's identity inside its layer's tree — **opaque** to everything but
/// the tree shape that minted it. For an implicit quadtree it packs `(z, x, y)`
/// ([`crate::quadtree::QuadKey::node_id`]); for an explicit tree it is an
/// index into the fetched tree arena. Never `(z, x, y)` in public signatures:
/// that assumption is exactly what this crate exists to remove.
#[derive(
    Debug, Clone, Copy, PartialEq, Eq, Hash, PartialOrd, Ord, Serialize, Deserialize,
)]
pub struct NodeId(pub u64);

/// The engine-wide address of one chunk of one layer — the key the lifecycle
/// table, caches, and the streaming plan speak.
#[derive(
    Debug, Clone, Copy, PartialEq, Eq, Hash, PartialOrd, Ord, Serialize, Deserialize,
)]
pub struct ChunkKey {
    pub layer: WorldLayerId,
    pub node: NodeId,
}

/// How children relate to their parent when they refine it (3D Tiles
/// semantics, adopted verbatim per decision D3).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum Refine {
    /// Children REPLACE the parent: once all visible children are resident
    /// the parent stops drawing. Every tile pyramid works this way.
    Replace,
    /// Children ADD to the parent: the parent keeps drawing and children
    /// contribute detail on top (sparse point clouds, additive scenery).
    Add,
}

/// A chunk's spatial extent. `Region` is the geographic form every 2D pyramid
/// uses; `Box3` and `Sphere` carry explicit-tree content (3D Tiles oriented
/// boxes / spheres) losslessly.
#[derive(Debug, Clone, Copy, PartialEq, Serialize, Deserialize)]
pub enum BoundingVolume {
    /// Geographic region: `[west, south, east, north]` in **radians** plus
    /// `[min_height, max_height]` in meters — exactly the 3D Tiles `region`
    /// form, so pyramid tiles and tileset regions share one representation.
    Region {
        west: f64,
        south: f64,
        east: f64,
        north: f64,
        min_height_m: f64,
        max_height_m: f64,
    },
    /// Oriented box: center + three half-axis vectors (12 numbers, 3D Tiles
    /// `box` order), in the tree's local frame.
    Box3([f64; 12]),
    /// Sphere: center + radius (3D Tiles `sphere` order).
    Sphere([f64; 4]),
}

/// Everything the streaming system may know about a chunk. Priority needs
/// nothing but this — which is the whole point: a raster tile, a TIN terrain
/// chunk, and a building tileset node are indistinguishable here.
#[derive(Debug, Clone, Copy, PartialEq, Serialize, Deserialize)]
pub struct ChunkMeta {
    pub bounds: BoundingVolume,
    /// Error, in meters, introduced if THIS chunk renders and its children do
    /// not. Screen-space error at runtime = f(geometric_error_m, bounds,
    /// camera); the LOD policy (S6) owns that projection.
    pub geometric_error_m: f64,
    pub refine: Refine,
}
