//! Built-in `CostLayer` implementations. New layers added by other
//! crates only need to implement the trait; these are the ones we
//! ship out-of-the-box.

use std::sync::Arc;

use rstar::{PointDistance, RTree, RTreeObject, AABB};
use turbo_tiles_elev::{Dem, PointXY};
use turbo_tiles_graph::{EdgeRecord, Graph, Profile};
use turbo_tiles_mask::{Mask, RefusalKind};

use crate::cost::{CellCost, CostLayer};

/// Slope penalty derived from the DEM. Real hiking effort is
/// non-linear with slope angle — the old linear `1 + slope/15`
/// model under-priced steep terrain badly. A 30° slope isn't twice
/// as hard as flat, it's 4–5× as hard; a 40° slope is climbing
/// terrain, not walking; past 45° you're roping up. We model this
/// as a quadratic with a hard refusal cap:
///
/// ```text
///   cost(slope) = 1 + (slope / `quadratic_scale_deg`)²    for slope <= refuse_above_deg
///   cost(slope) = REFUSED                                  otherwise
/// ```
///
/// Defaults: `quadratic_scale_deg = 12°` → cost is 1.5× at 8°, 2×
/// at ~12°, 3× at 17°, 5× at 24°, 10× at 36°. Refusal at 45°.
///
/// Out-of-coverage cells default to 1.0 (nominal).
pub struct SlopeLayer {
    pub dem: Arc<Dem>,
    /// Slope where the quadratic term hits 1.0 (i.e. total cost = 2×).
    pub quadratic_scale_deg: f32,
    /// Slope above which the cell is impassable. `None` = no cap.
    pub refuse_above_deg: Option<f32>,
}

impl SlopeLayer {
    pub fn new(dem: Arc<Dem>) -> Self {
        Self::with_knobs(dem, 12.0, 45.0)
    }

    /// Construct with explicit `quadratic_scale_deg` /
    /// `refuse_above_deg` values, typically sourced from
    /// `tools/cost-config.toml` at boot. This is the boot wiring's
    /// preferred entry point so the hardcoded defaults in `new`
    /// stop drifting from the config file.
    pub fn with_knobs(dem: Arc<Dem>, quadratic_scale_deg: f32, refuse_above_deg: f32) -> Self {
        Self {
            dem,
            quadratic_scale_deg,
            refuse_above_deg: Some(refuse_above_deg),
        }
    }
}

impl CostLayer for SlopeLayer {
    fn name(&self) -> &'static str {
        "slope"
    }
    fn cell_cost(&self, x: f64, y: f64, _p: Profile) -> CellCost {
        match self.dem.slope_aspect(PointXY { x, y }) {
            Ok(Some(sa)) => {
                if let Some(cap) = self.refuse_above_deg {
                    if sa.slope_deg >= cap {
                        return CellCost::refused("slope_too_steep");
                    }
                }
                // 1 + (s/k)^2. The +1 keeps flat terrain at 1× so
                // this layer composes multiplicatively with the
                // others without inflating "nominal" cells.
                let t = sa.slope_deg / self.quadratic_scale_deg;
                let m = 1.0 + t * t;
                CellCost::multiplier(m)
            }
            _ => CellCost::default(),
        }
    }
    fn covers(&self, x: f64, y: f64) -> bool {
        // `Dem::sample` succeeds (Ok) iff the point is inside the
        // DEM extent — nodata sub-cells still count as "covered".
        // OutOfCoverage is the only "we have no idea" signal.
        self.dem.sample(PointXY { x, y }).is_ok()
    }
}

/// Refusal mask layer: water + glacier → veto. Future kinds (cliff,
/// restricted area, marsh, etc.) can be slotted in here by either
/// reusing the same 2-bit mask's `Reserved3` slot or shipping a
/// separate mask artifact + layer.
pub struct MaskRefusalLayer {
    pub mask: Arc<Mask>,
    /// When `true`, water cells in the rasterised mask are *not*
    /// refused — the vector `water` integral layer is handling them
    /// instead. Glacier (and any other classes) still refuse.
    ///
    /// This avoids the original "5 m tarn = 100 m halo" pathology:
    /// the raster mask quantises a tiny tarn to a single 25 m cell,
    /// which the binary refusal then turns into a hard wall across
    /// whatever mesh cell contains it. The vector layer integrates
    /// the actual crossing length instead, producing finite cost
    /// proportional to how much of the candidate edge sits in
    /// water.
    pub defer_water_to_vector: bool,
}

