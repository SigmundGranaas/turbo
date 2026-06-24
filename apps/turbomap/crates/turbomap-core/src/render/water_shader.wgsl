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

// Terrain elevation in MESH world-z at a heightfield UV (clamped to the field).
fn terrain_mesh_z(uv: vec2<f32>) -> f32 {
    let t = uv * f32(HF_DIM);
    let xi = clamp(i32(t.x), 0, HF_DIM - 1);
    let yi = clamp(i32(t.y), 0, HF_DIM - 1);
    let h = textureLoad(height_tex, vec2<i32>(xi, yi), 0).r;
    return h * water.hf_to_mesh_z;
}

// March the reflected ray against the heightfield to mirror the surrounding
// terrain. Returns `vec4(reflected_colour, hit)` where `hit` is 1.0 on a terrain
// intersection inside the field, else 0.0 (caller falls back to the sky).
fn reflect_terrain(p: vec3<f32>, r: vec3<f32>) -> vec4<f32> {
    // One heightfield texel in world units — the march step + gradient epsilon.
    let texel_world = 1.0 / (water.hf_inv_size * f32(HF_DIM));
    // Cover ~1.5 fields; bias the start off the surface to avoid self-hit.
    let span = 1.5 / water.hf_inv_size;
    let steps = 48;
    let dt = span / f32(steps);
    var t = dt;
    var hit_uv = vec2<f32>(0.0);
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
            found = true;
            break;
        }
        t = t + dt;
    }
    if (!found) {
        return vec4<f32>(0.0, 0.0, 0.0, 0.0);
    }
    // Shade the reflected terrain: a hillshade from the heightfield gradient,
    // lit by the same sun. Approximate colour (no basemap texture in this pass),
    // but the lit/shadowed slopes read as mountains mirrored in the water.
    let du = vec2<f32>(1.0 / f32(HF_DIM), 0.0);
    let dv = vec2<f32>(0.0, 1.0 / f32(HF_DIM));
    let zx = terrain_mesh_z(hit_uv + du) - terrain_mesh_z(hit_uv - du);
    let zy = terrain_mesh_z(hit_uv + dv) - terrain_mesh_z(hit_uv - dv);
    let nrm = normalize(vec3<f32>(-zx, -zy, 2.0 * texel_world));
    let lambert = max(dot(nrm, water.sun_dir), 0.0);
    let albedo = vec3<f32>(0.20, 0.23, 0.18);
    let lit = albedo * (0.40 + 0.85 * lambert * water.sun_intensity);
    // Fade the mirror out as the hit nears the heightfield border, so the field's
    // edge doesn't draw a hard diagonal line across open water (beyond it we
    // simply can't resolve terrain → fall back to sky).
    let edge = min(min(hit_uv.x, 1.0 - hit_uv.x), min(hit_uv.y, 1.0 - hit_uv.y));
    let edge_fade = smoothstep(0.0, 0.10, edge);
    return vec4<f32>(lit, edge_fade);
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
    // Small gradient step so fine ripples register; the cascade mips (auto-
    // selected by `ocean_sample`) handle the far-distance anti-aliasing, so the
    // step itself stays tight rather than pre-smoothing the slope.
    let e = max(dist_m * 0.0006, 0.5);
    let fld = ocean_sample(p_m);
    let wh = fld.z;
    let whx = ocean_height(p_m + vec2<f32>(e, 0.0));
    let why = ocean_height(p_m + vec2<f32>(0.0, e));
    // Slope gain ramps HARD with distance. Up close the real slopes read as
    // waves; far away (km up at map zoom) the waves are physically sub-pixel and
    // the mips have smoothed the field to gentle low-frequency swell — so we
    // amplify that smoothed slope a lot to keep the sea visibly alive (the
    // accepted non-physical exaggeration). Because the far field is already
    // mip-smoothed (no high frequencies), amplifying it yields visible rolling
    // undulation, NOT a high-frequency grid.
    let steep = mix(4.5, 20.0, clamp(smoothstep(300.0, 6000.0, dist_m), 0.0, 1.0));
    let n = normalize(vec3<f32>(-(whx - wh) / e * steep, -(why - wh) / e * steep, 1.0));
    // Foam coverage from the field's Jacobian (where waves fold/break).
    let crest = clamp(fld.w, 0.0, 1.0);

    // View vector (surface → eye).
    let v = normalize(water.eye - in.world_pos);
    let ndotv = max(dot(n, v), 1e-3);

    // Fresnel: low floor so top-down/mid angles show the VIVID body colour
    // (turquoise/navy) rather than a constant grey sky mirror — the earlier high
    // floor washed the sea grey. Grazing angles still ramp to a near-full mirror
    // (the wet look) via the pow term.
    let fres = clamp(0.10 + 0.85 * pow(1.0 - ndotv, 5.0), 0.0, 1.0);

    // Reflection — the heart of the wet look. The FULL wave normal ripples the
    // sky reflection so the surface shimmers (sky is a smooth gradient → no
    // aliasing from rippling it). The terrain mirror march uses a smoothed normal
    // so the reflected mountains stay coherent, then blends in.
    let r_sky = reflect(-v, n);
    var reflection = sky_color(r_sky);
    let n_terr = normalize(mix(vec3<f32>(0.0, 0.0, 1.0), n, 0.30));
    let r_terr = reflect(-v, n_terr);
    if (water.ssr_enabled > 0.5 && r_terr.z > 0.02) {
        let terr = reflect_terrain(in.world_pos, r_terr);
        // terr.w is now a 0..1 edge-fade (not a hard hit flag) → no field-edge line.
        reflection = mix(reflection, terr.rgb, 0.7 * terr.w);
    }

    // Body colour: a uniform vivid deep-sea tint for now. (P3 replaces this with
    // Beer-Lambert depth colour + refraction driven by a CONTINUOUS shallowness
    // field — the per-tile shore-distance term used here previously produced
    // tile-aligned colour blocks at low zoom, so it's disabled until P3.)
    let deep_col = mix(vec3<f32>(0.015, 0.09, 0.19), in.color.rgb, 0.12);
    // Subtle subsurface glow on foamy crests. A TINT only — reflection-dominated.
    let sss_col = vec3<f32>(0.03, 0.13, 0.16);
    let sss = sss_col * (0.5 + 0.7 * crest) * (0.3 + 0.7 * water.sun_intensity);
    let body = deep_col + sss;

    var col = mix(body, reflection, fres);

    // Direct wave shading — the key to looking ALIVE at map zoom. Reflection only
    // modulates the surface where the reflected environment varies; over open sea
    // / overcast the sky is uniform, so without this the waves vanish into flat
    // colour (the "underwhelming dead-calm" look). Crests catch more light and sit
    // brighter, troughs darker — environment-independent structure that always
    // reads as waves. Driven by the same fragment wave field (no vertex aliasing).
    // Directional wave shading: now that the chop runs with the wind (not
    // isotropic cells), visible light/dark BANDS read as waves. Moderate so it
    // doesn't wash white.
    col *= (0.82 + 0.34 * crest);

    // (Removed the crossed-sine "swell mottle": sin(x)+sin(y) over perpendicular
    // directions is a regular grid of peaks, which over dark deep water rendered
    // as a hideous lattice of bright blobs. Distance liveliness must come from a
    // non-periodic source, not crossed sines — revisit with domain-warped noise.)

    // Sun glitter: TWO lobes on the rippling normal — a tight HDR sparkle that
    // bloom scatters into sea-sparkle across the crests, plus a broader sun sheen
    // that gives the whole sunward side life (not just pinpricks). Fades at night.
    let h = normalize(v + water.sun_dir);
    let ndoth = max(dot(n, h), 0.0);
    // Tight HDR sparkle only (no broad sheen — the sheen washed the whole sunward
    // half white). Fades with the ripple detail so far water doesn't fizz, and
    // scaled by sun intensity so it's a scattered sea-sparkle, not a sheet.
    let sparkle = pow(ndoth, 300.0) * 4.0;
    col += water.sun_color * sparkle * water.sun_intensity;

    // Whitecaps from the FIELD: `crest` is the Jacobian fold-mask (where the wave
    // geometry pinches/breaks), so foam appears on the actual steep crests — a
    // base presence in any sea, ramped up when the forecast reports a rough sea.
    // This is the big "alive ocean" cue (driven by the wave shape, not a guess).
    let breaking = smoothstep(0.5, 0.92, crest) * (0.4 + 0.9 * water.whitecap);
    let foam_lit = vec3<f32>(0.95, 0.97, 1.0) * (0.45 + 0.55 * water.sun_intensity);
    col = mix(col, foam_lit, clamp(breaking, 0.0, 1.0));

    // Shoreline foam: a thin lively band where the water meets rising land.
    let foam_band = shoreline_foam(in.world_pos, water.time);
    col = mix(col, foam_lit, foam_band);

    // Opaque for now; P3 adds shallow-water transparency / refraction (revealing
    // the seabed) driven by the continuous shallowness field.
    let alpha = in.color.a * tile.tile_alpha;
    return vec4<f32>(col, alpha);
}
