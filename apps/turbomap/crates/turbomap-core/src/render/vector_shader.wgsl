// Vector tile pipeline. Same world-to-clip transform as the raster
// pipeline — `Camera::view_projection_matrix` packed as a single
// mat4 so tilt + bearing flow through transparently.
struct CameraUniform {
    view_proj: mat4x4<f32>,
    params: vec4<f32>,  // .x = pixels per world unit
};

struct TileUniform {
    tile_alpha: f32,
    // >0.5 → ignore the baked vertex colour and use `paint_color` instead.
    // This is how zoom-interpolated / data-driven paint reaches the GPU
    // without re-tessellating: the host evaluates the paint per frame and
    // writes the result here. Vertex colour stays the fallback so the
    // baked multi-rule path is unchanged when no override is set.
    use_paint_color: f32,
    // Dash pattern in screen pixels: dash length then gap length. Both 0 ⇒
    // solid (no dashing). Occupies the 8..16 padding slot before paint_color.
    dash_len: f32,
    gap_len: f32,
    paint_color: vec4<f32>,
    // Tile placement: meshes are tessellated in tile-local units ([0,1]
    // across the tile) so f32 keeps full precision at any zoom; the vertex
    // shader places them with `origin + base * span`.
    origin: vec2<f32>,
    span: f32,
    _pad2: f32,
};

@group(0) @binding(0) var<uniform> camera: CameraUniform;
@group(1) @binding(0) var<uniform> tile: TileUniform;

struct VertexInput {
    @location(0) base: vec2<f32>,    // tile-local centerline ([0,1] across the tile)
    @location(1) normal: vec2<f32>,  // unit normal (0 for fills)
    @location(2) width_px: f32,      // screen px (0 for fills)
    @location(3) color: vec4<f32>,   // 8-bit sRGB, fed as Unorm
    // Cross-line position used for AA. .x: 0.0 at one stroke edge, 1.0 at
    // the other, ~0.5 for fills (no AA). Other components unused.
    @location(4) edge_pos: vec4<f32>,
    @location(5) dist: f32,          // world-space arc length along the path
    @location(6) z: f32,             // world height above ground (0 = flat)
};

struct VertexOutput {
    @builtin(position) clip_position: vec4<f32>,
    @location(0) color: vec4<f32>,
    @location(1) edge_pos: f32,
    // Arc length in *screen pixels* (world dist × pixels-per-world), so the
    // dash pattern stays a constant pixel size at every zoom.
    @location(2) dist_px: f32,
};

@vertex
fn vs_main(in: VertexInput) -> VertexOutput {
    var out: VertexOutput;
    // Place the tile-local mesh in world space, then extrude the
    // centerline by the normal to a half-width that is a constant number
    // of *screen* pixels — so a road stays N px wide at every zoom.
    // width_px 0 (fills) leaves the position untouched.
    let half_width_world = (in.width_px * 0.5) / camera.params.x;
    let world = tile.origin + in.base * tile.span + in.normal * half_width_world;
    // `z` is the world height for extruded geometry; 0 for flat features,
    // so they sit on the ground plane exactly as before.
    out.clip_position = camera.view_proj * vec4<f32>(world, in.z, 1.0);
    out.color = in.color;
    out.edge_pos = in.edge_pos.x;
    // Tile-unit arc length → screen px (span × pixels-per-world).
    out.dist_px = in.dist * tile.span * camera.params.x;
    return out;
}

@fragment
fn fs_main(in: VertexOutput) -> @location(0) vec4<f32> {
    // Distance from the stroke centerline, in [0, 1]. Fills have
    // edge_pos≈0.5 → dist≈0, no AA applied.
    let dist = abs(in.edge_pos - 0.5) * 2.0;
    // One screen pixel's worth of edge taper, computed from the screen-
    // space derivative of `dist`. fwidth = |dFdx| + |dFdy|.
    let fade = fwidth(dist);
    let edge_alpha = 1.0 - smoothstep(1.0 - fade, 1.0, dist);
    var base = in.color;
    if (tile.use_paint_color > 0.5) {
        base = tile.paint_color;
    }

    // Dashing: drop fragments that fall in the gap of the dash period. The
    // period is `dash_len + gap_len` screen px; phase within it past
    // `dash_len` is a gap. An anti-aliased edge over one pixel keeps the
    // dash ends from shimmering. `dash_len <= 0` ⇒ solid.
    var dash_alpha = 1.0;
    let period = tile.dash_len + tile.gap_len;
    if (tile.dash_len > 0.0 && period > 0.0) {
        let phase = in.dist_px - floor(in.dist_px / period) * period;
        let aa = fwidth(in.dist_px);
        // 1 inside the dash, 0 in the gap, smooth over one pixel at each end.
        dash_alpha = smoothstep(-aa, aa, phase)
            * (1.0 - smoothstep(tile.dash_len - aa, tile.dash_len + aa, phase));
    }

    return vec4<f32>(base.rgb, base.a * edge_alpha * dash_alpha * tile.tile_alpha);
}
