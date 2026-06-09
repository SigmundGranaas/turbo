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
