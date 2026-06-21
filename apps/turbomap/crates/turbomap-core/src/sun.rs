//! Sun position — the single light source that drives terrain shading,
//! the analytic sky, aerial perspective, and the cloud raymarch.
//!
//! Two ways to get a [`SunPosition`]:
//!   * [`solar_position`] — physically-real azimuth/altitude from a UTC
//!     timestamp + observer lat/lng, so the scene's light (and therefore
//!     its colours) track the actual time of day.
//!   * a fixed [`SunPosition`] — for deterministic goldens and for a
//!     pleasant default before the host supplies a clock.
//!
//! The world frame matches the rest of the engine (see `camera.rs`):
//! `x = east`, `y = south` (north is `-y`), `z = up`. Azimuth is measured
//! clockwise from north, altitude above the horizon — identical to the
//! hillshade pipeline's `sun_direction`, so all lit surfaces agree.

/// Where the sun sits, as seen from the observer.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct SunPosition {
    /// Compass bearing of the sun, degrees clockwise from north.
    pub azimuth_deg: f32,
    /// Elevation above the horizon, degrees. Negative = below horizon
    /// (night / civil twilight).
    pub altitude_deg: f32,
}

impl SunPosition {
    /// A warm mid-morning sun from the south-east — the deterministic
    /// default used for goldens and before a host sets a real time. Low
    /// enough to throw readable relief, high enough to light most slopes.
    pub const DEFAULT: SunPosition = SunPosition {
        azimuth_deg: 135.0,
        altitude_deg: 35.0,
    };

    /// Unit direction *towards* the sun in the engine world frame
    /// (`x` east, `y` south, `z` up). Matches the hillshade pipeline's
    /// `sun_direction` exactly so terrain, sky and clouds share one light.
    pub fn world_dir(self) -> [f32; 3] {
        let az = self.azimuth_deg.to_radians();
        let al = self.altitude_deg.to_radians();
        let cos_al = al.cos();
        [cos_al * az.sin(), -cos_al * az.cos(), al.sin()]
    }
}

/// Physically-based solar position from a UTC instant and observer
/// location, after the NOAA solar-position algorithm (the same one the
/// NOAA Solar Calculator uses). Accurate to ~0.1° for dates near the
/// present — far better than the eye can judge from sky colour.
///
/// * `unix_seconds` — seconds since the Unix epoch, UTC.
/// * `lat_deg`, `lng_deg` — observer latitude (+N) and longitude (+E).
pub fn solar_position(unix_seconds: f64, lat_deg: f64, lng_deg: f64) -> SunPosition {
    // Julian day / Julian century (J2000.0 epoch).
    let jd = unix_seconds / 86_400.0 + 2_440_587.5;
    let t = (jd - 2_451_545.0) / 36_525.0;

    // Geometric mean longitude + anomaly of the sun (degrees).
    let l0 = wrap_deg(280.466_46 + t * (36_000.769_83 + t * 0.000_303_2));
    let m = 357.529_11 + t * (35_999.050_29 - t * 0.000_153_7);
    let m_rad = m.to_radians();

    // Sun's equation of centre → true longitude → apparent longitude.
    let c = (1.914_602 - t * (0.004_817 + t * 0.000_014)) * m_rad.sin()
        + (0.019_993 - t * 0.000_101) * (2.0 * m_rad).sin()
        + 0.000_289 * (3.0 * m_rad).sin();
    let true_long = l0 + c;
    let omega = 125.04 - 1934.136 * t;
    let app_long = (true_long - 0.005_69 - 0.004_78 * omega.to_radians().sin()).to_radians();

    // Mean obliquity of the ecliptic (corrected) → declination + RA.
    let obliq = (23.439_291 - t * (0.013_004_2 + t * (0.000_000_163 - t * 0.000_000_503_6))
        + 0.002_56 * omega.to_radians().cos())
    .to_radians();
    let decl = (obliq.sin() * app_long.sin()).asin();

    // Equation of time (minutes), for apparent solar time.
    let y = (obliq / 2.0).tan().powi(2);
    let l0_rad = l0.to_radians();
    let eot = 4.0
        * (y * (2.0 * l0_rad).sin() - 2.0 * 0.016_708_634 * m_rad.sin()
            + 4.0 * 0.016_708_634 * y * m_rad.sin() * (2.0 * l0_rad).cos()
            - 0.5 * y * y * (4.0 * l0_rad).sin()
            - 1.25 * 0.016_708_634 * 0.016_708_634 * (2.0 * m_rad).sin())
        .to_degrees();

    // True solar time (minutes) → hour angle (degrees from solar noon).
    let secs_of_day = unix_seconds.rem_euclid(86_400.0);
    let minutes_utc = secs_of_day / 60.0;
    let true_solar_min = (minutes_utc + eot + 4.0 * lng_deg).rem_euclid(1440.0);
    let hour_angle = (true_solar_min / 4.0 - 180.0).to_radians();

    let lat = lat_deg.to_radians();
    // Solar zenith → altitude.
    let cos_zenith =
        (lat.sin() * decl.sin() + lat.cos() * decl.cos() * hour_angle.cos()).clamp(-1.0, 1.0);
    let zenith = cos_zenith.acos();
    let altitude = std::f64::consts::FRAC_PI_2 - zenith;

    // Azimuth, clockwise from north.
    let denom = lat.cos() * zenith.sin();
    let azimuth = if denom.abs() < 1e-9 {
        if altitude > 0.0 {
            180.0
        } else {
            0.0
        }
    } else {
        let cos_az = ((lat.sin() * zenith.cos() - decl.sin()) / denom).clamp(-1.0, 1.0);
        // NOAA azimuth, clockwise from NORTH. `acos` resolves the [0,180] core;
        // the half-day branch both disambiguates morning/afternoon AND shifts
        // that (from-south) core onto a from-north bearing. The previous
        // `acos` / `360 − acos` form skipped the shift, so the sun came out
        // ~180° off — the northern sky at N-hemisphere midday — and every lit
        // surface + cast shadow pointed the wrong way (in Norway, shadows fell
        // toward the south instead of away from the southern sun).
        let ac = cos_az.acos().to_degrees();
        if hour_angle > 0.0 {
            (ac + 180.0).rem_euclid(360.0)
        } else {
            (540.0 - ac).rem_euclid(360.0)
        }
    };

    SunPosition {
        azimuth_deg: azimuth as f32,
        altitude_deg: altitude.to_degrees() as f32,
    }
}

