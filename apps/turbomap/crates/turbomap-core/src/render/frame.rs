//! Per-frame render globals.
//!
//! `render()` used to open with ~100 lines computing a pile of loose locals —
//! `meters_to_world`, the sun + atmosphere palette, the aerial-perspective
//! `haze_density`, the sky uniform, the raster/vector terrain configs — and
//! then thread each one by hand into the raster, vector, hillshade and sky
//! passes. They're all derived from the same three inputs (camera, viewport,
//! sun) plus the terrain options, recomputed identically every frame.
//!
//! [`RenderFrame`] makes that an explicit, computed-once value: the Map builds
//! one at the top of the frame and the passes read named fields off it. No
//! behaviour change — the math is lifted verbatim — but the frame's derived
//! state is now a single typed thing instead of a scatter of `let`s.

use crate::camera::Camera;
use crate::dem::DemEncoding;
use crate::sun::{self, SunPosition};

use super::floor::FloorGlobals;
use super::raster::TerrainConfig;
use super::sky::SkyGlobals;

/// The terrain inputs a [`RenderFrame`] needs, pulled off the Map's optional
/// terrain so the frame builder doesn't have to know about `Terrain` itself.
/// `present == false` collapses all displacement to flat (the math below
/// forces `meters_to_world` to 0).
pub(crate) struct TerrainFrameInputs {
    pub present: bool,
    pub exaggeration: f32,
    pub encoding: DemEncoding,
    /// DEM tile halo in pixels (0 when no terrain / no halo).
    pub halo_px: u32,
}

/// Everything derived once per frame from the camera + sun + terrain, ready to
/// hand to the render passes.
pub(crate) struct RenderFrame {
    /// `1 / metres-per-world` at the camera latitude — the vertex
    /// displacement scale. 0 when no terrain (mesh draws flat).
    pub meters_to_world: f32,
    /// The raster ground-plane terrain config (also the source of the
    /// vector drape params below).
    pub raster_terrain_cfg: TerrainConfig,
    /// Combined z-scale for the vector pipeline's drape (meters_to_world ×
    /// exaggeration). 0 → vector geometry stays flat.
    pub vec_terrain_zscale: f32,
    /// DEM encoding tag for the vector shader (0 = MapboxRgb, 1 = Terrarium).
    pub vec_terrain_encoding: u32,
    /// DEM halo inset in UV space for the vector shader's tile sampling.
    pub vec_terrain_halo_uv: f32,
    /// Sky uniform, present only when the camera is tilted enough to expose
    /// the horizon (pure top-down stays byte-identical, so the flat golden
    /// holds). `None` → skip the sky pass.
    pub sky_globals: Option<SkyGlobals>,
    /// Floor backstop uniform — a sea-grey plane just below sea level that fills
    /// gaps where terrain tiles haven't streamed in. `None` → skip (flat 2D / no
    /// terrain). Drawn after the sky, before the terrain.
    pub floor_globals: Option<FloorGlobals>,
}

