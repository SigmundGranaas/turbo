//! Vector-tile tessellation. Walks each feature in a decoded `VectorTile`,
//! resolves the matching style rule, and emits a CPU-side mesh of position-
//! coloured triangles via lyon. The mesh is ready to upload to the GPU.
//!
//! Geometry is projected from tile-local coordinates into the renderer's
//! normalised world space at tessellation time, so the GPU sees the same
//! vertex format as the raster pipeline's quads. No coordinate work
//! happens per frame.

use bytemuck::{Pod, Zeroable};
use lyon::math::point;
use lyon::path::Path;
use lyon::tessellation::{
    BuffersBuilder, FillOptions, FillTessellator, FillVertex, StrokeOptions, StrokeTessellator,
    StrokeVertex, VertexBuffers,
};

use crate::{
    style::{Color, Paint, VectorStyle},
    tile::TileId,
    vector::{tile_local_to_world, Geometry, Value, VectorTile},
};

#[repr(C)]
#[derive(Debug, Clone, Copy, PartialEq, Pod, Zeroable)]
pub struct VectorVertex {
    pub position: [f32; 2], // world coords
    pub color: [u8; 4],
    /// Stored as `[u8; 4]` so it lines up with `Unorm8x4` (wgpu has no
    /// single-byte format). Only the first byte is meaningful: 0 at one
    /// stroke edge, 255 at the other, 128 for fills (no AA fringe).
    pub edge_pos: [u8; 4],
}

#[derive(Debug, Default, Clone)]
pub struct Mesh {
    pub vertices: Vec<VectorVertex>,
    pub indices: Vec<u32>,
}

impl Mesh {
    pub fn is_empty(&self) -> bool {
        self.indices.is_empty()
    }
}

/// One label "intent" extracted from a tile, ready to be laid out and
/// drawn by the text pipeline.
#[derive(Debug, Clone)]
pub struct LabelRequest {
    /// World-space anchor (normalised mercator).
    pub world_pos: (f32, f32),
    pub text: String,
    pub font_size_px: f32,
    pub color: Color,
}

/// A feature retained verbatim alongside the mesh, so the host can hit-test
/// it via `VectorMap::hit_test`. Only emitted for rules with
/// `interactive = true`.
#[derive(Debug, Clone)]
pub struct InteractiveFeature {
    pub source_layer: String,
    pub feature: crate::vector::Feature,
    /// Tile extent so the host can re-project tile-local coordinates if
    /// it wants to compare against world coords directly.
    pub extent: u32,
}

#[derive(Debug, Default, Clone)]
pub struct TessellationOutput {
    pub mesh: Mesh,
    pub labels: Vec<LabelRequest>,
    pub interactive: Vec<InteractiveFeature>,
}

/// Tessellate a tile through `style` into a single mesh. Errors out of
/// individual features (e.g. a degenerate polygon) are skipped silently —
/// it's better to render a slightly-incomplete tile than to drop the whole
/// tile because of one bad feature.
/// World-space tessellation tolerance for a given tile zoom. We target
/// roughly half-a-screen-pixel of curve approximation error, which is the
/// sweet spot between triangle count and visible faceting.
///
/// Pixels-per-world at zoom `z` = `256 * 2^z`. Half a pixel in world units
/// is `0.5 / (256 * 2^z)`. Clamped to a sane floor so deep zoom doesn't
/// pin lyon with vanishingly tight tolerances.
pub fn tolerance_for_zoom(z: u8) -> f32 {
    let ppw = 256.0_f64 * (1u64 << z) as f64;
    let raw = 0.5 / ppw;
    raw.max(1e-6) as f32
}

