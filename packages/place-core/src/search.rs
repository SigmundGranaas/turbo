//! Forward search: ordering already-matched candidates + icon assignment.
//!
//! The live `StedsnavnSearchBackend` does no real ranking — it maps an icon per
//! `navneobjekttype` and the composite service concatenates sources in a fixed
//! order. This module keeps the (shared, data-driven) icon mapping and adds the
//! proximity-biased ordering the plan calls for: among the backend's fuzzy
//! matches, an exact/prefix hit outranks a loose one, and within a match class
//! the nearer feature wins. There are many "Storvatnet" in Norway — the one
//! near the map centre should come first.

use crate::model::{SearchCandidate, SearchHit};
use crate::ruleset::Ruleset;

/// Match quality of `query` against a candidate name (lower is better):
/// 0 exact, 1 prefix, 2 substring, 3 neither (the backend still matched it
/// fuzzily, so it's kept but ranked last). Comparison is case-insensitive;
/// diacritic-insensitivity is the server-side FTS/trigram's job.
fn match_class(query_lc: &str, name_lc: &str) -> u32 {
    if name_lc == query_lc {
        0
    } else if name_lc.starts_with(query_lc) {
        1
    } else if name_lc.contains(query_lc) {
        2
    } else {
        3
    }
}

/// The icon for a feature `kind`, from the ruleset map (falling back to the
/// default). Port of the `StedsnavnSearchBackend` switch.
pub fn icon_for(ruleset: &Ruleset, kind: &str) -> String {
    let k = kind.to_lowercase();
    ruleset
        .icon_map
        .get(&k)
        .cloned()
        .unwrap_or_else(|| ruleset.icon_default.clone())
}

/// Orders `candidates` for the query into ranked [`SearchHit`]s. Nameless
/// candidates are dropped; ordering is by `(match_class, distance)` with a
/// stable fall-back to input order, so callers without proximity (`distance_m =
/// None`) keep the source ordering they passed in.
pub fn forward_search(
    ruleset: &Ruleset,
    query: &str,
    candidates: &[SearchCandidate],
) -> Vec<SearchHit> {
    let q = query.trim().to_lowercase();

    let mut scored: Vec<(f64, usize, SearchHit)> = Vec::new();
    for (i, c) in candidates.iter().enumerate() {
        if !ruleset.name_allowed(&c.name) {
            continue;
        }
        let class = match_class(&q, &c.name.to_lowercase()) as f64;
        let distance = c.distance_m.unwrap_or(0.0);
        let score = class * ruleset.match_multiplier + distance;
        scored.push((
            score,
            i,
            SearchHit {
                index: i,
                title: c.name.clone(),
                description: c.description.clone(),
                icon: icon_for(ruleset, &c.kind),
            },
        ));
    }

    // Stable by score, then by original index for deterministic ties.
    scored.sort_by(|a, b| {
        a.0.partial_cmp(&b.0)
            .unwrap_or(std::cmp::Ordering::Equal)
            .then(a.1.cmp(&b.1))
    });
    scored.into_iter().map(|(_, _, hit)| hit).collect()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn match_classes() {
        assert_eq!(match_class("stor", "stor"), 0);
        assert_eq!(match_class("stor", "storvatnet"), 1);
        assert_eq!(match_class("vatn", "storvatnet"), 2);
        assert_eq!(match_class("xyz", "storvatnet"), 3);
    }

    #[test]
    fn icon_fallback() {
        let rs = Ruleset::load_default();
        assert_eq!(icon_for(&rs, "Fjell"), "mountain");
        assert_eq!(icon_for(&rs, "Foss"), "place");
    }
}
