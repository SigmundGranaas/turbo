// Procedural cloud overlay for radar precipitation / cloud-coverage data.
//
// Single fragment pass, no compute, no 3D textures — mobile friendly.
// The input is a low-res "blocky" radar grid (R = precip, G = coverage).
// We smooth it, grow procedural cloud detail on top with domain-warped
// fBm + Perlin-Worley (the Horizon Zero Dawn "Nubis" density model
// collapsed to a flat field), fake volumetric lighting from the density
// gradient, and tint by rain intensity (light cumulus -> charcoal storm).
//
// Output is premultiplied RGBA so it composites straight onto the map.

struct Uniforms {
    resolution : vec2<f32>,
    time       : f32,   // seconds, drives drift + boil
    blend      : f32,   // crossfade radarA -> radarB, 0..1
    wind       : vec2<f32>,
    sun_dir    : vec2<f32>, // 2D sun azimuth for fake self-shadowing
    map_scale  : f32,   // noise frequency vs. screen
    erosion    : f32,   // high-freq edge erosion strength
    softness   : f32,   // alpha edge width
    intensity  : f32,   // overall opacity
    parallax   : f32,   // uniform fallback sweep (offscreen approx) when use_ray=0
    sun_elev   : f32,   // sun elevation 0=grazing .. 1=overhead
    extinction : f32,   // view-ray extinction
    light_ext  : f32,   // light-ray extinction (shadow strength)
    debug_view : u32,   // 0 = production; >0 = output an internal stage (AOV)
    use_ray    : u32,   // 1 = reconstruct the real per-pixel camera ray (pitch parallax)
    cloud_alt_base : f32, // cloud layer bottom altitude, world units
    cloud_alt_top  : f32, // cloud layer top altitude, world units
    inv_view_proj : mat4x4<f32>, // clip → world, for per-pixel ray reconstruction
    world_to_field : vec2<f32>, // world (mercator) → field-uv scale (= 1/radar_span)
    // Screen-uv → FIELD-uv affine, so the cloud field is locked to the WORLD
    // (geography) rather than the screen: panning/zooming the map moves and
    // scales the clouds with the terrain. `fuv = fuv_origin + uv.x*fuv_dx +
    // uv.y*fuv_dy`. The map fills these from its camera + the radar's geo box;
    // the offscreen/golden path leaves them at identity (origin=0, dx=(1,0),
    // dy=(0,1)) so `fuv == uv` and the screen-locked look is byte-identical.
    fuv_origin : vec2<f32>,
    fuv_dx     : vec2<f32>,
    fuv_dy     : vec2<f32>,
};

@group(0) @binding(0) var<uniform> U : Uniforms;
@group(0) @binding(1) var radar_a : texture_2d<f32>;
@group(0) @binding(2) var radar_b : texture_2d<f32>;
@group(0) @binding(3) var radar_s : sampler;
// Precomputed tileable 3D noise (R = Perlin-Worley base, G = hi-freq Worley
// detail, B = mid). Sampled with Repeat addressing — replaces the per-step
// analytic noise so the volumetric march is cheap enough for a mobile GPU.
@group(0) @binding(4) var noise_vol : texture_3d<f32>;
@group(0) @binding(5) var noise_samp : sampler;

struct VsOut {
    @builtin(position) pos : vec4<f32>,
    @location(0) uv : vec2<f32>,
};

@vertex
fn vs(@builtin(vertex_index) vi : u32) -> VsOut {
    // Fullscreen triangle.
    var p = array<vec2<f32>, 3>(
        vec2<f32>(-1.0, -1.0),
        vec2<f32>( 3.0, -1.0),
        vec2<f32>(-1.0,  3.0),
    );
    var out : VsOut;
    let c = p[vi];
    out.pos = vec4<f32>(c, 0.0, 1.0);
    out.uv = c * vec2<f32>(0.5, -0.5) + vec2<f32>(0.5, 0.5);
    return out;
}

// --- Hashing (Dave Hoskins, "Hash without Sine" — stable on mobile) ---

fn hash21(p_in : vec2<f32>) -> f32 {
    var p3 = fract(vec3<f32>(p_in.xyx) * 0.1031);
    p3 += dot(p3, p3.yzx + 33.33);
    return fract((p3.x + p3.y) * p3.z);
}

