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

// Continuous cloud-shape field in `0..1`: domain-warped fractal noise.
// The warp (fbm of an fbm offset) is what gives natural, billowy,
// wispy cloud forms — no Worley cells, so no round-blob "popcorn".
// This single field drives BOTH the silhouette and the lighting, so the
// shading always matches the shapes you see.
fn cloud_field(uv : vec2<f32>) -> f32 {
    let drift = U.wind * U.time * 0.02;
    let evo = U.time * 0.05;
    let p = uv * U.map_scale + drift;
    let q = vec2<f32>(
        fbm4(p + vec2<f32>(0.0, evo)),
        fbm4(p + vec2<f32>(5.2, 1.3) - evo),
    );
    // Warp strength 3.5 → turbulent, curdled cumulus rather than smooth
    // hills. One level keeps it affordable on mobile / software.
    return fbm4(p + 3.5 * q);
}

// Turn the continuous field into a soft cloud density for a given cloud
// coverage. Coverage drives the threshold the fractal field has to clear;
// the band is NARROW so the field's own valleys punch real sky gaps
// through the cloud (billows, not a flat overcast sheet) while the edges
// still feather over a few pixels. A faint floor keeps wisps at the rim.
fn shape_to_density(f : f32, coverage : f32) -> f32 {
    // c≈0 → high threshold (almost nothing clears it); c≈1 → low threshold
    // (most of the field is cloud, but the deepest valleys still gap).
    let threshold = mix(0.80, 0.26, coverage);
    let band = 0.16;
    return clamp(smoothstep(threshold, threshold + band, f), 0.0, 1.0);
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
    field    : f32, // cloud_field f0 (the silhouette + lighting source)
    density  : f32, // shape_to_density(field, coverage)
    light    : f32, // relief lighting term, ~0.15..1.8
    alpha    : f32, // final composited opacity
    albedo   : vec3<f32>, // UNLIT rain-coloured albedo (precip → darkness)
    col      : vec3<f32>, // final lit + tinted colour
};

fn shade(uv : vec2<f32>) -> Shade {
    let rv = radar_value(uv);
    let precip = rv.r;
    let coverage = rv.g;

    // Sample the cloud field at the pixel and two steps toward the sun.
    // The difference along the light direction IS the relief: where the
    // field rises toward the sun the surface faces the light (bright),
    // where it falls away it's in shadow. Same field as the silhouette,
    // so lighting and shape always agree — this is the whole trick.
    let sun_step = normalize(U.sun_dir) * (0.85 / U.map_scale);
    let f0 = cloud_field(uv);
    let f1 = cloud_field(uv + sun_step);
    let f2 = cloud_field(uv + sun_step * 2.4);

    let d = shape_to_density(f0, coverage);

    // Relief lighting. The field doubles as a height proxy: stepping toward
    // the sun, a FALLING field means this face tilts into the light (bright
    // sunlit top); a rising one means it tilts away (shadowed underside).
    // `dlight` is that signed slope; the strong gain + low ambient floor is
    // what gives fluffy 3D form instead of the old flat ~0.9 haze.
    let dlight = f0 - f1;
    // Ambient occlusion: deep field valleys are cloud interior/crevices and
    // read dark; the high parts (tops) catch full light.
    let ao = smoothstep(0.12, 0.62, f0);
    // Directional sun term, centred so flat areas sit mid-bright.
    let sun = clamp(0.5 + dlight * 9.0, 0.0, 1.5);
    // Cast self-shadow: cloud further toward the sun occludes us (Beer).
    let self_shadow = exp(-max(f2 - f0, 0.0) * 3.0);
    var light = (0.32 + 0.9 * sun * ao) * self_shadow;

    // Thin edges facing the sun catch a bright silver lining.
    let rim = smoothstep(0.5, 0.0, d) * clamp(dlight * 8.0, 0.0, 1.0);
    light = light + rim * 0.6;
    light = clamp(light, 0.12, 1.9);

    // Cool shadow / warm highlight tint for atmospheric depth.
    let albedo = rain_albedo(precip);
    let shadow_tint = vec3<f32>(0.80, 0.84, 0.95);
    let light_tint  = vec3<f32>(1.06, 1.03, 0.97);
    let tint = mix(shadow_tint, light_tint, smoothstep(0.5, 1.2, light));
    let col = clamp(albedo * light * tint, vec3<f32>(0.0), vec3<f32>(1.0));

    // Feathered alpha: because `d` is a wide smoothstep of a fractal
    // field, its low contour is naturally wispy. Rain-bearing cloud is
    // optically thick → push storms opaque so they don't read muddy.
    var alpha = smoothstep(0.0, U.softness, d);
    let storm = smoothstep(0.3, 0.75, precip) * smoothstep(0.05, 0.3, d);
    alpha = max(alpha, storm);
    alpha = clamp(alpha * U.intensity, 0.0, 0.97);

    var s : Shade;
    s.precip = precip;
    s.coverage = coverage;
    s.field = f0;
    s.density = d;
    s.light = light;
    s.alpha = alpha;
    s.albedo = albedo;
    s.col = col;
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
        default: { return vec4<f32>(s.col * s.alpha, s.alpha); }  // production composite
    }
}

@fragment
fn fs(in : VsOut) -> @location(0) vec4<f32> {
    let s = shade(in.uv);

    if (U.debug_view > 0u) {
        return debug_aov(s);
    }

    // Production path: bail on empty sky / sub-threshold density exactly
    // as before, then composite premultiplied.
    if (s.coverage <= 0.02) {
        return vec4<f32>(0.0);
    }
    if (s.density <= 0.004) {
        return vec4<f32>(0.0);
    }
    return vec4<f32>(s.col * s.alpha, s.alpha);
}
