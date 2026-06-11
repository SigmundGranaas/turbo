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
    BuffersBuilder, FillOptions, FillTessellator, FillVertex, LineCap, LineJoin, StrokeOptions,
    StrokeTessellator, StrokeVertex, VertexBuffers,
};

use crate::{
    style::{Color, Paint, VectorStyle},
    tile::TileId,
    vector::{tile_local_to_world, Geometry, Value, VectorTile},
};

#[repr(C)]
#[derive(Debug, Clone, Copy, PartialEq, Pod, Zeroable)]
pub struct VectorVertex {
    /// **Tile-local** centerline position (the point on the path, *not*
    /// the stroke edge), `[0, 1]` across the tile. The vertex shader
    /// places it in world space via the per-tile origin/span uniform and
    /// extrudes along `normal`.
    pub base: [f32; 2],
    /// Unit normal in tile space (0,0 for fills) — the tile→world
    /// transform is a uniform scale, so the direction is also the world
    /// normal. lyon defines it so `edge = base + normal * half_width`.
    pub normal: [f32; 2],
    /// Line width in screen pixels (0 for fills). The shader converts to
    /// world half-width per frame, giving pixel-constant, smoothly-zooming
    /// strokes regardless of the tessellation zoom.
    pub width_px: f32,
    pub color: [u8; 4],
    /// Stored as `[u8; 4]` so it lines up with `Unorm8x4` (wgpu has no
    /// single-byte format). Only the first byte is meaningful: 0 at one
    /// stroke edge, 255 at the other, 128 for fills (no AA fringe).
    pub edge_pos: [u8; 4],
    /// Distance along the path in world units (lyon's `advancement`), 0 for
    /// fills. The shader scales it to screen pixels for pixel-constant dash
    /// patterns; ignored when the layer isn't dashed.
    pub dist: f32,
    /// World-space height above the ground plane. 0 for all flat features
    /// (roads, ordinary fills) — the vertex shader places those at z=0
    /// exactly as before. Non-zero only for extruded geometry (3D building
    /// roofs/walls), where the perspective camera renders the height.
    pub z: f32,
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
    /// World-space anchor (normalised mercator). For line-placed labels
    /// this is the centerline's midpoint — used for collision sorting and
    /// off-screen culling.
    pub world_pos: (f32, f32),
    pub text: String,
    pub font_size_px: f32,
    pub color: Color,
    /// Readability outline. `halo_width` in glyph pixels (0 = none).
    pub halo_color: Color,
    pub halo_width: f32,
    /// Placement importance — higher wins collisions. Defaults to font
    /// size when no rank property is configured.
    pub rank: f32,
    /// World-space centerline for a label placed *along* a line (a road
    /// name). `None` for ordinary point labels. The text pipeline projects
    /// these to screen and runs each glyph along the curve.
    pub path: Option<Vec<(f32, f32)>>,
    /// When `Some(pad)`, a point label is left-anchored: its left edge sits
    /// `pad` screen-pixels right of the projected anchor (clearing an icon).
    /// `None` ⇒ centred on the anchor (the default). Ignored for line labels.
    pub left_pad_px: Option<f32>,
}

/// One sprite "intent" extracted from a tile: a named icon to draw at a
/// world-space point (a POI marker or a route-shield background).
#[derive(Debug, Clone)]
pub struct IconRequest {
    pub world_pos: (f32, f32),
    pub sprite: String,
    pub size_px: f32,
    pub color: Color,
    /// When `true`, this icon is the marker for a left-anchored label (a
    /// POI dot) and must only draw if that label survives collision — so a
    /// dot never appears orphaned without its name. The icon and its label
    /// share an identical `world_pos`, which is how the text pass tells the
    /// icon pass which markers were placed.
    pub requires_label: bool,
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
    pub icons: Vec<IconRequest>,
    pub interactive: Vec<InteractiveFeature>,
}

/// Tessellation tolerance in **tile units** (one tile = 1.0). One tile is
/// 256 px at its native zoom, so half a screen pixel of curve error is
/// `0.5 / 256` — and because meshes are built in tile-local space, this is
/// the same at every zoom.
///
/// Tile-local tessellation is what makes city zooms work at all: in
/// absolute world coordinates a z14+ street segment is ~1e-6 world units,
/// inside f32 ULP territory around x≈0.5, and lyon collapses the geometry.
/// In tile units the same segment is ~2e-2 — full precision everywhere.
/// The GPU places each mesh with the per-tile `origin + base * span`
/// transform carried in the tile uniform.
pub fn tile_tolerance() -> f32 {
    0.5 / 256.0
}