fn hash22(p_in : vec2<f32>) -> vec2<f32> {
    var p3 = fract(vec3<f32>(p_in.xyx) * vec3<f32>(0.1031, 0.1030, 0.0973));
    p3 += dot(p3, p3.yzx + 33.33);
    return fract((p3.xx + p3.yz) * p3.zy);
}

// --- Gradient (Perlin-style) noise, quintic fade ---

fn grad_noise(p : vec2<f32>) -> f32 {
    let i = floor(p);
    let f = fract(p);
    let u = f * f * f * (f * (f * 6.0 - 15.0) + 10.0);
    let ga = normalize(hash22(i + vec2<f32>(0.0, 0.0)) * 2.0 - 1.0);
    let gb = normalize(hash22(i + vec2<f32>(1.0, 0.0)) * 2.0 - 1.0);
    let gc = normalize(hash22(i + vec2<f32>(0.0, 1.0)) * 2.0 - 1.0);
    let gd = normalize(hash22(i + vec2<f32>(1.0, 1.0)) * 2.0 - 1.0);
    let va = dot(ga, f - vec2<f32>(0.0, 0.0));
    let vb = dot(gb, f - vec2<f32>(1.0, 0.0));
    let vc = dot(gc, f - vec2<f32>(0.0, 1.0));
    let vd = dot(gd, f - vec2<f32>(1.0, 1.0));
    let n = mix(mix(va, vb, u.x), mix(vc, vd, u.x), u.y);
    return n * 0.5 + 0.5;
}

const ROT = mat2x2<f32>(0.80, 0.60, -0.60, 0.80);

fn fbm4(p_in : vec2<f32>) -> f32 {
    var p = p_in;
    var amp = 0.5;
    var sum = 0.0;
    var norm = 0.0;
    for (var i = 0; i < 4; i = i + 1) {
        sum += amp * grad_noise(p);
        norm += amp;
        amp *= 0.5;
        p = ROT * p * 2.0;
    }
    return sum / norm;
}

// --- Worley / cellular noise → rounded "cauliflower" billows ---
// F1 nearest-feature distance over a jittered grid. `1 - F1` peaks at the
// feature points, giving the rounded lumps that read as puffy cumulus from
// above (pure fBm can only make wispy turbulence, never round puffs).
fn worley(p : vec2<f32>) -> f32 {
    let ip = floor(p);
    let fp = fract(p);
    var d = 8.0;
    for (var j = -1; j <= 1; j = j + 1) {
        for (var i = -1; i <= 1; i = i + 1) {
            let g = vec2<f32>(f32(i), f32(j));
            let o = hash22(ip + g);
            let r = g + o - fp;
            d = min(d, dot(r, r));
        }
    }
    return sqrt(d);
}

// Fractal billow noise in ~`0..1`: stacked inverted-Worley octaves. Big
// lumps with smaller lumps riding on them — the self-similar puff cluster.
fn billow_fbm(p_in : vec2<f32>) -> f32 {
    var p = p_in;
    var amp = 0.6;
    var sum = 0.0;
    var norm = 0.0;
    for (var i = 0; i < 3; i = i + 1) {
        sum += amp * (1.0 - worley(p));
        norm += amp;
        amp *= 0.5;
        p = ROT * p * 2.0 + vec2<f32>(11.3, 7.7);
    }
    return clamp(sum / norm, 0.0, 1.0);
}

// Linear remap of `x` from [a,b] into [c,d] (Nubis' workhorse for combining
// noise + coverage). Unclamped — callers clamp where they need 0..1.
fn remap(x : f32, a : f32, b : f32, c : f32, d : f32) -> f32 {
    return c + (x - a) * (d - c) / (b - a);
}

// Perlin–Worley base cloud noise (Schneider/Guerrilla). Take Perlin fBm and
// remap its low end UP toward the Worley-billow value: the billows fill in
// the round, puffy bases while the Perlin keeps natural variation on top —
// neither pure-Perlin (too wispy) nor pure-Worley (too cellular) alone.
fn perlin_worley(p : vec2<f32>) -> f32 {
    let perlin = fbm4(p);
    let billow = billow_fbm(p);
    return clamp(remap(perlin, billow - 1.0, 1.0, 0.0, 1.0), 0.0, 1.0);
}

