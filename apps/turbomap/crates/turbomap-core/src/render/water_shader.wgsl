// Realistic water surface. Draws the water-body fills split out of the vector
// mesh (`source_layer == "water"`) as an animated, reflective liquid surface
// instead of a flat matte fill.
//
// Vertex stage: identical tile placement + DEM draping to `vector_shader.wgsl`
// (groups 0/1/2 are bound from the very same VectorPipeline buffers), so water
// lies on the terrain exactly where the flat fill used to. It additionally
// passes the world-space (RTC) position to the fragment stage.
//
// Fragment stage: a wave-perturbed normal drives a Fresnel blend between a deep
// water-body tint (the baked style colour) and a reflection of the analytic
// atmosphere (the SAME colour function as `sky_shader.wgsl` — kept in sync by
// hand; see `sky_color`), plus an HDR sun glitter that feeds the bloom pass.

// ---- group 0/1/2: shared with the vector pipeline -------------------------
struct CameraUniform {
    view_proj: mat4x4<f32>,
    // .x = pixels per world unit, .y = meters_to_world·exaggeration (drape
    // z-scale; 0 ⇒ flat), .z = DEM encoding, .w = halo_uv inset.
    params: vec4<f32>,
};
struct TileUniform {
    tile_alpha: f32,
    use_paint_color: f32,
    dash_len: f32,
    gap_len: f32,
    paint_color: vec4<f32>,
    origin: vec2<f32>,
    span: f32,
    width_scale: f32,
    dem_uv_origin: vec2<f32>,
    dem_uv_size: f32,
};
@group(0) @binding(0) var<uniform> camera: CameraUniform;
@group(1) @binding(0) var<uniform> tile: TileUniform;
@group(2) @binding(0) var dem_tex: texture_2d<f32>;
@group(2) @binding(1) var dem_samp: sampler;

// ---- group 3: water lighting + animation ----------------------------------
struct WaterGlobals {
    // Direction TOWARD the sun (world frame, x=E y=S z=up) + glow intensity
    // (→0 at night). Same values the sky/terrain use, so reflections match.
    sun_dir: vec3<f32>,
    sun_intensity: f32,
    // Analytic-sky palette (linear RGB), shared with the sky pass.
    zenith_color: vec3<f32>,
    // Renderer wall-clock seconds, drives the wave animation.
    time: f32,
    horizon_color: vec3<f32>,
    // 1 / metres-per-world at the camera latitude — converts the RTC world
    // position to ground metres so waves have a physical, zoom-stable size.
    meters_to_world: f32,
    sun_color: vec3<f32>,
    // 1.0 ⇒ march the reflected ray against the terrain heightfield (group 4) to
    // mirror the surrounding mountains; 0.0 ⇒ reflect the analytic sky only.
    ssr_enabled: f32,
    // Camera eye in the RTC frame (relative to the tile origin frame), for the
    // per-fragment view vector.
    eye: vec3<f32>,
    // Shoreline-foam intensity (0 = none).
    foam: f32,
    // Heightfield placement (shared with the shadow/AO field): UV =
    // (world.xy - hf_origin) * hf_inv_size. `hf_origin` is in this frame's RTC.
    hf_origin: vec2<f32>,
    hf_inv_size: f32,
    // Heightfield texel (its steeper world-z) → mesh world-z: ×cos²lat.
    hf_to_mesh_z: f32,
    // Dominant wave propagation direction (unit, world x=E/y=S) from the forecast.
    wave_dir: vec2<f32>,
    // Sea-state ferocity (amplitude/steepness multiplier) and whitecap amount.
    wave_amp: f32,
    whitecap: f32,
    // 1 ⇒ realistic AAA water (Gerstner displacement + waves + reflection +
    // glitter); 0 ⇒ flat matte body-colour fill (rail toggle off).
    realistic: f32,
    // P3: metres of shore-proximity over which the shallow→deep colour ramp
    // resolves (Beer-Lambert absorption).
    shallow_scale: f32,
    // P3: refraction strength (screen-UV bend of the underlying scene colour in
    // shallow water). 0 = off.
    refract: f32,
    // P6 quality: 1 high (full SSR + refraction), 0.5 medium (cheap SSR, no
    // refraction), 0 low (analytic sky reflection only).
    quality: f32,
    // Viewport resolution (px) — `@builtin(position)`/resolution = screen UV.
    viewport: vec2<f32>,
    // P3/P5 clarity 0..1: low = murky green coastal/lake water, high = clear blue.
    clarity: f32,
    // Drape z-scale (mesh world-z → metres) for the shore-proximity test.
    zscale: f32,
    // RTC view-projection (group 0 is VERTEX-only) — projects the SSR hit to a
    // screen UV for the real Scene-Colour read.
    view_proj: mat4x4<f32>,
};
@group(3) @binding(0) var<uniform> water: WaterGlobals;
// Terrain heightfield (world-z elevations, R32Float, non-filterable) — the same
// field the cast-shadow + AO passes march. Folded into group 3 (binding 1)
// because devices cap `max_bind_groups` at 4. Sampled with `textureLoad`.
@group(3) @binding(1) var height_tex: texture_2d<f32>;
const HF_DIM: i32 = 256;

