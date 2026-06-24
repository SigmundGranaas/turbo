use std::sync::Arc;

use axum::extract::FromRef;
use turbo_tiles_auth::{AuthConfig, AuthState};
use turbo_tiles_db::DbPool;

/// Server state passed to every handler.
///
/// Stage 0 reset: the recommendation engine bundle (`TerrainServices`)
/// is gone. The new primitive handles land per stage:
///   - Stage 1: `dem: Option<Arc<Dem>>` (elevation primitive)
///   - Stage 3: `mask: Option<Arc<Mask>>`
///   - Stage 4: `graph: Option<Arc<Graph>>`
///   - Stage 5: `search: Option<Arc<Index>>`
///   - Stage 6: `pathfinder: Option<Arc<Pathfinder>>`
///
/// Each is an `Option` so the server boots in degraded mode when an
/// artifact isn't present; the affected endpoint returns 503 instead
/// of refusing to start.
///
/// `db` stays required for now; Stage 7 introduces `--no-db` mode
/// that drops the legacy catalog/resource/tiles endpoints.
#[derive(Clone)]
pub struct ApiState {
    pub db: DbPool,
    pub auth: AuthState,
    pub public_base_url: Arc<String>,
    pub dem: Option<Arc<turbo_tiles_elev::Dem>>,
    pub mask: Option<Arc<turbo_tiles_mask::Mask>>,
    pub graph: Option<Arc<turbo_tiles_graph::Graph>>,
    pub search: Option<Arc<turbo_tiles_search::Index>>,
    /// Auto-loaded landcover masks keyed by short class name
    /// (`wetland`, `forest`, …). Same `Mask` format as the
    /// water/glacier `mask` field; segregated by class so the SPA
    /// inspect endpoints can render them as separate overlays
    /// without scanning the pathfinder's `CostLayer` stack.
    pub landcover: std::collections::HashMap<&'static str, Arc<turbo_tiles_mask::Mask>>,
    /// Set at boot once the primitive artifacts are loaded. Holds
    /// the registered `CostLayer` stack — additions (custom layers
    /// for marsh, ridges, etc.) get plugged in at construction.
    pub pathfinder: Option<Arc<turbo_tiles_pathfind::Pathfinder>>,
    /// Named trip presets (Balanced, Avoid roads, …) resolved at boot
    /// from `tools/route-presets.toml`. A request's `preset` field maps
    /// to one of these; the SPA lists them in its "Trip style" dropdown.
    pub presets: Arc<turbo_tiles_pathfind::PresetSet>,
    /// Multi-layer N50 basemap definition, resolved at boot from
    /// `tools/basemap-layers.toml` (embedded fallback). Drives
    /// `/v1/basemap/{z}/{x}/{y}.mvt`.
    pub basemap: Arc<turbo_tiles_mvt::BasemapConfig>,
    /// The house style parsed for the server-side raster renderer
    /// (`/v1/raster/n50/...`). Same document as `/v1/basemap/style.json`.
    pub raster_style: Arc<turbo_tiles_raster::RasterStyle>,
    /// Global routing-solve concurrency cap. Each Pathfinder solve
    /// holds corridor + trail-graph scratch (MBs) for its duration and
    /// the default blocking pool allows 512 threads — so without a cap,
    /// N concurrent long solves are the realistic OOM path on a small
    /// node. At most `TILESERVER_ROUTING_CONCURRENCY` solves (default:
    /// CPU cores) run at once; excess requests await a permit cheaply
    /// instead of all allocating at once.
    pub routing_permits: Arc<tokio::sync::Semaphore>,
    /// Write-through cache (RAM + SSD) of rendered Terrain-RGB DEM tiles, plus
    /// the render-concurrency throttle. The DTM is immutable per deploy, so
    /// rendered tiles are reusable forever — a hit serves in ~1ms and is not
    /// CPU-bound, eliminating the per-tile re-render that made `/v1/dem`
    /// congestion-collapse under load. See [`crate::dem_tile_cache`].
    pub dem_tiles: crate::dem_tile_cache::DemTileCache,
    /// Cache of rendered MVT vector tiles (basemap + curated resources). The MVT
    /// render is CPU-heavy (per-feature simplify over large polygons) and the
    /// tiles are immutable between provisions, so caching turns a multi-second
    /// query into a memory hit and bounds concurrent cold renders. Invalidated by
    /// `bump_version` on (re)provision. See [`crate::mvt_tile_cache`].
    pub mvt_tiles: crate::mvt_tile_cache::MvtTileCache,
    /// Cache of rendered **raster** PNG tiles (`/v1/raster/n50/...`). Same two-
    /// tier LRU + render-concurrency limiter as `mvt_tiles`; the raster render is
    /// even costlier (rasterise + hillshade) and low-zoom tiles can exceed the
    /// pool's statement timeout cold, so caching the bytes is what makes the
    /// low-zoom basemap viable. Invalidated by `bump_version` on (re)provision.
    pub raster_tiles: crate::mvt_tile_cache::MvtTileCache,
    /// False until the N50 basemap has data in the DB. While false the
    /// `/v1/basemap` tile endpoint returns **503** instead of a (cacheable)
    /// `200`-with-empty body, so a client retries rather than caching an empty
    /// tile during a fresh-deploy provision. (An empty MVT cached client-side
    /// ingests as a valid-but-empty tile and is never refetched — the bug this
    /// guards against.) Flipped true by the boot readiness probe / boot-provision
    /// completion (see the binary's `main`).
    pub basemap_ready: Arc<std::sync::atomic::AtomicBool>,
}

