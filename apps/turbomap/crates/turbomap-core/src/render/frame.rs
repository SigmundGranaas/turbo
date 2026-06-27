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
        let altitude_world = camera.altitude_world(viewport_px).max(1e-9);
        let haze_density = aerial_haze_density(altitude_world, camera.pitch_deg);

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
            haze_color: atmos.haze_color,
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

/// Per-world-unit aerial-perspective density for `1 - exp(-dist_world · d)`,
/// where `dist_world` is the true eye→fragment distance. Altitude-relative
/// (`k / altitude_world`) so both terms share units and the look is zoom-stable;
/// a pitch ramp gates it to 0 on the flat 2D map. The nearest visible ground is
/// ~one altitude from the eye, so its haze is ≈ `1 - exp(-k)` — with `k = 0.08`
/// the near/mid field stays nearly clear (~8%) while the far field (many
/// altitudes out) still dissolves into the atmosphere (`1 - exp(-k·40) ≈ 0.96`).
/// `k = 0.22` tinted the WHOLE scene blue; this keeps the haze to the distance.
fn aerial_haze_density(_altitude_world: f32, _pitch_deg: f64) -> f32 {
    // Aerial-perspective fog REMOVED per device feedback — it wasn't really visible and only
    // muddied clarity. Returns 0 so terrain carries no distance haze. The eye-distance machinery
    // in the shader stays behind this single knob, so the fog can be restored by reinstating the
    // `(0.08 / altitude_world) * pitch_ramp` formula here.
    0.0
}

#[cfg(test)]
mod tests {
    //! Aerial-perspective invariants, independent of any basemap (the headless
    //! scenario basemap is ~white, so a rendered-frame luma check can't judge
    //! haze — see the `turbomap-harness-pale-basemap` note). These exercise the
    //! real density fn + the public camera projection, asserting the whiteout
    //! fix: near ground stays clear at every pitch, the far field dissolves.

    use super::aerial_haze_density;
    use crate::camera::Camera;
    use crate::geo::LatLng;

    const VP: (u32, u32) = (1080, 1600);

    /// `1 - exp(-dist·density)` haze fraction for an eye→point distance.
    fn haze(dist_world: f32, density: f32) -> f32 {
        1.0 - (-dist_world * density).exp()
    }

    #[test]
    fn flat_2d_map_carries_no_haze() {
        let cam = Camera::new(LatLng::new(67.23, 15.30), 14.0).with_pitch(0.0);
        let alt = cam.altitude_world(VP);
        assert_eq!(aerial_haze_density(alt, 0.0), 0.0, "pitch 0 must be haze-free");
    }

    #[test]
    fn nearest_ground_stays_clear_at_every_pitch() {
        // The bottom-centre pixel is the nearest visible ground. Its eye distance,
        // through the density at that pitch, must keep haze low — this is the
        // anti-whiteout invariant the center-distance model violated.
        let center = LatLng::new(67.23, 15.30);
        for pitch in [10.0_f64, 30.0, 50.0, 70.0, 80.0] {
            let cam = Camera::new(center, 14.0).with_pitch(pitch);
            let alt = cam.altitude_world(VP);
            let density = aerial_haze_density(alt, pitch);
            let eye = cam.eye_offset_world(VP);
            // Nearest ground = bottom-centre pixel unprojected onto z=0, taken
            // RELATIVE to centre (the RTC frame `eye` lives in — pixel_to_world
            // returns absolute Mercator coords, so subtract the centre).
            let c = center.to_world();
            let g = cam.pixel_to_world((VP.0 as f64 / 2.0, VP.1 as f64), (VP.0 as f64, VP.1 as f64));
            let (dx, dy, dz) = ((g.x - c.x) as f32 - eye[0], (g.y - c.y) as f32 - eye[1], -eye[2]);
            let dist = (dx * dx + dy * dy + dz * dz).sqrt();
            let h = haze(dist, density);
            assert!(h < 0.45, "near ground must stay readable at {pitch}° (haze {h:.2})");
        }
    }

    #[test]
    fn fog_is_disabled_at_every_pitch() {
        // Aerial-perspective fog was removed (not visible on device, hurt clarity): density is
        // 0 regardless of altitude/pitch, so no distance haze is applied anywhere — even deep
        // into the far field at a strong tilt.
        let center = LatLng::new(67.23, 15.30);
        for pitch in [0.0_f64, 30.0, 75.0] {
            let cam = Camera::new(center, 14.0).with_pitch(pitch);
            let alt = cam.altitude_world(VP);
            assert_eq!(aerial_haze_density(alt, pitch), 0.0, "fog must stay off at {pitch}°");
            assert_eq!(haze(40.0 * alt, aerial_haze_density(alt, pitch)), 0.0, "no far-field haze");
        }
    }
}
