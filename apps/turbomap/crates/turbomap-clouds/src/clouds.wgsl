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
    world_to_uv : f32,  // world units → screen-uv scale at the ground plane
    _pad0      : f32,
    _pad1      : f32,
    _pad2      : f32,
};

@group(0) @binding(0) var<uniform> U : Uniforms;
@group(0) @binding(1) var radar_a : texture_2d<f32>;
@group(0) @binding(2) var radar_b : texture_2d<f32>;
@group(0) @binding(3) var radar_s : sampler;

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

// 3D extinction density at a world point — this is what makes the clouds
// VOLUMETRIC rather than a flat card: a real σₜ sampled through a slab, so
// the raymarch can integrate translucency, soft edges and light transport.
//
// Construction: radar coverage says WHERE cloud exists; the billow height
// field is the bumpy TOP surface of the puffs; the layer is filled solid
// from the base up to that surface (rounded off top and bottom); a
// height-varying fBm erodes wispy 3D detail. Returns ~0..1.
fn cloud_density(uv : vec2<f32>, z : f32) -> f32 {
    let cov = radar_value(uv).g;
    if (cov < 0.02) {
        return 0.0;
    }
    let drift = U.wind * U.time * 0.02;
    let p = uv * U.map_scale + drift;

    // Nubis base shape: Perlin–Worley, then apply coverage with a Remap so
    // only noise above the (1−coverage) floor survives, rescaled to 0..1.
    // This is the crisp coverage-driven silhouette (broken vs. solid).
    var base = perlin_worley(p);
    base = clamp(remap(base, 1.0 - cov, 1.0, 0.0, 1.0), 0.0, 1.0);
    if (base <= 0.001) {
        return 0.0;
    }

    // Denser puffs stand TALLER, so their domed tops cast read-able shadows
    // on neighbours when seen from above (the key 3D cue top-down).
    let cloud_top = CLOUD_BASE + 0.05 + base * 0.85;
    if (z < CLOUD_BASE || z > cloud_top) {
        return 0.0;
    }
    let hn = (z - CLOUD_BASE) / max(cloud_top - CLOUD_BASE, 1e-3); // 0..1 in slab

    // Cumulus height gradient: rounded heavy base, billowy taper to the top.
    let grad = smoothstep(0.0, 0.12, hn) * (1.0 - smoothstep(0.45, 1.0, hn));
    var d = base * grad;
    if (d <= 0.001) {
        return 0.0;
    }

    // High-frequency Worley erosion carves the cauliflower edges; z in the
    // coords makes it a 3D field (varies with height), and we erode harder
    // toward the top where cumulus go wispy.
    let detail = billow_fbm(p * 3.1 + vec2<f32>(z * 4.0, drift.y));
    let ero = mix(0.36, 0.70, hn) * detail;
    d = clamp(remap(d, ero, 1.0, 0.0, 1.0), 0.0, 1.0);
    return d * 2.0;
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

const PRIMARY_STEPS : i32 = 32;
const LIGHT_STEPS : i32 = 6;

fn luminance_lin(c : vec3<f32>) -> f32 {
    return dot(c, vec3<f32>(0.2126, 0.7152, 0.0722));
}

// Per-pixel parallax shift (screen-uv) at the slab top and bottom, packed as
// (shift_top.xy, shift_bot.xy). The cloud sits at altitude above the ground
// the basemap drew at this pixel; following the real view ray, the slab is
// crossed at a different (x,y) than the ground point. The march samples the
// field at this offset and lerps top→bottom, so a pitched map rakes through
// the volume and reveals the puff sides. use_ray=0 → flat top-down fallback.
fn view_parallax(uv : vec2<f32>) -> vec4<f32> {
    if (U.use_ray != 1u) {
        return vec4<f32>(vec2<f32>(0.0, -0.5) * U.parallax, vec2<f32>(0.0, 0.5) * U.parallax);
    }
    let ndc = vec2<f32>(uv.x * 2.0 - 1.0, 1.0 - 2.0 * uv.y);
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
    var st = (p_top.xy - g.xy) * U.world_to_uv;
    var sb = (p_bot.xy - g.xy) * U.world_to_uv;
    // Field is screen-locked → clamp the shift so it can't sample off it.
    let m = 0.20;
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
fn render_volume(uv : vec2<f32>, lum_out : ptr<function, f32>) -> vec4<f32> {
    // LOW sun: a near-horizon light rakes across the cloud tops so puffs
    // throw long shadows over their neighbours — the drama you only get at
    // a low sun angle is what reads as 3D form from straight above.
    let sun = normalize(vec3<f32>(normalize(U.sun_dir), U.sun_elev));

    // Lit cloud reads bright white; shadowed sides/crevices fall to a cool
    // sky-blue but stay clearly visible. Ambient is graded by height.
    let sun_col = vec3<f32>(1.15, 1.12, 1.06);
    let sky_top = vec3<f32>(0.74, 0.80, 0.92);
    let sky_bot = vec3<f32>(0.46, 0.52, 0.64);

    let dz = 1.0 / f32(PRIMARY_STEPS);
    let ext = U.extinction; // view-ray extinction (opacity build-up)
    // Light march reaches a couple of puff widths toward the low sun so a
    // puff shadows its NEIGHBOURS (crisp inter-puff shadows), not itself.
    let lstep = 2.4 / (U.map_scale * f32(LIGHT_STEPS));
    let lext = U.light_ext; // light extinction (shadow strength)

    // Per-pixel parallax shift (top & bottom of the slab), in screen-uv.
    let sp = view_parallax(uv);
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

        // Beer–Lambert shadow toward the sun, with a low multi-scatter floor
        // so shadowed sides genuinely darken (to a soft grey-blue, not black).
        let shadow = exp(-ld * lstep * lext);
        let ms = mix(0.42, 1.0, shadow);
        // Powder: in-scatter probability gently darkens light-facing edges.
        let powder = mix(0.85, 1.0, 1.0 - exp(-d * 2.0));
        let lit = sun_col * ms * powder;
        let ambient = mix(sky_bot, sky_top, z) * 0.6;
        let scatter = lit + ambient;

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

fn shade(uv : vec2<f32>) -> Shade {
    let rv = radar_value(uv);
    var lum = 0.0;
    let vol = render_volume(uv, &lum);

    var s : Shade;
    s.precip = rv.r;
    s.coverage = rv.g;
    s.field = cloud_height(uv);
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
