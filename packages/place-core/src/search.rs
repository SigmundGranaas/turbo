//! Forward search: ordering already-matched candidates + icon assignment.
//!
//! The live `StedsnavnSearchBackend` does no real ranking — it maps an icon per
//! `navneobjekttype` and the composite service concatenates sources in a fixed
//! order. This module keeps the (shared, data-driven) icon mapping and adds the
//! proximity-biased ordering the plan calls for: among the backend's fuzzy
//! matches, an exact/prefix hit outranks a loose one, and within a match class a
//! *prominence prior* (a `by` beats an `annenKulturdetalj`) plus proximity
//! decides — so "Bergen" the city wins over an obscure toponym of the same name
//! even with no map centre, and the nearest "Storvatnet" still wins when a
//! centre is given. It also composes the human-readable subtitle (label, kommune,
//! trimmed fylke) and collapses exact duplicates, in one place shared with the
//! offline engine.

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

/// Trims a trilingual county name to its first form:
/// "Troms - Romsa - Tromssa" → "Troms". Leaves single-form names untouched.
fn trim_fylke(fylke: &str) -> String {
    fylke
        .split(" - ")
        .next()
        .unwrap_or(fylke)
        .trim()
        .to_string()
}

/// The subtitle for a hit. When the candidate carries kommune/fylke, the core
/// composes "label, kommune, fylke" (human label from the ruleset, fylke
/// trimmed) so this formatting lives in one place shared online + offline.
/// Otherwise the caller's pre-composed `description` (if any) is used verbatim.
fn compose_description(ruleset: &Ruleset, c: &SearchCandidate) -> Option<String> {
    if c.kommune.is_none() && c.fylke.is_none() {
        return c.description.clone();
    }
    let label = ruleset.label_for(&c.kind);
    let parts = [
        Some(label),
        c.kommune.clone(),
        c.fylke.as_deref().map(trim_fylke),
    ];
    let joined = parts
        .into_iter()
        .flatten()
        .filter(|s| !s.is_empty())
        .collect::<Vec<_>>()
        .join(", ");
    if joined.is_empty() {
        None
    } else {
        Some(joined)
    }
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

    let mut scored: Vec<(f64, usize, Option<String>, SearchHit)> = Vec::new();
    for (i, c) in candidates.iter().enumerate() {
        if !ruleset.name_allowed(&c.name) {
            continue;
        }
        let class = match_class(&q, &c.name.to_lowercase());
        let distance = c.distance_m.unwrap_or(0.0);
        // Prominence only reorders *real* matches (exact/prefix/substring). For
        // the loose fuzzy tail (class 3 — the query doesn't occur in the name at
        // all) the backend's trigram-similarity order is the better signal, so
        // leave it untouched. Kept strictly below `match_multiplier` so the prior
        // never lets a candidate jump match class — it only reorders within one.
        let prominence = if class < 3 {
            ruleset.prominence_bonus_m(&c.kind)
        } else {
            0.0
        };
        let score = class as f64 * ruleset.match_multiplier + distance - prominence;
        // Dedup key: same title + kommune + kind is a true duplicate (e.g. a
        // place that is also an area row, or a re-listed toponym). Distinct
        // "Storvatnet" in different kommuner keep different keys and survive.
        let dedup_key = format!(
            "{}\u{1}{}\u{1}{}",
            c.name.to_lowercase(),
            c.kommune.as_deref().unwrap_or("").to_lowercase(),
            c.kind.to_lowercase()
        );
        scored.push((
            score,
            i,
            Some(dedup_key),
            SearchHit {
                index: i as u64,
                title: c.name.clone(),
                description: compose_description(ruleset, c),
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

    // Collapse exact duplicates, keeping the best-ranked (first after sort).
    let mut seen = std::collections::HashSet::new();
    scored
        .into_iter()
        .filter(|(_, _, key, _)| key.as_ref().is_none_or(|k| seen.insert(k.clone())))
        .map(|(_, _, _, hit)| hit)
        .collect()
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

    /// Test-only builder for a search candidate.
    fn cand(name: &str, kind: &str, dist: Option<f64>) -> SearchCandidate {
        SearchCandidate {
            name: name.into(),
            kind: kind.into(),
            distance_m: dist,
            kommune: None,
            fylke: None,
            description: None,
        }
    }

    fn titles(hits: &[SearchHit]) -> Vec<&str> {
        hits.iter().map(|h| h.title.as_str()).collect()
    }

    #[test]
    fn prominent_kind_wins_without_a_map_centre() {
        // Two exact "Bergen" hits, no proximity: the city must beat the obscure
        // cultural detail purely on the prominence prior. (The real regression:
        // an `annenKulturdetalj` was ranked above the city of Bergen.)
        let rs = Ruleset::load_default();
        let hits = forward_search(
            &rs,
            "bergen",
            &[
                cand("Bergen", "annenKulturdetalj", None),
                cand("Bergen", "By", None),
            ],
        );
        assert_eq!(titles(&hits), vec!["Bergen", "Bergen"]);
        assert_eq!(hits[0].icon, "city", "the city Bergen must rank first");
    }

    #[test]
    fn prominent_prefix_hit_surfaces_over_obscure_ones() {
        // "Trom" — Tromsø (a city) must beat obscure equally-prefix toponyms.
        let rs = Ruleset::load_default();
        let hits = forward_search(
            &rs,
            "trom",
            &[
                cand("Tromma", "haug", None),
                cand("Tromsa", "øyISjø", None),
                cand("Tromsø", "By", None),
            ],
        );
        assert_eq!(hits[0].title, "Tromsø");
    }

    #[test]
    fn a_clearly_nearer_feature_still_beats_a_prominent_far_one() {
        // Proximity is not steam-rollered: same exact-match class, a farm 300 m
        // away beats a town 300 km away.
        let rs = Ruleset::load_default();
        let hits = forward_search(
            &rs,
            "os",
            &[
                cand("Os", "Tettsted", Some(300_000.0)),
                cand("Os", "Gard", Some(300.0)),
            ],
        );
        assert_eq!(hits[0].icon, "home", "the nearby farm wins on proximity");
    }

    #[test]
    fn composes_human_label_and_trims_trilingual_fylke() {
        let rs = Ruleset::load_default();
        let mut c = cand("Tromsøya", "øyISjø", None);
        c.kommune = Some("Tromsø".into());
        c.fylke = Some("Troms - Romsa - Tromssa".into());
        let hits = forward_search(&rs, "tromsøya", &[c]);
        assert_eq!(hits[0].description.as_deref(), Some("Øy, Tromsø, Troms"));
    }

    #[test]
    fn falls_back_to_raw_kind_when_unlabelled() {
        let rs = Ruleset::load_default();
        let mut c = cand("Rare", "someWeirdType", None);
        c.kommune = Some("Oslo".into());
        c.fylke = Some("Oslo".into());
        let hits = forward_search(&rs, "rare", &[c]);
        assert_eq!(
            hits[0].description.as_deref(),
            Some("someWeirdType, Oslo, Oslo")
        );
    }

    #[test]
    fn collapses_exact_duplicates_but_keeps_distinct_kommuner() {
        let rs = Ruleset::load_default();
        let mk = |kommune: &str| {
            let mut c = cand("Storgata", "adressenavn", None);
            c.kommune = Some(kommune.into());
            c.fylke = Some("Innlandet".into());
            c
        };
        // Two identical Storgata/Gjøvik + one Storgata/Larvik → 2 survivors.
        let hits = forward_search(&rs, "storgata", &[mk("Gjøvik"), mk("Gjøvik"), mk("Larvik")]);
        assert_eq!(hits.len(), 2);
    }

    #[test]
    fn fuzzy_tail_keeps_backend_order_not_prominence() {
        // class-3 (query absent from the name): the trigram-similarity order the
        // backend supplied must be preserved, prominence must NOT reshuffle it.
        let rs = Ruleset::load_default();
        let hits = forward_search(
            &rs,
            "zzz",
            &[cand("Lillevik", "Gard", None), cand("Storby", "By", None)],
        );
        assert_eq!(titles(&hits), vec!["Lillevik", "Storby"]);
    }
}
