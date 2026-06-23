// Realistic water surface. Draws the water-body fills split out of the vector
// mesh (`source_layer == "water"`) as an animated, reflective liquid surface
// instead of a flat matte fill.
//
// Vertex stage: identical tile placement + DEM draping to `vector_shader.wgsl`
// (groups 0/1/2 are bound from the very same VectorPipeline buffers), so water
// lies on the terrain exactly where the flat fill used to. It additionally
// passes the world-space (RTC) position to the fragment stage.
//
// Fragment stage: a wave-perturbed normal drives a Fresnel blend between a deep
// water-body tint (the baked style colour) and a reflection of the analytic
// atmosphere (the SAME colour function as `sky_shader.wgsl` — kept in sync by
// hand; see `sky_color`), plus an HDR sun glitter that feeds the bloom pass.

// ---- group 0/1/2: shared with the vector pipeline -------------------------
struct CameraUniform {
    view_proj: mat4x4<f32>,
    // .x = pixels per world unit, .y = meters_to_world·exaggeration (drape
    // z-scale; 0 ⇒ flat), .z = DEM encoding, .w = halo_uv inset.
    params: vec4<f32>,
};
struct TileUniform {
    tile_alpha: f32,
    use_paint_color: f32,
    dash_len: f32,
    gap_len: f32,
    paint_color: vec4<f32>,
    origin: vec2<f32>,
    span: f32,
    width_scale: f32,
    dem_uv_origin: vec2<f32>,
    dem_uv_size: f32,
};
@group(0) @binding(0) var<uniform> camera: CameraUniform;
@group(1) @binding(0) var<uniform> tile: TileUniform;
@group(2) @binding(0) var dem_tex: texture_2d<f32>;
@group(2) @binding(1) var dem_samp: sampler;

// ---- group 3: water lighting + animation ----------------------------------
struct WaterGlobals {
    // Direction TOWARD the sun (world frame, x=E y=S z=up) + glow intensity
    // (→0 at night). Same values the sky/terrain use, so reflections match.
    sun_dir: vec3<f32>,
    sun_intensity: f32,
    // Analytic-sky palette (linear RGB), shared with the sky pass.
    zenith_color: vec3<f32>,
    // Renderer wall-clock seconds, drives the wave animation.
    time: f32,
    horizon_color: vec3<f32>,
    // 1 / metres-per-world at the camera latitude — converts the RTC world
    // position to ground metres so waves have a physical, zoom-stable size.
    meters_to_world: f32,
    sun_color: vec3<f32>,
    // 1.0 ⇒ march the reflected ray against the terrain heightfield (group 4) to
    // mirror the surrounding mountains; 0.0 ⇒ reflect the analytic sky only.
    ssr_enabled: f32,
    // Camera eye in the RTC frame (relative to the tile origin frame), for the
    // per-fragment view vector.
    eye: vec3<f32>,
    // Shoreline-foam intensity (0 = none).
    foam: f32,
    // Heightfield placement (shared with the shadow/AO field): UV =
    // (world.xy - hf_origin) * hf_inv_size. `hf_origin` is in this frame's RTC.
    hf_origin: vec2<f32>,
    hf_inv_size: f32,
    // Heightfield texel (its steeper world-z) → mesh world-z: ×cos²lat.
    hf_to_mesh_z: f32,
    // Dominant wave propagation direction (unit, world x=E/y=S) from the forecast.
    wave_dir: vec2<f32>,
    // Sea-state ferocity (amplitude/steepness multiplier) and whitecap amount.
    wave_amp: f32,
    whitecap: f32,
    // 1 ⇒ realistic AAA water (Gerstner displacement + waves + reflection +
    // glitter); 0 ⇒ flat matte body-colour fill (rail toggle off).
    realistic: f32,
};
@group(3) @binding(0) var<uniform> water: WaterGlobals;
// Terrain heightfield (world-z elevations, R32Float, non-filterable) — the same
// field the cast-shadow + AO passes march. Folded into group 3 (binding 1)
// because devices cap `max_bind_groups` at 4. Sampled with `textureLoad`.
@group(3) @binding(1) var height_tex: texture_2d<f32>;
const HF_DIM: i32 = 256;