impl MaskRefusalLayer {
    pub fn new(mask: Arc<Mask>) -> Self {
        Self { mask, defer_water_to_vector: false }
    }

    /// Tell this layer to ignore water cells — the caller has wired
    /// a vector water-integral layer that supersedes them.
    pub fn deferring_water(mut self) -> Self {
        self.defer_water_to_vector = true;
        self
    }
}

impl CostLayer for MaskRefusalLayer {
    fn name(&self) -> &'static str {
        "mask_refusal"
    }
    fn cell_cost(&self, x: f64, y: f64, _p: Profile) -> CellCost {
        match self.mask.refused(x, y) {
            Ok(RefusalKind::Water) if self.defer_water_to_vector => CellCost::default(),
            Ok(RefusalKind::Water) => CellCost::refused("water"),
            Ok(RefusalKind::Glacier) => CellCost::refused("glacier"),
            Ok(RefusalKind::Reserved3) => CellCost::refused("restricted"),
            _ => CellCost::default(),
        }
    }
    fn covers(&self, x: f64, y: f64) -> bool {
        self.mask.refused(x, y).is_ok()
    }
}

/// Generic landcover-mask layer: when the loaded mask reports
/// "present" (bit value 1) at a point, multiply the cell cost by
/// `multiplier`. Lets us share the 2-bit mask format with the
/// water/glacier refusal mask — the difference is purely in the
/// layer's interpretation.
///
/// Constructed by name so the same struct serves `wetland`,
/// `forest`, and any future classes the build CLI emits.
pub struct LandcoverLayer {
    pub mask: Arc<turbo_tiles_mask::Mask>,
    pub layer_name: &'static str,
    pub multiplier: f32,
}

impl CostLayer for LandcoverLayer {
    fn name(&self) -> &'static str {
        self.layer_name
    }
    fn cell_cost(&self, x: f64, y: f64, _p: Profile) -> CellCost {
        // Single-class mask: `RefusalKind::Water` is repurposed as
        // "this layer's class is present here". The mask builder
        // for landcover writes bit value 1 for present cells.
        match self.mask.refused(x, y) {
            Ok(turbo_tiles_mask::RefusalKind::Water) => {
                // An infinite multiplier (e.g. buildings) collapses
                // to an absolute refusal so the solver can short-
                // circuit and the cell-inspect UI can show "refused
                // by: <layer>" rather than "cost = infinity".
                if self.multiplier.is_infinite() {
                    CellCost::refused(self.layer_name)
                } else {
                    CellCost::multiplier(self.multiplier)
                }
            }
            _ => CellCost::default(),
        }
    }
    fn covers(&self, x: f64, y: f64) -> bool {
        self.mask.refused(x, y).is_ok()
    }
}

/// Preferred-edge layer for "premade routes/tracks". Boosts edges
/// the curator has flagged as canonical (e.g. `source == manual`
/// or `source == dnt`) by lowering their effective cost.
///
/// Concretely the `source` u8 codes are baked into `EdgeRecord` by
/// `graph_builder.rs`:
///   1 = fkb (raw OSM/Kartverket import)
///   2 = turbase
///   3 = dnt
///   4 = manual (curator-edited)
///
/// Default: 0.5× cost (so the router halves the perceived length)
/// for `dnt` and `manual` edges, untouched for the rest. Tunable
/// per-source by clients via `with_source_multiplier`.
pub struct PreferredEdgeLayer {
    pub source_multipliers: [f32; 8],
}

impl Default for PreferredEdgeLayer {
    fn default() -> Self {
        // Index = EdgeRecord.source code.
        let mut m = [1.0f32; 8];
        m[3] = 0.5; // dnt
        m[4] = 0.5; // manual
        Self {
            source_multipliers: m,
        }
    }
}

impl PreferredEdgeLayer {
    pub fn with_source_multiplier(mut self, source_code: u8, mult: f32) -> Self {
        if (source_code as usize) < self.source_multipliers.len() {
            self.source_multipliers[source_code as usize] = mult;
        }
        self
    }
}

impl CostLayer for PreferredEdgeLayer {
    fn name(&self) -> &'static str {
        "preferred_edge"
    }
    fn edge_multiplier(&self, edge: &EdgeRecord, _p: Profile) -> f32 {
        self.source_multipliers[(edge.source as usize).min(7)]
    }
}

