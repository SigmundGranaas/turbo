//! `FetchPipeline` — the desktop host's tile-transport loop.
//!
//! Pre-P6.2 this module was `MapHost`: it OWNED the core `Map` and three
//! decode-happy pumps. The app is now an ordinary Scene host on
//! `TurbomapEngine` (the engine owns the map and the codec), so what
//! remains here is exactly the host half of the streaming contract:
//!
//! 1. take one `streaming_plan` per frame, sized to the free lane capacity
//! 2. spawn raw-byte fetches on the [`crate::runtime::BytesPump`]s
//! 3. drain finished fetches into `engine.ingest_*` (bytes, not decodes)
//! 4. report declines/failures back (`fetch_cancelled` / `fetch_failed`)
//! 5. keep the `(layer, tile)` inflight set + retry backoff
//!
//! This mirrors the pre-rewrite dispatch logic (it was already plan-driven,
//! slice B3.3b) — only the ingest side changed from decoded payloads to
//! encoded bytes.

use std::collections::{HashMap, HashSet};
use std::time::{Duration, Instant};

use turbomap_core::{PendingTile, TileId};
use turbomap_engine::TurbomapEngine;

use crate::runtime::BytesPump;
use crate::schedule::Workload;

/// Sentinel layer key for terrain DEM tiles — they live at engine level,
/// not on a named layer, but the inflight set still needs to disambiguate
/// them from a basemap raster at the same `TileId`.
pub const TERRAIN_KEY: &str = "__terrain";

/// Per-lane cap on outstanding network fetches. Bigger means faster fills
/// on cold start; smaller means fast pan/zoom doesn't queue hundreds of
/// doomed-stale fetches.
pub const MAX_INFLIGHT_PER_LANE: usize = 16;

/// How long a tile that failed to load stays on the don't-retry list.
/// Stops a single rate-limit response from snowballing into a per-frame
/// retry storm.
pub const RETRY_BACKOFF: Duration = Duration::from_secs(8);

/// The three transport lanes, one per pump. Vector layers all share one
/// lane (they share the one vector endpoint); raster/DEM likewise.
#[derive(Clone, Copy, PartialEq, Eq, Hash)]
enum Lane {
    Vector,
    Raster,
    Terrain,
}

pub struct FetchPipeline {
    vector_pump: BytesPump,
    raster_pump: BytesPump,
    /// `None` if the demo was launched without a DEM source (no
    /// `TURBO_API_URL` env var). Terrain starts from the plan are declined
    /// in that case — the scene shouldn't declare a hillshade layer then
    /// either, so in practice none are issued.
    dem_pump: Option<BytesPump>,
    /// `(lane, layer_id, tile)` — the lane + layer prefix lets raster and
    /// terrain fetch the same `(z,x,y)` concurrently without one silently
    /// masking the other.
    inflight: HashSet<(Lane, String, TileId)>,
    /// Fetches whose last attempt failed, plus the timestamp. `tick` GCs
    /// entries older than [`RETRY_BACKOFF`].
    recently_failed: HashMap<(Lane, String, TileId), Instant>,
    /// Plan attempt ids for spawned fetches, so worker outcomes can report
    /// `fetch_failed` (deliveries complete implicitly through `ingest_*`).
    attempts: HashMap<(Lane, String, TileId), turbomap_core::RequestId>,
}

impl FetchPipeline {
    pub fn new(
        vector_pump: BytesPump,
        raster_pump: BytesPump,
        dem_pump: Option<BytesPump>,
    ) -> Self {
        Self {
            vector_pump,
            raster_pump,
            dem_pump,
            inflight: HashSet::new(),
            recently_failed: HashMap::new(),
            attempts: HashMap::new(),
        }
    }

    /// Per-tick GC of the retry-backoff list.
    pub fn tick(&mut self, now: Instant) {
        self.recently_failed
            .retain(|_, ts| now.duration_since(*ts) < RETRY_BACKOFF);
    }