fn decode_elevation(enc: u32, rgb: vec3<f32>) -> f32 {
    let r = rgb.r * 255.0;
    let g = rgb.g * 255.0;
    let b = rgb.b * 255.0;
    if (enc == 1u) {
        return r * 256.0 + g + b / 256.0 - 32768.0; // Terrarium
    } else {
        return -10000.0 + (r * 256.0 * 256.0 + g * 256.0 + b) * 0.1;
    }
}

struct VertexInput {
    @location(0) base: vec2<f32>,
    @location(1) normal: vec2<f32>,
    @location(2) width_px: f32,
    @location(3) color: vec4<f32>,
    @location(4) edge_pos: vec4<f32>,
    @location(5) dist: f32,
    @location(6) z: f32,
};

struct VertexOutput {
    @builtin(position) clip_position: vec4<f32>,
    // Deep-water tint (baked style colour, linear).
    @location(0) color: vec4<f32>,
    // World-space (RTC) surface position (post-displacement), for the view
    // vector + fragment waves.
    @location(1) world_pos: vec3<f32>,
    // Geometric normal of the Gerstner swell (z-up), perturbed further in the FS.
    @location(2) wave_normal: vec3<f32>,
    // Distance to the nearest shore edge, tile-local units (baked into `dist`).
    // The FS turns it into a depth-based absorption tint (shallow → deep).
    @location(3) shore: f32,
};

@vertex
fn vs_main(in: VertexInput) -> VertexOutput {
    var out: VertexOutput;
    // Water fills carry width_px 0, so no stroke extrusion — placement is the
    // plain tile-local → world transform, draped onto the DEM exactly like the
    // vector fills it replaces.
    let world = tile.origin + in.base * tile.span;
    var wz = in.z;
    let zscale = camera.params.y;
    if (zscale > 0.0) {
        let dem_uv = tile.dem_uv_origin + in.base * tile.dem_uv_size;
        let s = textureSampleLevel(dem_tex, dem_samp, dem_uv, 0.0);
        if (s.a >= 0.5) {
            wz = wz + decode_elevation(u32(camera.params.z), s.rgb) * zscale;
        }
    }
    var world_pos = vec3<f32>(world, wz);
    var wave_n = vec3<f32>(0.0, 0.0, 1.0);
    // Gerstner swell: displace the (refined) grid into real 3-D crests. Work in
    // metres so the waves are physically sized + zoom-stable, then convert back —
    // xy with the true world-per-metre scale, z with the terrain drape z-scale so
    // wave height matches the relief's vertical exaggeration. Damp toward flat with
    // distance: far/coarse tiles can't resolve crests, so calm them (no aliasing).
    // Skipped entirely when the realistic path is off (flat fill).
    if (water.realistic > 0.5) {
        let inv_m = 1.0 / max(water.meters_to_world, 1e-12);
        let p_m = world * inv_m;
        let dist_m = length(water.eye - world_pos) * inv_m;
        let amp_scale = clamp(1.0 - smoothstep(2200.0, 12000.0, dist_m), 0.0, 1.0);
        let gw = gerstner(p_m, water.time, amp_scale);
        world_pos += vec3<f32>(
            gw.offset.xy * water.meters_to_world,
            gw.offset.z * zscale,
        );
        wave_n = gw.normal;
    }
    out.clip_position = camera.view_proj * vec4<f32>(world_pos, 1.0);
    out.color = in.color;
    out.world_pos = world_pos;
    out.wave_normal = wave_n;
    out.shore = in.dist;
    return out;
}

// Rotate a 2D direction by `a` radians.
fn rot2(d: vec2<f32>, a: f32) -> vec2<f32> {
    let c = cos(a);
    let s = sin(a);
    return vec2<f32>(d.x * c - d.y * s, d.x * s + d.y * c);
}

// ---- Gerstner swell (vertex displacement) ---------------------------------
// A few long trochoidal waves that actually displace the mesh into 3-D crests
// (GPU Gems Ch.1 Eq.9–12) — the *geometry* of the sea. Crests pinch toward their
// peaks (the xy term) and rise (the z term); the fragment stage then adds the
// fine exp(sin) ripple detail on top as a normal map. All metre-space.
const GERSTNER_WAVES: i32 = 4;

