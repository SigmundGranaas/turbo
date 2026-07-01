//! Domain types shared across every runtime. Field names mirror the clients'
//! `LocationDescription` so the FFI bindings map 1:1.

use serde::{Deserialize, Serialize};

/// Tier-classification of a toponym relative to the queried coordinate. Drives
/// whether the orchestrator accepts the hit outright or defers to the
/// protected-area / address / kommune fallbacks. Mirrors the Flutter
/// `LocationMatchTier`.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum Tier {
    /// Class 0 — exact contact: on a peak, at a waterline, at a building.
    #[serde(rename = "exactContact")]
    ExactContact,
    /// Class 1 — in a settlement (Tettsted / By / Bygd / …) within ~1.5 km.
    #[serde(rename = "inSettlement")]
    InSettlement,
    /// Class 2 — close to a real peak (≤ 800 m).
    #[serde(rename = "closeToPeak")]
    CloseToPeak,
    /// Class 4 — wider periphery (near settlement, close to farm/water/…).
    #[serde(rename = "periphery")]
    Periphery,
}

impl Tier {
    /// Tier class used in the composite score (`class * tier_multiplier`).
    pub fn class_index(self) -> u32 {
        match self {
            Tier::ExactContact => 0,
            Tier::InSettlement => 1,
            Tier::CloseToPeak => 2,
            Tier::Periphery => 3,
        }
    }

    /// `true` when the orchestrator should accept this hit immediately without
    /// consulting the protected-area / address / kommune fallbacks.
    pub fn is_tight(self) -> bool {
        matches!(
            self,
            Tier::ExactContact | Tier::InSettlement | Tier::CloseToPeak
        )
    }
}

/// Canonical spatial qualifier (the richer Flutter `LocationQualifier` set).
/// The Android binding folds `CloseTo`→Near and `InArea`→In to match its
/// 4-value `PlaceQualifier`; the UI turns these into localized words.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[cfg_attr(feature = "uniffi", derive(uniffi::Enum))]
pub enum Qualifier {
    /// Standing on the feature (peak / glacier / island).
    #[serde(rename = "on")]
    On,
    /// Near but outside a point feature (loose peak / farm / water).
    #[serde(rename = "closeTo")]
    CloseTo,
    /// Touching a water body or building.
    #[serde(rename = "atPlace")]
    AtPlace,
    /// Inside a bounded area (settlement / park).
    #[serde(rename = "inArea")]
    InArea,
    /// In the wider periphery of a feature.
    #[serde(rename = "near")]
    Near,
}

/// A named toponym candidate (one Stedsnavn `/punkt` row, one PostGIS KNN row,
/// or one SQLite R*Tree row). The caller resolves `distance_m` — from the
/// server's `meterFraPunkt` when present, else via [`crate::haversine_m`].
#[derive(Debug, Clone, Deserialize)]
#[cfg_attr(feature = "uniffi", derive(uniffi::Record))]
pub struct Candidate {
    pub name: String,
    /// Feature type (`navneobjekttype`), matched case-insensitively against the
    /// ruleset kind groups.
    #[serde(default)]
    pub kind: String,
    pub distance_m: f64,
    /// Lifecycle status (`stedstatus`); anything other than the ruleset's
    /// `active_status` incurs `status_penalty`.
    #[serde(default)]
    pub status: Option<String>,
    /// Pre-resolved subtitle, if the source carries one. Reverse-geocode
    /// (`/punkt`) leaves this `None`; the orchestrator enriches with
    /// kommune/fylke instead.
    #[serde(default)]
    pub secondary: Option<String>,
}

/// A containing protected area (national park / nature reserve / …).
#[derive(Debug, Clone, Deserialize)]
#[cfg_attr(feature = "uniffi", derive(uniffi::Record))]
pub struct ProtectedArea {
    pub name: String,
    /// Protection class (`verneform`), used as the subtitle.
    #[serde(default)]
    pub kind: Option<String>,
}

