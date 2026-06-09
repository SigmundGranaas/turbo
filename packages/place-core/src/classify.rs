//! Feature-type + distance → (tier, qualifier). Pure port of the Flutter
//! `categorizeFeature`, driven entirely by the ordered ruleset bands.

use crate::model::{Qualifier, Tier};
use crate::ruleset::Ruleset;

/// Classifies a feature `kind` at `meters` from the query point. Returns the
/// tier + qualifier, or `None` when the candidate is out of range for its type
/// and should be ignored entirely.
pub fn classify(ruleset: &Ruleset, kind: &str, meters: f64) -> Option<(Tier, Qualifier)> {
    let kind = kind.to_lowercase();
    for rule in &ruleset.rules {
        if meters <= rule.max_m && ruleset.kind_in_group(&rule.group, &kind) {
            return Some((rule.tier, rule.qualifier));
        }
    }
    None
}
