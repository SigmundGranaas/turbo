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
    // Padding to align the following vec4 to 16 bytes.
    _pad: vec2<f32>,
    paint_color: vec4<f32>,
};

@group(0) @binding(0) var<uniform> camera: CameraUniform;
@group(1) @binding(0) var<uniform> tile: TileUniform;

struct VertexInput {
    @location(0) base: vec2<f32>,    // world centerline
    @location(1) normal: vec2<f32>,  // unit world normal (0 for fills)
    @location(2) width_px: f32,      // screen px (0 for fills)
    @location(3) color: vec4<f32>,   // 8-bit sRGB, fed as Unorm
    // Cross-line position used for AA. .x: 0.0 at one stroke edge, 1.0 at
    // the other, ~0.5 for fills (no AA). Other components unused.
    @location(4) edge_pos: vec4<f32>,
};

struct VertexOutput {
    @builtin(position) clip_position: vec4<f32>,
    @location(0) color: vec4<f32>,
    @location(1) edge_pos: f32,
};

@vertex
fn vs_main(in: VertexInput) -> VertexOutput {
    var out: VertexOutput;
    // Extrude the centerline by the normal to a half-width that is a
    // constant number of *screen* pixels — so a road stays N px wide at
    // every zoom. width_px 0 (fills) leaves the position untouched.
    let half_width_world = (in.width_px * 0.5) / camera.params.x;
    let world = in.base + in.normal * half_width_world;
    out.clip_position = camera.view_proj * vec4<f32>(world, 0.0, 1.0);
    out.color = in.color;
    out.edge_pos = in.edge_pos.x;
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
    return vec4<f32>(base.rgb, base.a * edge_alpha * tile.tile_alpha);
}
