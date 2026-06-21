// Route/track tube: a single lit 3D mesh (not a tiled flat line), so the path
// reads as a real raised tube floating just above the terrain — no per-tile
// seams (the "spiderweb"), no painted-on look. Vertices are pre-extruded on the
// CPU in a route-local frame; here we just re-base to the camera origin (RTC,
// for f32 precision) and shade with the sun direction.

struct Uniforms {
    view_proj: mat4x4<f32>,
    // (route_origin - camera_origin) in world units — added per frame so the
    // baked, route-local xy stays small (f32-precise) as the camera moves.
    origin_delta: vec2<f32>,
    _pad0: vec2<f32>,
    // .x = pixels per world unit (so the tube is a constant screen thickness at
    //      every zoom — extruded here, not baked). .y = radius in px. .z = lift
    //      (centerline height above the surface, in tube radii).
    extrude: vec4<f32>,
    // Sun direction (world space, normalised) + ambient floor.
    sun_dir: vec3<f32>,
    ambient: f32,
    light_color: vec3<f32>,
    _pad1: f32,
};

@group(0) @binding(0) var<uniform> u: Uniforms;

struct VertexInput {
    @location(0) pos: vec3<f32>,     // route-local centerline xy + surface world z
    @location(1) normal: vec3<f32>,  // outward radial unit dir (extrude + lighting)
    @location(2) color: vec4<f32>,   // 8-bit sRGB, fed as Unorm
};

struct VertexOutput {
    @builtin(position) clip_position: vec4<f32>,
    @location(0) color: vec4<f32>,
    @location(1) normal: vec3<f32>,
};

@vertex
fn vs_main(in: VertexInput) -> VertexOutput {
    var out: VertexOutput;
    // Extrude the centerline by the radial normal to a constant *screen* radius,
    // and lift the centerline above the surface by `lift` radii so the tube's
    // underside rests just over the terrain. Same world units in xy and z, so it
    // reads as a round tube under the perspective transform.
    let r = u.extrude.y / max(u.extrude.x, 1e-6);
    let center = vec3<f32>(in.pos.xy + u.origin_delta, in.pos.z + u.extrude.z * r);
    let world = center + in.normal * r;
    out.clip_position = u.view_proj * vec4<f32>(world, 1.0);
    out.color = in.color;
    out.normal = in.normal;
    return out;
}

@fragment
fn fs_main(in: VertexOutput) -> @location(0) vec4<f32> {
    // Lambert with an ambient floor, so the tube's curve catches the light and
    // reads as 3D rather than a flat band. Two-sided (abs) so inward-facing
    // triangles near the silhouette don't go black.
    let n = normalize(in.normal);
    let diffuse = abs(dot(n, normalize(u.sun_dir)));
    let lit = u.ambient + (1.0 - u.ambient) * diffuse;
    return vec4<f32>(in.color.rgb * u.light_color * lit, in.color.a);
}
