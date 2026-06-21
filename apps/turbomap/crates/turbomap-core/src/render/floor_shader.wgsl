// Ground "floor" backstop. A full-screen pass that ray-casts each pixel onto a
// virtual ground plane a little below sea level, so where terrain tiles haven't
// loaded yet (or a mixed-LOD seam gapes) the hole shows a neutral sea-grey
// instead of see-through sky/clear. Drawn after the sky and BEFORE the terrain,
// writing depth, so real terrain (which sits higher) overdraws it everywhere it
// exists. The plane droops with the same Earth-curvature term as the terrain so
// it never pokes up through the curved-away far field.

struct Globals {
    // RTC view-projection and its inverse (origin = camera centre).
    view_proj: mat4x4<f32>,
    inv_view_proj: mat4x4<f32>,
    color: vec4<f32>,
    // Plane height in world-z (a little below sea level, i.e. slightly negative).
    floor_z: f32,
    // Earth-curvature drop coefficient (π·cos³φ); matches the terrain shader.
    curvature_coeff: f32,
    // 0 = don't draw (flat 2D map); 1 = draw.
    enabled: f32,
    _pad: f32,
};

@group(0) @binding(0) var<uniform> g: Globals;

struct VsOut {
    @builtin(position) pos: vec4<f32>,
    @location(0) ndc: vec2<f32>,
};

@vertex
fn vs_main(@builtin(vertex_index) i: u32) -> VsOut {
    // One oversized full-screen triangle.
    var p = array<vec2<f32>, 3>(vec2<f32>(-1.0, -1.0), vec2<f32>(3.0, -1.0), vec2<f32>(-1.0, 3.0));
    var o: VsOut;
    o.pos = vec4<f32>(p[i], 0.0, 1.0);
    o.ndc = p[i];
    return o;
}

struct FsOut {
    @location(0) color: vec4<f32>,
    @builtin(frag_depth) depth: f32,
};

@fragment
fn fs_main(in: VsOut) -> FsOut {
    var o: FsOut;
    if (g.enabled < 0.5) {
        discard;
    }
    // Unproject this pixel into an RTC world-space ray (near → far point).
    let near4 = g.inv_view_proj * vec4<f32>(in.ndc, 0.0, 1.0);
    let far4 = g.inv_view_proj * vec4<f32>(in.ndc, 1.0, 1.0);
    let n = near4.xyz / near4.w;
    let f = far4.xyz / far4.w;
    let dir = f - n;
    if (abs(dir.z) < 1e-9) {
        discard;
    }
    // First intersect the flat plane to get the ground xy, then apply the same
    // curvature droop the terrain uses (z lowered by coeff·|xy|²) and re-intersect,
    // so the floor tracks the curved-away far field instead of poking through it.
    let t0 = (g.floor_z - n.z) / dir.z;
    if (t0 < 0.0) {
        discard; // plane is behind the camera (e.g. rays above the horizon)
    }
    let hit0 = n + dir * t0;
    let plane_z = g.floor_z - g.curvature_coeff * dot(hit0.xy, hit0.xy);
    let t = (plane_z - n.z) / dir.z;
    if (t < 0.0) {
        discard;
    }
    let hit = n + dir * t;
    let clip = g.view_proj * vec4<f32>(hit, 1.0);
    if (clip.w <= 0.0) {
        discard;
    }
    o.depth = clip.z / clip.w;
    o.color = g.color;
    return o;
}
