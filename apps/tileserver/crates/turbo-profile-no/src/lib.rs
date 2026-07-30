//! **L5 — the Norwegian hiking profile.** Calibrated constants and
//! curated presets, as data the composition root hands to the engine.
//!
//! See `docs/architecture/2026-07-routing-engine-module-design.md` §7.2.
//!
//! # Why this is not in the engine
//!
//! It was. `turbo-tiles-pathfind` baked both TOML files in with
//! `include_str!` and `Pathfinder::new` silently fell back to them, so
//! the engine:
//!
//!   - shipped one country's calibration as its default behaviour,
//!   - *chose* that default rather than being told, and
//!   - read the filesystem (`TURBO_COST_CONFIG`, then the CWD) looking
//!     for a newer copy.
//!
//! The last one is the sharpest: an engine that resolves paths is an
//! engine you cannot embed. The first is the most insidious — a caller
//! who forgot to pass a config got Norwegian hiking constants and no
//! diagnostic, which is indistinguishable from working.
//!
//! Loading now happens here, at the composition layer, and the engine
//! takes a value.
//!
//! # The numbers are calibrated, not chosen
//!
//! These constants came out of corpus calibration against walked
//! Norwegian trails. They are not defaults in the "sensible starting
//! point" sense and should not be edited casually — the routing gate
//! (`tools/routing_gate.sh`) will notice, which is the intended
//! outcome. Another region, sport, or game ships a sibling crate;
//! nothing in the engine changes.

#![forbid(unsafe_code)]

use turbo_tiles_pathfind::{ConfigError, CostConfig, PresetSet};

/// The calibrated cost constants, baked into the binary.
///
/// `include_str!` rather than a runtime read so a fresh deploy with no
/// config directory still boots with the calibrated behaviour — the
/// property the engine's old embedded copy was protecting, kept, but
/// moved to a layer that is allowed to have it.
pub const COST_CONFIG_TOML: &str = include_str!("../../../tools/cost-config.toml");

/// The curated trip presets ("balanced", "avoid_roads", …).
pub const PRESETS_TOML: &str = include_str!("../../../tools/route-presets.toml");

/// The calibrated Norwegian cost config.
pub fn cost_config() -> Result<CostConfig, ConfigError> {
    CostConfig::from_toml(COST_CONFIG_TOML)
}

/// The curated presets. Never fails: a malformed preset file yields an
/// empty set, so an unknown preset name becomes a 400 naming the valid
/// ones rather than a boot failure that takes routing down entirely.
pub fn presets() -> PresetSet {
    PresetSet::from_toml(PRESETS_TOML).unwrap_or_default()
}

/// Load the cost config, preferring an operator-supplied file.
///
/// Resolution order — `$TURBO_COST_CONFIG`, then `tools/cost-config.toml`
/// relative to the CWD, then the baked-in copy. This *is* source
/// resolution, which is exactly why it lives here and not in the
/// engine: the engine may not know that files exist.
pub fn cost_config_or_default() -> CostConfig {
    let from_disk = std::env::var("TURBO_COST_CONFIG")
        .ok()
        .map(std::path::PathBuf::from)
        .or_else(|| {
            let p = std::path::PathBuf::from("tools/cost-config.toml");
            p.exists().then_some(p)
        });
    if let Some(path) = from_disk {
        match std::fs::read_to_string(&path)
            .map_err(|e| e.to_string())
            .and_then(|t| CostConfig::from_toml(&t).map_err(|e| e.to_string()))
        {
            Ok(c) => return c,
            Err(e) => {
                // Loud, then fall back. A typo in an operator's config
                // silently reverting to calibrated defaults is how you
                // spend an afternoon wondering why your knob does
                // nothing.
                eprintln!(
                    "turbo-profile-no: {}: {e}; using calibrated defaults",
                    path.display()
                );
            }
        }
    }
    cost_config().expect("the baked-in Norwegian cost config must parse")
}

/// Presets, preferring `tools/route-presets.toml` in the CWD.
pub fn presets_or_default() -> PresetSet {
    std::fs::read_to_string("tools/route-presets.toml")
        .ok()
        .and_then(|t| PresetSet::from_toml(&t).ok())
        .unwrap_or_else(presets)
}

#[cfg(test)]
mod tests {
    use super::*;

    /// The baked-in files must parse. They are `include_str!`d, so a
    /// broken edit is a compile-time-shaped problem that only shows up
    /// at runtime — this turns it back into a test failure.
    #[test]
    fn the_calibrated_config_parses() {
        let c = cost_config().expect("cost-config.toml must parse");
        assert!(
            c.base.pace_s_per_m > 0.0,
            "a pace of zero is not calibration"
        );
    }

    /// The calibrated values themselves, pinned here rather than in the
    /// engine (where they used to live, asserted through
    /// `CostConfig::from_embedded`). Testing a country's calibration
    /// through the engine is exactly the coupling D3 removed.
    ///
    /// These are not arbitrary: each came out of corpus calibration
    /// against walked Norwegian trails. A change to any of them will
    /// also move the routing gate, which is the intended relationship —
    /// this test says "you changed the profile", the gate says "and
    /// here is what it did to the routes".
    #[test]
    fn the_calibrated_constants_are_what_calibration_produced() {
        let c = cost_config().unwrap();
        assert!((c.base.pace_s_per_m - 0.714_285_7).abs() < 1e-4);
        assert!((c.off_trail_base.foot - 2.3).abs() < 1e-6);
        assert_eq!(c.trail_proximity.influence_radius_m, 30.0);
        assert!((c.trail_proximity.bonus_at_zero - 0.15).abs() < 1e-6);
        assert_eq!(c.slope_cell.refuse_above_deg, 45.0);
        assert_eq!(c.slope_graph.refuse_above_deg, 50.0);
        assert!(c.surface_multiplier.foot.by_kind.contains_key("sti"));
    }

    #[test]
    fn the_presets_parse_and_include_the_app_default() {
        let p = presets();
        assert!(
            p.get("balanced").is_some(),
            "the app sends `balanced` when no preset is given; it must exist"
        );
    }
}
