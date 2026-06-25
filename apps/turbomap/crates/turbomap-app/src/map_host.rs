//! `MapHost` — owns the `Map` plus the tile-fetch pipeline.
//!
//! Before this existed, three near-identical drain loops and a
//! 50-line "spawn fetches by priority" block lived inline in
//! `App::on_redraw`. Every new tile kind (we're adding vector
//! displacement next) required hand-copying the same five
//! responsibilities into `app.rs`:
//!
//! 1. drain the worker channel
//! 2. ingest decoded data into `Map`
//! 3. track the `(layer, tile)` inflight set
//! 4. honour the per-layer backpressure cap
//! 5. retry-backoff on failure
//!
//! Owning that pipeline here means the App never sees a pump
//! directly. It calls `drain_workers` and `dispatch_fetches`,
//! reads a `Workload` snapshot for the scheduler, and otherwise
//! talks to the `Map` through accessors.

use std::collections::{HashMap, HashSet};
use std::sync::Arc;
use std::time::{Duration, Instant};

use turbomap_core::{Map, PendingTile, TileId, TileSource, VectorStyle, VectorTileSource};

use crate::runtime::{RasterFetchPump, RasterOutcome, VectorFetchPump, VectorOutcome};
use crate::schedule::Workload;

/// Layer identifiers the App passes around so raster + vector
/// + DEM fetches can be tracked in a single `inflight` set
/// without colliding.
pub const RASTER_LAYER_ID: &str = "kartverket-topo-grey";
pub const HILLSHADE_LAYER_ID: &str = "turbo-dem";
pub const VECTOR_LAYER_ID: &str = "versatiles-osm";
/// Sentinel for terrain DEM tiles — they live at Map level,
/// not on a named layer, but the inflight set still needs to
/// disambiguate them from a basemap raster at the same TileId.
pub const TERRAIN_KEY: &str = "__terrain";

/// Per-layer cap on outstanding network fetches. Bigger means
/// faster fills on cold start; smaller means fast pan/zoom
/// doesn't queue hundreds of doomed-stale fetches.
pub const MAX_INFLIGHT_PER_LAYER: usize = 16;

/// How long a tile that failed to load stays on the
/// don't-retry list. Stops a single rate-limit response from
/// snowballing into a per-frame retry storm.
pub const RETRY_BACKOFF: Duration = Duration::from_secs(8);

/// Composite owner of the `Map` and its background-fetch
/// pipeline.
pub struct MapHost {
    map: Map,
    vector_pump: VectorFetchPump,
    raster_pump: RasterFetchPump,
    /// `None` if the demo was launched without a DEM source
    /// (no `TURBO_API_URL` env var). Pending Terrain tiles
    /// from the Map are silently dropped in that case — the
    /// hillshade layer just won't render until DEM is wired
    /// in.
    dem_pump: Option<RasterFetchPump>,
    /// `(layer_id, tile_id)` — the layer prefix lets raster
    /// and hillshade fetch the same (z,x,y) concurrently
    /// without one silently masking the other (an earlier
    /// bug from using a bare `HashSet<TileId>`).
    inflight: HashSet<(&'static str, TileId)>,
    /// Tiles whose last fetch failed, plus the timestamp.
    /// `tick` GCs entries older than `RETRY_BACKOFF`.
    recently_failed: HashMap<TileId, Instant>,
}

impl MapHost {
    pub fn new(
        map: Map,
        vector_pump: VectorFetchPump,
        raster_pump: RasterFetchPump,
        dem_pump: Option<RasterFetchPump>,
    ) -> Self {
        Self {
            map,
            vector_pump,
            raster_pump,
            dem_pump,
            inflight: HashSet::new(),
            recently_failed: HashMap::new(),
        }
    }

    /// Direct access to the Map for input handling (camera
    /// updates, markers, hit-test, etc.). Exposed because
    /// nothing in those paths benefits from a wrapper.
    pub fn map(&self) -> &Map {
        &self.map
    }
    pub fn map_mut(&mut self) -> &mut Map {
        &mut self.map
    }

