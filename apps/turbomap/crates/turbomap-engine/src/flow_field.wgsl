// The `flow-field` built-in custom layer (plan D4): an animated,
// world-anchored field of tapered streaks. Fully GPU-procedural — the
// vertex shader synthesises each streak from its instance index; direction
// comes from a hash of the ABSOLUTE world cell so the field is pinned to
// the ground, and sways gently with time.

struct Uniform {
    view_proj: mat4x4<f32>,
    // xy = RTC world position of the first cell centre; z = cell size
    // (world units); w = time (s).
    grid: vec4<f32>,
    // x,y = grid dims; z,w = absolute index of the first cell (hash seed).
    dims: vec4<f32>,
    color: vec4<f32>,
};

@group(0) @binding(0) var<uniform> u: Uniform;

struct VertexOutput {
    @builtin(position) clip_position: vec4<f32>,
    // Along-streak coordinate in [0,1] (1 = tip) for the taper fade.
    @location(0) along: f32,
};

// Integer hash (Wang-style) → [0,1). Seeded by the absolute cell index so
// the same ground cell always flows the same way.
fn hash2(x: u32, y: u32) -> f32 {
    var h = x * 374761393u + y * 668265263u;
    h = (h ^ (h >> 13u)) * 1274126177u;
    h = h ^ (h >> 16u);
    return f32(h & 0xffffffu) / 16777216.0;
}

@vertex
fn vs_main(
    @builtin(vertex_index) vi: u32,
    @builtin(instance_index) inst: u32,
) -> VertexOutput {
    let nx = u32(u.dims.x);
    let ix = inst % nx;
    let iy = inst / nx;
    let cell = u.grid.z;
    let centre = u.grid.xy + vec2<f32>(f32(ix), f32(iy)) * cell;

    // World-stable flow direction + a gentle time sway whose rate also
    // varies per cell (so the field shimmers rather than rotating rigidly).
    let sx = u32(u.dims.z) + ix;
    let sy = u32(u.dims.w) + iy;
    let h1 = hash2(sx, sy);
    let h2 = hash2(sx ^ 0x9e3779b9u, sy ^ 0x85ebca6bu);
    let angle = 6.28318548 * h1 + (0.6 + 0.8 * h2) * 0.5 * u.grid.w;
    let dir = vec2<f32>(cos(angle), sin(angle));
    let perp = vec2<f32>(-dir.y, dir.x);

    // Two triangles over a tapered quad: corner.x in [-1,1] along the
    // streak, corner.y in [-1,1] across it; width tapers toward the tip.
    var corners = array<vec2<f32>, 6>(
        vec2<f32>(-1.0, -1.0), vec2<f32>(1.0, -1.0), vec2<f32>(1.0, 1.0),
        vec2<f32>(-1.0, -1.0), vec2<f32>(1.0, 1.0), vec2<f32>(-1.0, 1.0),
    );
    let c = corners[vi];
    let along = 0.5 * (c.x + 1.0);
    let half_len = 0.32 * cell;
    let half_width = 0.05 * cell * (1.2 - along);
    let world = centre + dir * (c.x * half_len) + perp * (c.y * half_width);

    var out: VertexOutput;
    out.clip_position = u.view_proj * vec4<f32>(world, 0.0, 1.0);
    out.along = along;
    return out;
}

@fragment
fn fs_main(in: VertexOutput) -> @location(0) vec4<f32> {
    // Brighten toward the tip so the streak reads directional.
    let a = u.color.a * (0.35 + 0.65 * in.along);
    return vec4<f32>(u.color.rgb, a);
}
