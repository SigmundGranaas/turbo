use axum::http::StatusCode;
use axum::response::{IntoResponse, Response};
use axum::Json;
use serde_json::json;

#[derive(Debug, thiserror::Error)]
pub enum ApiError {
    #[error("not found")]
    NotFound,
    #[error("bad request: {0}")]
    BadRequest(String),
    #[error("database error: {0}")]
    Db(String),
    #[error(transparent)]
    Auth(#[from] turbo_tiles_auth::AuthError),
}

impl IntoResponse for ApiError {
    fn into_response(self) -> Response {
        let (status, message) = match &self {
            ApiError::NotFound => (StatusCode::NOT_FOUND, "not found".to_string()),
            ApiError::BadRequest(m) => (StatusCode::BAD_REQUEST, m.clone()),
            ApiError::Db(_) => (StatusCode::INTERNAL_SERVER_ERROR, "database error".into()),
            ApiError::Auth(e) => return e.clone_into_response(),
        };
        if matches!(self, ApiError::Db(_)) {
            tracing::error!(error = %self, "api error");
        }
        (status, Json(json!({"error": message}))).into_response()
    }
}

// Workaround: AuthError is not Clone, so propagate by re-rendering.
trait CloneIntoResponse {
    fn clone_into_response(&self) -> Response;
}

impl CloneIntoResponse for turbo_tiles_auth::AuthError {
    fn clone_into_response(&self) -> Response {
        let status = match self {
            turbo_tiles_auth::AuthError::MissingToken | turbo_tiles_auth::AuthError::Invalid(_) => {
                StatusCode::UNAUTHORIZED
            }
            turbo_tiles_auth::AuthError::MissingRole => StatusCode::FORBIDDEN,
        };
        let body = json!({"error": self.to_string()});
        (status, Json(body)).into_response()
    }
}

impl From<sqlx::Error> for ApiError {
    fn from(e: sqlx::Error) -> Self {
        ApiError::Db(e.to_string())
    }
}