// --- TRUE 3D noise: the volumetric basis. The density varies in x, y AND z,
// so the light march samples genuinely different cloud along the sun ray →
// real internal shadows + volume (not a 2D shape extruded into a slab). ---

fn hash13(p_in : vec3<f32>) -> f32 {
    var p3 = fract(p_in * 0.1031);
    p3 += dot(p3, p3.zyx + 31.32);
    return fract((p3.x + p3.y) * p3.z);
}

// Value noise in 3D, quintic-faded trilinear interpolation.
fn noise3(p : vec3<f32>) -> f32 {
    let i = floor(p);
    let f = fract(p);
    let u = f * f * f * (f * (f * 6.0 - 15.0) + 10.0);
    let c000 = hash13(i + vec3<f32>(0.0, 0.0, 0.0));
    let c100 = hash13(i + vec3<f32>(1.0, 0.0, 0.0));
    let c010 = hash13(i + vec3<f32>(0.0, 1.0, 0.0));
    let c110 = hash13(i + vec3<f32>(1.0, 1.0, 0.0));
    let c001 = hash13(i + vec3<f32>(0.0, 0.0, 1.0));
    let c101 = hash13(i + vec3<f32>(1.0, 0.0, 1.0));
    let c011 = hash13(i + vec3<f32>(0.0, 1.0, 1.0));
    let c111 = hash13(i + vec3<f32>(1.0, 1.0, 1.0));
    let x00 = mix(c000, c100, u.x);
    let x10 = mix(c010, c110, u.x);
    let x01 = mix(c001, c101, u.x);
    let x11 = mix(c011, c111, u.x);
    return mix(mix(x00, x10, u.y), mix(x01, x11, u.y), u.z);
}

const ROT3 = mat3x3<f32>(0.00, 0.80, 0.60, -0.80, 0.36, -0.48, -0.60, -0.48, 0.64);

fn fbm3(p_in : vec3<f32>) -> f32 {
    var p = p_in;
    var amp = 0.5;
    var sum = 0.0;
    var norm = 0.0;
    for (var i = 0; i < 4; i = i + 1) {
        sum += amp * noise3(p);
        norm += amp;
        amp *= 0.5;
        p = ROT3 * p * 2.0;
    }
    return sum / norm;
}

// (3D Worley/billow/Perlin-Worley are now precomputed into the noise texture
// and sampled in `cloud_density`; `fbm3`/`noise3` remain for `cloud_type`.)

// Smoothstep-bilinear sampling: de-blocks the radar grid for ~1 tap.
fn sample_radar(tex : texture_2d<f32>, uv : vec2<f32>) -> vec2<f32> {
    let dim = vec2<f32>(textureDimensions(tex));
    var p = uv * dim - 0.5;
    let i = floor(p);
    var f = fract(p);
    f = f * f * (3.0 - 2.0 * f);
    let uv2 = (i + 0.5 + f) / dim;
    let texel = textureSampleLevel(tex, radar_s, uv2, 0.0);
    return texel.rg; // (precip, coverage)
}

// Crossfaded radar value, with a slight wind advection per side so rain
// cells appear to travel during the A->B transition.
fn radar_value(uv : vec2<f32>) -> vec2<f32> {
    let drift = U.wind * 0.012;
    let a = sample_radar(radar_a, uv - drift * U.blend);
    let b = sample_radar(radar_b, uv + drift * (1.0 - U.blend));
    let t = smoothstep(0.0, 1.0, U.blend);
    return mix(a, b, t);
}

