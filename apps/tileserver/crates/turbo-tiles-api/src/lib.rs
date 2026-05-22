pub mod error;
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