fn wrap_deg(d: f64) -> f64 {
    d.rem_euclid(360.0)
}

/// Time-of-day colours derived from the sun's altitude. One source of
/// truth for the warm/cool palette so the terrain shading, aerial-
/// perspective haze and the sky dome all agree. Colours are linear-ish
/// RGB multipliers/targets (the framebuffer applies the sRGB encode).
#[derive(Debug, Clone, Copy)]
pub struct Atmosphere {
    /// Direct sunlight colour — neutral-bright at noon, deep amber at
    /// sunset, dim blue at night. Multiplies the lit basemap.
    pub light_color: [f32; 3],
    /// Ambient sky-fill floor in [0,1] for shadowed slopes.
    pub ambient: f32,
    /// Colour distant relief fades toward (aerial perspective) — the
    /// horizon hue.
    pub haze_color: [f32; 3],
    /// Sky colour straight up.
    pub zenith_color: [f32; 3],
    /// Sky colour at the horizon (where the haze matches).
    pub horizon_color: [f32; 3],
}

/// Smoothstep from `a`→`b` over `x`, clamped.
fn smoothstep(a: f32, b: f32, x: f32) -> f32 {
    let t = ((x - a) / (b - a)).clamp(0.0, 1.0);
    t * t * (3.0 - 2.0 * t)
}

fn mix3(a: [f32; 3], b: [f32; 3], t: f32) -> [f32; 3] {
    [
        a[0] + (b[0] - a[0]) * t,
        a[1] + (b[1] - a[1]) * t,
        a[2] + (b[2] - a[2]) * t,
    ]
}