// Cloud "height" field in ~`0..1` — a proxy for how tall/thick the cloud
// bulges up toward a viewer looking straight DOWN. Warped fractal billows:
// fBm warps the domain (natural drift/curl) and Worley billows give the
// rounded cauliflower lumps that read as 3D cumulus from above. This is
// the heightfield the lighting takes its normal + cast shadows from.
fn cloud_height(uv : vec2<f32>) -> f32 {
    let drift = U.wind * U.time * 0.02;
    let evo = U.time * 0.05;
    let p = uv * U.map_scale + drift;
    let q = vec2<f32>(
        fbm4(p + vec2<f32>(0.0, evo)),
        fbm4(p + vec2<f32>(5.2, 1.3) - evo),
    );
    // Warp the billow domain → clustered, organic puffs, not a regular
    // cellular grid. Billows give the round tops; a touch of fBm softens.
    let warped = p * 0.85 + 2.2 * q;
    let lumps = billow_fbm(warped);
    let soft = fbm4(p + 3.0 * q);
    return clamp(lumps * 0.78 + soft * 0.22, 0.0, 1.0);
}

// Cloud layer geometry (normalised slab, z in 0..1, viewer looks down -z).
const CLOUD_BASE : f32 = 0.12;

// Number of vertical noise cells across the cloud layer — enough that the
// field has real top-to-bottom structure (so the light march sees different
// cloud as it climbs toward the sun), not a 2D shape extruded.
const VHEIGHT : f32 = 3.0;

// MET coverage as a SMOOTH regional seed. The 64×42 radar grid is blurred
// over several taps so its cell structure never imprints as a checkerboard —
// the grid only biases *where* cloud roughly is; all visible structure is
// procedural.
fn coverage_seed(uv : vec2<f32>) -> f32 {
    let r = 0.018;
    var s = radar_value(uv).g * 0.36;
    s += radar_value(uv + vec2<f32>(r, 0.0)).g * 0.16;
    s += radar_value(uv - vec2<f32>(r, 0.0)).g * 0.16;
    s += radar_value(uv + vec2<f32>(0.0, r)).g * 0.16;
    s += radar_value(uv - vec2<f32>(0.0, r)).g * 0.16;
    return s;
}

// Likewise a smoothed precip seed (drives storm size + height).
fn precip_seed(uv : vec2<f32>) -> f32 {
    let r = 0.018;
    var s = radar_value(uv).r * 0.4;
    s += radar_value(uv + vec2<f32>(r, 0.0)).r * 0.15;
    s += radar_value(uv - vec2<f32>(r, 0.0)).r * 0.15;
    s += radar_value(uv + vec2<f32>(0.0, r)).r * 0.15;
    s += radar_value(uv - vec2<f32>(0.0, r)).r * 0.15;
    return s;
}

// Per-region "cloud type" in 0..1 — a LOW-frequency field, biased by
// precipitation, that gives clouds a variety of PRIMARY shapes instead of
// one uniform height. 0 = flat low stratus, ~0.5 = rounded cumulus, 1 =
// towering cumulonimbus. Precip pulls it UP (storms tower) but the procedural
// field keeps every cell's actual form unique. (Nubis carries this in the
// weather map's B channel; we synthesise it procedurally + from MET precip.)
fn cloud_type(uv : vec2<f32>) -> f32 {
    let drift = U.wind * U.time * 0.02;
    let p = (uv * U.map_scale + drift) * 0.22; // much coarser than the detail
    let big = fbm3(vec3<f32>(p, 13.0));
    let precip = precip_seed(uv); // wetter → taller storm cells
    return clamp(big * 1.2 - 0.12 + precip * 0.85, 0.0, 1.0);
}

// Vertical density gradient for a cloud of type `ct` at normalised altitude
// `z`. The type sets the cloud TOP (flat→towering) and the gradient shape:
// rounded heavy base, taper to the top; taller types taper more gently
// (anvil-ish). This is what produces height variety across the field.
fn height_gradient(z : f32, ct : f32) -> f32 {
    let base = 0.05;
    let top = mix(0.34, 0.96, ct); // flat stratus .. towering cumulonimbus
    if (z < base || z > top) {
        return 0.0;
    }
    let hn = (z - base) / max(top - base, 1e-3);
    let bottom = smoothstep(0.0, 0.18, hn);
    let taper = 1.0 - smoothstep(mix(0.5, 0.82, ct), 1.0, hn);
    return bottom * taper;
}

