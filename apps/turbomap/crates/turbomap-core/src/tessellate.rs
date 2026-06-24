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
    /// Placement priority — **lower wins** collisions, matching MapLibre's
    /// `symbol-sort-key`. This is what makes OMT `rank` (1 = most important
    /// place) Just Work: rank 1 is placed before rank 10. When no rank
    /// property is configured it defaults to `-font_size_px`, so a bigger
    /// label still beats a smaller one (more negative ⇒ placed first).
    pub sort_key: f32,
    /// World-space centerline for a label placed *along* a line (a road
    /// name). `None` for ordinary point labels. The text pipeline projects
    /// these to screen and runs each glyph along the curve.
    pub path: Option<Vec<(f32, f32)>>,
    /// When `Some(pad)`, a point label is left-anchored: its left edge sits
    /// `pad` screen-pixels right of the projected anchor (clearing an icon).
    /// `None` ⇒ centred on the anchor (the default). Ignored for line labels.
    pub left_pad_px: Option<f32>,
    /// Extra tracking between glyphs, in em (0 = none). Point/area labels
    /// (water, districts) space out the way real basemaps track them.
    pub letter_spacing: f32,
    /// Faux-bold weight in glyph raster pixels (0 = the font's natural
    /// weight). Drives the label-weight hierarchy: heavy place names, medium
    /// area labels, light street names.
    pub weight: f32,
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
    /// Water-fill triangles, split out of `mesh` so the renderer can draw them
    /// through the dedicated realistic-water pipeline (animated waves, Fresnel,
    /// sky reflection, sun glitter) instead of as a flat matte fill. Same vertex
    /// format as `mesh`; the baked `color` is reused as the deep-water tint.
    /// Empty when the tile has no water polygons.
    pub water_mesh: Mesh,
    pub labels: Vec<LabelRequest>,
    pub icons: Vec<IconRequest>,
    pub interactive: Vec<InteractiveFeature>,
}

