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
    // --- Terrain cast shadows (CPU horizon-march, see render/shadow.rs) ---
    // Maps a fragment's world-xy into the shadow grid's [0,1] UV:
    //   uv = (world.xy - shadow_origin) * shadow_inv_size
    // shadow_origin is in the camera-relative (RTC) frame the vertex shader
    // emits, so the fragment needs no extra camera math.
    shadow_origin: vec2<f32>,
    shadow_inv_size: f32,
    // 0 disables cast shadows entirely; > 0 blends the per-fragment sun-march
    // result into the direct sun term by this strength.
    shadow_strength: f32,
    // World-xy size of one heightfield texel — the per-fragment march step.
    shadow_texel_world: f32,
    // World-z band over which an occluder fades the shadow in (penumbra).
    shadow_softness: f32,
    _shadow_pad: vec2<f32>,
};

@group(0) @binding(0) var<uniform> camera: CameraUniform;
@group(0) @binding(1) var<uniform> globals: Globals;
@group(1) @binding(0) var tile_tex: texture_2d<f32>;
@group(1) @binding(1) var tile_sampler: sampler;
@group(2) @binding(0) var dem_tex: texture_2d<f32>;
@group(2) @binding(1) var dem_samp: sampler;
// Frame-global terrain HEIGHTFIELD (world-z elevation, R32Float), assembled
// across tiles on the CPU and centred on the camera. The fragment shader marches
// it toward the sun per-pixel every frame to cast shadows — this is what lets a
// peak in one tile shadow a valley in another (the per-tile DEM at group 2 can't
// reach across), and computing it per-fragment makes the shadow sharp + stable
// under pan (no precomputed low-res visibility texture to go stale).
@group(3) @binding(0) var height_tex: texture_2d<f32>;
@group(3) @binding(1) var height_samp: sampler;

// Cast-shadow march length, in heightfield texels. Longer = shadows reach
// further (low sun) at more cost; each step is one `shadow_texel_world`.
const SHADOW_STEPS: i32 = 64;

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
    // World-xy in the camera-relative (RTC) frame — used by the fragment
    // shader to sample the cast-shadow grid.
    @location(4) world_xy: vec2<f32>,
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
    out.world_xy = world;
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

    // Cast shadows: march the terrain heightfield from this fragment toward the
    // sun. If any upstream cell pokes above the grazing ray, this fragment is
    // occluded. Done per-pixel every frame → sharp edges + stable under pan (no
    // precomputed low-res visibility texture). `occ` ∈ [0, strength]: a shadowed
    // fragment loses the direct sun term AND some ambient skylight (AO), so it
    // reads as a clear dark shadow, never fully black.
    var occ = 0.0;
    if (globals.shadow_strength > 0.0) {
        let sxy = globals.sun_dir.xy;
        let lxy = length(sxy);
        // No meaningful cast shadows with the sun at/below the horizon or at the
        // zenith — and only march where the heightfield actually covers us.
        if (lxy > 1.0e-4 && globals.sun_dir.z > 1.0e-3) {
            let suv0 = (in.world_xy - globals.shadow_origin) * globals.shadow_inv_size;
            if (suv0.x >= 0.0 && suv0.y >= 0.0 && suv0.x <= 1.0 && suv0.y <= 1.0) {
                let dir = sxy / lxy;
                let tan_alt = globals.sun_dir.z / lxy;        // world-z rise per world-xy
                let step = globals.shadow_texel_world;
                let rise = tan_alt * step;                    // rise per march step
                let h0 = textureSampleLevel(height_tex, height_samp, suv0, 0.0).r;
                var over = 0.0;
                var p = in.world_xy;
                for (var k = 1; k <= SHADOW_STEPS; k = k + 1) {
                    p = p + dir * step;
                    let uv = (p - globals.shadow_origin) * globals.shadow_inv_size;
                    if (uv.x < 0.0 || uv.y < 0.0 || uv.x > 1.0 || uv.y > 1.0) {
                        break;
                    }
                    let ray_z = h0 + f32(k) * rise;
                    let hz = textureSampleLevel(height_tex, height_samp, uv, 0.0).r;
                    let excess = hz - ray_z;
                    if (excess > over) {
                        over = excess;
                    }
                }
                let soft = max(globals.shadow_softness, 1.0e-7);
                occ = clamp(over / soft, 0.0, 1.0) * globals.shadow_strength;
            }
        }
    }
    let direct = (1.0 - globals.ambient) * ndl * (1.0 - occ);
    let ambient_lit = globals.ambient * (1.0 - 0.5 * occ);
    let light = ambient_lit + direct;
    var rgb = s.rgb * light * globals.light_color;

    // Aerial perspective: fade distant relief toward the atmosphere colour for
    // depth + a believable horizon. Capped well below 1 so even the farthest
    // ground keeps some terrain showing through — the haze never fully erases
    // the map to a flat white wash (the steep-tilt white-out).
    rgb = mix(rgb, globals.haze_color, min(in.haze, 0.75));

    return vec4<f32>(rgb, s.a * in.alpha);
}