pub fn tessellate(tile_id: TileId, tile: &VectorTile, style: &VectorStyle) -> TessellationOutput {
    let mut fill_tess = FillTessellator::new();
    let mut stroke_tess = StrokeTessellator::new();
    let mut buffers: VertexBuffers<VectorVertex, u32> = VertexBuffers::new();
    let mut labels: Vec<LabelRequest> = Vec::new();
    let mut interactive: Vec<InteractiveFeature> = Vec::new();
    let tolerance = tolerance_for_zoom(tile_id.z);

    for layer in &tile.layers {
        let extent = layer.extent;
        for feature in &layer.features {
            let Some(idx) = style.matching_rule(&layer.name, feature, tile_id.z) else {
                continue;
            };
            let rule = &style.rules[idx];
            if rule.interactive {
                interactive.push(InteractiveFeature {
                    source_layer: layer.name.clone(),
                    feature: feature.clone(),
                    extent,
                });
            }
            // Clone-free borrow — Paint::Text holds a String we need to read.
            let paint = &rule.paint;
            match paint {
                Paint::Fill { color } => {
                    if let Geometry::Polygon(rings) = &feature.geometry {
                        let path = build_polygon_path(tile_id, extent, rings);
                        let packed = pack_color(*color);
                        let mut builder =
                            BuffersBuilder::new(&mut buffers, |v: FillVertex| VectorVertex {
                                position: v.position().to_array(),
                                color: packed,
                                edge_pos: [128, 0, 0, 0], // centre — no AA for fills
                            });
                        let _ = fill_tess.tessellate_path(
                            &path,
                            &FillOptions::default().with_tolerance(tolerance),
                            &mut builder,
                        );
                    }
                }
                Paint::Line { color, width } => {
                    if let Geometry::LineString(lines) = &feature.geometry {
                        let path = build_lines_path(tile_id, extent, lines);
                        let world_width =
                            (*width as f64 / extent as f64 / (1u64 << tile_id.z) as f64) as f32;
                        let packed = pack_color(*color);
                        let mut builder = BuffersBuilder::new(&mut buffers, |v: StrokeVertex| {
                            use lyon::tessellation::Side;
                            let edge_pos: u8 = match v.side() {
                                Side::Negative => 0,
                                Side::Positive => 255,
                            };
                            VectorVertex {
                                position: v.position().to_array(),
                                color: packed,
                                edge_pos: [edge_pos, 0, 0, 0],
                            }
                        });
                        let _ = stroke_tess.tessellate_path(
                            &path,
                            &StrokeOptions::default()
                                .with_line_width(world_width.max(1e-6))
                                .with_tolerance(tolerance),
                            &mut builder,
                        );
                    }
                }
                Paint::Text {
                    text_field,
                    font_size_px,
                    color,
                } => {
                    if let Geometry::Point(points) = &feature.geometry {
                        let Some(text) = read_text_property(feature, text_field) else {
                            continue;
                        };
                        for &p in points {
                            let (wx, wy) = tile_local_to_world(tile_id, extent, p);
                            labels.push(LabelRequest {
                                world_pos: (wx as f32, wy as f32),
                                text: text.clone(),
                                font_size_px: *font_size_px,
                                color: *color,
                            });
                        }
                    }
                }
            }
        }
    }

    TessellationOutput {
        mesh: Mesh {
            vertices: buffers.vertices,
            indices: buffers.indices,
        },
        labels,
        interactive,
    }
}

fn read_text_property(feature: &crate::vector::Feature, text_field: &str) -> Option<String> {
    match feature.properties.get(text_field)? {
        Value::String(s) => Some(s.clone()),
        Value::Int(i) => Some(i.to_string()),
        Value::UInt(u) => Some(u.to_string()),
        Value::Float(f) => Some(format!("{f}")),
        Value::Bool(b) => Some(b.to_string()),
        Value::Null => None,
    }
}

fn build_polygon_path(tile_id: TileId, extent: u32, rings: &[Vec<(i32, i32)>]) -> Path {
    let mut builder = Path::builder();
    for ring in rings {
        if ring.len() < 3 {
            continue;
        }
        let first = project(tile_id, extent, ring[0]);
        builder.begin(first);
        for &v in &ring[1..] {
            builder.line_to(project(tile_id, extent, v));
        }
        builder.close();
    }
    builder.build()
}

fn build_lines_path(tile_id: TileId, extent: u32, lines: &[Vec<(i32, i32)>]) -> Path {
    let mut builder = Path::builder();
    for line in lines {
        if line.len() < 2 {
            continue;
        }
        builder.begin(project(tile_id, extent, line[0]));
        for &v in &line[1..] {
            builder.line_to(project(tile_id, extent, v));
        }
        builder.end(false);
    }
    builder.build()
}

fn project(tile_id: TileId, extent: u32, local: (i32, i32)) -> lyon::math::Point {
    let (x, y) = tile_local_to_world(tile_id, extent, local);
    point(x as f32, y as f32)
}

