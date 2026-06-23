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
    // World-space (RTC) surface position, for the view vector + waves.
    @location(1) world_pos: vec3<f32>,
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
    let world_pos = vec3<f32>(world, wz);
    out.clip_position = camera.view_proj * vec4<f32>(world_pos, 1.0);
    out.color = in.color;
    out.world_pos = world_pos;
    return out;
}

// Rotate a 2D direction by `a` radians.
fn rot2(d: vec2<f32>, a: f32) -> vec2<f32> {
    let c = cos(a);
    let s = sin(a);
    return vec2<f32>(d.x * c - d.y * s, d.x * s + d.y * c);
}

// One travelling sine wave (wavelength `wl` m, amplitude `amp` m, speed m/s,
// direction `dir`). Accumulates into `grad` (∂h/∂xy) and `height` (signed
// displacement, for whitecap crests). Returns vec3(grad.x, grad.y, height).
fn wave_term(p: vec2<f32>, t: f32, dir: vec2<f32>, wl: f32, amp: f32, speed: f32) -> vec3<f32> {
    let k = 6.2831853 / wl;
    let d = normalize(dir);
    let phase = k * (dot(p, d) + speed * t);
    let g = d * (amp * k * cos(phase)); // ∇(amp·sin(phase))
    return vec3<f32>(g.x, g.y, amp * sin(phase));
}