struct Gerstner {
    // Metre-space displacement: xy = trochoidal crest pinch, z = wave height.
    offset: vec3<f32>,
    // Analytic surface normal (unitless, z-up), already amplitude-scaled.
    normal: vec3<f32>,
};

// Sum the swell at metre-space point `p_m` and time `t`. `amp_scale` damps the
// whole surface toward flat (distance LOD — coarse far tiles can't resolve crests).
fn gerstner(p_m: vec2<f32>, t: f32, amp_scale: f32) -> Gerstner {
    let g = 9.81;
    let base_dir = normalize(water.wave_dir + vec2<f32>(1e-4, 0.0));
    var disp = vec3<f32>(0.0, 0.0, 0.0);
    var slope = vec2<f32>(0.0, 0.0); // Σ d·(w·A)·cos — the xy gradient of height
    var wavelen = 28.0;              // dominant wavelength (m)
    var amp = 0.42 * water.wave_amp; // dominant amplitude (m)
    var ang = 0.0;
    for (var i = 0; i < GERSTNER_WAVES; i = i + 1) {
        let w = 6.2831853 / wavelen;     // spatial frequency k = 2π/L
        let speed = sqrt(g * w);         // deep-water dispersion ω = √(gk)
        let d = rot2(base_dir, ang);
        let theta = w * dot(d, p_m) + speed * t;
        let c = cos(theta);
        let s = sin(theta);
        // Steepness Q kept so Σ Q·w·A ≤ 1 (no looping/self-intersecting crests).
        let q = 0.78 / (w * max(amp, 1e-4) * f32(GERSTNER_WAVES));
        disp.x += q * amp * d.x * c;
        disp.y += q * amp * d.y * c;
        disp.z += amp * s;
        slope += d * (w * amp) * c;
        // Next octave: shorter, smaller, fanned to alternating sides of the swell.
        wavelen *= 0.62;
        amp *= 0.62;
        ang += 0.55 * select(-1.0, 1.0, (i & 1) == 0);
    }
    var out: Gerstner;
    out.offset = disp * amp_scale;
    // Normal of z = height(x,y): (−∂h/∂x, −∂h/∂y, 1), slope damped with amplitude.
    out.normal = normalize(vec3<f32>(-slope * amp_scale, 1.0));
    return out;
}

// One exp(sin) wavelet + its derivative. `exp(sin(x)-1)` has sharp crests and
// broad troughs (trochoidal-ish); the derivative drives domain warping. Summed
// plain sines always interfere into a regular corduroy/cross-hatch — exp(sin)
// with domain warp does not.
fn wavedx(p: vec2<f32>, dir: vec2<f32>, freq: f32, t: f32) -> vec2<f32> {
    let x = dot(dir, p) * freq + t;
    let wave = exp(sin(x) - 1.0);
    return vec2<f32>(wave, -wave * cos(x));
}

// Domain-warped sum of exp(sin) wavelets over pseudo-randomly rotated directions
// (Alekseev "Seascape" technique). Each octave warps the sample position by the
// previous derivative, clustering ripples on crests and breaking the regular
// interference — natural, non-repeating chop. Returns wave HEIGHT in metres.
fn getwaves(p_in: vec2<f32>, t: f32, zoom: f32, lod: f32) -> f32 {
    var p = p_in;
    var iter = 0.0;
    var freq = 6.2831853 / (11.0 * zoom); // base wavelength ~11·zoom m
    var tmul = 1.0;
    var weight = 1.0;
    var sum = 0.0;
    var tw = 0.0;
    for (var i = 0; i < 8; i = i + 1) {
        // Per-octave LOD: the finest octaves fade in only when they're
        // resolvable (lod high = close). This anti-aliases by dropping detail
        // that would shimmer, WITHOUT flattening the whole surface — the coarse
        // ripples stay alive so the reflection keeps moving (no clay).
        let oct = clamp(lod - f32(i), 0.0, 1.0);
        let d = vec2<f32>(sin(iter), cos(iter));
        let res = wavedx(p, d, freq, t * tmul);
        p = p + d * res.y * weight * oct * 0.42; // domain warp
        sum = sum + res.x * weight * oct;
        tw = tw + weight * oct;
        weight = weight * 0.82;
        freq = freq * 1.18;
        tmul = tmul * 1.07;
        iter = iter + 2.39996; // golden-angle direction spread
    }
    return (sum / max(tw, 1e-4)) * water.wave_amp * 0.5;
}