impl RenderFrame {
    /// Compute the frame's render globals. Lifted verbatim from the head of
    /// `Map::render`; see the field docs for what each drives.
    pub fn build(
        camera: &Camera,
        viewport_px: (u32, u32),
        sun: SunPosition,
        terrain: TerrainFrameInputs,
        sky_enabled: bool,
    ) -> Self {
        // Mercator metres→world units at the camera latitude:
        //   metres_per_world = 2π·R / cos(lat); meters_to_world = 1/that.
        let lat = camera.center.lat.to_radians();
        let earth_circumference_m: f32 = 40_075_017.0;
        let meters_to_world = (lat.cos().abs() as f32 / earth_circumference_m).max(1e-12);

        // Sun + time-of-day palette: one light for the whole scene.
        let atmos = sun::atmosphere(sun);

        // Sun glow intensity, fading out as the sun sets (gone a touch below the
        // horizon). Shared by the sky pass and the water reflection so both dim
        // together at dusk.
        let sun_intensity = {
            let a = sun.altitude_deg;
            let t = ((a + 3.0) / 9.0).clamp(0.0, 1.0);
            t * t * (3.0 - 2.0 * t)
        };

        // Aerial-perspective density. The shader computes
        // `1 - exp(-dist_world · haze_density)` over the TRUE eye→fragment
        // distance (RTC frame; see the `eye_world` plumbing). Density is
        // altitude-relative — `k / altitude_world` — so BOTH terms are in world
        // units (no unit mismatch) and the look is zoom-stable. The key property:
        // the nearest visible ground sits ~one altitude from the eye, so its
        // haze is bounded near `1 - exp(-k)` (~0.2) at ANY pitch — it can never
        // white out — while the far field (dist ≫ altitude) dissolves to the
        // horizon colour. That makes the old grazing-angle taper unnecessary:
        // eye-distance + bounded near field replaces the hack that the
        // center-distance model needed. A pitch ramp gates haze to 0 on the
        // flat 2D map (it's a tilt-only depth cue).
        let haze_density = aerial_haze_density(camera.pitch_deg);
        // Bluish atmospheric tint for the far distance — the horizon colour alone
        // is near-white (reads grey over dark imagery), so pull it toward the
        // saturated blue zenith. At golden hour both go warm, so dusk haze warms
        // automatically (and the shader's sun-glow term adds forward in-scatter).
        let haze_tint = {
            let (h, z, t) = (atmos.horizon_color, atmos.zenith_color, 0.55_f32);
            [
                h[0] + (z[0] - h[0]) * t,
                h[1] + (z[1] - h[1]) * t,
                h[2] + (z[2] - h[2]) * t,
            ]
        };

        // Sky: only when tilted enough to expose the horizon, and not suppressed
        // (the debug viewer can turn it off to isolate the water/terrain).
        let draw_sky = sky_enabled && camera.pitch_deg > 0.5;
        let sky_globals = if draw_sky {
            let origin = camera.center.to_world();
            let vp = camera.view_projection_matrix_rtc(origin, viewport_px);
            let inv_view_proj = glam::Mat4::from_cols_array_2d(&vp)
                .inverse()
                .to_cols_array_2d();
            // A near-singular VP can invert to NaN; never feed that to the sky
            // shader (mobile drivers hang). Skip the sky pass this frame —
            // terrain/vectors overdraw it anyway except at the horizon.
            if !super::mat4_is_finite(&inv_view_proj) {
                None
            } else {
                Some(SkyGlobals {
                    inv_view_proj,
                    sun_dir: sun.world_dir(),
                    sun_intensity,
                    zenith_color: atmos.zenith_color,
                    _p0: 0.0,
                    horizon_color: atmos.horizon_color,
                    _p1: 0.0,
                    sun_color: atmos.light_color,
                    _p2: 0.0,
                })
            }
        } else {
            None
        };

        // Floor backstop: a sea-grey plane just below sea level filling any gap
        // where terrain tiles haven't loaded (or a seam gapes). Only in 3D
        // (pitched) with terrain present. Droops with the SAME curvature term as
        // the terrain (`earth_curvature_coeff`) so it never pokes up through the
        // curved-away far field.
        let floor_globals = if draw_sky && terrain.present {
            let origin = camera.center.to_world();
            let vp = camera.view_projection_matrix_rtc(origin, viewport_px);
            let inv_view_proj = glam::Mat4::from_cols_array_2d(&vp)
                .inverse()
                .to_cols_array_2d();
            if !super::mat4_is_finite(&inv_view_proj) {
                None
            } else {
                let coslat = (camera.center.lat.to_radians().cos() as f32).abs();
                Some(FloorGlobals {
                    view_proj: vp,
                    inv_view_proj,
                    // Muted sea-grey (linear RGB).
                    color: [0.27, 0.31, 0.36, 1.0],
                    // ~150 m below sea level, in world-z.
                    floor_z: -150.0 * meters_to_world,
                    curvature_coeff: crate::camera::earth_curvature_coeff(coslat),
                    enabled: 1.0,
                    _pad: 0.0,
                })
            }
        } else {
            None
        };

        // Raster ground-plane terrain config. No terrain → meters_to_world 0,
        // so the shader displacement collapses and the mesh draws flat.
        let raster_terrain_cfg = TerrainConfig {
            meters_to_world: if terrain.present {
                meters_to_world
            } else {
                0.0
            },
            exaggeration: if terrain.present {
                terrain.exaggeration
            } else {
                1.0
            },
            encoding: match terrain.encoding {
                DemEncoding::MapboxRgb => 0u32,
                DemEncoding::Terrarium => 1u32,
            },
            sun_dir: sun.world_dir(),
            ambient: atmos.ambient,
            haze_color: haze_tint,
            haze_density,
            light_color: atmos.light_color,
            // Cast shadows are off here; `Map::render` computes the shadow
            // field (it needs the terrain cache + visible region, which this
            // builder doesn't see) and patches these fields when enabled.
            shadow_origin: [0.0, 0.0],
            shadow_inv_size: 0.0,
            shadow_strength: 0.0,
            shadow_texel_world: 0.0,
            shadow_softness: 1.0,
            // Stamped in `Map::render` from the renderer's wall clock (this
            // pure builder has no clock); drives the haze drift.
            time: 0.0,
            // Patched in `Map::render` from `Map::basemap_gain` (this builder
            // doesn't see the active basemap). 1.0 = no change.
            basemap_gain: 1.0,
            // Patched in `Map::render` from `Map::terrain_lit` (host "sun mode").
            lit: true,
        };

        // Vector drape params, derived from the raster config.
        let vec_terrain_zscale =
            raster_terrain_cfg.meters_to_world * raster_terrain_cfg.exaggeration;
        let vec_terrain_encoding = raster_terrain_cfg.encoding;
        let vec_terrain_halo_uv = {
            let halo = terrain.halo_px;
            if halo == 0 {
                0.0
            } else {
                halo as f32 / (256.0 + 2.0 * halo as f32)
            }
        };

        Self {
            meters_to_world,
            raster_terrain_cfg,
            vec_terrain_zscale,
            vec_terrain_encoding,
            vec_terrain_halo_uv,
            sky_globals,
            floor_globals,
        }
    }
}