// Sum of wave octaves fanned around the forecast propagation direction, with
// amplitude scaled by sea-state ferocity (`wave_amp`). Returns
// vec4(normal.xyz, crest) where `crest` ∈ [0,1] is how close this point is to a
// wave crest (drives whitecaps). `normal` is the wave-perturbed up normal.
// `zoom` stretches every wavelength so the surface stays screen-constant across
// the map's huge zoom range: far away the waves become long, gentle swells
// (low slope ⇒ no normal aliasing, but still a living surface — not a flat
// sheet); up close they're crisp chop. Amplitude stays fixed, so a longer
// wavelength means a gentler slope — the anti-alias falls out for free.
fn wave_surface(p: vec2<f32>, t: f32, zoom: f32) -> vec4<f32> {
    let amp = water.wave_amp;
    let d0 = normalize(water.wave_dir + vec2<f32>(1e-5, 0.0));
    var grad = vec2<f32>(0.0, 0.0);
    var height = 0.0;
    // (angular spread off d0, wavelength m, base amplitude m, speed m/s)
    let w0 = wave_term(p, t, d0, 9.0 * zoom, 0.060 * amp, 1.10);
    let w1 = wave_term(p, t, rot2(d0, 0.42), 5.0 * zoom, 0.035 * amp, 0.90);
    let w2 = wave_term(p, t, rot2(d0, -0.70), 2.7 * zoom, 0.018 * amp, 1.40);
    grad = w0.xy + w1.xy + w2.xy;
    height = w0.z + w1.z + w2.z;
    // Total possible amplitude → normalise the crest measure to [0,1].
    let total_amp = (0.060 + 0.035 + 0.018) * amp;
    let crest = clamp(height / max(total_amp, 1e-5) * 0.5 + 0.5, 0.0, 1.0);
    let n = normalize(vec3<f32>(-grad.x, -grad.y, 1.0));
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
    // Ground-metre position (RTC) for physically-sized, zoom-stable waves.
    let inv = 1.0 / max(water.meters_to_world, 1e-12);
    let p_m = in.world_pos.xy * inv;
    // Eye→surface distance in real metres → zoom-matched wave scale. Far water
    // becomes long gentle swells (anti-aliased, still alive); close water is
    // crisp chop. This is what keeps the surface looking like the SAME ocean at
    // every map zoom instead of sub-pixel noise.
    let dist_m = length(water.eye - in.world_pos) * inv;
    let zoom = clamp(dist_m / 350.0, 1.0, 48.0);
    let surf = wave_surface(p_m, water.time, zoom);

    // Screen-space anti-alias. `fwidth(p_m)` is how many ground-metres a single
    // pixel covers — it captures BOTH distance and grazing foreshortening. Where
    // a pixel spans a large fraction of the (zoom-scaled) wavelength the waves
    // are sub-pixel and the raw normal aliases into the moiré "zigzag", so we
    // flatten the normal toward up there. Crisp waves survive only where they're
    // actually resolvable — no zigzag at any angle or zoom.
    let pix_m = max(fwidth(p_m.x), fwidth(p_m.y));
    let wl = 9.0 * zoom;
    let aa = clamp(1.0 - smoothstep(wl * 0.04, wl * 0.28, pix_m), 0.0, 1.0);
    // Anti-alias BOTH wave-derived signals together: the normal AND the crest
    // (the crest drives the scattering colour, and a sub-pixel crest is what was
    // aliasing the teal into moiré). Fade each toward its flat-sea value where a
    // pixel spans the wavelength.
    let n = normalize(mix(vec3<f32>(0.0, 0.0, 1.0), surf.xyz, aa));
    let crest = mix(0.5, surf.w, aa);

    // View vector (surface → eye).
    let v = normalize(water.eye - in.world_pos);
    let ndotv = max(dot(n, v), 1e-3);

    // Schlick Fresnel for water (F0 ≈ 0.02): near-grazing → reflective,
    // looking down → the body/scattering colour shows through.
    let f0 = 0.02;
    let fresnel = clamp(f0 + (1.0 - f0) * pow(1.0 - ndotv, 5.0), 0.0, 1.0);

    // Reflection uses a GENTLY-perturbed normal (mostly up), NOT the full sharp
    // wave normal. Over a long reflected-ray march, a sharp per-pixel normal
    // sends adjacent rays to wildly different terrain points → the mirror image
    // aliases into swirling moiré. A near-flat normal keeps the reflected image
    // coherent and just lets it ripple softly.
    let n_refl = normalize(mix(vec3<f32>(0.0, 0.0, 1.0), n, 0.18));
    let r = reflect(-v, n_refl);
    var reflection = sky_color(r);
    if (water.ssr_enabled > 0.5 && r.z > 0.02) {
        let terr = reflect_terrain(in.world_pos, r);
        if (terr.w > 0.5) {
            reflection = terr.rgb;
        }
    }

    // Subsurface scattering — the living colour of real sea. Sunlight that
    // enters the water scatters back out as a teal-green glow, brightest where
    // the wave bulges toward the light (crest) and where we look toward the sun
    // through the wave (back-scatter). This replaces the flat dark swatch.
    let deep = in.color.rgb * 0.45;                 // baked deep-water tint
    let sss_col = vec3<f32>(0.045, 0.17, 0.20);     // teal-green scatter
    let backscatter = pow(max(dot(v, -water.sun_dir), 0.0), 3.0);
    let sss = sss_col * (0.45 + 0.9 * crest + 1.3 * backscatter)
        * (0.30 + 0.70 * water.sun_intensity);
    let body = deep + sss;

    // Fresnel blends the body (looking into the water) toward the reflection
    // (grazing/mirror). A small floor keeps a hint of sky/terrain on the surface
    // even top-down, so it never reads as flat paint.
    var col = mix(body, reflection, max(fresnel, 0.06));

    // Sun glitter: a moderate HDR specular lobe about the half-vector — bloom
    // turns it into sparkle. Power kept sane so a sub-pixel wave normal can't
    // alias the highlight into banding; fades at night.
    let h = normalize(v + water.sun_dir);
    let spec = pow(max(dot(n, h), 0.0), 120.0);
    col += water.sun_color * spec * water.sun_intensity * 3.5;

    // Whitecaps: the top band of each wave crest breaks white when the sea
    // state is extreme (forecast-driven). Sharpened so only the very tops break.
    let breaking = water.whitecap * smoothstep(0.66, 0.90, crest);
    let foam_lit = vec3<f32>(1.0) * (0.40 + 0.60 * water.sun_intensity);
    col = mix(col, foam_lit, clamp(breaking, 0.0, 1.0));

    // Shoreline foam: a thin lively band where the water meets rising land.
    let shore = shoreline_foam(in.world_pos, water.time);
    col = mix(col, foam_lit, shore);

    return vec4<f32>(col, in.color.a * tile.tile_alpha);
}
