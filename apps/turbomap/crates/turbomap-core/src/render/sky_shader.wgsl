// Analytic atmosphere sky. A single full-screen triangle drawn first in
// the frame pass (depth disabled); the terrain then paints over it, so
// the sky shows only where there's no ground — the horizon band you see
// when the map is tilted.
//
// The colour is an analytic single-scattering gradient (Preetham/Hosek
// class): a zenith→horizon vertical blend plus Mie forward-scatter glow
// around the sun. The zenith/horizon/sun colours are supplied per frame
// from one time-of-day palette (`sun::atmosphere`), the same palette the
// terrain shading and aerial-perspective haze use — so the faded distant
// relief meets the horizon seamlessly and the whole scene shares the
// sun's warm/cool cast.

struct SkyGlobals {
    // Inverse of the RTC view-projection — turns NDC into world-space
    // ray directions (translation drops out, so RTC is fine).
    inv_view_proj: mat4x4<f32>,
    // Direction towards the sun, world frame (x=E, y=S, z=up).
    sun_dir: vec3<f32>,
    // Overall sun glow intensity (→0 at night).
    sun_intensity: f32,
    // Sky colour straight up.
    zenith_color: vec3<f32>,
    _p0: f32,
    // Sky colour at the horizon (matches the terrain haze colour).
    horizon_color: vec3<f32>,
    _p1: f32,
    // Sun disk / glow colour for the time of day.
    sun_color: vec3<f32>,
    _p2: f32,
};

@group(0) @binding(0) var<uniform> sky: SkyGlobals;

struct VsOut {
    @builtin(position) pos: vec4<f32>,
    // NDC of this corner, for reconstructing the view ray in the FS.
    @location(0) ndc: vec2<f32>,
};

// Oversized triangle covering the screen — no vertex buffer needed.
@vertex
fn vs_main(@builtin(vertex_index) i: u32) -> VsOut {
    var out: VsOut;
    let x = f32((i << 1u) & 2u) * 2.0 - 1.0; // -1, 3, -1
    let y = f32(i & 2u) * 2.0 - 1.0;          // -1, -1, 3
    out.pos = vec4<f32>(x, y, 0.0, 1.0);
    out.ndc = vec2<f32>(x, y);
    return out;
}

// World-space ray direction for this pixel, from the inverse VP.
fn ray_dir(ndc: vec2<f32>) -> vec3<f32> {
    let near = sky.inv_view_proj * vec4<f32>(ndc, 0.0, 1.0);
    let far = sky.inv_view_proj * vec4<f32>(ndc, 1.0, 1.0);
    let p0 = near.xyz / near.w;
    let p1 = far.xyz / far.w;
    return normalize(p1 - p0);
}

@fragment
fn fs_main(in: VsOut) -> @location(0) vec4<f32> {
    let rd = ray_dir(in.ndc);
    let up = clamp(rd.z, -1.0, 1.0);

    // Vertical gradient: horizon → zenith. The pow() keeps most of the
    // sky the deeper zenith hue, with the bright band hugging the horizon.
    let t = pow(clamp(up, 0.0, 1.0), 0.42);
    var col = mix(sky.horizon_color, sky.zenith_color, t);

    // Below the horizon (rays toward distant ground when tilted): settle
    // to a slightly deeper haze so the band reads as a clean horizon and
    // meets the aerial-perspective-faded terrain.
    if (up < 0.0) {
        col = mix(sky.horizon_color, sky.horizon_color * 0.72, clamp(-up * 4.0, 0.0, 1.0));
    }

    // Mie forward scattering: a broad warm halo plus a tight near-disk
    // around the sun. Both fade with `sun_intensity` at night.
    let mu = clamp(dot(rd, sky.sun_dir), -1.0, 1.0);
    let halo = pow(max(mu, 0.0), 8.0) * 0.5;
    let disk = pow(max(mu, 0.0), 350.0) * 5.0;
    col += sky.sun_color * (halo + disk) * sky.sun_intensity;

    return vec4<f32>(col, 1.0);
}