// ---- Ocean Field cascades (group 3, bindings 2..5) ------------------------
// Three mip-filtered cascade textures: RGBA = (displacement.x, displacement.y,
// height, foam), all in metres, periodic over their patch size. Sampled at the
// fragment's world position to build the surface — mip filtering is what kills
// the zoom-out tiling/aliasing the procedural waves suffered.
@group(3) @binding(2) var ocean_c0: texture_2d<f32>;
@group(3) @binding(3) var ocean_c1: texture_2d<f32>;
@group(3) @binding(4) var ocean_c2: texture_2d<f32>;
@group(3) @binding(5) var ocean_samp: sampler;
// Scene Colour (binding 6): the opaque pass's resolved HDR image — the lit
// basemap/terrain BEFORE water. The reflected-ray march projects its terrain hit
// to screen and reads the real colour here (true mirrored mountains); shallow
// water also samples it at a refracted offset (see-through to the sea bed).
@group(3) @binding(6) var scene_tex: texture_2d<f32>;
@group(3) @binding(7) var scene_samp: sampler;
// Patch sizes (metres) — MUST match PATCH_M in ocean.rs.
const OCEAN_PATCH0: f32 = 457.0;
const OCEAN_PATCH1: f32 = 97.0;
const OCEAN_PATCH2: f32 = 23.0;

/// Sum the cascades' displacement (xy choppiness, z height) + foam at metre
/// position `p`, at an explicit mip `lod` (for the vertex stage, which has no
/// screen derivatives). Returns vec4(disp.x, disp.y, height, foam).
fn ocean_sample_lod(p: vec2<f32>, lod: f32) -> vec4<f32> {
    var acc = vec4<f32>(0.0, 0.0, 0.0, 0.0);
    acc += textureSampleLevel(ocean_c0, ocean_samp, p / OCEAN_PATCH0, lod);
    acc += textureSampleLevel(ocean_c1, ocean_samp, p / OCEAN_PATCH1, lod);
    acc += textureSampleLevel(ocean_c2, ocean_samp, p / OCEAN_PATCH2, lod);
    return acc;
}

/// Same, but with automatic mip selection (fragment stage) — proper distance
/// anti-aliasing via the hardware derivative.
fn ocean_sample(p: vec2<f32>) -> vec4<f32> {
    var acc = vec4<f32>(0.0, 0.0, 0.0, 0.0);
    acc += textureSample(ocean_c0, ocean_samp, p / OCEAN_PATCH0);
    acc += textureSample(ocean_c1, ocean_samp, p / OCEAN_PATCH1);
    acc += textureSample(ocean_c2, ocean_samp, p / OCEAN_PATCH2);
    return acc;
}

/// Summed wave HEIGHT (metres) at metre position `p`, fragment-mip-filtered.
fn ocean_height(p: vec2<f32>) -> f32 {
    return ocean_sample(p).z;
}

fn decode_elevation(enc: u32, rgb: vec3<f32>) -> f32 {
    let r = rgb.r * 255.0;
    let g = rgb.g * 255.0;
    let b = rgb.b * 255.0;
    if (enc == 1u) {
        return r * 256.0 + g + b / 256.0 - 32768.0; // Terrarium
    } else {
        return -10000.0 + (r * 256.0 * 256.0 + g * 256.0 + b) * 0.1;
    }
}

struct VertexInput {
    @location(0) base: vec2<f32>,
    @location(1) normal: vec2<f32>,
    @location(2) width_px: f32,
    @location(3) color: vec4<f32>,
    @location(4) edge_pos: vec4<f32>,
    @location(5) dist: f32,
    @location(6) z: f32,
};

struct VertexOutput {
    @builtin(position) clip_position: vec4<f32>,
    // Deep-water tint (baked style colour, linear).
    @location(0) color: vec4<f32>,
    // World-space (RTC) surface position (post-displacement), for the view
    // vector + fragment waves.
    @location(1) world_pos: vec3<f32>,
    // Geometric normal of the Gerstner swell (z-up), perturbed further in the FS.
    @location(2) wave_normal: vec3<f32>,
    // Distance to the nearest shore edge, tile-local units (baked into `dist`).
    // The FS turns it into a depth-based absorption tint (shallow → deep).
    @location(3) shore: f32,
    // Draped surface elevation in metres (sea ≈ 0, inland lakes higher) — the FS
    // uses it to pick sea vs lake treatment without needing group 0 (camera).
    @location(4) elev_m: f32,
};