// Wave-perturbed normal (.xyz) + crest measure (.w). Normal from finite
// differences of the height field; epsilon scales with zoom so the slope stays
// physical at every wavelength. `steep` exaggerates the slope so the ripples
// read strongly on a flat mesh (normal-mapped water needs punchy normals to not
// look like clay). `lod` controls how many octaves resolve (distance AA).
fn wave_surface(p: vec2<f32>, t: f32, zoom: f32, lod: f32) -> vec4<f32> {
    let amp_m = max(water.wave_amp * 0.5, 1e-4);
    let e = 0.5 * zoom;
    let h = getwaves(p, t, zoom, lod);
    let hx = getwaves(p + vec2<f32>(e, 0.0), t, zoom, lod);
    let hy = getwaves(p + vec2<f32>(0.0, e), t, zoom, lod);
    let steep = 2.6;
    let n = normalize(vec3<f32>(-(hx - h) / e * steep, -(hy - h) / e * steep, 1.0));
    let crest = clamp(h / amp_m * 0.5 + 0.5, 0.0, 1.0);
    return vec4<f32>(n, crest);
}

// Shoreline foam: probe the heightfield in a ring around this point; where land
// rises above the water surface nearby, we're at a shore → foam. Returns 0..1.
// 0 when no heightfield is bound (open sea / no terrain). Animated edge wobble
// keeps the foam line alive rather than a hard band.
fn shoreline_foam(world_pos: vec3<f32>, t: f32) -> f32 {
    if (water.ssr_enabled < 0.5 || water.foam <= 0.0) {
        return 0.0;
    }
    let texel_world = 1.0 / (water.hf_inv_size * f32(HF_DIM));
    let radius = texel_world * 2.0;
    var max_rise = 0.0;
    // 6-tap ring.
    for (var i = 0; i < 6; i = i + 1) {
        let a = f32(i) * 1.0472; // 60°
        let off = vec2<f32>(cos(a), sin(a)) * radius;
        let tz = terrain_mesh_z(hf_uv(world_pos.xy + off));
        max_rise = max(max_rise, tz - world_pos.z);
    }
    // Rise of ~1.5 texels of relief over the water level → full foam. Wobble the
    // band with a little travelling noise so it breathes.
    let pm = world_pos.xy / max(water.meters_to_world, 1e-12);
    let wob = 0.6 + 0.4 * sin(dot(pm, water.wave_dir) * 0.6 + t * 1.5);
    let band = smoothstep(0.0, texel_world * 1.5, max_rise)
        * (1.0 - smoothstep(texel_world * 1.5, texel_world * 4.0, max_rise));
    return clamp(band * wob * water.foam, 0.0, 1.0);
}

// Analytic atmosphere colour for a world-space ray direction. Mirrors the
// zenith↔horizon gradient + Mie sun halo/disk of `sky_shader.wgsl::fs_main`
// (minus the stars). Kept in sync by hand; if one changes, change both.
fn sky_color(rd: vec3<f32>) -> vec3<f32> {
    let up = clamp(rd.z, -1.0, 1.0);
    let t = pow(clamp(up, 0.0, 1.0), 0.42);
    var col = mix(water.horizon_color, water.zenith_color, t);
    if (up < 0.0) {
        col = mix(water.horizon_color, water.horizon_color * 0.72, clamp(-up * 4.0, 0.0, 1.0));
    }
    let mu = clamp(dot(rd, water.sun_dir), -1.0, 1.0);
    let halo = pow(max(mu, 0.0), 8.0) * 0.5;
    let disk = pow(max(mu, 0.0), 350.0) * 5.0;
    col += water.sun_color * (halo + disk) * water.sun_intensity;
    return col;
}

// World (RTC) xy → heightfield UV.
fn hf_uv(world_xy: vec2<f32>) -> vec2<f32> {
    return (world_xy - water.hf_origin) * water.hf_inv_size;
}

