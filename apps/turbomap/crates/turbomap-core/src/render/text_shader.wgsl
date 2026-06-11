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
    @location(6) halo_color: vec4<f32>,   // halo colour (unorm rgba)
    @location(7) halo_width: f32,         // SDF threshold offset, 0 = none
    @location(8) angle: f32,              // glyph rotation about pivot (rad)
    @location(9) pivot: vec2<f32>,        // screen-space rotation centre
    @location(10) mode: f32,              // 1 = halo pass, 0 = fill pass
    @location(11) weight: f32,            // faux-bold: SDF fill dilation, 0 = none
};

struct VertexOutput {
    @builtin(position) clip_position: vec4<f32>,
    @location(0) uv: vec2<f32>,
    @location(1) color: vec4<f32>,
    @location(2) halo_color: vec4<f32>,
    @location(3) @interpolate(flat) halo_width: f32,
    @location(4) @interpolate(flat) mode: f32,
    @location(5) @interpolate(flat) weight: f32,
};

@vertex
fn vs_main(in: VertexInput, inst: InstanceInput) -> VertexOutput {
    // Axis-aligned glyph quad in screen pixels, then rotated about `pivot`
    // by `angle` (0 for point labels ⇒ identity, byte-for-byte unchanged).
    let flat = inst.screen_origin + in.corner * inst.screen_size;
    let rel = flat - inst.pivot;
    let c = cos(inst.angle);
    let s = sin(inst.angle);
    let rotated = vec2<f32>(rel.x * c - rel.y * s, rel.x * s + rel.y * c);
    let pixel = inst.pivot + rotated;
    let ndc = vec2<f32>(
        pixel.x / globals.viewport.x * 2.0 - 1.0,
        1.0 - pixel.y / globals.viewport.y * 2.0,
    );
    var out: VertexOutput;
    out.clip_position = vec4<f32>(ndc, 0.0, 1.0);
    out.uv = inst.atlas_origin + in.corner * inst.atlas_size;
    out.color = inst.color;
    out.halo_color = inst.halo_color;
    out.halo_width = inst.halo_width;
    out.mode = inst.mode;
    out.weight = inst.weight;
    return out;
}

// Text renders in two instance passes per label (halos staged before
// fills), so a glyph's halo never paints over a neighbouring glyph's
// body — the same ordering MapLibre uses.
@fragment
fn fs_main(in: VertexOutput) -> @location(0) vec4<f32> {
    let sdf = textureSample(atlas_tex, atlas_samp, in.uv).r;
    // Screen-space derivative of the sdf gives our per-pixel AA width.
    // `fwidth(sdf) * 0.7` is the conventional "half a pixel" smooth band.
    let aa = fwidth(sdf) * 0.7;

    // Faux-bold dilates the glyph by pushing the fill cutoff outside the
    // contour by `weight` (in SDF units). The halo then sits a further
    // `halo_width` beyond the thickened stroke, so heavier text keeps its
    // casing instead of swallowing it.
    let fill_cutoff = 0.5 + in.weight;

    if (in.mode > 0.5) {
        // Halo pass: the full disc out to `halo_width` beyond the (dilated)
        // contour. The fill pass covers its centre afterwards, and the
        // fill's AA edge blends onto halo colour — the correct fringe.
        let halo_outer = fill_cutoff + in.halo_width;
        let alpha = (1.0 - smoothstep(halo_outer - aa, halo_outer + aa, sdf)) * in.halo_color.a;
        if (alpha <= 0.0) {
            discard;
        }
        return vec4<f32>(in.halo_color.rgb, alpha);
    }

    // Fill pass: alpha = 1 inside the dilated contour, 0 outside, smooth
    // band of `2*aa` straddling the cutoff.
    let fill_alpha = 1.0 - smoothstep(fill_cutoff - aa, fill_cutoff + aa, sdf);
    if (fill_alpha <= 0.0) {
        discard;
    }
    return vec4<f32>(in.color.rgb, fill_alpha * in.color.a);
}