/// 0..1 PITCH gate for the aerial-perspective effect, fed to the shader as
/// `haze_density`. The shader applies a zoom-relative depth haze only where this
/// is > 0; gating on pitch means a top-down / looking-down view carries NONE
/// (no nadir "circle", no sudden wash when you start to tilt) — the colour
/// appears only as you gaze toward the horizon. The shader's relative-depth term
/// keeps the foreground crisp; this decides *whether* the far field colours at
/// all based on view angle.
fn aerial_haze_density(pitch_deg: f64) -> f32 {
    // Off until 25° (still inspecting the ground), full by 55° (gazing out).
    let t = (((pitch_deg - 25.0) / 30.0) as f32).clamp(0.0, 1.0);
    t * t * (3.0 - 2.0 * t) // smoothstep
}

#[cfg(test)]
mod tests {
    //! Aerial-perspective invariants. The look is a ZOOM-RELATIVE depth haze in
    //! the shader (depth = eye→fragment distance / camera altitude), gated by
    //! pitch. Here we test (a) the pitch gate and (b) reproduce the shader's
    //! relative-depth ramp to assert: the foreground is crisp at ANY zoom, only
    //! the far field colours, and it's zoom-invariant (same look at 10× altitude
    //! with 10× distances). Colour can't be judged headlessly (pale-basemap).

    use super::aerial_haze_density;

    #[test]
    fn pitch_gate_off_looking_down_on_only_toward_horizon() {
        for p in [0.0_f64, 10.0, 25.0] {
            assert_eq!(
                aerial_haze_density(p),
                0.0,
                "no haze looking down at {p}° (kills nadir circle)"
            );
        }
        assert!(
            aerial_haze_density(40.0) > 0.0 && aerial_haze_density(40.0) < 1.0,
            "fades in tilting"
        );
        assert!(
            (aerial_haze_density(60.0) - 1.0).abs() < 1e-6,
            "full when gazing to the horizon"
        );
    }

    // --- Mirror of the shader's exponential-height-fog optical depth ----------
    // (see `apply_aerial`). Scale-invariant in the height unit, so we work in
    // metres: eye altitude `cz_m`, fragment elevation `pz_m`, ray length `l_m`.
    const HAZE_SCALE_HEIGHT_M: f32 = 1200.0;
    const HAZE_SIGMA: f32 = 0.13;
    const HAZE_MAX: f32 = 0.72;
    /// Coloration amount for a fragment at elevation `pz_m`, a straight-line
    /// `l_m` from an eye at altitude `cz_m`, gazing at `pitch_deg`. Integrates
    /// exp(-h/H) along the eye→fragment ray (analytic) → tau → 1-exp(-tau).
    fn amount(l_m: f32, cz_m: f32, pz_m: f32, pitch_deg: f64) -> f32 {
        let k = 1.0 / HAZE_SCALE_HEIGHT_M;
        let pz = pz_m.max(0.0);
        let ec = (-cz_m * k).exp();
        let ep = (-pz * k).exp();
        let dz = pz - cz_m;
        let tau = if dz.abs() > 1.0 {
            HAZE_SIGMA * l_m * (ec - ep) / dz
        } else {
            HAZE_SIGMA * l_m * k * ec
        };
        aerial_haze_density(pitch_deg) * (1.0 - (-tau.max(0.0)).exp()) * HAZE_MAX
    }

    #[test]
    fn foreground_is_crisp_and_far_field_colours_when_tilted() {
        // Gazing to the horizon from a 2 km eye over sea-level ground: the near
        // foreground (short ray) stays crisp; the distant low horizon colours.
        let cz = 2_000.0;
        assert!(
            amount(2_500.0, cz, 300.0, 70.0) < 0.15,
            "near foreground stays crisp"
        );
        assert!(
            amount(60_000.0, cz, 0.0, 70.0) > 0.35,
            "far low horizon colours"
        );
    }

    #[test]
    fn valleys_haze_more_than_peaks_at_equal_distance() {
        // The volumetric core: at the SAME range, a valley floor sits deep in the
        // haze band and colours strongly; a peak climbs out of it (its ray spends
        // less length in dense air) and reads clearer. This is what makes it an
        // environmental volume, not a flat depth veil.
        let cz = 2_000.0;
        let l = 40_000.0;
        let valley = amount(l, cz, 0.0, 70.0);
        let peak = amount(l, cz, 1_600.0, 70.0);
        assert!(valley > 0.4, "distant valley floor colours strongly");
        assert!(
            peak < valley * 0.85,
            "a peak at the same range reads clearer than the valley"
        );
    }

    #[test]
    fn from_high_above_the_band_the_scene_is_mostly_clear() {
        // Zoomed way out (eye far above the haze band): looking down/across, most
        // of the scene is clear because the ray runs through thin high air — only
        // grazing rays to the distant low horizon accumulate. No nadir whiteout.
        let cz = 40_000.0; // well above H
        assert!(
            amount(30_000.0, cz, 1_500.0, 70.0) < 0.15,
            "mid-field terrain from high up stays clear"
        );
    }
}