@vertex
fn vs_main(in: VertexInput) -> VertexOutput {
    var out: VertexOutput;
    // Water fills carry width_px 0, so no stroke extrusion — placement is the
    // plain tile-local → world transform, draped onto the DEM exactly like the
    // vector fills it replaces.
    let world = tile.origin + in.base * tile.span;
    var wz = in.z;
    let zscale = camera.params.y;
    if (zscale > 0.0) {
        let dem_uv = tile.dem_uv_origin + in.base * tile.dem_uv_size;
        let s = textureSampleLevel(dem_tex, dem_samp, dem_uv, 0.0);
        if (s.a >= 0.5) {
            wz = wz + decode_elevation(u32(camera.params.z), s.rgb) * zscale;
        }
    }
    var world_pos = vec3<f32>(world, wz);
    // Ocean Field displacement: sample the cascade textures at this vertex's
    // metre position and offset the grid into real 3-D crests (xy = Gerstner
    // choppiness, z = wave height). Mip choice scales with view distance so far
    // vertices read a coarse, calm field (no per-vertex aliasing on the coarse
    // grid — the texture's own mips solve what the procedural path couldn't).
    if (water.realistic > 0.5) {
        let inv_m = 1.0 / max(water.meters_to_world, 1e-12);
        let p_m = world * inv_m;
        let dist_m = length(water.eye - world_pos) * inv_m;
        // Coarser mip the farther the vertex: ~level per doubling of distance,
        // so the vertex-sampled displacement stays resolvable for the grid.
        let lod = clamp(log2(max(dist_m, 1.0) / 60.0), 0.0, 7.0);
        let s = ocean_sample_lod(p_m, lod);
        world_pos += vec3<f32>(s.xy * water.meters_to_world, s.z * zscale);
    }
    out.clip_position = camera.view_proj * vec4<f32>(world_pos, 1.0);
    out.color = in.color;
    out.world_pos = world_pos;
    out.wave_normal = vec3<f32>(0.0, 0.0, 1.0);
    out.shore = in.dist;
    // Draped base elevation in metres (before wave displacement) for sea/lake.
    out.elev_m = select(0.0, wz / max(zscale, 1e-9), zscale > 0.0);
    return out;
}

// Rotate a 2D direction by `a` radians.
fn rot2(d: vec2<f32>, a: f32) -> vec2<f32> {
    let c = cos(a);
    let s = sin(a);
    return vec2<f32>(d.x * c - d.y * s, d.x * s + d.y * c);
}

// ---- Gerstner swell (vertex displacement) ---------------------------------
// A few long trochoidal waves that actually displace the mesh into 3-D crests
// (GPU Gems Ch.1 Eq.9–12) — the *geometry* of the sea. Crests pinch toward their
// peaks (the xy term) and rise (the z term); the fragment stage then adds the
// fine exp(sin) ripple detail on top as a normal map. All metre-space.
const GERSTNER_WAVES: i32 = 4;

struct Gerstner {
    // Metre-space displacement: xy = trochoidal crest pinch, z = wave height.
    offset: vec3<f32>,
    // Analytic surface normal (unitless, z-up), already amplitude-scaled.
    normal: vec3<f32>,
};

// Sum the swell at metre-space point `p_m` and time `t`. `amp_scale` damps the
// whole surface toward flat (distance LOD — coarse far tiles can't resolve crests).
fn gerstner(p_m: vec2<f32>, t: f32, amp_scale: f32) -> Gerstner {
    let g = 9.81;
    let base_dir = normalize(water.wave_dir + vec2<f32>(1e-4, 0.0));
    var disp = vec3<f32>(0.0, 0.0, 0.0);
    var slope = vec2<f32>(0.0, 0.0); // Σ d·(w·A)·cos — the xy gradient of height
    var wavelen = 28.0;              // dominant wavelength (m)
    var amp = 0.42 * water.wave_amp; // dominant amplitude (m)
    var ang = 0.0;
    for (var i = 0; i < GERSTNER_WAVES; i = i + 1) {
        let w = 6.2831853 / wavelen;     // spatial frequency k = 2π/L
        let speed = sqrt(g * w);         // deep-water dispersion ω = √(gk)
        let d = rot2(base_dir, ang);
        let theta = w * dot(d, p_m) + speed * t;
        let c = cos(theta);
        let s = sin(theta);
        // Steepness Q kept so Σ Q·w·A ≤ 1 (no looping/self-intersecting crests).
        let q = 0.78 / (w * max(amp, 1e-4) * f32(GERSTNER_WAVES));
        disp.x += q * amp * d.x * c;
        disp.y += q * amp * d.y * c;
        disp.z += amp * s;
        slope += d * (w * amp) * c;
        // Next octave: shorter, smaller, fanned to alternating sides of the swell.
        wavelen *= 0.62;
        amp *= 0.62;
        ang += 0.55 * select(-1.0, 1.0, (i & 1) == 0);
    }
    var out: Gerstner;
    out.offset = disp * amp_scale;
    // Normal of z = height(x,y): (−∂h/∂x, −∂h/∂y, 1), slope damped with amplitude.
    out.normal = normalize(vec3<f32>(-slope * amp_scale, 1.0));
    return out;
}

