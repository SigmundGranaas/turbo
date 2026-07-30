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

pub use turbo_route_model::Requirement;
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

/// Memoized elevation samples along one edge, shared across the
/// contributor stack via [`EdgeContext::elev_probe`].
///
/// The slope-family contributors (tobler slope, naismith gain,
/// dem-coverage, contour crossing) all sample the same evenly-spaced
/// points along the edge; without sharing, each re-samples the DEM
/// independently — measured at ~93% of ALL DEM work on the off-trail
/// corpus (~4× redundancy per evaluated cell). The probe computes the
/// profile once per requested density `n` and hands the same samples
/// to every consumer. Values are identical to direct sampling; only
/// the redundancy is removed.
pub struct EdgeElevProbe<'a> {
    dem: &'a dyn turbo_route_model::Heightfield,
    fx: f64,
    fy: f64,
    tx: f64,
    ty: f64,
    /// `(n, samples)` memo — contributors request specific densities;
    /// they almost always agree on one `n`, so this stays at 1-2
    /// entries and a linear scan beats any map.
    memo: std::cell::RefCell<Vec<(usize, std::rc::Rc<Vec<Option<f32>>>)>>,
    /// Per-point memo keyed on the parameter `t`'s bit pattern. Equal
    /// rationals produce bit-identical `i as f64 / n as f64` (IEEE
    /// division is correctly rounded), so coincident points SHARE
    /// across densities — e.g. n=2's {0, ½, 1} are a subset of n=4's
    /// {0, ¼, ½, ¾, 1}. Tiny (≤ ~10 entries); linear scan.
    points: std::cell::RefCell<Vec<(u64, Option<f32>)>>,
}

impl<'a> EdgeElevProbe<'a> {
    pub fn new(
        dem: &'a dyn turbo_route_model::Heightfield,
        fx: f64,
        fy: f64,
        tx: f64,
        ty: f64,
    ) -> Self {
        Self {
            dem,
            fx,
            fy,
            tx,
            ty,
            memo: std::cell::RefCell::new(Vec::new()),
            points: std::cell::RefCell::new(Vec::new()),
        }
    }

    /// Elevation at parameter `t` along the edge, point-memoized.
    fn point(&self, t: f64, dx: f64, dy: f64) -> Option<f32> {
        let bits = t.to_bits();
        if let Some(&(_, z)) = self.points.borrow().iter().find(|(b, _)| *b == bits) {
            return z;
        }
        let p = turbo_route_model::Point {
            x: self.fx + dx * t,
            y: self.fy + dy * t,
        };
        let z = self.dem.height_at(p);
        self.points.borrow_mut().push((bits, z));
        z
    }

    /// `n+1` evenly-spaced elevation samples from (fx,fy) to (tx,ty),
    /// memoized per `n` (and per point across densities). `None`
    /// entries = DEM nodata at that point.
    pub fn elevations(&self, n: usize) -> std::rc::Rc<Vec<Option<f32>>> {
        if let Some((_, zs)) = self.memo.borrow().iter().find(|(m, _)| *m == n) {
            return zs.clone();
        }
        let dx = self.tx - self.fx;
        let dy = self.ty - self.fy;
        let mut zs: Vec<Option<f32>> = Vec::with_capacity(n + 1);
        for i in 0..=n {
            let t = i as f64 / n as f64;
            zs.push(self.point(t, dx, dy));
        }
        let rc = std::rc::Rc::new(zs);
        self.memo.borrow_mut().push((n, rc.clone()));
        rc
    }
}

/// Geometric + categorical context for one edge a contributor is
/// asked to cost. Planar metres throughout — the engine has no CRS (C4).
pub struct EdgeContext<'a> {
    /// Start coords, planar metres.
    pub fx: f64,
    pub fy: f64,
    /// End coords, planar metres.
    pub tx: f64,
    pub ty: f64,
    /// Edge length in metres. For graph edges this is the
    /// `EdgeRecord.length_m`, NOT the straight from→to distance,
    /// so contributions are correct against the actual trail
    /// polyline length (which may be much longer than the secant).
    pub length_m: f64,
    pub profile: Profile,
    pub kind: EdgeKind<'a>,
    /// Shared elevation sampler for this edge (see [`EdgeElevProbe`]).
    /// `None` = contributors sample their own primitives directly
    /// (graph edges, tests, callers that don't price terrain).
    pub elev_probe: Option<&'a EdgeElevProbe<'a>>,
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

