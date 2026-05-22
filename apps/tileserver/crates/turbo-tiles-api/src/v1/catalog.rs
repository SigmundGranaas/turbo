use axum::extract::State;
use axum::Json;
use serde_json::Value;
use turbo_tiles_core::resource::Resource;

use crate::state::ApiState;

pub async fn catalog(State(state): State<ApiState>) -> Json<Value> {
    let resources: Vec<Value> = Resource::ALL
        .iter()
        .map(|r| serde_json::to_value(r.descriptor(&state.public_base_url)).unwrap())
        .collect();
    Json(serde_json::json!({ "resources": resources }))
}