// One exp(sin) wavelet + its derivative. `exp(sin(x)-1)` has sharp crests and
// broad troughs (trochoidal-ish); the derivative drives domain warping. Summed
// plain sines always interfere into a regular corduroy/cross-hatch — exp(sin)
// with domain warp does not.
fn wavedx(p: vec2<f32>, dir: vec2<f32>, freq: f32, t: f32) -> vec2<f32> {
    let x = dot(dir, p) * freq + t;
    let wave = exp(sin(x) - 1.0);
    return vec2<f32>(wave, -wave * cos(x));
}

// Sum of exp(sin) wavelets running DIRECTIONALLY with the wind. The octaves'
// directions fan only ±~25° around `wave_dir` (NOT the old golden-angle spread,
// which made isotropic foam cells/bubble-wrap instead of waves) and each travels
// at the deep-water phase speed, so the surface reads as directional chop. A
// mild domain warp adds texture + breaks exact tiling without forming cells.
// Returns wave HEIGHT in metres.
fn getwaves(p_in: vec2<f32>, t: f32, zoom: f32, lod: f32) -> f32 {
    var p = p_in;
    let base = normalize(water.wave_dir + vec2<f32>(1e-4, 0.0));
    var freq = 6.2831853 / (11.0 * zoom); // base wavelength ~11·zoom m
    var weight = 1.0;
    var sum = 0.0;
    var tw = 0.0;
    let g = 9.81;
    for (var i = 0; i < 6; i = i + 1) {
        // Per-octave distance LOD: finest octaves fade out far away (anti-alias).
        let oct = clamp(lod - f32(i), 0.0, 1.0);
        // Narrow directional fan: i=0..5 → ±0.45 rad (±26°) around the wind.
        let ang = (f32(i) - 2.5) * 0.18;
        let d = rot2(base, ang);
        let speed = sqrt(g * freq); // ω = √(g·k) deep-water dispersion
        let res = wavedx(p, d, freq, speed * t);
        p = p + d * res.y * weight * oct * 0.30; // mild warp (texture, not cells)
        sum = sum + res.x * weight * oct;
        tw = tw + weight * oct;
        weight = weight * 0.78;
        freq = freq * 1.32;
    }
    return (sum / max(tw, 1e-4)) * water.wave_amp * 0.5;
}

// Wave-perturbed normal (.xyz) + crest measure (.w). Normal from finite
// differences of the height field; epsilon scales with zoom so the slope stays
// physical at every wavelength. `steep` exaggerates the slope so the ripples
// read strongly on a flat mesh (normal-mapped water needs punchy normals to not
// look like clay). `lod` controls how many octaves resolve (distance AA).
fn wave_surface(p: vec2<f32>, t: f32, zoom: f32, lod: f32) -> vec4<f32> {
    let amp_m = max(water.wave_amp * 0.5, 1e-4);
    let e = 0.5 * zoom;
    let h = getwaves(p, t, zoom, lod);
    let hx = getwaves(p + vec2<f32>(e, 0.0), t, zoom, lod);
    let hy = getwaves(p + vec2<f32>(0.0, e), t, zoom, lod);
    let steep = 3.0;
    let n = normalize(vec3<f32>(-(hx - h) / e * steep, -(hy - h) / e * steep, 1.0));
    let crest = clamp(h / amp_m * 0.5 + 0.5, 0.0, 1.0);
    return vec4<f32>(n, crest);
}

// Shoreline foam: probe the heightfield in a ring around this point; where land
// rises above the water surface nearby, we're at a shore → foam. Returns 0..1.
// 0 when no heightfield is bound (open sea / no terrain). Animated edge wobble
// keeps the foam line alive rather than a hard band.
fn shoreline_foam(world_pos: vec3<f32>, t: f32) -> f32 {
    if (water.ssr_enabled < 0.5 || water.foam <= 0.0) {
        return 0.0;
    }
    let texel_world = 1.0 / (water.hf_inv_size * f32(HF_DIM));
    let radius = texel_world * 2.0;
    var max_rise = 0.0;
    // 6-tap ring.
    for (var i = 0; i < 6; i = i + 1) {
        let a = f32(i) * 1.0472; // 60°
        let off = vec2<f32>(cos(a), sin(a)) * radius;
        let tz = terrain_mesh_z(hf_uv(world_pos.xy + off));
        max_rise = max(max_rise, tz - world_pos.z);
    }
    // Rise of ~1.5 texels of relief over the water level → full foam. Wobble the
    // band with a little travelling noise so it breathes.
    let pm = world_pos.xy / max(water.meters_to_world, 1e-12);
    let wob = 0.6 + 0.4 * sin(dot(pm, water.wave_dir) * 0.6 + t * 1.5);
    let band = smoothstep(0.0, texel_world * 1.5, max_rise)
        * (1.0 - smoothstep(texel_world * 1.5, texel_world * 4.0, max_rise));
    return clamp(band * wob * water.foam, 0.0, 1.0);
}

