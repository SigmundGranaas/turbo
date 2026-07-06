// Hillshade pipeline. One tile = one textured quad in world space; the
// fragment shader samples the decoded-elevation texture (Rg16Float:
// .r = metres, .g = coverage — the ingest codec already ran), takes
// one-texel differences to get the gradient, and mixes between shadow +
// highlight colours based on the slope/aspect lighting formula.

struct CameraUniform {
    // World → clip 4×4 — see raster `shader.wgsl` for the rationale.
    view_proj: mat4x4<f32>,
};

// Per-pass globals: lighting.
//   sun_dir: pre-computed direction in xy (azimuth) + z (altitude)
struct Globals {
    sun_dir: vec3<f32>,
    exaggeration: f32,
    shadow_color: vec4<f32>,
    highlight_color: vec4<f32>,
    opacity: f32,
    // Spare slot (held the DEM-encoding tag until the decode moved to
    // ingest; kept so the std140 layout is untouched).
    _pad0: u32,
    // Fractional UV inset that maps the displayed tile to the
    // texture's interior, so the gradient kernel at the edge can
    // step into the halo ring instead of clamping. 0 = no halo.
    halo_uv: f32,
    // Conversion factor from metres of elevation to world units (one
    // world unit = full Mercator extent). Multiplied into the per-
    // vertex height to give actual 3D displacement. CPU-side picks
    // it from the tile's mid-latitude. When zero (no terrain), the
    // mesh degenerates to a flat tile at z=0.
    meters_to_world: f32,
};

@group(0) @binding(0) var<uniform> camera: CameraUniform;
@group(0) @binding(1) var<uniform> globals: Globals;
@group(1) @binding(0) var dem_tex: texture_2d<f32>;
@group(1) @binding(1) var dem_samp: sampler;

struct VertexInput {
    @location(0) corner: vec2<f32>, // unit-quad corner [0, 1]
};

struct InstanceInput {
    @location(1) world_origin: vec2<f32>,
    @location(2) world_size: f32,
    @location(3) alpha: f32,
};

struct VertexOutput {
    @builtin(position) clip_position: vec4<f32>,
    @location(0) uv: vec2<f32>,
    @location(1) alpha: f32,
};

@vertex
fn vs_main(in: VertexInput, inst: InstanceInput) -> VertexOutput {
    let world = inst.world_origin + in.corner * inst.world_size;
    // Same UV inset as before — sample the texture's non-halo
    // interior so adjacent tiles agree on edge heights.
    let lo = vec2<f32>(globals.halo_uv);
    let hi = vec2<f32>(1.0 - globals.halo_uv);
    let uv = mix(lo, hi, in.corner);

    // Sample the DEM at this vertex's UV and displace world z by the
    // elevation (.r is metres — decoded at ingest; no-data is already
    // resolved to sea level). `meters_to_world == 0` (no terrain
    // registered) leaves the mesh flat at z=0 — same look as the 2D era.
    let elev_m = textureSampleLevel(dem_tex, dem_samp, uv, 0.0).r;
    let world_z = elev_m * globals.meters_to_world * globals.exaggeration;

    var out: VertexOutput;
    out.clip_position = camera.view_proj * vec4<f32>(world, world_z, 1.0);
    out.uv = uv;
    out.alpha = inst.alpha;
    return out;
}

@fragment
fn fs_main(in: VertexOutput) -> @location(0) vec4<f32> {
    // Sample the DEM at the current fragment and at one-pixel neighbours.
    // Texel offsets come from the texture dimensions so this is correct
    // at any resolution.
    let dims = vec2<f32>(textureDimensions(dem_tex, 0));
    let texel = 1.0 / dims;

    // The source marks "no data" over water / outside DTM coverage; the
    // ingest codec resolves those heights to 0 m and records the mask in
    // the .g (coverage) channel, filterable exactly like the old alpha.
    //   1. Centre pixel nodata → output fully transparent so the
    //      basemap shows through (no shaded ocean).
    //   2. Neighbour pixel nodata → reuse the centre's elevation in
    //      the gradient kernel so the slope doesn't cliff across the
    //      water line.
    let centre = textureSample(dem_tex, dem_samp, in.uv);
    if (centre.g < 0.5) {
        return vec4<f32>(0.0, 0.0, 0.0, 0.0);
    }
    let h00 = centre.r;

    let sx_pos = textureSample(dem_tex, dem_samp, in.uv + vec2<f32>(texel.x, 0.0));
    let sx_neg = textureSample(dem_tex, dem_samp, in.uv - vec2<f32>(texel.x, 0.0));
    let sy_pos = textureSample(dem_tex, dem_samp, in.uv + vec2<f32>(0.0, texel.y));
    let sy_neg = textureSample(dem_tex, dem_samp, in.uv - vec2<f32>(0.0, texel.y));
    let hx_pos = select(h00, sx_pos.r, sx_pos.g >= 0.5);
    let hx_neg = select(h00, sx_neg.r, sx_neg.g >= 0.5);
    let hy_pos = select(h00, sy_pos.r, sy_pos.g >= 0.5);
    let hy_neg = select(h00, sy_neg.r, sy_neg.g >= 0.5);

    // Gradient in metres per texel, exaggerated.
    let dzdx = (hx_pos - hx_neg) * 0.5 * globals.exaggeration;
    let dzdy = (hy_pos - hy_neg) * 0.5 * globals.exaggeration;

    // Surface normal (in some abstract "metres per texel" units; we only
    // care about direction, so we can normalise).
    let normal = normalize(vec3<f32>(-dzdx, -dzdy, 1.0));

    // Lambertian dot product with the sun. Clamp so deep self-shadow
    // gets full shadow weight, not negative values.
    let intensity = clamp(dot(normal, globals.sun_dir), 0.0, 1.0);

    let color = mix(globals.shadow_color.rgb, globals.highlight_color.rgb, intensity);
    return vec4<f32>(color, globals.opacity * in.alpha);
}
