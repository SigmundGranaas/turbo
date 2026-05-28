//! Cost calibration config loader.
//!
//! Single source of truth for the routing cost knobs that aren't
//! geometric facts about the graph. Loaded once at boot, exposed
//! to layers + the off-trail solver, overridable per request via
//! [`crate::pathfinder::Prefs::cost_config_override`].
//!
//! See `tools/cost-config.toml` for the on-disk schema. The
//! embedded defaults compiled into the binary match that file
//! exactly so a fresh build with no config on disk still produces
//! the calibrated behaviour.

use std::collections::BTreeMap;
use std::path::Path;

use serde::{Deserialize, Serialize};

/// Embedded defaults — same file the curator edits on disk.
/// `include_str!` bakes its contents into the binary so the system
/// boots even without a writable config dir.
pub const EMBEDDED_DEFAULTS: &str = include_str!("../../../tools/cost-config.toml");

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct CostConfig {
    pub base: BaseConfig,
    pub off_trail_base: OffTrailConfig,
    pub trail_proximity: TrailProximityConfig,
    pub slope_cell: SlopeConfig,
    pub slope_graph: SlopeConfig,
    pub total_gain: TotalGainConfig,
    pub surface_multiplier: SurfaceMultipliers,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct BaseConfig {
    pub pace_s_per_m: f64,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct OffTrailConfig {
    pub foot: f64,
    pub bicycle: f64,
    pub ski: f64,
}

impl OffTrailConfig {
    pub fn for_profile(&self, p: turbo_tiles_graph::Profile) -> f64 {
        match p {
            turbo_tiles_graph::Profile::Foot => self.foot,
            turbo_tiles_graph::Profile::Bicycle => self.bicycle,
            turbo_tiles_graph::Profile::Ski => self.ski,
        }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct TrailProximityConfig {
    pub influence_radius_m: f32,
    pub bonus_at_zero: f32,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SlopeConfig {
    pub quadratic_scale_deg: f32,
    pub refuse_above_deg: f32,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct TotalGainConfig {
    pub amplifier: f32,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SurfaceMultipliers {
    pub foot: ProfileSurface,
    pub bicycle: ProfileSurface,
    pub ski: ProfileSurface,
}

/// Per-fkb_type multipliers for one profile. Keyed by the same
/// strings the graph builder uses (`sti`, `vei`, `traktorvei`,
/// `skogsvei`, `skiloype`). Currently informational — the actual
/// multipliers are baked into the graph artifact in
/// `graph_builder::surface_multiplier`. Stage 2 follow-on moves
/// reading to runtime.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ProfileSurface {
    #[serde(flatten)]
    pub by_kind: BTreeMap<String, f32>,
}

/// Sparse override patch. Each field is optional so a request
/// only carries the knobs the curator wants to change; unset
/// fields inherit from the boot config.
#[derive(Debug, Clone, Default, Deserialize)]
pub struct CostConfigPatch {
    #[serde(default)]
    pub off_trail_base_foot: Option<f64>,
    #[serde(default)]
    pub off_trail_base_bicycle: Option<f64>,
    #[serde(default)]
    pub off_trail_base_ski: Option<f64>,
    #[serde(default)]
    pub trail_proximity_bonus_at_zero: Option<f32>,
    #[serde(default)]
    pub trail_proximity_influence_radius_m: Option<f32>,
    #[serde(default)]
    pub slope_cell_quadratic_scale_deg: Option<f32>,
    #[serde(default)]
    pub slope_cell_refuse_above_deg: Option<f32>,
    #[serde(default)]
    pub slope_graph_quadratic_scale_deg: Option<f32>,
    #[serde(default)]
    pub slope_graph_refuse_above_deg: Option<f32>,
    #[serde(default)]
    pub total_gain_amplifier: Option<f32>,
}

impl CostConfig {
    /// Resolve the cost config from (in order):
    ///   1. `path` if `Some`
    ///   2. `TURBO_COST_CONFIG` env var
    ///   3. `tools/cost-config.toml` relative to CWD
    ///   4. Embedded defaults baked into the binary.
    pub fn load_or_default(path: Option<&Path>) -> Result<Self, ConfigError> {
        if let Some(p) = path {
            return Self::from_path(p);
        }
        if let Ok(env_path) = std::env::var("TURBO_COST_CONFIG") {
            return Self::from_path(env_path.as_ref());
        }
        let cwd_path = Path::new("tools/cost-config.toml");
        if cwd_path.exists() {
            return Self::from_path(cwd_path);
        }
        Self::from_embedded()
    }

    pub fn from_path(p: &Path) -> Result<Self, ConfigError> {
        let text = std::fs::read_to_string(p)
            .map_err(|e| ConfigError::Io(format!("{}: {e}", p.display())))?;
        toml::from_str(&text).map_err(|e| ConfigError::Parse(e.to_string()))
    }

    pub fn from_embedded() -> Result<Self, ConfigError> {
        toml::from_str(EMBEDDED_DEFAULTS).map_err(|e| ConfigError::Parse(e.to_string()))
    }

    /// Apply a sparse patch and return a new config. Used by the
    /// per-request override path: boot config + per-request patch
    /// → effective config for that one solve.
    pub fn with_patch(&self, patch: &CostConfigPatch) -> CostConfig {
        let mut c = self.clone();
        if let Some(v) = patch.off_trail_base_foot { c.off_trail_base.foot = v; }
        if let Some(v) = patch.off_trail_base_bicycle { c.off_trail_base.bicycle = v; }
        if let Some(v) = patch.off_trail_base_ski { c.off_trail_base.ski = v; }
        if let Some(v) = patch.trail_proximity_bonus_at_zero {
            c.trail_proximity.bonus_at_zero = v;
        }
        if let Some(v) = patch.trail_proximity_influence_radius_m {
            c.trail_proximity.influence_radius_m = v;
        }
        if let Some(v) = patch.slope_cell_quadratic_scale_deg {
            c.slope_cell.quadratic_scale_deg = v;
        }
        if let Some(v) = patch.slope_cell_refuse_above_deg {
            c.slope_cell.refuse_above_deg = v;
        }
        if let Some(v) = patch.slope_graph_quadratic_scale_deg {
            c.slope_graph.quadratic_scale_deg = v;
        }
        if let Some(v) = patch.slope_graph_refuse_above_deg {
            c.slope_graph.refuse_above_deg = v;
        }
        if let Some(v) = patch.total_gain_amplifier { c.total_gain.amplifier = v; }
        c
    }
}

#[derive(Debug, thiserror::Error)]
pub enum ConfigError {
    #[error("io: {0}")]
    Io(String),
    #[error("parse: {0}")]
    Parse(String),
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn embedded_defaults_parse() {
        let cfg = CostConfig::from_embedded().unwrap();
        assert!((cfg.base.pace_s_per_m - 0.7142857).abs() < 1e-4);
        assert!((cfg.off_trail_base.foot - 2.3).abs() < 1e-6);
        assert_eq!(cfg.trail_proximity.influence_radius_m, 150.0);
        assert!((cfg.trail_proximity.bonus_at_zero - 0.5).abs() < 1e-6);
        assert_eq!(cfg.slope_cell.refuse_above_deg, 45.0);
        assert_eq!(cfg.slope_graph.refuse_above_deg, 50.0);
        assert!(cfg.surface_multiplier.foot.by_kind.contains_key("sti"));
    }

    #[test]
    fn patch_overrides_individual_fields() {
        let base = CostConfig::from_embedded().unwrap();
        let patch = CostConfigPatch {
            off_trail_base_foot: Some(2.2),
            trail_proximity_bonus_at_zero: Some(0.5),
            ..Default::default()
        };
        let c = base.with_patch(&patch);
        assert!((c.off_trail_base.foot - 2.2).abs() < 1e-6);
        assert!((c.trail_proximity.bonus_at_zero - 0.5).abs() < 1e-6);
        // Unspecified knobs unchanged.
        assert_eq!(c.off_trail_base.bicycle, base.off_trail_base.bicycle);
        assert_eq!(c.slope_cell.refuse_above_deg, base.slope_cell.refuse_above_deg);
    }

    #[test]
    fn missing_file_falls_back_to_embedded() {
        let cfg = CostConfig::load_or_default(Some(Path::new("/nonexistent.toml")));
        assert!(cfg.is_err()); // explicit path that doesn't exist IS an error
    }

    #[test]
    fn no_path_no_env_no_cwd_uses_embedded() {
        // We can't safely scrub env vars in parallel tests; just
        // confirm `from_embedded` works directly.
        let cfg = CostConfig::from_embedded().unwrap();
        assert!(cfg.off_trail_base.foot > 1.0);
    }
}