/// Marking-aware bonus: well-marked trails (red T, blue paint, cairn)
/// are easier to follow than unmarked ones. Bonus is multiplicative.
///
/// Codes from `graph_builder::encode_marking`:
///   1 = red_t (DNT standard)
///   2 = cairn
///   3 = blue_paint
///   4 = unmarked
pub struct MarkingLayer {
    pub marking_multipliers: [f32; 8],
}

impl Default for MarkingLayer {
    fn default() -> Self {
        let mut m = [1.0f32; 8];
        m[1] = 0.85; // red T - canonical Norwegian marking
        m[2] = 0.90; // cairn
        m[3] = 0.92; // blue paint
        m[4] = 1.15; // unmarked - slight penalty
        Self {
            marking_multipliers: m,
        }
    }
}

impl CostLayer for MarkingLayer {
    fn name(&self) -> &'static str {
        "marking"
    }
    fn edge_multiplier(&self, edge: &EdgeRecord, _p: Profile) -> f32 {
        self.marking_multipliers[(edge.marking as usize).min(7)]
    }
}

/// Direction-aware slope cost. The symmetric `SlopeLayer` already
/// charges for steep terrain in general; this layer adds the
/// directional component: **traveling across a contour is cheaper
/// than climbing it**.
///
/// Model: we compute the slope-projected component along the edge
/// direction using the DEM's aspect (downhill direction). A signed
/// slope is then plugged into a Tobler-like hiking function:
///
/// ```text
///   v(s) = exp(-3.5 * |s + 0.05|)            (Tobler, normalised)
///   mult = v_flat / v(s)                      (effective-time ratio)
/// ```
///
/// where `s` is the signed slope along the edge (positive = uphill).
/// Pure traverse → `s = 0` → no contribution. Steep climb → multi-x.
/// Steep descent → smaller multi-x (joints, control).
///
/// Defaults: profile-aware via `cell_cost = 1.0` (no symmetric
/// contribution) plus `edge_cost_modifier` that varies with the
/// traversal direction.
pub struct DirectionalSlopeLayer {
    pub dem: Arc<Dem>,
    /// Below this slope, direction doesn't matter (treat as flat).
    /// Saves a DEM lookup per edge in flat terrain.
    pub min_relevant_slope_deg: f32,
}

impl DirectionalSlopeLayer {
    pub fn new(dem: Arc<Dem>) -> Self {
        Self {
            dem,
            min_relevant_slope_deg: 3.0,
        }
    }
}

impl CostLayer for DirectionalSlopeLayer {
    fn name(&self) -> &'static str {
        "slope_direction"
    }
    fn edge_cost_modifier(
        &self,
        fx: f64,
        fy: f64,
        tx: f64,
        ty: f64,
        profile: Profile,
    ) -> f32 {
        // Cycling on uphill roads is direction-aware too, but the
        // model below is calibrated for hiking. For ski we re-use
        // the hiking model — coarse but conservative.
        let _ = profile;
        let mid_x = 0.5 * (fx + tx);
        let mid_y = 0.5 * (fy + ty);
        let sa = match self.dem.slope_aspect(PointXY { x: mid_x, y: mid_y }) {
            Ok(Some(s)) => s,
            _ => return 1.0,
        };
        if sa.slope_deg < self.min_relevant_slope_deg {
            return 1.0;
        }
        let edge_dx = tx - fx;
        let edge_dy = ty - fy;
        let edge_len = (edge_dx * edge_dx + edge_dy * edge_dy).sqrt() as f32;
        if edge_len < 1e-3 {
            return 1.0;
        }
        // Aspect: degrees clockwise from north → downhill unit vec
        // in world XY (north = +y, east = +x).
        let aspect_rad = sa.aspect_deg.to_radians();
        let downhill_x = aspect_rad.sin();
        let downhill_y = -aspect_rad.cos();
        let edge_ux = edge_dx as f32 / edge_len;
        let edge_uy = edge_dy as f32 / edge_len;
        // dot ∈ [-1, 1]. +1 = exactly downhill; -1 = exactly uphill;
        // 0 = perfect traverse. We want signed slope = -dot * slope
        // (positive when climbing).
        let dot = edge_ux * downhill_x + edge_uy * downhill_y;
        let signed_slope = -dot * sa.slope_deg.to_radians().tan();

        // Tobler's hiking function (normalised to flat ≈ 5 km/h).
        // Constants are the original empirical fit:
        //   v(s) = 6 km/h * exp(-3.5 * |s + 0.05|)
        // We work in ratios so the absolute speed cancels.
        let s_offset = signed_slope + 0.05;
        let speed = (-3.5f32 * s_offset.abs()).exp();
        let speed_flat = (-3.5f32 * 0.05f32).exp();
        let mult = speed_flat / speed;
        // Cap the multiplier so a single steep cell doesn't dominate
        // the whole route cost when the DEM has a single-pixel cliff.
        mult.clamp(0.5, 8.0)
    }
}

