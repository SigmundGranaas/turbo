use axum::routing::{get, post};
use axum::Router;

use crate::state::ApiState;

mod basemap;
mod catalog;
mod dem;
mod elev;
mod inspect;
mod mask;
mod pathfind;
mod raster;
mod resource;
mod route;
mod route_plan;
mod search;
mod slope;
mod tiles;

// Per-primitive endpoint modules land here as stages complete:
//   Stage 1: elev      ✓
//   Stage 3: mask
//   Stage 4: route     (replaces the old /routing/* group)
//   Stage 5: search
//   Stage 6: pathfind
//
// Debug endpoints per primitive are siblings under `/v1/debug/*`.

pub fn router() -> Router<ApiState> {
    Router::new()
        .route("/catalog", get(catalog::catalog))
        // Multi-layer N50 topo basemap (water/landcover/contour/building/
        // transportation/place) + its TileJSON descriptor. Registered before
        // the generic `/:resource/...` routes so `basemap` isn't captured as
        // a resource slug.
        .route("/basemap", get(basemap::describe))
        .route("/basemap/style.json", get(basemap::style))
        .route("/basemap/:z/:x/:y.mvt", get(basemap::tile))
        // Raster fallback: same data + style, rasterised at the origin.
        .route("/raster/n50/:z/:x/:y.png", get(raster::tile))
        .route("/:resource/tiles/:z/:x/:y.mvt", get(tiles::tile))
        .route("/:resource", get(resource::list))
        .route("/:resource/:id", get(resource::detail))
        // Stage 1: elevation primitive
        .route("/elev/sample", post(elev::sample))
        .route("/elev/profile", post(elev::profile))
        .route("/debug/elev/coverage", get(elev::coverage))
        .route("/debug/elev/bench", get(elev::bench))
        // Stage 1b: DEM raster tiles. Mapbox Terrain-RGB encoding so
        // a generic web-mercator client (turbomap, MapLibre, …) can
        // pull our DTM at any zoom and run hillshade / 3D-terrain on
        // the GPU side without needing access to the raw GeoTIFFs.
        .route("/dem/rgb/:z/:x/:y_ext", get(dem::rgb))
        // Stage 2: slope + aspect (derived from DEM)
        .route("/slope/sample", post(slope::sample))
        // Slope-angle (bratthet) overlay tiles from our own DEM — the
        // self-hosted replacement for the NVE steepness WMTS.
        .route("/slope/tiles/:z/:x/:y.png", get(slope::tile))
        .route("/slope/along", post(slope::along))
        // Stage 3: refusal mask
        .route("/mask/sample", post(mask::sample))
        .route("/debug/mask/coverage", get(mask::coverage))
        // Stage 4: routing graph
        .route("/route", post(route::route))
        .route("/debug/graph/stats", get(route::stats))
        .route("/debug/graph/density", get(route::density))
        // Stage 5: anchor search
        .route("/search/nearest", post(search::nearest))
        .route("/search/name", get(search::name))
        .route("/debug/search/coverage", get(search::coverage))
        // Stage 6: off-trail pathfinding (composes elev + mask + graph)
        // Curated, app-facing routing API (stable contract, gateway /api/route/*):
        .route("/route/plan", post(route_plan::plan))
        .route("/route/plan/stream", post(route_plan::plan_stream))
        .route("/route/presets", get(pathfind::presets))
        // Internal/admin solve surface (debug, recording, layer weights):
        .route("/pathfind", post(pathfind::pathfind))
        .route("/pathfind/record", post(pathfind::pathfind_record))
        .route("/pathfind/stream", post(pathfind::pathfind_stream))
        .route("/debug/pathfind/layers", get(pathfind::layers))
        .route("/debug/pathfind/inspect", post(pathfind::inspect))
        .route("/debug/pathfind/cell", post(pathfind::cell_inspect))
        .route("/debug/cost-breakdown", post(pathfind::cost_breakdown))
        .route("/debug/cost-config", get(pathfind::cost_config))
        .route("/debug/recent-crashes", get(pathfind::recent_crashes))
        .route("/debug/induce-panic", post(pathfind::induce_panic))
        // Viewport-bbox data inspectors — every primitive layer
        // gets a SPA-renderable overlay so curators can see exactly
        // what data is driving pathfinding decisions where.
        .route("/debug/data/water", get(inspect::mask_water))
        .route("/debug/data/wetland", get(inspect::landcover_wetland))
        .route("/debug/data/forest", get(inspect::landcover_forest))
        .route("/debug/data/edges", get(inspect::edges))
        .route("/debug/data/anchors", get(inspect::anchors))
}
