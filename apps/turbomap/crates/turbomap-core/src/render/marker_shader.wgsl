// Screen-space anti-aliased discs. Each instance is a marker; the vertex
// shader stretches a unit quad to `2*radius` pixels around the screen
// anchor, and the fragment shader fades the alpha based on distance from
// the centre — soft edge in one screen pixel.

struct Globals {
    viewport: vec2<f32>,
    _pad: vec2<f32>,
};

@group(0) @binding(0) var<uniform> globals: Globals;

struct VertexInput {
    @location(0) corner: vec2<f32>,
};

struct InstanceInput {
    @location(1) screen_centre: vec2<f32>,
    @location(2) radius_px: f32,
    @location(3) _pad: f32,
    @location(4) color: vec4<f32>,
};

struct VertexOutput {
    @builtin(position) clip_position: vec4<f32>,
    /// Position relative to the centre, in pixels. The fragment computes
    /// length here to know how far we are from the centre.
    @location(0) local_px: vec2<f32>,
    @location(1) radius_px: f32,
    @location(2) color: vec4<f32>,
};

@vertex
fn vs_main(in: VertexInput, inst: InstanceInput) -> VertexOutput {
    // Quad corner is in [-1, 1] (we rebase the unit quad below).
    let offset = (in.corner * 2.0 - vec2<f32>(1.0, 1.0)) * inst.radius_px;
    let pixel = inst.screen_centre + offset;
    let ndc = vec2<f32>(
        pixel.x / globals.viewport.x * 2.0 - 1.0,
        1.0 - pixel.y / globals.viewport.y * 2.0,
    );
    var out: VertexOutput;
    out.clip_position = vec4<f32>(ndc, 0.0, 1.0);
    out.local_px = offset;
    out.radius_px = inst.radius_px;
    out.color = inst.color;
    return out;
}

@fragment
fn fs_main(in: VertexOutput) -> @location(0) vec4<f32> {
    let d = length(in.local_px);
    // 1-pixel feathered edge.
    let aa = clamp(in.radius_px - d, 0.0, 1.0);
    if (aa <= 0.0) {
        discard;
    }
    return vec4<f32>(in.color.rgb, in.color.a * aa);
}
