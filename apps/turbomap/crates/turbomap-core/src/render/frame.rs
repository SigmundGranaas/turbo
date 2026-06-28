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
            let inv_view_proj =
                glam::Mat4::from_cols_array_2d(&vp).inverse().to_cols_array_2d();
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
            meters_to_world: if terrain.present { meters_to_world } else { 0.0 },
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
        let vec_terrain_zscale = raster_terrain_cfg.meters_to_world * raster_terrain_cfg.exaggeration;
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

/// Max strength of the far-distance atmospheric coloration, gated by PITCH.
///
/// The shader keys the effect on HORIZONTAL distance from the camera centre
/// (onset ~50 km), so ground near/below the camera is never coloured regardless
/// of zoom. This gate adds the "only when gazing toward the horizon" rule: it's
/// 0 when looking down (you're inspecting the ground, not the far distance) and
/// ramps to `HAZE_MAX` as the camera tilts up toward the horizon. The value is
/// the uniform `haze_density` the shader multiplies the distance term by; 0
/// turns the whole effect off (top-down / 2D).
fn aerial_haze_density(pitch_deg: f64) -> f32 {
    // Coloured, not fogged: a low cap so distant ridges take the sky's hue
    // without being erased.
    const HAZE_MAX: f32 = 0.55;
    // Off below 30° (looking down), full by ~62° (gazing to the horizon).
    let t = (((pitch_deg - 30.0) / 32.0) as f32).clamp(0.0, 1.0);
    let gate = t * t * (3.0 - 2.0 * t); // smoothstep
    HAZE_MAX * gate
}

#[cfg(test)]
mod tests {
    //! Atmospheric-coloration invariants. The shader keys the effect on
    //! HORIZONTAL distance from the camera centre (onset ~50 km), so these test
    //! the CPU PITCH gate (`aerial_haze_density`) and reproduce the shader's
    //! distance ramp to assert: looking down → none; near/below the camera →
    //! none at any zoom; only the far (≥~50 km) field, when gazing toward the
    //! horizon, takes a (capped, never-whiteout) amount. Colour can't be judged
    //! headlessly (see `turbomap-harness-pale-basemap`).

    use super::aerial_haze_density;

    // Mirror of the shader's `apply_aerial` distance ramp (metres).
    const HAZE_ONSET_M: f32 = 45_000.0;
    const HAZE_RANGE_M: f32 = 120_000.0;
    fn smoothstep(e0: f32, e1: f32, x: f32) -> f32 {
        let t = ((x - e0) / (e1 - e0)).clamp(0.0, 1.0);
        t * t * (3.0 - 2.0 * t)
    }
    /// Final coloration amount the shader applies at `horiz_m` metres outward,
    /// for a camera at `pitch_deg`.
    fn amount(horiz_m: f32, pitch_deg: f64) -> f32 {
        let strength = aerial_haze_density(pitch_deg);
        smoothstep(HAZE_ONSET_M, HAZE_ONSET_M + HAZE_RANGE_M, horiz_m) * strength
    }

    #[test]
    fn no_coloration_when_looking_down() {
        // Top-down / shallow tilt: the pitch gate is fully closed, so even the
        // far field carries nothing — you're inspecting the ground, not the sky.
        for pitch in [0.0_f64, 15.0, 30.0] {
            assert_eq!(aerial_haze_density(pitch), 0.0, "gate must be shut at {pitch}°");
            assert_eq!(amount(200_000.0, pitch), 0.0, "no coloration at {pitch}° even 200 km out");
        }
    }

    #[test]
    fn near_and_mid_field_never_coloured_even_gazing_to_horizon() {
        // At a strong horizon-gazing tilt, ground near/below the camera and out
        // to tens of km stays fully crisp — the onset is ~50 km.
        for horiz_m in [0.0_f32, 5_000.0, 30_000.0, 45_000.0] {
            assert_eq!(amount(horiz_m, 75.0), 0.0, "{horiz_m} m out must stay crisp (onset 50 km)");
        }
    }

    #[test]
    fn far_field_takes_a_capped_coloration_only_at_high_pitch() {
        // Beyond the onset, gazing to the horizon, the far field is coloured —
        // but capped well below 1 (coloration, not a whiteout) and monotonic.
        let near = amount(60_000.0, 75.0);
        let far = amount(120_000.0, 75.0);
        assert!(far > near && near > 0.0, "coloration grows past 50 km ({near:.3} → {far:.3})");
        assert!(far <= 0.55 + 1e-6, "capped at HAZE_MAX, never a full whiteout (got {far:.3})");
    }
}