/// Avalanche terrain classification. Pure DEM derivative — no
/// external ingest required. Encodes the standard ATES-like rule
/// of thumb used by Norwegian ski touring guides:
///
/// - Slope 30°–45° is **starting-zone** terrain — where slides
///   release. Above 30° is the bottom-end of avalanche risk; the
///   peak hazard band is 35°–40°; above 45° slopes tend to
///   sluff continuously rather than release big.
/// - Aspect matters: lee slopes (N, NE, E in Norway's prevailing
///   westerlies) accumulate wind-deposited snow and dominate
///   accident statistics.
/// - Below ~300 m the snowpack rarely supports a slab; above
///   treeline (~600–1100 m depending on latitude) anchoring trees
///   are absent.
///
/// Output: large cost multipliers on the `ski` profile, ignored
/// for foot/bicycle. Doesn't outright refuse — Norwegian ski
/// touring DOES traverse 30–45° terrain knowingly; the cost
/// expresses preference, not feasibility. A skier who really
/// wants the steep run reweights the layer to 0.
pub struct AvalancheTerrainLayer {
    pub dem: Arc<Dem>,
    /// Lower bound of the dangerous slope band (degrees).
    pub slope_min_deg: f32,
    /// Upper bound — beyond this slopes sluff more than they slab.
    pub slope_max_deg: f32,
    /// Treeline anchor — below this, forest reduces risk. Norway
    /// varies 600 m (north) to 1100 m (south). Coarse default 800.
    pub treeline_m: f32,
    /// Base multiplier when in the danger band, above treeline,
    /// on a lee aspect (NE quadrant). Scales for ski profile only.
    pub base_multiplier: f32,
}

impl AvalancheTerrainLayer {
    pub fn new(dem: Arc<Dem>) -> Self {
        Self {
            dem,
            slope_min_deg: 30.0,
            slope_max_deg: 45.0,
            treeline_m: 800.0,
            base_multiplier: 3.5,
        }
    }
}

impl CostLayer for AvalancheTerrainLayer {
    fn name(&self) -> &'static str {
        "avalanche_terrain"
    }
    fn cell_cost(&self, x: f64, y: f64, profile: Profile) -> CellCost {
        // Foot + bicycle ignore this layer entirely. Hikers will
        // never benefit from the wide aspect penalty (avalanches
        // matter on snow); cyclists don't visit slab terrain.
        if !matches!(profile, Profile::Ski) {
            return CellCost::default();
        }
        let sa = match self.dem.slope_aspect(PointXY { x, y }) {
            Ok(Some(s)) => s,
            _ => return CellCost::default(),
        };
        if sa.slope_deg < self.slope_min_deg || sa.slope_deg > self.slope_max_deg {
            return CellCost::default();
        }
        // Lee-aspect bonus: peaks at NE (45°), tapers to nothing on
        // the windward W aspect (270°). Use a cosine of `(aspect -
        // 45°)` so NE = 1.0, SW = 0.0, NW/SE = 0.5. Lee-side risk
        // dominates the accident record.
        let lee_delta = ((sa.aspect_deg - 45.0) * std::f32::consts::PI / 180.0).cos();
        let lee_factor = (lee_delta + 1.0) * 0.5;
        // Slope-band shape: maximum at 37.5° (halfway through 30–45),
        // tapering to 0 at the bounds. Smooth so the cost field
        // doesn't have step discontinuities.
        let band_centre = (self.slope_min_deg + self.slope_max_deg) * 0.5;
        let band_half = (self.slope_max_deg - self.slope_min_deg) * 0.5;
        let band_dist = (sa.slope_deg - band_centre).abs() / band_half;
        let slope_factor = (1.0 - band_dist).max(0.0);
        // Elevation gate: below treeline, anchoring trees + smaller
        // snowpack drastically reduce risk. Smooth ramp 200 m below
        // treeline → 0; above treeline → 1.
        let elev = match self.dem.sample(PointXY { x, y }) {
            Ok(Some(e)) => e,
            _ => return CellCost::default(),
        };
        let elev_factor =
            ((elev - (self.treeline_m - 200.0)) / 200.0).clamp(0.0, 1.0);
        let risk = lee_factor * slope_factor * elev_factor;
        let extra = (self.base_multiplier - 1.0) * risk;
        CellCost::multiplier(1.0 + extra)
    }
    fn covers(&self, x: f64, y: f64) -> bool {
        // Coverage only when the DEM has data — without slope we
        // can't classify avalanche terrain.
        self.dem.sample(PointXY { x, y }).is_ok()
    }
}

