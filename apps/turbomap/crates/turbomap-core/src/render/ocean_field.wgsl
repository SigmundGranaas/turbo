// Ocean Field generation — evaluates the inverse spectral transform DIRECTLY
// (sum of the cascade's selected cosines) into one cascade texture level.
// Output RGBA16F = (displacement.x, displacement.y, height, foam).
//
// Rendered once per (cascade, mip level) per frame; `k_max` band-limits each mip
// so coarse levels are pre-filtered (no aliasing → the anti-tiling fix). See
// `ocean.rs` for the spectrum/cascade design.

struct OceanU {
    time: f32,
    patch_l: f32,    // world metres across the patch
    k_max: f32,      // Nyquist cap for this mip level
    choppiness: f32, // horizontal (Gerstner) displacement gain
    wave_count: u32,
};

struct Wave {
    k: vec2<f32>,    // wave vector (rad/m); periodic in patch_l for seamless tiling
    amp: f32,
    phase: f32,
};

@group(0) @binding(0) var<uniform> u: OceanU;
@group(0) @binding(1) var<storage, read> waves: array<Wave>;

struct VOut {
    @builtin(position) pos: vec4<f32>,
    @location(0) uv: vec2<f32>,
};

@vertex
fn vs_main(@builtin(vertex_index) vi: u32) -> VOut {
    // Fullscreen triangle; uv spans [0,1] across the render target (overscan
    // beyond 1 is clipped). uv → patch position so the field is periodic in
    // patch_l → the texture tiles seamlessly.
    var p = array<vec2<f32>, 3>(
        vec2<f32>(-1.0, -1.0),
        vec2<f32>(3.0, -1.0),
        vec2<f32>(-1.0, 3.0),
    );
    let xy = p[vi];
    var o: VOut;
    o.pos = vec4<f32>(xy, 0.0, 1.0);
    o.uv = xy * 0.5 + vec2<f32>(0.5, 0.5);
    return o;
}

const G: f32 = 9.81;

@fragment
fn fs_main(in: VOut) -> @location(0) vec4<f32> {
    let pos = in.uv * u.patch_l;
    var disp = vec2<f32>(0.0, 0.0);
    var height = 0.0;
    // Jacobian of the horizontal displacement → foam where the surface folds.
    var jxx = 0.0;
    var jyy = 0.0;
    var jxy = 0.0;
    var jyx = 0.0;

    let n = u.wave_count;
    for (var i = 0u; i < n; i = i + 1u) {
        let w = waves[i];
        let kmag = length(w.k);
        if (kmag < 1.0e-6 || kmag > u.k_max) {
            continue;
        }
        let omega = sqrt(G * kmag);
        let theta = dot(w.k, pos) - omega * u.time + w.phase;
        let s = sin(theta);
        let c = cos(theta);
        height = height + w.amp * c;
        let dir = w.k / kmag;
        let a = w.amp * u.choppiness;
        disp = disp - dir * (a * s);
        // d(disp)/d(pos): disp = -dir·a·sin(θ), dθ/dpos = k
        jxx = jxx - dir.x * a * c * w.k.x;
        jyy = jyy - dir.y * a * c * w.k.y;
        jxy = jxy - dir.x * a * c * w.k.y;
        jyx = jyx - dir.y * a * c * w.k.x;
    }

    let jdet = (1.0 + jxx) * (1.0 + jyy) - jxy * jyx;
    let foam = clamp(1.0 - jdet, 0.0, 1.0);
    return vec4<f32>(disp.x, disp.y, height, foam);
}
