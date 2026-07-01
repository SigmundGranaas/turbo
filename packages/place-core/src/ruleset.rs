//! The versioned ruleset: the *tunable* half of the spec (kind groups, distance
//! bands, qualifiers, penalties). Loaded from `ruleset.v1.json`; passed into the
//! algorithm so the numbers can change without recompiling the native library.

use std::collections::BTreeMap;

use serde::Deserialize;

use crate::model::{Qualifier, Tier};

/// One classification band, evaluated in order — the first whose group matches
/// the candidate's kind and whose `max_m` covers the distance wins. Mirrors the
/// sequential `if` ladder in the Flutter `categorizeFeature`.
#[derive(Debug, Clone, Deserialize)]
pub struct ClassifyRule {
    /// A kind group name (key in [`Ruleset::kind_groups`]) or the literal
    /// `"any"` for "any non-empty kind".
    pub group: String,
    pub max_m: f64,
    pub tier: Tier,
    pub qualifier: Qualifier,
}

#[derive(Debug, Clone, Deserialize)]
pub struct Ruleset {
    pub version: String,
    /// Feature-type sets (lowercased `navneobjekttype` values) keyed by group.
    pub kind_groups: BTreeMap<String, Vec<String>>,
    /// Ordered classification bands.
    pub rules: Vec<ClassifyRule>,
    /// Score weight per tier class (`class * tier_multiplier + distance + penalty`).
    pub tier_multiplier: f64,
    /// Added to the score of any toponym whose status != `active_status`.
    pub status_penalty: f64,
    pub active_status: String,
    /// Lowercased names that must never surface as a title ("unknown", "ukjent").
    pub name_rejections: Vec<String>,
    /// Inclusive elevation sanity bounds; values outside are dropped.
    pub elevation_min: f64,
    pub elevation_max: f64,
    /// Forward-search: score weight per match class (`class * match_multiplier
    /// + distance`), so an exact/prefix hit dominates a loose fuzzy one.
    pub match_multiplier: f64,
    /// Forward-search icon for a lowercased `navneobjekttype`.
    pub icon_map: BTreeMap<String, String>,
    /// Icon for kinds absent from [`Ruleset::icon_map`].
    pub icon_default: String,
    /// Forward-search prominence prior per lowercased `navneobjekttype` (higher =
    /// more important — a `by` beats an `annenKulturdetalj` of the same match
    /// class). Multiplied by [`Ruleset::prominence_weight`] into a metres-of-
    /// head-start the ranker subtracts from a candidate's distance, so it decides
    /// order when distances are absent (no map centre) or comparable, while a
    /// clearly nearer feature still wins. Defaulted (empty) so pre-prominence
    /// bundles keep their old behaviour.
    #[serde(default)]
    pub kind_prominence: BTreeMap<String, f64>,
    /// Prominence for kinds absent from [`Ruleset::kind_prominence`].
    #[serde(default)]
    pub prominence_default: f64,
    /// Metres-of-head-start per unit of prominence (0 disables the prior).
    #[serde(default)]
    pub prominence_weight: f64,
    /// Human-readable subtitle label per lowercased `navneobjekttype`
    /// (`annenKulturdetalj` → "Kulturminne"). Absent kinds fall back to
    /// [`Ruleset::label_default`] if set, else the raw code.
    #[serde(default)]
    pub label_map: BTreeMap<String, String>,
    /// Fallback subtitle label for kinds absent from [`Ruleset::label_map`]
    /// (empty → use the raw kind code).
    #[serde(default)]
    pub label_default: String,
}

impl Ruleset {
    /// The ruleset embedded at build time (`ruleset.v1.json`). This is what the
    /// clients ship by default; the server also serves it at
    /// `GET /api/places/ruleset/{version}` so a downloaded bundle and the live
    /// app agree on behaviour.
    pub fn load_default() -> Self {
        let raw = include_str!("../ruleset.v1.json");
        serde_json::from_str(raw).expect("embedded ruleset.v1.json is valid")
    }

    pub fn from_json(raw: &str) -> serde_json::Result<Self> {
        serde_json::from_str(raw)
    }

    /// Whether `name` may surface as a title: non-empty and not in
    /// `name_rejections` (case-insensitive). Shared by reverse + forward.
    pub fn name_allowed(&self, name: &str) -> bool {
        let trimmed = name.trim();
        !trimmed.is_empty() && !self.name_rejections.contains(&trimmed.to_lowercase())
    }

    /// Metres-of-head-start the ranker subtracts from a candidate's distance,
    /// from its `kind`'s prominence × [`Ruleset::prominence_weight`]. `kind` is
    /// matched case-insensitively; absent kinds use [`Ruleset::prominence_default`].
    pub fn prominence_bonus_m(&self, kind: &str) -> f64 {
        let k = kind.to_lowercase();
        let p = self
            .kind_prominence
            .get(&k)
            .copied()
            .unwrap_or(self.prominence_default);
        p * self.prominence_weight
    }

    /// The human-readable subtitle label for a `kind` (case-insensitive), from
    /// [`Ruleset::label_map`]; falls back to [`Ruleset::label_default`] when set,
    /// else the raw `kind` unchanged (never loses information).
    pub fn label_for(&self, kind: &str) -> String {
        let k = kind.to_lowercase();
        if let Some(label) = self.label_map.get(&k) {
            return label.clone();
        }
        if !self.label_default.is_empty() {
            return self.label_default.clone();
        }
        kind.to_string()
    }

    /// Whether `kind` (already lowercased) belongs to `group`, with `"any"`
    /// matching any non-empty kind.
    pub(crate) fn kind_in_group(&self, group: &str, kind: &str) -> bool {
        if group == "any" {
            return !kind.is_empty();
        }
        self.kind_groups
            .get(group)
            .is_some_and(|kinds| kinds.iter().any(|k| k == kind))
    }
}
