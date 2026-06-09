//! Native [`CostContributor`] implementations — the first set of
//! contributors that report their cost in **walk-seconds directly**
//! rather than going through the legacy multiplier-→-seconds
//! [`crate::contributor::LegacyLayerAdapter`].
//!
//! ## Why "native" matters
//!
//! `LegacyLayerAdapter` converts `multiplier - 1.0` to walk-seconds.
//! That's a faithful conversion for the composer's arithmetic, but
//! the underlying multipliers themselves were gut-feel scales (the
//! original `SlopeLayer` used `1 + (slope/12)²`, an empirical curve
//! tuned by eye). The native impls below replace those with physical
//! models — Tobler's hiking function for slope, a per-marking
//! seconds-per-kilometre bonus table, mask veto for water/glacier
//! — and the breakdown endpoint reports their walk-seconds in real
//! physical units rather than approximated ones.
//!
//! ## What's wired
//!
//! These contributors are surfaced through
//! [`Pathfinder::cost_breakdown`] when the corresponding legacy
//! layer is registered: the breakdown swaps the legacy adapter for
//! the native implementation so the curator inspecting an edge in
//! the SPA sees real physics. The solver loops themselves still run
//! on the multiplicative composer for now — switching them is the
//! Stage 2 follow-up tracked under the same task.
//!
//! Each native contributor is paired with the legacy layer it
//! displaces via `displaces_legacy_layer`, so the boot wiring can
//! recognise the pair and avoid double-counting.

use std::sync::Arc;

use rstar::{PointDistance, RTree, RTreeObject, AABB};
use turbo_tiles_elev::{Dem, PointXY};
use turbo_tiles_graph::{Graph, Profile};
use turbo_tiles_mask::{Mask, RefusalKind};
use turbo_tiles_vector::{AttrView, GeomKind, VectorCollection};

use crate::contributor::{
    ContributorKind, CostContributor, EdgeContext, EdgeKind, BASE_PACE_S_PER_M,
};

/// Slope cost via Tobler's hiking function, integrated along the
/// edge with multi-point sampling so long Theta* line-of-sight
/// jumps can't "skip over" a ridge or valley between endpoints.
///
/// Tobler's signed-slope hiking velocity in m/s:
///
/// ```text
///   v(s) = 1.6667 × exp(-3.5 × |s + 0.05|)
/// ```
///
/// where `s = dz/dx` is the signed slope (uphill positive, downhill
/// negative). The model is asymmetric: gentle descent (s = -0.05) is
/// the optimum, gentle ascent is slightly worse than flat, and steep
/// up OR steep down both penalise.
///
/// Algorithm:
///   1. Sample DEM elevations at N+1 evenly-spaced points along the
///      edge from `(fx,fy)` to `(tx,ty)`. N = clamp(length / 12 m,
///      2, 64) so 25 m mesh cells get 2 segments and 500 m LoS
///      jumps get 42 segments — enough to catch any single 30 m
///      DEM cell of bad terrain along the line.
///   2. For each sub-segment compute signed slope, then Tobler pace.
///   3. Extra walk-seconds added by the edge = Σ over segments of
///      `(pace_i - BASE_PACE) × seg_len_i`. Multi-segment integration
///      makes "midpoint slope = 0 but the path actually climbs a
///      ridge" impossible — the ridge cell contributes its own
///      large pace term and the sum dominates.
///
/// Veto fires when ANY sub-segment exceeds `refuse_above_deg`, so a
/// short cliff inside a long LoS jump still refuses the whole edge.
/// Returns 0 contribution for entirely out-of-DEM edges (nothing to
/// physically opine about).
pub struct ToblerSlopeContributor {
    pub dem: Arc<Dem>,
    pub refuse_above_deg: f32,
    /// Target metres per sample. Smaller → finer integration but
    /// more DEM lookups per edge. 12 m matches the underlying DEM
    /// resolution (10 m bottom layer); anything finer is wasted
    /// effort.
    pub sample_step_m: f64,
}

impl ToblerSlopeContributor {
    pub fn new(dem: Arc<Dem>, refuse_above_deg: f32) -> Self {
        Self {
            dem,
            refuse_above_deg,
            sample_step_m: 6.0,
        }
    }

    fn sample_count(&self, length_m: f64) -> usize {
        ((length_m / self.sample_step_m).round() as usize).clamp(2, 64)
    }

    /// Sample N+1 elevations along the edge. Returns None for
    /// edges fully outside the DEM (the contributor stays neutral
    /// in that case). `Some(vec)` of length N+1 otherwise; missing
    /// per-cell samples are filled by linear interpolation from
    /// neighbours so a single-pixel gap doesn't kill the integration.
    fn sample_elevations(&self, ctx: &EdgeContext<'_>) -> Option<Vec<f32>> {
        let n = self.sample_count(ctx.length_m);
        // Shared probe when present (one DEM pass for the whole
        // contributor stack); direct sampling otherwise.
        let mut zs: Vec<Option<f32>> = match ctx.elev_probe {
            Some(p) => p.elevations(n).as_ref().clone(),
            None => {
                let mut v = Vec::with_capacity(n + 1);
                let dx = ctx.tx - ctx.fx;
                let dy = ctx.ty - ctx.fy;
                for i in 0..=n {
                    let t = i as f64 / n as f64;
                    let x = ctx.fx + dx * t;
                    let y = ctx.fy + dy * t;
                    v.push(self.dem.sample(PointXY { x, y }).ok().flatten());
                }
                v
            }
        };
        if zs.iter().all(Option::is_none) {
            return None;
        }
        // Forward-fill then back-fill so small gaps don't sink the
        // integral; an all-None edge already short-circuited above.
        let mut last: Option<f32> = None;
        for z in &mut zs {
            if z.is_none() {
                *z = last;
            }
            if z.is_some() {
                last = *z;
            }
        }
        let mut last: Option<f32> = None;
        for z in zs.iter_mut().rev() {
            if z.is_none() {
                *z = last;
            }
            if z.is_some() {
                last = *z;
            }
        }
        Some(zs.into_iter().map(|z| z.unwrap_or(0.0)).collect())
    }

    /// Fraction of evenly-spaced DEM samples along the edge that
    /// came back as nodata. 0.0 = full coverage, 1.0 = entirely
    /// outside the DEM. Used by [`DemCoveragePenaltyContributor`]
    /// to charge edges that route through terrain we can't see.
    pub fn sample_missing_fraction(&self, ctx: &EdgeContext<'_>) -> f64 {
        let n = self.sample_count(ctx.length_m);
        if let Some(p) = ctx.elev_probe {
            let zs = p.elevations(n);
            let missing = zs.iter().filter(|z| z.is_none()).count();
            return missing as f64 / zs.len() as f64;
        }
        let dx = ctx.tx - ctx.fx;
        let dy = ctx.ty - ctx.fy;
        let mut missing = 0usize;
        let mut total = 0usize;
        for i in 0..=n {
            let t = i as f64 / n as f64;
            let x = ctx.fx + dx * t;
            let y = ctx.fy + dy * t;
            total += 1;
            if self.dem.sample(PointXY { x, y }).ok().flatten().is_none() {
                missing += 1;
            }
        }
        if total == 0 {
            0.0
        } else {
            missing as f64 / total as f64
        }
    }

    /// Pure function over slope-degrees → walk-seconds per metre
    /// delta against the flat-trail baseline. Exposed for unit
    /// tests. Treats slope as absolute (symmetric) — the integrated
    /// `contribute` path computes signed slope per sub-segment.
    pub fn delta_seconds_per_metre(slope_deg: f32) -> f64 {
        let s_rad = (slope_deg as f64).to_radians();
        let grad = s_rad.tan().abs();
        let v = 1.6667 * (-3.5 * (grad + 0.05).abs()).exp();
        if v <= 1e-6 {
            return 100.0;
        }
        let pace = 1.0 / v;
        pace - BASE_PACE_S_PER_M
    }

    /// Pace (seconds per metre) at a given signed slope (rise/run).
    /// Asymmetric Tobler — minimum at `s = -0.05` (gentle descent).
    fn pace_at_signed_slope(slope: f64) -> f64 {
        let v = 1.6667 * (-3.5 * (slope + 0.05).abs()).exp();
        if v <= 1e-6 {
            100.0
        } else {
            1.0 / v
        }
    }
}