// Analytic atmosphere colour for a world-space ray direction. Mirrors the
// zenith↔horizon gradient + Mie sun halo/disk of `sky_shader.wgsl::fs_main`
// (minus the stars). Kept in sync by hand; if one changes, change both.
fn sky_color(rd: vec3<f32>) -> vec3<f32> {
    let up = clamp(rd.z, -1.0, 1.0);
    let t = pow(clamp(up, 0.0, 1.0), 0.42);
    var col = mix(water.horizon_color, water.zenith_color, t);
    if (up < 0.0) {
        col = mix(water.horizon_color, water.horizon_color * 0.72, clamp(-up * 4.0, 0.0, 1.0));
    }
    let mu = clamp(dot(rd, water.sun_dir), -1.0, 1.0);
    let halo = pow(max(mu, 0.0), 8.0) * 0.5;
    let disk = pow(max(mu, 0.0), 350.0) * 5.0;
    col += water.sun_color * (halo + disk) * water.sun_intensity;
    return col;
}

// World (RTC) xy → heightfield UV.
fn hf_uv(world_xy: vec2<f32>) -> vec2<f32> {
    return (world_xy - water.hf_origin) * water.hf_inv_size;
}

// One heightfield texel (nearest), in MESH world-z.
fn terrain_texel_z(xi: i32, yi: i32) -> f32 {
    let cx = clamp(xi, 0, HF_DIM - 1);
    let cy = clamp(yi, 0, HF_DIM - 1);
    return textureLoad(height_tex, vec2<i32>(cx, cy), 0).r * water.hf_to_mesh_z;
}

// Terrain elevation in MESH world-z at a heightfield UV, BILINEARLY filtered.
// The heightfield is R32Float (non-filterable, `textureLoad` only), so we do the
// 2×2 lerp by hand — this removes the texel-quantization stepping that made the
// shore/reflection fields band into hatching.
fn terrain_mesh_z(uv: vec2<f32>) -> f32 {
    let t = uv * f32(HF_DIM) - vec2<f32>(0.5, 0.5);
    let i0 = vec2<i32>(i32(floor(t.x)), i32(floor(t.y)));
    let f = fract(t);
    let z00 = terrain_texel_z(i0.x, i0.y);
    let z10 = terrain_texel_z(i0.x + 1, i0.y);
    let z01 = terrain_texel_z(i0.x, i0.y + 1);
    let z11 = terrain_texel_z(i0.x + 1, i0.y + 1);
    return mix(mix(z00, z10, f.x), mix(z01, z11, f.x), f.y);
}

// Project an RTC world point to a screen UV (origin top-left, y-down to match
// texture space). `.z` is 1.0 when the point is in front of the camera and on
// screen, else 0.0 — the caller falls back when the reflected geometry isn't
// actually visible in the Scene-Colour buffer (off-screen / behind camera).
fn world_to_screen_uv(p: vec3<f32>) -> vec3<f32> {
    let clip = water.view_proj * vec4<f32>(p, 1.0);
    if (clip.w <= 1e-4) {
        return vec3<f32>(0.0, 0.0, 0.0);
    }
    let ndc = clip.xy / clip.w;
    let uv = vec2<f32>(ndc.x * 0.5 + 0.5, 0.5 - ndc.y * 0.5);
    let on = select(0.0, 1.0,
        uv.x >= 0.0 && uv.x <= 1.0 && uv.y >= 0.0 && uv.y <= 1.0);
    return vec3<f32>(uv, on);
}

