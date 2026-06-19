// Camera uniform: the 4×4 view-projection matrix produced by
// `Camera::view_projection_matrix`. Applied to world-space
// `(x, y, z, 1)` it yields clip space. Tilt + bearing are encoded
// inside the matrix; at pitch=bearing=0 + flat terrain the matrix
// collapses to the legacy 2D centre+scale mapping.
struct CameraUniform {
    view_proj: mat4x4<f32>,
};

// Terrain + lighting configuration the host writes per frame. When
// `meters_to_world == 0` the displacement collapses to zero and the
// mesh stays flat at z=0 (legacy behaviour, no breakage). When
// `terrain_lit == 0` the fragment shader skips sun-shading + haze and
// just samples the basemap texture (the flat 2D era look).
struct Globals {
    halo_uv: f32,
    meters_to_world: f32,
    exaggeration: f32,
    encoding: u32,
    // Unit direction towards the sun, engine world frame (x=E, y=S,
    // z=up). Drives the Lambertian relief shading.
    sun_dir: vec3<f32>,
    // Ambient (sky-fill) term in [0,1]: the darkest a fully self-
    // shadowed slope gets. Keeps shadowed faces readable, not black.
    ambient: f32,
    // Atmosphere colour distant terrain fades toward (aerial
    // perspective) — the same hue as the horizon sky.
    haze_color: vec3<f32>,
    // Pre-scaled aerial-perspective density: `1 - exp(-dist·density)`.
    // Folded CPU-side with `1/altitude` (zoom-stable) and a pitch ramp
    // (0 when top-down, so the flat 2D map carries no haze).
    haze_density: f32,
    // Sunlight colour — warm near sunrise/sunset, neutral at midday.
    // Multiplies the lit basemap so the whole scene shares the sky's
    // time-of-day tint.
    light_color: vec3<f32>,
    // 1 = apply sun-shading + haze (3D terrain), 0 = flat texture only.
    terrain_lit: f32,
};

@group(0) @binding(0) var<uniform> camera: CameraUniform;
@group(0) @binding(1) var<uniform> globals: Globals;
@group(1) @binding(0) var tile_tex: texture_2d<f32>;
@group(1) @binding(1) var tile_sampler: sampler;
@group(2) @binding(0) var dem_tex: texture_2d<f32>;
@group(2) @binding(1) var dem_samp: sampler;

fn decode_elevation(rgb: vec3<f32>) -> f32 {
    let r = rgb.r * 255.0;
    let g = rgb.g * 255.0;
    let b = rgb.b * 255.0;
    if (globals.encoding == 1u) {
        return r * 256.0 + g + b / 256.0 - 32768.0;  // Terrarium
    } else {
        return -10000.0 + (r * 256.0 * 256.0 + g * 256.0 + b) * 0.1;
    }
}

// Decode the DEM at a UV, returning 0 where the sample is "no data"
// (alpha < 0.5) so water / out-of-coverage stays at sea level instead
// of cliffing to -10 km.
fn elev_at(uv: vec2<f32>) -> f32 {
    let s = textureSampleLevel(dem_tex, dem_samp, uv, 0.0);
    if (s.a < 0.5) {
        return 0.0;
    }
    return decode_elevation(s.rgb);
}

struct VertexInput {
    // Unit-quad corner, in [0, 1].
    @location(0) corner: vec2<f32>,
};

struct InstanceInput {
    // Where this tile lives in world space.
    @location(1) world_origin: vec2<f32>,
    @location(2) world_size: f32,
    // Where this tile samples from in the bound basemap texture (for
    // parent fallback: sample a sub-rect of an ancestor's texture).
    @location(3) uv_origin: vec2<f32>,
    @location(4) uv_size: f32,
    // Per-instance alpha multiplier — used to fade newly-loaded
    // tiles in over their parent-fallback ancestor.
    @location(5) alpha: f32,
    // Sub-UV into the bound DEM texture. When the DEM is the basemap's
    // own tile, this is `(halo_uv, halo_uv, 1 - 2*halo_uv)`. When the
    // DEM is an ancestor, this narrows the sampled window to the
    // basemap tile's slice of the ancestor's coverage.
    @location(6) dem_uv_origin: vec2<f32>,
    @location(7) dem_uv_size: f32,
};

