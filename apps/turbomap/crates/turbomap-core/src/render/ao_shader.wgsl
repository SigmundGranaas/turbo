// Progressive horizon-based ambient occlusion (HBAO) accumulation.
//
// A fullscreen pass over the world-locked AO field (same grid + extent as the
// cast-shadow heightfield). Each invocation is one AO cell; it marches the
// heightfield in a *batch* of azimuth directions, finds the horizon angle in
// each, and outputs the mean sky occlusion contributed by that batch.
//
// The pass is run once per frame with a different direction batch and ADDITIVE
// blending, so the AO field refines over several frames (cheap first pass →
// higher quality as more directions land) and is then cached until the terrain
// region changes. AO is sun-independent, so it survives time-of-day changes.
//
// Each direction's occlusion is `sin(horizon_elevation)` — the fraction of that
// vertical slice of sky blocked by upstream relief. Averaged over all
// directions, the field holds the blocked fraction of the sky hemisphere in
// [0,1]; the terrain shader turns that into an ambient dimming.

@group(0) @binding(0) var height_tex: texture_2d<f32>;

struct AoParams {
    // 1 / HEIGHT_DIM — used to recover the grid resolution.
    inv_dim: f32,
    // World-xy size of one heightfield texel (the march step's run length).
    texel_world: f32,
    // First direction index of this batch, and how many directions it covers.
    dir_start: f32,
    dir_count: f32,
    // Total directions across the full accumulation (the normaliser).
    total_dirs: f32,
    // Heightfield texels to march per direction (the AO radius).
    steps: f32,
    _pad: vec2<f32>,
};
@group(1) @binding(0) var<uniform> p: AoParams;

struct VsOut {
    @builtin(position) pos: vec4<f32>,
};

// Fullscreen triangle covering the AO target.
@vertex
fn vs_ao(@builtin(vertex_index) i: u32) -> VsOut {
    var o: VsOut;
    let x = f32((i << 1u) & 2u) * 2.0 - 1.0;
    let y = f32(i & 2u) * 2.0 - 1.0;
    o.pos = vec4<f32>(x, y, 0.0, 1.0);
    return o;
}

const TWO_PI: f32 = 6.28318530718;

@fragment
fn fs_ao(@builtin(position) frag: vec4<f32>) -> @location(0) vec4<f32> {
    let dim = i32(round(1.0 / p.inv_dim));
    let cx = i32(frag.x);
    let cy = i32(frag.y);
    let h0 = textureLoad(height_tex, vec2<i32>(cx, cy), 0).r;

    let dir_count = i32(p.dir_count);
    let steps = i32(p.steps);
    var sum = 0.0;
    var d = 0;
    loop {
        if (d >= dir_count) { break; }
        let ang = (p.dir_start + f32(d)) * (TWO_PI / max(p.total_dirs, 1.0));
        let dir = vec2<f32>(cos(ang), sin(ang));
        // March outward, tracking the steepest (highest) horizon tangent.
        var max_tan = 0.0;
        var t = 1;
        loop {
            if (t > steps) { break; }
            let sx = cx + i32(round(dir.x * f32(t)));
            let sy = cy + i32(round(dir.y * f32(t)));
            if (sx < 0 || sy < 0 || sx >= dim || sy >= dim) { break; }
            let h = textureLoad(height_tex, vec2<i32>(sx, sy), 0).r;
            let rise = h - h0;
            if (rise > 0.0) {
                let run = f32(t) * p.texel_world;
                max_tan = max(max_tan, rise / max(run, 1.0e-9));
            }
            t = t + 1;
        }
        // sin(atan(max_tan)) = fraction of this direction's sky slice blocked.
        sum = sum + max_tan / sqrt(1.0 + max_tan * max_tan);
        d = d + 1;
    }
    // Normalise by the FULL direction count so the additive accumulation across
    // batches converges to the mean occlusion over the whole hemisphere.
    let contrib = sum / max(p.total_dirs, 1.0);
    return vec4<f32>(contrib, 0.0, 0.0, 1.0);
}
