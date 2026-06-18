// Upscale-composite pass for the half-resolution cloud buffer.
//
// The expensive volumetric march runs into a half-res offscreen target; this
// pass samples that target with bilinear filtering and composites it onto the
// full-res surface. The cloud buffer holds PREMULTIPLIED colour, and bilinear
// interpolation of premultiplied values is the correct filter (no edge halos),
// so a straight `textureSampleLevel` + premultiplied blend reproduces the
// full-res look at a fraction of the march cost.

@group(0) @binding(0) var src_tex : texture_2d<f32>;
@group(0) @binding(1) var src_samp : sampler;

struct VsOut {
    @builtin(position) pos : vec4<f32>,
    @location(0) uv : vec2<f32>,
};

@vertex
fn vs(@builtin(vertex_index) vi : u32) -> VsOut {
    // Fullscreen triangle, same convention as clouds.wgsl.
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

@fragment
fn fs(in : VsOut) -> @location(0) vec4<f32> {
    return textureSampleLevel(src_tex, src_samp, in.uv, 0.0);
}
