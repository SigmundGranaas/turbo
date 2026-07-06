//! The Environment — one value, sampled by everyone (plan slice E1,
//! architecture §III.4 / Decima D4).
//!
//! Everything "environmental" about a frame — the clock, the one sun, the
//! sun-derived sky palette, the host's look gates, and (ahead of their
//! first consumers) wind and season — is built ONCE per frame from the
//! atmosphere subsystem's state and handed to [`RenderFrame::build`], which
//! derives every environmental uniform from it. No pass patches lighting
//! state in after the fact: sky, aerial haze, terrain lighting and the
//! frame clock all read this value, so there is exactly one write site per
//! environmental input (the E1 grep-gate).
//!
//! [`RenderFrame::build`]: crate::render::frame::RenderFrame::build

use crate::sun::{Atmosphere, SunPosition};

/// The frame's environment. Constructed by the atmosphere subsystem
/// (`Map::render` → `AtmosphereSubsystem::environment`) — the single
/// derivation site for every field.
pub struct Environment {
    /// The frame clock in seconds (renderer wall clock, or the pinned
    /// [`crate::Map::set_time_override`] value). Drives every environmental
    /// animation: haze drift, custom-layer time, and — with E2 — simulation
    /// ticks.
    pub time_s: f32,
    /// The one sun for the whole scene, resolved by the lighting mode at
    /// the camera's location. Terrain shading, cast shadows, the sky and
    /// aerial perspective all share it — that is what keeps them coherent.
    pub sun: SunPosition,
    /// The sun-derived sky/light palette (see [`crate::sun::atmosphere`]).
    pub atmosphere: Atmosphere,
    /// Far-distance atmospheric coloration on/off (host "distance haze"
    /// toggle). The pitch-gated density derives from this in the frame
    /// builder.
    pub aerial_haze: bool,
    /// Terrain sun-lighting on/off (host "sun mode"): `false` keeps the 3D
    /// displaced geometry but draws the bare bright basemap.
    pub terrain_lit: bool,
    /// Basemap brightness gain under the sun-lit path (1.0 = unchanged).
    pub basemap_gain: f32,
    /// Wind vector (m/s, world axes x=E, y=S). Declared ahead of its first
    /// consumer — E2 drives cloud drift from it; until a scene/host writes
    /// it, calm.
    pub wind: [f32; 2],
    /// Season in `[0, 1]` (0 = mid-winter, 0.5 = mid-summer). Declared
    /// ahead of its first consumer (M-MODELS styling); neutral until then.
    pub season: f32,
}
