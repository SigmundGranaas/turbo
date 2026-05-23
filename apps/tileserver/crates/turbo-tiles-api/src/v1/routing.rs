//! Routing endpoints backed by `turbo-tiles-routing` (pgRouting under
//! the hood). `loop` stays 501 — that's M7 stretch.

use axum::extract::State;
use axum::http::StatusCode;
use axum::response::IntoResponse;
use axum::Json;
use serde::Deserialize;
use serde_json::{json, Value};
use turbo_tiles_core::route::Profile;
use turbo_tiles_routing::{
    find_route, isochrone, IsochroneRequest, LonLat, RoutePreferences, RoutingError,
};

use crate::error::ApiError;
use crate::state::ApiState;

#[derive(Debug, Deserialize)]
pub struct RouteRequest {
    pub from: [f64; 2],
    pub to: [f64; 2],
    pub profile: Profile,
    #[serde(default)]
    pub preferences: PreferencesIn,
}

#[derive(Debug, Default, Deserialize)]
pub struct PreferencesIn {
    #[serde(default)]
    pub avoid_unmarked: bool,
    #[serde(default)]
    pub prefer_marked: bool,
}

pub async fn route(
    State(state): State<ApiState>,
    Json(req): Json<RouteRequest>,
) -> Result<Json<Value>, ApiError> {
    let prefs = RoutePreferences {
        avoid_unmarked: req.preferences.avoid_unmarked,
        prefer_marked: req.preferences.prefer_marked,
    };
    let from = LonLat {
        lon: req.from[0],
        lat: req.from[1],
    };
    let to = LonLat {
        lon: req.to[0],
        lat: req.to[1],
    };
    match find_route(&state.db, from, to, req.profile, prefs).await {
        Ok(r) => Ok(Json(serde_json::to_value(r).unwrap_or(Value::Null))),
        Err(RoutingError::NoRouteFound) => Err(ApiError::NotFound),
        Err(RoutingError::NoNearbyNode) => Err(ApiError::BadRequest(
            "no nearby path node within 500m".into(),
        )),
        Err(RoutingError::Db(e)) => Err(ApiError::Db(e.to_string())),
    }
}

pub async fn isochrone_endpoint(
    State(state): State<ApiState>,
    Json(req): Json<IsochroneRequest>,
) -> Result<Json<Value>, ApiError> {
    match isochrone(&state.db, &req).await {
        Ok(v) => Ok(Json(v)),
        Err(RoutingError::NoNearbyNode) => Err(ApiError::BadRequest(
            "no nearby path node within 500m".into(),
        )),
        Err(RoutingError::NoRouteFound) => Err(ApiError::NotFound),
        Err(RoutingError::Db(e)) => Err(ApiError::Db(e.to_string())),
    }
}

pub async fn loop_route(Json(_req): Json<serde_json::Value>) -> impl IntoResponse {
    (
        StatusCode::NOT_IMPLEMENTED,
        Json(json!({"error": "routing/loop lands in M7"})),
    )
}

pub async fn profiles() -> Json<serde_json::Value> {
    Json(json!({
        "profiles": [
            {"id": "hiking", "label": {"nb": "Fottur", "en": "Hiking"}},
            {"id": "ski", "label": {"nb": "Ski", "en": "Skiing"}},
            {"id": "bike-gravel", "label": {"nb": "Grussykkel", "en": "Gravel cycling"}},
            {"id": "bike-road", "label": {"nb": "Landeveissykkel", "en": "Road cycling"}}
        ]
    }))
}