pub trait CostContributor: Send + Sync {
    /// Stable lower-case identifier. Same convention as
    /// `CostLayer::name`.
    fn name(&self) -> &'static str;

    fn kind(&self) -> ContributorKind;

    /// Is this contributor's data load-bearing? Default `Advisory`, so a
    /// new contributor can never accidentally widen routing coverage.
    ///
    /// Contributors sharing one data source (all the DEM-backed slope
    /// family, say) have identical extents, so marking each of them
    /// `Required` is correct and idempotent rather than redundant.
    fn requirement(&self) -> Requirement {
        Requirement::Advisory
    }

    /// Does this contributor have authoritative data at this point?
    /// Only consulted for `Required` contributors.
    fn covers(&self, _x: f64, _y: f64) -> bool {
        true
    }

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

/// A single physical contribution to the cost of traversing an
/// edge. All contributions are in **walk-seconds** (not metres,
/// not multipliers).
///
/// Positive contributions make the edge harder (slope, brush,
/// wetland); negative contributions make it preferred (DNT
/// marking, cairns, viewpoints).
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

// `LegacyLayerAdapter` was deleted in B1 along with the multiplicative
// cost channel it bridged. Every production layer has a native
// contributor (`DISPLACED_LEGACY_LAYERS`), so the adapter's only caller
// -- the breakdown fallback for `Pathfinder::new` consumers with no
// natives registered -- was unreachable in practice.

#[cfg(test)]
mod tests {
    use super::*;

    /// A contributor that adds a fixed number of walk-seconds per metre,
    /// or vetoes. Replaces the `LegacyLayerAdapter` that these tests used
    /// as a double before B1 deleted it.
    struct Flat {
        s_per_m: f64,
        veto: bool,
    }
    impl CostContributor for Flat {
        fn name(&self) -> &'static str {
            "flat_test"
        }
        fn kind(&self) -> ContributorKind {
            ContributorKind::Legacy
        }
        fn contribute(&self, ctx: &EdgeContext<'_>) -> f64 {
            self.s_per_m * ctx.length_m
        }
        fn veto(&self, _ctx: &EdgeContext<'_>) -> Option<&'static str> {
            self.veto.then_some("flat_test")
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
            elev_probe: None,
        }
    }

    #[test]
    fn base_walk_seconds_at_flat_pace() {
        // 100 m at 1.4 m/s = ~71.4 s.
        let c = ctx(100.0);
        assert!((c.base_walk_seconds() - 71.428).abs() < 0.01);
    }

    #[test]
    fn compose_sums_contributions() {
        let layers: Vec<Arc<dyn CostContributor>> = vec![
            Arc::new(Flat {
                s_per_m: BASE_PACE_S_PER_M,
                veto: false,
            }),
            Arc::new(Flat {
                s_per_m: 0.5 * BASE_PACE_S_PER_M,
                veto: false,
            }),
        ];
        let c = ctx(100.0);
        let cost = compose_edge_walk_seconds(&layers, &c);
        // base = 71.4; first +71.4; second +35.7 -> total 178.5.
        assert!((cost.base_walk_seconds - 71.428).abs() < 0.01);
        assert!((cost.total_walk_seconds - 178.571).abs() < 0.01);
        assert_eq!(cost.contributions.len(), 2);
        assert!(cost.vetoed_by.is_none());
    }

    #[test]
    fn compose_veto_short_circuits() {
        let layers: Vec<Arc<dyn CostContributor>> = vec![
            Arc::new(Flat {
                s_per_m: BASE_PACE_S_PER_M,
                veto: false,
            }),
            Arc::new(Flat {
                s_per_m: 0.0,
                veto: true,
            }),
            Arc::new(Flat {
                s_per_m: 0.5 * BASE_PACE_S_PER_M,
                veto: false,
            }),
        ];
        let c = ctx(100.0);
        let cost = compose_edge_walk_seconds(&layers, &c);
        assert!(cost.total_walk_seconds.is_infinite());
        assert_eq!(cost.vetoed_by.as_deref(), Some("flat_test"));
        // Only the first (passing) contributor was recorded.
        assert_eq!(cost.contributions.len(), 1);
    }
}
