//! Scene lighting — the single sun that drives terrain shading, the sky,
//! aerial perspective, and the cloud raymarch.
//!
//! This replaces the `Map`'s loose `sun_time_unix: Option<f64>` +
//! `fixed_sun: Option<SunPosition>` pair (two fields encoding one decision)
//! with an explicit [`LightingMode`] state machine. There is exactly one
//! source of the sun at any time, so the "which wins?" ambiguity is gone:
//! the mode *is* the answer.

use crate::geo::LatLng;
use crate::sun::{self, Atmosphere, SunPosition};

/// How the scene's sun position is determined.
#[derive(Debug, Clone, Copy, PartialEq)]
pub enum LightingMode {
    /// A fixed, pleasant default — used for deterministic goldens and before
    /// a host supplies a clock or an override.
    Default,
    /// Track a real instant (`unix_seconds`, UTC); the sun is solved per
    /// frame at the camera's location, so the light follows the clock and
    /// where the user is looking.
    TimeTracked(f64),
    /// Pinned azimuth/altitude — an explicit override (manual control).
    Fixed(SunPosition),
}

/// The scene's lighting state. Owns the mode and resolves the sun + the
/// derived time-of-day [`Atmosphere`] palette.
#[derive(Debug, Clone, Copy)]
pub struct Lighting {
    mode: LightingMode,
}

impl Default for Lighting {
    fn default() -> Self {
        Self {
            mode: LightingMode::Default,
        }
    }
}

impl Lighting {
    pub fn mode(&self) -> LightingMode {
        self.mode
    }

    /// Track a real UTC instant, or `None` to revert to the default sun.
    pub fn set_time(&mut self, unix_seconds: Option<f64>) {
        self.mode = match unix_seconds {
            Some(t) => LightingMode::TimeTracked(t),
            None => LightingMode::Default,
        };
    }

    /// Pin the sun to an explicit position, or `None` to revert to default.
    pub fn set_fixed(&mut self, sun: Option<SunPosition>) {
        self.mode = match sun {
            Some(s) => LightingMode::Fixed(s),
            None => LightingMode::Default,
        };
    }

    /// The sun used this frame, resolved against the camera `center` (only
    /// the time-tracked mode needs the location).
    pub fn sun_at(&self, center: LatLng) -> SunPosition {
        match self.mode {
            LightingMode::Default => SunPosition::DEFAULT,
            LightingMode::Fixed(s) => s,
            LightingMode::TimeTracked(t) => sun::solar_position(t, center.lat, center.lng),
        }
    }

    /// The time-of-day colour palette for this frame's sun.
    pub fn atmosphere_at(&self, center: LatLng) -> Atmosphere {
        sun::atmosphere(self.sun_at(center))
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn mode_transitions_and_resolution() {
        let c = LatLng::new(67.25, 15.3);
        let mut l = Lighting::default();
        assert_eq!(l.mode(), LightingMode::Default);
        assert_eq!(l.sun_at(c), SunPosition::DEFAULT);

        l.set_time(Some(1_718_899_200.0)); // a real instant
        assert!(matches!(l.mode(), LightingMode::TimeTracked(_)));
        // Solved sun is finite + on the unit sphere.
        let d = l.sun_at(c).world_dir();
        let len = (d[0] * d[0] + d[1] * d[1] + d[2] * d[2]).sqrt();
        assert!((len - 1.0).abs() < 1e-4);

        let pinned = SunPosition {
            azimuth_deg: 120.0,
            altitude_deg: 40.0,
        };
        l.set_fixed(Some(pinned));
        assert_eq!(l.sun_at(c), pinned);

        l.set_fixed(None);
        assert_eq!(l.mode(), LightingMode::Default);
    }
}