// Terrain elevation in MESH world-z at a heightfield UV (clamped to the field).
fn terrain_mesh_z(uv: vec2<f32>) -> f32 {
    let t = uv * f32(HF_DIM);
    let xi = clamp(i32(t.x), 0, HF_DIM - 1);
    let yi = clamp(i32(t.y), 0, HF_DIM - 1);
    let h = textureLoad(height_tex, vec2<i32>(xi, yi), 0).r;
    return h * water.hf_to_mesh_z;
}

// March the reflected ray against the heightfield to mirror the surrounding
// terrain. Returns `vec4(reflected_colour, hit)` where `hit` is 1.0 on a terrain
// intersection inside the field, else 0.0 (caller falls back to the sky).
fn reflect_terrain(p: vec3<f32>, r: vec3<f32>) -> vec4<f32> {
    // One heightfield texel in world units — the march step + gradient epsilon.
    let texel_world = 1.0 / (water.hf_inv_size * f32(HF_DIM));
    // Cover ~1.5 fields; bias the start off the surface to avoid self-hit.
    let span = 1.5 / water.hf_inv_size;
    let steps = 48;
    let dt = span / f32(steps);
    var t = dt;
    var hit_uv = vec2<f32>(0.0);
    var found = false;
    for (var i = 0; i < steps; i = i + 1) {
        let s = p + r * t;
        let uv = hf_uv(s.xy);
        // Outside the assembled field → can't resolve; stop and miss (sky).
        if (uv.x < 0.0 || uv.x > 1.0 || uv.y < 0.0 || uv.y > 1.0) {
            break;
        }
        let tz = terrain_mesh_z(uv);
        if (s.z < tz - texel_world * 0.05) {
            // Crossed below the surface — refine once between the last two samples.
            let s0 = p + r * (t - dt);
            let mid = (s0 + s) * 0.5;
            hit_uv = hf_uv(mid.xy);
            found = true;
            break;
        }
        t = t + dt;
    }
    if (!found) {
        return vec4<f32>(0.0, 0.0, 0.0, 0.0);
    }
    // Shade the reflected terrain: a hillshade from the heightfield gradient,
    // lit by the same sun. Approximate colour (no basemap texture in this pass),
    // but the lit/shadowed slopes read as mountains mirrored in the water.
    let du = vec2<f32>(1.0 / f32(HF_DIM), 0.0);
    let dv = vec2<f32>(0.0, 1.0 / f32(HF_DIM));
    let zx = terrain_mesh_z(hit_uv + du) - terrain_mesh_z(hit_uv - du);
    let zy = terrain_mesh_z(hit_uv + dv) - terrain_mesh_z(hit_uv - dv);
    let nrm = normalize(vec3<f32>(-zx, -zy, 2.0 * texel_world));
    let lambert = max(dot(nrm, water.sun_dir), 0.0);
    let albedo = vec3<f32>(0.20, 0.23, 0.18);
    let lit = albedo * (0.40 + 0.85 * lambert * water.sun_intensity);
    return vec4<f32>(lit, 1.0);
}