/// Vertex colours are consumed by shaders writing to an sRGB target, so
/// the sRGB-authored style colour is decoded to linear exactly here.
fn pack_color(c: Color) -> [u8; 4] {
    c.to_linear_bytes()
}

#[cfg(test)]
mod tests {
    //! The tessellator is the bridge between vector data and the GPU. The
    //! value boundary worth testing: a feature with a matching rule produces
    //! at least one triangle in the mesh, and a feature with no rule
    //! produces none. We don't test the geometry of the triangles — lyon's
    //! own test suite covers the tessellation algorithm.
    use super::*;
    use crate::style::{Filter, Paint, Rule};
    use crate::vector::{Feature, GeomType, Geometry, VectorTile};
    use std::collections::HashMap;

    #[test]
    fn tolerance_for_zoom_is_strictly_decreasing_in_zoom() {
        // Higher zoom = smaller world units = tighter tolerance.
        let mut last = f32::INFINITY;
        for z in 0..=20u8 {
            let t = tolerance_for_zoom(z);
            assert!(t > 0.0 && t.is_finite(), "tolerance @ z={z} = {t}");
            assert!(t <= last, "non-monotonic: z={z} got {t}, prev {last}");
            last = t;
        }
    }

    #[test]
    fn tolerance_at_zoom_zero_is_about_half_a_pixel_world_unit() {
        // ppw at z=0 = 256, so tolerance ≈ 0.5/256 = 0.001953125.
        let t = tolerance_for_zoom(0);
        assert!(
            (t - (0.5 / 256.0_f32)).abs() < 1e-6,
            "expected ~0.00195, got {t}",
        );
    }

    #[test]
    fn tolerance_clamps_above_a_floor_at_deep_zoom() {
        // At z=22, raw tolerance is ~1e-10 — well below our 1e-6 floor.
        let t = tolerance_for_zoom(22);
        assert!(t >= 1e-6, "got {t}");
    }

    fn poly(rings: Vec<Vec<(i32, i32)>>) -> Feature {
        Feature {
            id: 0,
            geom_type: GeomType::Polygon,
            geometry: Geometry::Polygon(rings),
            properties: HashMap::new(),
        }
    }

    fn line(lines: Vec<Vec<(i32, i32)>>) -> Feature {
        Feature {
            id: 0,
            geom_type: GeomType::LineString,
            geometry: Geometry::LineString(lines),
            properties: HashMap::new(),
        }
    }

    #[test]
    fn polygon_matching_a_fill_rule_produces_triangles() {
        let tile = VectorTile {
            layers: vec![crate::vector::Layer {
                name: "water".into(),
                version: 2,
                extent: 4096,
                features: vec![poly(vec![vec![
                    (100, 100),
                    (200, 100),
                    (200, 200),
                    (100, 200),
                    (100, 100),
                ]])],
            }],
        };
        let style = VectorStyle {
            background: Color::rgb(255, 255, 255),
            rules: vec![Rule {
                source_layer: "water".into(),
                filter: Filter::Always,
                paint: Paint::Fill {
                    color: Color::rgb(0, 0, 255),
                },
                min_zoom: 0,
                max_zoom: 22,
                interactive: false,
            }],
        };
        let out = tessellate(TileId::new(5, 1, 1), &tile, &style);
        let mesh = &out.mesh;
        assert!(
            mesh.indices.len() >= 3,
            "a quad must tessellate to at least one triangle (3 indices), got {}",
            mesh.indices.len(),
        );
        assert!(
            mesh.indices.len().is_multiple_of(3),
            "index count must be a multiple of 3"
        );
    }

    #[test]
    fn feature_with_no_matching_rule_is_omitted() {
        let tile = VectorTile {
            layers: vec![crate::vector::Layer {
                name: "buildings".into(),
                version: 2,
                extent: 4096,
                features: vec![poly(vec![vec![
                    (100, 100),
                    (200, 100),
                    (200, 200),
                    (100, 100),
                ]])],
            }],
        };
        let style = VectorStyle {
            background: Color::rgb(255, 255, 255),
            rules: vec![Rule {
                // only matches "water", not "buildings"
                source_layer: "water".into(),
                filter: Filter::Always,
                paint: Paint::Fill {
                    color: Color::rgb(0, 0, 255),
                },
                min_zoom: 0,
                max_zoom: 22,
                interactive: false,
            }],
        };
        let out = tessellate(TileId::new(5, 1, 1), &tile, &style);
        let mesh = &out.mesh;
        assert!(
            mesh.is_empty(),
            "no rule matches → mesh must be empty (got {} indices)",
            mesh.indices.len(),
        );
    }