/// Off-trail proximity bias toward the routing graph's edges.
///
/// Theta\* finds the cost-optimal mesh path. When the mesh has
/// uniform 1.0× cost everywhere, that's a straight line —
/// regardless of whether visible trails would have been the right
/// answer for a human hiker.
///
/// This layer makes mesh cells *near* graph edges cheaper. The
/// off-trail solver then naturally drifts toward known trails and
/// follows them when they're close. Profile-aware: foot biases
/// toward `sti` (hiking trails), bicycle toward `vei` (roads), ski
/// toward `skiloype` (groomed tracks). The fkb_type codes match
/// `turbo-tiles-build::graph_builder::encode_fkb_type`.
///
/// Cost falls off linearly from `bonus_at_zero` on the trail itself
/// to 1.0× at `influence_radius_m` away. Outside the radius, no
/// effect. Refused cells stay refused — proximity never overrides
/// a hard veto (water, glacier).
pub struct TrailProximityLayer {
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
        // Classic point-to-segment squared distance.
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

impl TrailProximityLayer {
    /// Build all three per-fkb-type rtrees from the loaded graph.
    /// `influence_radius_m` controls how far the bonus extends —
    /// 100 m is sensible (the mesh cell size). `bonus_at_zero` is
    /// the multiplier directly on top of a trail (0.3 = "three
    /// times cheaper than open ground").
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

    /// True iff at least one segment of any kind sits within the
    /// influence radius. Used by the `covers` impl.
    fn any_near(&self, x: f64, y: f64) -> bool {
        let r2 = self.influence_radius_m * self.influence_radius_m;
        for rt in [&self.sti, &self.vei, &self.skiloype] {
            if let Some(s) = rt.nearest_neighbor(&[x, y]) {
                if s.distance_2(&[x, y]) <= r2 {
                    return true;
                }
            }
        }
        false
    }
}

/// Per-edge slope penalty derived from baked metrics on the
/// `EdgeRecord` itself (`slope_max_deg`, populated by the graph
/// builder when DEM data was available). Lives on the graph side
/// because the off-trail mesh already has [`SlopeLayer`] /
/// [`DirectionalSlopeLayer`] for the same purpose.
///
/// Cost shape, mirroring [`SlopeLayer`] but applied to graph edges:
///
/// ```text
///   slope <= 5°    → multiplier = 1.0 (flat — no penalty)
///   slope < 45°    → multiplier = 1 + (slope / `quadratic_scale_deg`)²
///   slope >= 50°   → multiplier = INFINITY (refused — cliff)
/// ```
///
/// Combined with the Naismith `length_m + 8×gain_m` already baked
/// into `profile_cost`, this stops the router from happily walking
/// over peaks and cliffs. A 30° spike inside a 1 km edge now costs
/// 4–5× more, a 45° spike pushes the edge close to refusal, and a
/// > 50° "trail through a cliff face" (which N50/Turrutebasen
/// genuinely sometimes contain) becomes impassable.
pub struct GraphSlopeLayer {
    pub quadratic_scale_deg: f32,
    /// Edges with `slope_max_deg >= refuse_above_deg` are refused
    /// outright via INFINITY. Pick this to match how aggressive
    /// you want cliff avoidance to be — 50° is a good default for
    /// hiking (anything above that is climbing).
    pub refuse_above_deg: f32,
}

impl Default for GraphSlopeLayer {
    fn default() -> Self {
        Self {
            // 15° → 1.7× cost; 30° → 5×; 45° → 10×.
            quadratic_scale_deg: 15.0,
            refuse_above_deg: 50.0,
        }
    }
}

impl CostLayer for GraphSlopeLayer {
    fn name(&self) -> &'static str {
        "graph_slope"
    }
    fn edge_multiplier(&self, edge: &EdgeRecord, _p: Profile) -> f32 {
        let s = edge.slope_max_deg;
        if s <= 5.0 {
            return 1.0;
        }
        if s >= self.refuse_above_deg {
            return f32::INFINITY;
        }
        let t = s / self.quadratic_scale_deg;
        1.0 + t * t
    }
}

