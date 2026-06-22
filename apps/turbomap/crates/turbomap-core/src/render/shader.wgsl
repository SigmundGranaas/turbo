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
    // Camera eye in the same RTC frame the vertex shader emits — haze is the
    // extinction over the TRUE eye→fragment distance, so near ground stays
    // clear at any pitch (distance-from-look-at whites out at grazing angles).
    eye_world: vec3<f32>,
    // Earth-curvature drop coefficient (π·cos³φ, 0 = flat). world_z is lowered
    // by `curvature_coeff · dot(world_xy, world_xy)` so distant terrain bends
    // away over the horizon instead of standing on a flat disc.
    curvature_coeff: f32,
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

// Heightfield resolution — must match `render::shadow::HEIGHT_DIM`. The march
// bilinearly interpolates the field (the texture is R32Float / unfiltered, so we
// do it by hand): nearest sampling leaves each texel a flat plateau whose edges
// are tiny vertical steps, and the per-pixel march turns those into a corduroy
// of shadow stripes. Bilinear smooths the relief so only real ridges occlude.
const HEIGHT_DIM_I: i32 = 192;
const HEIGHT_DIM_F: f32 = 192.0;

fn height_bilinear(uv: vec2<f32>) -> f32 {
    let t = uv * HEIGHT_DIM_F - vec2<f32>(0.5);
    let base = floor(t);
    let f = t - base;
    let x0 = clamp(i32(base.x), 0, HEIGHT_DIM_I - 1);
    let y0 = clamp(i32(base.y), 0, HEIGHT_DIM_I - 1);
    let x1 = min(x0 + 1, HEIGHT_DIM_I - 1);
    let y1 = min(y0 + 1, HEIGHT_DIM_I - 1);
    let h00 = textureLoad(height_tex, vec2<i32>(x0, y0), 0).r;
    let h10 = textureLoad(height_tex, vec2<i32>(x1, y0), 0).r;
    let h01 = textureLoad(height_tex, vec2<i32>(x0, y1), 0).r;
    let h11 = textureLoad(height_tex, vec2<i32>(x1, y1), 0).r;
    return mix(mix(h00, h10, f.x), mix(h01, h11, f.x), f.y);
}

