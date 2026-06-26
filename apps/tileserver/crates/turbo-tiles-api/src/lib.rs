// The solver recorder/tracer Arcs that reach the streaming pathfind /
// route handlers are thread-local plumbing (their inner type isn't
// Send+Sync); Arc keeps the recorder API uniform across call sites.
#![allow(clippy::arc_with_non_send_sync)]

pub mod crash_dump;
pub mod dem_tile_cache;
pub mod error;
pub mod fonts;
pub mod mvt_tile_cache;
pub mod sprite;
pub mod state;
pub mod v1;

use axum::routing::get;
use axum::Router;
use tower_http::compression::CompressionLayer;
use tower_http::cors::CorsLayer;
use tower_http::trace::TraceLayer;

pub use state::ApiState;

pub fn router(state: ApiState) -> Router {
    Router::new()
        .route("/healthz", get(health))
        .route("/readyz", get(ready))
        // Root-mounted SDF glyphs (the basemap style references {base}/fonts/…).
        .route("/fonts/:fontstack/:range", get(fonts::glyphs))
        // Icon sprite sheet (json/png × 1x/2x) for the place layers.
        .route("/sprite.json", get(sprite::json_1x))
        .route("/sprite.png", get(sprite::png_1x))
        .route("/sprite@2x.json", get(sprite::json_2x))
        .route("/sprite@2x.png", get(sprite::png_2x))
        .nest("/v1", v1::router())
        // Negotiated response compression (br/zstd/gzip/deflate per the
        // client's Accept-Encoding). The big public payloads — vector MVT
        // basemap tiles and the JSON (search, catalog, elev, TileJSON) — are
        // highly compressible (protobuf + text), so this is a large bandwidth
        // win to the Cloudflare origin-pull and to direct clients. tower-http's
        // DefaultPredicate skips already-compressed bodies (image/* — the PNG
        // sprites/rasters — and tiny responses), so it never wastes CPU
        // double-compressing PNGs. Caches store the plain bytes; this layer
        // compresses on the way out (origin fetches are ~once per tile behind
        // the CDN, so the per-tile cost is amortized).
        .layer(CompressionLayer::new())
        .layer(TraceLayer::new_for_http())
        .layer(CorsLayer::permissive())
        .with_state(state)
}

async fn health() -> &'static str {
    "ok"
}

async fn ready(
    axum::extract::State(state): axum::extract::State<ApiState>,
) -> Result<&'static str, error::ApiError> {
    sqlx::query("SELECT 1")
        .execute(&state.db)
        .await
        .map_err(|e| error::ApiError::Db(e.to_string()))?;
    Ok("ready")
}