@fragment
fn fs_main(in: VertexOutput) -> @location(0) vec4<f32> {
    // Flat path (rail toggle off): the plain matte body fill, as before AAA.
    if (water.realistic < 0.5) {
        return vec4<f32>(in.color.rgb, in.color.a * tile.tile_alpha);
    }
    // Ground-metre position (RTC) for physically-sized, zoom-stable waves.
    let inv = 1.0 / max(water.meters_to_world, 1e-12);
    let p_m = in.world_pos.xy * inv;
    // Eye→surface distance in real metres → zoom-matched wave scale. Far water
    // becomes long gentle swells (anti-aliased, still alive); close water is
    // crisp chop. This is what keeps the surface looking like the SAME ocean at
    // every map zoom instead of sub-pixel noise.
    let dist_m = length(water.eye - in.world_pos) * inv;
    // Mild zoom-scale (keeps waves near real-scale ocean texture) + per-octave
    // LOD: far away only the coarse ripples resolve, close up the full chop. This
    // anti-aliases by dropping the finest octaves, NOT by flattening the surface —
    // so the normals keep rippling the reflection (no clay) at every zoom.
    let zoom = clamp(dist_m / 700.0, 1.0, 18.0);
    let lod = mix(2.5, 8.0, clamp(1.0 - smoothstep(500.0, 9000.0, dist_m), 0.0, 1.0));
    let surf = wave_surface(p_m, water.time, zoom, lod);
    // Combine the big-swell geometric normal (from the displaced vertices) with
    // the fine ripple detail: tilt the interpolated swell normal by the detail
    // slope so the surface shimmers on top of the real 3-D crests instead of
    // replacing them. Detail weight backs off so it perturbs, not dominates.
    let swell_n = normalize(in.wave_normal);
    let n = normalize(swell_n + vec3<f32>(surf.x, surf.y, 0.0) * 0.6);
    let crest = surf.w;

    // View vector (surface → eye).
    let v = normalize(water.eye - in.world_pos);
    let ndotv = max(dot(n, v), 1e-3);

    // Fresnel with a high FLOOR — real water is strongly reflective at ALL angles
    // (the "wet mirror"). Without the floor, looking down shows only the body
    // colour and the surface reads as matte clay. F0 0.02, floor 0.30.
    let fres = clamp(0.14 + 0.86 * pow(1.0 - ndotv, 5.0), 0.0, 1.0);

    // Reflection — the heart of the wet look. The FULL wave normal ripples the
    // sky reflection so the surface shimmers (sky is a smooth gradient → no
    // aliasing from rippling it). The terrain mirror march uses a smoothed normal
    // so the reflected mountains stay coherent, then blends in.
    let r_sky = reflect(-v, n);
    var reflection = sky_color(r_sky);
    let n_terr = normalize(mix(vec3<f32>(0.0, 0.0, 1.0), n, 0.30));
    let r_terr = reflect(-v, n_terr);
    if (water.ssr_enabled > 0.5 && r_terr.z > 0.02) {
        let terr = reflect_terrain(in.world_pos, r_terr);
        if (terr.w > 0.5) {
            reflection = mix(reflection, terr.rgb, 0.85);
        }
    }

    // Depth-based absorption (Beer-Lambert, proxied by distance to shore — the
    // shallowness cue baked into `shore`). Near an edge the bed is close, so the
    // water is bright + turquoise and lets the basemap show through; toward the
    // interior light is absorbed → dark, saturated blue and opaque. `shore` is in
    // tile-local units; `span / meters_to_world` converts it to ground metres.
    let shore_m = in.shore * tile.span / max(water.meters_to_world, 1e-12);
    let depth01 = smoothstep(0.0, 55.0, shore_m);
    let shallow_tint = in.color.rgb * 1.30 + vec3<f32>(0.03, 0.09, 0.09);
    let deep_tint = in.color.rgb * 0.30;
    let deep = mix(shallow_tint, deep_tint, depth01);
    // Subtle subsurface glow on crests. A TINT only — water is reflection-dominated.
    let sss_col = vec3<f32>(0.03, 0.13, 0.16);
    let sss = sss_col * (0.5 + 0.7 * crest) * (0.3 + 0.7 * water.sun_intensity);
    let body = deep + sss;

    var col = mix(body, reflection, fres);

    // Sun glitter: a tight HDR specular lobe on the rippling normal — bloom turns
    // it into sparkle scattered across the wave crests. Fades at night.
    let h = normalize(v + water.sun_dir);
    let spec = pow(max(dot(n, h), 0.0), 200.0);
    col += water.sun_color * spec * water.sun_intensity * 6.0;

    // Whitecaps: the top band of each wave crest breaks white when the sea
    // state is extreme (forecast-driven). Sharpened so only the very tops break.
    let breaking = water.whitecap * smoothstep(0.66, 0.90, crest);
    let foam_lit = vec3<f32>(1.0) * (0.40 + 0.60 * water.sun_intensity);
    col = mix(col, foam_lit, clamp(breaking, 0.0, 1.0));

    // Shoreline foam: a thin lively band where the water meets rising land.
    let foam_band = shoreline_foam(in.world_pos, water.time);
    col = mix(col, foam_lit, foam_band);

    // Shallow + looking-down water lets the bed (basemap under the water) show
    // through; grazing angles stay an opaque mirror (high Fresnel). Foam is opaque.
    let bed_reveal = (1.0 - depth01) * (1.0 - fres) * (1.0 - foam_band);
    let alpha = in.color.a * tile.tile_alpha * (1.0 - 0.45 * bed_reveal);
    return vec4<f32>(col, alpha);
}
