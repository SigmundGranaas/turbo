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
};

struct VertexOutput {
    @builtin(position) clip_position: vec4<f32>,
    @location(0) uv: vec2<f32>,
    @location(1) color: vec4<f32>,
    @location(2) halo_color: vec4<f32>,
    @location(3) @interpolate(flat) halo_width: f32,
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

    // Halo: a band reaching `halo_width` (normalised SDF units) beyond the
    // contour, per-instance. width 0 → outer == fill threshold → no halo.
    let halo_outer = 0.5 + in.halo_width;
    let halo_total = 1.0 - smoothstep(halo_outer - aa, halo_outer + aa, sdf);
    // The "halo-only" region is the band between contour and outer edge —
    // i.e. halo_total minus the fill region underneath. Gated on alpha so a
    // transparent halo colour contributes nothing.
    let halo_only = max(0.0, halo_total - fill_alpha) * in.halo_color.a;

    // Composite halo behind, text on top, in straight-alpha space.
    let combined_alpha = fill_alpha + halo_only;
    if (combined_alpha <= 0.0) {
        discard;
    }
    let rgb =
        (in.color.rgb * fill_alpha + in.halo_color.rgb * halo_only) / max(combined_alpha, 1e-5);
    return vec4<f32>(rgb, combined_alpha * in.color.a);
}
