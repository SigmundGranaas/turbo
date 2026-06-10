// The solver recorder/tracer Arcs that reach the streaming pathfind /
// route handlers are thread-local plumbing (their inner type isn't
// Send+Sync); Arc keeps the recorder API uniform across call sites.
#![allow(clippy::arc_with_non_send_sync)]

pub mod crash_dump;
pub mod error;
pub mod fonts;
pub mod sprite;
pub mod state;
pub mod v1;

use axum::routing::get;
use axum::Router;
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
