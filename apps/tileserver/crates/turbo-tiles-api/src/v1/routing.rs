//! Routing endpoints. M5 lands the actual pgRouting integration —
//! everything here returns 501 in V1 with a documented contract so the
//! Flutter side can wire UI against stable shapes.
#![allow(dead_code)]

use axum::http::StatusCode;
use axum::response::IntoResponse;
use axum::Json;
use serde::{Deserialize, Serialize};
use serde_json::json;

#[derive(Debug, Deserialize)]
pub struct RouteRequest {
    pub from: [f64; 2],
    pub to: [f64; 2],
    pub profile: String,
    #[serde(default)]
    pub preferences: serde_json::Value,
}

#[derive(Debug, Serialize)]
pub struct RouteResponse {
    pub geom: serde_json::Value,
    pub distance_m: f64,
    pub duration_s: f64,
    pub elevation_gain_m: f64,
    pub edge_ids: Vec<i64>,
    pub warnings: Vec<String>,
}

pub async fn route(Json(_req): Json<RouteRequest>) -> impl IntoResponse {
    (
        StatusCode::NOT_IMPLEMENTED,
        Json(json!({"error": "routing/route lands in M5"})),
    )
}

pub async fn isochrone(Json(_req): Json<serde_json::Value>) -> impl IntoResponse {
    (
        StatusCode::NOT_IMPLEMENTED,
        Json(json!({"error": "routing/isochrone lands in M5"})),
    )
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
