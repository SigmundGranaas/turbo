//! The streaming priority score (plan slice B2): ONE explainable function
//! deciding what loads first, replacing per-host approximations (the desktop
//! host re-sorted the engine's careful order by raw centre-distance,
//! discarding tiers — "most important first" was approximated, not stated).
//!
//! `Priority` is a single ordered `u64` (lower = more urgent) packed from
//! decomposable terms, so a trace can always answer *why* a chunk led:
//!
//! ```text
//! bits 62..64  tier         — the coarse law: Overview ≺ Visible ≺
//!                             SurfaceForVisible ≺ Prefetch. Never violated
//!                             by any lower term.
//! bits 30..62  distance     — effective distance² to the camera eye,
//!                             IEEE-bit-ordered (positive floats compare as
//!                             integers). "Effective": modulated by motion
//!                             alignment so the map streams WHERE THE CAMERA
//!                             IS HEADING (up to ±30 %), never enough to
//!                             cross tiers.
//! bits  0..30  reserved     — the SSE-benefit term lands here with S6, once
//!                             selection speaks geometric error end-to-end.
//! ```
//!
//! Parity contract (the B2 gate): with zero camera velocity the score orders
//! exactly like the historical `(tier, distance²)` sort — pinned by the
//! oracle fuzz test below and by `turbomap-core`'s parity test against the
//! live selection.

use serde::{Deserialize, Serialize};

/// Why a chunk is wanted — the coarse fetch law. Variant order IS the fetch
/// order.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash, Serialize, Deserialize)]
pub enum Tier {
    /// The cheap coarse backdrop: a handful of chunks that guarantee the
    /// screen is never empty. Fetched first, always.
    Overview,
    /// On-screen content at target detail.
    Visible,
    /// Ground-surface data (DEM/TIN) needed by visible content. Reserved —
    /// activates in a later, measured slice; today's callers map surface
    /// data to [`Tier::Visible`] to preserve the shipped interleave.
    SurfaceForVisible,
    /// The off-screen warm-up ring: nice to have, never at the expense of
    /// anything above.
    Prefetch,
}

impl Tier {
    fn rank(self) -> u64 {
        match self {
            Tier::Overview => 0,
            Tier::Visible => 1,
            Tier::SurfaceForVisible => 2,
            Tier::Prefetch => 3,
        }
    }
}

/// One ordered score; lower fetches first. See the module docs for the bit
/// layout.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub struct Priority(pub u64);

/// Fraction by which perfect motion alignment shrinks (ahead) or grows
/// (behind) a chunk's effective distance. 0.3 keeps the bias meaningful but
/// strictly within-tier.
pub const MOTION_GAIN: f32 = 0.3;

/// Modulate a squared distance by how well the chunk lies along the camera's
/// direction of travel. `alignment` is `dot(travel_dir, dir_to_chunk)` in
/// `[-1, 1]`; 0 (stationary or perpendicular) is the identity — the parity
/// case.
pub fn effective_distance_sq(distance_sq: f32, alignment: f32) -> f32 {
    let a = if alignment.is_finite() {
        alignment.clamp(-1.0, 1.0)
    } else {
        0.0
    };
    let d = if distance_sq.is_finite() {
        distance_sq.max(0.0)
    } else {
        f32::MAX
    };
    d * (1.0 - MOTION_GAIN * a)
}

/// Pack the terms into the ordered score.
pub fn score(tier: Tier, effective_distance_sq: f32) -> Priority {
    // Positive IEEE-754 floats order identically to their bit patterns, so
    // the packed integer preserves distance order exactly. Non-finite guards
    // sort last within their tier rather than poisoning the order.
    let d = if effective_distance_sq.is_finite() {
        effective_distance_sq.max(0.0)
    } else {
        f32::MAX
    };
    Priority((tier.rank() << 62) | ((d.to_bits() as u64) << 30))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn tier_dominates_any_distance() {
        // The farthest Visible chunk still beats the nearest Prefetch chunk.
        let far_visible = score(Tier::Visible, f32::MAX / 2.0);
        let near_prefetch = score(Tier::Prefetch, 0.0);
        assert!(far_visible < near_prefetch);
        let far_overview = score(Tier::Overview, f32::MAX / 2.0);
        assert!(far_overview < score(Tier::Visible, 0.0));
    }

    /// The parity oracle: over arbitrary (tier, distance) pairs the packed
    /// score orders exactly like the historical lexicographic sort.
    #[test]
    fn fuzz_score_orders_exactly_like_the_tier_then_distance_oracle() {
        let mut state: u64 = 0xC0FF_EE00_D15E_A5E5;
        let mut next = move || {
            state = state
                .wrapping_mul(6_364_136_223_846_793_005)
                .wrapping_add(1_442_695_040_888_963_407);
            state
        };
        let tiers = [
            Tier::Overview,
            Tier::Visible,
            Tier::SurfaceForVisible,
            Tier::Prefetch,
        ];
        let mut items: Vec<(Tier, f32)> = (0..512)
            .map(|_| {
                let t = tiers[(next() % 4) as usize];
                // Distances spanning subnormal-ish to huge, incl. exact ties.
                let d = match next() % 5 {
                    0 => 0.0,
                    1 => (next() % 1000) as f32 * 1e-6,
                    2 => (next() % 1000) as f32,
                    3 => (next() % 1000) as f32 * 1e6,
                    _ => 42.0,
                };
                (t, d)
            })
            .collect();

        let mut by_score = items.clone();
        by_score.sort_by_key(|&(t, d)| score(t, d));
        items.sort_by(|a, b| a.0.cmp(&b.0).then(a.1.partial_cmp(&b.1).expect("finite")));
        // Compare as (tier, distance) sequences: equal-score ties may permute
        // equal items, which is exactly what the oracle allows too.
        let a: Vec<(Tier, u32)> = by_score.iter().map(|&(t, d)| (t, d.to_bits())).collect();
        let b: Vec<(Tier, u32)> = items.iter().map(|&(t, d)| (t, d.to_bits())).collect();
        assert_eq!(a, b);
    }

    #[test]
    fn motion_pulls_ahead_chunks_forward_and_pushes_behind_chunks_back() {
        let d = 100.0f32;
        let ahead = effective_distance_sq(d, 1.0);
        let still = effective_distance_sq(d, 0.0);
        let behind = effective_distance_sq(d, -1.0);
        assert!(ahead < still && still < behind);
        assert_eq!(still, d, "zero alignment is the exact parity case");
        assert!((ahead - d * (1.0 - MOTION_GAIN)).abs() < 1e-3);
        // Never enough to cross tiers regardless of alignment.
        assert!(score(Tier::Visible, behind) < score(Tier::Prefetch, ahead));
    }
}