/// Whether an MVT source layer carries water-body polygons that should render
/// through the realistic-water pipeline rather than as a flat fill. Matches the
/// OpenMapTiles convention (`"water"`); waterway *rivers* are `Line` paint and
/// stay in the ordinary vector mesh, so only fills land here.
pub fn is_water_source_layer(name: &str) -> bool {
    name == "water"
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

/// Per-tile vertex budget for the refined water grid (the displaceable surface
/// the realistic-water Gerstner path samples).
const WATER_GRID_VERT_BUDGET: usize = 8192;
/// Max uniform-subdivision rounds (each multiplies triangles by 4).
const WATER_GRID_MAX_ROUNDS: u32 = 3;

/// Shared edge midpoint vertex (cached by the unordered endpoint pair so
/// adjacent triangles reuse it ⇒ the refined mesh stays watertight). Interpolates
/// `base`/`z`; fills carry no stroke attributes (normal/width/dist = 0).
fn edge_midpoint(
    verts: &mut Vec<VectorVertex>,
    cache: &mut std::collections::HashMap<(u32, u32), u32>,
    a: u32,
    b: u32,
) -> u32 {
    let key = if a < b { (a, b) } else { (b, a) };
    if let Some(&m) = cache.get(&key) {
        return m;
    }
    let va = verts[a as usize];
    let vb = verts[b as usize];
    let m = VectorVertex {
        base: [
            (va.base[0] + vb.base[0]) * 0.5,
            (va.base[1] + vb.base[1]) * 0.5,
        ],
        normal: [0.0, 0.0],
        width_px: 0.0,
        color: va.color,
        edge_pos: [128, 128, 128, 128],
        dist: 0.0,
        z: (va.z + vb.z) * 0.5,
    };
    let idx = verts.len() as u32;
    verts.push(m);
    cache.insert(key, idx);
    idx
}

/// Shore distance (tile-local) for water vertices with no real shoreline in
/// their tile — open sea, or the interior of a body whose banks lie in other
/// tiles. 1.0 ⇒ a full tile from shore ⇒ unambiguously deep at every zoom.
const WATER_DEEP_DEFAULT: f32 = 1.0;

/// A water-polygon edge lying along the tile boundary is an MVT *clipping*
/// artefact, not a real shoreline; baking shore distance from it paints a false
/// shallow band at every tile seam (and makes all-water tiles read shallow).
/// Detect axis-aligned segments on — or just outside, allowing for the tile
/// buffer — the `[0, 1]` tile border.
fn is_tile_border_edge(a: [f32; 2], b: [f32; 2]) -> bool {
    const EPS: f32 = 0.02;
    let vertical = (a[0] - b[0]).abs() < EPS;
    let horizontal = (a[1] - b[1]).abs() < EPS;
    let x_border = a[0] <= EPS || a[0] >= 1.0 - EPS;
    let y_border = a[1] <= EPS || a[1] >= 1.0 - EPS;
    (vertical && x_border) || (horizontal && y_border)
}

/// Squared distance from point `p` to segment `a`→`b` (tile-local units).
fn point_seg_dist2(p: [f32; 2], a: [f32; 2], b: [f32; 2]) -> f32 {
    let ab = [b[0] - a[0], b[1] - a[1]];
    let ap = [p[0] - a[0], p[1] - a[1]];
    let len2 = ab[0] * ab[0] + ab[1] * ab[1];
    let t = if len2 > 0.0 {
        ((ap[0] * ab[0] + ap[1] * ab[1]) / len2).clamp(0.0, 1.0)
    } else {
        0.0
    };
    let c = [a[0] + t * ab[0], a[1] + t * ab[1]];
    let d = [p[0] - c[0], p[1] - c[1]];
    d[0] * d[0] + d[1] * d[1]
}

/// Bake each water vertex's distance to the nearest shoreline edge (tile-local
/// units) into its `dist` attribute. `segments` are the water polygons' boundary
/// edges, `[x0,y0,x1,y1]`. With no boundary (shouldn't happen for water) every
/// vertex stays at `dist = 0` (treated as shallow — the safe, brighter default).
fn bake_shore_distance(mesh: &mut Mesh, segments: &[[f32; 4]]) {
    if segments.is_empty() {
        // No real shoreline in this tile ⇒ deep everywhere (NOT the shallow
        // `dist = 0` default), so open water doesn't read as a bright shallows.
        for v in &mut mesh.vertices {
            v.dist = WATER_DEEP_DEFAULT;
        }
        return;
    }
    for v in &mut mesh.vertices {
        let p = v.base;
        let mut best = f32::MAX;
        for s in segments {
            let d2 = point_seg_dist2(p, [s[0], s[1]], [s[2], s[3]]);
            if d2 < best {
                best = d2;
                if best == 0.0 {
                    break;
                }
            }
        }
        v.dist = best.sqrt();
    }
}

/// Uniformly subdivide a triangle mesh (each triangle → 4 via shared edge
/// midpoints, so it stays watertight — no T-junction cracks) until another round
/// would exceed `max_verts`, or `max_rounds` is reached. Turns the minimal lyon
/// water fill into a fine grid the realistic-water path can displace.
pub fn refine_mesh(mut mesh: Mesh, max_verts: usize, max_rounds: u32) -> Mesh {
    for _ in 0..max_rounds {
        let tris = mesh.indices.len() / 3;
        // A round adds ≈ one vertex per unique edge (~1.5·tris); stop before the
        // budget is blown.
        if tris == 0 || mesh.vertices.len() + tris * 2 > max_verts {
            break;
        }
        let Mesh {
            mut vertices,
            indices,
        } = mesh;
        let mut out_idx: Vec<u32> = Vec::with_capacity(indices.len() * 4);
        let mut cache: std::collections::HashMap<(u32, u32), u32> = std::collections::HashMap::new();
        let mut i = 0;
        while i + 2 < indices.len() {
            let (a, b, c) = (indices[i], indices[i + 1], indices[i + 2]);
            let ab = edge_midpoint(&mut vertices, &mut cache, a, b);
            let bc = edge_midpoint(&mut vertices, &mut cache, b, c);
            let ca = edge_midpoint(&mut vertices, &mut cache, c, a);
            out_idx.extend_from_slice(&[a, ab, ca, ab, b, bc, ca, bc, c, ab, bc, ca]);
            i += 3;
        }
        mesh = Mesh {
            vertices,
            indices: out_idx,
        };
    }
    mesh
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
    // Water fills tessellate into their own buffer so the renderer can draw them
    // through the dedicated water pipeline. Everything else (incl. water outlines,
    // which are `Line` paint) stays in `buffers`.
    let mut water_buffers: VertexBuffers<VectorVertex, u32> = VertexBuffers::new();
    // Shoreline edge segments (tile-local coords) of every water polygon, used
    // after refinement to bake each water vertex's distance-to-shore into `dist`
    // — the shallowness cue the water shader turns into depth-based absorption.
    let mut water_boundary: Vec<[f32; 4]> = Vec::new();
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
                        // Route water-body fills into the dedicated water buffer
                        // (realistic-water pipeline); everything else into the
                        // ordinary vector mesh.
                        let is_water = is_water_source_layer(&layer.name);
                        if is_water {
                            // Collect this body's REAL shoreline edges (tile-local)
                            // for the post-refinement shore-distance bake, skipping
                            // tile-clip edges (see is_tile_border_edge).
                            let mut push_edge = |a: lyon::math::Point, b: lyon::math::Point| {
                                let pa = [a.x, a.y];
                                let pb = [b.x, b.y];
                                if !is_tile_border_edge(pa, pb) {
                                    water_boundary.push([pa[0], pa[1], pb[0], pb[1]]);
                                }
                            };
                            for ring in rings {
                                if ring.len() < 2 {
                                    continue;
                                }
                                let mut prev = project(tile_id, extent, ring[0]);
                                for &pt in &ring[1..] {
                                    let cur = project(tile_id, extent, pt);
                                    push_edge(prev, cur);
                                    prev = cur;
                                }
                                // Close the ring (last → first).
                                let first = project(tile_id, extent, ring[0]);
                                push_edge(prev, first);
                            }
                        }
                        let target = if is_water {
                            &mut water_buffers
                        } else {
                            &mut buffers
                        };
                        let mut builder =
                            BuffersBuilder::new(target, |v: FillVertex| VectorVertex {
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
                    min_height_property,
                } => {
                    if let Geometry::Polygon(rings) = &feature.geometry {
                        // Per-feature height (OMT `render_height`) when the
                        // property is present and numeric, else the default.
                        let height_m = height_property
                            .as_deref()
                            .and_then(|f| read_number_property(feature, f))
                            .map(|n| n as f32)
                            .unwrap_or(*height_m);
                        // Base height (OMT `render_min_height`) — walls start
                        // here so rooftop structures float; default 0.
                        let min_m = min_height_property
                            .as_deref()
                            .and_then(|f| read_number_property(feature, f))
                            .map(|n| n as f32)
                            .unwrap_or(0.0);
                        let h = meters_to_world_z(tile_id, height_m);
                        let base_z = meters_to_world_z(tile_id, min_m);
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
                        // Walls: a vertical quad per ring edge, ground → roof.
                        // Each face is flat-shaded by its compass orientation
                        // (sunlit vs shadowed), then a vertical ambient-
                        // occlusion gradient darkens the base — the GPU
                        // interpolates it up the wall, giving the soft contact
                        // shadow that grounds the building instead of leaving
                        // it a floating box.
                        for ring in rings {
                            let pts: Vec<[f32; 2]> = ring
                                .iter()
                                .map(|&p| project(tile_id, extent, p).to_array())
                                .collect();
                            for seg in pts.windows(2) {
                                let (a, b) = (seg[0], seg[1]);
                                let face = wall_shade(a, b);
                                // Top carries the face shade; the base is the
                                // same face darkened by the AO factor.
                                let wall_top = pack_color(shade(*color, face));
                                let wall_base = pack_color(shade(*color, face * WALL_AO_GROUND));
                                let v = |xy: [f32; 2], z: f32, color: [u8; 4]| VectorVertex {
                                    base: xy,
                                    normal: [0.0, 0.0],
                                    width_px: 0.0,
                                    color,
                                    edge_pos: [128, 0, 0, 0],
                                    dist: 0.0,
                                    z,
                                };
                                let base_i = buffers.vertices.len() as u32;
                                buffers.vertices.push(v(a, base_z, wall_base));
                                buffers.vertices.push(v(b, base_z, wall_base));
                                buffers.vertices.push(v(b, h, wall_top));
                                buffers.vertices.push(v(a, h, wall_top));
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
                    letter_spacing,
                    weight,
                } => {
                    // Text is optional: an icon-only layer (a bare POI
                    // marker) has no `text_field` value, but still draws.
                    let text = read_text_property(feature, text_field);
                    // Placement priority (lower wins, MapLibre sort-key). A
                    // configured rank property is used directly so OMT's
                    // `rank` (1 = most important) orders collisions by true
                    // importance. Unranked features in a ranked layer sort
                    // last (f32::MAX). With no rank property at all, fall back
                    // to `-font_size` so the bigger label wins.
                    let sort_key = match rank_field.as_deref() {
                        Some(field) => read_number_property(feature, field)
                            .map(|n| n as f32)
                            .unwrap_or(f32::MAX),
                        None => -*font_size_px,
                    };
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
                            sort_key,
                            path,
                            left_pad_px,
                            letter_spacing: *letter_spacing,
                            weight: *weight,
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

    // Refine the water fill into a fine grid so the realistic-water path has
    // vertices to displace (Gerstner). lyon emits minimal large interior
    // triangles; uniform 1→4 subdivision (shared midpoints ⇒ watertight, no
    // cracks) gives a dense grid, bounded by a per-tile vertex budget. The flat
    // water path renders the denser mesh identically (still a flat fill).
    let mut water_mesh = refine_mesh(
        Mesh {
            vertices: water_buffers.vertices,
            indices: water_buffers.indices,
        },
        WATER_GRID_VERT_BUDGET,
        WATER_GRID_MAX_ROUNDS,
    );
    // Bake distance-to-shore (tile-local units) into each water vertex's `dist`
    // so the shader can shade shallows (near an edge) brighter than deeps.
    bake_shore_distance(&mut water_mesh, &water_boundary);
    TessellationOutput {
        mesh: Mesh {
            vertices: buffers.vertices,
            indices: buffers.indices,
        },
        water_mesh,
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

/// Fraction of a wall face's brightness retained at ground level. The base
/// vertices are shaded this much darker than the top, and the GPU interpolates
/// the gradient up the wall — a cheap ambient-occlusion contact shadow.
const WALL_AO_GROUND: f32 = 0.62;

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

    fn fill_vertex(x: f32, y: f32) -> VectorVertex {
        VectorVertex {
            base: [x, y],
            normal: [0.0, 0.0],
            width_px: 0.0,
            color: [10, 30, 60, 255],
            edge_pos: [128, 128, 128, 128],
            dist: 0.0,
            z: 0.0,
        }
    }

    #[test]
    fn refine_mesh_subdivides_and_shares_midpoints() {
        // Two triangles sharing edge (1,2) — a unit quad. One round must add
        // exactly one midpoint per *unique* edge (5 edges ⇒ 5 new verts), and
        // turn 2 triangles into 8, with no vertex left unreferenced.
        let mesh = Mesh {
            vertices: vec![
                fill_vertex(0.0, 0.0),
                fill_vertex(1.0, 0.0),
                fill_vertex(0.0, 1.0),
                fill_vertex(1.0, 1.0),
            ],
            indices: vec![0, 1, 2, 1, 3, 2],
        };
        let refined = refine_mesh(mesh, 1_000, 1);
        assert_eq!(refined.indices.len(), 2 * 4 * 3, "each triangle → 4");
        // 4 corners + 5 shared edge midpoints (the diagonal 1↔2 is shared).
        assert_eq!(refined.vertices.len(), 9, "midpoints shared across the seam");
        for &i in &refined.indices {
            assert!((i as usize) < refined.vertices.len());
        }
        // The shared-edge midpoint is the quad centre, interpolated cleanly.
        assert!(refined
            .vertices
            .iter()
            .any(|v| (v.base[0] - 0.5).abs() < 1e-6 && (v.base[1] - 0.5).abs() < 1e-6));
    }

    #[test]
    fn bake_shore_distance_marks_edges_shallow_and_interior_deep() {
        // A unit-square water body; boundary = its 4 edges. A vertex on an edge
        // must get ~0 distance (shallow); the centre must get ~0.5 (deep).
        let segments = [
            [0.0, 0.0, 1.0, 0.0],
            [1.0, 0.0, 1.0, 1.0],
            [1.0, 1.0, 0.0, 1.0],
            [0.0, 1.0, 0.0, 0.0],
        ];
        let mut mesh = Mesh {
            vertices: vec![
                fill_vertex(0.0, 0.5), // on the left edge
                fill_vertex(0.5, 0.5), // centre
            ],
            indices: vec![],
        };
        bake_shore_distance(&mut mesh, &segments);
        assert!(mesh.vertices[0].dist < 1e-6, "edge vertex is shallow");
        assert!(
            (mesh.vertices[1].dist - 0.5).abs() < 1e-6,
            "centre is the deepest point (0.5 from every edge)"
        );
        // No real shoreline ⇒ deep default (open sea is deep, not shallow).
        let mut bare = Mesh {
            vertices: vec![fill_vertex(0.3, 0.7)],
            indices: vec![],
        };
        bake_shore_distance(&mut bare, &[]);
        assert_eq!(bare.vertices[0].dist, WATER_DEEP_DEFAULT);
    }

    #[test]
    fn tile_border_edges_are_not_shoreline() {
        // Clip edges along the tile border are rejected; a real interior shore is
        // kept.
        assert!(is_tile_border_edge([0.0, 0.0], [0.0, 1.0]), "left border");
        assert!(is_tile_border_edge([1.0, 0.0], [1.0, 1.0]), "right border");
        assert!(is_tile_border_edge([0.0, 1.0], [1.0, 1.0]), "top border");
        assert!(is_tile_border_edge([-0.05, 0.2], [-0.05, 0.8]), "buffer overhang");
        assert!(!is_tile_border_edge([0.3, 0.3], [0.7, 0.6]), "interior coastline");
        assert!(!is_tile_border_edge([0.5, 0.0], [0.6, 0.4]), "diagonal off the border");
    }

    #[test]
    fn refine_mesh_respects_vertex_budget() {
        let mesh = Mesh {
            vertices: vec![fill_vertex(0.0, 0.0), fill_vertex(1.0, 0.0), fill_vertex(0.0, 1.0)],
            indices: vec![0, 1, 2],
        };
        // Budget that admits zero rounds (3 + 1·2 = 5 > 4) leaves it untouched.
        let tight = refine_mesh(mesh.clone(), 4, 8);
        assert_eq!(tight.indices.len(), 3);
        // A generous budget but max_rounds=0 is also a no-op.
        let capped = refine_mesh(mesh, 100_000, 0);
        assert_eq!(capped.indices.len(), 3);
    }

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
                    min_height_property: None,
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

        // Ambient-occlusion gradient: every wall vertex at the ground (z=0)
        // must be darker than the brightest wall vertex at the top of the
        // same wall, so the contact shadow reads. Compare luminance of the
        // darkest base vertex against the brightest top vertex.
        let lum = |c: [u8; 4]| c[0] as u32 + c[1] as u32 + c[2] as u32;
        let wall_verts: Vec<_> = out
            .mesh
            .vertices
            .iter()
            .filter(|v| v.z == 0.0) // base vertices (roof is at z=h, not 0)
            .collect();
        assert!(!wall_verts.is_empty(), "walls have ground vertices");
        let darkest_base = wall_verts.iter().map(|v| lum(v.color)).min().unwrap();
        let top_lum = out
            .mesh
            .vertices
            .iter()
            .filter(|v| (v.z - h).abs() < 1e-9)
            .map(|v| lum(v.color))
            .max()
            .unwrap();
        assert!(
            darkest_base < top_lum,
            "wall base must be AO-darkened below the top: base {darkest_base} vs top {top_lum}",
        );
    }

    #[test]
    fn fill_extrusion_floats_on_a_min_height_base() {
        // A rooftop structure with render_min_height must extrude between its
        // base and top — no vertex touches the ground (z=0).
        let mut props = HashMap::new();
        props.insert("h".to_owned(), crate::vector::Value::Int(30));
        props.insert("base".to_owned(), crate::vector::Value::Int(20));
        let feat = Feature {
            id: 0,
            geom_type: GeomType::Polygon,
            geometry: Geometry::Polygon(vec![vec![
                (1000, 1000),
                (1200, 1000),
                (1200, 1200),
                (1000, 1200),
                (1000, 1000),
            ]]),
            properties: props,
        };
        let tile = VectorTile {
            layers: vec![crate::vector::Layer {
                name: "building".into(),
                version: 2,
                extent: 4096,
                features: vec![feat],
            }],
        };
        let style = VectorStyle {
            background: Color::rgb(255, 255, 255),
            rules: vec![Rule {
                source_layer: "building".into(),
                filter: Filter::Always,
                paint: Paint::FillExtrusion {
                    color: Color::rgb(200, 190, 180),
                    height_m: 5.0,
                    height_property: Some("h".into()),
                    min_height_property: Some("base".into()),
                },
                min_zoom: 0,
                max_zoom: 22,
                interactive: false,
            }],
        };
        let out = tessellate(TileId::new(14, 8434, 4722), &tile, &style);
        let lo = out.mesh.vertices.iter().map(|v| v.z).fold(f32::MAX, f32::min);
        let hi = out.mesh.vertices.iter().map(|v| v.z).fold(0.0f32, f32::max);
        assert!(lo > 0.0, "floating base sits above the ground, got {lo}");
        assert!(hi > lo, "top is above the base");
        // base 20 m : top 30 m ⇒ base is ~2/3 of the top height.
        assert!((lo / hi - 20.0 / 30.0).abs() < 1e-3, "base/top ratio");
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
                // Non-water fill → ordinary vector mesh. (Water fills are split
                // into `water_mesh`; that path is covered by
                // `water_fill_lands_in_the_water_mesh_not_the_main_mesh`.)
                name: "landcover".into(),
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
                source_layer: "landcover".into(),
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
    fn water_fill_lands_in_the_water_mesh_not_the_main_mesh() {
        // A "water" source-layer fill must be split into `water_mesh` (drawn by
        // the realistic-water pipeline) and produce no triangles in the ordinary
        // mesh; a non-water fill stays in `mesh`.
        let square = vec![vec![(100, 100), (200, 100), (200, 200), (100, 200), (100, 100)]];
        let fill_rule = |layer: &str| VectorStyle {
            background: Color::rgb(255, 255, 255),
            rules: vec![Rule {
                source_layer: layer.into(),
                filter: Filter::Always,
                paint: Paint::Fill { color: Color::rgb(0, 0, 255) },
                min_zoom: 0,
                max_zoom: 22,
                interactive: false,
            }],
        };
        let tile = |layer: &str| VectorTile {
            layers: vec![crate::vector::Layer {
                name: layer.into(),
                version: 2,
                extent: 4096,
                features: vec![poly(square.clone())],
            }],
        };

        let water = tessellate(TileId::new(5, 1, 1), &tile("water"), &fill_rule("water"));
        assert!(water.mesh.is_empty(), "water fill must not land in the main mesh");
        assert!(!water.water_mesh.is_empty(), "water fill must land in the water mesh");

        let land = tessellate(TileId::new(5, 1, 1), &tile("landcover"), &fill_rule("landcover"));
        assert!(!land.mesh.is_empty(), "non-water fill stays in the main mesh");
        assert!(land.water_mesh.is_empty(), "non-water fill must not land in the water mesh");
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
            letter_spacing: 0.0,
            weight: 0.0,
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
                    letter_spacing: 0.0,
                    weight: 0.0,
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
        // No rank property configured ⇒ sort-key falls back to -font_size, so
        // a bigger label still wins collisions (lower sort-key = placed first).
        assert!(
            (label.sort_key + 14.0).abs() < 1e-6,
            "unranked sort-key should be -font_size, got {}",
            label.sort_key,
        );
        // Middle of tile (512, 512) at z=10: world centre = (512.5/1024, 512.5/1024).
        assert!((label.world_pos.0 as f64 - 512.5 / 1024.0).abs() < 1e-6);
        assert!((label.world_pos.1 as f64 - 512.5 / 1024.0).abs() < 1e-6);
    }

    #[test]
    fn rank_property_becomes_the_sort_key_with_lower_winning() {
        // A configured `rank_field` is read straight into `sort_key` so OMT's
        // convention (rank 1 = most important) drives collisions: rank 3 here
        // must surface as sort_key 3, *below* a default-font label's -size.
        let mut props = HashMap::new();
        props.insert("name".to_owned(), crate::vector::Value::String("Oslo".into()));
        props.insert("rank".to_owned(), crate::vector::Value::Int(3));
        let feat = Feature {
            id: 0,
            geom_type: GeomType::Point,
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
                    rank_field: Some("rank".into()),
                    along_line: false,
                    icon: None,
                    left_anchor: false,
                    letter_spacing: 0.0,
                    weight: 0.0,
                },
                min_zoom: 0,
                max_zoom: 22,
                interactive: false,
            }],
        };
        let out = tessellate(TileId::new(10, 512, 512), &tile, &style);
        assert_eq!(out.labels.len(), 1);
        assert!(
            (out.labels[0].sort_key - 3.0).abs() < 1e-6,
            "rank 3 should become sort_key 3, got {}",
            out.labels[0].sort_key,
        );
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
                    letter_spacing: 0.0,
                    weight: 0.0,
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
