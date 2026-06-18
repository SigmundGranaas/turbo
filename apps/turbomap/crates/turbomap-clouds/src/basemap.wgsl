// Stylised 2D basemap, drawn underneath the cloud overlay purely so the
// demo screenshots look like a real map. Land/sea split from a couple of
// low-frequency value-noise lobes, gentle hillshade, a faint graticule.
// This is throwaway scaffolding — in the live app the clouds composite
// over the actual turbomap raster/vector layers instead.

struct VsOut {
    @builtin(position) pos : vec4<f32>,
    @location(0) uv : vec2<f32>,
};

@vertex
fn vs(@builtin(vertex_index) vi : u32) -> VsOut {
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

fn hash21(p_in : vec2<f32>) -> f32 {
    var p3 = fract(vec3<f32>(p_in.xyx) * 0.1031);
    p3 += dot(p3, p3.yzx + 33.33);
    return fract((p3.x + p3.y) * p3.z);
}

fn vnoise(p : vec2<f32>) -> f32 {
    let i = floor(p);
    let f = fract(p);
    let u = f * f * (3.0 - 2.0 * f);
    let a = hash21(i + vec2<f32>(0.0, 0.0));
    let b = hash21(i + vec2<f32>(1.0, 0.0));
    let c = hash21(i + vec2<f32>(0.0, 1.0));
    let d = hash21(i + vec2<f32>(1.0, 1.0));
    return mix(mix(a, b, u.x), mix(c, d, u.x), u.y);
}

fn fbm(p_in : vec2<f32>) -> f32 {
    var p = p_in;
    var a = 0.5;
    var s = 0.0;
    for (var i = 0; i < 5; i = i + 1) {
        s += a * vnoise(p);
        a *= 0.5;
        p *= 2.0;
    }
    return s;
}

// Terrain height field; >0.5 is land.
fn land_height(uv : vec2<f32>) -> f32 {
    let p = uv * 3.0;
    // Bias so a coastline runs across the frame.
    return fbm(p + vec2<f32>(1.3, 4.7)) * 0.9 + (uv.y * 0.25);
}

@fragment
fn fs(in : VsOut) -> @location(0) vec4<f32> {
    let uv = in.uv;
    let h = land_height(uv);

    let sea_deep    = vec3<f32>(0.16, 0.28, 0.42);
    let sea_shallow = vec3<f32>(0.27, 0.45, 0.58);
    let shore       = vec3<f32>(0.78, 0.74, 0.58);
    let lowland     = vec3<f32>(0.40, 0.52, 0.36);
    let upland      = vec3<f32>(0.52, 0.50, 0.40);
    let peak        = vec3<f32>(0.86, 0.86, 0.84);

    var col : vec3<f32>;
    let coast = 0.5;
    if (h < coast) {
        col = mix(sea_deep, sea_shallow, smoothstep(0.30, coast, h));
    } else {
        var land = mix(shore, lowland, smoothstep(coast, coast + 0.06, h));
        land = mix(land, upland, smoothstep(0.62, 0.80, h));
        land = mix(land, peak, smoothstep(0.86, 0.98, h));
        // Cheap hillshade from the height gradient.
        let e = 1.0 / 512.0;
        let dx = land_height(uv + vec2<f32>(e, 0.0)) - land_height(uv - vec2<f32>(e, 0.0));
        let dy = land_height(uv + vec2<f32>(0.0, e)) - land_height(uv - vec2<f32>(0.0, e));
        let n = normalize(vec3<f32>(-dx, -dy, 0.02));
        let shade = clamp(dot(n, normalize(vec3<f32>(-0.5, -0.6, 0.6))), 0.0, 1.0);
        col = land * (0.7 + 0.5 * shade);
    }

    // Faint graticule.
    let g = abs(fract(uv * 8.0 - 0.5) - 0.5);
    let grid = smoothstep(0.49, 0.5, max(g.x, g.y)) * 0.05;
    col = mix(col, vec3<f32>(1.0), grid);

    return vec4<f32>(col, 1.0);
}
