use axum::routing::{get, post};
use axum::Router;

use crate::state::ApiState;

mod catalog;
mod resource;
mod routing;
mod tiles;

pub fn router() -> Router<ApiState> {
    Router::new()
        .route("/catalog", get(catalog::catalog))
        .route("/:resource/tiles/:z/:x/:y.mvt", get(tiles::tile))
        .route("/:resource", get(resource::list))
        .route("/:resource/:id", get(resource::detail))
        .route("/routing/route", post(routing::route))
        .route("/routing/isochrone", post(routing::isochrone_endpoint))
        .route("/routing/loop", post(routing::loop_route))
        .route("/routing/profiles", get(routing::profiles))
}
