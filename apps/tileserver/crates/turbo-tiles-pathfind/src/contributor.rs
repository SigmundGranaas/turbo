//! Unit-aware cost contributor abstraction.
//!
//! ## Why this exists
//!
//! The legacy [`crate::cost::CostLayer`] returns multipliers. Eight
//! cost sources at three different lifecycles get multiplied
//! together with no shared unit and no rule saying "the result is
//! still walkable cost". That's the root cause every calibration
//! audit in this codebase has hit: turn one knob, break another
//! scenario, no way to express layer semantics in physical units.
//!
//! This module is the foundation of the cost-model unification
//! (plan Stage 2). Every contribution is expressed in **walk-
//! seconds added to traversing an edge**, where the baseline is
//! flat-trail pace ([`BASE_PACE_S_PER_M`]):
//!
//!   - Walking 100 m on a flat marked trail costs 100 × 0.714 ≈ 71 s.
//!   - A 30° slope on that same 100 m adds maybe +180 s (Tobler).
//!   - A 5 m wide stream crossing adds +35 s (per-crossing pace
//!     delta × crossing length).
//!   - A red-T marked sub-segment subtracts a small bonus, say -7 s.
//!
//! Composition is **addition**, not multiplication. Total walk-
//! seconds for an edge is a real, comparable quantity that closes
//! at the path level (sum across all edges).
//!
//! ## Migration strategy
//!
//! 1. Existing layers continue to work via the legacy `CostLayer`
//!    trait + multiplicative `compose_*` functions. Solver loops
//!    are unchanged.
//! 2. [`LegacyLayerAdapter`] in this module wraps any old layer as
//!    a `CostContributor`, translating its multiplier output into
//!    walk-seconds equivalent against the edge length.
//! 3. The new `/v1/debug/cost-breakdown` endpoint uses the
//!    contributor view exclusively, giving the curator a way to
//!    inspect per-contributor walk-seconds for any candidate edge
//!    without changing routing behaviour.
//! 4. Future layers can be written directly as `CostContributor`s
//!    with documented physical parameters. Each one ported away
//!    from the legacy trait reduces multiplicative coupling.
//! 5. Once enough layers are ported, the solver loops switch to
//!    `compose_edge_walk_seconds` and the old `compose_*` functions
//!    + `CostLayer` trait get removed.

use std::sync::Arc;

use turbo_tiles_graph::{EdgeRecord, Profile};

/// Walk pace at flat, maintained trail — seconds per metre.
/// 1.4 m/s ≈ 5 km/h is the standard hiking-cost baseline (Naismith,
/// Tobler). Adopting this as the unit means a 1 km route on a
/// flat trail costs exactly 714 walk-seconds, with deviations
/// expressed as additions / subtractions on top.
pub const BASE_PACE_S_PER_M: f64 = 1.0 / 1.4;

/// What edge the contributor is being asked about. Knowing graph
/// vs mesh lets contributors self-select — e.g. a marking-aware
/// layer only contributes for graph edges; an off-trail-base
/// contributor only fires for mesh edges.
pub enum EdgeKind<'a> {
    /// On-graph edge. Carries the baked attributes (gain/loss/
    /// slope_max, fkb_type, marking, source, attr_flags) so the
    /// contributor doesn't have to re-sample the DEM or look up
    /// surface from a config.
    Graph(&'a EdgeRecord),
    /// Off-trail mesh edge. No baked attributes; the contributor
    /// uses the from/to coords and queries any per-cell data it
    /// needs from its own attached primitives (DEM, vectors, etc).
    Mesh,
}

/// Geometric + categorical context for one edge a contributor is
/// asked to cost. UTM33N metres throughout; the API layer projects.
pub struct EdgeContext<'a> {
    /// Start coords (EPSG:25833 m).
    pub fx: f64,
    pub fy: f64,
    /// End coords (EPSG:25833 m).
    pub tx: f64,
    pub ty: f64,
    /// Edge length in metres. For graph edges this is the
    /// `EdgeRecord.length_m`, NOT the straight from→to distance,
    /// so contributions are correct against the actual trail
    /// polyline length (which may be much longer than the secant).
    pub length_m: f64,
    pub profile: Profile,
    pub kind: EdgeKind<'a>,
}