// Cast-shadow march length, in heightfield texels. Longer = shadows reach
// further (low sun) at more cost; each step is one `shadow_texel_world`.
const SHADOW_STEPS: i32 = 48;

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
    // Skirt depth as a fraction of tile world-size (0 for grid verts). The
    // perimeter curtain verts share an edge vertex's xy + UV but hang down in
    // world-z by this fraction, covering mixed-LOD T-junction cracks.
    @location(8) skirt: f32,
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
    // Displaced world-z (terrain height) — used by the fragment shader for
    // height-based valley fog (mist pools in the low ground).
    @location(5) world_z: f32,
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
    // Skirt verts hang straight down from the displaced surface by a fraction
    // of the tile's world size — a vertical curtain backing mixed-LOD cracks.
    // Earth-curvature droop: distant ground bends below the tangent plane by
    // `curvature_coeff · s²` (s = horizontal distance from the camera centre,
    // which is the RTC origin, so `dot(world, world)`).
    // Skirt: drop a fixed RELIEF depth (≈300 m in world-z), NOT a fraction of the
    // tile's world size. `0.5·world_size` is fine for a fine tile but is hundreds
    // of km for a coarse far tile — giant curtains that flicker as the LOD set
    // shifts. A bounded relief covers the mixed-LOD seam cracks at every level.
    let skirt_drop = in.skirt * 600.0 * zscale;
    let world_z = elev_m * zscale
        - skirt_drop
        - globals.curvature_coeff * dot(world, world);

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

    // Aerial perspective: extinction over the TRUE distance from the camera
    // eye to this vertex (both in the relative-to-centre frame). Using the
    // eye — not the look-at point — keeps the near ground clear at grazing
    // pitch instead of washing the whole frame to the horizon colour.
    // `haze_density` is a per-world-unit coefficient (physical per-metre,
    // converted CPU-side) gated by a pitch ramp, so it's 0 on the flat 2D map.
    let dist = length(vec3<f32>(world, world_z) - globals.eye_world);
    out.haze = 1.0 - exp(-dist * globals.haze_density);
    out.world_xy = world;
    out.world_z = world_z;
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
                let h0 = height_bilinear(suv0);
                var over = 0.0;
                var hit_k = 0.0;
                var p = in.world_xy;
                for (var k = 1; k <= SHADOW_STEPS; k = k + 1) {
                    p = p + dir * step;
                    let uv = (p - globals.shadow_origin) * globals.shadow_inv_size;
                    if (uv.x < 0.0 || uv.y < 0.0 || uv.x > 1.0 || uv.y > 1.0) {
                        break;
                    }
                    let ray_z = h0 + f32(k) * rise;
                    let hz = height_bilinear(uv);
                    let excess = hz - ray_z;
                    if (excess > over) {
                        over = excess;
                        hit_k = f32(k);
                    }
                }
                // Contact hardening: the penumbra widens with the occluder's
                // distance (a far ridge throws a soft edge, a nearby lip a crisp
                // one) — the single biggest cue that makes terrain shadows read as
                // real rather than a hard stencil. `smoothstep` gives a gentle
                // toe/shoulder instead of a linear ramp.
                let soft = max(globals.shadow_softness, 1.0e-7)
                    * (1.0 + 2.5 * hit_k / f32(SHADOW_STEPS));
                occ = smoothstep(0.0, soft, over) * globals.shadow_strength;
            }
        }
    }
    // Split light into warm direct sun and cool sky-fill. A cast shadow removes
    // the direct sun entirely but keeps most of the skylight (open terrain isn't a
    // sealed crevice — the blue sky still lights it), so shadows stay luminous and
    // cool rather than a crushed flat-grey wash. The sky tint is a subtle pull
    // toward the horizon hue.
    // Gate the direct sun by the sun being above the horizon — without this the
    // N·L term keeps lighting slopes that face a just-below-horizon sun, so night
    // never goes properly dark. Fades over the horizon for a smooth dusk.
    let sun_up = smoothstep(-0.04, 0.04, globals.sun_dir.z);
    let direct = (1.0 - globals.ambient) * ndl * (1.0 - occ) * sun_up;
    let ambient_lit = globals.ambient * (1.0 - 0.2 * occ);
    let light = ambient_lit + direct;
    var rgb = s.rgb * light * globals.light_color;
    if (occ > 0.0) {
        let sky_tint = mix(vec3<f32>(1.0), globals.haze_color, 0.5);
        rgb = mix(rgb, rgb * sky_tint, occ);
    }

    // Low-sun factor (1 near the horizon → 0 high up): drives the warm haze glow
    // and the dawn/dusk valley mist.
    let low_sun = 1.0 - smoothstep(0.10, 0.45, globals.sun_dir.z);

    // Volumetric valley fog: mist pools in the low ground (elevation-based,
    // squared so it concentrates in deep valleys/fjords), thicker at dawn/dusk.
    // A cool near-white tinted by the horizon hue → golden at sunrise, blue by day.
    let zscale = max(globals.meters_to_world * globals.exaggeration, 1.0e-9);
    let elev_m = in.world_z / zscale;
    let valley = clamp(1.0 - elev_m / 400.0, 0.0, 1.0);
    // Mist peaks when the sun is AT the horizon (dawn/dusk) and falls off for both
    // high noon and deep night — so midday stays clean and night goes dark.
    let fog_tod = exp(-globals.sun_dir.z * globals.sun_dir.z * 18.0);
    let fog_amt = clamp(valley * valley * fog_tod * 0.55, 0.0, 0.55);
    // Fog colour tracks the time-of-day horizon hue (dark-blue at night, warm at
    // golden hour, pale by day). The lift toward white scales with ambient
    // daylight, so night mist stays dark instead of washing the ground to grey.
    let fog_color = mix(globals.haze_color, vec3<f32>(1.0), 0.3 * globals.ambient);
    rgb = mix(rgb, fog_color, fog_amt);

    // Aerial perspective: fade distant relief toward the atmosphere colour. The
    // haze glows WARM toward the sun (in-scatter) and stays cool away from it, so
    // distant ridges toward a low sun pick up the golden-hour light. Capped well
    // below 1 so the farthest ground never fully whites out.
    let view_dir = normalize(in.world_xy - globals.eye_world.xy);
    let sun_xy = globals.sun_dir.xy;
    let sun_align = dot(view_dir, normalize(sun_xy + vec2<f32>(1.0e-6)));
    let glow = pow(max(sun_align, 0.0), 4.0) * low_sun;
    let haze_tint = mix(globals.haze_color, globals.light_color, glow * 0.7);
    rgb = mix(rgb, haze_tint, min(in.haze, 0.78));

    return vec4<f32>(rgb, s.a * in.alpha);
}