    #[test]
    fn linestring_matching_a_line_rule_produces_a_stroke_mesh() {
        let tile = VectorTile {
            layers: vec![crate::vector::Layer {
                name: "transportation".into(),
                version: 2,
                extent: 4096,
                features: vec![line(vec![vec![(0, 0), (1000, 0), (2000, 500)]])],
            }],
        };
        let style = VectorStyle {
            background: Color::rgb(255, 255, 255),
            rules: vec![Rule {
                source_layer: "transportation".into(),
                filter: Filter::Always,
                paint: Paint::Line {
                    color: Color::rgb(255, 0, 0),
                    width: 30.0,
                },
                min_zoom: 0,
                max_zoom: 22,
                interactive: false,
            }],
        };
        let out = tessellate(TileId::new(5, 1, 1), &tile, &style);
        let mesh = &out.mesh;
        assert!(
            mesh.indices.len() >= 6,
            "two segments must tessellate to at least 2 triangles (6 indices), got {}",
            mesh.indices.len(),
        );
    }

    #[test]
    fn point_feature_matching_a_text_rule_produces_a_label_request() {
        let mut props = HashMap::new();
        props.insert(
            "name".to_owned(),
            crate::vector::Value::String("Bergen".into()),
        );
        let feat = Feature {
            id: 0,
            geom_type: GeomType::Point,
            // Tile-local (2048, 2048) at extent 4096 = middle of the tile.
            geometry: Geometry::Point(vec![(2048, 2048)]),
            properties: props,
        };
        let tile = VectorTile {
            layers: vec![crate::vector::Layer {
                name: "place".into(),
                version: 2,
                extent: 4096,
                features: vec![feat],
            }],
        };
        let style = VectorStyle {
            background: Color::rgb(255, 255, 255),
            rules: vec![Rule {
                source_layer: "place".into(),
                filter: Filter::Always,
                paint: Paint::Text {
                    text_field: "name".into(),
                    font_size_px: 14.0,
                    color: Color::rgb(0, 0, 0),
                },
                min_zoom: 0,
                max_zoom: 22,
                interactive: false,
            }],
        };
        let out = tessellate(TileId::new(10, 512, 512), &tile, &style);
        assert!(out.mesh.is_empty(), "text emits no geometry triangles");
        assert_eq!(out.labels.len(), 1);
        let label = &out.labels[0];
        assert_eq!(label.text, "Bergen");
        assert!((label.font_size_px - 14.0).abs() < 1e-6);
        // Middle of tile (512, 512) at z=10: world centre = (512.5/1024, 512.5/1024).
        assert!((label.world_pos.0 as f64 - 512.5 / 1024.0).abs() < 1e-6);
        assert!((label.world_pos.1 as f64 - 512.5 / 1024.0).abs() < 1e-6);
    }

    #[test]
    fn point_without_the_text_field_produces_no_label() {
        // A `place` feature without a `name` property must be skipped — the
        // alternative is showing empty boxes or worse, raw IDs.
        let feat = Feature {
            id: 1,
            geom_type: GeomType::Point,
            geometry: Geometry::Point(vec![(2048, 2048)]),
            properties: HashMap::new(),
        };
        let tile = VectorTile {
            layers: vec![crate::vector::Layer {
                name: "place".into(),
                version: 2,
                extent: 4096,
                features: vec![feat],
            }],
        };
        let style = VectorStyle {
            background: Color::rgb(255, 255, 255),
            rules: vec![Rule {
                source_layer: "place".into(),
                filter: Filter::Always,
                paint: Paint::Text {
                    text_field: "name".into(),
                    font_size_px: 14.0,
                    color: Color::rgb(0, 0, 0),
                },
                min_zoom: 0,
                max_zoom: 22,
                interactive: false,
            }],
        };
        let out = tessellate(TileId::new(10, 512, 512), &tile, &style);
        assert!(out.labels.is_empty());
    }
}