impl<'a> EdgeContext<'a> {
    /// Baseline walk-seconds for an edge before any contributions.
    /// This is the floor any composer adds contributions onto.
    pub fn base_walk_seconds(&self) -> f64 {
        self.length_m * BASE_PACE_S_PER_M
    }
}

/// What category a contributor falls in. The breakdown endpoint
/// uses this to group contributions in the response so the curator
/// can see "all slope-driven costs added 145 s in total" at a
/// glance. Purely informational — does not affect composition.
#[derive(Debug, Clone, Copy, PartialEq, Eq, serde::Serialize)]
#[serde(rename_all = "snake_case")]
pub enum ContributorKind {
    Slope,
    Surface,
    Vegetation,
    Hazard,
    Marking,
    Proximity,
    /// Contributions adapted from the legacy multiplicative
    /// [`crate::cost::CostLayer`] trait. As contributors get
    /// rewritten in physical units these tags shift to a more
    /// specific kind.
    Legacy,
}

/// A single physical contribution to the cost of traversing an
/// edge. All contributions are in **walk-seconds** (not metres,
/// not multipliers).
///
/// Positive contributions make the edge harder (slope, brush,
/// wetland); negative contributions make it preferred (DNT
/// marking, cairns, viewpoints).
pub trait CostContributor: Send + Sync {
    /// Stable lower-case identifier. Same convention as
    /// `CostLayer::name`.
    fn name(&self) -> &'static str;

    fn kind(&self) -> ContributorKind;