    /// Per-tick advance + GC.
    pub fn tick(&mut self, now: Instant) {
        self.map.tick(now);
        self.recently_failed
            .retain(|_, ts| now.duration_since(*ts) < RETRY_BACKOFF);
    }

    /// Resize the map's render viewport (depth texture is
    /// recreated inside `Map::resize`).
    pub fn resize(&mut self, width: u32, height: u32) {
        self.map.resize(width, height);
    }

    /// Render the map to `target` (single render pass per
    /// visible layer, see `turbomap-core`).
    pub fn render(&mut self, encoder: &mut wgpu::CommandEncoder, target: &wgpu::TextureView) {
        self.map.render(encoder, target);
    }

    /// Hook GPU-timestamp readback for the just-submitted
    /// frame.
    pub fn after_submit(&mut self) {
        self.map.after_submit();
    }

    /// Drain decoded tiles from worker channels into the map.
    /// Returns whether any work was applied (caller doesn't
    /// currently use this but it's the natural shape for a
    /// future "tick had effects" flag).
    pub fn drain_workers(&mut self) -> bool {
        let mut applied = false;
        while let Ok(outcome) = self.vector_pump.rx.try_recv() {
            applied = true;
            match outcome {
                VectorOutcome::Decoded {
                    id,
                    mesh,
                    labels,
                    icons,
                    interactive,
                } => {
                    self.inflight.remove(&(VECTOR_LAYER_ID, id));
                    self.recently_failed.remove(&id);
                    self.map.ingest_vector_mesh(
                        VECTOR_LAYER_ID,
                        id,
                        &mesh,
                        labels,
                        icons,
                        interactive,
                    );
                }
                VectorOutcome::Failed(id) => {
                    self.inflight.remove(&(VECTOR_LAYER_ID, id));
                    self.recently_failed.insert(id, Instant::now());
                }
            }
        }
        while let Ok(outcome) = self.raster_pump.rx.try_recv() {
            applied = true;
            match outcome {
                RasterOutcome::Decoded {
                    id,
                    rgba,
                    width,
                    height,
                } => {
                    self.inflight.remove(&(RASTER_LAYER_ID, id));
                    self.recently_failed.remove(&id);
                    self.map
                        .ingest_raster(RASTER_LAYER_ID, id, &rgba, width, height);
                }
                RasterOutcome::Failed(id) => {
                    self.inflight.remove(&(RASTER_LAYER_ID, id));
                    self.recently_failed.insert(id, Instant::now());
                }
            }
        }
        if let Some(dem_pump) = self.dem_pump.as_ref() {
            while let Ok(outcome) = dem_pump.rx.try_recv() {
                applied = true;
                match outcome {
                    RasterOutcome::Decoded {
                        id,
                        rgba,
                        width,
                        height,
                    } => {
                        self.inflight.remove(&(TERRAIN_KEY, id));
                        self.recently_failed.remove(&id);
                        self.map.ingest_terrain_tile(id, &rgba, width, height);
                    }
                    RasterOutcome::Failed(id) => {
                        self.inflight.remove(&(TERRAIN_KEY, id));
                        self.recently_failed.insert(id, Instant::now());
                    }
                }
            }
        }
        applied
    }