impl CostContributor for ToblerSlopeContributor {
    fn name(&self) -> &'static str {
        "slope"
    }
    fn kind(&self) -> ContributorKind {
        ContributorKind::Slope
    }
    fn contribute(&self, ctx: &EdgeContext<'_>) -> f64 {
        let Some(zs) = self.sample_elevations(ctx) else {
            return 0.0;
        };
        if zs.len() < 2 {
            return 0.0;
        }
        let seg_len = ctx.length_m / (zs.len() - 1) as f64;
        if seg_len < 1e-6 {
            return 0.0;
        }
        let mut extra = 0.0;
        for w in zs.windows(2) {
            let dz = (w[1] - w[0]) as f64;
            let slope = dz / seg_len;
            let pace = Self::pace_at_signed_slope(slope);
            extra += (pace - BASE_PACE_S_PER_M) * seg_len;
        }
        extra
    }
    fn veto(&self, ctx: &EdgeContext<'_>) -> Option<&'static str> {
        // Mesh slope is NEVER hard-vetoed: the continuous Tobler cost
        // (`contribute`) makes steep ground exponentially expensive, so
        // the geodesic curves around it on the gentle line — but the
        // corridor stays *connected*, so the FMM always reaches the goal
        // and we never drop to the blocky Theta* fallback. Genuine
        // near-vertical cliffs are refused by the FMM metric
        // (`tobler_aniso::metric_at` above `cliff_refuse_deg`); graph
        // edges that traverse cliffs are vetoed by GraphSlopeContributor.
        // A hard 45° (or even 60°) mesh wall here severed alpine
        // ascent corridors and forced the angular fallback.
        let _ = ctx;
        None
    }
}

/// Marking bonus on graph edges. Maps the baked `EdgeRecord.marking`
/// code to a walk-seconds-per-kilometre delta. Red-T marked trails
/// return a small NEGATIVE contribution (the curator prefers them
/// over equivalent unmarked routes); explicitly unmarked footpaths
/// return a small positive contribution (penalty for harder
/// wayfinding). Mesh edges always return zero — marking is a graph-
/// edge attribute that doesn't exist outside the trail network.
///
/// The seconds-per-km table is intentionally small (single-digit %
/// of base pace) so marking influence stays a preference signal
/// rather than a routing dominator.
pub struct MarkingBonusContributor {
    /// Indexed by `EdgeRecord.marking`. Units: seconds saved per
    /// metre traversed on this marking (negative = bonus, positive
    /// = penalty). At 1/1.4 = 0.714 s/m base pace, a -0.07 s/m
    /// bonus corresponds to ~10% speedup, roughly matching the
    /// old multiplier 0.90.
    pub seconds_per_metre: [f64; 8],
}

impl Default for MarkingBonusContributor {
    fn default() -> Self {
        let mut tbl = [0.0f64; 8];
        // -0.107 s/m ≈ -107 s/km ≈ -1.8 min/km bonus on red-T trails,
        // the canonical Norwegian marking. Matches the legacy 0.85×
        // multiplier when expressed in walk-seconds against the
        // flat-trail pace.
        tbl[1] = -(BASE_PACE_S_PER_M) * 0.15; // red T
        tbl[2] = -(BASE_PACE_S_PER_M) * 0.10; // cairn
        tbl[3] = -(BASE_PACE_S_PER_M) * 0.08; // blue paint
        tbl[4] = (BASE_PACE_S_PER_M) * 0.15; // unmarked → mild penalty
        Self {
            seconds_per_metre: tbl,
        }
    }
}

impl CostContributor for MarkingBonusContributor {
    fn name(&self) -> &'static str {
        "marking"
    }
    fn kind(&self) -> ContributorKind {
        ContributorKind::Marking
    }
    fn contribute(&self, ctx: &EdgeContext<'_>) -> f64 {
        match ctx.kind {
            EdgeKind::Graph(er) => {
                let idx = (er.marking as usize).min(7);
                self.seconds_per_metre[idx] * ctx.length_m
            }
            EdgeKind::Mesh => 0.0,
        }
    }
}

/// Per-surface pace multiplier on graph edges. Reads the baked
/// `EdgeRecord.fkb_type` (0 = unknown, 1 = sti/trail, 2 = vei/road-
/// class, 3 = skiloype) and scales the WHOLE edge pace via the
/// `pace_factor` channel — so a car road can be made only slightly
/// cheaper than open ground (`vei` ≈ `off_trail_base`) to stop the
/// router taking big road detours. Mesh edges are unaffected (their
/// off-trail cost is the `OffTrailRoughness` pace factor).
///
/// NOTE: `traktorvei` and `skogsvei` fold into the `vei` (code 2)
/// class in the graph artifact, so all road-class edges share one
/// knob today. A finer gravel-vs-asphalt split would key on
/// `EdgeRecord.surface` (a follow-up).
pub struct SurfacePaceContributor {
    /// `[profile_id][fkb_type code 0..=3]` → pace multiplier (1.0 =
    /// no effect). profile_id: Foot = 0, Bicycle = 1, Ski = 2.
    pub factor: [[f64; 4]; 3],
}

impl SurfacePaceContributor {
    /// Build the lookup table from config. The per-profile rows are
    /// laid out by `fkb_type` code: `[unknown, sti, vei, skiloype]`.
    pub fn from_config(cfg: &crate::config::SurfacePaceConfig) -> Self {
        let row = |p: &crate::config::SurfacePaceProfile| [p.unknown, p.sti, p.vei, p.skiloype];
        Self {
            factor: [row(&cfg.foot), row(&cfg.bicycle), row(&cfg.ski)],
        }
    }
}

impl CostContributor for SurfacePaceContributor {
    fn name(&self) -> &'static str {
        "surface_pace"
    }
    fn kind(&self) -> ContributorKind {
        ContributorKind::Surface
    }
    fn contribute(&self, _ctx: &EdgeContext<'_>) -> f64 {
        0.0
    }
    fn pace_factor(&self, ctx: &EdgeContext<'_>) -> f64 {
        match ctx.kind {
            EdgeKind::Graph(er) => {
                let p = match ctx.profile {
                    Profile::Foot => 0,
                    Profile::Bicycle => 1,
                    Profile::Ski => 2,
                };
                self.factor[p][(er.fkb_type as usize).min(3)]
            }
            EdgeKind::Mesh => 1.0,
        }
    }
}

/// Hard refusal at water + glacier cells. Mirrors
/// [`crate::layers::MaskRefusalLayer`]; native because there's
/// nothing to convert — refusal is binary, walk-seconds are
/// irrelevant, the contributor reports zero and vetoes when the
/// mask says so.
pub struct MaskRefusalContributor {
    pub mask: Arc<Mask>,
    /// When `true`, water cells are NOT vetoed by this contributor —
    /// the vector water-integral layer handles them with finite
    /// crossing-length cost instead. Matches
    /// `MaskRefusalLayer::deferring_water`.
    pub defer_water_to_vector: bool,
    /// Extra walk-seconds per metre for traversing a *passable*
    /// (shoreline) water cell. The deep-water interior stays a hard
    /// veto; the shoreline ring is finite-but-expensive so the
    /// off-trail geodesic can hug a shore like the marked trail
    /// without shortcutting across open water.
    pub water_cost_s_per_m: f64,
    /// Ring radius (m) used to classify a water cell as shoreline
    /// (passable) vs deep interior (refused).
    pub water_shore_band_m: f64,
}

/// Classification of a water cell for the continuous-water model.
#[derive(PartialEq)]
enum WaterState {
    /// Not water (or out of mask) — handled by other refusal kinds.
    NotWater,
    /// Water with non-water within `shore_band_m` — passable, costly.
    Shoreline,
    /// Water surrounded by water out to `shore_band_m` — impassable.
    Deep,
}

impl MaskRefusalContributor {
    pub fn new(mask: Arc<Mask>) -> Self {
        let d = crate::config::WaterConfig::default();
        Self {
            mask,
            defer_water_to_vector: false,
            water_cost_s_per_m: d.cost_s_per_m,
            water_shore_band_m: d.shore_band_m,
        }
    }
    pub fn deferring_water(mut self) -> Self {
        self.defer_water_to_vector = true;
        self
    }
    pub fn with_water(mut self, cost_s_per_m: f64, shore_band_m: f64) -> Self {
        self.water_cost_s_per_m = cost_s_per_m;
        self.water_shore_band_m = shore_band_m;
        self
    }