/// Nearest civic address. `text` is the street + number; `secondary` is the
/// post code + post town (e.g. "2686 LOM").
#[derive(Debug, Clone, Deserialize)]
#[cfg_attr(feature = "uniffi", derive(uniffi::Record))]
pub struct Address {
    pub text: String,
    #[serde(default)]
    pub secondary: Option<String>,
}

/// The municipality containing the point (final fallback + subtitle context).
#[derive(Debug, Clone, Deserialize)]
#[cfg_attr(feature = "uniffi", derive(uniffi::Record))]
pub struct Kommune {
    pub name: String,
    #[serde(default)]
    pub fylke: Option<String>,
}

/// Everything a platform has gathered for one reverse-geocode. All sources are
/// optional; `reverse_geocode` runs the cascade over whatever is present.
#[derive(Debug, Clone, Default, Deserialize)]
#[cfg_attr(feature = "uniffi", derive(uniffi::Record))]
pub struct ReverseInput {
    #[serde(default)]
    pub toponyms: Vec<Candidate>,
    #[serde(default)]
    pub protected_area: Option<ProtectedArea>,
    #[serde(default)]
    pub address: Option<Address>,
    #[serde(default)]
    pub kommune: Option<Kommune>,
    /// Elevation at the queried point; merged onto any winner when finite and
    /// within the ruleset's `[elevation_min, elevation_max]`.
    #[serde(default)]
    pub elevation_m: Option<f64>,
}

/// The lenient reverse-geocode result. Mirrors the clients' `LocationDescription`
/// field-for-field.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[cfg_attr(feature = "uniffi", derive(uniffi::Record))]
pub struct LocationDescription {
    pub title: String,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub qualifier: Option<Qualifier>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub secondary: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub kommune: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub fylke: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub distance_m: Option<f64>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub elevation_m: Option<f64>,
}

/// One forward-search candidate (a Stedsnavn `/navn` row, a PostGIS trigram
/// match, or a local marker/path). The platform resolves `distance_m` from the
/// map centre / user location when it wants proximity bias.
#[derive(Debug, Clone, Deserialize)]
#[cfg_attr(feature = "uniffi", derive(uniffi::Record))]
pub struct SearchCandidate {
    pub name: String,
    /// Feature type (`navneobjekttype`), used for the icon.
    #[serde(default)]
    pub kind: String,
    /// Distance from the search origin (map centre), if known. `None` disables
    /// proximity bias for this candidate (ties fall back to input order).
    #[serde(default)]
    pub distance_m: Option<f64>,
    /// Municipality (`kommunenavn`). When set (with `fylke`), the core composes
    /// the subtitle from `label_for(kind)` + kommune + trimmed fylke, so labels
    /// and formatting live in one place shared with the offline engine.
    #[serde(default)]
    pub kommune: Option<String>,
    /// County (`fylkesnavn`); trilingual names are trimmed to the first form
    /// ("Troms - Romsa - Tromssa" → "Troms") when composing the subtitle.
    #[serde(default)]
    pub fylke: Option<String>,
    /// Pre-composed subtitle fallback for callers that carry no kommune/fylke
    /// (e.g. local markers). Used verbatim only when `kommune` and `fylke` are
    /// both absent.
    #[serde(default)]
    pub description: Option<String>,
}

/// One ranked forward-search result. `index` points back into the input
/// candidates so the caller can recover position / source / metadata.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[cfg_attr(feature = "uniffi", derive(uniffi::Record))]
pub struct SearchHit {
    pub index: u64,
    pub title: String,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub description: Option<String>,
    pub icon: String,
}

impl LocationDescription {
    pub(crate) fn titled(title: impl Into<String>) -> Self {
        LocationDescription {
            title: title.into(),
            qualifier: None,
            secondary: None,
            kommune: None,
            fylke: None,
            distance_m: None,
            elevation_m: None,
        }
    }
}