    /// Look at `Map::pending_tiles`, prioritise by distance
    /// to the camera centre, then spawn fetches up to the
    /// per-layer cap. Tiles in the recently-failed set are
    /// skipped this tick.
    pub fn dispatch_fetches(&mut self) {
        let pending = self.map.pending_tiles();
        let centre = self.map.camera().center.to_world();
        let mut prioritised: Vec<PendingTile> = pending;
        prioritised.sort_by(|a, b| {
            let da = tile_distance_to(a, centre);
            let db = tile_distance_to(b, centre);
            da.partial_cmp(&db).unwrap_or(std::cmp::Ordering::Equal)
        });
        let mut vector_in = self
            .inflight
            .iter()
            .filter(|(k, _)| *k == VECTOR_LAYER_ID)
            .count();
        let mut raster_in = self
            .inflight
            .iter()
            .filter(|(k, _)| *k == RASTER_LAYER_ID)
            .count();
        let mut terrain_in = self
            .inflight
            .iter()
            .filter(|(k, _)| *k == TERRAIN_KEY)
            .count();
        for pending in prioritised {
            match pending {
                PendingTile::Vector { tile, .. } => {
                    if self.recently_failed.contains_key(&tile)
                        || vector_in >= MAX_INFLIGHT_PER_LAYER
                    {
                        continue;
                    }
                    if self.inflight.insert((VECTOR_LAYER_ID, tile)) {
                        self.vector_pump.spawn_fetch(tile);
                        vector_in += 1;
                    }
                }
                PendingTile::Raster { tile, .. } => {
                    if self.recently_failed.contains_key(&tile)
                        || raster_in >= MAX_INFLIGHT_PER_LAYER
                    {
                        continue;
                    }
                    if self.inflight.insert((RASTER_LAYER_ID, tile)) {
                        self.raster_pump.spawn_fetch(tile);
                        raster_in += 1;
                    }
                }
                // Hillshade is fed by terrain DEM tiles, not its
                // own pump — drop these.
                PendingTile::Hillshade { .. } => {}
                PendingTile::Terrain { tile } => {
                    let Some(dem_pump) = self.dem_pump.as_ref() else {
                        continue;
                    };
                    if self.recently_failed.contains_key(&tile)
                        || terrain_in >= MAX_INFLIGHT_PER_LAYER
                    {
                        continue;
                    }
                    if self.inflight.insert((TERRAIN_KEY, tile)) {
                        dem_pump.spawn_fetch(tile);
                        terrain_in += 1;
                    }
                }
            }
        }
    }

    /// Snapshot for the render scheduler. Read once per tick
    /// in `App::about_to_wait`.
    pub fn workload(&self) -> Workload {
        Workload {
            workers_have_data: !self.vector_pump.rx.is_empty()
                || !self.raster_pump.rx.is_empty()
                || self
                    .dem_pump
                    .as_ref()
                    .map(|p| !p.rx.is_empty())
                    .unwrap_or(false),
            workers_in_flight: !self.inflight.is_empty(),
            map_animating: self.map.is_animating(),
        }
    }
}

fn tile_distance_to(p: &PendingTile, centre: turbomap_core::WorldPoint) -> f64 {
    let tile = match p {
        PendingTile::Vector { tile, .. }
        | PendingTile::Raster { tile, .. }
        | PendingTile::Hillshade { tile, .. }
        | PendingTile::Terrain { tile } => tile,
    };
    let n = (1u64 << tile.z) as f64;
    let cx = (tile.x as f64 + 0.5) / n;
    let cy = (tile.y as f64 + 0.5) / n;
    let dx = cx - centre.x;
    let dy = cy - centre.y;
    (dx * dx + dy * dy).sqrt()
}

/// Convenience helper to construct a `MapHost` plus its pumps
/// from a set of tile sources + a vector style. Used by
/// `App::resumed` so that constructor doesn't need to spell
/// out the pump wiring.
pub fn build(
    map: Map,
    vector_source: Arc<dyn VectorTileSource>,
    raster_source: Arc<dyn TileSource>,
    dem_source: Option<Arc<dyn TileSource>>,
    style: VectorStyle,
    vector_workers: usize,
    raster_workers: usize,
    dem_workers: usize,
) -> MapHost {
    let vector_pump = VectorFetchPump::new(vector_source, style, vector_workers);
    let raster_pump = RasterFetchPump::new(raster_source, raster_workers);
    let dem_pump = dem_source.map(|d| RasterFetchPump::new(d, dem_workers));
    MapHost::new(map, vector_pump, raster_pump, dem_pump)
}