// March the reflected ray against the terrain heightfield to mirror the
// surrounding mountains, then read the REAL lit basemap colour by projecting the
// hit point into the opaque Scene-Colour buffer (true screen-space reflection of
// the terrain — the headline "real" cue). Where the hit isn't on screen (behind
// the camera / outside the frame) it falls back to a sun-lit hillshade of the
// reflected slope so distant mountains still read. Returns `vec4(colour, weight)`
// where `weight` is a 0..1 edge fade (0 = miss → caller keeps the sky).
fn reflect_terrain(p: vec3<f32>, r: vec3<f32>) -> vec4<f32> {
    // One heightfield texel in world units — the march step + gradient epsilon.
    let texel_world = 1.0 / (water.hf_inv_size * f32(HF_DIM));
    // Cover ~1.5 fields; bias the start off the surface to avoid self-hit.
    let span = 1.5 / water.hf_inv_size;
    // P6 quality: fewer march steps on the medium tier.
    let steps = i32(select(24.0, 48.0, water.quality > 0.75));
    let dt = span / f32(steps);
    var t = dt;
    var hit_uv = vec2<f32>(0.0);
    var hit_world = vec3<f32>(0.0);
    var found = false;
    for (var i = 0; i < steps; i = i + 1) {
        let s = p + r * t;
        let uv = hf_uv(s.xy);
        // Outside the assembled field → can't resolve; stop and miss (sky).
        if (uv.x < 0.0 || uv.x > 1.0 || uv.y < 0.0 || uv.y > 1.0) {
            break;
        }
        let tz = terrain_mesh_z(uv);
        if (s.z < tz - texel_world * 0.05) {
            // Crossed below the surface — refine once between the last two samples.
            let s0 = p + r * (t - dt);
            let mid = (s0 + s) * 0.5;
            hit_uv = hf_uv(mid.xy);
            hit_world = vec3<f32>(mid.xy, terrain_mesh_z(hit_uv));
            found = true;
            break;
        }
        t = t + dt;
    }
    if (!found) {
        return vec4<f32>(0.0, 0.0, 0.0, 0.0);
    }
    // Hillshade fallback colour (used off-screen): lit/shadowed reflected slope.
    let du = vec2<f32>(1.0 / f32(HF_DIM), 0.0);
    let dv = vec2<f32>(0.0, 1.0 / f32(HF_DIM));
    let zx = terrain_mesh_z(hit_uv + du) - terrain_mesh_z(hit_uv - du);
    let zy = terrain_mesh_z(hit_uv + dv) - terrain_mesh_z(hit_uv - dv);
    let nrm = normalize(vec3<f32>(-zx, -zy, 2.0 * texel_world));
    let lambert = max(dot(nrm, water.sun_dir), 0.0);
    let albedo = vec3<f32>(0.20, 0.23, 0.18);
    var lit = albedo * (0.40 + 0.85 * lambert * water.sun_intensity);
    // Real screen-space colour: project the hit into the Scene-Colour buffer.
    // This is the genuine mirrored terrain (lit basemap + shading), not a guess.
    let su = world_to_screen_uv(hit_world);
    if (su.z > 0.5) {
        let real = textureSampleLevel(scene_tex, scene_samp, su.xy, 0.0).rgb;
        // Fade to the hillshade near the screen border so a reflection sliding
        // off-frame doesn't pop (the classic SSR edge artefact).
        let b = min(min(su.x, 1.0 - su.x), min(su.y, 1.0 - su.y));
        let screen_edge = smoothstep(0.0, 0.06, b);
        lit = mix(lit, real, screen_edge);
    }
    // Fade the mirror out as the hit nears the heightfield border, so the field's
    // edge doesn't draw a hard diagonal line across open water (beyond it we
    // simply can't resolve terrain → fall back to sky).
    let edge = min(min(hit_uv.x, 1.0 - hit_uv.x), min(hit_uv.y, 1.0 - hit_uv.y));
    let edge_fade = smoothstep(0.0, 0.10, edge);
    return vec4<f32>(lit, edge_fade);
}

// Continuous shore-proximity shallowness (P3): ~1.0 right at a shoreline,
// fading to 0.0 by `shallow_scale` metres offshore. A 16-tap spiral with a
// CONTINUOUSLY growing radius (golden-angle), each tap reading the BILINEAR
// heightfield — so the field is smooth in world space (no discrete-radius
// banding, no texel hatching). For each tap we measure how far land rises above
// the surface (in metres) and weight by closeness; the smooth max is the
// proximity. 0 when no heightfield is bound (open sea / no terrain).
fn shore_shallow(world_pos: vec3<f32>) -> f32 {
    if (water.ssr_enabled < 0.5) {
        return 0.0;
    }
    let m2w = max(water.meters_to_world, 1e-12);
    let max_r = water.shallow_scale * m2w;
    let inv_z = 1.0 / max(water.zscale, 1e-9);
    var prox = 0.0;
    for (var i = 0; i < 16; i = i + 1) {
        let f = (f32(i) + 0.5) / 16.0;        // 0..1 fractional radius
        let r = max_r * f;
        let a = f32(i) * 2.39996323;          // golden angle → even coverage
        let off = vec2<f32>(cos(a), sin(a)) * r;
        let bed_z = terrain_mesh_z(hf_uv(world_pos.xy + off));
        let rise_m = (bed_z - world_pos.z) * inv_z; // metres land rises above us
        // Clearly land once it rises ~1→6 m above the surface (a coastline jumps
        // far more than that); smooth so the band edge isn't hard.
        let land = smoothstep(1.0, 6.0, rise_m);
        // Closer land → shallower; (1-f) is the smooth closeness weight.
        prox = max(prox, land * (1.0 - f));
    }
    return clamp(prox, 0.0, 1.0);
}