/// Lift the Naismith gain term out of the baked per-profile cost
/// into a runtime-tunable layer. The baked cost includes
/// `length_m + 8×gain_m` (foot) / `… + 20×gain_m` (bicycle) /
/// `… + 6×gain_m` (ski). This layer can amplify that further at
/// request time without rebuilding the graph artifact — useful
/// when the curator wants "very steep aversion" for less-fit
/// users, or "I don't mind climbing" for an alpine profile.
///
/// `gain_amplifier = 1.0` is a no-op (default — keep baked cost).
/// `gain_amplifier = 2.0` makes climbs feel twice as bad as the
/// Naismith default; `0.5` halves them.
pub struct TotalGainLayer {
    pub gain_amplifier: f32,
}

impl Default for TotalGainLayer {
    fn default() -> Self {
        Self { gain_amplifier: 1.0 }
    }
}

impl CostLayer for TotalGainLayer {
    fn name(&self) -> &'static str {
        "total_gain"
    }
    fn edge_multiplier(&self, edge: &EdgeRecord, profile: Profile) -> f32 {
        if self.gain_amplifier == 1.0 || edge.length_m < 1.0 {
            return 1.0;
        }
        // Same per-profile gain coefficient the graph builder uses
        // in profile_cost. Keep these in lockstep — drift here
        // means the gain amplifier compounds incorrectly.
        let k = match profile {
            Profile::Foot => 8.0,
            Profile::Bicycle => 20.0,
            Profile::Ski => 6.0,
        };
        // Extra cost = (amplifier - 1) × Naismith gain term.
        // As an edge multiplier: 1 + extra / length.
        let baseline = k * edge.gain_m;
        let extra = baseline * (self.gain_amplifier - 1.0);
        1.0 + extra / edge.length_m
    }
}