    /// Drain finished fetches from the worker channels into the engine.
    /// Bytes go straight through `ingest_*` — the engine's codec decodes
    /// off the render thread and applies under the per-frame budget.
    pub fn drain(&mut self, engine: &mut TurbomapEngine) {
        while let Ok(out) = self.vector_pump.rx.try_recv() {
            let key = (Lane::Vector, out.layer, out.tile);
            self.inflight.remove(&key);
            match out.bytes {
                Some(bytes) => {
                    self.attempts.remove(&key);
                    self.recently_failed.remove(&key);
                    engine.ingest_mvt(&key.1, out.tile, &bytes);
                }
                None => {
                    if let Some(req) = self.attempts.remove(&key) {
                        engine.fetch_failed(req);
                    }
                    self.recently_failed.insert(key, Instant::now());
                }
            }
        }
        while let Ok(out) = self.raster_pump.rx.try_recv() {
            let key = (Lane::Raster, out.layer, out.tile);
            self.inflight.remove(&key);
            match out.bytes {
                Some(bytes) => {
                    self.attempts.remove(&key);
                    self.recently_failed.remove(&key);
                    engine.ingest_raster_encoded(&key.1, out.tile, &bytes);
                }
                None => {
                    if let Some(req) = self.attempts.remove(&key) {
                        engine.fetch_failed(req);
                    }
                    self.recently_failed.insert(key, Instant::now());
                }
            }
        }
        if let Some(dem_pump) = self.dem_pump.as_ref() {
            let mut done = Vec::new();
            while let Ok(out) = dem_pump.rx.try_recv() {
                done.push(out);
            }
            for out in done {
                let key = (Lane::Terrain, out.layer, out.tile);
                self.inflight.remove(&key);
                match out.bytes {
                    Some(bytes) => {
                        self.attempts.remove(&key);
                        self.recently_failed.remove(&key);
                        engine.ingest_terrain_encoded(out.tile, &bytes);
                    }
                    None => {
                        if let Some(req) = self.attempts.remove(&key) {
                            engine.fetch_failed(req);
                        }
                        self.recently_failed.insert(key, Instant::now());
                    }
                }
            }
        }
    }

    /// Take one engine `streaming_plan` sized to the free lane capacity and
    /// spawn its `start` fetches (already globally ordered by the one
    /// priority score). Every start carries a `RequestId`; starts we decline
    /// (lane full, retry backoff, unsupported kind) are reported cancelled
    /// so the engine re-issues them later, and `cancel` entries are
    /// acknowledged immediately — a blocking `reqwest` fetch can't be
    /// aborted mid-flight, but the inflight set prevents a duplicate spawn
    /// and a late delivery simply completes whatever attempt is current.
    pub fn dispatch(&mut self, engine: &mut TurbomapEngine) {
        let (mut vector_in, mut raster_in, mut terrain_in) = (0usize, 0usize, 0usize);
        for (lane, _, _) in &self.inflight {
            match lane {
                Lane::Vector => vector_in += 1,
                Lane::Raster => raster_in += 1,
                Lane::Terrain => terrain_in += 1,
            }
        }
        let free = |used: usize| MAX_INFLIGHT_PER_LANE.saturating_sub(used);
        let budget = free(vector_in) + free(raster_in) + free(terrain_in);
        let plan = engine.streaming_plan(budget);
        for id in plan.cancel {
            engine.fetch_cancelled(id);
        }
        let mut declined: Vec<turbomap_core::RequestId> = Vec::new();
        for req in plan.start {
            let id = req.id;
            let (lane, layer, tile, pump, lane_used) = match req.fetch {
                PendingTile::Vector { layer_id, tile } => (
                    Lane::Vector,
                    layer_id,
                    tile,
                    &self.vector_pump,
                    &mut vector_in,
                ),
                PendingTile::Raster { layer_id, tile } => (
                    Lane::Raster,
                    layer_id,
                    tile,
                    &self.raster_pump,
                    &mut raster_in,
                ),
                PendingTile::Terrain { tile } => {
                    let Some(dem_pump) = self.dem_pump.as_ref() else {
                        declined.push(id);
                        continue;
                    };
                    (
                        Lane::Terrain,
                        TERRAIN_KEY.to_string(),
                        tile,
                        dem_pump,
                        &mut terrain_in,
                    )
                }
                // Hillshade overlays are fed by terrain DEM tiles, not
                // their own tile stream — decline these honestly.
                PendingTile::Hillshade { .. } => {
                    declined.push(id);
                    continue;
                }
            };
            let key = (lane, layer, tile);
            if self.recently_failed.contains_key(&key)
                || *lane_used >= MAX_INFLIGHT_PER_LANE
                || !self.inflight.insert(key.clone())
            {
                declined.push(id);
                continue;
            }
            pump.spawn_fetch(key.1.clone(), tile);
            self.attempts.insert(key, id);
            *lane_used += 1;
        }
        // Declined starts go back to the engine so they re-issue on a
        // later plan.
        for id in declined {
            engine.fetch_cancelled(id);
        }
    }

    /// Snapshot for the render scheduler. Read once per tick in
    /// `App::about_to_wait`. The engine's `is_animating` already folds in
    /// its decode backlog (delivered-but-not-yet-drawable tiles apply
    /// inside `render()`, so a sleeping host would strand them); the
    /// backlog also counts as worker data so a fresh delivery wakes the
    /// loop immediately.
    pub fn workload(&self, engine: &TurbomapEngine) -> Workload {
        Workload {
            workers_have_data: !self.vector_pump.rx.is_empty()
                || !self.raster_pump.rx.is_empty()
                || self
                    .dem_pump
                    .as_ref()
                    .map(|p| !p.rx.is_empty())
                    .unwrap_or(false)
                || engine.decode_backlog() > 0,
            workers_in_flight: !self.inflight.is_empty(),
            map_animating: engine.is_animating(),
        }
    }
}
