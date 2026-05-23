//! Admin panel JSON API.
//!
//! The HTMX direction was reversed — the SPA at `apps/admin/` (Vite +
//! React) talks to these endpoints. All routes require role `curator`
//! or `admin`. Cookie- and bearer-token auth both work, so the SPA
//! can rely on the `access_token` cookie the .NET auth service sets.
//!
//! Surface:
//!   GET    /admin/api/resources                       — counts per resource
//!   GET    /admin/api/resources/{resource}            — paginated curated_route list
//!   GET    /admin/api/resources/{resource}/{id}       — single route + GeoJSON
//!   POST   /admin/api/resources/{resource}            — create
//!   PUT    /admin/api/resources/{resource}/{id}       — update metadata
//!   DELETE /admin/api/resources/{resource}/{id}       — archive (soft)
//!   POST   /admin/api/upload-gpx                      — multipart, parse to draft
//!   POST   /admin/api/ingest/{job}/trigger            — fire-and-forget job
//!   GET    /admin/api/ingest/jobs                     — job log
//!
//! The SPA itself is served by the `bin` crate as a static file
//! mount at `/admin/*` using `tower-http::ServeDir`.

pub mod error;
pub mod routes;
pub mod state;

pub use state::AdminState;

use axum::routing::{delete, get, post, put};
use axum::Router;

pub fn router(state: AdminState) -> Router {
    Router::new()
        .route("/api/resources", get(routes::resources::summary))
        .route("/api/resources/:resource", get(routes::resources::list))
        .route("/api/resources/:resource", post(routes::resources::create))
        .route(
            "/api/resources/:resource/:id",
            get(routes::resources::detail),
        )
        .route(
            "/api/resources/:resource/:id",
            put(routes::resources::update),
        )
        .route(
            "/api/resources/:resource/:id",
            delete(routes::resources::archive),
        )
        .route("/api/upload-gpx", post(routes::upload::upload_gpx))
        .route("/api/ingest/:job/trigger", post(routes::ingest::trigger))
        .route("/api/ingest/jobs", get(routes::ingest::jobs))
        .with_state(state)
}