    /// Classify the water at `(x, y)`: shoreline if any non-water cell
    /// lies within `water_shore_band_m`, else deep. We scan 8 spokes at
    /// several radii (not a single ring) — sampling only the band radius
    /// overshoots a thin shoreline strip and lands back in water on the
    /// far side, misclassifying near-shore cells as deep. Step ≈ half a
    /// water-raster cell (12.5 m) so a shore puffed inward by ~1 cell is
    /// always caught. Cheap: ~8×N mmap point lookups.
    fn water_state(&self, x: f64, y: f64) -> WaterState {
        if !matches!(self.mask.refused(x, y), Ok(RefusalKind::Water)) {
            return WaterState::NotWater;
        }
        use std::f64::consts::FRAC_1_SQRT_2 as D;
        const DIRS: [(f64, f64); 8] = [
            (1.0, 0.0),
            (-1.0, 0.0),
            (0.0, 1.0),
            (0.0, -1.0),
            (D, D),
            (-D, D),
            (D, -D),
            (-D, -D),
        ];
        // Hole-robust shore test: a cell is "shoreline" (passable) only if
        // genuine land lies within the band in some direction. We sample
        // each of the 8 directions at the band-distance RING endpoint, not
        // along the whole spoke. The old "any non-water sample anywhere
        // within the band" test was fragile: the 25 m water raster has
        // interior holes (cells not flagged water inside a large lake), so a
        // mid-lake cell would "find shore" at a hole and be wrongly marked
        // passable — letting routes cut straight across large water bodies.
        // Checking only the ring endpoint ignores those interior holes: a
        // truly-deep cell is surrounded by water at the band ring in every
        // direction, while a near-shore cell reaches land in at least one.
        let band = self.water_shore_band_m;
        for (dx, dy) in DIRS {
            if !matches!(
                self.mask.refused(x + dx * band, y + dy * band),
                Ok(RefusalKind::Water)
            ) {
                return WaterState::Shoreline;
            }
        }
        WaterState::Deep
    }
}

impl CostContributor for MaskRefusalContributor {
    fn name(&self) -> &'static str {
        "mask_refusal"
    }
    fn kind(&self) -> ContributorKind {
        ContributorKind::Hazard
    }
    fn contribute(&self, ctx: &EdgeContext<'_>) -> f64 {
        if self.defer_water_to_vector {
            return 0.0;
        }
        let mid_x = 0.5 * (ctx.fx + ctx.tx);
        let mid_y = 0.5 * (ctx.fy + ctx.ty);
        // Passable shoreline water → finite high cost. (Deep water is
        // vetoed in `veto`, so it never reaches the cost stack.)
        if self.water_state(mid_x, mid_y) == WaterState::Shoreline {
            self.water_cost_s_per_m * ctx.length_m
        } else {
            0.0
        }
    }
    fn veto(&self, ctx: &EdgeContext<'_>) -> Option<&'static str> {
        let mid_x = 0.5 * (ctx.fx + ctx.tx);
        let mid_y = 0.5 * (ctx.fy + ctx.ty);
        match self.mask.refused(mid_x, mid_y).ok()? {
            RefusalKind::Water if self.defer_water_to_vector => None,
            // Continuous water: refuse only the deep interior; the
            // shoreline ring is passable (high cost via `contribute`).
            RefusalKind::Water => match self.water_state(mid_x, mid_y) {
                WaterState::Deep => Some("water"),
                _ => None,
            },
            RefusalKind::Glacier => Some("glacier"),
            RefusalKind::Reserved3 => Some("restricted"),
            _ => None,
        }
    }
}

/// Penalty for edges that traverse terrain transverse to the
/// contour lines — i.e., cut across a slope instead of following
/// it. Tobler + Naismith both charge by *net* elevation change
/// between endpoints, so a 500 m segment that climbs a ridge and
/// then descends to the same elevation at the other end pays the
/// same as 500 m along a level contour. Real hiker preference
/// strongly disagrees: cutting across a steep wedge is harder
/// than walking the contour around it.
///
/// Algorithm:
///   1. Sample DEM at N+1 points along the edge.
///   2. Build the linear interpolation of elevation from endpoint
///      to endpoint (`z_lin[i] = z[0] + i/N × (z[N] - z[0])`).
///   3. Compute the RMS deviation `|z[i] - z_lin[i]|` over the
///      interior samples — this is the "how much does the actual
///      terrain bulge from a straight line between endpoints?"
///      signal. A contour-following edge has tiny deviation
///      because every sample's elevation is close to the endpoint
///      interpolation. A cross-ridge edge has large deviation
///      because the middle samples sit hundreds of metres above
///      the linear interpolation.
///   4. Charge `k × rms × length / base_pace` extra walk-seconds.
///      Default `k = 0.4` gives a 100 m RMS deviation over a 1 km
///      edge ~57 walk-seconds extra; a 5 m deviation is ~3 s.
///
/// Mesh-edge only; graph edges already follow their baked
/// polyline so the contour-following property is implicit.
pub struct ContourCrossingContributor {
    pub dem: Arc<Dem>,
    pub k: f64,
    pub sample_step_m: f64,
}

impl ContourCrossingContributor {
    pub fn new(dem: Arc<Dem>) -> Self {
        Self {
            dem,
            k: 0.4,
            sample_step_m: 6.0,
        }
    }
}

impl CostContributor for ContourCrossingContributor {
    fn name(&self) -> &'static str {
        "contour_crossing"
    }
    fn kind(&self) -> ContributorKind {
        ContributorKind::Slope
    }
    fn contribute(&self, ctx: &EdgeContext<'_>) -> f64 {
        if !matches!(ctx.kind, EdgeKind::Mesh) {
            return 0.0;
        }
        if ctx.length_m < 1.0 {
            return 0.0;
        }
        let n = ((ctx.length_m / self.sample_step_m).round() as usize).clamp(4, 64);
        let probe_zs = ctx.elev_probe.map(|p| p.elevations(n));
        let direct_zs: Vec<Option<f32>>;
        let zs: &[Option<f32>] = match &probe_zs {
            Some(rc) => rc.as_ref(),
            None => {
                let dx = ctx.tx - ctx.fx;
                let dy = ctx.ty - ctx.fy;
                let mut v: Vec<Option<f32>> = Vec::with_capacity(n + 1);
                for i in 0..=n {
                    let t = i as f64 / n as f64;
                    let x = ctx.fx + dx * t;
                    let y = ctx.fy + dy * t;
                    v.push(self.dem.sample(PointXY { x, y }).ok().flatten());
                }
                direct_zs = v;
                &direct_zs
            }
        };
        // Need endpoint elevations to define the linear interp.
        let (Some(z0), Some(z_n)) = (zs.first().and_then(|z| *z), zs.last().and_then(|z| *z))
        else {
            return 0.0;
        };
        let mut sum_sq = 0.0f64;
        let mut count = 0usize;
        for (i, z) in zs.iter().enumerate().skip(1).take(n.saturating_sub(1)) {
            let Some(zv) = z else { continue };
            let t = i as f64 / n as f64;
            let z_lin = z0 as f64 + t * (z_n - z0) as f64;
            let dev = *zv as f64 - z_lin;
            sum_sq += dev * dev;
            count += 1;
        }
        if count == 0 {
            return 0.0;
        }
        let rms = (sum_sq / count as f64).sqrt();
        self.k * rms * ctx.length_m * BASE_PACE_S_PER_M
    }
}

/// Penalty for routing through cells the DEM has no data for.
///
/// The Norway DEM has ~6 k absent tiles (mountainous Jotunheimen,
/// some coastal margins). When [`ToblerSlopeContributor`] and
/// [`NaismithGainContributor`] hit nodata they silently report 0
/// extra walk-seconds, which gives the solver no reason to prefer
/// known-easy terrain over unknown terrain. Visually this shows up
/// as straight-line LoS jumps across DEM holes regardless of what
/// they actually contain.
///
/// This contributor adds an explicit penalty per metre of edge
/// that lacks DEM coverage. The default `delta_s_per_m_missing =
/// 2.0` makes one metre of unknown terrain cost ~3.5× a flat-trail
/// metre — high enough that the solver will detour around a known
/// DEM hole when an alternative exists, low enough that it'll
/// still cross small gaps when there's no choice. Mesh-edge only;
/// graph edges have baked attributes from the build phase so DEM
/// coverage isn't load-bearing for them.
pub struct DemCoveragePenaltyContributor {
    pub dem: Arc<Dem>,
    pub delta_s_per_m_missing: f64,
    pub sample_step_m: f64,
}

impl DemCoveragePenaltyContributor {
    pub fn new(dem: Arc<Dem>) -> Self {
        Self {
            dem,
            delta_s_per_m_missing: 2.0,
            sample_step_m: 6.0,
        }
    }
}