struct VertexOutput {
    @builtin(position) clip_position: vec4<f32>,
    @location(0) tex_uv: vec2<f32>,
    @location(1) alpha: f32,
    // World-space surface normal from the DEM gradient (for sun
    // shading). (0,0,1) when terrain is flat.
    @location(2) normal: vec3<f32>,
    // `1 - exp(-dist·density)` aerial-perspective blend factor, 0..1.
    @location(3) haze: f32,
};

@vertex
fn vs_main(in: VertexInput, inst: InstanceInput) -> VertexOutput {
    let world = inst.world_origin + in.corner * inst.world_size;

    // Sample the DEM at this vertex using the per-instance sub-UV.
    // `dem_uv_origin + corner * dem_uv_size` lands within the bound
    // DEM tile's non-halo interior. When the bind is the basemap's
    // own tile, the sub-UV spans the full interior; when the bind
    // is an ancestor, only the basemap's slice of it.
    let dem_uv = inst.dem_uv_origin + in.corner * inst.dem_uv_size;
    let dem = textureSampleLevel(dem_tex, dem_samp, dem_uv, 0.0);
    var elev_m: f32 = 0.0;
    if (dem.a >= 0.5) {
        elev_m = decode_elevation(dem.rgb);
    }
    let zscale = globals.meters_to_world * globals.exaggeration;
    let world_z = elev_m * zscale;

    var out: VertexOutput;
    out.clip_position = camera.view_proj * vec4<f32>(world, world_z, 1.0);
    out.tex_uv = inst.uv_origin + in.corner * inst.uv_size;
    out.alpha = inst.alpha;

    // Surface normal from the DEM gradient, sampled one mesh-cell to
    // each side (GRID=16 → cell = 1/16 of the tile). The horizontal
    // step in world units is `cell·world_size`; the vertical is the
    // decoded-elevation delta scaled to world z. cross(tangent_x,
    // tangent_y) ∝ (-dz/dx, -dz/dy, 1).
    if (globals.terrain_lit > 0.5 && dem.a >= 0.5) {
        let cell = 1.0 / 16.0;
        let euv = inst.dem_uv_size * cell;
        let ew = inst.world_size * cell;
        let hx = (elev_at(dem_uv + vec2<f32>(euv, 0.0)) - elev_at(dem_uv - vec2<f32>(euv, 0.0))) * zscale;
        let hy = (elev_at(dem_uv + vec2<f32>(0.0, euv)) - elev_at(dem_uv - vec2<f32>(0.0, euv))) * zscale;
        out.normal = normalize(vec3<f32>(-hx, -hy, 2.0 * ew));
    } else {
        out.normal = vec3<f32>(0.0, 0.0, 1.0);
    }

    // Aerial perspective: distance from the camera centre (world coords
    // are relative-to-centre, so the camera sits at the origin in xy).
    // `haze_density` already folds in 1/altitude (zoom-stable) and a
    // pitch ramp, so this is 0 on the flat 2D map.
    let dist = length(world);
    out.haze = 1.0 - exp(-dist * globals.haze_density);
    return out;
}

@fragment
fn fs_main(in: VertexOutput) -> @location(0) vec4<f32> {
    let s = textureSample(tile_tex, tile_sampler, in.tex_uv);

    // Flat 2D era (no terrain): straight texture, no lighting/haze.
    if (globals.terrain_lit < 0.5) {
        return vec4<f32>(s.rgb, s.a * in.alpha);
    }

    // Lambertian relief shading. The ambient floor keeps shadowed
    // faces readable; the sunlit term carries the sky's warm tint.
    let n = normalize(in.normal);
    let ndl = clamp(dot(n, globals.sun_dir), 0.0, 1.0);
    let light = globals.ambient + (1.0 - globals.ambient) * ndl;
    var rgb = s.rgb * light * globals.light_color;

    // Aerial perspective: fade distant relief toward the atmosphere
    // colour for depth + a believable horizon.
    rgb = mix(rgb, globals.haze_color, clamp(in.haze, 0.0, 1.0));

    return vec4<f32>(rgb, s.a * in.alpha);
}