impl CostLayer for TrailProximityLayer {
    fn name(&self) -> &'static str {
        "trail_proximity"
    }
    fn cell_cost(&self, x: f64, y: f64, profile: Profile) -> CellCost {
        let rt = self.rtree_for(profile);
        let Some(seg) = rt.nearest_neighbor(&[x, y]) else {
            return CellCost::default();
        };
        let d = seg.distance_2(&[x, y]).sqrt();
        if d >= self.influence_radius_m {
            return CellCost::default();
        }
        // Linear fall-off: bonus_at_zero at d=0, 1.0 at d=radius.
        let t = (d / self.influence_radius_m) as f32;
        let mult = self.bonus_at_zero + t * (1.0 - self.bonus_at_zero);
        CellCost::multiplier(mult)
    }
    fn covers(&self, x: f64, y: f64) -> bool {
        // Proximity is a *bias*, not a coverage claim. It returning
        // true would short-circuit the "no terrain data" precheck
        // and let queries succeed in regions with only a graph but
        // no DEM/mask — that's actually desirable when the graph is
        // dense, but only when the query is within reach of a real
        // edge. Hence: covers iff some segment is within radius.
        self.any_near(x, y)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::cost::compose_edge;

    fn mk_edge(source: u8, marking: u8) -> EdgeRecord {
        EdgeRecord {
            from_id: 0,
            to_id: 1,
            length_m: 100.0,
            gain_m: 0.0,
            loss_m: 0.0,
            slope_max_deg: 0.0,
            fkb_type: 0,
            marking,
            surface: 0,
            source,
            attr_flags: 0,
        }
    }

    #[test]
    fn preferred_edge_layer_halves_dnt() {
        let l = PreferredEdgeLayer::default();
        let dnt_edge = mk_edge(3, 0);
        assert!((l.edge_multiplier(&dnt_edge, Profile::Foot) - 0.5).abs() < 1e-3);
        let fkb_edge = mk_edge(1, 0);
        assert!((l.edge_multiplier(&fkb_edge, Profile::Foot) - 1.0).abs() < 1e-3);
    }

    #[test]
    fn marking_layer_favours_red_t() {
        let l = MarkingLayer::default();
        let red_t = mk_edge(1, 1);
        let unmarked = mk_edge(1, 4);
        assert!(l.edge_multiplier(&red_t, Profile::Foot) < 1.0);
        assert!(l.edge_multiplier(&unmarked, Profile::Foot) > 1.0);
    }

    fn mk_edge_with_slope(slope_max: f32, gain_m: f32, length_m: f32) -> EdgeRecord {
        EdgeRecord {
            from_id: 0,
            to_id: 1,
            length_m,
            gain_m,
            loss_m: 0.0,
            slope_max_deg: slope_max,
            fkb_type: 0,
            marking: 0,
            surface: 0,
            source: 0,
            attr_flags: 0,
        }
    }

    #[test]
    fn graph_slope_layer_quadratic_under_45() {
        let l = GraphSlopeLayer::default();
        // 0° → 1.0
        assert!((l.edge_multiplier(&mk_edge_with_slope(0.0, 0.0, 100.0), Profile::Foot) - 1.0).abs() < 1e-3);
        // 5° → 1.0 (flat band)
        assert!((l.edge_multiplier(&mk_edge_with_slope(5.0, 0.0, 100.0), Profile::Foot) - 1.0).abs() < 1e-3);
        // 15° → 1 + (15/15)² = 2.0
        assert!((l.edge_multiplier(&mk_edge_with_slope(15.0, 0.0, 100.0), Profile::Foot) - 2.0).abs() < 1e-2);
        // 30° → 1 + 4 = 5.0
        assert!((l.edge_multiplier(&mk_edge_with_slope(30.0, 0.0, 100.0), Profile::Foot) - 5.0).abs() < 1e-2);
        // 45° → 1 + 9 = 10.0
        assert!((l.edge_multiplier(&mk_edge_with_slope(45.0, 0.0, 100.0), Profile::Foot) - 10.0).abs() < 1e-2);
    }

    #[test]
    fn graph_slope_layer_refuses_above_50() {
        let l = GraphSlopeLayer::default();
        let m = l.edge_multiplier(&mk_edge_with_slope(51.0, 0.0, 100.0), Profile::Foot);
        assert!(m.is_infinite());
        let m = l.edge_multiplier(&mk_edge_with_slope(60.0, 0.0, 100.0), Profile::Foot);
        assert!(m.is_infinite());
    }

    #[test]
    fn total_gain_default_is_identity() {
        let l = TotalGainLayer::default();
        let m = l.edge_multiplier(&mk_edge_with_slope(0.0, 100.0, 1000.0), Profile::Foot);
        assert!((m - 1.0).abs() < 1e-3);
    }

    #[test]
    fn total_gain_amplifier_scales_naismith_term() {
        let l = TotalGainLayer { gain_amplifier: 2.0 };
        // edge: 1000m length, 100m gain. Naismith baseline = 800.
        // Extra = baseline × (2-1) = 800. Multiplier = 1 + 800/1000 = 1.8.
        let m = l.edge_multiplier(&mk_edge_with_slope(0.0, 100.0, 1000.0), Profile::Foot);
        assert!((m - 1.8).abs() < 1e-2, "got {m}");
    }

    #[test]
    fn total_gain_zero_amplifier_subtracts_naismith() {
        // amplifier = 0 → effective gain cost removed.
        // Naismith baseline 800 over 1000m → multiplier should be
        // 1 - 800/1000 = 0.2 (i.e. uphill becomes cheaper).
        let l = TotalGainLayer { gain_amplifier: 0.0 };
        let m = l.edge_multiplier(&mk_edge_with_slope(0.0, 100.0, 1000.0), Profile::Foot);
        assert!((m - 0.2).abs() < 1e-2, "got {m}");
    }

    #[test]
    fn layered_compose_combines_marking_and_source() {
        // Combined: red_t (0.85) × dnt (0.5) = 0.425
        let layers: Vec<Arc<dyn CostLayer>> = vec![
            Arc::new(MarkingLayer::default()),
            Arc::new(PreferredEdgeLayer::default()),
        ];
        let edge = mk_edge(3, 1);
        let total = compose_edge(&layers, &|_| 1.0, &edge, Profile::Foot);
        assert!((total - 0.425).abs() < 0.01, "got {total}");
    }
}
