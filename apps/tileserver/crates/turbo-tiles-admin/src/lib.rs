//! Admin panel. HTMX + Askama, server-rendered. Lands in M3-M4.
//!
//! V1 surface:
//!   GET    /admin                                — dashboard
//!   GET    /admin/resources/{resource}           — listing
//!   GET    /admin/resources/{resource}/{id}      — edit form
//!   PUT    /admin/resources/{resource}/{id}      — update
//!   POST   /admin/resources/{resource}           — create
//!   DELETE /admin/resources/{resource}/{id}      — archive
//!   POST   /admin/upload-gpx                     — multipart upload
//!   POST   /admin/ingest/{job_name}/trigger      — kick off a job
//!   GET    /admin/ingest/jobs                    — job history

use axum::extract::FromRef;
use axum::routing::get;
use axum::Router;

use turbo_tiles_auth::{AuthState, Curator, RequireRole};

#[derive(Clone)]
pub struct AdminState {
    pub db: turbo_tiles_db::DbPool,
    pub auth: AuthState,
}

impl FromRef<AdminState> for AuthState {
    fn from_ref(input: &AdminState) -> Self {
        input.auth.clone()
    }
}

pub fn router(state: AdminState) -> Router {
    Router::new().route("/", get(dashboard)).with_state(state)
}

async fn dashboard(_: RequireRole<Curator>) -> &'static str {
    // Replaced with an Askama template in M3.
    "tileserver admin · M3"
}