// TRUE 3D extinction density at a sample. `z` ∈ 0..1 is normalised altitude.
// The visible cloud is ENTIRELY procedural; MET coverage is only a smooth
// regional *seed* biasing the threshold. Structured as Nubis does it:
//  - a LOW-frequency primary 3D shape (the big cloud forms) × a type-driven
//    height gradient → variety of primary shapes (flat → towering),
//  - HIGH-frequency 3D Worley billows erode detail (the small puffs) AROUND
//    that primary shape.
fn cloud_density(uv : vec2<f32>, z : f32) -> f32 {
    if (z <= 0.0 || z >= 1.0) {
        return 0.0;
    }
    let seed = coverage_seed(uv);
    if (seed < 0.03) {
        return 0.0;
    }
    let drift = U.wind * U.time * 0.02;
    let ct = cloud_type(uv);
    // Precip pulls the primary shape markedly LARGER + denser (storm cells
    // are big towering masses), while the procedural field keeps each cell's
    // actual form unique.
    let big_seed = clamp(seed + precip_seed(uv) * 0.7, 0.0, 1.0);

    // Primary big-shape field — low frequency, so it sets the major forms.
    // Sampled from the precomputed tileable 3D noise (R = Perlin-Worley).
    let pp = vec3<f32>((uv * U.map_scale + drift) * 0.55, z * VHEIGHT * 0.55 + U.time * 0.04);
    var primary = textureSampleLevel(noise_vol, noise_samp, pp * 0.5, 0.0).r;
    primary = clamp(remap(primary, 1.0 - big_seed, 1.0, 0.0, 1.0), 0.0, 1.0);
    if (primary <= 0.001) {
        return 0.0;
    }

    // Type-driven vertical gradient → the PRIMARY shape's height varies.
    var d = primary * height_gradient(z, ct);
    if (d <= 0.001) {
        return 0.0;
    }

    // High-frequency detail erodes small puffs around the primary shape
    // (stronger toward the top), sampled from the noise texture's G channel
    // (hi-freq Worley billow) at a finer scale; then crisp the fringe. Kept
    // coarser + gentler than before: the previous high frequency / strong
    // erosion turned the (now darker) shadow interior into salt-and-pepper
    // speckle rather than rounded billows.
    let dc = vec3<f32>(uv * U.map_scale + drift, z * VHEIGHT) * 0.62 + vec3<f32>(3.1, 1.7, 2.3);
    let detail = textureSampleLevel(noise_vol, noise_samp, dc, 0.0).g;
    d = clamp(remap(d, (1.0 - detail) * mix(0.18, 0.42, z), 1.0, 0.0, 1.0), 0.0, 1.0);
    d = clamp((d - 0.08) / 0.92, 0.0, 1.0);
    return d * 2.7;
}

// Base albedo from rain intensity. Cloud stays BRIGHT WHITE until the rain
// is genuinely moderate — the old low-end boost greyed almost everything
// down to dirty smoke ("spilled milk"). Now only real precipitation darkens
// it: white cumulus → grey → charcoal storm core, with a dead-zone at the
// bottom so fair-weather cloud reads clean and white.
fn rain_albedo(precip : f32) -> vec3<f32> {
    let c_dry   = vec3<f32>(0.98, 0.99, 1.00); // bright cumulus
    let c_light = vec3<f32>(0.66, 0.69, 0.77); // light rain grey
    let c_mid   = vec3<f32>(0.32, 0.35, 0.43); // steady rain
    let c_heavy = vec3<f32>(0.09, 0.10, 0.14); // charcoal storm core
    let p = clamp(precip, 0.0, 1.0);
    var c = mix(c_dry, c_light, smoothstep(0.15, 0.45, p));
    c = mix(c, c_mid, smoothstep(0.45, 0.70, p));
    c = mix(c, c_heavy, smoothstep(0.70, 0.95, p));
    return c;
}

// All intermediate quantities of one shaded fragment. Computed once in
// `shade()` so the production path and the diagnostic AOV branch see
// exactly the same numbers — the debug views never lie about what the
// final pixel was built from.
struct Shade {
    precip   : f32,
    coverage : f32,
    field    : f32, // billow top-surface height (AOV only)
    density  : f32, // = final opacity (column-integrated extinction)
    light    : f32, // mean scattered luminance (AOV only)
    alpha    : f32, // final composited opacity
    albedo   : vec3<f32>, // rain-coloured albedo (deferred — clouds are white for now)
    col      : vec3<f32>, // final premultiplied scattered colour
};

