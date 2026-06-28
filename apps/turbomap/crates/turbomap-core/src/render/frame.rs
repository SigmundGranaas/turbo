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
        let haze_density = aerial_haze_density(meters_to_world, camera.pitch_deg);

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
/// where `dist_world` is the true eye→fragment distance (RTC frame).
///
/// Expressed in REAL metres — `HAZE_PER_M` per metre, converted to world units
/// via `meters_to_world` — so the haze onset is in KILOMETRES regardless of zoom
/// or latitude (what "distant terrain gets hazy, near stays crisp" means). The
/// shader squares the resulting term, so with `HAZE_PER_M = 1e-4` (≈ a 10 km
/// e-folding distance): ground within ~2 km carries ≈3% haze (crisp), ~10 km
/// reads ≈40%, and the far field (≳25 km) saturates at the shader's 0.82 cap and
/// dissolves into the atmosphere colour. A pitch ramp eases it in over the first
/// few degrees of tilt so the flat 2D map carries none (it's a depth cue).
fn aerial_haze_density(meters_to_world: f32, pitch_deg: f64) -> f32 {
    if pitch_deg <= 1.0 || meters_to_world <= 0.0 {
        return 0.0;
    }
    const HAZE_PER_M: f32 = 1.0e-4;
    let ramp = (((pitch_deg - 1.0) / 8.0) as f32).clamp(0.0, 1.0);
    (HAZE_PER_M / meters_to_world) * ramp
}

#[cfg(test)]
mod tests {
    //! Aerial-perspective invariants. The density is now km-scaled (real metres
    //! via `meters_to_world`), so the look is checked in DISTANCE terms: the flat
    //! 2D map carries none; near ground (a km or two) stays crisp at a hiking
    //! zoom; the far field (tens of km) dissolves into the atmosphere. A rendered
    //! luma check can't judge colour headlessly (see `turbomap-harness-pale-basemap`).

    use super::aerial_haze_density;

    const EARTH_CIRC_M: f32 = 40_075_017.0;

    /// Mercator metres→world units at a latitude — mirrors `RenderFrame::build`.
    fn m2w(lat_deg: f64) -> f32 {
        ((lat_deg.to_radians().cos().abs() as f32) / EARTH_CIRC_M).max(1e-12)
    }

    /// The shader's haze fraction for an eye→point distance of `dist_m` metres,
    /// including the `min(h², 0.82)` near-crisp/cap shaping the fragment applies.
    fn shaped_haze(dist_m: f32, lat_deg: f64, pitch_deg: f64) -> f32 {
        let density = aerial_haze_density(m2w(lat_deg), pitch_deg);
        let dist_world = dist_m * m2w(lat_deg);
        let raw = 1.0 - (-dist_world * density).exp();
        (raw * raw).min(0.82)
    }

    #[test]
    fn flat_2d_map_carries_no_haze() {
        assert_eq!(aerial_haze_density(m2w(67.23), 0.0), 0.0, "pitch 0 must be haze-free");
        assert_eq!(aerial_haze_density(m2w(67.23), 1.0), 0.0, "≤1° still flat → no haze");
    }

    #[test]
    fn near_ground_stays_crisp_and_far_field_dissolves() {
        // At any tilt, ground within ~2 km reads essentially crisp while relief
        // tens of km out saturates toward the atmosphere colour — that's the
        // "distant hazy, close crisp" goal, expressed in real distance.
        for pitch in [10.0_f64, 45.0, 80.0] {
            let near = shaped_haze(1_500.0, 67.23, pitch);
            let far = shaped_haze(35_000.0, 67.23, pitch);
            assert!(near < 0.08, "near ground (1.5 km) must stay crisp at {pitch}° (haze {near:.3})");
            assert!(far > 0.5, "far ridges (35 km) must read hazy at {pitch}° (haze {far:.3})");
        }
    }

    #[test]
    fn haze_grows_monotonically_with_distance() {
        let d = |m| shaped_haze(m, 67.23, 60.0);
        assert!(d(2_000.0) < d(8_000.0));
        assert!(d(8_000.0) < d(20_000.0));
        assert!(d(20_000.0) <= 0.82, "far field is capped, never full whiteout");
    }
}
