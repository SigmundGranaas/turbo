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
    #[serde(default)]
    pub surface_pace: SurfacePaceConfig,
    /// Water traversal model. `#[serde(default)]` so configs predating
    /// the continuous-water change still parse.
    #[serde(default)]
    pub water: WaterConfig,
    /// State-augmented (x,y,heading) grade-limited solver — switchbacks
    /// up steep ground. Opt-in; off by default until validated.
    #[serde(default)]
    pub grade_limited: GradeLimitedConfig,
}

/// Knobs for the grade-limited (switchbacking) off-trail solver.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct GradeLimitedConfig {
    /// Master switch. When off, off-trail uses the 2D anisotropic FMM.
    pub enabled: bool,
    /// Forward moves steeper than this (deg) are refused, forcing the
    /// path to traverse/switchback instead of climbing the fall line.
    pub max_grade_deg: f32,
    /// Seconds per 45° heading change — tunes switchback spacing.
    pub turn_penalty_s: f32,
}

impl Default for GradeLimitedConfig {
    fn default() -> Self {
        Self { enabled: false, max_grade_deg: 27.0, turn_penalty_s: 8.0 }
    }
}

/// Water is finite-cost-but-passable near shores, hard-refused only in
/// the deep interior. A water cell is "shoreline" (passable, costly) if
/// any sample on a ring of radius `shore_band_m` is non-water; otherwise
/// it's "deep" and refused. This lets the off-trail solver hug a
/// shoreline like the marked trail (the 25 m water raster puffs shores,
/// so trails often sit on water-flagged cells) without letting routes
/// shortcut across a lake or fjord.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct WaterConfig {
    /// Extra walk-seconds per metre when traversing a passable
    /// (shoreline) water cell. High, so the geodesic enters water only
    /// to follow a shore, never to shortcut.
    pub cost_s_per_m: f64,
    /// Ring radius (m) used to classify shoreline vs deep water.
    pub shore_band_m: f64,
}

impl Default for WaterConfig {
    fn default() -> Self {
        Self { cost_s_per_m: 4.0, shore_band_m: 60.0 }
    }
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
    /// True-cliff threshold: only slopes above this are hard-refused
    /// (impassable). Between `refuse_above_deg` and this, terrain is
    /// continuous high cost (Tobler explodes near 50°), so a 45° hard
    /// wall no longer severs off-trail corridors at 10 m DEM resolution
    /// — it just becomes very expensive. `#[serde(default)]` so configs
    /// predating the change still parse.
    #[serde(default = "default_cliff_refuse_deg")]
    pub cliff_refuse_deg: f32,
}

fn default_cliff_refuse_deg() -> f32 {
    // Near-vertical. Only genuine cliffs are impassable; everything
    // below is continuous (very high) Tobler cost so the FMM corridor
    // stays connected and the geodesic curves around steep ground on
    // the gentle line instead of the corridor being severed → Theta*.
    78.0
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

/// Runtime per-surface pace multipliers, applied live in the unified
/// solve (unlike the baked, solver-ignored `surface_multiplier` above).
/// A road-class (`vei`) value near `off_trail_base` makes roads only
/// slightly cheaper than open ground, killing big road detours.
#[derive(Debug, Clone, Default, Serialize, Deserialize)]
pub struct SurfacePaceConfig {
    #[serde(default)]
    pub foot: SurfacePaceProfile,
    #[serde(default)]
    pub bicycle: SurfacePaceProfile,
    #[serde(default)]
    pub ski: SurfacePaceProfile,
}

/// Per-surface pace multiplier for one profile (1.0 = no effect).
/// Keyed by `fkb_type` category, not raw string, because
/// `traktorvei`/`skogsvei` fold into `vei` in the graph artifact.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SurfacePaceProfile {
    #[serde(default = "one_f64")]
    pub sti: f64,
    #[serde(default = "one_f64")]
    pub vei: f64,
    #[serde(default = "one_f64")]
    pub skiloype: f64,
    #[serde(default = "one_f64")]
    pub unknown: f64,
}

fn one_f64() -> f64 {
    1.0
}

impl Default for SurfacePaceProfile {
    fn default() -> Self {
        Self { sti: 1.0, vei: 1.0, skiloype: 1.0, unknown: 1.0 }
    }
}

/// Sparse override patch. Each field is optional so a request
/// only carries the knobs the curator wants to change; unset
/// fields inherit from the boot config.
#[derive(Debug, Clone, Default, Deserialize)]
pub struct CostConfigPatch {
    /// Per-surface pace overrides apply to the FOOT profile (presets
    /// are foot-focused; bicycle/ski keep their config defaults).
    #[serde(default)]
    pub surface_pace_sti: Option<f64>,
    #[serde(default)]
    pub surface_pace_vei: Option<f64>,
    #[serde(default)]
    pub surface_pace_skiloype: Option<f64>,
    #[serde(default)]
    pub surface_pace_unknown: Option<f64>,
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
    #[serde(default)]
    pub water_cost_s_per_m: Option<f64>,
    #[serde(default)]
    pub water_shore_band_m: Option<f64>,
    #[serde(default)]
    pub grade_limited_enabled: Option<bool>,
    #[serde(default)]
    pub grade_limited_max_grade_deg: Option<f32>,
    #[serde(default)]
    pub grade_limited_turn_penalty_s: Option<f32>,
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
        // Surface-pace overrides apply to the foot profile (preset focus).
        if let Some(v) = patch.surface_pace_sti { c.surface_pace.foot.sti = v; }
        if let Some(v) = patch.surface_pace_vei { c.surface_pace.foot.vei = v; }
        if let Some(v) = patch.surface_pace_skiloype { c.surface_pace.foot.skiloype = v; }
        if let Some(v) = patch.surface_pace_unknown { c.surface_pace.foot.unknown = v; }
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
        if let Some(v) = patch.water_cost_s_per_m { c.water.cost_s_per_m = v; }
        if let Some(v) = patch.water_shore_band_m { c.water.shore_band_m = v; }
        if let Some(v) = patch.grade_limited_enabled { c.grade_limited.enabled = v; }
        if let Some(v) = patch.grade_limited_max_grade_deg { c.grade_limited.max_grade_deg = v; }
        if let Some(v) = patch.grade_limited_turn_penalty_s { c.grade_limited.turn_penalty_s = v; }
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
        assert_eq!(cfg.trail_proximity.influence_radius_m, 30.0);
        assert!((cfg.trail_proximity.bonus_at_zero - 0.15).abs() < 1e-6);
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
