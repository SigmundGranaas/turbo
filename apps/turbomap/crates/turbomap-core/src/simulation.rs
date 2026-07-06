//! Simulation systems (plan slice E2, architecture §III.4 / Decima S5).
//!
//! A simulation system advances world state from the frame's
//! [`Environment`] — the same clock, sun and wind every render pass
//! samples, so simulated content stays coherent with the lighting by
//! construction. Systems are DETERMINISTIC by design: state derives from
//! `(fields, env.time_s, seed)`, not from accumulated per-frame deltas, so
//! pinning the clock ([`crate::Map::set_time_override`]) replays the exact
//! frame — the E2 replay gate.
//!
//! An active system counts as animation ([`crate::Map::is_animating`]),
//! which is what keeps render-on-demand hosts pumping frames — no host-side
//! `request_redraw` warts.

use crate::environment::Environment;

/// A per-frame world-state advance. `dt_s` is the frame delta (0 under a
/// pinned clock); implementations should prefer deriving state from
/// `env.time_s` directly — integrating `dt_s` accumulates float error and
/// breaks replay.
pub trait SimulationSystem {
    /// Advance by `dt_s` seconds under `env`. Returns `true` while the
    /// system is actively animating (keeps render-on-demand hosts awake).
    fn tick(&mut self, dt_s: f32, env: &Environment) -> bool;
}
