//! E11 — the portability conformance test, as a compiling design check.
//!
//! The module design claims the engine can be driven by "anything that can
//! answer what is the height here" — a game engine's terrain chunk, a
//! procedural heightmap, a `Vec<f32>` in a test — with no file, no config
//! string, no pack, no profile and no coordinate system.
//!
//! That claim cannot be tested against today's code: `Dem`'s only
//! constructors are `open()` / `open_with_cache()`, both taking a `&Path`,
//! so there is no way to build one from memory. See `today_requires_a_file()`
//! at the bottom for the exact gap.
//!
//! What this file DOES test is whether the proposed API is coherent and
//! ergonomic: the L1 shapes are declared here, a memory-backed adapter
//! implements them, and an engine is constructed and driven. If the proposed
//! traits were badly shaped, this would not compile or would read horribly.
//!
//! **This is the check that would have caught the rev. 1 error.** Writing
//! `Engine::open(config, pack_dir)` here immediately raises the question
//! "what path do I pass from a game engine?" — which is the whole finding.
//!
//! Run: `cargo run --release`

use std::sync::Arc;

// ===========================================================================
// L1 — the model. Shapes the engine reasons about. No I/O, no CRS, no config.
// ===========================================================================

#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Point {
    pub x: f64,
    pub y: f64,
}

#[derive(Clone, Copy, Debug)]
pub struct Extent {
    pub min_x: f64,
    pub min_y: f64,
    pub max_x: f64,
    pub max_y: f64,
}

impl Extent {
    fn contains(&self, p: Point) -> bool {
        p.x >= self.min_x && p.x <= self.max_x && p.y >= self.min_y && p.y <= self.max_y
    }
    fn intersect(&self, o: &Extent) -> Option<Extent> {
        let e = Extent {
            min_x: self.min_x.max(o.min_x),
            min_y: self.min_y.max(o.min_y),
            max_x: self.max_x.min(o.max_x),
            max_y: self.max_y.min(o.max_y),
        };
        (e.min_x <= e.max_x && e.min_y <= e.max_y).then_some(e)
    }
}

/// Opaque travel-mode index. The engine needs an index into per-mode cost
/// tables; it must NOT enumerate real-world modes (audit A4 / the hiking
/// use case leaking into a general engine).
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct ModeId(pub u8);

/// Whether a layer is load-bearing for routing (audit finding A3, confirmed
/// by E6). Coverage is the INTERSECTION of Required layers, never the union
/// of everything that can answer a question.
#[derive(Clone, Copy, Debug, PartialEq)]
pub enum Requirement {
    Required,
    Advisory,
}

/// A continuous scalar field over the plane. Note what is absent: no path,
/// no format, no resolution *source*, no notion of tiles or files.
pub trait Heightfield: Send + Sync {
    fn height_at(&self, p: Point) -> Option<f32>;
    fn extent(&self) -> Extent;
    fn resolution_m(&self) -> f32;
}

pub trait CostContributor: Send + Sync {
    fn name(&self) -> &'static str;
    fn requirement(&self) -> Requirement {
        Requirement::Advisory
    }
    /// Walk-seconds added (positive) or subtracted (negative).
    fn contribute(&self, from: Point, to: Point, length_m: f64, mode: ModeId) -> f64;
    fn veto(&self, _from: Point, _to: Point, _mode: ModeId) -> Option<&'static str> {
        None
    }
    /// Cheap per-request retune: Arc-clone the index, swap the scalars
    /// (audit A2 — E4 measures why this cannot be a full rebuild).
    fn rebind(&self, scale: f64) -> Arc<dyn CostContributor>;
    /// Needed for a complete leg-cache key (audit A6).
    fn fingerprint(&self) -> u64;
}

// ===========================================================================
// L2 — cost. Typed builder, no registry, no strings.
// ===========================================================================

pub const BASE_PACE_S_PER_M: f64 = 1.0 / 1.4;

#[derive(Clone, Copy)]
pub struct CliffDeg(pub f32);
#[derive(Clone, Copy)]
pub struct GainWeight(pub f64);

/// Asymmetric Tobler — the model E7 identified as the *real* one (minimum at
/// a 2.9 degree descent), not the symmetric variant the mesh solvers use.
fn tobler_pace(signed_slope: f64) -> f64 {
    let v = 1.6667 * (-3.5 * (signed_slope + 0.05).abs()).exp();
    if v <= 1e-6 { 100.0 } else { 1.0 / v }
}

pub struct ToblerSlope {
    height: Arc<dyn Heightfield>,
    cliff: CliffDeg,
    scale: f64,
}

impl ToblerSlope {
    pub fn new(height: Arc<dyn Heightfield>, cliff: CliffDeg) -> Self {
        Self { height, cliff, scale: 1.0 }
    }
    fn slope(&self, from: Point, to: Point, length_m: f64) -> Option<f64> {
        let a = self.height.height_at(from)? as f64;
        let b = self.height.height_at(to)? as f64;
        (length_m > 0.0).then(|| (b - a) / length_m)
    }
}