// Step counts dropped from 28/5 to cut the fullscreen-march GPU cost ~40% on
// mobile (it stacks on the terrain mesh + tiles in 3D mode — a likely
// device-lost/OOM crash source). With the translucent overlay the slightly
// coarser march isn't noticeable.
const PRIMARY_STEPS : i32 = 16;
const LIGHT_STEPS : i32 = 4;

fn luminance_lin(c : vec3<f32>) -> f32 {
    return dot(c, vec3<f32>(0.2126, 0.7152, 0.0722));
}

// Per-pixel parallax shift in FIELD-uv at the slab top and bottom, packed as
// (shift_top.xy, shift_bot.xy). `scr` is the SCREEN uv (the ray comes from the
// camera, not the world-locked field). The cloud sits at altitude above the
// ground the basemap drew at this pixel; following the real view ray, the slab
// is crossed at a different world (x,y) than the ground point. That world
// offset is converted to field-uv via `world_to_field` (= 1/radar_span) and the
// march lerps top→bottom, so a pitched map rakes through the volume and reveals
// the puff SIDES. use_ray=0 → flat top-down (no shift).
fn view_parallax(scr : vec2<f32>) -> vec4<f32> {
    if (U.use_ray != 1u) {
        return vec4<f32>(0.0);
    }
    let ndc = vec2<f32>(scr.x * 2.0 - 1.0, 1.0 - 2.0 * scr.y);
    let pn = U.inv_view_proj * vec4<f32>(ndc, 0.0, 1.0);
    let pf = U.inv_view_proj * vec4<f32>(ndc, 1.0, 1.0);
    let ro = pn.xyz / pn.w;
    let rd = normalize(pf.xyz / pf.w - ro);
    if (rd.z >= -1.0e-4) {
        return vec4<f32>(0.0);
    }
    let g = ro + rd * (-ro.z / rd.z); // ray ∩ ground (z=0)
    let p_top = ro + rd * ((U.cloud_alt_top - ro.z) / rd.z);
    let p_bot = ro + rd * ((U.cloud_alt_base - ro.z) / rd.z);
    var st = (p_top.xy - g.xy) * U.world_to_field;
    var sb = (p_bot.xy - g.xy) * U.world_to_field;
    // Clamp the shift so a near-horizon ray can't rake wildly across the field.
    // Near the top of a tilted view rd.z -> 0 so rd.xy/rd.z blows up; without a
    // tight cap the march sweeps several puffs and averages the cloud/gap
    // structure into a flat white wash. ~0.10 keeps even the grazing band under
    // one puff (a puff is ~0.125 of the field at map_scale 8).
    let m = 0.10;
    if (length(st) > m) { st = normalize(st) * m; }
    if (length(sb) > m) { sb = normalize(sb) * m; }
    return vec4<f32>(st, sb);
}