impl CostContributor for DemCoveragePenaltyContributor {
    fn name(&self) -> &'static str {
        "dem_coverage"
    }
    fn kind(&self) -> ContributorKind {
        ContributorKind::Hazard
    }
    fn contribute(&self, ctx: &EdgeContext<'_>) -> f64 {
        if !matches!(ctx.kind, EdgeKind::Mesh) {
            return 0.0;
        }
        if ctx.length_m < 1.0 {
            return 0.0;
        }
        // Reuse Tobler's sampler so we share the same step density
        // and lookup cost across the slope-side contributors (and the
        // shared `ctx.elev_probe` when present — one DEM pass for the
        // whole stack). The returned fraction scales the per-metre
        // penalty linearly: an edge that's 30% nodata pays 30% of the
        // full penalty across its full length.
        let sampler = ToblerSlopeContributor {
            dem: self.dem.clone(),
            refuse_above_deg: 90.0,
            sample_step_m: self.sample_step_m,
        };
        let missing_frac = sampler.sample_missing_fraction(ctx);
        if missing_frac <= 0.0 {
            return 0.0;
        }
        self.delta_s_per_m_missing * missing_frac * ctx.length_m
    }
}

/// Naismith-style vertical-gain penalty for off-trail mesh edges.
///
/// Tobler's hiking function alone under-penalises moderate climbs.
/// A 10° ascent costs ~1.3× pace, but a real hiker climbing 250 m
/// over 1.5 km pays *time*, not just pace × distance — Naismith's
/// rule of thumb adds roughly one hour per 600 m of vertical gain
/// on top of the horizontal walking time. The graph builder bakes
/// the equivalent term into per-edge cost (`length_m + 8 × gain_m`
/// for foot), but mesh edges previously only saw Tobler, so a long
/// LoS jump straight up a mountainside looked cheaper than the
/// switchback alternative even though every real hiker would take
/// the long way.
///
/// This contributor integrates *positive* elevation change along
/// the edge (descents don't refund time the way the model used to
/// implicitly assume), multiplies by a per-profile gain weight,
/// and converts to walk-seconds via the base pace. Same `k` values
/// the graph builder uses, so graph and mesh edges price gain
/// consistently:
///
///   foot:    8 effective metres per gain metre
///   bicycle: 20 effective metres per gain metre
///   ski:     6 effective metres per gain metre
///
/// Mesh-edge only. Graph edges already carry baked gain via
/// `profile_cost` so this contributor returns 0 for `EdgeKind::Graph`
/// to avoid double-counting.
pub struct NaismithGainContributor {
    pub dem: Arc<Dem>,
    pub sample_step_m: f64,
}

impl NaismithGainContributor {
    pub fn new(dem: Arc<Dem>) -> Self {
        Self {
            dem,
            sample_step_m: 6.0,
        }
    }

    fn sample_count(&self, length_m: f64) -> usize {
        ((length_m / self.sample_step_m).round() as usize).clamp(2, 64)
    }

    fn sample_gain(&self, ctx: &EdgeContext<'_>) -> f64 {
        let n = self.sample_count(ctx.length_m);
        // Positive-gain integral over the sample sequence; `None`
        // (nodata) samples are skipped without breaking the chain.
        let integrate = |samples: &mut dyn Iterator<Item = Option<f32>>| -> f64 {
            let mut last_z: Option<f32> = None;
            let mut gain: f64 = 0.0;
            for z in samples {
                let Some(z) = z else { continue };
                if let Some(prev) = last_z {
                    let dz = z - prev;
                    if dz > 0.0 {
                        gain += dz as f64;
                    }
                }
                last_z = Some(z);
            }
            gain
        };
        if let Some(p) = ctx.elev_probe {
            return integrate(&mut p.elevations(n).iter().copied());
        }
        let dx = ctx.tx - ctx.fx;
        let dy = ctx.ty - ctx.fy;
        integrate(&mut (0..=n).map(|i| {
            let t = i as f64 / n as f64;
            let x = ctx.fx + dx * t;
            let y = ctx.fy + dy * t;
            self.dem.sample(PointXY { x, y }).ok().flatten()
        }))
    }

    fn k_for(profile: Profile) -> f64 {
        match profile {
            Profile::Foot => 8.0,
            Profile::Bicycle => 20.0,
            Profile::Ski => 6.0,
        }
    }
}

impl CostContributor for NaismithGainContributor {
    fn name(&self) -> &'static str {
        "naismith_gain"
    }
    fn kind(&self) -> ContributorKind {
        ContributorKind::Slope
    }
    fn contribute(&self, ctx: &EdgeContext<'_>) -> f64 {
        if !matches!(ctx.kind, EdgeKind::Mesh) {
            return 0.0;
        }
        let gain = self.sample_gain(ctx);
        if gain <= 0.0 {
            return 0.0;
        }
        Self::k_for(ctx.profile) * gain * BASE_PACE_S_PER_M
    }
}

/// Direction-aware slope using Tobler's hiking function with the
/// signed slope component along the edge direction. Mesh-edge only:
/// graph edges already aggregate a `slope_max_deg` baked attribute
/// that [`GraphSlopeContributor`] handles.
///
/// Walk-seconds per metre delta is computed from Tobler in m/s,
/// minus the flat-pace baseline. Clamped so a one-pixel DEM cliff
/// can't dominate route cost.
pub struct DirectionalSlopeContributor {
    pub dem: Arc<Dem>,
    pub min_relevant_slope_deg: f32,
    pub max_delta_s_per_m: f64,
}

impl DirectionalSlopeContributor {
    pub fn new(dem: Arc<Dem>) -> Self {
        Self {
            dem,
            min_relevant_slope_deg: 3.0,
            // Hard cap on per-metre cost so a single-cell DEM artefact
            // can't push a 1 km edge cost to thousands of seconds.
            max_delta_s_per_m: 5.0,
        }
    }
}

impl CostContributor for DirectionalSlopeContributor {
    fn name(&self) -> &'static str {
        "slope_direction"
    }
    fn kind(&self) -> ContributorKind {
        ContributorKind::Slope
    }
    fn contribute(&self, ctx: &EdgeContext<'_>) -> f64 {
        if ctx.length_m < 1e-3 {
            return 0.0;
        }
        let mid_x = 0.5 * (ctx.fx + ctx.tx);
        let mid_y = 0.5 * (ctx.fy + ctx.ty);
        let sa = match self.dem.slope_aspect(PointXY { x: mid_x, y: mid_y }) {
            Ok(Some(s)) => s,
            _ => return 0.0,
        };
        if sa.slope_deg < self.min_relevant_slope_deg {
            return 0.0;
        }
        let edge_dx = (ctx.tx - ctx.fx) as f32;
        let edge_dy = (ctx.ty - ctx.fy) as f32;
        let edge_len = ctx.length_m as f32;
        // Aspect (degrees clockwise from north) → downhill unit vec.
        let aspect_rad = sa.aspect_deg.to_radians();
        let downhill_x = aspect_rad.sin();
        let downhill_y = -aspect_rad.cos();
        let edge_ux = edge_dx / edge_len;
        let edge_uy = edge_dy / edge_len;
        let dot = edge_ux * downhill_x + edge_uy * downhill_y;
        // Signed slope as tangent (positive = uphill). Capped so a
        // 90° cliff returns finite seconds.
        let signed_slope = -dot * sa.slope_deg.to_radians().tan();
        // Tobler in m/s; convert to s/m and subtract flat baseline.
        let s_offset = (signed_slope + 0.05).abs();
        let v = 1.6667 * (-3.5 * s_offset as f64).exp();
        if v <= 1e-6 {
            return self.max_delta_s_per_m * ctx.length_m;
        }
        let delta = 1.0 / v - BASE_PACE_S_PER_M;
        delta.clamp(-self.max_delta_s_per_m, self.max_delta_s_per_m) * ctx.length_m
    }
}

/// Avalanche-terrain hazard for the `ski` profile. Translates the
/// legacy ATES-shaped multiplier into walk-seconds delta against the
/// same Tobler baseline. No-op for foot/bicycle. Doesn't veto —
/// curators willing to traverse 30–45° terrain disable the
/// contributor via `layer_weights["avalanche_terrain"] = 0`.
pub struct AvalancheTerrainContributor {
    pub dem: Arc<Dem>,
    pub slope_min_deg: f32,
    pub slope_max_deg: f32,
    pub treeline_m: f32,
    /// Peak walk-seconds-per-metre delta in the danger band on a lee
    /// aspect above treeline. Default 1.78 s/m corresponds to the
    /// legacy `base_multiplier = 3.5` at the flat-trail pace
    /// (3.5 - 1.0) × BASE_PACE_S_PER_M ≈ 1.79.
    pub peak_delta_s_per_m: f64,
}

