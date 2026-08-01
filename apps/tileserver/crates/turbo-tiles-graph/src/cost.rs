//! Per-profile edge cost and the attribute encodings it reads.
//!
//! This lives beside the format, not in a builder, for the same reason
//! the mask's scanline fill does: two builders that cost *nearly* the
//! same produce two graphs that route differently over the same ground,
//! and nothing about that failure announces itself. A route is simply
//! different. Sharing the cost function is what lets a PostGIS-built
//! graph and a GML-built one be compared at all.
//!
//! Changing anything here changes every route in every pack, including
//! packs already on phones — the costs are baked into the artifact, not
//! computed at solve time.

use crate::EdgeRecord;

/// `fkb_type` byte. 0 is "unknown", and deliberately not an error: N50
/// and FKB both carry values this pipeline has no opinion about, and
/// they should route as road-like rather than vanish.
pub fn encode_fkb_type(s: Option<&str>) -> u8 {
    match s.unwrap_or("") {
        "sti" => 1,
        "vei" => 2,
        "skiloype" | "skiløype_preparert" | "lysløype" => 3,
        // Legacy / future surface kinds — map to "trail-ish" by
        // default so they aren't accidentally treated as roads.
        "sti_terreng" => 1,
        "traktorvei" | "skogsvei" | "sykkelvei" => 2,
        // Defensive aliases for the legacy "veg" spellings.
        "traktorveg" | "skogsbilveg" | "skogsveg" | "sykkelveg" => 2,
        _ => 0,
    }
}

pub fn encode_marking(s: Option<&str>) -> u8 {
    match s.unwrap_or("") {
        "red_t" => 1,
        "cairn" => 2,
        "blue_paint" => 3,
        "unmarked" => 4,
        _ => 0,
    }
}

pub fn encode_surface(s: Option<&str>) -> u8 {
    match s.unwrap_or("") {
        "natural" => 1,
        "gravel" => 2,
        "asphalt" => 3,
        "boardwalk" => 4,
        _ => 0,
    }
}

/// How much a profile dislikes a given way type.
pub fn surface_multiplier(fkb_type: u8, profile_id: u32) -> f32 {
    match (profile_id, fkb_type) {
        // FOOT — strongly prefer real trails; tolerate roads but
        // tax them so a 6 km trail beats a 60 km road detour.
        (0, 1) => 1.0, // sti (hiking trail — ideal)
        (0, 2) => 1.6, // vei (road — possible, but punishing on foot)
        (0, 3) => 1.2, // skiloype (open ground in summer)
        (0, _) => 1.4, // unknown — assume road-like

        // BICYCLE — prefer roads, penalise trails + ski tracks.
        (1, 1) => 1.5, // sti (rough for bikes)
        (1, 2) => 1.0, // vei (ideal)
        (1, 3) => 2.0, // skiloype (not really bikeable)
        (1, _) => 1.0,

        // SKI — prepared tracks first, then trails, then roads.
        (2, 1) => 1.2, // sti (skiable but slow)
        (2, 2) => 1.4, // vei (plowed, fast but unrewarding)
        (2, 3) => 1.0, // skiloype (ideal!)
        (2, _) => 1.3,

        _ => 1.0,
    }
}

/// Naismith-ish blend of distance and climb, in metres-equivalent.
pub fn profile_cost(e: &EdgeRecord, profile_id: u32) -> f32 {
    let gain = e.gain_m.max(0.0);
    let base = match profile_id {
        // Foot — Naismith: every 600 m of vertical = +1 h compared
        // to flat at 5 km/h, i.e. +8 × gain in equivalent distance.
        0 => e.length_m + 8.0 * gain,
        // Bicycle — slightly faster on the flat, much slower uphill.
        1 => e.length_m * 0.6 + 20.0 * gain,
        // Ski — ungroomed default; surface mult favours groomed.
        2 => e.length_m * 1.2 + 6.0 * gain,
        _ => e.length_m,
    };
    base * surface_multiplier(e.fkb_type, profile_id)
}

#[cfg(test)]
mod tests {
    use super::*;

    fn edge(length_m: f32, gain_m: f32, fkb_type: u8) -> EdgeRecord {
        EdgeRecord {
            from_id: 0,
            to_id: 1,
            length_m,
            gain_m,
            loss_m: 0.0,
            slope_max_deg: 0.0,
            fkb_type,
            marking: 0,
            surface: 0,
            source: 0,
            attr_flags: 0,
        }
    }

    #[test]
    fn flat_trail_on_foot_costs_its_length() {
        assert_eq!(profile_cost(&edge(1000.0, 0.0, 1), 0), 1000.0);
    }

    /// Naismith: 600 m of climb is worth 4 800 m of flat walking.
    #[test]
    fn climb_is_taxed_at_eight_times_on_foot() {
        assert_eq!(profile_cost(&edge(1000.0, 600.0, 1), 0), 1000.0 + 4800.0);
    }

    /// The property the multipliers exist for: on foot a trail should
    /// win against a road that is only modestly shorter.
    #[test]
    fn a_trail_beats_a_slightly_shorter_road_on_foot() {
        let trail = profile_cost(&edge(1000.0, 0.0, 1), 0);
        let road = profile_cost(&edge(800.0, 0.0, 2), 0);
        assert!(trail < road, "trail {trail} should beat road {road}");
    }

    /// …and the reverse for a bicycle, which is what makes the profiles
    /// distinct rather than three names for one cost.
    #[test]
    fn a_road_beats_a_trail_on_a_bicycle() {
        let trail = profile_cost(&edge(1000.0, 0.0, 1), 1);
        let road = profile_cost(&edge(1000.0, 0.0, 2), 1);
        assert!(road < trail, "road {road} should beat trail {trail}");
    }

    /// Descent must not pay: gain is clamped, so a downhill edge costs
    /// its length, never less.
    #[test]
    fn descent_does_not_earn_a_discount() {
        let mut e = edge(1000.0, -50.0, 1);
        e.loss_m = 50.0;
        assert_eq!(profile_cost(&e, 0), 1000.0);
    }

    #[test]
    fn unknown_way_types_route_as_road_like_rather_than_free() {
        assert!(surface_multiplier(0, 0) > 1.0);
        assert_eq!(encode_fkb_type(None), 0);
        assert_eq!(encode_fkb_type(Some("noe helt annet")), 0);
    }

    #[test]
    fn the_legacy_veg_spellings_still_encode_as_roads() {
        for s in ["traktorveg", "skogsbilveg", "skogsveg", "sykkelveg"] {
            assert_eq!(encode_fkb_type(Some(s)), 2, "{s}");
        }
        for s in ["traktorvei", "skogsvei", "sykkelvei"] {
            assert_eq!(encode_fkb_type(Some(s)), 2, "{s}");
        }
    }
}