/// Tessellate a tile through `style` into a single mesh in **tile-local
/// units** (`[0, 1]` across the tile, buffer geometry slightly outside).
/// Errors out of individual features (e.g. a degenerate polygon) are
/// skipped silently — it's better to render a slightly-incomplete tile
/// than to drop the whole tile because of one bad feature.
pub fn tessellate(tile_id: TileId, tile: &VectorTile, style: &VectorStyle) -> TessellationOutput {
    let mut fill_tess = FillTessellator::new();
    let mut stroke_tess = StrokeTessellator::new();
    let mut buffers: VertexBuffers<VectorVertex, u32> = VertexBuffers::new();
    let mut labels: Vec<LabelRequest> = Vec::new();
    let mut icons: Vec<IconRequest> = Vec::new();
    let mut interactive: Vec<InteractiveFeature> = Vec::new();
    let tolerance = tile_tolerance();

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
                                base: v.position().to_array(),
                                normal: [0.0, 0.0], // fills don't extrude
                                width_px: 0.0,
                                color: packed,
                                edge_pos: [128, 0, 0, 0], // centre — no AA for fills
                                dist: 0.0,
                                z: 0.0,
                            });
                        let _ = fill_tess.tessellate_path(
                            &path,
                            &FillOptions::default().with_tolerance(tolerance),
                            &mut builder,
                        );
                    }
                }
                Paint::FillExtrusion {
                    color,
                    height_m,
                    height_property,
                } => {
                    if let Geometry::Polygon(rings) = &feature.geometry {
                        // Per-feature height (OMT `render_height`) when the
                        // property is present and numeric, else the default.
                        let height_m = height_property
                            .as_deref()
                            .and_then(|f| read_number_property(feature, f))
                            .map(|n| n as f32)
                            .unwrap_or(*height_m);
                        let h = meters_to_world_z(tile_id, height_m);
                        let roof = pack_color(*color);
                        // Roof: the polygon fill, lifted to z = h.
                        let path = build_polygon_path(tile_id, extent, rings);
                        let mut builder =
                            BuffersBuilder::new(&mut buffers, |v: FillVertex| VectorVertex {
                                base: v.position().to_array(),
                                normal: [0.0, 0.0],
                                width_px: 0.0,
                                color: roof,
                                edge_pos: [128, 0, 0, 0],
                                dist: 0.0,
                                z: h,
                            });
                        let _ = fill_tess.tessellate_path(
                            &path,
                            &FillOptions::default().with_tolerance(tolerance),
                            &mut builder,
                        );
                        // Walls: a vertical quad per ring edge, ground → roof,
                        // each face flat-shaded by its compass orientation so
                        // sunlit and shadowed sides read distinctly.
                        for ring in rings {
                            let pts: Vec<[f32; 2]> = ring
                                .iter()
                                .map(|&p| project(tile_id, extent, p).to_array())
                                .collect();
                            for seg in pts.windows(2) {
                                let (a, b) = (seg[0], seg[1]);
                                let wall = pack_color(shade(*color, wall_shade(a, b)));
                                let v = |xy: [f32; 2], z: f32| VectorVertex {
                                    base: xy,
                                    normal: [0.0, 0.0],
                                    width_px: 0.0,
                                    color: wall,
                                    edge_pos: [128, 0, 0, 0],
                                    dist: 0.0,
                                    z,
                                };
                                let base_i = buffers.vertices.len() as u32;
                                buffers.vertices.push(v(a, 0.0));
                                buffers.vertices.push(v(b, 0.0));
                                buffers.vertices.push(v(b, h));
                                buffers.vertices.push(v(a, h));
                                buffers.indices.extend_from_slice(&[
                                    base_i,
                                    base_i + 1,
                                    base_i + 2,
                                    base_i,
                                    base_i + 2,
                                    base_i + 3,
                                ]);
                            }
                        }
                    }
                }
                Paint::Line { color, width } => {
                    // A line rule strokes line geometry *and* polygon rings
                    // (the latter is the outline around fills).
                    let path = match &feature.geometry {
                        Geometry::LineString(lines) => {
                            Some(build_lines_path(tile_id, extent, lines))
                        }
                        Geometry::Polygon(rings) => {
                            Some(build_polygon_path(tile_id, extent, rings))
                        }
                        Geometry::Point(_) => None,
                    };
                    if let Some(path) = path {
                        // `width` is screen pixels. Tessellate at the
                        // equivalent width in tile units (one tile = 256 px
                        // at its native zoom) — that only sets arc density
                        // for round joins/caps; the shader re-extrudes to
                        // the camera's exact px.
                        let width_px = *width;
                        let tile_width = width_px / 256.0;
                        let packed = pack_color(*color);
                        let mut builder = BuffersBuilder::new(&mut buffers, |v: StrokeVertex| {
                            use lyon::tessellation::Side;
                            let edge_pos: u8 = match v.side() {
                                Side::Negative => 0,
                                Side::Positive => 255,
                            };
                            let n = v.normal();
                            VectorVertex {
                                base: v.position_on_path().to_array(),
                                normal: [n.x, n.y],
                                width_px,
                                color: packed,
                                edge_pos: [edge_pos, 0, 0, 0],
                                // Tile-unit arc length for dash patterns
                                // (the shader scales by span × ppw).
                                dist: v.advancement(),
                                z: 0.0, // lines are flat on the ground plane
                            }
                        });
                        let _ = stroke_tess.tessellate_path(
                            &path,
                            // Round joins + caps — the cartographic default.
                            // Miter joins spike at sharp route bends and
                            // butt caps leave hard line ends; rounding both
                            // is what makes roads and routes read as roads.
                            &StrokeOptions::default()
                                .with_line_width(tile_width.max(1e-4))
                                .with_line_join(LineJoin::Round)
                                .with_line_cap(LineCap::Round)
                                .with_tolerance(tolerance),
                            &mut builder,
                        );
                    }
                }
                Paint::Text {
                    text_field,
                    font_size_px,
                    color,
                    halo_color,
                    halo_width,
                    rank_field,
                    along_line,
                    icon,
                    left_anchor,
                } => {
                    // Text is optional: an icon-only layer (a bare POI
                    // marker) has no `text_field` value, but still draws.
                    let text = read_text_property(feature, text_field);
                    // Importance: the ranked property if present and
                    // numeric, else the font size (bigger ⇒ stronger).
                    let rank = rank_field
                        .as_deref()
                        .and_then(|f| read_number_property(feature, f))
                        .map(|n| n as f32)
                        .unwrap_or(*font_size_px);
                    // Left-anchored labels sit to the right of their anchor,
                    // clearing the icon (half its width) plus a small gap.
                    let left_pad_px = left_anchor.then(|| {
                        icon.as_ref().map(|i| i.size_px * 0.5).unwrap_or(0.0) + 3.0
                    });
                    let make_label = |world_pos: (f32, f32), path: Option<Vec<(f32, f32)>>| {
                        text.as_ref().map(|t| LabelRequest {
                            world_pos,
                            text: t.clone(),
                            font_size_px: *font_size_px,
                            color: *color,
                            halo_color: *halo_color,
                            halo_width: *halo_width,
                            rank,
                            path,
                            left_pad_px,
                        })
                    };
                    if *along_line {
                        // Icons aren't placed along lines — only the name.
                        if let Geometry::LineString(lines) = &feature.geometry {
                            for line in lines {
                                if line.len() < 2 {
                                    continue;
                                }
                                let path: Vec<(f32, f32)> = line
                                    .iter()
                                    .map(|&p| {
                                        let (x, y) = tile_local_to_world(tile_id, extent, p);
                                        (x as f32, y as f32)
                                    })
                                    .collect();
                                // Anchor = the vertex nearest the path's
                                // halfway arc length, a stable representative
                                // for collision sorting and culling.
                                let anchor = path_midpoint(&path);
                                if let Some(l) = make_label(anchor, Some(path)) {
                                    labels.push(l);
                                }
                            }
                        }
                    } else if let Geometry::Point(points) = &feature.geometry {
                        for &p in points {
                            let (wx, wy) = tile_local_to_world(tile_id, extent, p);
                            let world_pos = (wx as f32, wy as f32);
                            // Icon behind, label on top — at the same anchor
                            // they compose into a route shield (centred) or a
                            // POI marker (left-anchored). A left-anchored
                            // marker's dot is gated on its label surviving.
                            if let Some(spec) = icon {
                                icons.push(IconRequest {
                                    world_pos,
                                    sprite: spec.sprite.clone(),
                                    size_px: spec.size_px,
                                    color: spec.color,
                                    requires_label: *left_anchor,
                                });
                            }
                            if let Some(l) = make_label(world_pos, None) {
                                labels.push(l);
                            }
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
        icons,
        interactive,
    }
}

/// The point at half the total arc length of a world-space polyline — the
/// stable anchor a line label is sorted and culled by.
fn path_midpoint(path: &[(f32, f32)]) -> (f32, f32) {
    let total: f32 = path
        .windows(2)
        .map(|w| {
            let dx = w[1].0 - w[0].0;
            let dy = w[1].1 - w[0].1;
            (dx * dx + dy * dy).sqrt()
        })
        .sum();
    let mut acc = 0.0;
    let half = total * 0.5;
    for w in path.windows(2) {
        let dx = w[1].0 - w[0].0;
        let dy = w[1].1 - w[0].1;
        let seg = (dx * dx + dy * dy).sqrt();
        if acc + seg >= half {
            let t = if seg > 0.0 { (half - acc) / seg } else { 0.0 };
            return (w[0].0 + dx * t, w[0].1 + dy * t);
        }
        acc += seg;
    }
    path.first().copied().unwrap_or((0.0, 0.0))
}

/// Read a numeric feature property for ranking. Strings that parse as
/// numbers are accepted too (real data is inconsistent).
fn read_number_property(feature: &crate::vector::Feature, field: &str) -> Option<f64> {
    match feature.properties.get(field)? {
        Value::Float(f) => Some(*f),
        Value::Int(i) => Some(*i as f64),
        Value::UInt(u) => Some(*u as f64),
        Value::Bool(b) => Some(*b as i64 as f64),
        Value::String(s) => s.parse().ok(),
        Value::Null => None,
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

/// Project MVT integer coordinates into tile-local units: `[0, 1]` across
/// the tile, buffer geometry slightly outside. Mesh placement into world
/// space happens on the GPU via the per-tile `origin + base * span`
/// transform, so f32 keeps full precision at every zoom.
fn project(_tile_id: TileId, extent: u32, local: (i32, i32)) -> lyon::math::Point {
    let e = extent as f32;
    point(local.0 as f32 / e, local.1 as f32 / e)
}

/// Vertex colours are consumed by shaders writing to an sRGB target, so
/// the sRGB-authored style colour is decoded to linear exactly here.
fn pack_color(c: Color) -> [u8; 4] {
    c.to_linear_bytes()
}

/// Multiply a colour's RGB by `f` (alpha unchanged) for cheap wall shading.
fn shade(c: Color, f: f32) -> Color {
    let s = |v: u8| (v as f32 * f).clamp(0.0, 255.0) as u8;
    Color::rgba(s(c.r), s(c.g), s(c.b), c.a)
}

/// Diffuse-light brightness for a wall face running from tile-local point
/// `a` to `b`. The face's outward normal (perpendicular to the edge) is
/// dotted with a fixed light from the north-west — the cartographic
/// convention shared with the hillshade. Result in `[0.5, 0.85]`: shadowed
/// sides stay dark, lit sides brighten, all below the roof's full colour.
fn wall_shade(a: [f32; 2], b: [f32; 2]) -> f32 {
    // Tile-local axes: +x east, +y south. Light *coming from* the NW means
    // the vector toward the light is (west, north) = (-x, -y).
    const TO_LIGHT: [f32; 2] = [-std::f32::consts::FRAC_1_SQRT_2, -std::f32::consts::FRAC_1_SQRT_2];
    let (dx, dy) = (b[0] - a[0], b[1] - a[1]);
    let len = (dx * dx + dy * dy).sqrt();
    if len < 1e-9 {
        return 0.7;
    }
    // Outward normal of the edge (one of the two perpendiculars). Building
    // rings share a winding, so the choice is consistent across the city.
    let (nx, ny) = (dy / len, -dx / len);
    let lambert = (nx * TO_LIGHT[0] + ny * TO_LIGHT[1]).max(0.0);
    0.5 + 0.35 * lambert
}

/// Convert a height in metres to world-Z (the same units as the tile-local
/// mesh once placed by `origin + base * span`). Web Mercator stretches with
/// latitude, so 1 world unit = `EQUATOR_M · cos(lat)` ground metres at the
/// tile's centre latitude — heights then read proportional to the streets
/// around them.
fn meters_to_world_z(tile_id: TileId, height_m: f32) -> f32 {
    const EQUATOR_M: f64 = 40_075_016.7;
    let n = (1u64 << tile_id.z) as f64;
    let world_y = (tile_id.y as f64 + 0.5) / n;
    let lat = crate::geo::WorldPoint::new(0.5, world_y).to_lat_lng().lat;
    let m_per_world = EQUATOR_M * lat.to_radians().cos();
    (height_m as f64 / m_per_world) as f32
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
    fn tile_tolerance_is_half_a_native_pixel() {
        // One tile = 256 px at its native zoom; tolerance is half a pixel
        // of curve error in tile units — and zoom-independent, because
        // meshes are tile-local.
        let t = tile_tolerance();
        assert!((t - 0.5 / 256.0).abs() < 1e-9, "expected 0.5/256, got {t}");
    }

    #[test]
    fn high_zoom_geometry_keeps_full_precision() {
        // The regression behind tile-local tessellation: a short z14
        // street segment must produce a real stroke mesh, not collapse
        // into f32 ULPs as it did in absolute world coordinates.
        let tile = VectorTile {
            layers: vec![crate::vector::Layer {
                name: "transportation".into(),
                version: 2,
                extent: 4096,
                // ~100 extent units ≈ 30 m at z14 — a short city block.
                features: vec![line(vec![vec![(2000, 2000), (2100, 2030)]])],
            }],
        };
        let style = VectorStyle {
            background: Color::rgb(255, 255, 255),
            rules: vec![Rule {
                source_layer: "transportation".into(),
                filter: Filter::Always,
                paint: Paint::Line { color: Color::rgb(255, 0, 0), width: 4.0 },
                min_zoom: 0,
                max_zoom: 22,
                interactive: false,
            }],
        };
        let out = tessellate(TileId::new(14, 8434, 4722), &tile, &style);
        assert!(
            out.mesh.indices.len() >= 6,
            "a z14 street segment must tessellate ({} indices)",
            out.mesh.indices.len()
        );
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
    fn fill_extrusion_emits_a_raised_roof_and_walls() {
        // A square building extruded: the mesh must contain roof vertices at
        // z = height and wall vertices spanning ground (z=0) to roof — a flat
        // fill would be entirely z=0.
        let tile = VectorTile {
            layers: vec![crate::vector::Layer {
                name: "building".into(),
                version: 2,
                extent: 4096,
                features: vec![poly(vec![vec![
                    (1000, 1000),
                    (1200, 1000),
                    (1200, 1200),
                    (1000, 1200),
                    (1000, 1000),
                ]])],
            }],
        };
        let style = VectorStyle {
            background: Color::rgb(255, 255, 255),
            rules: vec![Rule {
                source_layer: "building".into(),
                filter: Filter::Always,
                paint: Paint::FillExtrusion {
                    color: Color::rgb(200, 190, 180),
                    height_m: 20.0,
                    height_property: None,
                },
                min_zoom: 0,
                max_zoom: 22,
                interactive: false,
            }],
        };
        let out = tessellate(TileId::new(14, 8434, 4722), &tile, &style);
        let zs: Vec<f32> = out.mesh.vertices.iter().map(|v| v.z).collect();
        let h = zs.iter().cloned().fold(0.0f32, f32::max);
        assert!(h > 0.0, "extrusion has a non-zero height");
        assert!(zs.contains(&0.0), "walls reach the ground (z=0)");
        assert!(zs.iter().any(|&z| (z - h).abs() < 1e-9), "roof + wall tops at z=h");
        // Roof (≥1 tri) + 4 wall quads (2 tris each) ⇒ well over a flat fill.
        assert!(out.mesh.indices.len() >= 3 + 4 * 6, "roof + four walls");
    }

    #[test]
    fn polygon_matching_a_line_rule_strokes_its_outline() {
        // A line rule on a polygon source-layer strokes the ring — the
        // building/water/landuse outline. Must produce stroke geometry
        // carrying the line's pixel width (fills carry width_px 0).
        let tile = VectorTile {
            layers: vec![crate::vector::Layer {
                name: "building".into(),
                version: 2,
                extent: 4096,
                features: vec![poly(vec![vec![
                    (100, 100),
                    (300, 100),
                    (300, 300),
                    (100, 300),
                    (100, 100),
                ]])],
            }],
        };
        let style = VectorStyle {
            background: Color::rgb(255, 255, 255),
            rules: vec![Rule {
                source_layer: "building".into(),
                filter: Filter::Always,
                paint: Paint::Line { color: Color::rgb(120, 110, 90), width: 1.5 },
                min_zoom: 0,
                max_zoom: 22,
                interactive: false,
            }],
        };
        let out = tessellate(TileId::new(14, 8434, 4722), &tile, &style);
        assert!(out.mesh.indices.len() >= 6, "ring must stroke to triangles");
        assert!(
            out.mesh.vertices.iter().all(|v| (v.width_px - 1.5).abs() < 1e-4),
            "outline vertices must carry the line width, not 0 (fill)"
        );
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
    fn left_anchored_text_with_an_icon_sets_a_clearing_pad() {
        // A POI marker: dot icon + left-anchored name. The label must carry
        // a left pad of (icon half-width + gap) so the text clears the dot;
        // a centred label carries None.
        let mut props = HashMap::new();
        props.insert("name".to_owned(), crate::vector::Value::String("Cafe".into()));
        let feat = Feature {
            id: 0,
            geom_type: GeomType::Point,
            geometry: Geometry::Point(vec![(2048, 2048)]),
            properties: props,
        };
        let tile = VectorTile {
            layers: vec![crate::vector::Layer {
                name: "poi".into(),
                version: 2,
                extent: 4096,
                features: vec![feat],
            }],
        };
        let text = |left_anchor, icon| Paint::Text {
            text_field: "name".into(),
            font_size_px: 11.0,
            color: Color::rgb(0, 0, 0),
            halo_color: Color::rgba(0, 0, 0, 0),
            halo_width: 0.0,
            rank_field: None,
            along_line: false,
            icon,
            left_anchor,
        };
        let rule = |paint| Rule {
            source_layer: "poi".into(),
            filter: Filter::Always,
            paint,
            min_zoom: 0,
            max_zoom: 22,
            interactive: false,
        };
        let icon = crate::style::IconSpec {
            sprite: "dot".into(),
            size_px: 8.0,
            color: Color::rgb(255, 128, 0),
        };
        let left = VectorStyle {
            background: Color::rgb(255, 255, 255),
            rules: vec![rule(text(true, Some(icon)))],
        };
        let out = tessellate(TileId::new(14, 0, 0), &tile, &left);
        // pad = icon half (4.0) + 3.0 gap.
        assert_eq!(out.labels[0].left_pad_px, Some(7.0));
        // The dot is gated on its label and shares the label's world_pos —
        // that pairing is what makes them cull as a unit.
        assert_eq!(out.icons.len(), 1);
        assert!(out.icons[0].requires_label, "POI dot waits for its label");
        assert_eq!(out.icons[0].world_pos, out.labels[0].world_pos);

        let centred = VectorStyle {
            background: Color::rgb(255, 255, 255),
            rules: vec![rule(text(false, None))],
        };
        let out = tessellate(TileId::new(14, 0, 0), &tile, &centred);
        assert_eq!(out.labels[0].left_pad_px, None, "centred label has no pad");
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
                    halo_color: Color::rgba(0, 0, 0, 0),
                    halo_width: 0.0,
                    rank_field: None,
                    along_line: false,
                    icon: None,
                    left_anchor: false,
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
                    halo_color: Color::rgba(0, 0, 0, 0),
                    halo_width: 0.0,
                    rank_field: None,
                    along_line: false,
                    icon: None,
                    left_anchor: false,
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