// Volumetric raymarch (Nubis-style): march a vertical view ray DOWN through
// the cloud slab accumulating Beer–Lambert extinction (→ translucency &
// soft edges), and at each lit sample take a short cone march toward the sun
// for self-shadowing, fed through a multi-scatter energy approximation
// (octaves of Beer) plus a powder term and a height-graded sky ambient.
// Returns premultiplied (scattered colour, alpha).
fn render_volume(uv : vec2<f32>, scr : vec2<f32>, lum_out : ptr<function, f32>) -> vec4<f32> {
    // LOW sun: a near-horizon light rakes across the cloud tops so puffs
    // throw long shadows over their neighbours — the drama you only get at
    // a low sun angle is what reads as 3D form from straight above.
    let sun = normalize(vec3<f32>(normalize(U.sun_dir), U.sun_elev));

    // Direct sunlight as a SPECTRUM that the sun travels through as it sets,
    // not one flat orange: warm white high up → gold → orange → deep red as
    // it grazes the horizon (Rayleigh reddening of the low-angle beam).
    let e = clamp(U.sun_elev, 0.0, 1.0);
    let c_red = vec3<f32>(1.40, 0.44, 0.30);
    let c_orange = vec3<f32>(1.40, 0.72, 0.44);
    let c_gold = vec3<f32>(1.34, 0.98, 0.72);
    let c_white = vec3<f32>(1.12, 1.10, 1.05);
    var sun_col = mix(c_red, c_orange, smoothstep(0.04, 0.14, e));
    sun_col = mix(sun_col, c_gold, smoothstep(0.14, 0.26, e));
    sun_col = mix(sun_col, c_white, smoothstep(0.26, 0.46, e));

    // Sky ambient (fills the shadows) as a vertical SPECTRUM too — sampled by
    // cloud altitude z. Day: blue zenith over paler horizon. Dusk: a deep
    // blue-purple zenith grading down through mauve to a warm glowing horizon,
    // so shadowed undersides catch the warm horizon and tops stay cool.
    let sunset = 1.0 - smoothstep(0.10, 0.42, U.sun_elev);
    // sky_bot = horizon, sky_top = zenith; the per-sample mix(sky_bot, sky_top, z)
    // sweeps warm-horizon → cool-zenith (through mauve) over cloud altitude.
    let sky_top = mix(vec3<f32>(0.60, 0.69, 0.86), vec3<f32>(0.20, 0.24, 0.50), sunset);
    let sky_bot = mix(vec3<f32>(0.42, 0.46, 0.55), vec3<f32>(0.58, 0.44, 0.50), sunset);

    let dz = 1.0 / f32(PRIMARY_STEPS);
    let ext = U.extinction; // view-ray extinction (opacity build-up)
    // Light march reaches a couple of puff widths toward the low sun so a
    // puff shadows its NEIGHBOURS (crisp inter-puff shadows), not itself.
    let lstep = 2.4 / (U.map_scale * f32(LIGHT_STEPS));
    let lext = U.light_ext; // light extinction (shadow strength)

    // Per-pixel parallax shift (top & bottom of the slab), in field-uv, from
    // the real camera ray (screen uv). Zero when the map is top-down.
    let sp = view_parallax(scr);
    let shift_top = sp.xy;
    let shift_bot = sp.zw;

    var transmit = 1.0;
    var scattered = vec3<f32>(0.0);
    var lum_sum = 0.0;
    var lum_w = 0.0;

    for (var i = 0; i < PRIMARY_STEPS; i = i + 1) {
        if (transmit < 0.02) {
            break;
        }
        let z = 1.0 - (f32(i) + 0.5) * dz;
        let suv = uv + mix(shift_bot, shift_top, z);
        let d = cloud_density(suv, z);
        if (d <= 0.002) {
            continue;
        }

        // Cone march toward the sun → optical depth of cloud above us.
        var ld = 0.0;
        for (var j = 1; j <= LIGHT_STEPS; j = j + 1) {
            let t = lstep * f32(j);
            ld += cloud_density(suv + sun.xy * t, z + sun.z * t);
        }

        // Beer–Lambert light transmittance toward the sun. `ld` is the cloud
        // optical depth between this sample and the sun, so the deeper/thicker
        // the cloud above it, the less light reaches it — that IS the internal
        // shadow, and it's what makes thick cloud read dark. Three multi-
        // scatter octaves (decaying extinction/contribution) add a little fill
        // so the darkest interior is deep grey, not pure black — no flat floor.
        let optical = ld * lstep * lext;
        // Tighter multi-scatter: the big fill octave was washing the shadows
        // flat (milky, no form). Pull it down so thick cloud goes genuinely
        // dark underneath — that sun→shade contrast is what reads as 3D volume.
        // Tighter still: the fill octaves were lifting the undersides into a
        // uniform bright mass ("spilled milk"). Pull them down hard so thick
        // cloud goes genuinely dark beneath — the deep sun→shade contrast is
        // what reads as 3D puffs from above rather than a flat white wash.
        let ms = exp(-optical)
            + 0.09 * exp(-optical * 0.25)
            + 0.02 * exp(-optical * 0.06);
        // Powder: in-scatter probability darkens thin edges facing the light.
        // Lower floor → crisper, darker billow fringes (more shape definition).
        let powder = mix(0.28, 1.0, 1.0 - exp(-d * 2.2));
        let lit = sun_col * ms * powder;
        // Small sky ambient so the deepest shadow stays a readable grey-blue
        // without flattening the form — kept low so it doesn't re-milk the
        // shadows we just darkened.
        let ambient = mix(sky_bot, sky_top, z) * 0.10;
        // Cooler shadows at sunset: the away-from-sun (low light) parts are
        // lit only by the cool upper sky → push them blue-purple, for a strong
        // warm-sun / cool-shade split rather than a uniform orange wash.
        let shade_amt = (1.0 - clamp(ms, 0.0, 1.0)) * sunset;
        let cool_shadow = vec3<f32>(0.30, 0.38, 0.66) * shade_amt * 0.20;
        let scatter = lit + ambient + cool_shadow;

        let st = d * dz * ext;
        let absorb = 1.0 - exp(-st);
        scattered += transmit * absorb * scatter;
        transmit = transmit * exp(-st);

        lum_sum += luminance_lin(scatter) * absorb;
        lum_w += absorb;
    }

    *lum_out = select(0.0, lum_sum / lum_w, lum_w > 0.0);
    return vec4<f32>(scattered, clamp((1.0 - transmit) * U.intensity, 0.0, 1.0));
}

