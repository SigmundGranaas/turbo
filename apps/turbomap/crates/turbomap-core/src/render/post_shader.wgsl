// Post-process: HDR bloom + filmic tonemap.
//
// The frame pass renders the whole scene into a linear HDR texture (Rgba16Float)
// where highlights — the sun disk, sky glow, snow under a low sun — can exceed
// 1.0. This shader turns that into the final sRGB image in four fullscreen
// passes, all driven by `vs_fullscreen` (one oversized triangle, no vertex
// buffer):
//
//   1. fs_bright   — sample the HDR scene at half res, keep only the energy
//                    above `THRESHOLD` (soft knee), write to bloom buffer A.
//   2. fs_blur_h   — separable Gaussian, horizontal, A → B.
//   3. fs_blur_v   — separable Gaussian, vertical,   B → A.
//   4. fs_tonemap  — sample the full-res HDR scene, add the upsampled bloom,
//                    apply exposure + ACES filmic tonemap, write to the surface
//                    (the sRGB target applies the OETF on store).
//
// Each pass binds exactly one input texture + sampler at group 0; the targets
// are wired up host-side in `post.rs`.

@group(0) @binding(0) var src_tex: texture_2d<f32>;
@group(0) @binding(1) var src_samp: sampler;

struct VsOut {
    @builtin(position) pos: vec4<f32>,
    @location(0) uv: vec2<f32>,
};

// Fullscreen triangle. Generates UVs in [0,1] with v flipped so the sampled
// image is upright (NDC y is up, texture v is down).
@vertex
fn vs_fullscreen(@builtin(vertex_index) i: u32) -> VsOut {
    var out: VsOut;
    let x = f32((i << 1u) & 2u) * 2.0 - 1.0; // -1, 3, -1
    let y = f32(i & 2u) * 2.0 - 1.0;          // -1, -1, 3
    out.pos = vec4<f32>(x, y, 0.0, 1.0);
    out.uv = vec2<f32>((x + 1.0) * 0.5, (1.0 - y) * 0.5);
    return out;
}

// --- bloom tuning -----------------------------------------------------------
// Energy below THRESHOLD never blooms; a soft knee around it avoids a hard
// flicker edge as a highlight ramps past the cutoff.
const THRESHOLD: f32 = 1.05;
const KNEE: f32 = 0.6;
// Camera exposure applied before tonemapping. <1 reins in the bright daytime
// scene (the pale topo basemap under a high sun read as too hot/washed) and
// keeps the analytic sky from blowing out the moment bloom is added.
const EXPOSURE: f32 = 0.74;
// How much of the blurred highlight buffer to add back over the scene. Kept
// modest so daytime midtones don't wash out (bloom adds energy everywhere a
// highlight bleeds).
const BLOOM_INTENSITY: f32 = 0.42;

// Soft-knee bright-pass extraction (Karis / Unreal style). Returns the part of
// `c` above the threshold, ramping smoothly through the knee.
fn bright_knee(c: vec3<f32>) -> vec3<f32> {
    let lum = max(c.r, max(c.g, c.b));
    let knee = THRESHOLD * KNEE;
    var soft = lum - THRESHOLD + knee;
    soft = clamp(soft, 0.0, 2.0 * knee);
    soft = soft * soft / (4.0 * knee + 1e-4);
    let contrib = max(soft, lum - THRESHOLD) / max(lum, 1e-4);
    return c * contrib;
}

// Pass 1: bright-pass + downsample (target is half-res, so the bilinear fetch
// already averages a 2×2 neighbourhood).
@fragment
fn fs_bright(in: VsOut) -> @location(0) vec4<f32> {
    let c = textureSample(src_tex, src_samp, in.uv).rgb;
    return vec4<f32>(bright_knee(c), 1.0);
}

// 9-tap Gaussian weights (σ≈2), normalised. Shared by both blur directions.
const W0: f32 = 0.227027;
const W1: f32 = 0.194595;
const W2: f32 = 0.121622;
const W3: f32 = 0.054054;
const W4: f32 = 0.016216;

fn blur(uv: vec2<f32>, dir: vec2<f32>) -> vec3<f32> {
    let texel = 1.0 / vec2<f32>(textureDimensions(src_tex));
    let step = dir * texel;
    var acc = textureSample(src_tex, src_samp, uv).rgb * W0;
    acc += textureSample(src_tex, src_samp, uv + step * 1.0).rgb * W1;
    acc += textureSample(src_tex, src_samp, uv - step * 1.0).rgb * W1;
    acc += textureSample(src_tex, src_samp, uv + step * 2.0).rgb * W2;
    acc += textureSample(src_tex, src_samp, uv - step * 2.0).rgb * W2;
    acc += textureSample(src_tex, src_samp, uv + step * 3.0).rgb * W3;
    acc += textureSample(src_tex, src_samp, uv - step * 3.0).rgb * W3;
    acc += textureSample(src_tex, src_samp, uv + step * 4.0).rgb * W4;
    acc += textureSample(src_tex, src_samp, uv - step * 4.0).rgb * W4;
    return acc;
}

@fragment
fn fs_blur_h(in: VsOut) -> @location(0) vec4<f32> {
    return vec4<f32>(blur(in.uv, vec2<f32>(1.0, 0.0)), 1.0);
}

@fragment
fn fs_blur_v(in: VsOut) -> @location(0) vec4<f32> {
    return vec4<f32>(blur(in.uv, vec2<f32>(0.0, 1.0)), 1.0);
}

// ACES filmic tonemap (Narkowicz fit). Maps unbounded linear HDR into [0,1]
// with a pleasing toe + shoulder so highlights roll off instead of clipping.
fn aces(x: vec3<f32>) -> vec3<f32> {
    let a = 2.51;
    let b = 0.03;
    let c = 2.43;
    let d = 0.59;
    let e = 0.14;
    return clamp((x * (a * x + b)) / (x * (c * x + d) + e), vec3<f32>(0.0), vec3<f32>(1.0));
}

// Pass 4: composite scene + bloom, expose, tonemap. The bloom buffer is bound
// at group 1 (the scene HDR is group 0) so both are available at once.
@group(1) @binding(0) var bloom_tex: texture_2d<f32>;
@group(1) @binding(1) var bloom_samp: sampler;

@fragment
fn fs_tonemap(in: VsOut) -> @location(0) vec4<f32> {
    let scene = textureSample(src_tex, src_samp, in.uv).rgb;
    let bloom = textureSample(bloom_tex, bloom_samp, in.uv).rgb;
    let hdr = (scene + bloom * BLOOM_INTENSITY) * EXPOSURE;
    return vec4<f32>(aces(hdr), 1.0);
}