impl ApiState {
    pub fn new(db: DbPool, auth: AuthConfig, public_base_url: String) -> Self {
        let permits = std::env::var("TILESERVER_ROUTING_CONCURRENCY")
            .ok()
            .and_then(|v| v.parse::<usize>().ok())
            .filter(|&n| n > 0)
            .unwrap_or_else(|| {
                std::thread::available_parallelism()
                    .map(|n| n.get())
                    .unwrap_or(4)
            });
        Self {
            db,
            auth: AuthState(Arc::new(auth)),
            public_base_url: Arc::new(public_base_url),
            dem: None,
            mask: None,
            graph: None,
            search: None,
            landcover: std::collections::HashMap::new(),
            pathfinder: None,
            presets: Arc::new(turbo_tiles_pathfind::PresetSet::load_or_default()),
            basemap: Arc::new(turbo_tiles_mvt::BasemapConfig::load_or_default()),
            raster_style: Arc::new(
                turbo_tiles_raster::RasterStyle::load_or_default()
                    .expect("embedded n50-topo style must parse"),
            ),
            routing_permits: Arc::new(tokio::sync::Semaphore::new(permits)),
            dem_tiles: crate::dem_tile_cache::DemTileCache::from_env(),
            mvt_tiles: crate::mvt_tile_cache::MvtTileCache::from_env(),
            raster_tiles: crate::mvt_tile_cache::MvtTileCache::png_from_env(),
            basemap_ready: Arc::new(std::sync::atomic::AtomicBool::new(false)),
        }
    }

    /// Acquire a routing-solve permit (see `routing_permits`). Await is
    /// cheap — the request queues instead of allocating solver scratch.
    /// Hold the returned permit for the duration of the blocking solve
    /// (move it into the `spawn_blocking` closure).
    pub async fn acquire_routing_permit(&self) -> tokio::sync::OwnedSemaphorePermit {
        self.routing_permits
            .clone()
            .acquire_owned()
            .await
            .expect("routing semaphore is never closed")
    }
}

impl FromRef<ApiState> for AuthState {
    fn from_ref(input: &ApiState) -> Self {
        input.auth.clone()
    }
}
