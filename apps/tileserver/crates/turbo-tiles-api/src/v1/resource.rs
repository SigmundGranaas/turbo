use axum::extract::{Path, Query, State};
use axum::Json;
use serde::Deserialize;
use serde_json::{json, Value};
use turbo_tiles_core::bbox::Bbox;
use turbo_tiles_core::resource::Resource;

use crate::error::ApiError;
use crate::state::ApiState;

#[derive(Debug, Deserialize)]
pub struct ListQuery {
    pub bbox: String,
    #[serde(default = "default_limit")]
    pub limit: i64,
}

fn default_limit() -> i64 {
    500
}

pub async fn list(
    State(state): State<ApiState>,
    Path(resource): Path<String>,
    Query(q): Query<ListQuery>,
) -> Result<Json<Value>, ApiError> {
    let resource: Resource = resource
        .parse()
        .map_err(|_| ApiError::BadRequest(format!("unknown resource `{resource}`")))?;
    let bbox: Bbox = q
        .bbox
        .parse()
        .map_err(|e: turbo_tiles_core::bbox::BboxParseError| ApiError::BadRequest(e.to_string()))?;
    let limit = q.limit.clamp(1, 5000);

    let rows = turbo_tiles_mvt::list_by_bbox(&state.db, resource, bbox, limit).await?;
    let features: Vec<Value> = rows
        .into_iter()
        .map(|r| {
            json!({
                "type": "Feature",
                "id": r.id,
                "geometry": r.geometry,
                "properties": r.properties,
            })
        })
        .collect();
    Ok(Json(json!({
        "type": "FeatureCollection",
        "features": features,
    })))
}

pub async fn detail(
    State(state): State<ApiState>,
    Path((resource, id)): Path<(String, String)>,
) -> Result<Json<Value>, ApiError> {
    let resource: Resource = resource
        .parse()
        .map_err(|_| ApiError::BadRequest(format!("unknown resource `{resource}`")))?;
    let row = turbo_tiles_mvt::feature_by_id(&state.db, resource, &id)
        .await?
        .ok_or(ApiError::NotFound)?;
    Ok(Json(json!({
        "type": "Feature",
        "id": row.id,
        "geometry": row.geometry,
        "properties": row.properties,
    })))
}
