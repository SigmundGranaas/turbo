// Icon/sprite rendering: instanced quads in screen space sampling the
// single-channel SDF sprite atlas. The shape is monochrome and resolution-
// independent; the fragment thresholds the distance field for a crisp,
// antialiased edge at any scale and multiplies by the per-instance tint.

struct Globals {
    viewport: vec2<f32>,
    _pad: vec2<f32>,
};

@group(0) @binding(0) var<uniform> globals: Globals;
@group(0) @binding(1) var atlas_tex: texture_2d<f32>;
@group(0) @binding(2) var atlas_samp: sampler;

struct VertexInput {
    @location(0) corner: vec2<f32>, // unit quad [0,1]^2
};

struct InstanceInput {
    @location(1) screen_centre: vec2<f32>,
    @location(2) size_px: vec2<f32>,
    @location(3) atlas_origin: vec2<f32>, // normalised [0,1]
    @location(4) atlas_size: vec2<f32>,   // normalised [0,1]
    @location(5) tint: vec4<f32>,         // linear rgba
};

struct VertexOutput {
    @builtin(position) clip_position: vec4<f32>,
    @location(0) uv: vec2<f32>,
    @location(1) tint: vec4<f32>,
};

@vertex
fn vs_main(in: VertexInput, inst: InstanceInput) -> VertexOutput {
    // Centre the unit quad on the anchor.
    let pixel = inst.screen_centre + (in.corner - vec2<f32>(0.5, 0.5)) * inst.size_px;
    let ndc = vec2<f32>(
        pixel.x / globals.viewport.x * 2.0 - 1.0,
        1.0 - pixel.y / globals.viewport.y * 2.0,
    );
    var out: VertexOutput;
    out.clip_position = vec4<f32>(ndc, 0.0, 1.0);
    out.uv = inst.atlas_origin + in.corner * inst.atlas_size;
    out.tint = inst.tint;
    return out;
}

@fragment
fn fs_main(in: VertexOutput) -> @location(0) vec4<f32> {
    // SDF: 0.5 = contour, < 0.5 inside the shape. Antialias over one screen
    // pixel using the field's screen-space derivative.
    let sdf = textureSample(atlas_tex, atlas_samp, in.uv).r;
    let aa = fwidth(sdf) * 0.7;
    let alpha = 1.0 - smoothstep(0.5 - aa, 0.5 + aa, sdf);
    if (alpha <= 0.0) {
        discard;
    }
    return vec4<f32>(in.tint.rgb, in.tint.a * alpha);
}