impl AvalancheTerrainContributor {
    pub fn new(dem: Arc<Dem>) -> Self {
        Self {
            dem,
            slope_min_deg: 30.0,
            slope_max_deg: 45.0,
            treeline_m: 800.0,
            peak_delta_s_per_m: (3.5 - 1.0) * BASE_PACE_S_PER_M,
        }
    }

    fn risk_at(&self, x: f64, y: f64, profile: Profile) -> f64 {
        if !matches!(profile, Profile::Ski) {
            return 0.0;
        }
        let sa = match self.dem.slope_aspect(PointXY { x, y }) {
            Ok(Some(s)) => s,
            _ => return 0.0,
        };
        if sa.slope_deg < self.slope_min_deg || sa.slope_deg > self.slope_max_deg {
            return 0.0;
        }
        let lee_delta = ((sa.aspect_deg - 45.0).to_radians()).cos();
        let lee_factor = ((lee_delta + 1.0) * 0.5) as f64;
        let centre = (self.slope_min_deg + self.slope_max_deg) * 0.5;
        let half = (self.slope_max_deg - self.slope_min_deg) * 0.5;
        let band_dist = (sa.slope_deg - centre).abs() / half;
        let slope_factor = (1.0 - band_dist).max(0.0) as f64;
        let elev = match self.dem.sample(PointXY { x, y }) {
            Ok(Some(e)) => e,
            _ => return 0.0,
        };
        let elev_factor = (((elev - (self.treeline_m - 200.0)) / 200.0).clamp(0.0, 1.0)) as f64;
        lee_factor * slope_factor * elev_factor
    }
}

impl CostContributor for AvalancheTerrainContributor {
    fn name(&self) -> &'static str {
        "avalanche_terrain"
    }
    fn kind(&self) -> ContributorKind {
        ContributorKind::Hazard
    }
    fn contribute(&self, ctx: &EdgeContext<'_>) -> f64 {
        let mid_x = 0.5 * (ctx.fx + ctx.tx);
        let mid_y = 0.5 * (ctx.fy + ctx.ty);
        let risk = self.risk_at(mid_x, mid_y, ctx.profile);
        if risk <= 0.0 {
            return 0.0;
        }
        self.peak_delta_s_per_m * risk * ctx.length_m
    }
}

/// Single-class landcover mask layer. When the mask reports the
/// class present at the edge midpoint, adds `delta_s_per_m * length`
/// to the walk-seconds. `delta_s_per_m = INFINITY` collapses to a
/// veto with the layer's label.
pub struct LandcoverContributor {
    pub mask: Arc<Mask>,
    pub name: &'static str,
    pub delta_s_per_m: f64,
}

impl LandcoverContributor {
    pub fn new(mask: Arc<Mask>, name: &'static str, delta_s_per_m: f64) -> Self {
        Self {
            mask,
            name,
            delta_s_per_m,
        }
    }

    fn present_at_midpoint(&self, ctx: &EdgeContext<'_>) -> bool {
        let mid_x = 0.5 * (ctx.fx + ctx.tx);
        let mid_y = 0.5 * (ctx.fy + ctx.ty);
        matches!(self.mask.refused(mid_x, mid_y), Ok(RefusalKind::Water))
    }
}

impl CostContributor for LandcoverContributor {
    fn name(&self) -> &'static str {
        self.name
    }
    fn kind(&self) -> ContributorKind {
        ContributorKind::Vegetation
    }
    fn contribute(&self, ctx: &EdgeContext<'_>) -> f64 {
        if !self.delta_s_per_m.is_finite() {
            return 0.0;
        }
        if self.present_at_midpoint(ctx) {
            self.delta_s_per_m * ctx.length_m
        } else {
            0.0
        }
    }
    fn veto(&self, ctx: &EdgeContext<'_>) -> Option<&'static str> {
        if self.delta_s_per_m.is_infinite() && self.present_at_midpoint(ctx) {
            Some(self.name)
        } else {
            None
        }
    }
}

/// Per-edge bonus for "preferred" graph edges (DNT-maintained,
/// curator-edited). Mesh edges contribute zero. The legacy
/// multiplier `0.5` on DNT maps to a -50% walk-seconds-per-metre
/// delta against the flat-trail baseline.
pub struct PreferredEdgeContributor {
    pub seconds_per_metre: [f64; 8],
}

impl Default for PreferredEdgeContributor {
    fn default() -> Self {
        let mut tbl = [0.0f64; 8];
        // A MAINTAINED trail (DNT / manually curated) is modestly
        // easier underfoot than a faint unmaintained path — a footing
        // nudge, not a routing dominator. The dominant on-trail vs
        // off-trail advantage now comes from the off-trail roughness
        // factor (~2.3×) that trail edges don't pay; the old -50% value
        // was calibrated for the legacy `baked × multiplier` weight
        // (which squared the discount) and, under honest walk-seconds,
        // let marked trails win multi-× detours. -15% keeps maintenance
        // a preference, matching the red-T marking magnitude.
        tbl[3] = -0.15 * BASE_PACE_S_PER_M; // dnt
        tbl[4] = -0.15 * BASE_PACE_S_PER_M; // manual
        Self {
            seconds_per_metre: tbl,
        }
    }
}

impl CostContributor for PreferredEdgeContributor {
    fn name(&self) -> &'static str {
        "preferred_edge"
    }
    fn kind(&self) -> ContributorKind {
        ContributorKind::Marking
    }
    fn contribute(&self, ctx: &EdgeContext<'_>) -> f64 {
        match ctx.kind {
            EdgeKind::Graph(er) => {
                let idx = (er.source as usize).min(7);
                self.seconds_per_metre[idx] * ctx.length_m
            }
            EdgeKind::Mesh => 0.0,
        }
    }
}

/// Off-trail roughness as a first-class **multiplicative** contributor.
///
/// Rough, untracked ground makes every metre — including the climb —
/// proportionally harder, so this scales the whole composed pace by
/// `factor` (≈2.3 for foot) on MESH edges, and is a no-op on GRAPH
/// (trail) edges. This replaces the `off_trail_factor` that used to be
/// hard-coded into the solver's cost (`tobler × off × mul`), moving it
/// into the same composable cost stack as everything else — so nothing
/// geographic lives in the solver and the factor is tuned like any
/// other layer. It contributes no additive walk-seconds; its effect is
/// entirely via [`CostContributor::pace_factor`].
pub struct OffTrailRoughnessContributor {
    pub factor: f64,
}

impl OffTrailRoughnessContributor {
    pub fn new(factor: f32) -> Self {
        Self {
            factor: factor as f64,
        }
    }
}

impl CostContributor for OffTrailRoughnessContributor {
    fn name(&self) -> &'static str {
        "off_trail_roughness"
    }
    fn kind(&self) -> ContributorKind {
        ContributorKind::Surface
    }
    fn contribute(&self, _ctx: &EdgeContext<'_>) -> f64 {
        0.0
    }
    fn pace_factor(&self, ctx: &EdgeContext<'_>) -> f64 {
        match ctx.kind {
            EdgeKind::Mesh => self.factor,
            EdgeKind::Graph(_) => 1.0,
        }
    }
}

/// Per-edge graph slope from the baked `slope_max_deg`. Quadratic
/// curve, identical shape to the legacy multiplier model but
/// expressed in walk-seconds delta:
///
/// ```text
///   slope <= 5°   → 0 contribution
///   slope < refuse → delta_per_m = (slope / k)² × BASE_PACE
///   slope >= refuse → veto with label "slope_too_steep"
/// ```
pub struct GraphSlopeContributor {
    pub quadratic_scale_deg: f32,
    pub refuse_above_deg: f32,
}

impl Default for GraphSlopeContributor {
    fn default() -> Self {
        Self {
            quadratic_scale_deg: 15.0,
            refuse_above_deg: 50.0,
        }
    }
}

impl CostContributor for GraphSlopeContributor {
    fn name(&self) -> &'static str {
        "graph_slope"
    }
    fn kind(&self) -> ContributorKind {
        ContributorKind::Slope
    }
    fn contribute(&self, ctx: &EdgeContext<'_>) -> f64 {
        match ctx.kind {
            EdgeKind::Graph(er) => {
                let s = er.slope_max_deg;
                if s <= 5.0 || s >= self.refuse_above_deg {
                    return 0.0;
                }
                let t = (s / self.quadratic_scale_deg) as f64;
                (t * t) * BASE_PACE_S_PER_M * ctx.length_m
            }
            EdgeKind::Mesh => 0.0,
        }
    }
    fn veto(&self, ctx: &EdgeContext<'_>) -> Option<&'static str> {
        if let EdgeKind::Graph(er) = ctx.kind {
            if er.slope_max_deg >= self.refuse_above_deg {
                return Some("slope_too_steep");
            }
        }
        None
    }
}