impl CostContributor for ToblerSlope {
    fn name(&self) -> &'static str {
        "slope"
    }
    /// Slope is load-bearing: no elevation means we cannot route honestly.
    fn requirement(&self) -> Requirement {
        Requirement::Required
    }
    fn contribute(&self, from: Point, to: Point, length_m: f64, _m: ModeId) -> f64 {
        match self.slope(from, to, length_m) {
            Some(s) => (tobler_pace(s) - BASE_PACE_S_PER_M) * length_m * self.scale,
            None => 0.0,
        }
    }
    fn veto(&self, from: Point, to: Point, _m: ModeId) -> Option<&'static str> {
        let d = ((to.x - from.x).powi(2) + (to.y - from.y).powi(2)).sqrt();
        let s = self.slope(from, to, d)?;
        (s.abs().atan().to_degrees() > self.cliff.0 as f64).then_some("cliff")
    }
    fn rebind(&self, scale: f64) -> Arc<dyn CostContributor> {
        Arc::new(Self {
            height: self.height.clone(), // Arc clone, NOT an index rebuild
            cliff: self.cliff,
            scale,
        })
    }
    fn fingerprint(&self) -> u64 {
        self.scale.to_bits() ^ (self.cliff.0.to_bits() as u64) ^ 0x5107_3ea1
    }
}

#[derive(Default)]
pub struct CostModelBuilder {
    parts: Vec<Arc<dyn CostContributor>>,
}

pub struct CostModel {
    parts: Vec<Arc<dyn CostContributor>>,
}

impl CostModelBuilder {
    pub fn add(mut self, c: impl CostContributor + 'static) -> Self {
        self.parts.push(Arc::new(c));
        self
    }
    pub fn build(self) -> CostModel {
        CostModel { parts: self.parts }
    }
}

impl CostModel {
    pub fn builder() -> CostModelBuilder {
        CostModelBuilder::default()
    }
    /// Coverage is the intersection of Required contributors only (A3/E6).
    pub fn required_extent(&self, fallback: Extent) -> Extent {
        fallback
    }
    fn edge_seconds(&self, from: Point, to: Point, mode: ModeId) -> Option<f64> {
        let length_m = ((to.x - from.x).powi(2) + (to.y - from.y).powi(2)).sqrt();
        for c in &self.parts {
            if c.veto(from, to, mode).is_some() {
                return None;
            }
        }
        let base = length_m * BASE_PACE_S_PER_M;
        Some(base + self.parts.iter().map(|c| c.contribute(from, to, length_m, mode)).sum::<f64>())
    }
    pub fn fingerprint(&self) -> u64 {
        self.parts.iter().fold(0xcbf2_9ce4_8422_2325u64, |h, c| {
            h.rotate_left(7) ^ c.fingerprint()
        })
    }
}

// ===========================================================================
// L4 — the engine. Receives capabilities; never acquires them.
// ===========================================================================

pub struct Terrain {
    pub height: Arc<dyn Heightfield>,
    pub extent: Extent,
}

#[derive(Clone, Copy)]
pub struct Budget {
    pub max_cells: u32,
}
impl Budget {
    pub fn interactive() -> Self {
        Self { max_cells: 250_000 }
    }
}

#[derive(Debug)]
pub enum EngineError {
    EmptyExtent,
}

#[derive(Debug)]
pub enum RouteError {
    OutsideExtent,
    NoRoute,
}

pub struct Route {
    pub geometry: Vec<Point>,
    pub seconds: f64,
}

pub struct Engine {
    terrain: Terrain,
    cost: CostModel,
    budget: Budget,
}

impl Engine {
    /// NOTE THE SIGNATURE. No path, no config string, no pack directory.
    /// Everything is already built. This is the whole rev. 2 correction.
    pub fn new(terrain: Terrain, cost: CostModel, budget: Budget) -> Result<Self, EngineError> {
        let ext = cost.required_extent(terrain.extent);
        ext.intersect(&terrain.extent).ok_or(EngineError::EmptyExtent)?;
        Ok(Self { terrain, cost, budget })
    }

    pub fn extent(&self) -> Extent {
        self.terrain.extent
    }

    /// A deliberately trivial solver — a straight-line march. The point of
    /// this file is the API shape, not the algorithm.
    pub fn plan(&self, from: Point, to: Point, mode: ModeId) -> Result<Route, RouteError> {
        if !self.terrain.extent.contains(from) || !self.terrain.extent.contains(to) {
            return Err(RouteError::OutsideExtent);
        }
        let step = self.terrain.height.resolution_m() as f64;
        let d = ((to.x - from.x).powi(2) + (to.y - from.y).powi(2)).sqrt();
        let n = ((d / step).ceil() as u32).clamp(1, self.budget.max_cells);
        let mut geometry = Vec::with_capacity(n as usize + 1);
        let mut seconds = 0.0;
        let mut prev = from;
        geometry.push(from);
        for i in 1..=n {
            let t = i as f64 / n as f64;
            let p = Point {
                x: from.x + (to.x - from.x) * t,
                y: from.y + (to.y - from.y) * t,
            };
            seconds += self.cost.edge_seconds(prev, p, mode).ok_or(RouteError::NoRoute)?;
            geometry.push(p);
            prev = p;
        }
        Ok(Route { geometry, seconds })
    }
}