@fragment
fn fs_main(in: VertexOutput) -> @location(0) vec4<f32> {
    // Flat path (rail toggle off): the plain matte body fill, as before AAA.
    if (water.realistic < 0.5) {
        return vec4<f32>(in.color.rgb, in.color.a * tile.tile_alpha);
    }
    // Ground-metre position for sampling the Ocean Field.
    let inv = 1.0 / max(water.meters_to_world, 1e-12);
    let p_m = in.world_pos.xy * inv;
    let dist_m = length(water.eye - in.world_pos) * inv;

    // Surface normal from the Ocean Field height gradient (finite difference in
    // metres). Because the cascade textures are mip-filtered, the gradient — and
    // thus the normal — anti-aliases smoothly with distance instead of shimmering
    // into a tiling grid (the failure of the per-fragment procedural waves). The
    // gradient step widens with distance so far water reads calm, not noisy.
    // Gradient baseline SCALES with view distance. Up close (e≈0.6 m) the fine
    // ripples drive the normal; far away the baseline widens to tens of metres so
    // the big swells (the 97 m / 457 m cascades) — which barely change over a
    // metre — actually produce slope, hence large-scale light/dark bands instead
    // of a flat mip-averaged fill. This is what makes the sea read as shaded
    // rolling swell at regional zoom, not a painted blue shape.
    let e = clamp(dist_m * 0.010, 0.6, 30.0);
    let fld = ocean_sample(p_m);
    let wh = fld.z;
    let whx = ocean_height(p_m + vec2<f32>(e, 0.0));
    let why = ocean_height(p_m + vec2<f32>(0.0, e));
    // Slope gain ramps with distance to keep the swell visibly alive once the
    // amplitude shrinks in screen space (a mild, non-physical exaggeration; the
    // far field is already mip-smoothed so this is rolling undulation, no grid).
    let steep = mix(3.5, 11.0, clamp(smoothstep(300.0, 6000.0, dist_m), 0.0, 1.0));
    let n = normalize(vec3<f32>(-(whx - wh) / e * steep, -(why - wh) / e * steep, 1.0));
    // Foam coverage from the field's Jacobian (where waves fold/break).
    let crest = clamp(fld.w, 0.0, 1.0);

    // --- P5: sea vs lake, one system, decided per-fragment from elevation ----
    // The draped surface sits at its real elevation; inland lakes/tarns are well
    // above sea level. `lakeness` 0 (sea, ~0 m) → 1 (mountain lake) makes the
    // surface calmer and glassier and recolours it toward dark alpine teal, with
    // no ocean whitecaps — a continuous transition, not a separate preset.
    let lakeness = clamp(smoothstep(2.0, 30.0, in.elev_m), 0.0, 1.0);
    // Lakes are calmer: damp the wave normal toward flat so they mirror sharply.
    let up = vec3<f32>(0.0, 0.0, 1.0);
    let nf = normalize(mix(n, up, 0.55 * lakeness));

    // P3: continuous shore shallowness (1 at the shore, fading offshore).
    let shallow = shore_shallow(in.world_pos);

    // View vector (surface → eye).
    let v = normalize(water.eye - in.world_pos);
    let ndotv = max(dot(nf, v), 1e-3);

    // Fresnel: low floor so top-down/mid angles show the VIVID body colour
    // (turquoise/navy) rather than a constant grey sky mirror — the earlier high
    // floor washed the sea grey. Grazing angles still ramp to a near-full mirror
    // (the wet look) via the pow term.
    let fres = clamp(0.10 + 0.85 * pow(1.0 - ndotv, 5.0), 0.0, 1.0);

    // ---- Reflection: analytic sky + TRUE screen-space terrain mirror (P2) ----
    // The full wave normal ripples the sky reflection (a smooth gradient → no
    // aliasing). The terrain mirror march uses a calmer normal so the reflected
    // mountains stay coherent; lakes (already calm) mirror almost flat. The march
    // projects its hit into the opaque Scene Colour, so the reflection is the REAL
    // lit basemap/terrain — genuine mirrored mountains, not a hillshade guess.
    let r_sky = reflect(-v, nf);
    var reflection = sky_color(r_sky);
    let n_terr = normalize(mix(up, nf, mix(0.30, 0.12, lakeness)));
    let r_terr = reflect(-v, n_terr);
    if (water.ssr_enabled > 0.5 && water.quality > 0.25 && r_terr.z > 0.02) {
        let terr = reflect_terrain(in.world_pos, r_terr);
        // Sharper/stronger mirror on glassy lakes; terr.w is a 0..1 edge fade.
        let mstr = mix(0.7, 0.9, lakeness);
        reflection = mix(reflection, terr.rgb, mstr * terr.w);
    }

    // ---- P3: depth + clarity body colour (Beer-Lambert shore→deep) ----------
    // Deep tint runs murky-green (low clarity) → clear blue (high clarity);
    // shallow tint teal → bright cyan. Lakes pull toward a darker alpine teal.
    // The shore→deep ramp is driven by the CONTINUOUS shore-proximity field
    // (sampled from the heightfield), so there are no tile-aligned colour blocks.
    let deep_sea = mix(vec3<f32>(0.010, 0.055, 0.115), vec3<f32>(0.015, 0.10, 0.205), water.clarity);
    let shallow_sea = mix(vec3<f32>(0.06, 0.26, 0.28), vec3<f32>(0.10, 0.42, 0.47), water.clarity);
    let deep_col = mix(deep_sea, vec3<f32>(0.015, 0.07, 0.085), lakeness);
    let shallow_col = mix(shallow_sea, vec3<f32>(0.05, 0.20, 0.20), lakeness);
    // Curve the shore→deep ramp so the bright shallow tint hugs the actual
    // coastline (a thin turquoise fringe) and the body goes deep quickly offshore.
    let shore_t = shallow * shallow * (3.0 - 2.0 * shallow); // smoothstep(shallow)
    var water_body = mix(deep_col, shallow_col, shore_t);
    // Respect the basemap palette a touch (the baked style colour).
    water_body = mix(water_body, in.color.rgb, 0.08);

    // NOTE (P3 refraction, deferred): true refraction would sample the sea bed
    // behind the surface, but this renderer has no bathymetry/seabed pass — the
    // only thing "behind" the water is the tiled satellite basemap, and sampling
    // it through the wave normal just reveals per-tile JPEG seams as hatching
    // (verified in the harness). Refraction is therefore intentionally OFF until a
    // real depth/seabed source exists; the shallow-water cue is delivered as the
    // Beer-Lambert depth COLOUR above + the shoreline foam band below, which read
    // correctly without a seabed. The `refract` uniform is retained for that future.

    // Subsurface scattering (P4): back-lit crests glow with a teal-green tint as
    // light passes through the thin pinched crest — strongest when the sun is low
    // and behind the wave. A tint added under the reflection, not over it.
    let back = clamp(dot(-v, water.sun_dir) * 0.5 + 0.5, 0.0, 1.0);
    let sss_col = mix(vec3<f32>(0.02, 0.11, 0.13), vec3<f32>(0.03, 0.16, 0.12), lakeness);
    let sss = sss_col * (0.35 + 0.9 * crest) * back * (0.3 + 0.7 * water.sun_intensity);
    let body = water_body + sss;

    // ---- Diffuse wave shading — THE cue that makes the sea read as 3-D --------
    // Light the wave normal against the sun: faces turned toward the sun are
    // bright, faces turned away fall into shadow. Without this the surface is a
    // flat tinted fill no matter how detailed the normal is. A wrapped lambert
    // (half-lit at the terminator) keeps troughs from crushing to black, and a
    // sky-ambient floor fills the shadow side with the horizon colour so shaded
    // water still looks like water, not slate. Collapses to ambient at night.
    let ndotl = dot(nf, water.sun_dir);
    let diffuse = clamp(ndotl * 0.5 + 0.5, 0.0, 1.0);
    // Ambient from the sky the surface faces (cheap up-facing sky term).
    let ambient = mix(water.horizon_color, water.zenith_color, 0.5) * 0.18;
    let sun_lit = water.sun_color * water.sun_intensity;
    // Shaded body: ambient sky fill + directional sun on the wave faces. The 0.55
    // sun weight gives a strong light/dark swing across crests vs troughs.
    let shade = ambient + sun_lit * diffuse * 0.55;
    // Signed wave height (crest > 0, trough < 0) adds a little extra lift/sink on
    // top of the normal shading so long swells read even where the slope is gentle.
    let amp_m = max(water.wave_amp * 0.5, 1e-4);
    let tone = clamp(wh / amp_m, -1.0, 1.0);
    let lit_body = body * (0.32 + shade) * (1.0 + 0.20 * tone);

    var col = mix(lit_body, reflection, fres);

    // Sun glitter (P4): a tight HDR sparkle on the rippling normal that bloom
    // scatters into sea-sparkle, plus a soft sunward sheen (kept modest so it
    // doesn't sheet-white). Both fade at night and on glassy lakes (which get the
    // sharp terrain mirror instead).
    let hvec = normalize(v + water.sun_dir);
    let ndoth = max(dot(nf, hvec), 0.0);
    let sparkle = pow(ndoth, 300.0) * 4.0;
    let sheen = pow(ndoth, 40.0) * 0.25 * (1.0 - lakeness);
    col += water.sun_color * (sparkle + sheen) * water.sun_intensity;

    // Whitecaps from the FIELD Jacobian (`crest` = fold-mask where geometry
    // pinches/breaks) — open sea only (lakes get no ocean whitecaps), ramped by
    // the forecast sea state. The big "alive ocean" cue, driven by wave shape.
    let breaking = smoothstep(0.5, 0.92, crest) * (0.4 + 0.9 * water.whitecap) * (1.0 - lakeness);
    let foam_lit = vec3<f32>(0.95, 0.97, 1.0) * (0.45 + 0.55 * water.sun_intensity);
    // Spray lift (P6, in-shader): the strongest breaking crests lift a faint mist
    // that bloom blows into spray at the scale we view the sea — a cheap, robust
    // stand-in for a particle system (a true GPU spray pass is deferred; it can't
    // be validated headlessly and risks mobile perf for little gain here).
    let spray = smoothstep(0.8, 1.0, crest) * water.whitecap * (1.0 - lakeness);
    col = mix(col, foam_lit, clamp(breaking, 0.0, 1.0));
    col += foam_lit * spray * 0.5;

    // Shoreline foam: a thin lively band where the water meets rising land.
    let foam_band = shoreline_foam(in.world_pos, water.time);
    col = mix(col, foam_lit, foam_band);

    let alpha = in.color.a * tile.tile_alpha;
    return vec4<f32>(col, alpha);
}