/// Naismith-style total-gain amplifier on graph edges. The baked
/// `profile_cost` already adds `k × gain_m` per profile; this
/// contributor lets the curator amplify or attenuate that term per
/// request without a graph rebuild. Mesh edges → 0 (graph-only).
pub struct TotalGainContributor {
    pub gain_amplifier: f32,
}

impl Default for TotalGainContributor {
    fn default() -> Self {
        Self {
            gain_amplifier: 1.0,
        }
    }
}

impl CostContributor for TotalGainContributor {
    fn name(&self) -> &'static str {
        "total_gain"
    }
    fn kind(&self) -> ContributorKind {
        ContributorKind::Slope
    }
    fn contribute(&self, ctx: &EdgeContext<'_>) -> f64 {
        if (self.gain_amplifier - 1.0).abs() < 1e-6 {
            return 0.0;
        }
        match ctx.kind {
            EdgeKind::Graph(er) => {
                let k = match ctx.profile {
                    Profile::Foot => 8.0,
                    Profile::Bicycle => 20.0,
                    Profile::Ski => 6.0,
                };
                // Legacy multiplier was 1 + (extra/length) where
                // extra = k × gain × (amp - 1). As walk-seconds
                // delta over the edge: extra × BASE_PACE_S_PER_M
                // (the Naismith term is already in length-equivalent
                // units that flatten to seconds via base pace).
                let gain = (er.gain_m as f64).max(0.0);
                let extra = k * gain * (self.gain_amplifier as f64 - 1.0);
                extra * BASE_PACE_S_PER_M
            }
            EdgeKind::Mesh => 0.0,
        }
    }
}

/// Mesh-edge bias toward known graph edges. Replaces the legacy
/// [`crate::TrailProximityLayer`]: for each profile, a per-fkb-type
/// rtree holds canonical edge segments; cells within
/// `influence_radius_m` get a negative walk-seconds contribution
/// that linearly decays to 0 at the radius. Same physical
/// magnitude as the legacy multiplicative form: `bonus_at_zero =
/// 0.95` corresponds to `-0.05 × BASE_PACE_S_PER_M × length` at
/// distance zero.
pub struct TrailProximityContributor {
    sti: RTree<TrailSegment>,
    vei: RTree<TrailSegment>,
    skiloype: RTree<TrailSegment>,
    pub influence_radius_m: f64,
    pub bonus_at_zero: f32,
}

#[derive(Clone, Copy, Debug, PartialEq)]
struct TrailSegment {
    a: [f64; 2],
    b: [f64; 2],
}

impl RTreeObject for TrailSegment {
    type Envelope = AABB<[f64; 2]>;
    fn envelope(&self) -> Self::Envelope {
        AABB::from_corners(
            [self.a[0].min(self.b[0]), self.a[1].min(self.b[1])],
            [self.a[0].max(self.b[0]), self.a[1].max(self.b[1])],
        )
    }
}

impl PointDistance for TrailSegment {
    fn distance_2(&self, point: &[f64; 2]) -> f64 {
        let px = point[0];
        let py = point[1];
        let ax = self.a[0];
        let ay = self.a[1];
        let bx = self.b[0];
        let by = self.b[1];
        let dx = bx - ax;
        let dy = by - ay;
        let len_sq = dx * dx + dy * dy;
        if len_sq < 1e-12 {
            let ex = px - ax;
            let ey = py - ay;
            return ex * ex + ey * ey;
        }
        let t = ((px - ax) * dx + (py - ay) * dy) / len_sq;
        let t = t.clamp(0.0, 1.0);
        let qx = ax + t * dx;
        let qy = ay + t * dy;
        let ex = px - qx;
        let ey = py - qy;
        ex * ex + ey * ey
    }
}

impl TrailProximityContributor {
    pub fn new(graph: &Graph, influence_radius_m: f64, bonus_at_zero: f32) -> Self {
        let build = |fkb_type: u8| -> RTree<TrailSegment> {
            let segs: Vec<TrailSegment> = graph
                .collect_segments_with_fkb_types(&[fkb_type])
                .into_iter()
                .map(|(a, b)| TrailSegment {
                    a: [a.0 as f64, a.1 as f64],
                    b: [b.0 as f64, b.1 as f64],
                })
                .collect();
            RTree::bulk_load(segs)
        };
        Self {
            sti: build(1),
            vei: build(2),
            skiloype: build(3),
            influence_radius_m,
            bonus_at_zero,
        }
    }

    fn rtree_for(&self, profile: Profile) -> &RTree<TrailSegment> {
        match profile {
            Profile::Foot => &self.sti,
            Profile::Bicycle => &self.vei,
            Profile::Ski => &self.skiloype,
        }
    }

    fn delta_at(&self, x: f64, y: f64, profile: Profile) -> f64 {
        let rt = self.rtree_for(profile);
        let Some(seg) = rt.nearest_neighbor(&[x, y]) else {
            return 0.0;
        };
        let d = seg.distance_2(&[x, y]).sqrt();
        if d >= self.influence_radius_m {
            return 0.0;
        }
        // Linear fall-off: bonus_at_zero at d=0, 0 at d=radius.
        let t = (d / self.influence_radius_m) as f32;
        let mult = self.bonus_at_zero + t * (1.0 - self.bonus_at_zero);
        ((mult as f64) - 1.0) * BASE_PACE_S_PER_M
    }
}

impl CostContributor for TrailProximityContributor {
    fn name(&self) -> &'static str {
        "trail_proximity"
    }
    fn kind(&self) -> ContributorKind {
        ContributorKind::Proximity
    }
    fn contribute(&self, ctx: &EdgeContext<'_>) -> f64 {
        match ctx.kind {
            EdgeKind::Mesh => {
                let mid_x = 0.5 * (ctx.fx + ctx.tx);
                let mid_y = 0.5 * (ctx.fy + ctx.ty);
                self.delta_at(mid_x, mid_y, ctx.profile) * ctx.length_m
            }
            EdgeKind::Graph(_) => 0.0,
        }
    }
}

