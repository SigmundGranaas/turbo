use axum::http::StatusCode;
use axum::response::{IntoResponse, Response};
use axum::Json;
use serde_json::json;

#[derive(Debug, thiserror::Error)]
pub enum AdminError {
    #[error("not found")]
    NotFound,
    #[error("bad request: {0}")]
    BadRequest(String),
    #[error("database error: {0}")]
    Db(String),
    #[error("unauthorized: {0}")]
    Auth(String),
    #[error("upload too large or malformed: {0}")]
    Upload(String),
}

impl IntoResponse for AdminError {
    fn into_response(self) -> Response {
        let (status, msg) = match &self {
            AdminError::NotFound => (StatusCode::NOT_FOUND, "not found".to_string()),
            AdminError::BadRequest(m) => (StatusCode::BAD_REQUEST, m.clone()),
            AdminError::Auth(m) => (StatusCode::UNAUTHORIZED, m.clone()),
            AdminError::Upload(m) => (StatusCode::UNPROCESSABLE_ENTITY, m.clone()),
            AdminError::Db(_) => (StatusCode::INTERNAL_SERVER_ERROR, "database error".into()),
        };
        if matches!(self, AdminError::Db(_)) {
            tracing::error!(error = %self, "admin error");
        }
        (status, Json(json!({"error": msg}))).into_response()
    }
}

impl From<sqlx::Error> for AdminError {
    fn from(e: sqlx::Error) -> Self {
        AdminError::Db(e.to_string())
    }
}

impl From<turbo_tiles_auth::AuthError> for AdminError {
    fn from(e: turbo_tiles_auth::AuthError) -> Self {
        AdminError::Auth(e.to_string())
    }
}
