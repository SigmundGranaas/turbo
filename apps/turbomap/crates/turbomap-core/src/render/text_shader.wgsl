// Text rendering with SDF-based fill + halo. The atlas stores a signed
// distance field per glyph: 128 = on the contour, lower = inside, higher
// = outside. The fragment shader thresholds at 0.5 (the centre) for the
// glyph fill, and at a wider threshold for the halo outline.

struct Globals {
    viewport: vec2<f32>,
    _pad: vec2<f32>,
};

@group(0) @binding(0) var<uniform> globals: Globals;
@group(0) @binding(1) var atlas_tex: texture_2d<f32>;
@group(0) @binding(2) var atlas_samp: sampler;

struct VertexInput {
    @location(0) corner: vec2<f32>,
};

struct InstanceInput {
    @location(1) screen_origin: vec2<f32>,
    @location(2) screen_size: vec2<f32>,
    @location(3) atlas_origin: vec2<f32>, // normalised [0, 1]
    @location(4) atlas_size: vec2<f32>,   // normalised [0, 1]
    @location(5) color: vec4<f32>,        // text colour (unorm rgba)
};

struct VertexOutput {
    @builtin(position) clip_position: vec4<f32>,
    @location(0) uv: vec2<f32>,
    @location(1) color: vec4<f32>,
};

@vertex
fn vs_main(in: VertexInput, inst: InstanceInput) -> VertexOutput {
    let pixel = inst.screen_origin + in.corner * inst.screen_size;
    let ndc = vec2<f32>(
        pixel.x / globals.viewport.x * 2.0 - 1.0,
        1.0 - pixel.y / globals.viewport.y * 2.0,
    );
    var out: VertexOutput;
    out.clip_position = vec4<f32>(ndc, 0.0, 1.0);
    out.uv = inst.atlas_origin + in.corner * inst.atlas_size;
    out.color = inst.color;
    return out;
}

@fragment
fn fs_main(in: VertexOutput) -> @location(0) vec4<f32> {
    let sdf = textureSample(atlas_tex, atlas_samp, in.uv).r;
    // Screen-space derivative of the sdf gives our per-pixel AA width.
    // `fwidth(sdf) * 0.7` is the conventional "half a pixel" smooth band.
    let aa = fwidth(sdf) * 0.7;

    // Glyph fill: alpha = 1 inside contour (sdf < 0.5), 0 outside, smooth
    // band of `2*aa` straddling the contour.
    let fill_alpha = 1.0 - smoothstep(0.5 - aa, 0.5 + aa, sdf);

    // Halo: a wider band beyond the contour. Threshold 0.625 ≈ ~1.5 raster
    // pixels of outline; tuned by eye to be readable but not visually loud.
    let halo_outer: f32 = 0.625;
    let halo_total = 1.0 - smoothstep(halo_outer - aa, halo_outer + aa, sdf);
    // The "halo-only" region is the band between contour and outer edge —
    // i.e. halo_total minus the fill region underneath.
    let halo_only = max(0.0, halo_total - fill_alpha);

    // Halo colour is fixed (dark grey) — could be promoted to a uniform
    // later for per-rule halo control.
    let halo_rgb = vec3<f32>(0.96, 0.96, 0.96);

    // Composite halo first, text on top, in straight-alpha space.
    let combined_alpha = fill_alpha + halo_only;
    if (combined_alpha <= 0.0) {
        discard;
    }
    let rgb =
        (in.color.rgb * fill_alpha + halo_rgb * halo_only) / max(combined_alpha, 1e-5);
    return vec4<f32>(rgb, combined_alpha * in.color.a);
}
