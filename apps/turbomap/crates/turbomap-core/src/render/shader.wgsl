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
    // Seconds since renderer start — slowly drifts the valley-fog field.
    time: f32,
    // Basemap brightness gain applied before sun-lighting (3D only; 1.0 in 2D).
    basemap_gain: f32,
    // Absolute world-xy of the camera centre. Added to the camera-relative
    // fragment world-xy to reconstruct an absolute position, so the valley-fog
    // field stays welded to the terrain instead of sliding with the screen.
    cam_origin: vec2<f32>,
    _pad1: vec2<f32>,
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
// World-locked baked ambient-occlusion field (see render/ao.rs): accumulated sky
// occlusion in [0,1] over the same grid + extent as the heightfield. Filterable.
@group(3) @binding(2) var ao_tex: texture_2d<f32>;
@group(3) @binding(3) var ao_samp: sampler;

// Heightfield resolution — must match `render::shadow::HEIGHT_DIM`. The march
// bilinearly interpolates the field (the texture is R32Float / unfiltered, so we
// do it by hand): nearest sampling leaves each texel a flat plateau whose edges
// are tiny vertical steps, and the per-pixel march turns those into a corduroy
// of shadow stripes. Bilinear smooths the relief so only real ridges occlude.
const HEIGHT_DIM_I: i32 = 256;
const HEIGHT_DIM_F: f32 = 256.0;

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
// further (low sun) at more cost; each step is one `shadow_texel_world`. Scaled
// up with HEIGHT_DIM (256) so the world reach is unchanged at the finer grid.
const SHADOW_STEPS: i32 = 64;

// Self-shadow start bias, in march steps. The ray is lifted by this many steps'
// worth of (local relief + ray rise) before testing, so it clears the surface it
// starts on instead of grazing it into acne stripes. Larger = fewer acne
// artifacts but near contact shadows start slightly further out.
const SHADOW_BIAS_STEPS: f32 = 2.5;

// March the heightfield from `start` toward the sun, jittered by `jit ∈ [0,1)`
// (sub-step offset to break the discrete-step comb on long grazing shadows).
// Returns vec2(over, hit_k): `over` = max world-z the relief pokes above the
// grazing ray (0 = lit), `hit_k` = the step that hit (drives the penumbra). The
// caller averages a couple of jittered calls to supersample the soft edge.
fn march_shadow(
    start: vec2<f32>, dir: vec2<f32>, step: f32, rise: f32,
    h0: f32, bias: f32, jit: f32,
) -> vec2<f32> {
    var over = 0.0;
    var hit_k = 0.0;
    var p = start + dir * (step * jit);
    for (var k = 1; k <= SHADOW_STEPS; k = k + 1) {
        p = p + dir * step;
        let uv = (p - globals.shadow_origin) * globals.shadow_inv_size;
        if (uv.x < 0.0 || uv.y < 0.0 || uv.x > 1.0 || uv.y > 1.0) {
            break;
        }
        let ray_z = h0 + bias + (f32(k) + jit) * rise;
        let excess = height_bilinear(uv) - ray_z;
        if (excess > over) {
            over = excess;
            hit_k = f32(k);
        }
    }
    return vec2<f32>(over, hit_k);
}

// --- Procedural haze field -------------------------------------------------
// Cheap value-noise fbm used to break the haze out of a flat uniform veil:
// it makes the low haze patchy (banks here, clear there), and because the
// sample coordinate drifts with `globals.time` the patches roll across the
// terrain over time. Sampled in a metres-relative world frame so the feature
// size is zoom-stable.
fn hash2(p: vec2<f32>) -> f32 {
    return fract(sin(dot(p, vec2<f32>(127.1, 311.7))) * 43758.5453123);
}

// Interleaved gradient noise (Jimenez) on a screen-pixel coord: a low-discrepancy
// dither whose energy is spread far more evenly than white noise, so as a
// sampling offset it reads like fine film grain instead of salt-and-pepper. Used
// to jitter the shadow march so the discrete step grid + coarse heightfield don't
// alias into a hard comb on long grazing-sun shadows.
fn ign(p: vec2<f32>) -> f32 {
    return fract(52.9829189 * fract(dot(p, vec2<f32>(0.06711056, 0.00583715))));
}

fn vnoise(p: vec2<f32>) -> f32 {
    let i = floor(p);
    let f = fract(p);
    // Quintic smootherstep — no grid-aligned creasing as patches drift.
    let u = f * f * f * (f * (f * 6.0 - 15.0) + 10.0);
    let a = hash2(i + vec2<f32>(0.0, 0.0));
    let b = hash2(i + vec2<f32>(1.0, 0.0));
    let c = hash2(i + vec2<f32>(0.0, 1.0));
    let d = hash2(i + vec2<f32>(1.0, 1.0));
    return mix(mix(a, b, u.x), mix(c, d, u.x), u.y);
}

