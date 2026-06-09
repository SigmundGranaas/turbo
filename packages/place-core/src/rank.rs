//! The reverse-geocode cascade. Pure port of the Flutter
//! `KartverketReverseGeocoder._describeUnbounded` + `StedsnavnBackend._pickBest`,
//! plus the Android parcel-code rejection — unified into one canonical path.

use crate::classify::classify;
use crate::model::{LocationDescription, Qualifier, ReverseInput, Tier};
use crate::ruleset::Ruleset;

/// Resolves a [`ReverseInput`] to a [`LocationDescription`], or `None` when no
/// source produced a usable label (e.g. outside Norway).
///
/// Priority (faithful to the live orchestrator):
///   1. a *tight* toponym (tier `is_tight`) wins outright;
///   2. else a containing protected area;
///   3. else a looser (periphery) toponym;
///   4. else the nearest address (unless it's a bare parcel code);
///   5. else the kommune fallback;
///   6. else `None`.
///
/// Elevation and kommune are enrichments merged onto whichever source wins.
pub fn reverse_geocode(ruleset: &Ruleset, input: &ReverseInput) -> Option<LocationDescription> {
    let best = pick_best(ruleset, input);

    let mut winner = best
        .as_ref()
        .filter(|b| b.tier.is_tight())
        .map(|b| b.desc.clone())
        .or_else(|| {
            input.protected_area.as_ref().map(|pa| LocationDescription {
                qualifier: Some(Qualifier::InArea),
                secondary: pa.kind.clone(),
                ..LocationDescription::titled(&pa.name)
            })
        })
        .or_else(|| best.as_ref().map(|b| b.desc.clone()))
        .or_else(|| {
            input
                .address
                .as_ref()
                .filter(|a| !is_parcel_code(&a.text))
                .map(|a| LocationDescription {
                    qualifier: Some(Qualifier::Near),
                    secondary: a.secondary.clone(),
                    ..LocationDescription::titled(&a.text)
                })
        })
        .or_else(|| {
            // Kommune-as-winner: stamp its own `kommune` field so the enrich
            // step below doesn't re-append it. No qualifier — "In <giant rural
            // kommune>" reads wrong; genuine containment keeps its qualifier.
            input.kommune.as_ref().map(|k| LocationDescription {
                secondary: k.fylke.clone(),
                kommune: Some(k.name.clone()),
                ..LocationDescription::titled(&k.name)
            })
        })?;

    enrich(ruleset, &mut winner, input);
    Some(winner)
}

/// Merge the parallel enrichments (elevation, containing kommune/fylke) onto the
/// winning description.
fn enrich(ruleset: &Ruleset, d: &mut LocationDescription, input: &ReverseInput) {
    if let Some(e) = input.elevation_m {
        if e.is_finite() && e >= ruleset.elevation_min && e <= ruleset.elevation_max {
            d.elevation_m = Some(e);
        }
    }
    if d.kommune.is_none() {
        if let Some(k) = &input.kommune {
            d.kommune = Some(k.name.clone());
            d.fylke = k.fylke.clone();
        }
    }
}

struct Scored {
    desc: LocationDescription,
    tier: Tier,
    score: f64,
}

/// Dedupe toponyms by lowercased title (keeping the lowest score per title),
/// then return the global lowest-scored hit. Ties resolve to the
/// earliest-seen candidate, matching the insertion-ordered Dart map.
fn pick_best(ruleset: &Ruleset, input: &ReverseInput) -> Option<Scored> {
    // Preserve first-insertion order for deterministic tie-breaking.
    let mut order: Vec<String> = Vec::new();
    let mut by_title: std::collections::HashMap<String, Scored> = std::collections::HashMap::new();

    for c in &input.toponyms {
        if !name_ok(ruleset, &c.name) {
            continue;
        }
        let Some((tier, qualifier)) = classify(ruleset, &c.kind, c.distance_m) else {
            continue;
        };
        let active = c.status.as_deref().map(str::to_lowercase).as_deref()
            == Some(ruleset.active_status.as_str());
        let penalty = if active { 0.0 } else { ruleset.status_penalty };
        let score = tier.class_index() as f64 * ruleset.tier_multiplier + c.distance_m + penalty;

        let desc = LocationDescription {
            qualifier: Some(qualifier),
            secondary: c.secondary.clone(),
            distance_m: Some(c.distance_m),
            ..LocationDescription::titled(&c.name)
        };
        let scored = Scored { desc, tier, score };
        let key = c.name.to_lowercase();
        match by_title.get(&key) {
            Some(prior) if prior.score <= scored.score => {}
            Some(_) => {
                by_title.insert(key, scored);
            }
            None => {
                order.push(key.clone());
                by_title.insert(key, scored);
            }
        }
    }

    let mut best: Option<Scored> = None;
    for key in order {
        let candidate = by_title.remove(&key).expect("key was inserted");
        match &best {
            Some(b) if b.score <= candidate.score => {}
            _ => best = Some(candidate),
        }
    }
    best
}

fn name_ok(ruleset: &Ruleset, name: &str) -> bool {
    let trimmed = name.trim();
    if trimmed.is_empty() {
        return false;
    }
    !ruleset.name_rejections.contains(&trimmed.to_lowercase())
}

/// Bare cadastral references like "155/1/73" (gnr/bnr/…): slash-separated,
/// two-or-more all-numeric segments. These must never be a title — the kommune
/// fallback reads better.
fn is_parcel_code(text: &str) -> bool {
    let segments: Vec<&str> = text.trim().split('/').collect();
    segments.len() >= 2
        && segments
            .iter()
            .all(|s| !s.is_empty() && s.chars().all(|c| c.is_ascii_digit()))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn detects_parcel_codes() {
        assert!(is_parcel_code("155/1/73"));
        assert!(is_parcel_code("12/3"));
        assert!(!is_parcel_code("Storgården 4"));
        assert!(!is_parcel_code("Lom"));
    }
}