/// Generic vector polygon-integral contributor. The closure returns
/// the "extra effective metres" the polygon's interior adds across
/// an AB segment; this contributor converts those to walk-seconds
/// via `× BASE_PACE_S_PER_M`. `INFINITY` from the closure collapses
/// to a veto with the layer's name as label.
pub struct PolygonIntegralContributor {
    pub name: &'static str,
    pub collection: Arc<VectorCollection>,
    pub cost_fn: Box<dyn Fn(f64, &AttrView<'_>, Profile) -> f64 + Send + Sync + 'static>,
    pub aabb_pad_m: f32,
}

impl PolygonIntegralContributor {
    pub fn new<F>(name: &'static str, collection: Arc<VectorCollection>, cost_fn: F) -> Self
    where
        F: Fn(f64, &AttrView<'_>, Profile) -> f64 + Send + Sync + 'static,
    {
        assert_eq!(collection.kind(), GeomKind::Polygon);
        Self {
            name,
            collection,
            cost_fn: Box::new(cost_fn),
            aabb_pad_m: 0.0,
        }
    }
    pub fn with_aabb_pad(mut self, m: f32) -> Self {
        self.aabb_pad_m = m;
        self
    }
    fn extra_metres(&self, ctx: &EdgeContext<'_>) -> f64 {
        use turbo_tiles_geom::segment_polygon_intersection_length;
        use turbo_tiles_geom::Point;
        let a = Point::new(ctx.fx as f32, ctx.fy as f32);
        let b = Point::new(ctx.tx as f32, ctx.ty as f32);
        let mut extra: f64 = 0.0;
        for fid in self.collection.query_segment(a, b, self.aabb_pad_m) {
            let coords = self.collection.feature_coords(fid);
            let len_in = segment_polygon_intersection_length(a, b, coords);
            if len_in < 1e-6 {
                continue;
            }
            let attrs = self.collection.feature_attrs(fid);
            let contrib = (self.cost_fn)(len_in, &attrs, ctx.profile);
            if !contrib.is_finite() {
                return f64::INFINITY;
            }
            extra += contrib;
        }
        extra
    }
}

impl CostContributor for PolygonIntegralContributor {
    fn name(&self) -> &'static str {
        self.name
    }
    fn kind(&self) -> ContributorKind {
        ContributorKind::Vegetation
    }
    fn contribute(&self, ctx: &EdgeContext<'_>) -> f64 {
        let extra = self.extra_metres(ctx);
        if !extra.is_finite() {
            return 0.0;
        }
        extra * BASE_PACE_S_PER_M
    }
    fn veto(&self, ctx: &EdgeContext<'_>) -> Option<&'static str> {
        if !self.extra_metres(ctx).is_finite() {
            Some(self.name)
        } else {
            None
        }
    }
}

/// Generic vector line-crossing contributor. `cost_fn(n_crossings,
/// attrs, profile)` returns "extra effective metres" added by the
/// crossings; converted to walk-seconds via base pace.
pub struct LineCrossingContributor {
    pub name: &'static str,
    pub collection: Arc<VectorCollection>,
    pub cost_fn: Box<dyn Fn(usize, &AttrView<'_>, Profile) -> f64 + Send + Sync + 'static>,
    pub aabb_pad_m: f32,
}

impl LineCrossingContributor {
    pub fn new<F>(name: &'static str, collection: Arc<VectorCollection>, cost_fn: F) -> Self
    where
        F: Fn(usize, &AttrView<'_>, Profile) -> f64 + Send + Sync + 'static,
    {
        assert_eq!(collection.kind(), GeomKind::LineString);
        Self {
            name,
            collection,
            cost_fn: Box::new(cost_fn),
            aabb_pad_m: 0.0,
        }
    }
    pub fn with_aabb_pad(mut self, m: f32) -> Self {
        self.aabb_pad_m = m;
        self
    }
    fn extra_metres(&self, ctx: &EdgeContext<'_>) -> f64 {
        use turbo_tiles_geom::segment_linestring_crossings;
        use turbo_tiles_geom::Point;
        let a = Point::new(ctx.fx as f32, ctx.fy as f32);
        let b = Point::new(ctx.tx as f32, ctx.ty as f32);
        let mut extra: f64 = 0.0;
        for fid in self.collection.query_segment(a, b, self.aabb_pad_m) {
            let coords = self.collection.feature_coords(fid);
            let crossings = segment_linestring_crossings(a, b, coords);
            if crossings.is_empty() {
                continue;
            }
            let attrs = self.collection.feature_attrs(fid);
            let contrib = (self.cost_fn)(crossings.len(), &attrs, ctx.profile);
            if !contrib.is_finite() {
                return f64::INFINITY;
            }
            extra += contrib;
        }
        extra
    }
}

impl CostContributor for LineCrossingContributor {
    fn name(&self) -> &'static str {
        self.name
    }
    fn kind(&self) -> ContributorKind {
        ContributorKind::Vegetation
    }
    fn contribute(&self, ctx: &EdgeContext<'_>) -> f64 {
        let extra = self.extra_metres(ctx);
        if !extra.is_finite() {
            return 0.0;
        }
        extra * BASE_PACE_S_PER_M
    }
    fn veto(&self, ctx: &EdgeContext<'_>) -> Option<&'static str> {
        if !self.extra_metres(ctx).is_finite() {
            Some(self.name)
        } else {
            None
        }
    }
}

/// Generic vector point-proximity contributor. The closure returns a
/// `multiplier` (the legacy semantics — point bonuses naturally
/// expressed multiplicatively, e.g. `0.7×` near a viewpoint). We
/// translate to walk-seconds via `(M - 1) × L × BASE_PACE`.
/// `INFINITY` from the closure becomes a veto.
pub struct PointProximityContributor {
    pub name: &'static str,
    pub collection: Arc<VectorCollection>,
    pub cost_fn: Box<dyn Fn(f64, &AttrView<'_>, Profile) -> f64 + Send + Sync + 'static>,
    pub influence_radius_m: f32,
}

impl PointProximityContributor {
    pub fn new<F>(
        name: &'static str,
        collection: Arc<VectorCollection>,
        influence_radius_m: f32,
        cost_fn: F,
    ) -> Self
    where
        F: Fn(f64, &AttrView<'_>, Profile) -> f64 + Send + Sync + 'static,
    {
        assert_eq!(collection.kind(), GeomKind::Point);
        Self {
            name,
            collection,
            cost_fn: Box::new(cost_fn),
            influence_radius_m,
        }
    }

    fn nearest_multiplier(&self, x: f64, y: f64, profile: Profile) -> f64 {
        use turbo_tiles_geom::Point;
        let p = Point::new(x as f32, y as f32);
        let r2 = (self.influence_radius_m as f64).powi(2);
        let mut best: Option<(f64, u32)> = None;
        for fid in self.collection.query_point(p, self.influence_radius_m) {
            let pts = self.collection.feature_coords(fid);
            if pts.is_empty() {
                continue;
            }
            let q = pts[0];
            let dx = (q.x - p.x) as f64;
            let dy = (q.y - p.y) as f64;
            let d2 = dx * dx + dy * dy;
            if d2 > r2 {
                continue;
            }
            match best {
                Some((bd, _)) if bd <= d2 => {}
                _ => best = Some((d2, fid)),
            }
        }
        let Some((d2, fid)) = best else {
            return 1.0;
        };
        let attrs = self.collection.feature_attrs(fid);
        (self.cost_fn)(d2.sqrt(), &attrs, profile)
    }
}

impl CostContributor for PointProximityContributor {
    fn name(&self) -> &'static str {
        self.name
    }
    fn kind(&self) -> ContributorKind {
        ContributorKind::Proximity
    }
    fn contribute(&self, ctx: &EdgeContext<'_>) -> f64 {
        let mid_x = 0.5 * (ctx.fx + ctx.tx);
        let mid_y = 0.5 * (ctx.fy + ctx.ty);
        let mult = self.nearest_multiplier(mid_x, mid_y, ctx.profile);
        if !mult.is_finite() {
            return 0.0;
        }
        (mult - 1.0) * BASE_PACE_S_PER_M * ctx.length_m
    }
    fn veto(&self, ctx: &EdgeContext<'_>) -> Option<&'static str> {
        let mid_x = 0.5 * (ctx.fx + ctx.tx);
        let mid_y = 0.5 * (ctx.fy + ctx.ty);
        if !self
            .nearest_multiplier(mid_x, mid_y, ctx.profile)
            .is_finite()
        {
            Some(self.name)
        } else {
            None
        }
    }
}

/// Vector polygon refusal. Mirrors the legacy
/// [`crate::PolygonRefusalLayer`]: veto when ANY segment from→to
/// overlaps a polygon ring, otherwise zero contribution.
pub struct PolygonRefusalContributor {
    pub name: &'static str,
    pub collection: Arc<VectorCollection>,
    pub label: &'static str,
}

impl PolygonRefusalContributor {
    pub fn new(name: &'static str, collection: Arc<VectorCollection>, label: &'static str) -> Self {
        assert_eq!(collection.kind(), GeomKind::Polygon);
        Self {
            name,
            collection,
            label,
        }
    }
    fn intersects_segment(&self, ctx: &EdgeContext<'_>) -> bool {
        use turbo_tiles_geom::segment_polygon_intersection_length;
        use turbo_tiles_geom::Point;
        let a = Point::new(ctx.fx as f32, ctx.fy as f32);
        let b = Point::new(ctx.tx as f32, ctx.ty as f32);
        for fid in self.collection.query_segment(a, b, 0.0) {
            let ring = self.collection.feature_coords(fid);
            if segment_polygon_intersection_length(a, b, ring) > 0.0 {
                return true;
            }
        }
        false
    }
}

impl CostContributor for PolygonRefusalContributor {
    fn name(&self) -> &'static str {
        self.name
    }
    fn kind(&self) -> ContributorKind {
        ContributorKind::Hazard
    }
    fn contribute(&self, _ctx: &EdgeContext<'_>) -> f64 {
        0.0
    }
    fn veto(&self, ctx: &EdgeContext<'_>) -> Option<&'static str> {
        if self.intersects_segment(ctx) {
            Some(self.label)
        } else {
            None
        }
    }
}

/// Names of legacy layers that have a native replacement above.
/// All shipped legacy layers now have native CostContributor
/// equivalents — the breakdown / solver paths never need
/// [`crate::contributor::LegacyLayerAdapter`] for production
/// queries. The legacy trait remains for downstream code that
/// hasn't migrated yet; once nothing depends on it we delete it.
pub const DISPLACED_LEGACY_LAYERS: &[&str] = &[
    "slope",
    "marking",
    "mask_refusal",
    "slope_direction",
    "avalanche_terrain",
    "preferred_edge",
    "graph_slope",
    "total_gain",
    "trail_proximity",
    "wetland",
    "forest",
    "open",
    "cultivated",
    "developed",
    "building",
    "stream_barrier",
    "bridge_zone",
    "water",
    "water_polygon",
    "stream_barrier_lines",
    "tarn",
];

/// True if `legacy_name` has a registered native replacement.
pub fn has_native_replacement(legacy_name: &str) -> bool {
    DISPLACED_LEGACY_LAYERS.contains(&legacy_name)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::contributor::{compose_edge_walk_seconds, EdgeContext, EdgeKind};
    use turbo_tiles_graph::Profile;

    fn mesh_ctx(length_m: f64) -> EdgeContext<'static> {
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
    fn tobler_flat_slope_adds_zero_or_small_bonus() {
        // At 0° Tobler's curve is slightly cheaper than the
        // flat-trail baseline (it bakes in a small downhill
        // preference at slope=-2.86°). The contribution should
        // be at most a few % of base pace.
        let d = ToblerSlopeContributor::delta_seconds_per_metre(0.0);
        assert!(d.abs() < 0.15);
    }

    #[test]
    fn tobler_thirty_deg_adds_substantial_cost() {
        // 30° is a real climb; expect a multi-second-per-metre
        // delta. Use a coarse bound — the literature varies but
        // any sane Tobler impl returns at least +1 s/m here.
        let d = ToblerSlopeContributor::delta_seconds_per_metre(30.0);
        assert!(d > 1.0, "expected >1 s/m at 30°, got {d}");
    }

    #[test]
    fn tobler_monotone_increasing_above_inflection() {
        // From ~5° upward, more slope = more time per metre.
        let a = ToblerSlopeContributor::delta_seconds_per_metre(5.0);
        let b = ToblerSlopeContributor::delta_seconds_per_metre(15.0);
        let c = ToblerSlopeContributor::delta_seconds_per_metre(25.0);
        assert!(a < b);
        assert!(b < c);
    }

    #[test]
    fn marking_red_t_is_negative_contribution() {
        use turbo_tiles_graph::EdgeRecord;
        let layer = MarkingBonusContributor::default();
        let mut er: EdgeRecord = unsafe { std::mem::zeroed() };
        er.marking = 1; // red T
        let ctx = EdgeContext {
            fx: 0.0,
            fy: 0.0,
            tx: 1000.0,
            ty: 0.0,
            length_m: 1000.0,
            profile: Profile::Foot,
            kind: EdgeKind::Graph(&er),
            elev_probe: None,
        };
        let c = layer.contribute(&ctx);
        assert!(c < 0.0, "red-T should be a bonus, got {c}");
        // Magnitude: ~15% of base pace × 1000 m = ~107 s/km.
        assert!(c.abs() > 90.0 && c.abs() < 130.0);
    }

    #[test]
    fn marking_unmarked_is_positive_contribution() {
        use turbo_tiles_graph::EdgeRecord;
        let layer = MarkingBonusContributor::default();
        let mut er: EdgeRecord = unsafe { std::mem::zeroed() };
        er.marking = 4; // unmarked
        let ctx = EdgeContext {
            fx: 0.0,
            fy: 0.0,
            tx: 1000.0,
            ty: 0.0,
            length_m: 1000.0,
            profile: Profile::Foot,
            kind: EdgeKind::Graph(&er),
            elev_probe: None,
        };
        let c = layer.contribute(&ctx);
        assert!(c > 0.0);
    }

    #[test]
    fn marking_mesh_edge_contributes_zero() {
        let layer = MarkingBonusContributor::default();
        let c = layer.contribute(&mesh_ctx(100.0));
        assert_eq!(c, 0.0);
    }

    #[test]
    fn displaced_layers_list_is_stable() {
        assert!(has_native_replacement("slope"));
        assert!(has_native_replacement("marking"));
        assert!(has_native_replacement("mask_refusal"));
        // Stage 2 finish ported the trail-proximity layer too.
        assert!(has_native_replacement("trail_proximity"));
        assert!(!has_native_replacement("not_a_real_layer"));
    }

    #[test]
    fn preferred_dnt_is_negative_contribution() {
        use turbo_tiles_graph::EdgeRecord;
        let layer = PreferredEdgeContributor::default();
        let mut er: EdgeRecord = unsafe { std::mem::zeroed() };
        er.source = 3; // dnt
        let ctx = EdgeContext {
            fx: 0.0,
            fy: 0.0,
            tx: 1000.0,
            ty: 0.0,
            length_m: 1000.0,
            profile: Profile::Foot,
            kind: EdgeKind::Graph(&er),
            elev_probe: None,
        };
        let c = layer.contribute(&ctx);
        assert!(c < 0.0, "dnt should be a bonus, got {c}");
    }

    #[test]
    fn graph_slope_vetoes_above_threshold() {
        use turbo_tiles_graph::EdgeRecord;
        let layer = GraphSlopeContributor::default();
        let mut er: EdgeRecord = unsafe { std::mem::zeroed() };
        er.slope_max_deg = 55.0; // > 50° refuse threshold
        let ctx = EdgeContext {
            fx: 0.0,
            fy: 0.0,
            tx: 100.0,
            ty: 0.0,
            length_m: 100.0,
            profile: Profile::Foot,
            kind: EdgeKind::Graph(&er),
            elev_probe: None,
        };
        assert_eq!(layer.veto(&ctx), Some("slope_too_steep"));
    }

    #[test]
    fn graph_slope_quadratic_below_threshold() {
        use turbo_tiles_graph::EdgeRecord;
        let layer = GraphSlopeContributor::default();
        let mut er: EdgeRecord = unsafe { std::mem::zeroed() };
        er.slope_max_deg = 15.0;
        let ctx = EdgeContext {
            fx: 0.0,
            fy: 0.0,
            tx: 100.0,
            ty: 0.0,
            length_m: 100.0,
            profile: Profile::Foot,
            kind: EdgeKind::Graph(&er),
            elev_probe: None,
        };
        // (15/15)² × 0.714 × 100 ≈ 71 s.
        let c = layer.contribute(&ctx);
        assert!(c > 60.0 && c < 90.0, "expected ~71s, got {c}");
        assert!(layer.veto(&ctx).is_none());
    }

    #[test]
    fn total_gain_amplifies_only_on_climb() {
        use turbo_tiles_graph::EdgeRecord;
        let layer = TotalGainContributor {
            gain_amplifier: 2.0,
        };
        let mut er: EdgeRecord = unsafe { std::mem::zeroed() };
        er.gain_m = 50.0;
        er.length_m = 1000.0;
        let ctx = EdgeContext {
            fx: 0.0,
            fy: 0.0,
            tx: 1000.0,
            ty: 0.0,
            length_m: 1000.0,
            profile: Profile::Foot,
            kind: EdgeKind::Graph(&er),
            elev_probe: None,
        };
        // k × gain × (amp-1) = 8 × 50 × 1 = 400 "extra metres".
        // In walk-seconds: 400 × 0.714 ≈ 286 s.
        let c = layer.contribute(&ctx);
        assert!(c > 250.0 && c < 320.0);
        // Flat edge → zero contribution.
        er.gain_m = 0.0;
        let ctx2 = EdgeContext {
            fx: 0.0,
            fy: 0.0,
            tx: 1000.0,
            ty: 0.0,
            length_m: 1000.0,
            profile: Profile::Foot,
            kind: EdgeKind::Graph(&er),
            elev_probe: None,
        };
        assert_eq!(layer.contribute(&ctx2), 0.0);
    }

    #[test]
    fn displaced_layers_list_covers_all_legacy() {
        for name in [
            "slope",
            "slope_direction",
            "avalanche_terrain",
            "mask_refusal",
            "preferred_edge",
            "marking",
            "graph_slope",
            "total_gain",
            "trail_proximity",
        ] {
            assert!(
                has_native_replacement(name),
                "missing native for built-in `{name}`"
            );
        }
    }

    #[test]
    fn compose_with_natives_only_sums_correctly() {
        // Composition with a single native contributor that yields
        // a known delta should add it to base pace.
        struct FixedDelta(f64);
        impl CostContributor for FixedDelta {
            fn name(&self) -> &'static str {
                "fixed"
            }
            fn kind(&self) -> ContributorKind {
                ContributorKind::Slope
            }
            fn contribute(&self, ctx: &EdgeContext<'_>) -> f64 {
                self.0 * ctx.length_m
            }
        }
        let contribs: Vec<Arc<dyn CostContributor>> = vec![Arc::new(FixedDelta(0.1))];
        let c = mesh_ctx(100.0);
        let cost = compose_edge_walk_seconds(&contribs, &c);
        assert!((cost.base_walk_seconds - 71.428).abs() < 0.01);
        assert!((cost.total_walk_seconds - 81.428).abs() < 0.01);
    }
}