// Two-octave value noise — deliberately low-frequency, so the result is smooth
// and puffy rather than a high-frequency patchwork. Used only to decide WHICH
// low areas hold valley fog (a non-uniform presence). ~[0,1].
fn smooth_field(p: vec2<f32>) -> f32 {
    let v = vnoise(p) * 0.65 + vnoise(p * 2.3 + vec2<f32>(11.0, 5.0)) * 0.35;
    // Gentle contrast around the midpoint so fog reads as clear "bank vs clear"
    // swaths with soft edges, not a flat grey average that never crosses the
    // selection threshold.
    return clamp((v - 0.5) * 1.5 + 0.5, 0.0, 1.0);
}

// Valley-fog tuning (metres unless noted). Named consts so the look is easy to
// tune; validated on device (the headless basemap is ~white).
const FOG_FEATURE_M: f32 = 4500.0;   // size of the fog/clear swaths — large = broad + smooth
const FOG_DRIFT_M: f32 = 4.0;        // how fast the field crawls over time (m/s) — gentle
const FOG_FLOOR_M: f32 = 120.0;      // full fog at/below this elevation
const FOG_TOP_M: f32 = 420.0;        // fog gone above this elevation
const FOG_SELECT_LO: f32 = 0.48;     // field value where fog begins to appear (higher = less coverage)
const FOG_SELECT_HI: f32 = 0.74;     // field value of full fog presence
const FOG_STRENGTH: f32 = 0.0;       // valley fog DISABLED for the clean water base (restore to 0.5)

// --- Ambient occlusion (cheap, per-vertex) ---------------------------------
// Sampled from a ring of DEM taps around each vertex: the more the neighbours
// rise above this point, the more of the sky hemisphere is blocked. Ring radius
// in mesh cells (GRID=16) and how hard the occlusion darkens the ambient.
const AO_RADIUS_CELLS: f32 = 2.5;
const AO_STRENGTH: f32 = 0.85;

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

