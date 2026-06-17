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
    debug_view : u32,   // 0 = production; >0 = output an internal stage (AOV)
    _pad0      : u32,
    _pad1      : u32,
    _pad2      : u32,
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
    let top_surf = cloud_height(uv); // billow height 0..1
    // Only billow that clears the coverage threshold becomes cloud; a NARROW
    // band separates the puffs into distinct lumps with real sky gaps rather
    // than one diffuse misty sheet. Square it for a denser, harder core.
    let mask = smoothstep(1.0 - cov, 1.0 - cov + 0.22, top_surf);
    if (mask <= 0.001) {
        return 0.0;
    }
    let body = mask * mask;
    // Puff top altitude: clearly taller where billow + coverage are high, so
    // tall puffs cast read-able shadows over their neighbours.
    let cloud_top = CLOUD_BASE + 0.05 + top_surf * cov * 0.95;
    // Vertical profile: ramp up off a soft base, round off under the top.
    let v = smoothstep(CLOUD_BASE, CLOUD_BASE + 0.05, z)
          * (1.0 - smoothstep(cloud_top - 0.14, cloud_top, z));
    if (v <= 0.001) {
        return 0.0;
    }
    // Wispy 3D erosion — z in the noise coords so the column varies with
    // height (cauliflower interior). Bites deep so gaps open between puffs.
    let drift = U.wind * U.time * 0.02;
    let ero = fbm4(uv * U.map_scale * 2.1 + drift + vec2<f32>(z * 3.0, U.time * 0.04));
    let d = body * v * smoothstep(0.25, 0.75, ero);
    return clamp(d * 1.6, 0.0, 1.0);
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

// Volumetric raymarch (Nubis-style): march a vertical view ray DOWN through
// the cloud slab accumulating Beer–Lambert extinction (→ translucency &
// soft edges), and at each lit sample take a short cone march toward the sun
// for self-shadowing, fed through a multi-scatter energy approximation
// (octaves of Beer) plus a powder term and a height-graded sky ambient.
// Returns premultiplied (scattered colour, alpha).
fn render_volume(uv : vec2<f32>, lum_out : ptr<function, f32>) -> vec4<f32> {
    let sun = normalize(vec3<f32>(normalize(U.sun_dir), 0.55));

    // Lit cloud reads near-white; shadowed sides/crevices fall to a cool
    // sky-blue. Ambient is graded by height (more sky light up top).
    let sun_col = vec3<f32>(1.00, 0.97, 0.92);
    let sky_top = vec3<f32>(0.62, 0.69, 0.84);
    let sky_bot = vec3<f32>(0.30, 0.35, 0.46);

    let dz = 1.0 / f32(PRIMARY_STEPS);
    let ext = 7.0;     // view-ray extinction (opacity build-up)
    let lstep = 0.10;  // light march step length
    let lext = 8.0;    // light extinction (shadow strength)

    var transmit = 1.0;
    var scattered = vec3<f32>(0.0);
    var lum_sum = 0.0;
    var lum_w = 0.0;

    for (var i = 0; i < PRIMARY_STEPS; i = i + 1) {
        if (transmit < 0.02) {
            break;
        }
        let z = 1.0 - (f32(i) + 0.5) * dz;
        let d = cloud_density(uv, z);
        if (d <= 0.002) {
            continue;
        }

        // Cone march toward the sun → optical depth of cloud above us.
        var ld = 0.0;
        for (var j = 1; j <= LIGHT_STEPS; j = j + 1) {
            let t = lstep * f32(j);
            ld += cloud_density(uv + sun.xy * t, z + sun.z * t);
        }

        // Beer–Lambert shadow toward the sun, with a multi-scatter floor so
        // deep shadow stays a soft grey-blue (not crushed black).
        let shadow = exp(-ld * lstep * lext);
        let ms = mix(0.32, 1.0, shadow);
        // Powder: in-scatter probability gently darkens light-facing edges.
        let powder = mix(0.78, 1.0, 1.0 - exp(-d * 2.0));
        let lit = sun_col * 0.95 * ms * powder;
        let ambient = mix(sky_bot, sky_top, z) * 0.4;
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
