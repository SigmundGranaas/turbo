//! Tree shapes — how a layer's chunks are addressed and enumerated.

use serde::{Deserialize, Serialize};

use crate::chunk::{ChunkMeta, NodeId};
use crate::quadtree::QuadKey;

/// The addressable pyramid of an implicit quadtree layer.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub struct PyramidSpec {
    pub min_zoom: u8,
    pub max_zoom: u8,
    /// Nominal tile edge in pixels/texels — the denominator of the layer's
    /// geometric-error table (256 for classic rasters, 512 for retina MVT).
    pub tile_px: u32,
}

/// How a layer's chunk tree is shaped.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum TreeShape {
    /// Instance #1: the Web-Mercator XYZ pyramid. Node identity, bounds,
    /// error, and children are all COMPUTED — nothing about the tree is ever
    /// fetched. Every source turbomap consumes today lives here.
    ImplicitQuadtree(PyramidSpec),
    /// Instance #2 (milestone M-3DTILES): a fetched tree (3D Tiles
    /// `tileset.json` and kin) whose pages stream in like any other payload
    /// (`TreePage` representation). The variant exists now so nothing below
    /// it can accidentally assume "the tree is a quadtree"; the codec that
    /// populates it lands behind its milestone gate.
    Explicit,
}

impl TreeShape {
    /// The [`ChunkMeta`] of a node, when the shape can compute it (implicit
    /// trees). Explicit trees carry metas in their fetched pages — asking the
    /// shape yields `None`, forcing callers to go through the arena.
    pub fn meta_of(&self, node: NodeId) -> Option<ChunkMeta> {
        match self {
            TreeShape::ImplicitQuadtree(spec) => {
                let key = QuadKey::from_node_id(node);
                (spec.min_zoom..=spec.max_zoom)
                    .contains(&key.z)
                    .then(|| key.meta(spec.tile_px))
            }
            TreeShape::Explicit => None,
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::chunk::Refine;

    #[test]
    fn implicit_quadtree_computes_meta_inside_zoom_bounds_only() {
        let shape = TreeShape::ImplicitQuadtree(PyramidSpec {
            min_zoom: 3,
            max_zoom: 14,
            tile_px: 256,
        });
        let inside = QuadKey::new(11, 1058, 588).node_id();
        let meta = shape.meta_of(inside).expect("in-range node has meta");
        assert_eq!(meta.refine, Refine::Replace);
        assert!(meta.geometric_error_m > 0.0);
        let below = QuadKey::new(2, 1, 1).node_id();
        let above = QuadKey::new(15, 0, 0).node_id();
        assert!(shape.meta_of(below).is_none());
        assert!(shape.meta_of(above).is_none());
        assert!(TreeShape::Explicit.meta_of(inside).is_none());
    }
}