/// Derive the time-of-day palette from a sun position. Three regimes,
/// cross-faded by altitude so transitions are smooth:
///   * **day** (alt ≳ 10°): bright neutral sun, blue sky.
///   * **golden hour** (alt ≈ 0–10°): warm amber sun, orange horizon.
///   * **night** (alt < 0°): no direct sun, dim blue ambient.
pub fn atmosphere(sun: SunPosition) -> Atmosphere {
    let alt = sun.altitude_deg;

    // Day palette.
    let day_light = [1.0, 0.97, 0.92];
    let day_zenith = [0.16, 0.34, 0.66];
    let day_horizon = [0.66, 0.78, 0.92];

    // Golden-hour palette.
    let gold_light = [1.0, 0.62, 0.36];
    let gold_zenith = [0.22, 0.30, 0.52];
    let gold_horizon = [0.96, 0.62, 0.40];

    // Night palette.
    let night_light = [0.16, 0.20, 0.34];
    let night_zenith = [0.02, 0.03, 0.07];
    let night_horizon = [0.06, 0.08, 0.16];

    // 0 at/below horizon → 1 by ~10° (day vs golden blend).
    let day_t = smoothstep(0.0, 10.0, alt);
    // 1 above horizon → 0 by ~ -6° (civil twilight into night).
    let lit_t = smoothstep(-6.0, 1.0, alt);

    let light_day_gold = mix3(gold_light, day_light, day_t);
    let zenith_day_gold = mix3(gold_zenith, day_zenith, day_t);
    let horizon_day_gold = mix3(gold_horizon, day_horizon, day_t);

    let light_color = mix3(night_light, light_day_gold, lit_t);
    let zenith_color = mix3(night_zenith, zenith_day_gold, lit_t);
    let horizon_color = mix3(night_horizon, horizon_day_gold, lit_t);

    // Brighter ambient by day, dim at night.
    let ambient = 0.16 + 0.26 * lit_t;

    Atmosphere {
        light_color,
        ambient,
        haze_color: horizon_color,
        zenith_color,
        horizon_color,
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn approx(a: f32, b: f32, eps: f32) -> bool {
        (a - b).abs() < eps
    }

    #[test]
    fn default_dir_is_unit_and_above_horizon() {
        let d = SunPosition::DEFAULT.world_dir();
        let len = (d[0] * d[0] + d[1] * d[1] + d[2] * d[2]).sqrt();
        assert!(approx(len, 1.0, 1e-5), "not unit: {len}");
        assert!(d[2] > 0.0, "sun should be above horizon: {d:?}");
    }

    #[test]
    fn overhead_noon_points_up() {
        // The sun is near the zenith over the equator at the March
        // equinox local solar noon. 2024-03-20 12:00 UTC at lng 0.
        let unix = 1_710_936_000.0; // 2024-03-20T12:00:00Z
        let p = solar_position(unix, 0.0, 0.0);
        assert!(
            p.altitude_deg > 75.0,
            "equator equinox noon should be high, got {p:?}"
        );
    }

    #[test]
    fn night_is_below_horizon() {
        // Midnight UTC at Greenwich → sun well below the horizon.
        let unix = 1_710_892_800.0; // 2024-03-20T00:00:00Z
        let p = solar_position(unix, 51.5, 0.0);
        assert!(p.altitude_deg < 0.0, "midnight should be night, got {p:?}");
    }

    #[test]
    fn bergen_summer_afternoon_sun_in_the_west() {
        // Bergen, Norway, a June afternoon → sun in the western half.
        let unix = 1_718_899_200.0; // 2024-06-20T16:00:00Z (~18:00 local)
        let p = solar_position(unix, 60.39, 5.32);
        assert!(p.altitude_deg > 0.0, "afternoon sun is up: {p:?}");
        assert!(
            p.azimuth_deg > 180.0 && p.azimuth_deg < 330.0,
            "afternoon sun should be west-ish: {p:?}"
        );
    }
}

#[cfg(test)]
mod sun_azimuth_check {
    use super::*;
    // Regression: the N-hemisphere daytime sun must sit in the SOUTHERN sky, so
    // shading + cast shadows point the right way. A prior azimuth-branch bug put
    // it ~180° off (northern sky at midday in Norway).
    #[test]
    fn bodo_daytime_sun_is_in_the_south() {
        // Near solar noon at Bodø (14.4°E, 67.28°N), 2024-06-21.
        let noon = solar_position(1_718_967_600.0, 67.28, 14.4);
        assert!(noon.altitude_deg > 0.0, "daytime: sun above horizon");
        assert!(
            (150.0..=210.0).contains(&noon.azimuth_deg),
            "midday sun must be ~due south, got az={}",
            noon.azimuth_deg
        );
        // world_dir points TOWARD the sun → its south (y) component is positive.
        assert!(noon.world_dir()[1] > 0.5, "midday sun direction points south");

        // Morning (08:00 UTC ≈ 09:00 local) → south-EAST: az in the SE quadrant.
        let morning = solar_position(1_718_956_800.0, 67.28, 14.4);
        assert!(
            (90.0..180.0).contains(&morning.azimuth_deg),
            "morning sun is in the SE, got az={}",
            morning.azimuth_deg
        );
        // Afternoon (15:00 UTC) → south-WEST.
        let afternoon = solar_position(1_718_982_000.0, 67.28, 14.4);
        assert!(
            (180.0..270.0).contains(&afternoon.azimuth_deg),
            "afternoon sun is in the SW, got az={}",
            afternoon.azimuth_deg
        );
    }
}