// One AO ring tap: the clamped horizon tangent (rise/run) of a neighbour. Only
// neighbours HIGHER than the centre occlude (positive rise); a uniform slope
// has one side up and the other down, so it averages to ~0 and open hillsides
// stay lit — only genuine concavities accumulate occlusion. `center_m` is the
// centre elevation in metres, `run_world` the horizontal step in world units.
fn ao_tap(uv: vec2<f32>, center_m: f32, run_world: f32, zscale: f32) -> f32 {
    let rise = (elev_at(uv) - center_m) * zscale;
    return clamp(rise / max(run_world, 1.0e-6), 0.0, 1.0);
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
    // Cheap per-vertex ambient occlusion in [0,1] (1 = open sky, →0 = boxed in
    // by surrounding higher terrain). Sampled from the DEM ring in the vertex
    // shader and smoothly interpolated; darkens the ambient/sky-fill term so
    // valley floors, gully bottoms and cliff bases sit in soft shade.
    @location(6) ao: f32,
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

        // Cheap ambient occlusion: an 8-tap DEM ring at AO_RADIUS_CELLS. Each
        // tap adds the clamped rise of a HIGHER neighbour, so concavities (valley
        // floors, gully bottoms, cliff bases) accumulate occlusion while open
        // slopes and ridges stay near 1. Coarse per-vertex, smoothly interpolated.
        let ru = inst.dem_uv_size * (AO_RADIUS_CELLS / 16.0);
        let run = inst.world_size * (AO_RADIUS_CELLS / 16.0);
        let dd = ru * 0.70710678;
        var occ_sum = ao_tap(dem_uv + vec2<f32>(ru, 0.0), elev_m, run, zscale);
        occ_sum = occ_sum + ao_tap(dem_uv + vec2<f32>(-ru, 0.0), elev_m, run, zscale);
        occ_sum = occ_sum + ao_tap(dem_uv + vec2<f32>(0.0, ru), elev_m, run, zscale);
        occ_sum = occ_sum + ao_tap(dem_uv + vec2<f32>(0.0, -ru), elev_m, run, zscale);
        occ_sum = occ_sum + ao_tap(dem_uv + vec2<f32>(dd, dd), elev_m, run, zscale);
        occ_sum = occ_sum + ao_tap(dem_uv + vec2<f32>(-dd, dd), elev_m, run, zscale);
        occ_sum = occ_sum + ao_tap(dem_uv + vec2<f32>(dd, -dd), elev_m, run, zscale);
        occ_sum = occ_sum + ao_tap(dem_uv + vec2<f32>(-dd, -dd), elev_m, run, zscale);
        out.ao = clamp(1.0 - (occ_sum / 8.0) * AO_STRENGTH, 0.0, 1.0);
    } else {
        out.normal = vec3<f32>(0.0, 0.0, 1.0);
        out.ao = 1.0;
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
            // `over` (max height the relief pokes above the grazing sun ray) and
            // `hit_k` (which step hit) are hoisted so the edge AA below runs in
            // UNIFORM control flow — `fwidth` is only valid once the per-fragment
            // bounds check has closed. Out-of-field fragments keep over=0 → no
            // shadow.
            var over = 0.0;
            var hit_k = 0.0;
            if (suv0.x >= 0.0 && suv0.y >= 0.0 && suv0.x <= 1.0 && suv0.y <= 1.0) {
                let dir = sxy / lxy;
                let tan_alt = globals.sun_dir.z / lxy;        // world-z rise per world-xy
                let step = globals.shadow_texel_world;
                let rise = tan_alt * step;                    // rise per march step
                let h0 = height_bilinear(suv0);
                // Start bias: lift the grazing ray above the LOCAL surface before
                // testing, so the near field doesn't self-shadow into acne (the comb
                // of stripes over gentle ground). The lift clears the relief the ray
                // would otherwise skim — `step*slope` (relief per step, from the
                // smooth normal) plus the ray's own rise, times a few steps.
                let slope = length(n.xy) / max(n.z, 0.05);
                let bias = (step * slope + rise) * SHADOW_BIAS_STEPS;
                // Two jittered marches, averaged: a 2-tap SUPERSAMPLE of the soft
                // shadow. At a low evening sun a tall peak throws long finger
                // shadows through its cols; the coarse heightfield + discrete march
                // alias those into a hard jagged comb. Offsetting two marches by
                // half a step (interleaved-gradient dither, screen-locked) and
                // averaging blends the fingers into a soft penumbra — the soft look
                // the stochastic version had, but with ~half the noise and no acne.
                let j0 = ign(floor(in.clip_position.xy));
                let j1 = fract(j0 + 0.5);
                let m0 = march_shadow(in.world_xy, dir, step, rise, h0, bias, j0);
                let m1 = march_shadow(in.world_xy, dir, step, rise, h0, bias, j1);
                over = (m0.x + m1.x) * 0.5;
                hit_k = (m0.y + m1.y) * 0.5;
            }
            // Contact hardening: the penumbra widens with the occluder's distance
            // (a far ridge throws a soft edge, a nearby lip a crisp one). Pushed
            // harder than a pure stencil so long evening shadows go soft + diffuse
            // (which is also how low-sun shadows really read, lost in scattered
            // light) — this is what dissolves the residual finger-comb at distance.
            let soft_art = max(globals.shadow_softness, 1.0e-7)
                * (1.0 + 14.0 * hit_k / f32(SHADOW_STEPS));
            // Analytic screen-space antialiasing: widen the penumbra to at least
            // cover one pixel's worth of variation in `over`. This is what turns
            // the per-pixel march noise (from the dither + step quantisation) and
            // thin-ridge shadow slivers into smooth soft edges instead of dotted,
            // jagged lines — `fwidth` is the 2×2-quad screen-space derivative, so
            // the softening tracks how fast the shadow boundary moves on screen at
            // any zoom. Near-free (one derivative, no extra marching).
            let soft = max(soft_art, fwidth(over) * 2.5);
            occ = smoothstep(0.0, soft, over) * globals.shadow_strength;
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
    // Baked horizon AO (world-locked field) where it covers this fragment, with
    // a soft fade to the cheap per-vertex AO at the field's edge (no seam). The
    // baked field is finer + multi-directional; the per-vertex term backstops the
    // far field beyond the assembled region. `textureSampleLevel` (explicit LOD)
    // is used so the sample is legal in this data-dependent branch.
    var ao = in.ao;
    if (globals.shadow_inv_size > 0.0) {
        let auv = (in.world_xy - globals.shadow_origin) * globals.shadow_inv_size;
        if (auv.x >= 0.0 && auv.y >= 0.0 && auv.x <= 1.0 && auv.y <= 1.0) {
            let occ_baked = textureSampleLevel(ao_tex, ao_samp, auv, 0.0).r;
            let baked = clamp(1.0 - occ_baked * AO_STRENGTH, 0.0, 1.0);
            let edge = min(min(auv.x, 1.0 - auv.x), min(auv.y, 1.0 - auv.y));
            ao = mix(in.ao, baked, smoothstep(0.0, 0.06, edge));
        }
    }

    let sun_up = smoothstep(-0.04, 0.04, globals.sun_dir.z);
    let direct_amt = (1.0 - globals.ambient) * ndl * (1.0 - occ) * sun_up;
    // Skylight (the ambient/indirect term) is occluded BOTH by the per-vertex
    // AO (boxed-in concavities) and, a little, by cast shadow (a shadowed pocket
    // also sees less open sky). Direct sun is warm (`light_color`); the sky-fill
    // is the same brightness pulled toward the cool atmosphere hue, so shadows
    // and cavities read as cool ambient light rather than a flat grey wash —
    // the core of the more "atmospheric" look.
    let ambient_amt = globals.ambient * ao * (1.0 - 0.25 * occ);
    let sky_fill = mix(globals.light_color, globals.haze_color, 0.35);
    // Per-basemap brightness lift (e.g. dark satellite) before lighting, so it
    // reads under the same sun/ambient that suits bright topo.
    let base_rgb = s.rgb * globals.basemap_gain;
    var rgb = base_rgb * (ambient_amt * sky_fill + direct_amt * globals.light_color);

    // Low-sun factor (1 near the horizon → 0 high up): drives the warm haze glow.
    let low_sun = 1.0 - smoothstep(0.10, 0.45, globals.sun_dir.z);

    let zscale = max(globals.meters_to_world * globals.exaggeration, 1.0e-9);
    let elev_m = in.world_z / zscale;

    // --- Selective valley fog: smooth, world-locked, low ground only -----
    // Reconstruct the ABSOLUTE world-xy (in.world_xy is camera-relative, which
    // would pin the fog to the screen) so the fog stays welded to the terrain as
    // the camera pans. A large, low-frequency smooth field then decides WHICH low
    // areas hold fog — broad and puffy, a non-uniform *presence* rather than a
    // high-frequency patchwork. The field crawls slowly with time.
    let world_abs = in.world_xy + globals.cam_origin;
    let feat = max(FOG_FEATURE_M * globals.meters_to_world, 1.0e-12);
    let drift = vec2<f32>(1.0, 0.4) * (FOG_DRIFT_M * globals.time * globals.meters_to_world);
    let presence = smooth_field((world_abs + drift) / feat);
    // Only the upper part of the field's range becomes fog → broad swaths stay
    // clear, some valleys fill, with soft edges (no hard patch borders).
    let sel = smoothstep(FOG_SELECT_LO, FOG_SELECT_HI, presence);
    // Low ground only: full at/below FOG_FLOOR_M, gone above FOG_TOP_M, smooth between.
    let low = 1.0 - smoothstep(FOG_FLOOR_M, FOG_TOP_M, elev_m);
    // Subtle by day (it's mainly a distance + dawn/dusk effect, not a daytime
    // ground veil); thickens into warm valley mist as the sun nears the horizon;
    // gone deep at night.
    let tod = 0.22 + 0.78 * exp(-globals.sun_dir.z * globals.sun_dir.z * 12.0);
    let fog_amt = clamp(sel * low * tod * FOG_STRENGTH, 0.0, 0.72);
    // Mist colour tracks the time-of-day horizon hue; the lift toward white
    // scales with daylight so night mist stays dark instead of glowing grey.
    let mist_color = mix(globals.haze_color, vec3<f32>(1.0), 0.35 * globals.ambient);
    rgb = mix(rgb, mist_color, fog_amt);

    // --- Aerial perspective: the main distance haze (smooth, uniform) ----
    // Fade distant relief toward the atmosphere colour over the true eye→fragment
    // distance. Uniform with distance (that's what aerial haze is); glows WARM
    // toward a low sun (in-scatter), cool away from it. Capped below 1 so the
    // farthest ground never fully whites out.
    let view_dir = normalize(in.world_xy - globals.eye_world.xy);
    let sun_xy = globals.sun_dir.xy;
    let sun_align = dot(view_dir, normalize(sun_xy + vec2<f32>(1.0e-6)));
    let glow = pow(max(sun_align, 0.0), 4.0) * low_sun;
    let haze_tint = mix(globals.haze_color, globals.light_color, glow * 0.7);
    // Square the distance term so the NEAR field stays clear and haze builds only
    // with real distance. The raw `1 - exp(-dist·density)` already floors the
    // nearest ground at ~8%, so tilting in lifted a uniform veil onto even the
    // ground right below the camera; squaring drops that near value to ~0.6% while
    // leaving the far field (raw ≈ 0.95 → 0.90) almost untouched.
    let aerial = min(in.haze * in.haze, 0.82);
    rgb = mix(rgb, haze_tint, aerial);

    return vec4<f32>(rgb, s.a * in.alpha);
}