// Map a screen-uv to the FIELD-uv the cloud is sampled in. Identity by default
// (offscreen/golden), and the world-locked affine when the map sets it — so the
// clouds pan and zoom with the terrain instead of being painted on the glass.
fn field_uv(uv : vec2<f32>) -> vec2<f32> {
    return U.fuv_origin + uv.x * U.fuv_dx + uv.y * U.fuv_dy;
}

fn shade(uv : vec2<f32>) -> Shade {
    let fuv = field_uv(uv);
    let rv = radar_value(fuv);
    var lum = 0.0;
    let vol = render_volume(fuv, uv, &lum);

    var s : Shade;
    s.precip = rv.r;
    s.coverage = rv.g;
    s.field = cloud_height(fuv);
    s.density = vol.a;
    s.light = lum;
    s.alpha = vol.a;
    s.albedo = rain_albedo(rv.r);
    s.col = vol.rgb;
    return s;
}

// Pack a scalar as an opaque greyscale pixel for an AOV readback. Written
// through the sRGB target, so the harness sRGB-decodes it back to linear.
fn gray(v : f32) -> vec4<f32> {
    let c = clamp(v, 0.0, 1.0);
    return vec4<f32>(c, c, c, 1.0);
}

// Diagnostic "arbitrary output variable" views — each isolates one stage
// of the pipeline so a human (and the fidelity metrics) can see where the
// look comes from. Selected by `U.debug_view`; `0` is the real overlay.
fn debug_aov(s : Shade) -> vec4<f32> {
    switch (U.debug_view) {
        case 1u: { return gray(s.precip); }                       // radar precip (de-blocked)
        case 2u: { return gray(s.coverage); }                     // radar coverage (de-blocked)
        case 3u: { return gray(s.field); }                        // raw cloud_field
        case 4u: { return gray(s.density); }                      // thresholded density
        case 5u: { return gray(s.light / 1.8); }                  // relief lighting term
        case 6u: { return gray(s.alpha); }                        // final opacity
        case 7u: { return vec4<f32>(s.albedo, 1.0); }             // unlit rain albedo (opaque)
        default: { return vec4<f32>(s.col, s.alpha); }            // production composite (premultiplied)
    }
}

@fragment
fn fs(in : VsOut) -> @location(0) vec4<f32> {
    // Parallax-debug AOV (code 8): visualise the per-pixel shift directly
    // (R = shift.x, G = shift.y, biased to grey=zero), so a broken ray reads
    // obviously wrong. Skips the expensive march.
    if (U.debug_view == 8u) {
        let sp = view_parallax(in.uv);
        return vec4<f32>(sp.x * 2.5 + 0.5, sp.y * 2.5 + 0.5, 0.5, 1.0);
    }

    let s = shade(in.uv);

    if (U.debug_view > 0u) {
        return debug_aov(s);
    }

    // Production path: empty sky → fully transparent. `s.col` is already
    // premultiplied scattered radiance from the volume march.
    if (s.coverage <= 0.02 || s.alpha <= 0.004) {
        return vec4<f32>(0.0);
    }
    return vec4<f32>(s.col, s.alpha);
}