    /// Walk-seconds added (positive) or subtracted (negative) by
    /// this contributor for the given edge.
    ///
    /// Must NOT return `INFINITY` to signal refusal — use `veto`
    /// for that. Returning `INFINITY` here would silently veto via
    /// the composer's sum, hiding the reason from the breakdown.
    fn contribute(&self, ctx: &EdgeContext<'_>) -> f64;

    /// If the contributor refuses this edge, return a short label
    /// (`"water"`, `"glacier"`, `"private"`). First non-`None`
    /// veto wins; composition stops there and reports the label.
    fn veto(&self, _ctx: &EdgeContext<'_>) -> Option<&'static str> {
        None
    }

    /// Multiplicative pace factor applied to the WHOLE composed pace
    /// (base + all additive deltas), as opposed to `contribute`'s
    /// additive walk-seconds. Default `1.0` (no effect).
    ///
    /// This is the channel for effects that *scale* effort rather
    /// than *add* a constant: off-trail roughness (rough ground makes
    /// every metre — including the climb — proportionally harder),
    /// and future surface / seasonal-snow layers. The composer applies
    /// the product of all factors after summing the additive deltas:
    /// `total = (base + Σ contribute) × Π pace_factor`. Keeping it
    /// separate from `contribute` is what lets, e.g., a trail-proximity
    /// *bonus* and an off-trail *roughness* compose correctly (the
    /// roughness scales the post-bonus pace, matching the previous
    /// hard-coded `tobler × off × mul` in the solver).
    fn pace_factor(&self, _ctx: &EdgeContext<'_>) -> f64 {
        1.0
    }
}

/// Composed cost of one edge: base traversal time + contribution
/// from each contributor, expressed in walk-seconds. Vetoed edges
/// short-circuit with the layer name that vetoed them.
#[derive(Debug, Clone, serde::Serialize)]
pub struct EdgeWalkCost {
    pub base_walk_seconds: f64,
    pub contributions: Vec<NamedContribution>,
    pub total_walk_seconds: f64,
    /// `Some(layer_name)` if any contributor vetoed. When set,
    /// `total_walk_seconds = f64::INFINITY` and contributions
    /// lists only those evaluated before the veto.
    pub vetoed_by: Option<String>,
}

#[derive(Debug, Clone, serde::Serialize)]
pub struct NamedContribution {
    pub name: String,
    pub kind: ContributorKind,
    pub walk_seconds: f64,
}

/// Sum every contributor's walk-seconds contribution onto the base
/// traversal time. Veto short-circuits.
pub fn compose_edge_walk_seconds(
    contributors: &[Arc<dyn CostContributor>],
    ctx: &EdgeContext<'_>,
) -> EdgeWalkCost {
    let base = ctx.base_walk_seconds();
    let mut contributions: Vec<NamedContribution> = Vec::with_capacity(contributors.len());
    let mut total = base;
    for c in contributors {
        if let Some(label) = c.veto(ctx) {
            return EdgeWalkCost {
                base_walk_seconds: base,
                contributions,
                total_walk_seconds: f64::INFINITY,
                vetoed_by: Some(label.to_string()),
            };
        }
        let s = c.contribute(ctx);
        if !s.is_finite() {
            // A non-finite contribution from a non-vetoing
            // contributor is a contract violation; surface it as
            // an explicit veto so the breakdown shows the bug.
            return EdgeWalkCost {
                base_walk_seconds: base,
                contributions,
                total_walk_seconds: f64::INFINITY,
                vetoed_by: Some(c.name().to_string()),
            };
        }
        total += s;
        contributions.push(NamedContribution {
            name: c.name().to_string(),
            kind: c.kind(),
            walk_seconds: s,
        });
    }
    // Multiplicative pace factors apply to the WHOLE composed pace
    // after the additive deltas (default 1.0 for every contributor, so
    // this is a no-op unless a multiplicative contributor — e.g.
    // off-trail roughness — is present).
    let mut factor = 1.0f64;
    for c in contributors {
        let f = c.pace_factor(ctx);
        if f.is_finite() && f > 0.0 {
            factor *= f;
        }
    }
    total *= factor;
    EdgeWalkCost {
        base_walk_seconds: base,
        contributions,
        total_walk_seconds: total,
        vetoed_by: None,
    }
}

/// Adapter: present any [`crate::cost::CostLayer`] (the legacy
/// multiplicative trait) as a [`CostContributor`] by converting
/// its multiplier output into walk-seconds-equivalent against
/// the edge length.
///
/// The conversion: a legacy multiplier `M` on a length `L`
/// represents a cost of `baked × M` in opaque units. In walk-
/// seconds the equivalent contribution is `(M - 1) × L × BASE_PACE
/// _S_PER_M` — the "extra time" the multiplier represents over
/// the flat-trail baseline. `M = 1.0` → zero contribution. `M =
/// 1.5` → +50% of base traversal time. `M = INFINITY` → veto.
///
/// This is an APPROXIMATION (the legacy multipliers don't really
/// represent walk-time deltas — they're gut-feel scales). It's
/// good enough to keep existing layers running while the cost
/// model migrates contributor-by-contributor.
pub struct LegacyLayerAdapter {
    inner: Arc<dyn crate::cost::CostLayer>,
}

impl LegacyLayerAdapter {
    pub fn new(inner: Arc<dyn crate::cost::CostLayer>) -> Self {
        Self { inner }
    }
}

impl CostContributor for LegacyLayerAdapter {
    fn name(&self) -> &'static str {
        self.inner.name()
    }
    fn kind(&self) -> ContributorKind {
        ContributorKind::Legacy
    }
    fn contribute(&self, ctx: &EdgeContext<'_>) -> f64 {
        let mult = match &ctx.kind {
            EdgeKind::Graph(er) => self.inner.edge_multiplier(er, ctx.profile),
            EdgeKind::Mesh => {
                self.inner
                    .edge_cost_modifier(ctx.fx, ctx.fy, ctx.tx, ctx.ty, ctx.profile)
            }
        };
        if !mult.is_finite() {
            // The composer reads `veto()` first; if we get here it
            // means the legacy layer is INFINITY for this edge but
            // didn't surface it via veto. Return INFINITY so the
            // composer's contract violation branch flags this in
            // the breakdown.
            return f64::INFINITY;
        }
        ((mult as f64) - 1.0) * ctx.length_m * BASE_PACE_S_PER_M
    }
    fn veto(&self, ctx: &EdgeContext<'_>) -> Option<&'static str> {
        let mult = match &ctx.kind {
            EdgeKind::Graph(er) => self.inner.edge_multiplier(er, ctx.profile),
            EdgeKind::Mesh => {
                self.inner
                    .edge_cost_modifier(ctx.fx, ctx.fy, ctx.tx, ctx.ty, ctx.profile)
            }
        };
        if !mult.is_finite() {
            // Use the layer's name as the veto label; the legacy
            // trait doesn't carry a refusal reason string.
            Some(static_name_or_default(self.inner.name()))
        } else {
            None
        }
    }
}

/// Trait-object `name()` returns `&'static str` already, but the
/// adapter wants a stable identifier even if the inner trait's
/// implementation got dynamic. Keep this trivial today; revisit
/// when refusal labels grow richer.
fn static_name_or_default(name: &'static str) -> &'static str {
    name
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::cost::{CellCost, CostLayer};

    struct LegacyFlat(f32);
    impl CostLayer for LegacyFlat {
        fn name(&self) -> &'static str { "flat_test" }
        fn cell_cost(&self, _x: f64, _y: f64, _p: Profile) -> CellCost {
            CellCost::multiplier(1.0)
        }
        fn edge_cost_modifier(&self, _fx: f64, _fy: f64, _tx: f64, _ty: f64, _p: Profile) -> f32 {
            self.0
        }
        fn edge_multiplier(&self, _e: &EdgeRecord, _p: Profile) -> f32 {
            self.0
        }
    }

    fn ctx(length_m: f64) -> EdgeContext<'static> {
        EdgeContext {
            fx: 0.0,
            fy: 0.0,
            tx: length_m,
            ty: 0.0,
            length_m,
            profile: Profile::Foot,
            kind: EdgeKind::Mesh,
        }
    }

    #[test]
    fn base_walk_seconds_at_flat_pace() {
        // 100 m at 1.4 m/s = ~71.4 s.
        let c = ctx(100.0);
        assert!((c.base_walk_seconds() - 71.428).abs() < 0.01);
    }

    #[test]
    fn legacy_adapter_multiplier_one_is_zero_contribution() {
        let adapter = LegacyLayerAdapter::new(Arc::new(LegacyFlat(1.0)));
        let c = ctx(100.0);
        assert!((adapter.contribute(&c)).abs() < 1e-6);
    }

    #[test]
    fn legacy_adapter_multiplier_two_doubles_base() {
        // Multiplier 2.0 on 100 m → (2-1) × 100 × 0.714 ≈ +71.4 s
        // (an extra trail's worth of time on top of base).
        let adapter = LegacyLayerAdapter::new(Arc::new(LegacyFlat(2.0)));
        let c = ctx(100.0);
        assert!((adapter.contribute(&c) - 71.428).abs() < 0.01);
    }

    #[test]
    fn legacy_adapter_infinity_is_veto() {
        let adapter = LegacyLayerAdapter::new(Arc::new(LegacyFlat(f32::INFINITY)));
        let c = ctx(100.0);
        assert_eq!(adapter.veto(&c), Some("flat_test"));
    }

    #[test]
    fn compose_sums_contributions() {
        let layers: Vec<Arc<dyn CostContributor>> = vec![
            Arc::new(LegacyLayerAdapter::new(Arc::new(LegacyFlat(2.0)))),
            Arc::new(LegacyLayerAdapter::new(Arc::new(LegacyFlat(1.5)))),
        ];
        let c = ctx(100.0);
        let cost = compose_edge_walk_seconds(&layers, &c);
        // base = 71.4; first +71.4; second +35.7 → total 178.5.
        assert!((cost.base_walk_seconds - 71.428).abs() < 0.01);
        assert!((cost.total_walk_seconds - 178.571).abs() < 0.01);
        assert_eq!(cost.contributions.len(), 2);
        assert!(cost.vetoed_by.is_none());
    }

    #[test]
    fn compose_veto_short_circuits() {
        let layers: Vec<Arc<dyn CostContributor>> = vec![
            Arc::new(LegacyLayerAdapter::new(Arc::new(LegacyFlat(2.0)))),
            Arc::new(LegacyLayerAdapter::new(Arc::new(LegacyFlat(f32::INFINITY)))),
            Arc::new(LegacyLayerAdapter::new(Arc::new(LegacyFlat(1.5)))),
        ];
        let c = ctx(100.0);
        let cost = compose_edge_walk_seconds(&layers, &c);
        assert!(cost.total_walk_seconds.is_infinite());
        assert_eq!(cost.vetoed_by.as_deref(), Some("flat_test"));
        // Only the first (passing) contributor was recorded.
        assert_eq!(cost.contributions.len(), 1);
    }
}
