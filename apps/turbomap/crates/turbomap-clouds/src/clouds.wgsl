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

// Cheaper 3-octave field used for lighting (gradient + shadow taps),
// so the expensive density model is only evaluated once per pixel.
fn fbm3(p_in : vec2<f32>) -> f32 {
    var p = p_in;
    var amp = 0.5;
    var sum = 0.0;
    var norm = 0.0;
    for (var i = 0; i < 3; i = i + 1) {
        sum += amp * grad_noise(p);
        norm += amp;
        amp *= 0.5;
        p = ROT * p * 2.0;
    }
    return sum / norm;
}

// --- Worley (inverted -> billowy "cauliflower" lumps) ---

fn worley(p : vec2<f32>) -> f32 {
    let ip = floor(p);
    let fp = fract(p);
    var d = 1.0;
    for (var y = -1; y <= 1; y = y + 1) {
        for (var x = -1; x <= 1; x = x + 1) {
            let g = vec2<f32>(f32(x), f32(y));
            let o = hash22(ip + g);
            let r = g + o - fp;
            d = min(d, dot(r, r));
        }
    }
    return sqrt(d);
}

fn remap(v : f32, lo0 : f32, hi0 : f32, lo1 : f32, hi1 : f32) -> f32 {
    return lo1 + (v - lo0) * (hi1 - lo1) / max(hi0 - lo0, 1e-4);
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

// Cloud "shape" field at a map point: domain-warped billowy fBm blended
// with inverted-Worley lumps. Returns ~0..1, with strong contrast so the
// cloud breaks into distinct puffs with gaps between them rather than a
// flat overcast sheet.
fn cloud_shape(p : vec2<f32>, evo : f32) -> f32 {
    // One-level domain warp — billowy, churning shapes.
    let q = vec2<f32>(
        fbm4(p + vec2<f32>(0.0, evo)),
        fbm4(p + vec2<f32>(5.2 + evo, 1.3)),
    );
    let base = fbm4(p + 2.4 * q);
    // Cauliflower lumps (single octave keeps the hot path cheap enough
    // for software rasterisation).
    let billow = 1.0 - worley(p * 1.8);
    return base * 0.58 + billow * 0.42;
}

// Full cloud density at a map uv: shape thresholded by coverage so there
// are real holes between puffs, then high-freq edge erosion.
fn density(uv : vec2<f32>, coverage : f32) -> f32 {
    if (coverage <= 0.001) {
        return 0.0;
    }
    let drift = U.wind * U.time * 0.03;
    let evo = U.time * 0.06;
    let p = uv * U.map_scale + drift;

    let shape = cloud_shape(p, evo);

    // Coverage sets a threshold: low coverage => only the densest lumps
    // survive (scattered puffs); high coverage => most of the field fills
    // but valleys still read as gaps. The 0.30 band keeps soft, rounded
    // cumulus edges instead of a hard cut.
    let thr = mix(0.78, 0.12, coverage);
    var d = smoothstep(thr, thr + 0.30, shape);

    // High-frequency erosion bites wispy detail into the edges only.
    let detail = 1.0 - worley(p * 6.0 + evo);
    d *= 1.0 - U.erosion * detail * (1.0 - d) * 0.9;
    return clamp(d, 0.0, 1.0);
}

// Cheap height proxy for lighting — evaluated ~7× per pixel for the
// normal + shadow taps, so it must stay light (plain fBm, no Worley).
// Squared lift makes puff tops dome upward for crisper self-shadowing.
fn height(uv : vec2<f32>, coverage : f32) -> f32 {
    let drift = U.wind * U.time * 0.03;
    let evo = U.time * 0.06;
    let p = uv * U.map_scale + drift;
    let s = fbm3(p + vec2<f32>(evo, 0.0));
    let lifted = smoothstep(mix(0.62, 0.18, coverage), 1.0, s);
    return lifted * lifted;
}

// Base albedo from rain intensity: dry cloud is bright white, drizzle
// greys it down, heavy rain drives it to near-black charcoal.
fn rain_albedo(precip : f32) -> vec3<f32> {
    let c_dry   = vec3<f32>(0.98, 0.99, 1.00); // bright cumulus
    let c_light = vec3<f32>(0.68, 0.71, 0.78); // light rain grey
    let c_mid   = vec3<f32>(0.34, 0.37, 0.45); // steady rain
    let c_heavy = vec3<f32>(0.09, 0.10, 0.14); // charcoal storm core
    let p = pow(clamp(precip, 0.0, 1.0), 0.75); // boost low end
    var c = mix(c_dry, c_light, smoothstep(0.02, 0.30, p));
    c = mix(c, c_mid, smoothstep(0.30, 0.60, p));
    c = mix(c, c_heavy, smoothstep(0.60, 1.0, p));
    return c;
}

@fragment
fn fs(in : VsOut) -> @location(0) vec4<f32> {
    let uv = in.uv;
    let rv = radar_value(uv);
    let precip = rv.r;
    let coverage = rv.g;

    let d = density(uv, coverage);
    if (d <= 0.001) {
        return vec4<f32>(0.0);
    }

    // Fake normal from the height-proxy gradient (analytic finite diff).
    // Small z => pronounced relief so puffs read as 3D domes.
    let e = 0.6 / U.map_scale;
    let hc = height(uv, coverage);
    let hx = height(uv + vec2<f32>(e, 0.0), coverage) - hc;
    let hy = height(uv + vec2<f32>(0.0, e), coverage) - hc;
    let n = normalize(vec3<f32>(-hx * 6.0, -hy * 6.0, 0.5));

    let sun = normalize(vec3<f32>(U.sun_dir, 0.62));
    let ndl = clamp(dot(n, sun), 0.0, 1.0);

    // Fake self-shadow: march toward the sun in map space, accumulate
    // height, attenuate Beer-style for soft directional darkening.
    var sh = 0.0;
    let step = normalize(U.sun_dir) * (1.3 / U.map_scale);
    var s = uv;
    for (var i = 0; i < 3; i = i + 1) {
        s += step;
        sh += height(s, coverage);
    }
    let shadow = exp(-sh * 1.1);

    // Compose lighting with real dynamic range: dark shadowed undersides,
    // bright sunlit tops, plus a silver-lining rim on thin edges.
    let ambient = 0.42;
    let direct = ndl * shadow * 1.35;
    let ao = mix(0.55, 1.0, smoothstep(0.0, 0.55, d));
    let rim = pow(1.0 - clamp(d, 0.0, 1.0), 3.0) * ndl * shadow;
    let light = (ambient + direct) * ao + rim * 0.6;

    // Cool shadow / warm highlight tint, applied around mid-grey.
    let albedo = rain_albedo(precip);
    let shadow_tint = vec3<f32>(0.78, 0.82, 0.94);
    let light_tint  = vec3<f32>(1.06, 1.03, 0.97);
    let tint = mix(shadow_tint, light_tint, clamp(light - 0.4, 0.0, 1.0));
    let col = clamp(albedo * light * tint, vec3<f32>(0.0), vec3<f32>(1.0));

    // Soft alpha: dissolve cloud edges, gate on coverage. Rain-bearing
    // cloud is optically thick, so push storms toward fully opaque
    // (otherwise the dark band lets the green map bleed through, reading
    // muddy instead of stormy).
    var alpha = smoothstep(0.03, 0.03 + U.softness, d);
    alpha *= smoothstep(0.0, 0.12, coverage);
    let storm = smoothstep(0.35, 0.8, precip) * smoothstep(0.02, 0.18, d);
    alpha = max(alpha, storm);
    alpha = clamp(alpha * U.intensity, 0.0, 0.98);

    // Premultiplied output.
    return vec4<f32>(col * alpha, alpha);
}