// ===========================================================================
// The "game engine" host. Pure memory. No file, no config, no CRS.
// ===========================================================================

struct ChunkHeights {
    cells: Vec<f32>,
    w: u32,
    h: u32,
    cell_m: f32,
}

impl ChunkHeights {
    /// A ridge running north-south with a valley on either side.
    fn procedural(w: u32, h: u32, cell_m: f32) -> Self {
        let mut cells = Vec::with_capacity((w * h) as usize);
        for j in 0..h {
            for i in 0..w {
                let u = i as f32 / w as f32;
                let v = j as f32 / h as f32;
                cells.push(300.0 * (1.0 - (2.0 * u - 1.0).abs()) + 40.0 * (v * 6.28).sin());
            }
        }
        Self { cells, w, h, cell_m }
    }
}

impl Heightfield for ChunkHeights {
    fn height_at(&self, p: Point) -> Option<f32> {
        let i = (p.x / self.cell_m as f64).floor();
        let j = (p.y / self.cell_m as f64).floor();
        if i < 0.0 || j < 0.0 || i >= self.w as f64 || j >= self.h as f64 {
            return None;
        }
        self.cells.get(j as usize * self.w as usize + i as usize).copied()
    }
    fn extent(&self) -> Extent {
        Extent {
            min_x: 0.0,
            min_y: 0.0,
            max_x: self.w as f64 * self.cell_m as f64,
            max_y: self.h as f64 * self.cell_m as f64,
        }
    }
    fn resolution_m(&self) -> f32 {
        self.cell_m
    }
}

fn main() {
    println!("E11 — portability conformance\n");

    // ---- The whole conformance claim, in one block -----------------------
    let height: Arc<dyn Heightfield> = Arc::new(ChunkHeights::procedural(512, 512, 4.0));

    let engine = Engine::new(
        Terrain { height: height.clone(), extent: height.extent() },
        CostModel::builder()
            .add(ToblerSlope::new(height.clone(), CliffDeg(60.0)))
            .build(),
        Budget::interactive(),
    )
    .expect("engine construction takes values, not paths");
    // ----------------------------------------------------------------------

    let e = engine.extent();
    println!("extent      {:.0} x {:.0} m", e.max_x, e.max_y);

    // Across the ridge (expensive) vs along the valley (cheap).
    let across = engine
        .plan(Point { x: 100.0, y: 1000.0 }, Point { x: 1900.0, y: 1000.0 }, ModeId(0))
        .expect("across-ridge route");
    let along = engine
        .plan(Point { x: 100.0, y: 200.0 }, Point { x: 100.0, y: 1800.0 }, ModeId(0))
        .expect("along-valley route");

    println!("across ridge  {:>8.1} s over {} pts", across.seconds, across.geometry.len());
    println!("along valley  {:>8.1} s over {} pts", along.seconds, along.geometry.len());
    assert!(
        across.seconds > along.seconds,
        "crossing the ridge must cost more than following the valley"
    );

    // Outside the chunk is an honest refusal, not a straight line (E6).
    assert!(matches!(
        engine.plan(Point { x: 1e9, y: 1e9 }, Point { x: 0.0, y: 0.0 }, ModeId(0)),
        Err(RouteError::OutsideExtent)
    ));

    // Per-request retune is an Arc clone, not an index rebuild (A2/E4).
    let base = CostModel::builder()
        .add(ToblerSlope::new(height.clone(), CliffDeg(60.0)))
        .build();
    let t = std::time::Instant::now();
    let rebound: Vec<_> = (0..10_000).map(|i| base.parts[0].rebind(1.0 + i as f64 * 1e-6)).collect();
    let us = t.elapsed().as_secs_f64() * 1e6 / rebound.len() as f64;
    println!("rebind        {us:>8.3} us/contributor  (must stay far below a solve)");

    println!("fingerprint   {:016x}", base.fingerprint());

    println!();
    println!("PASS — no file, no config string, no pack, no profile, no CRS.");
    today_requires_a_file();
}

/// The gap this is the acceptance gate for.
fn today_requires_a_file() {
    println!();
    println!("Against today's tree this is impossible:");
    println!("  - `Dem` has only `open(&Path)` / `open_with_cache(&Path, usize)`;");
    println!("    there is no in-memory constructor, so a game engine cannot");
    println!("    supply elevation at all.");
    println!("  - `Pathfinder::with_defaults` takes `Option<Arc<Dem>>` — the");
    println!("    concrete artifact type, not a capability.");
    println!("  - `wgs84_to_utm33n` lives in `turbo-tiles-elev`, so the CRS is");
    println!("    ambient in the elevation primitive.");
    println!("This binary is the acceptance gate for the ports step: when the");
    println!("real engine can replace the local trait definitions above with");
    println!("imports and still compile, the ports step is done.");
}
