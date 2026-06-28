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

/// 0..1 gate for the aerial-perspective effect, fed to the shader as
/// `haze_density`. The real strength + distance/altitude falloff is the
/// optical-depth integral in the shader (`apply_aerial`); this only fades the
/// effect out on the near-flat 2D map (no 3D depth to colour). The physics
/// alone already keeps near/down sightlines clear (short path through thin air),
/// so this is a soft 2D→3D fade, not a strength knob.
fn aerial_haze_density(pitch_deg: f64) -> f32 {
    let t = (((pitch_deg - 2.0) / 12.0) as f32).clamp(0.0, 1.0);
    t * t * (3.0 - 2.0 * t) // smoothstep: 0 at ≤2°, 1 by ~14°
}

#[cfg(test)]
mod tests {
    //! Aerial-perspective invariants. The look is the OPTICAL-DEPTH integral in
    //! the shader; here we (a) test the CPU 2D→3D gate and (b) reproduce the
    //! shader's optical-depth math to assert the PHYSICS: haze stacks along a
    //! long low-altitude sightline, a short/down look stays clear, and a higher
    //! (thinner-air) eye hazes less than a low one for the same far target.
    //! Colour can't be judged headlessly (see `turbomap-harness-pale-basemap`).

    use super::aerial_haze_density;

    #[test]
    fn gate_off_in_2d_and_on_in_3d() {
        assert_eq!(aerial_haze_density(0.0), 0.0, "flat 2D map carries no haze");
        assert_eq!(aerial_haze_density(2.0), 0.0, "≤2° still flat");
        assert!(aerial_haze_density(8.0) > 0.0 && aerial_haze_density(8.0) < 1.0, "fades in");
        assert!((aerial_haze_density(20.0) - 1.0).abs() < 1e-6, "full once tilted into 3D");
    }

    // --- Mirror of the shader's `apply_aerial` optical depth (metres) ---------
    const SCALE_H_M: f32 = 8000.0;
    const BETA_PER_M: f32 = 8.0e-6;
    /// Haze fraction for an eye at altitude `z_eye_m` looking at a fragment at
    /// altitude `z_frag_m`, `l_m` metres away along the sightline.
    fn haze(z_eye_m: f32, z_frag_m: f32, l_m: f32) -> f32 {
        let dz = z_frag_m - z_eye_m;
        let column = if dz.abs() < 1e-6 {
            l_m * (-z_eye_m / SCALE_H_M).exp()
        } else {
            l_m * SCALE_H_M * ((-z_eye_m / SCALE_H_M).exp() - (-z_frag_m / SCALE_H_M).exp()) / dz
        };
        1.0 - (-(BETA_PER_M * column.max(0.0))).exp()
    }

    #[test]
    fn haze_stacks_along_a_low_horizontal_sightline() {
        // Hiker-altitude eye looking across at the same height: haze grows with
        // distance (the "stacks when looking over at the same low height" case).
        let near = haze(300.0, 300.0, 5_000.0);
        let mid = haze(300.0, 300.0, 30_000.0);
        let far = haze(300.0, 300.0, 80_000.0);
        assert!(near < 0.10, "5 km is fairly clear (got {near:.3})");
        assert!(mid > near && far > mid, "haze accumulates with distance");
        assert!(far > 0.30, "80 km across low air reads clearly hazy (got {far:.3})");
    }

    #[test]
    fn short_downward_look_stays_clear() {
        // Eye 1.5 km up looking ~straight down at near ground: short path → clear.
        assert!(haze(1_500.0, 0.0, 1_600.0) < 0.05, "looking down is near-crisp");
    }

    #[test]
    fn higher_thinner_air_eye_hazes_less_for_same_far_target() {
        // Same far ground point; a high-altitude eye sees it through more thin
        // air (and the dense layer is a smaller fraction of the path) → less haze
        // than a low-altitude eye. This is why zoomed-out top-down stops washing.
        let low_eye = haze(500.0, 0.0, 60_000.0);
        let high_eye = haze(40_000.0, 0.0, 70_000.0);
        assert!(high_eye < low_eye, "thin-air eye hazes less ({high_eye:.3} < {low_eye:.3})");
    }
}
