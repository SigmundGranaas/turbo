// Vector tile pipeline. Same world-to-clip transform as the raster
// pipeline — `Camera::view_projection_matrix` packed as a single
// mat4 so tilt + bearing flow through transparently.
struct CameraUniform {
    view_proj: mat4x4<f32>,
    // .x = pixels per world unit.
    // .y = meters_to_world · exaggeration (0 ⇒ no 3D terrain → no
    //      displacement, so the 2D map is byte-identical).
    // .z = DEM encoding (0 = Mapbox-RGB, 1 = Terrarium).
    // .w = halo_uv: fractional inset to the DEM tile's non-halo interior.
    params: vec4<f32>,
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
    // Per-frame multiplier on every line's baked screen width, so road
    // widths grow/shrink smoothly with zoom without re-tessellating. The
    // host evaluates a zoom curve per layer and writes it here; 1.0 (the
    // default) leaves widths exactly as baked. Fills/text/extrusions carry
    // width_px = 0, so the scale is a no-op for them.
    width_scale: f32,
    // DEM draping sub-UV: maps this tile's local base position into the
    // sub-rectangle of `dem_tex` that covers it. When the exact DEM tile is
    // resident these are (halo_uv, halo_uv) / (1 - 2·halo_uv) — the tile's own
    // non-halo interior. When only a shallower ancestor is cached (deep zoom),
    // the host narrows them to the matching quadrant so the line still drapes.
    // (0,0)/1 with the 1×1 placeholder ⇒ flat (no terrain resident yet).
    dem_uv_origin: vec2<f32>,
    dem_uv_size: f32,
};

@group(0) @binding(0) var<uniform> camera: CameraUniform;
@group(1) @binding(0) var<uniform> tile: TileUniform;
// DEM for THIS tile (or a 1×1 zero-elevation placeholder when no terrain /
// the exact tile isn't resident) so lines + fills drape onto the 3D
// terrain instead of floating at z=0.
@group(2) @binding(0) var dem_tex: texture_2d<f32>;
@group(2) @binding(1) var dem_samp: sampler;

fn decode_elevation(enc: u32, rgb: vec3<f32>) -> f32 {
    let r = rgb.r * 255.0;
    let g = rgb.g * 255.0;
    let b = rgb.b * 255.0;
    if (enc == 1u) {
        return r * 256.0 + g + b / 256.0 - 32768.0;  // Terrarium
    } else {
        return -10000.0 + (r * 256.0 * 256.0 + g * 256.0 + b) * 0.1;
    }
}

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
    let half_width_world = (in.width_px * tile.width_scale * 0.5) / camera.params.x;
    let world = tile.origin + in.base * tile.span + in.normal * half_width_world;
    // `z` is the world height for extruded geometry; 0 for flat features.
    // Drape onto the 3D terrain: sample THIS tile's DEM at the centerline
    // position and add the ground elevation, so lines/fills follow the
    // relief. `params.y == 0` (2D, no terrain) skips it → unchanged.
    var wz = in.z;
    let zscale = camera.params.y;
    if (zscale > 0.0) {
        // Sample the DEM (the tile's own, or an ancestor's matching quadrant)
        // via the per-tile sub-UV the host resolved, so lines drape at any zoom
        // — not just when the exact DEM tile is loaded.
        let dem_uv = tile.dem_uv_origin + in.base * tile.dem_uv_size;
        let s = textureSampleLevel(dem_tex, dem_samp, dem_uv, 0.0);
        if (s.a >= 0.5) {
            wz = wz + decode_elevation(u32(camera.params.z), s.rgb) * zscale;
        }
    }
    out.clip_position = camera.view_proj * vec4<f32>(world, wz, 1.0);
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
